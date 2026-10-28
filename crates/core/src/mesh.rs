//! Meshing plane parts: the outlines (arcs split by the local size), point targets as vertices,
//! a constrained Delaunay triangulation refined to a minimum angle and a largest size (spade, MIT
//! or Apache-2.0), triangles kept where there is material, and 6-node triangles with midside nodes
//! on the true arcs. Every boundary edge of the mesh knows which edge of which shape it lies on, so
//! supports and loads find their nodes.

use crate::model::{arc, Geom, Model, Target};
use serde::Serialize;
use spade::{ConstrainedDelaunayTriangulation, Point2, RefinementParameters, AngleLimit, Triangulation};

/// One edge of an element on the outline: its end nodes and (for 6-node triangles) the middle one.
#[derive(Clone, Debug, Serialize)]
pub struct BEdge {
    pub shape: u32,
    pub edge: u32,
    /// [start, middle, end]; middle is u32::MAX for 3-node triangles.
    pub n: [u32; 3],
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Quality {
    /// Smallest angle of any triangle (degrees).
    pub min_angle: f64,
    /// Largest aspect ratio (longest edge over the height an equilateral triangle would need; 1 is perfect).
    pub max_aspect: f64,
    /// Share of triangles with every angle over 30°.
    pub good: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Mesh {
    pub nodes: Vec<[f64; 2]>,
    /// Corner nodes first, counter-clockwise; then the midside nodes of edges 0–1, 1–2, 2–0 (6-node only).
    pub tris: Vec<Vec<u32>>,
    pub order: u8,
    /// Material id of every element.
    pub mat: Vec<u32>,
    pub edges: Vec<BEdge>,
    /// The node at each point target (supports and loads at points), by (x, y).
    pub points: Vec<([f64; 2], u32)>,
    /// The global element size used (m).
    pub size: f64,
    pub quality: Quality,
}

/// A piece of the outline between two inserted points: on which shape edge, and its arc if curved.
#[derive(Clone, Copy)]
struct Seg {
    shape: u32,
    edge: u32,
    a: [f64; 2],
    b: [f64; 2],
    /// Centre and radius when the edge is an arc.
    arc: Option<([f64; 2], f64)>,
}

/// The largest number of triangles a mesh may have.
pub const MAX_ELEMENTS: usize = 400_000;

/// Points along edge `i` of a shape, at most `h` apart, with the arc of the edge if it has one.
fn edge_points(g: &Geom, i: usize, h: f64) -> (Vec<[f64; 2]>, Option<([f64; 2], f64)>) {
    match g {
        Geom::Circle { c, r } => {
            let n = ((std::f64::consts::TAU * r / h).ceil() as usize).clamp(24, 20_000);
            let mut v: Vec<[f64; 2]> = (0..=n).map(|k| { let t = k as f64 / n as f64 * std::f64::consts::TAU; [c[0] + r * t.cos(), c[1] + r * t.sin()] }).collect();
            // Closed exactly: cos 2π is not quite 1, and a gap of 1e-17 lets the outside leak in.
            v[n] = v[0];
            (v, Some((*c, *r)))
        }
        Geom::Poly { pts, bulge } => {
            let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
            let b = bulge.get(i).copied().unwrap_or(0.0);
            if b == 0.0 {
                let l = (q[0] - p[0]).hypot(q[1] - p[1]);
                let n = ((l / h).ceil() as usize).clamp(1, 20_000);
                return ((0..=n).map(|k| { let t = k as f64 / n as f64; [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t] }).collect(), None);
            }
            let (cen, r, a0, th) = arc(p, q, b);
            // At least 32 pieces per full turn, so holes and fillets stay round.
            let n = ((r * th.abs() / h).ceil() as usize).max((th.abs() / std::f64::consts::TAU * 32.0).ceil() as usize).clamp(2, 20_000);
            let mut v: Vec<[f64; 2]> = (0..=n).map(|k| { let t = a0 + th * k as f64 / n as f64; [cen[0] + r * t.cos(), cen[1] + r * t.sin()] }).collect();
            // The ends exactly as given.
            v[0] = p;
            v[n] = q;
            (v, Some((cen, r)))
        }
    }
}

fn dist_seg(p: [f64; 2], s: &Seg) -> f64 {
    if let Some((c, r)) = s.arc {
        // On the arc's circle and between the piece's ends.
        let d = ((p[0] - c[0]).hypot(p[1] - c[1]) - r).abs();
        let chord = (s.b[0] - s.a[0]).hypot(s.b[1] - s.a[1]);
        let to_ends = (p[0] - s.a[0]).hypot(p[1] - s.a[1]).max((p[0] - s.b[0]).hypot(p[1] - s.b[1]));
        return if to_ends <= chord * 1.0001 + 1e-15 { d } else { f64::INFINITY };
    }
    let (dx, dy) = (s.b[0] - s.a[0], s.b[1] - s.a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - s.a[0]) * dx + (p[1] - s.a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (s.a[0] + t * dx - p[0]).hypot(s.a[1] + t * dy - p[1])
}

fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

/// The automatic element size: about a twenty-fifth of the part's larger side.
pub fn auto_size(m: &Model) -> f64 {
    m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1]) / 25.0).unwrap_or(0.01)
}

/// Mesh the model's shapes.
pub fn mesh(m: &Model) -> Result<Mesh, String> {
    m.check()?;
    let solids: Vec<_> = m.shapes.iter().filter(|s| !s.hole).collect();
    if solids.is_empty() {
        return Err("There is nothing to mesh: draw the part's outline first (Geometry › Rectangle, Circle or Polygon).".into());
    }
    let bounds = m.bounds().ok_or("The part is empty.")?;
    let diag = (bounds[2] - bounds[0]).hypot(bounds[3] - bounds[1]);
    let h = if m.mesh.size > 0.0 { m.mesh.size } else { auto_size(m) };
    // A rough element count before meshing: refuse sizes that would make millions of triangles.
    let area: f64 = solids.iter().map(|s| s.geom.area().abs()).sum();
    if area / (h * h * 0.43) > MAX_ELEMENTS as f64 {
        return Err(format!("The element size is too small for this part: it would make over {} triangles. Use a larger size, and refine only where it matters.", MAX_ELEMENTS));
    }
    let local = |shape: u32, edge: u32| -> f64 {
        m.mesh.refine.iter().filter(|r| r.at == Target::Edge { shape, edge }).map(|r| r.size).fold(h, f64::min)
    };
    let mut cdt: ConstrainedDelaunayTriangulation<Point2<f64>> = ConstrainedDelaunayTriangulation::new();
    let mut segs: Vec<Seg> = vec![];
    let ins = |cdt: &mut ConstrainedDelaunayTriangulation<Point2<f64>>, p: [f64; 2]| cdt.insert(Point2::new(p[0], p[1])).map_err(|e| format!("A point of the outline could not be meshed: {e:?}"));
    for s in &m.shapes {
        for e in 0..s.geom.edges() {
            let (pts, arc) = edge_points(&s.geom, e, local(s.id, e as u32));
            let mut prev = ins(&mut cdt, pts[0])?;
            for w in pts.windows(2) {
                let next = ins(&mut cdt, w[1])?;
                if prev != next {
                    cdt.add_constraint_and_split(prev, next, |p| p);
                }
                segs.push(Seg { shape: s.id, edge: e as u32, a: w[0], b: w[1], arc });
                prev = next;
            }
        }
    }
    // A frame round everything, so the hull is not made of near-collinear points of a sloping
    // edge (their slivers can never be refined); the triangles between frame and part are dropped.
    let pad = 2.0 * h;
    for p in [[bounds[0] - pad, bounds[1] - pad], [bounds[2] + pad, bounds[1] - pad], [bounds[2] + pad, bounds[3] + pad], [bounds[0] - pad, bounds[3] + pad]] {
        ins(&mut cdt, p)?;
    }
    // Point targets become vertices; refinement near a point adds rings that grow to the global size.
    let mut targets: Vec<[f64; 2]> = vec![];
    for t in m.supports.iter().map(|s| &s.at).chain(m.loads.iter().map(|l| &l.at)) {
        if let Target::Point { x, y } = t {
            targets.push([*x, *y]);
        }
    }
    for p in &targets {
        ins(&mut cdt, *p)?;
    }
    let max_area = h * h * 3f64.sqrt() / 4.0 * 1.3;
    // Triangles far smaller than the finest size asked for are left as they are (refinement could
    // otherwise chase slivers for ever); the bound must sit below the local sizes too.
    let hmin = m.mesh.refine.iter().map(|r| r.size).fold(h, f64::min);
    let params = || RefinementParameters::<f64>::new()
        .with_angle_limit(AngleLimit::from_deg(m.mesh.min_angle.clamp(5.0, 33.0)))
        .with_max_allowed_area(max_area)
        .with_min_required_area(hmin * hmin * 1e-4)
        .with_max_additional_vertices(MAX_ELEMENTS)
        .exclude_outer_faces(false);
    let mut res = cdt.refine(params());
    // Local sizes grade into the global one: the size wanted at a point is the local size plus a
    // fifth of the distance to where it was asked for. Triangles too large for the size at their
    // centre get a point there, and the mesh is refined again for shape, until all fit.
    let fine = (h / 8.0).min(diag / 400.0).max(diag * 1e-6);
    let solid_out: Vec<(u32, Vec<[f64; 2]>)> = solids.iter().map(|s| (s.material, s.geom.outline(fine, 64).0)).collect();
    let holes_out: Vec<Vec<[f64; 2]>> = m.shapes.iter().filter(|s| s.hole).map(|s| s.geom.outline(fine, 64).0).collect();
    let material = |c: [f64; 2]| solid_out.iter().any(|(_, o)| inside(o, c)) && !holes_out.iter().any(|o| inside(o, c));
    let local_segs: Vec<(Vec<Seg>, f64)> = m.mesh.refine.iter().filter_map(|r| match r.at {
        Target::Edge { shape, edge } => Some((segs.iter().filter(|s| s.shape == shape && s.edge == edge).copied().collect(), r.size)),
        Target::Point { x, y } => Some((vec![Seg { shape: 0, edge: 0, a: [x, y], b: [x, y], arc: None }], r.size)),
        _ => None,
    }).collect();
    const GROW: f64 = 0.2;
    let size_at = |p: [f64; 2]| local_segs.iter().fold(h, |acc, (ss, sz)| {
        // Far away the local size cannot matter: skip the distance work.
        let d = ss.iter().map(|s| { let mut s2 = *s; s2.arc = None; dist_seg(p, &s2) }).fold(f64::INFINITY, f64::min);
        acc.min(sz + GROW * d)
    });
    if !local_segs.is_empty() {
        for _ in 0..60 {
            let mut add = vec![];
            for f in cdt.inner_faces() {
                let v = f.vertices();
                let p: Vec<[f64; 2]> = v.iter().map(|x| { let q = x.position(); [q.x, q.y] }).collect();
                let l = (0..3).map(|i| (p[(i + 1) % 3][0] - p[i][0]).hypot(p[(i + 1) % 3][1] - p[i][1])).fold(0.0, f64::max);
                if l <= 1.3 * hmin {
                    continue;
                }
                let c = [(p[0][0] + p[1][0] + p[2][0]) / 3.0, (p[0][1] + p[1][1] + p[2][1]) / 3.0];
                if l > 1.3 * size_at(c) && material(c) {
                    add.push(c);
                }
            }
            if add.is_empty() {
                break;
            }
            if cdt.num_vertices() + add.len() > MAX_ELEMENTS {
                return Err(format!("The local element sizes would make over {MAX_ELEMENTS} triangles. Use larger local sizes."));
            }
            for p in add {
                ins(&mut cdt, p)?;
            }
            res = cdt.refine(params());
            if !res.refinement_complete {
                break;
            }
        }
    }
    // Which faces are material: inside a solid and in no hole (checked at the centroid).
    let mut map = vec![u32::MAX; cdt.num_vertices()];
    let mut nodes: Vec<[f64; 2]> = vec![];
    let mut tris: Vec<Vec<u32>> = vec![];
    let mut mat = vec![];
    // Every face is refined (spade's own inside/outside flood fill marked a whole heat sink as
    // outside), so all triangles are well shaped and the centroid decides reliably.
    for f in cdt.inner_faces() {
        let v = f.vertices();
        let p: Vec<[f64; 2]> = v.iter().map(|x| { let q = x.position(); [q.x, q.y] }).collect();
        let a2 = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if a2.abs() < 1e-12 * h * h {
            continue;
        }
        let c = [(p[0][0] + p[1][0] + p[2][0]) / 3.0, (p[0][1] + p[1][1] + p[2][1]) / 3.0];
        let Some(mt) = solid_out.iter().rev().find(|(_, o)| inside(o, c)).map(|x| x.0) else { continue };
        if holes_out.iter().any(|o| inside(o, c)) {
            continue;
        }
        let mut t = vec![];
        for x in &v {
            let i = x.fix().index();
            if map[i] == u32::MAX {
                map[i] = nodes.len() as u32;
                let q = x.position();
                nodes.push([q.x, q.y]);
            }
            t.push(map[i]);
        }
        tris.push(t);
        mat.push(mt);
    }
    if tris.is_empty() {
        return Err("The mesh has no triangles: check that the holes do not cover the whole part.".into());
    }
    if tris.len() > MAX_ELEMENTS {
        return Err(format!("The mesh would have over {MAX_ELEMENTS} triangles. Use a larger element size."));
    }
    if !res.refinement_complete {
        return Err("The mesh needed more triangles than allowed. Use a larger element size or a smaller minimum angle.".into());
    }
    // Boundary edges: element edges used once. Tag each with the outline piece it lies on.
    let mut count: std::collections::HashMap<(u32, u32), (u32, usize)> = std::collections::HashMap::new();
    for (k, t) in tris.iter().enumerate() {
        for j in 0..3 {
            let (a, b) = (t[j], t[(j + 1) % 3]);
            let e = count.entry((a.min(b), a.max(b))).or_insert((0, k));
            e.0 += 1;
        }
    }
    // Bin the outline pieces for a fast nearest search.
    let cell = (h / 2.0).max(diag * 1e-6);
    let key = |p: [f64; 2]| ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64);
    let mut bins: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        let (k0, k1) = (key(s.a), key(s.b));
        for x in k0.0.min(k1.0) - 1..=k0.0.max(k1.0) + 1 {
            for y in k0.1.min(k1.1) - 1..=k0.1.max(k1.1) + 1 {
                bins.entry((x, y)).or_default().push(i);
            }
        }
    }
    let nearest = |p: [f64; 2]| -> Option<usize> {
        let k = key(p);
        bins.get(&k)?.iter().map(|&i| (i, dist_seg(p, &segs[i]))).filter(|x| x.1 <= diag * 1e-6 + cell * 0.05).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0)
    };
    let mut bedges: Vec<(u32, u32, usize)> = vec![];
    for (&(a, b), &(c, k)) in &count {
        if c != 1 {
            continue;
        }
        // Keep the element's direction (counter-clockwise round the material).
        let t = &tris[k];
        let (a, b) = if (0..3).any(|j| t[j] == a && t[(j + 1) % 3] == b) { (a, b) } else { (b, a) };
        let mid = [(nodes[a as usize][0] + nodes[b as usize][0]) / 2.0, (nodes[a as usize][1] + nodes[b as usize][1]) / 2.0];
        let near = nearest(mid).or_else(|| nearest(nodes[a as usize]));
        if let Some(s) = near {
            bedges.push((a, b, s));
        }
    }
    bedges.sort_by_key(|x| (segs[x.2].shape, segs[x.2].edge, x.0, x.1));
    // Steiner points that refinement put on an arc's chord go onto the arc itself.
    for &(a, b, s) in &bedges {
        if let Some((c, r)) = segs[s].arc {
            for n in [a, b] {
                let p = nodes[n as usize];
                let d = (p[0] - c[0]).hypot(p[1] - c[1]);
                if d > 0.0 && (d - r).abs() > 1e-12 * r {
                    nodes[n as usize] = [c[0] + (p[0] - c[0]) * r / d, c[1] + (p[1] - c[1]) * r / d];
                }
            }
        }
    }
    let order = m.mesh.order;
    let mut edges: Vec<BEdge> = bedges.iter().map(|&(a, b, s)| BEdge { shape: segs[s].shape, edge: segs[s].edge, n: [a, u32::MAX, b] }).collect();
    if order == 2 {
        let mut mids: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        let arc_of: std::collections::HashMap<(u32, u32), ([f64; 2], f64)> = bedges.iter().filter_map(|&(a, b, s)| segs[s].arc.map(|x| ((a.min(b), a.max(b)), x))).collect();
        for t in tris.iter_mut() {
            for j in 0..3 {
                let (a, b) = (t[j], t[(j + 1) % 3]);
                let id = *mids.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    let (p, q) = (nodes[a as usize], nodes[b as usize]);
                    let mut c = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
                    if let Some((o, r)) = arc_of.get(&(a.min(b), a.max(b))) {
                        let d = (c[0] - o[0]).hypot(c[1] - o[1]);
                        if d > 0.0 {
                            c = [o[0] + (c[0] - o[0]) * r / d, o[1] + (c[1] - o[1]) * r / d];
                        }
                    }
                    nodes.push(c);
                    (nodes.len() - 1) as u32
                });
                t.push(id);
            }
        }
        for e in edges.iter_mut() {
            e.n[1] = mids[&(e.n[0].min(e.n[2]), e.n[0].max(e.n[2]))];
        }
    }
    // Point targets: the nearest mesh node (they were inserted as vertices, so it is exact).
    let points = targets.iter().filter_map(|p| nodes.iter().enumerate().take(if order == 2 { usize::MAX } else { nodes.len() }).min_by(|a, b| (a.1[0] - p[0]).hypot(a.1[1] - p[1]).total_cmp(&(b.1[0] - p[0]).hypot(b.1[1] - p[1]))).map(|(i, _)| (*p, i as u32))).collect();
    let quality = quality(&nodes, &tris);
    Ok(Mesh { nodes, tris, order, mat, edges, points, size: h, quality })
}

fn quality(nodes: &[[f64; 2]], tris: &[Vec<u32>]) -> Quality {
    let (mut min_angle, mut max_aspect, mut good) = (180.0f64, 1.0f64, 0usize);
    for t in tris {
        let p = [nodes[t[0] as usize], nodes[t[1] as usize], nodes[t[2] as usize]];
        let l = [0, 1, 2].map(|i| (p[(i + 1) % 3][0] - p[i][0]).hypot(p[(i + 1) % 3][1] - p[i][1]));
        let mut amin = 180.0f64;
        for i in 0..3 {
            let (a, b, c) = (l[i], l[(i + 1) % 3], l[(i + 2) % 3]);
            // The angle opposite side c.
            let cos = ((a * a + b * b - c * c) / (2.0 * a * b)).clamp(-1.0, 1.0);
            amin = amin.min(cos.acos().to_degrees());
        }
        min_angle = min_angle.min(amin);
        if amin > 30.0 {
            good += 1;
        }
        let area = ((p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1])).abs() / 2.0;
        let lmax = l[0].max(l[1]).max(l[2]);
        // Equilateral: area = √3/4 l², so this is 1 for a perfect triangle.
        let aspect = if area > 0.0 { lmax * lmax * 3f64.sqrt() / 4.0 / area } else { f64::INFINITY };
        max_aspect = max_aspect.max(aspect);
    }
    Quality { min_angle, max_aspect, good: good as f64 / tris.len().max(1) as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, Shape};

    fn part(shapes: Vec<(Geom, bool)>, size: f64, order: u8) -> Model {
        let mut d = Doc::new(Model::new(Analysis::Stress));
        for (g, hole) in shapes {
            d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: g, hole, ..Shape::default() }) }).unwrap();
        }
        d.m.mesh.size = size;
        d.m.mesh.order = order;
        d.m
    }

    fn area(m: &Mesh) -> f64 {
        m.tris.iter().map(|t| {
            let p = [m.nodes[t[0] as usize], m.nodes[t[1] as usize], m.nodes[t[2] as usize]];
            ((p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1])) / 2.0
        }).sum()
    }

    #[test]
    fn a_square_meshes_whole_with_good_triangles() {
        let m = mesh(&part(vec![(Geom::rect(0.0, 0.0, 1.0, 1.0), false)], 0.1, 1)).unwrap();
        assert!((area(&m) - 1.0).abs() < 1e-12, "{}", area(&m));
        assert!(m.quality.min_angle >= 24.0, "{:?}", m.quality);
        assert!(m.tris.len() > 150 && m.tris.len() < 600, "{}", m.tris.len());
        // Every boundary edge is tagged; each side of the square gets its share.
        let len = |e: u32| m.edges.iter().filter(|x| x.edge == e).map(|x| { let (a, b) = (m.nodes[x.n[0] as usize], m.nodes[x.n[2] as usize]); (b[0] - a[0]).hypot(b[1] - a[1]) }).sum::<f64>();
        for e in 0..4 {
            assert!((len(e) - 1.0).abs() < 1e-12, "edge {e}: {}", len(e));
        }
        // Counter-clockwise triangles.
        assert!(m.tris.iter().all(|t| { let p = [m.nodes[t[0] as usize], m.nodes[t[1] as usize], m.nodes[t[2] as usize]]; (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]) > 0.0 }));
    }

    #[test]
    fn holes_are_cut_and_arcs_stay_round() {
        // A 2 × 2 plate with a hole of radius 0.5: area 4 − π/4 (the hole's chords cut a little
        // less than the circle).
        let md = part(vec![(Geom::rect(-1.0, -1.0, 1.0, 1.0), false), (Geom::Circle { c: [0.0, 0.0], r: 0.5 }, true)], 0.1, 2);
        let hole_id = md.shapes.iter().find(|s| s.hole).unwrap().id;
        let m = mesh(&md).unwrap();
        let a = area(&m);
        let exact = 4.0 - std::f64::consts::PI * 0.25;
        assert!(a > exact && (a - exact).abs() / exact < 2e-3, "{a}");
        // Every node on the hole's edge lies on the circle, midside nodes included.
        let hole = m.edges.iter().filter(|e| e.shape == hole_id).collect::<Vec<_>>();
        assert!(hole.len() >= 24);
        for e in &hole {
            for n in e.n {
                let p = m.nodes[n as usize];
                assert!((p[0].hypot(p[1]) - 0.5).abs() < 1e-12);
            }
        }
        assert!(m.tris.iter().all(|t| t.len() == 6));
        assert!(m.quality.min_angle > 20.0);
    }

    #[test]
    fn point_targets_become_nodes_and_sizes_are_respected() {
        let mut md = part(vec![(Geom::rect(0.0, 0.0, 1.0, 0.5), false)], 0.05, 1);
        md.loads.push(crate::model::Load { id: 90, at: Target::Point { x: 0.3141, y: 0.2718 }, kind: crate::model::LoadKind::Force { fx: 1.0, fy: 0.0 } });
        md.next = 91;
        let m = mesh(&md).unwrap();
        let (p, n) = m.points[0];
        assert_eq!(m.nodes[n as usize], p);
        let coarse = mesh(&part(vec![(Geom::rect(0.0, 0.0, 1.0, 0.5), false)], 0.1, 1)).unwrap();
        assert!(m.tris.len() > 3 * coarse.tris.len());
        // Absurd sizes are refused before any work.
        assert!(mesh(&part(vec![(Geom::rect(0.0, 0.0, 1.0, 1.0), false)], 1e-5, 1)).unwrap_err().contains("too small"));
    }

    #[test]
    fn every_example_part_meshes() {
        for e in crate::examples::list().into_iter().filter(|e| e.analysis != Analysis::Frame && !e.id.starts_with("blank")) {
            let md = crate::examples::get(e.id).unwrap();
            let m = mesh(&md).unwrap_or_else(|x| panic!("{}: {x}", e.id));
            assert!(m.quality.min_angle > 15.0, "{}: {:?}", e.id, m.quality);
            // Every edge with a support or load has mesh edges on it.
            for t in md.supports.iter().map(|s| &s.at).chain(md.loads.iter().map(|l| &l.at)) {
                if let Target::Edge { shape, edge } = t {
                    assert!(m.edges.iter().any(|x| x.shape == *shape && x.edge == *edge), "{}: edge {edge} of shape {shape} has no mesh edges", e.id);
                }
            }
        }
    }
}
