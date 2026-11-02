//! Assemblies (0.6.0): a 3D solid model made of parts. Each part is a sketch (its shapes) swept by
//! extruding or revolving, meshed on its own and placed in the assembly. The parts' meshes are
//! joined into one tetrahedral mesh, with each part's material on its elements, and parts that
//! touch are tied together where they touch. The tetrahedral solvers (static, plastic, large
//! deformation, frequencies, heat; `tet.rs`) then solve the assembly as they solve an imported mesh.
//!
//! A part's sketch is triangulated (linear triangles) and swept into layers of prisms; each prism
//! is cut into three tetrahedra by the rule of Dompierre et al. (*How to subdivide pyramids,
//! prisms and hexahedra into tetrahedra*, 1999): every quadrilateral face is cut along the diagonal
//! through its lowest-numbered corner, so neighbouring prisms always agree and the mesh is
//! conforming. Prisms pinched on a revolved part's axis become pyramids (two tetrahedra) or a
//! tetrahedron. Quadratic meshes add mid-edge nodes placed on the swept geometry (on the arc for
//! revolved parts).
//!
//! Faces keep stable names: a side face is the sweep of one sketch edge (`Target::Edge`), the
//! start and end faces are `Target::PartEnd`. Supports and loads put on them are mapped to the
//! joined mesh's faces when solving.

use super::tet::{TetMesh, FACES};
use crate::model::{Analysis, Model, Part, Target};
use std::collections::HashMap;

/// The joined mesh and, for each of its faces, the part face it is.
pub struct Assembled {
    pub mesh: TetMesh,
    pub targets: Vec<Option<Target>>,
    /// The part of each face.
    pub face_part: Vec<u32>,
}

/// One part meshed in place: its nodes (placed), elements, named facets.
struct PartMesh {
    nodes: Vec<[f64; 3]>,
    tets: Vec<Vec<u32>>,
    named: Vec<(String, Target, Vec<[u32; 3]>)>,
}

/// The tetrahedra of a prism with corners `v` (bottom 0, 1, 2; top 3, 4, 5 above them), cut so
/// each quadrilateral face's diagonal passes through its lowest corner. Corners that coincide (a
/// revolved part's axis) give a pyramid or a single tetrahedron.
fn prism(v: [u32; 6]) -> Vec<[usize; 4]> {
    let pinched: Vec<usize> = (0..3).filter(|&i| v[i] == v[i + 3]).collect();
    match pinched.len() {
        0 => {
            // Turn the prism so its lowest corner is corner 0 (Dompierre's table).
            const P: [[usize; 6]; 6] = [[0, 1, 2, 3, 4, 5], [1, 2, 0, 4, 5, 3], [2, 0, 1, 5, 3, 4], [3, 5, 4, 0, 2, 1], [4, 3, 5, 1, 0, 2], [5, 4, 3, 2, 1, 0]];
            let lo = (0..6).min_by_key(|&i| v[i]).unwrap_or(0);
            let p = P[lo];
            if v[p[1]].min(v[p[5]]) < v[p[2]].min(v[p[4]]) {
                vec![[p[0], p[1], p[2], p[5]], [p[0], p[1], p[5], p[4]], [p[0], p[4], p[5], p[3]]]
            } else {
                vec![[p[0], p[1], p[2], p[4]], [p[0], p[4], p[2], p[5]], [p[0], p[4], p[5], p[3]]]
            }
        }
        1 => {
            // A pyramid: the apex on the axis, its base the quadrilateral of the other two edges.
            let a = pinched[0];
            let (j, k) = ((a + 1) % 3, (a + 2) % 3);
            let q = [j, k, k + 3, j + 3];
            let lo = (0..4).min_by_key(|&i| v[q[i]]).unwrap_or(0);
            let (b0, b1, b2, b3) = (q[lo], q[(lo + 1) % 4], q[(lo + 2) % 4], q[(lo + 3) % 4]);
            vec![[a, b0, b1, b2], [a, b0, b2, b3]]
        }
        2 => {
            let k = (0..3).find(|i| !pinched.contains(i)).unwrap_or(0);
            vec![[pinched[0], pinched[1], k, k + 3]]
        }
        _ => vec![],
    }
}

/// The two triangles of a quadrilateral face a-b-c-d, cut through its lowest corner (as `prism`
/// cuts it), leaving out triangles pinched on an axis.
fn quad(a: u32, b: u32, c: u32, d: u32) -> Vec<[u32; 3]> {
    let q = [a, b, c, d];
    let lo = (0..4).min_by_key(|&i| q[i]).unwrap_or(0);
    let t = if lo % 2 == 0 { [[a, b, c], [a, c, d]] } else { [[b, c, d], [b, d, a]] };
    t.into_iter().filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]).collect()
}

/// Mesh one part in place.
fn part_mesh(m: &Model, p: &Part, order: u8) -> Result<PartMesh, String> {
    // The part's sketch as a model of its own, meshed into linear triangles.
    let mut sub = m.clone();
    sub.analysis = Analysis::Solid;
    sub.parts.clear();
    sub.volume = None;
    sub.supports.clear();
    sub.loads.clear();
    sub.steps.clear();
    sub.shapes = m.shapes.iter().filter(|s| s.part == p.id && s.plate.is_none()).cloned().map(|mut s| { s.part = 0; s }).collect();
    let ids: Vec<u32> = sub.shapes.iter().map(|s| s.id).collect();
    sub.mesh.refine.retain(|r| matches!(r.at, Target::Edge { shape, .. } if ids.contains(&shape)));
    sub.mesh.order = 1;
    if p.size > 0.0 {
        sub.mesh.size = p.size;
    } else if m.mesh.size <= 0.0 {
        // Automatic: the sketch's own size, but no finer than about 15 000 tetrahedra over the
        // part's sweep (3 per prism, area / 0.43 h² triangles, length / h layers).
        let area: f64 = sub.shapes.iter().map(|s| if s.hole { -s.geom.area().abs() } else { s.geom.area().abs() }).sum::<f64>().max(0.0);
        let rmax = sub.bounds().map_or(0.0, |b| b[2].abs().max(b[0].abs()));
        let length = if p.angle > 0.0 { p.angle * rmax } else { p.depth };
        let by_volume = (3.0 * area * length / (0.43 * 15_000.0)).cbrt();
        sub.mesh.size = super::mesh::auto_size(&sub).max(by_volume);
    }
    if sub.shapes.iter().all(|s| s.hole) {
        return Err(format!("{} has nothing in its sketch yet: draw its outline (select it, then Edit Sketch).", p.name));
    }
    let ms = super::mesh::mesh(&sub).map_err(|e| format!("{}: {e}", p.name))?;
    let n2 = ms.nodes.len();
    let rev = p.angle > 0.0;
    let rmax = ms.nodes.iter().map(|q| q[0]).fold(0.0, f64::max);
    if rev && ms.nodes.iter().any(|q| q[0] < -1e-9 * rmax.max(1e-12)) {
        return Err(format!("{} turns about its sketch's y axis: draw its sketch right of the axis (x ≥ 0).", p.name));
    }
    let length = if rev { p.angle * rmax } else { p.depth };
    let per = if order == 2 { 1.0 } else { 2.0 };
    let by_angle = if rev { (p.angle / (std::f64::consts::PI / if order == 2 { 8.0 } else { 24.0 })).ceil() } else { 1.0 };
    let layers = if m.mesh.layers > 0 { m.mesh.layers as usize } else { (per * length / ms.size).ceil().max(by_angle).clamp(1.0, 128.0) as usize };
    let full = rev && p.angle >= std::f64::consts::TAU - 1e-9;
    let tol = 1e-9 * rmax.max(1e-12);
    // A point of the sweep: sketch node i (or a point between two) at level lv (a fraction of
    // the layers), in the part's own space, then placed.
    let local = |x: f64, y: f64, f: f64| -> [f64; 3] {
        if rev {
            let th = p.angle * f;
            [x * th.cos(), y, x * th.sin()]
        } else {
            [x, y, p.depth * f]
        }
    };
    let mut nodes: Vec<[f64; 3]> = vec![];
    let mut map = vec![0u32; n2 * (layers + 1)];
    for lv in 0..=layers {
        for (i, q) in ms.nodes.iter().enumerate() {
            let same = if rev && lv > 0 && (q[0].abs() < tol || (full && lv == layers)) { Some(map[i]) } else { None };
            map[lv * n2 + i] = match same {
                Some(n) => n,
                None => {
                    nodes.push(p.place(local(q[0], q[1], lv as f64 / layers as f64)));
                    (nodes.len() - 1) as u32
                }
            };
        }
    }
    let at = |i: usize, lv: usize| map[lv * n2 + i];
    // Elements, with mid-edge nodes for quadratic ones placed on the sweep.
    let mut mids: HashMap<(u32, u32), u32> = HashMap::new();
    let mut tets = vec![];
    for tri in &ms.tris {
        let c = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        for l in 0..layers {
            let par = [(c[0], l), (c[1], l), (c[2], l), (c[0], l + 1), (c[1], l + 1), (c[2], l + 1)];
            let v = par.map(|(i, lv)| at(i, lv));
            for t in prism(v) {
                let mut el: Vec<u32> = t.iter().map(|&k| v[k]).collect();
                if order == 2 {
                    for (a, b) in super::tet::EDGES {
                        let (pa, pb) = (par[t[a]], par[t[b]]);
                        let (ga, gb) = (v[t[a]], v[t[b]]);
                        let key = (ga.min(gb), ga.max(gb));
                        let id = *mids.entry(key).or_insert_with(|| {
                            let (qa, qb) = (ms.nodes[pa.0], ms.nodes[pb.0]);
                            // A node on the axis has no angle of its own: take the other's.
                            let (fa, fb) = (pa.1 as f64, pb.1 as f64);
                            let f = if rev && qa[0].abs() < tol { fb } else if rev && qb[0].abs() < tol { fa } else { (fa + fb) / 2.0 };
                            nodes.push(p.place(local((qa[0] + qb[0]) / 2.0, (qa[1] + qb[1]) / 2.0, f / layers as f64)));
                            (nodes.len() - 1) as u32
                        });
                        el.push(id);
                    }
                }
                tets.push(el);
            }
        }
    }
    // Faces: each sketch edge swept is a side face; the start and end faces (none for a full turn).
    let mut named: Vec<(String, Target, Vec<[u32; 3]>)> = vec![];
    let mut side: Vec<((u32, u32), Vec<[u32; 3]>)> = vec![];
    for e in &ms.edges {
        let (a, b) = (e.n[0] as usize, e.n[2] as usize);
        let k = match side.iter().position(|s| s.0 == (e.shape, e.edge)) {
            Some(k) => k,
            None => {
                side.push(((e.shape, e.edge), vec![]));
                side.len() - 1
            }
        };
        for l in 0..layers {
            side[k].1.extend(quad(at(a, l), at(b, l), at(b, l + 1), at(a, l + 1)));
        }
    }
    for (n, ((shape, edge), tris)) in side.into_iter().enumerate() {
        named.push((format!("{} · side {}", p.name, n + 1), Target::Edge { shape, edge }, tris));
    }
    if !full {
        for (front, lv) in [(false, 0), (true, layers)] {
            let tris: Vec<[u32; 3]> = ms.tris.iter().map(|t| [at(t[0] as usize, lv), at(t[1] as usize, lv), at(t[2] as usize, lv)]).filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]).collect();
            let what = match (rev, front) {
                (false, false) => "back",
                (false, true) => "front",
                (true, false) => "start",
                (true, true) => "end",
            };
            named.push((format!("{} · {what}", p.name), Target::PartEnd { part: p.id, front }, tris));
        }
    }
    Ok(PartMesh { nodes, tets, named })
}

/// Mesh every part, join them, and tie the parts together where they touch.
pub fn mesh(m: &Model) -> Result<Assembled, String> {
    if m.parts.is_empty() {
        return Err("The model has no parts.".into());
    }
    let order = m.mesh.order.clamp(1, 2);
    let mut all = TetMesh { order, material: m.parts[0].material, source: format!("{} part{}", m.parts.len(), if m.parts.len() == 1 { "" } else { "s" }), ..Default::default() };
    let mut named: Vec<(String, Vec<[u32; 3]>)> = vec![];
    let mut target_of: HashMap<String, (Target, u32)> = HashMap::new();
    let mut part_nodes: Vec<std::ops::Range<usize>> = vec![];
    for p in &m.parts {
        let pm = part_mesh(m, p, order)?;
        let off = all.nodes.len() as u32;
        part_nodes.push(all.nodes.len()..all.nodes.len() + pm.nodes.len());
        all.nodes.extend(pm.nodes);
        for el in pm.tets {
            all.tets.push(el.into_iter().map(|q| q + off).collect());
            all.mats.push(p.material);
        }
        for (mut name, t, tris) in pm.named {
            while target_of.contains_key(&name) {
                name.push('′');
            }
            target_of.insert(name.clone(), (t, p.id));
            named.push((name, tris.into_iter().map(|t| t.map(|q| q + off)).collect()));
        }
        if all.nodes.len() > super::tet::MAX_NODES {
            return Err(format!("The parts' meshes have over {} nodes together, more than a desktop solve keeps to. Use a larger element size (Mesh tab).", super::tet::MAX_NODES));
        }
    }
    let mut all = all.finish(named)?;
    let targets: Vec<Option<Target>> = all.faces.iter().map(|f| target_of.get(&f.name).map(|x| x.0.clone())).collect();
    let face_part: Vec<u32> = all.faces.iter().map(|f| target_of.get(&f.name).map_or(0, |x| x.1)).collect();
    all.ties = ties(&all, &part_nodes);
    Ok(Assembled { mesh: all, targets, face_part })
}

/// Where parts touch: each surface node of one part lying on a surface facet of another, with its
/// weights on the facet's corners. Both ways round, so neither part's mesh decides alone.
fn ties(ms: &TetMesh, parts: &[std::ops::Range<usize>]) -> Vec<(u32, [u32; 3], [f64; 3])> {
    if parts.len() < 2 {
        return vec![];
    }
    let part_of = |n: u32| parts.iter().position(|r| r.contains(&(n as usize))).unwrap_or(0);
    // Surface facets (corners) of each part, and the surface nodes.
    let mut facets: Vec<([u32; 3], usize)> = vec![];
    let mut on_surface = vec![false; ms.nodes.len()];
    for f in &ms.faces {
        for &(e, k) in &f.facets {
            let c = FACES[k as usize].map(|i| ms.tets[e as usize][i]);
            facets.push((c, part_of(c[0])));
            for q in ms.facet(e as usize, k as usize) {
                on_surface[q as usize] = true;
            }
        }
    }
    let p = |q: u32| ms.nodes[q as usize];
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    // A grid of cells, a typical facet across, holding the facets that reach into each.
    let h = facets.iter().map(|(c, _)| dot(sub(p(c[1]), p(c[0])), sub(p(c[1]), p(c[0]))).sqrt()).sum::<f64>() / facets.len().max(1) as f64;
    let h = h.max(1e-12);
    let cell = |x: [f64; 3]| [(x[0] / h).floor() as i64, (x[1] / h).floor() as i64, (x[2] / h).floor() as i64];
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (i, (c, _)) in facets.iter().enumerate() {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for &q in c {
            for d in 0..3 {
                lo[d] = lo[d].min(p(q)[d]);
                hi[d] = hi[d].max(p(q)[d]);
            }
        }
        let (a, b) = (cell(lo.map(|v| v - 0.05 * h)), cell(hi.map(|v| v + 0.05 * h)));
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    grid.entry([x, y, z]).or_default().push(i);
                }
            }
        }
    }
    let mut out = vec![];
    for n in 0..ms.nodes.len() {
        if !on_surface[n] {
            continue;
        }
        let me = part_of(n as u32);
        let x = ms.nodes[n];
        let mut best: Option<(f64, [u32; 3], [f64; 3])> = None;
        for &i in grid.get(&cell(x)).map(|v| v.as_slice()).unwrap_or(&[]) {
            let (c, pt) = facets[i];
            if pt == me {
                continue;
            }
            let (a, b, cc) = (p(c[0]), p(c[1]), p(c[2]));
            let (e1, e2) = (sub(b, a), sub(cc, a));
            let nv = cross(e1, e2);
            let area2 = dot(nv, nv).sqrt();
            if area2 <= 0.0 {
                continue;
            }
            let size = dot(e1, e1).sqrt().min(dot(e2, e2).sqrt());
            let dist = dot(sub(x, a), nv).abs() / area2;
            // Touching: within a small gap (a curved face's chords sit a little off the curve).
            if dist > 0.02 * size {
                continue;
            }
            let w1 = dot(cross(sub(x, a), e2), nv) / (area2 * area2);
            let w2 = dot(cross(e1, sub(x, a)), nv) / (area2 * area2);
            let w0 = 1.0 - w1 - w2;
            let slack = -1e-6;
            if w0 < slack || w1 < slack || w2 < slack {
                continue;
            }
            if best.as_ref().is_none_or(|bb| dist < bb.0) {
                best = Some((dist, c, [w0, w1, w2]));
            }
        }
        if let Some((_, c, w)) = best {
            out.push((n as u32, c, w));
        }
    }
    out
}

/// The model as a tetrahedral one: the parts meshed and joined, supports and loads on the parts'
/// faces put on the joined mesh's faces.
pub fn as_tet(m: &Model) -> Result<Model, String> {
    let a = mesh(m)?;
    let mut t = m.clone();
    let face_of = |at: &Target| -> Option<Target> { a.targets.iter().position(|x| x.as_ref() == Some(at)).map(|f| Target::Face { face: f as u32 }) };
    for s in t.supports.iter_mut() {
        if let Some(f) = face_of(&s.at) {
            s.at = f;
        }
    }
    for l in t.loads.iter_mut() {
        if let Some(f) = face_of(&l.at) {
            l.at = f;
        }
    }
    t.volume = Some(Box::new(a.mesh));
    t.parts.clear();
    t.shapes.clear();
    Ok(t)
}

/// What the window shows of an assembly: the joined mesh's preview, each face's part face and part.
pub fn preview(m: &Model) -> Result<serde_json::Value, String> {
    let a = mesh(m)?;
    let mut v = serde_json::to_value(super::tet::preview(&a.mesh)).map_err(|e| e.to_string())?;
    if let Some(o) = v.as_object_mut() {
        o.insert("targets".into(), serde_json::to_value(&a.targets).unwrap_or_default());
        o.insert("face_part".into(), serde_json::to_value(&a.face_part).unwrap_or_default());
        o.insert("ties".into(), a.mesh.ties.len().into());
    }
    Ok(v)
}

/// Solve an assembly: linear (`nl` None), elastic-plastic or large deformation (Some((increments, large))).
pub fn solve_with(m: &Model, nl: Option<(usize, bool)>) -> Result<super::solid::SolidResults, String> {
    let t = as_tet(m)?;
    if matches!(nl, Some((_, true))) && t.volume.as_ref().is_some_and(|v| !v.ties.is_empty()) {
        return Err("Large deformation of parts tied together comes with contact (0.6.2); solve one part, or make the step elastic-plastic or linear.".into());
    }
    super::tet::solve_with(&t, nl)
}

/// The lowest natural frequencies of an assembly.
pub fn modes(m: &Model, count: usize) -> Result<super::solid::SolidModes, String> {
    super::tet::modes(&as_tet(m)?, count)
}

/// Steady heat conduction through an assembly.
pub fn heat(m: &Model) -> Result<super::solid::SolidHeatResults, String> {
    super::tet::heat(&as_tet(m)?)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Geom, Shape};

    /// A model of parts: each (sketch outline, depth or angle, place).
    pub(crate) fn parts(list: &[(Geom, f64, f64, [f64; 3], [f64; 3])]) -> Doc {
        let mut d = Doc::new(Model::new(Analysis::Solid));
        d.m.mesh.size = 0.05;
        for (k, (g, depth, angle, at, turn)) in list.iter().enumerate() {
            let id = d.apply(Op::Add { item: Item::Part(Part { name: format!("P{}", k + 1), depth: *depth, angle: *angle, at: *at, turn: *turn, ..Default::default() }) }).unwrap()[0];
            d.apply(Op::Add { item: Item::Shape(Shape { geom: g.clone(), material: d.m.materials[0].id, part: id, ..Default::default() }) }).unwrap();
        }
        d
    }

    #[test]
    fn every_example_survives_saving_and_opening() {
        // 0.6.0: opening a saved assembly dropped every shape in a part (the repair checked each
        // shape in a model without the parts).
        for e in crate::examples::list() {
            let m = crate::examples::get(e.id).unwrap();
            let (b, warn) = crate::model::from_json(&crate::model::to_json(&m)).unwrap();
            assert!(warn.is_empty(), "{}: {warn:?}", e.id);
            assert_eq!(b, m, "{}", e.id);
        }
    }

    #[test]
    fn an_automatic_part_meshes_to_a_desktop_size() {
        // 0.6.0, found in the app: a new part (30 × 20 mm sketch, 100 mm deep) at the automatic
        // size was meshed by its sketch alone, into over 66 000 nodes.
        let mut d = parts(&[(Geom::rect(0.055, 0.05, 0.085, 0.07), 0.1, 0.0, [0.0; 3], [0.0; 3])]);
        d.m.mesh.size = 0.0;
        let a = mesh(&d.m).unwrap();
        assert!(a.mesh.nodes.len() < 40_000 && a.mesh.tets.len() > 2_000, "{} {}", a.mesh.nodes.len(), a.mesh.tets.len());
    }

    #[test]
    fn prisms_cut_into_three_tetrahedra_that_fill_them() {
        // A unit prism: the three tetrahedra's volumes add to the prism's (1/2), for every corner
        // numbering.
        let pos = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]];
        let perms: [[u32; 6]; 4] = [[0, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 0], [3, 0, 4, 1, 5, 2], [2, 5, 0, 4, 1, 3]];
        for ids in perms {
            let vol: f64 = prism(ids)
                .iter()
                .map(|t| {
                    let q = t.map(|i| pos[i]);
                    let s = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                    let (a, b, c) = (s(q[1], q[0]), s(q[2], q[0]), s(q[3], q[0]));
                    (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])).abs() / 6.0
                })
                .sum();
            assert!((vol - 0.5).abs() < 1e-12, "{ids:?} {vol}");
        }
    }

    #[test]
    fn a_part_meshes_into_a_conforming_mesh_of_the_right_volume_and_faces() {
        // A 0.2 × 0.1 block 0.3 deep, turned and moved: volume 0.006, and 6 faces (4 sides, back,
        // front), every one named for the part.
        let d = parts(&[(Geom::rect(0.0, 0.0, 0.2, 0.1), 0.3, 0.0, [1.0, 2.0, 3.0], [0.3, -0.2, 0.5])]);
        let a = mesh(&d.m).unwrap();
        let vol: f64 = (0..a.mesh.tets.len()).map(|e| {
            let el = &a.mesh.tets[e];
            let p = |i: usize| a.mesh.nodes[el[i] as usize];
            let s = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
            let (u, v, w) = (s(p(1), p(0)), s(p(2), p(0)), s(p(3), p(0)));
            (u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])) / 6.0
        }).sum();
        assert!((vol - 0.006).abs() < 1e-12, "{vol}");
        assert_eq!(a.mesh.faces.len(), 6, "{:?}", a.mesh.faces.iter().map(|f| &f.name).collect::<Vec<_>>());
        assert!(a.targets.iter().all(|t| t.is_some()));
        assert!(a.mesh.ties.is_empty());
    }
}
