//! Plates in a 3D frame (0.4.1): the shapes drawn on each working plane meshed into shell
//! triangles, joined to the frame where nodes meet.
//!
//! Each plane (a working plane at one depth) is meshed on its own by the parts mesher, in the
//! plane's (u, v), with three-node triangles. Every frame node on that plane becomes a mesh
//! node, and so do points along members lying in it, so the beams and the plate share nodes; the
//! members are then cut at every plate node along them. Plates on different planes join where
//! their mesh nodes meet in 3D (along a shared edge meshed the same way from both sides).

use crate::model::{Analysis, Model, Plate, PlaneKind, Support, SupportKind, Target};

/// The plates' mesh in 3D.
#[derive(Clone, Debug, Default)]
pub struct PlateMesh {
    pub nodes: Vec<[f64; 3]>,
    pub tris: Vec<[usize; 3]>,
    /// For each triangle: the plate (shape id), its thickness and material, and its plane's
    /// u and v directions (the axes moments are given in).
    pub shape: Vec<u32>,
    pub t: Vec<f64>,
    pub mat: Vec<u32>,
    pub axes: Vec<[[f64; 3]; 2]>,
    /// Boundary pieces: (shape, edge, node, node).
    pub edges: Vec<(u32, u32, usize, usize)>,
}

const ON: f64 = 1e-6;

fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
            c = !c;
        }
    }
    c
}

/// Mesh every plate of a 3D frame model.
pub fn mesh(m: &Model) -> Result<PlateMesh, String> {
    let mut out = PlateMesh::default();
    let mut groups: Vec<(PlaneKind, f64)> = vec![];
    for s in &m.shapes {
        if let Some(p) = s.plate {
            if !groups.iter().any(|g| g.0 == p.plane && (g.1 - p.depth).abs() < ON) {
                groups.push((p.plane, p.depth));
            }
        }
    }
    let size = if m.mesh.size > 0.0 { m.mesh.size } else { auto(m) };
    for (plane, depth) in groups {
        let pl = Plate { plane, depth, t: 1.0 };
        let shapes: Vec<_> = m.shapes.iter().filter(|s| s.plate.is_some_and(|p| p.plane == plane && (p.depth - depth).abs() < ON)).collect();
        let mut tmp = Model::new(Analysis::Stress);
        tmp.materials = m.materials.clone();
        tmp.shapes = shapes.iter().map(|s| crate::model::Shape { plate: None, ..(*s).clone() }).collect();
        tmp.mesh = m.mesh.clone();
        tmp.mesh.order = 1;
        tmp.mesh.size = size;
        tmp.mesh.refine.retain(|r| match r.at {
            Target::Edge { shape, .. } => shapes.iter().any(|s| s.id == shape),
            _ => false,
        });
        // Frame nodes on this plane, and points along members lying in it, become mesh nodes.
        let mut pts: Vec<[f64; 2]> = vec![];
        for n in &m.nodes {
            let (uv, off) = pl.local(n.at());
            if off.abs() < ON {
                pts.push(uv);
            }
        }
        for mb in &m.members {
            let (Some(a), Some(b)) = (m.node(mb.a), m.node(mb.b)) else { continue };
            let ((ua, oa), (ub, ob)) = (pl.local(a.at()), pl.local(b.at()));
            if oa.abs() < ON && ob.abs() < ON {
                let len = (ub[0] - ua[0]).hypot(ub[1] - ua[1]);
                let k = (len / size).ceil().max(1.0) as usize;
                for i in 1..k {
                    let t = i as f64 / k as f64;
                    pts.push([ua[0] + (ub[0] - ua[0]) * t, ua[1] + (ub[1] - ua[1]) * t]);
                }
            }
        }
        let next = tmp.next.max(tmp.max_id() + 1);
        tmp.supports = pts.iter().enumerate().map(|(i, p)| Support { id: next + i as u32, at: Target::Point { x: p[0], y: p[1] }, kind: SupportKind::Pinned }).collect();
        tmp.next = next + pts.len() as u32 + 1;
        let mesh = crate::mesh::mesh(&tmp).map_err(|e| format!("Plates on the {} plane at {depth} m: {e}", name(plane)))?;
        // Nodes into 3D, shared with any already there.
        let mut map = Vec::with_capacity(mesh.nodes.len());
        for p in &mesh.nodes {
            let x = pl.at(p[0], p[1]);
            let k = out.nodes.iter().position(|q| crate::model::dist3(*q, x) < ON).unwrap_or_else(|| {
                out.nodes.push(x);
                out.nodes.len() - 1
            });
            map.push(k);
        }
        let outlines: Vec<(u32, bool, Vec<[f64; 2]>)> = shapes.iter().map(|s| (s.id, s.hole, s.geom.outline(size / 8.0, 32).0)).collect();
        let axes = pl.axes();
        for t in &mesh.tris {
            let c = [t[0] as usize, t[1] as usize, t[2] as usize];
            let g = [(mesh.nodes[c[0]][0] + mesh.nodes[c[1]][0] + mesh.nodes[c[2]][0]) / 3.0, (mesh.nodes[c[0]][1] + mesh.nodes[c[1]][1] + mesh.nodes[c[2]][1]) / 3.0];
            // The plate it lies in: the last solid shape round its centre.
            let Some(sid) = outlines.iter().rev().find(|o| !o.1 && inside(&o.2, g)).map(|o| o.0) else { continue };
            let sh = shapes.iter().find(|s| s.id == sid).expect("plate");
            out.tris.push([map[c[0]], map[c[1]], map[c[2]]]);
            out.shape.push(sid);
            out.t.push(sh.plate.map_or(0.0, |p| p.t));
            out.mat.push(sh.material);
            out.axes.push(axes);
        }
        for e in &mesh.edges {
            out.edges.push((e.shape, e.edge, map[e.n[0] as usize], map[e.n[2] as usize]));
        }
    }
    Ok(out)
}

fn name(p: PlaneKind) -> &'static str {
    match p {
        PlaneKind::Xy => "Front",
        PlaneKind::Zy => "Side",
        PlaneKind::Xz => "Plan",
    }
}

/// An automatic element size for plates: about a twentieth of the largest plate.
fn auto(m: &Model) -> f64 {
    let big = m
        .shapes
        .iter()
        .filter(|s| s.plate.is_some() && !s.hole)
        .map(|s| {
            let o = s.geom.outline(f64::INFINITY, 16).0;
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for p in o {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            (hi[0] - lo[0]).max(hi[1] - lo[1])
        })
        .fold(0.0, f64::max);
    if big > 0.0 { big / 20.0 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, Geom, Load, LoadKind, MemberKind, Model, Plate, PlaneKind, Shape, Support, SupportKind, Target};

    /// A 4 × 4 m slab on the Plan plane at height 3, 0.2 thick, of the default material.
    pub(crate) fn slab(size: f64) -> (Doc, u32) {
        let mut d = Doc::new(Model::new(Analysis::Space));
        d.m.materials[0].rho = 0.0;
        d.m.mesh.size = size;
        let id = d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::rect(0.0, 0.0, 4.0, 4.0), material: d.m.materials[0].id, plate: Some(Plate { plane: PlaneKind::Xz, depth: 3.0, t: 0.2 }), ..Shape::default() }) }).unwrap()[0];
        (d, id)
    }

    #[test]
    fn a_simply_supported_slab_under_pressure() {
        let (mut d, id) = slab(0.25);
        for e in 0..4 {
            d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: id, edge: e }, kind: SupportKind::Pinned }) }).unwrap();
        }
        let q = 10e3;
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Plate { shape: id }, kind: LoadKind::Pressure { p: q } }) }).unwrap();
        let r = crate::space::solve(&d.m).unwrap();
        let p = r.plates.as_ref().unwrap();
        let mat = &d.m.materials[0];
        let dd = mat.e * 0.2f64.powi(3) / (12.0 * (1.0 - mat.nu * mat.nu));
        // Timoshenko: w = α q a⁴ / D, α = 0.00406 for ν = 0.3 (0.00416 at ν = 0.25 …): the plate
        // material's ν decides; compare with the Navier series instead.
        let a = 4.0;
        // The node nearest the middle, and Navier's series for the deflection there.
        let mid = (0..p.nodes.len()).min_by(|&i, &j| ((p.nodes[i][0] - 2.0).hypot(p.nodes[i][2] + 2.0)).total_cmp(&(p.nodes[j][0] - 2.0).hypot(p.nodes[j][2] + 2.0))).unwrap();
        let (x, y) = (p.nodes[mid][0], -p.nodes[mid][2]);
        let pi = std::f64::consts::PI;
        let mut w = 0.0;
        for m in (1..80).step_by(2) {
            for n in (1..80).step_by(2) {
                let (mf, nf) = (m as f64, n as f64);
                w += 16.0 * q / (pi.powi(6) * dd * mf * nf * ((mf / a).powi(2) + (nf / a).powi(2)).powi(2)) * (mf * pi * x / a).sin() * (nf * pi * y / a).sin();
            }
        }
        let uy = p.disp[mid][1];
        assert!(uy < 0.0, "a positive pressure pushes the slab down: {uy}");
        assert!((-uy - w).abs() < 0.01 * w, "{uy} against {w}");
        // Reactions: four edges carrying the whole load.
        let fy: f64 = r.reactions.iter().map(|x| x.f[1]).sum();
        assert!((fy - q * 16.0).abs() < 1e-6 * q * 16.0, "{fy}");
        assert!(r.imbalance < 1e-9, "{}", r.imbalance);
    }

    #[test]
    fn a_slab_on_four_columns_and_edge_beams_balances() {
        let (mut d, id) = slab(0.5);
        d.m.materials[0].rho = 2500.0;
        for (x, z) in [(0.0, 0.0), (4.0, 0.0), (4.0, -4.0), (0.0, -4.0)] {
            d.apply(Op::AddMember { from: [x, 0.0], to: [x, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [z, z] }).unwrap();
        }
        for w in [[(0.0, 0.0), (4.0, 0.0)], [(4.0, 0.0), (4.0, -4.0)], [(4.0, -4.0), (0.0, -4.0)], [(0.0, -4.0), (0.0, 0.0)]] {
            d.apply(Op::AddMember { from: [w[0].0, 3.0], to: [w[1].0, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [w[0].1, w[1].1] }).unwrap();
        }
        let bases: Vec<u32> = d.m.nodes.iter().filter(|n| n.y == 0.0).map(|n| n.id).collect();
        for b in bases {
            d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: b }, kind: SupportKind::Fixed }) }).unwrap();
        }
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Plate { shape: id }, kind: LoadKind::Pressure { p: 5e3 } }) }).unwrap();
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Gravity { gx: 0.0, gy: -9.81, gz: 0.0 } }) }).unwrap();
        let r = crate::space::solve(&d.m).unwrap();
        assert!(r.imbalance < 1e-9, "{}", r.imbalance);
        // The columns carry the slab's pressure and weight, and the members' weight.
        let fy: f64 = r.reactions.iter().map(|x| x.f[1]).sum();
        let slab = 16.0 * (5e3 + 2500.0 * 9.81 * 0.2);
        assert!(fy > slab && fy < slab * 1.2, "{fy} {slab}");
        // Edge beams were cut where the slab's nodes meet them: their results run end to end.
        let b = &r.members[4];
        assert!(b.x.len() > 3 && (b.x.last().unwrap() - 4.0).abs() < 1e-9, "{:?}", b.x);
        // The slab sags most in its middle.
        let p = r.plates.unwrap();
        let low = p.fields.iter().find(|f| f.key == "uy").unwrap();
        assert!((low.at_min[0] - 2.0).abs() < 0.6 && (low.at_min[2] + 2.0).abs() < 0.6, "{:?}", low.at_min);
    }
}
