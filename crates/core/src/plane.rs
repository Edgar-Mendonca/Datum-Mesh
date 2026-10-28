//! Plane stress and plane strain on the triangle mesh: isoparametric 3- and 6-node triangles,
//! edge pressure and forces, gravity and point loads as consistent nodal loads, supports on edges
//! and points (rollers on sloping or curved edges by a stiff spring along the normal), the solve,
//! stresses recovered to the nodes, and the reactions with the equilibrium check.

use crate::mesh::{self, Mesh};
use crate::model::{Analysis, LoadKind, Model, SupportKind, Target};
use crate::sparse::{Stats, System};
use serde::Serialize;

/// Forces a support pushes with, summed over its nodes.
#[derive(Clone, Debug, Serialize)]
pub struct Reaction {
    pub support: u32,
    pub fx: f64,
    pub fy: f64,
}

/// One result field over the nodes, with where it is largest and smallest.
#[derive(Clone, Debug, Serialize)]
pub struct Field {
    pub key: &'static str,
    pub v: Vec<f64>,
    pub min: f64,
    pub max: f64,
    pub at_min: [f64; 2],
    pub at_max: [f64; 2],
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaneResults {
    pub mesh: Mesh,
    /// Triangles for drawing: 6-node triangles split into four, so contours follow the curved field.
    pub draw: Vec<[u32; 3]>,
    /// Nodal displacements (m).
    pub disp: Vec<[f64; 2]>,
    /// ux, uy, umag, sx, sy, txy, sz, vm, s1, s2, sf (safety factor against yield; 0 where unknown).
    pub fields: Vec<Field>,
    /// Von Mises stress averaged over each element (unsmoothed contours).
    pub elem_vm: Vec<f64>,
    pub reactions: Vec<Reaction>,
    /// Every applied load and every reaction summed (Fx, Fy, and M about the origin).
    pub applied: [f64; 3],
    pub reacted: [f64; 3],
    /// The size of the loads: nodal forces summed without signs, and their moments likewise.
    pub scale: [f64; 2],
    /// The equilibrium check, as for frames: the largest imbalance over the load scale.
    pub imbalance: f64,
    pub stats: Stats,
    pub warnings: Vec<String>,
}

impl PlaneResults {
    pub fn field(&self, k: &str) -> &Field {
        self.fields.iter().find(|f| f.key == k).expect("field")
    }
    pub fn imbalance(&self) -> f64 {
        self.imbalance
    }
}

/// Sums of nodal forces: the net force and moment about the origin, and the forces' unsigned size.
#[derive(Default)]
struct Tally {
    sum: [f64; 3],
    scale: [f64; 2],
}
impl Tally {
    fn add(&mut self, p: [f64; 2], f: [f64; 2]) {
        let m = p[0] * f[1] - p[1] * f[0];
        self.sum[0] += f[0];
        self.sum[1] += f[1];
        self.sum[2] += m;
        self.scale[0] += f[0].abs() + f[1].abs();
    }
}

/// The equilibrium check: |ΣF| over the force scale and |ΣM| over the moment scale.
pub fn imbalance(applied: [f64; 3], reacted: [f64; 3], scale: [f64; 2]) -> f64 {
    let f = if scale[0] > 0.0 { (applied[0] + reacted[0]).abs().max((applied[1] + reacted[1]).abs()) / scale[0] } else { 0.0 };
    let m = if scale[1] > 0.0 { (applied[2] + reacted[2]).abs() / scale[1] } else { 0.0 };
    f.max(m)
}

/// Shape functions and their (ξ, η) derivatives at a point of the reference triangle.
pub(crate) fn shape(order: u8, xi: f64, eta: f64) -> (Vec<f64>, Vec<[f64; 2]>) {
    let l1 = 1.0 - xi - eta;
    if order == 1 {
        return (vec![l1, xi, eta], vec![[-1.0, -1.0], [1.0, 0.0], [0.0, 1.0]]);
    }
    let (l2, l3) = (xi, eta);
    let n = vec![l1 * (2.0 * l1 - 1.0), l2 * (2.0 * l2 - 1.0), l3 * (2.0 * l3 - 1.0), 4.0 * l1 * l2, 4.0 * l2 * l3, 4.0 * l3 * l1];
    // dL1 = (−1, −1), dL2 = (1, 0), dL3 = (0, 1).
    let d = vec![
        [-(4.0 * l1 - 1.0), -(4.0 * l1 - 1.0)],
        [4.0 * l2 - 1.0, 0.0],
        [0.0, 4.0 * l3 - 1.0],
        [4.0 * (l1 - l2), -4.0 * l2],
        [4.0 * l3, 4.0 * l2],
        [-4.0 * l3, 4.0 * (l1 - l3)],
    ];
    (n, d)
}

/// Gauss points on the reference triangle (ξ, η, weight; weights sum to ½).
pub(crate) fn gauss(order: u8) -> &'static [(f64, f64, f64)] {
    const G1: [(f64, f64, f64); 1] = [(1.0 / 3.0, 1.0 / 3.0, 0.5)];
    // Six points, exact for degree 4 (curved 6-node triangles).
    const A: f64 = 0.445_948_490_915_965;
    const B: f64 = 0.091_576_213_509_771;
    const WA: f64 = 0.111_690_794_839_005;
    const WB: f64 = 0.054_975_871_827_661;
    const G6: [(f64, f64, f64); 6] = [(A, A, WA), (1.0 - 2.0 * A, A, WA), (A, 1.0 - 2.0 * A, WA), (B, B, WB), (1.0 - 2.0 * B, B, WB), (B, 1.0 - 2.0 * B, WB)];
    if order == 1 { &G1 } else { &G6 }
}

/// Reference coordinates of an element's nodes (for stresses at the nodes).
pub(crate) const NODES_REF: [(f64, f64); 6] = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.5, 0.0), (0.5, 0.5), (0.0, 0.5)];

/// The strain–displacement matrix at a point: B (3 × 2n), and det J.
fn bmat(xy: &[[f64; 2]], order: u8, xi: f64, eta: f64) -> (Vec<[f64; 3]>, Vec<f64>, f64) {
    let (n, d) = shape(order, xi, eta);
    let (mut j11, mut j12, mut j21, mut j22) = (0.0, 0.0, 0.0, 0.0);
    for (k, p) in xy.iter().enumerate() {
        j11 += d[k][0] * p[0];
        j12 += d[k][0] * p[1];
        j21 += d[k][1] * p[0];
        j22 += d[k][1] * p[1];
    }
    let det = j11 * j22 - j12 * j21;
    // dN/dx, dN/dy from the inverse Jacobian; columns of B per node: (εx, εy, γxy) for ux and uy.
    let mut b = vec![[0.0; 3]; 2 * xy.len()];
    for k in 0..xy.len() {
        let dx = (j22 * d[k][0] - j12 * d[k][1]) / det;
        let dy = (-j21 * d[k][0] + j11 * d[k][1]) / det;
        b[2 * k] = [dx, 0.0, dy];
        b[2 * k + 1] = [0.0, dy, dx];
    }
    (b, n, det)
}

fn dmat(a: Analysis, e: f64, nu: f64) -> [[f64; 3]; 3] {
    if a == Analysis::Strain {
        let c = e / ((1.0 + nu) * (1.0 - 2.0 * nu));
        [[c * (1.0 - nu), c * nu, 0.0], [c * nu, c * (1.0 - nu), 0.0], [0.0, 0.0, c * (1.0 - 2.0 * nu) / 2.0]]
    } else {
        let c = e / (1.0 - nu * nu);
        [[c, c * nu, 0.0], [c * nu, c, 0.0], [0.0, 0.0, c * (1.0 - nu) / 2.0]]
    }
}

/// Points on a mesh edge (quadratic or straight) and its tangent, for line integrals.
pub(crate) fn edge_geo(nodes: &[[f64; 2]], e: &mesh::BEdge, s: f64) -> ([f64; 2], [f64; 2], [f64; 3]) {
    let (a, b) = (nodes[e.n[0] as usize], nodes[e.n[2] as usize]);
    if e.n[1] == u32::MAX {
        let n = [1.0 - s, 0.0, s];
        return ([a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s], [b[0] - a[0], b[1] - a[1]], n);
    }
    let m = nodes[e.n[1] as usize];
    let n = [(1.0 - s) * (1.0 - 2.0 * s), 4.0 * s * (1.0 - s), s * (2.0 * s - 1.0)];
    let dn = [4.0 * s - 3.0, 4.0 - 8.0 * s, 4.0 * s - 1.0];
    let p = [n[0] * a[0] + n[1] * m[0] + n[2] * b[0], n[0] * a[1] + n[1] * m[1] + n[2] * b[1]];
    let t = [dn[0] * a[0] + dn[1] * m[0] + dn[2] * b[0], dn[0] * a[1] + dn[1] * m[1] + dn[2] * b[1]];
    (p, t, n)
}
pub(crate) const G3: [(f64, f64); 3] = [(0.112_701_665_379_258_3, 5.0 / 18.0), (0.5, 8.0 / 18.0), (0.887_298_334_620_741_7, 5.0 / 18.0)];

/// Mesh and solve a plane stress or plane strain model.
pub fn solve(m: &Model) -> Result<PlaneResults, String> {
    if !matches!(m.analysis, Analysis::Stress | Analysis::Strain) {
        return Err("This solves plane stress and plane strain models.".into());
    }
    let ms = mesh::mesh(m)?;
    solve_mesh(m, ms)
}

pub fn solve_mesh(m: &Model, ms: Mesh) -> Result<PlaneResults, String> {
    let t = if m.analysis == Analysis::Strain { 1.0 } else { m.thickness };
    let nn = ms.nodes.len();
    let mut sys = System::new(2 * nn);
    let mut warnings = vec![];
    let g = m.loads.iter().filter_map(|l| if let (LoadKind::Gravity { gx, gy }, Target::Body) = (&l.kind, &l.at) { Some([*gx, *gy]) } else { None }).fold([0.0, 0.0], |a, b| [a[0] + b[0], a[1] + b[1]]);
    let mats: std::collections::HashMap<u32, &crate::model::Material> = m.materials.iter().map(|x| (x.id, x)).collect();
    let ord = ms.order;
    for (k, tri) in ms.tris.iter().enumerate() {
        let mt = mats.get(&ms.mat[k]).ok_or("A shape's material is missing.")?;
        let d = dmat(m.analysis, mt.e, mt.nu);
        let xy: Vec<[f64; 2]> = tri.iter().map(|&i| ms.nodes[i as usize]).collect();
        let nd = 2 * xy.len();
        let mut ke = vec![0.0; nd * nd];
        let mut fe = vec![0.0; nd];
        for &(xi, eta, w) in gauss(ord) {
            let (b, n, det) = bmat(&xy, ord, xi, eta);
            if det <= 0.0 {
                return Err("The mesh has a folded element; try a different element size.".into());
            }
            let f = w * det * t;
            // DB columns.
            let db: Vec<[f64; 3]> = b.iter().map(|c| [d[0][0] * c[0] + d[0][1] * c[1] + d[0][2] * c[2], d[1][0] * c[0] + d[1][1] * c[1] + d[1][2] * c[2], d[2][0] * c[0] + d[2][1] * c[1] + d[2][2] * c[2]]).collect();
            for i in 0..nd {
                for j in i..nd {
                    let v = (b[i][0] * db[j][0] + b[i][1] * db[j][1] + b[i][2] * db[j][2]) * f;
                    ke[i * nd + j] += v;
                }
            }
            if g != [0.0, 0.0] && mt.rho > 0.0 {
                for (q, ni) in n.iter().enumerate() {
                    fe[2 * q] += ni * mt.rho * g[0] * f;
                    fe[2 * q + 1] += ni * mt.rho * g[1] * f;
                }
            }
        }
        let dof = |i: usize| 2 * tri[i / 2] as usize + i % 2;
        for i in 0..nd {
            for j in i..nd {
                let v = ke[i * nd + j];
                sys.add(dof(i), dof(j), v);
                if i != j {
                    sys.add(dof(j), dof(i), v);
                }
            }
            sys.f[dof(i)] += fe[i];
        }
    }
    // Edge loads.
    let edge_len = |shape: u32, edge: u32| -> f64 {
        ms.edges.iter().filter(|e| e.shape == shape && e.edge == edge).map(|e| G3.iter().map(|&(s, w)| { let (_, tg, _) = edge_geo(&ms.nodes, e, s); w * tg[0].hypot(tg[1]) }).sum::<f64>()).sum()
    };
    let point_node = |x: f64, y: f64| ms.points.iter().find(|p| p.0 == [x, y]).map(|p| p.1 as usize);
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Pressure { .. } | LoadKind::Traction { .. }, Target::Edge { shape, edge }) => {
                let tr_total = if let LoadKind::Traction { fx, fy, total: true } = l.kind { let len = edge_len(*shape, *edge); if len > 0.0 { Some([fx / len, fy / len]) } else { None } } else { None };
                for e in ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge) {
                    for &(s, w) in &G3 {
                        let (_, tg, n) = edge_geo(&ms.nodes, e, s);
                        // Mesh edges run with the material on their left: the outward normal is (ty, −tx).
                        let jl = tg[0].hypot(tg[1]);
                        let out = [tg[1] / jl, -tg[0] / jl];
                        let tr = match l.kind {
                            LoadKind::Pressure { p } => [-p * out[0], -p * out[1]],
                            LoadKind::Traction { fx, fy, total } => if total { tr_total.unwrap_or([0.0, 0.0]) } else { [fx, fy] },
                            _ => [0.0, 0.0],
                        };
                        for (q, ni) in n.iter().enumerate() {
                            let node = e.n[q];
                            if node == u32::MAX {
                                continue;
                            }
                            for c in 0..2 {
                                sys.f[2 * node as usize + c] += ni * tr[c] * w * jl * t;
                            }
                        }
                    }
                }
            }
            (LoadKind::Force { fx, fy }, Target::Point { x, y }) => {
                if let Some(n) = point_node(*x, *y) {
                    sys.f[2 * n] += fx;
                    sys.f[2 * n + 1] += fy;
                } else {
                    warnings.push(format!("A point load at ({x}, {y}) is outside the part; it was left out."));
                }
            }
            (LoadKind::Gravity { .. }, _) => {}
            (LoadKind::Flux { .. } | LoadKind::Convection { .. } | LoadKind::Heat { .. }, _) => warnings.push("Heat loads act only in heat conduction models; they were left out.".into()),
            _ => warnings.push("A load that does not act on parts (it belongs to frames) was left out.".into()),
        }
    }
    let mut applied = Tally::default();
    for n in 0..nn {
        applied.add(ms.nodes[n], [sys.f[2 * n], sys.f[2 * n + 1]]);
    }
    if applied.scale[0] == 0.0 {
        warnings.push("The model has no loads, so nothing moves. Add a pressure, an edge force, a point force or gravity (Loads tab).".into());
    }
    // Supports.
    let mut held = vec![false; 2 * nn];
    let diag_mean = {
        let mut dg = vec![0.0; 2 * nn];
        for &(i, j, v) in &sys.k {
            if i == j {
                dg[i] += v;
            }
        }
        dg.iter().sum::<f64>() / dg.len().max(1) as f64
    };
    let kpen = 1e7 * diag_mean;
    let mut penalties: Vec<(u32, usize, [f64; 2])> = vec![]; // (support, node, normal)
    let mut springs: Vec<(u32, usize, f64, f64)> = vec![];
    let mut sup_nodes: Vec<(u32, Vec<(usize, [bool; 2])>)> = vec![];
    for s in &m.supports {
        let mut nodes: Vec<(usize, [bool; 2])> = vec![];
        match (&s.kind, &s.at) {
            (k, Target::Edge { shape, edge }) => {
                let es: Vec<&mesh::BEdge> = ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge).collect();
                match k {
                    SupportKind::Fixed | SupportKind::Pinned => {
                        for e in &es {
                            for &n in e.n.iter().filter(|n| **n != u32::MAX) {
                                nodes.push((n as usize, [true, true]));
                            }
                        }
                    }
                    SupportKind::Roller | SupportKind::Symmetry => {
                        // The normal at each node, averaged over the mesh edges that meet there.
                        let mut nrm: std::collections::BTreeMap<usize, [f64; 2]> = std::collections::BTreeMap::new();
                        for e in &es {
                            for (q, &n) in e.n.iter().enumerate() {
                                if n == u32::MAX {
                                    continue;
                                }
                                let (_, tg, _) = edge_geo(&ms.nodes, e, q as f64 / 2.0);
                                let jl = tg[0].hypot(tg[1]);
                                let v = nrm.entry(n as usize).or_insert([0.0, 0.0]);
                                v[0] += tg[1] / jl;
                                v[1] -= tg[0] / jl;
                            }
                        }
                        for (n, v) in nrm {
                            let l = v[0].hypot(v[1]);
                            let nv = [v[0] / l, v[1] / l];
                            if nv[1].abs() < 1e-9 {
                                nodes.push((n, [true, false]));
                            } else if nv[0].abs() < 1e-9 {
                                nodes.push((n, [false, true]));
                            } else {
                                penalties.push((s.id, n, nv));
                            }
                        }
                    }
                    _ => warnings.push("A support that does not act on parts was left out.".into()),
                }
            }
            (k, Target::Point { x, y }) => {
                let Some(n) = point_node(*x, *y) else { continue };
                match k {
                    SupportKind::Fixed | SupportKind::Pinned => nodes.push((n, [true, true])),
                    SupportKind::RollerX => nodes.push((n, [false, true])),
                    SupportKind::RollerY => nodes.push((n, [true, false])),
                    SupportKind::Spring { kx, ky, .. } => springs.push((s.id, n, *kx, *ky)),
                    _ => warnings.push("A support that does not act on a point was left out.".into()),
                }
            }
            _ => warnings.push("A support on a node of a frame does not act on parts; it was left out.".into()),
        }
        for (n, h) in &nodes {
            held[2 * n] |= h[0];
            held[2 * n + 1] |= h[1];
        }
        sup_nodes.push((s.id, nodes));
    }
    for &(_, n, nv) in &penalties {
        for a in 0..2 {
            for b in 0..2 {
                sys.add(2 * n + a, 2 * n + b, kpen * nv[a] * nv[b]);
            }
        }
    }
    for &(_, n, kx, ky) in &springs {
        sys.add(2 * n, 2 * n, kx);
        sys.add(2 * n + 1, 2 * n + 1, ky);
    }
    let (u, stats) = sys.solve(&held, &vec![0.0; 2 * nn])?;
    // Reactions: K u − f at held unknowns (each node counted once, for the support that holds it
    // first), plus the stiff springs of rollers on sloping edges and the springs.
    let ku = sys.mul(&u);
    let mut taken = vec![false; 2 * nn];
    let mut reactions = vec![];
    let mut reacted = Tally::default();
    for (sid, nodes) in &sup_nodes {
        let mut r = [0.0; 2];
        let mut add = |n: usize, f: [f64; 2]| {
            r[0] += f[0];
            r[1] += f[1];
            reacted.add(ms.nodes[n], f);
        };
        for (n, h) in nodes {
            let mut f = [0.0; 2];
            for c in 0..2 {
                if h[c] && !taken[2 * n + c] {
                    taken[2 * n + c] = true;
                    f[c] = ku[2 * n + c] - sys.f[2 * n + c];
                }
            }
            add(*n, f);
        }
        for &(s2, n, nv) in &penalties {
            if s2 == *sid {
                let un = u[2 * n] * nv[0] + u[2 * n + 1] * nv[1];
                add(n, [-kpen * un * nv[0], -kpen * un * nv[1]]);
            }
        }
        for &(s2, n, kx, ky) in &springs {
            if s2 == *sid {
                add(n, [-kx * u[2 * n], -ky * u[2 * n + 1]]);
            }
        }
        reactions.push(Reaction { support: *sid, fx: r[0], fy: r[1] });
    }
    // Stresses at each element's nodes, averaged over the elements that share a node (weighted by area).
    let mut acc = vec![[0.0f64; 4]; nn];
    let mut wsum = vec![0.0f64; nn];
    let mut elem_vm = Vec::with_capacity(ms.tris.len());
    let nu_of = |k: usize| mats.get(&ms.mat[k]).map(|x| x.nu).unwrap_or(0.3);
    for (k, tri) in ms.tris.iter().enumerate() {
        let mt = mats[&ms.mat[k]];
        let d = dmat(m.analysis, mt.e, mt.nu);
        let xy: Vec<[f64; 2]> = tri.iter().map(|&i| ms.nodes[i as usize]).collect();
        let (_, _, det0) = bmat(&xy, ord, 1.0 / 3.0, 1.0 / 3.0);
        let area = det0.abs() / 2.0;
        let mut vm_sum = 0.0;
        for (q, &(xi, eta)) in NODES_REF.iter().take(tri.len()).enumerate() {
            let (b, _, _) = bmat(&xy, ord, xi, eta);
            let mut eps = [0.0; 3];
            for (i, c) in b.iter().enumerate() {
                let ui = u[2 * tri[i / 2] as usize + i % 2];
                for r in 0..3 {
                    eps[r] += c[r] * ui;
                }
            }
            let s = [d[0][0] * eps[0] + d[0][1] * eps[1], d[1][0] * eps[0] + d[1][1] * eps[1], d[2][2] * eps[2]];
            let sz = if m.analysis == Analysis::Strain { mt.nu * (s[0] + s[1]) } else { 0.0 };
            let n = tri[q] as usize;
            for r in 0..3 {
                acc[n][r] += s[r] * area;
            }
            acc[n][3] += sz * area;
            wsum[n] += area;
            vm_sum += von_mises(s[0], s[1], s[2], sz);
        }
        elem_vm.push(vm_sum / tri.len() as f64);
    }
    let _ = nu_of;
    let fy_node = {
        // The weakest yield strength of the elements at each node.
        let mut v = vec![f64::INFINITY; nn];
        for (k, tri) in ms.tris.iter().enumerate() {
            let fy = mats[&ms.mat[k]].fy;
            for &n in tri {
                v[n as usize] = v[n as usize].min(fy);
            }
        }
        v
    };
    let mut cols: Vec<(&'static str, Vec<f64>)> = ["ux", "uy", "umag", "sx", "sy", "txy", "sz", "vm", "s1", "s2", "sf"].iter().map(|k| (*k, Vec::with_capacity(nn))).collect();
    for n in 0..nn {
        let w = wsum[n].max(1e-300);
        let (sx, sy, txy, sz) = (acc[n][0] / w, acc[n][1] / w, acc[n][2] / w, acc[n][3] / w);
        let (c, r) = ((sx + sy) / 2.0, (((sx - sy) / 2.0).powi(2) + txy * txy).sqrt());
        let vm = von_mises(sx, sy, txy, sz);
        let fy = fy_node[n];
        let vals = [u[2 * n], u[2 * n + 1], u[2 * n].hypot(u[2 * n + 1]), sx, sy, txy, sz, vm, c + r, c - r, if fy.is_finite() && fy > 0.0 && vm > 0.0 { (fy / vm).min(1e6) } else { 0.0 }];
        for (k, v) in vals.iter().enumerate() {
            cols[k].1.push(*v);
        }
    }
    let fields = cols.into_iter().map(|(key, v)| field(key, v, &ms.nodes)).collect();
    let draw = draw_tris(&ms);
    let disp = (0..nn).map(|n| [u[2 * n], u[2 * n + 1]]).collect();
    let size = m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(1.0);
    let umax = (0..nn).map(|n| u[2 * n].hypot(u[2 * n + 1])).fold(0.0, f64::max);
    if umax > 0.1 * size {
        warnings.push(format!("The largest movement is {:.0} % of the part's size: far beyond small deflections, so this linear answer is not reliable. Check the supports, loads and units.", umax / size * 100.0));
    }
    // As for frames: moments are judged against the forces times the model's reach from the
    // origin (a pressure on a bore centred there has no moment of its own to compare with).
    let reach = ms.nodes.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max) + m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(0.0);
    applied.scale[1] = applied.scale[0] * reach;
    let imbalance = imbalance(applied.sum, reacted.sum, applied.scale);
    Ok(PlaneResults { mesh: ms, draw, disp, fields, elem_vm, reactions, applied: applied.sum, reacted: reacted.sum, scale: applied.scale, imbalance, stats, warnings })
}

/// A nodal field with where it is smallest and largest.
pub(crate) fn field(key: &'static str, v: Vec<f64>, nodes: &[[f64; 2]]) -> Field {
    let (mut imin, mut imax) = (0, 0);
    for i in 0..v.len() {
        if v[i] < v[imin] {
            imin = i;
        }
        if v[i] > v[imax] {
            imax = i;
        }
    }
    Field { key, min: v[imin], max: v[imax], at_min: nodes[imin], at_max: nodes[imax], v }
}

/// Triangles for drawing: 6-node triangles split into four, so contours follow the curved field.
pub(crate) fn draw_tris(ms: &Mesh) -> Vec<[u32; 3]> {
    ms.tris.iter().flat_map(|t| if t.len() == 6 { vec![[t[0], t[3], t[5]], [t[3], t[1], t[4]], [t[5], t[4], t[2]], [t[3], t[4], t[5]]] } else { vec![[t[0], t[1], t[2]]] }).collect()
}

pub fn von_mises(sx: f64, sy: f64, txy: f64, sz: f64) -> f64 {
    (0.5 * ((sx - sy).powi(2) + (sy - sz).powi(2) + (sz - sx).powi(2)) + 3.0 * txy * txy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Geom, Load, Shape, Support};

    fn patch(analysis: Analysis, order: u8, size: f64) -> Doc {
        let mut d = Doc::new(Model::new(analysis));
        d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.2, 0.1), ..Shape::default() }) }).unwrap();
        let s = d.m.shapes[0].id;
        // Left edge (3) and bottom edge (0) on rollers; 50 MPa pulling the right edge (1).
        for (e, k) in [(3, SupportKind::Roller), (0, SupportKind::Roller)] {
            d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: e }, kind: k }) }).unwrap();
        }
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: -50e6 } }) }).unwrap();
        d.m.mesh.order = order;
        d.m.mesh.size = size;
        d
    }

    #[test]
    fn patch_tests_reproduce_constant_stress_exactly() {
        // Benchmark 8 (also in bench.rs): σx = 50 MPa everywhere, σy = τ = 0, for any mesh.
        for (a, order) in [(Analysis::Stress, 1), (Analysis::Stress, 2), (Analysis::Strain, 1), (Analysis::Strain, 2)] {
            let d = patch(a, order, 0.023);
            let r = solve(&d.m).unwrap();
            let sx = r.field("sx");
            assert!((sx.max - 50e6).abs() < 1e-6 * 50e6 && (sx.min - 50e6).abs() < 1e-6 * 50e6, "{a:?} T{}: {} .. {}", if order == 1 { 3 } else { 6 }, sx.min, sx.max);
            assert!(r.field("sy").max.abs() < 1e-6 * 50e6 && r.field("txy").max.abs() < 1e-6 * 50e6);
            // ux at the right end: σ L / E' (plane strain: E' = E/(1−ν²)).
            let e = if a == Analysis::Strain { 210e9 / (1.0 - 0.09) } else { 210e9 };
            assert!((r.field("ux").max - 50e6 * 0.2 / e).abs() < 1e-6 * 50e6 * 0.2 / e);
            assert!(r.imbalance() < 1e-9, "{}", r.imbalance());
        }
    }

    #[test]
    fn a_roller_on_a_sloping_edge_holds_only_across_it() {
        // A square turned 30°: rollers on two edges, the opposite edge pulled. Constant stress
        // along the pull, none across: the penalty spring must not take load along the edge.
        let th = 30f64.to_radians();
        let rot = |x: f64, y: f64| [x * th.cos() - y * th.sin(), x * th.sin() + y * th.cos()];
        let mut d = Doc::new(Model::new(Analysis::Stress));
        d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::Poly { pts: vec![rot(0.0, 0.0), rot(0.1, 0.0), rot(0.1, 0.1), rot(0.0, 0.1)], bulge: vec![0.0; 4] }, ..Shape::default() }) }).unwrap();
        let s = d.m.shapes[0].id;
        for e in [3, 0] {
            d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: e }, kind: SupportKind::Roller }) }).unwrap();
        }
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: -10e6 } }) }).unwrap();
        d.m.mesh.size = 0.02;
        let r = solve(&d.m).unwrap();
        let s1 = r.field("s1");
        assert!((s1.max - 10e6).abs() < 1e-4 * 10e6 && (s1.min - 10e6).abs() < 1e-4 * 10e6, "{} {}", s1.min, s1.max);
        assert!(r.imbalance() < 1e-6, "{}", r.imbalance());
    }
}
