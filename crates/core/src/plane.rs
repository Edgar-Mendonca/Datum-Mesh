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
    #[serde(skip_serializing)]
    pub draw: Vec<[u32; 3]>,
    /// Nodal displacements (m).
    pub disp: Vec<[f64; 2]>,
    /// ux, uy, umag, sx, sy, txy, sz, vm, s1, s2, sf (safety factor against yield; 0 where unknown).
    pub fields: Vec<Field>,
    /// Stresses per element, from the element's own values (not averaged with its neighbours):
    /// sx, sy, txy, sz, vm, s1, s2, sf. Where they differ much from the smoothed fields, the mesh
    /// is too coarse there.
    pub elem: Vec<Field>,
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

/// The strain–displacement matrix at a point: B (4 × 2n: εx, εy, γxy and, for axisymmetric
/// solids, the hoop strain u/r), the shape functions and det J.
fn bmat(xy: &[[f64; 2]], order: u8, xi: f64, eta: f64, axi: bool) -> (Vec<[f64; 4]>, Vec<f64>, f64) {
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
    let r: f64 = n.iter().zip(xy).map(|(ni, p)| ni * p[0]).sum();
    let mut b = vec![[0.0; 4]; 2 * xy.len()];
    for k in 0..xy.len() {
        let dx = (j22 * d[k][0] - j12 * d[k][1]) / det;
        let dy = (-j21 * d[k][0] + j11 * d[k][1]) / det;
        // On the axis u = 0, and u/r tends to ∂u/∂r.
        let hoop = if !axi { 0.0 } else if r > 1e-12 * (j11.abs() + j22.abs()) { n[k] / r } else { dx };
        b[2 * k] = [dx, 0.0, dy, hoop];
        b[2 * k + 1] = [0.0, dy, dx, 0.0];
    }
    (b, n, det)
}

/// The elasticity matrix for (εx, εy, γxy, εθ); the hoop row and column are used by
/// axisymmetric solids only.
fn dmat(a: Analysis, e: f64, nu: f64) -> [[f64; 4]; 4] {
    match a {
        Analysis::Strain | Analysis::Axi => {
            let c = e / ((1.0 + nu) * (1.0 - 2.0 * nu));
            let h = if a == Analysis::Axi { c * nu } else { 0.0 };
            let hh = if a == Analysis::Axi { c * (1.0 - nu) } else { 0.0 };
            [[c * (1.0 - nu), c * nu, 0.0, h], [c * nu, c * (1.0 - nu), 0.0, h], [0.0, 0.0, c * (1.0 - 2.0 * nu) / 2.0, 0.0], [h, h, 0.0, hh]]
        }
        _ => {
            let c = e / (1.0 - nu * nu);
            [[c, c * nu, 0.0, 0.0], [c * nu, c, 0.0, 0.0], [0.0, 0.0, c * (1.0 - nu) / 2.0, 0.0], [0.0; 4]]
        }
    }
}
fn dot4(d: &[[f64; 4]; 4], v: [f64; 4]) -> [f64; 4] {
    [0, 1, 2, 3].map(|i| d[i][0] * v[0] + d[i][1] * v[1] + d[i][2] * v[2] + d[i][3] * v[3])
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
    if !matches!(m.analysis, Analysis::Stress | Analysis::Strain | Analysis::Axi) {
        return Err("This solves plane stress, plane strain and axisymmetric models.".into());
    }
    if m.analysis == Analysis::Axi {
        if let Some(b) = m.bounds() {
            if b[0] < -1e-9 * (b[2] - b[0]).max(b[3] - b[1]) {
                return Err("An axisymmetric part is drawn as its half-section to the right of the axis (x is the radius, x ≥ 0): move it so no part of it is left of x = 0.".into());
            }
        }
    }
    let ms = mesh::mesh(m)?;
    solve_mesh(m, ms)
}

pub fn solve_mesh(m: &Model, ms: Mesh) -> Result<PlaneResults, String> {
    let axi = m.analysis == Analysis::Axi;
    let t = if m.analysis == Analysis::Strain { 1.0 } else { m.thickness };
    // What an area or a length of the drawing stands for: the thickness in plane stress, a metre
    // in plane strain, and the full circle 2πr in an axisymmetric solid.
    let area_w = |p: [f64; 2]| if axi { std::f64::consts::TAU * p[0] } else { t };
    // Edge forces are per length for the whole thickness in a plate; round the circle when turned.
    let len_w = |p: [f64; 2]| if axi { std::f64::consts::TAU * p[0] } else { 1.0 };
    let nn = ms.nodes.len();
    let mut sys = System::new(2 * nn);
    let mut warnings = vec![];
    let g = m.loads.iter().filter_map(|l| if let (LoadKind::Gravity { gx, gy }, Target::Body) = (&l.kind, &l.at) { Some([*gx, *gy]) } else { None }).fold([0.0, 0.0], |a, b| [a[0] + b[0], a[1] + b[1]]);
    let mats: std::collections::HashMap<u32, &crate::model::Material> = m.materials.iter().map(|x| (x.id, x)).collect();
    let ord = ms.order;
    // Thermal stress (0.2.0): the change of temperature at every node from the stress-free one,
    // from fixed temperatures, fluxes and convection solved on this mesh, plus any uniform change.
    let (temps, dt) = temperatures(m, &ms)?;
    if temps.is_none() && m.loads.iter().any(|l| matches!(l.kind, LoadKind::Flux { .. } | LoadKind::Heat { .. })) {
        warnings.push("Heat fluxes and generation need a fixed temperature or convection somewhere to set the temperatures for thermal stress; they were left out.".into());
    }
    // The free thermal strain (x, y, xy, hoop); in plane strain the held length along the body
    // adds (1 + ν); a turned solid grows round its circumference too.
    let eps0 = |mt: &crate::model::Material, dt: f64| { let e = mt.alpha * dt * if m.analysis == Analysis::Strain { 1.0 + mt.nu } else { 1.0 }; [e, e, 0.0, if axi { e } else { 0.0 }] };
    for (k, tri) in ms.tris.iter().enumerate() {
        let mt = mats.get(&ms.mat[k]).ok_or("A shape's material is missing.")?;
        let d = dmat(m.analysis, mt.e, mt.nu);
        let xy: Vec<[f64; 2]> = tri.iter().map(|&i| ms.nodes[i as usize]).collect();
        let nd = 2 * xy.len();
        let mut ke = vec![0.0; nd * nd];
        let mut fe = vec![0.0; nd];
        for &(xi, eta, w) in gauss(ord) {
            let (b, n, det) = bmat(&xy, ord, xi, eta, axi);
            if det <= 0.0 {
                return Err("The mesh has a folded element; try a different element size.".into());
            }
            let pt = [n.iter().zip(&xy).map(|(ni, p)| ni * p[0]).sum::<f64>(), 0.0];
            let f = w * det * area_w(pt);
            // DB columns.
            let db: Vec<[f64; 4]> = b.iter().map(|c| dot4(&d, *c)).collect();
            for i in 0..nd {
                for j in i..nd {
                    let v = (b[i][0] * db[j][0] + b[i][1] * db[j][1] + b[i][2] * db[j][2] + b[i][3] * db[j][3]) * f;
                    ke[i * nd + j] += v;
                }
            }
            if g != [0.0, 0.0] && mt.rho > 0.0 {
                for (q, ni) in n.iter().enumerate() {
                    fe[2 * q] += ni * mt.rho * g[0] * f;
                    fe[2 * q + 1] += ni * mt.rho * g[1] * f;
                }
            }
            if let Some(dt) = &dt {
                // f = ∫ Bᵀ D ε₀.
                let s0 = dot4(&d, eps0(mt, n.iter().zip(tri.iter()).map(|(ni, &i)| ni * dt[i as usize]).sum()));
                for i in 0..nd {
                    fe[i] += (b[i][0] * s0[0] + b[i][1] * s0[1] + b[i][2] * s0[2] + b[i][3] * s0[3]) * f;
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
    // The length of an edge (in a turned solid, the area of the surface it sweeps).
    let edge_len = |shape: u32, edge: u32| -> f64 {
        ms.edges.iter().filter(|e| e.shape == shape && e.edge == edge).map(|e| G3.iter().map(|&(s, w)| { let (p, tg, _) = edge_geo(&ms.nodes, e, s); w * tg[0].hypot(tg[1]) * len_w(p) }).sum::<f64>()).sum()
    };
    let point_node = |x: f64, y: f64| ms.points.iter().find(|p| p.0 == [x, y]).map(|p| p.1 as usize);
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Pressure { .. } | LoadKind::Traction { .. }, Target::Edge { shape, edge }) => {
                let tr_total = if let LoadKind::Traction { fx, fy, total: true } = l.kind { let len = edge_len(*shape, *edge); if len > 0.0 { Some([fx / len, fy / len]) } else { None } } else { None };
                for e in ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge) {
                    for &(s, w) in &G3 {
                        let (pt, tg, n) = edge_geo(&ms.nodes, e, s);
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
                                // Pressure acts per area (times the thickness); an edge force is per
                                // length of edge for the whole thickness, or the whole force.
                                let tt = if matches!(l.kind, LoadKind::Pressure { .. }) { area_w(pt) } else { len_w(pt) };
                                sys.f[2 * node as usize + c] += ni * tr[c] * w * jl * tt;
                            }
                        }
                    }
                }
            }
            (LoadKind::Bearing { .. }, Target::Edge { .. }) if axi => warnings.push("A pin load does not act on an axisymmetric solid (a pin is not round the axis); it was left out.".into()),
            (LoadKind::Bearing { fx, fy }, Target::Edge { shape, edge }) => {
                // The pin presses where the edge's outward normal points against the force:
                // p = p₀ max(0, −n·F̂), pushing into the part; p₀ makes the push along F̂ equal |F|.
                let f = fx.hypot(*fy);
                if f == 0.0 {
                    continue;
                }
                let dir = [fx / f, fy / f];
                let es: Vec<&mesh::BEdge> = ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge).collect();
                let cos = |tg: [f64; 2]| { let jl = tg[0].hypot(tg[1]); (-(tg[1] / jl * dir[0] - tg[0] / jl * dir[1])).max(0.0) };
                let along: f64 = es.iter().map(|e| G3.iter().map(|&(s, w)| { let (_, tg, _) = edge_geo(&ms.nodes, e, s); let c = cos(tg); c * c * w * tg[0].hypot(tg[1]) }).sum::<f64>()).sum();
                if along <= 0.0 {
                    warnings.push("A pin load pushes away from its edge everywhere (the edge faces the other way): it was left out. Put it on the far side of the hole, or reverse the force.".into());
                    continue;
                }
                let p0 = f / along;
                for e in &es {
                    for &(s, w) in &G3 {
                        let (_, tg, n) = edge_geo(&ms.nodes, e, s);
                        let jl = tg[0].hypot(tg[1]);
                        let out = [tg[1] / jl, -tg[0] / jl];
                        let p = p0 * cos(tg);
                        for (q, ni) in n.iter().enumerate() {
                            let node = e.n[q];
                            if node != u32::MAX {
                                for c in 0..2 {
                                    sys.f[2 * node as usize + c] -= ni * p * out[c] * w * jl;
                                }
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
            (LoadKind::Flux { .. } | LoadKind::Convection { .. } | LoadKind::Heat { .. } | LoadKind::Warming { .. }, _) => {}
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
        if matches!(s.kind, SupportKind::Temperature { .. }) {
            // Fixed temperatures set the temperatures for thermal stress; they hold nothing.
            continue;
        }
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
    if axi {
        // Points on the axis cannot move away from it.
        let size = m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(1.0);
        for (n, p) in ms.nodes.iter().enumerate() {
            if p[0].abs() <= 1e-9 * size {
                held[2 * n] = true;
            }
        }
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
    if stats.residual > 1e-8 {
        // Rollers on curved or sloping edges are stiff springs, so a part they hold only nearly
        // (a roller on one arc lets it turn about the arc's centre) is not caught as a mechanism.
        warnings.push(format!("The supports barely hold the part (the equations were solved only to {:.0e}): it can probably turn or slide. Add a fixed or pinned support, or rollers on edges at an angle to each other.", stats.residual));
    }
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
    let ne = ms.tris.len();
    let mut ecols: Vec<(&'static str, Vec<f64>)> = ["sx", "sy", "txy", "sz", "vm", "s1", "s2", "sf"].iter().map(|k| (*k, Vec::with_capacity(ne))).collect();
    let mut centres = Vec::with_capacity(ne);
    for (k, tri) in ms.tris.iter().enumerate() {
        let mt = mats[&ms.mat[k]];
        let d = dmat(m.analysis, mt.e, mt.nu);
        let xy: Vec<[f64; 2]> = tri.iter().map(|&i| ms.nodes[i as usize]).collect();
        let (_, _, det0) = bmat(&xy, ord, 1.0 / 3.0, 1.0 / 3.0, axi);
        let area = det0.abs() / 2.0;
        let mut es = [0.0f64; 4];
        for (q, &(xi, eta)) in NODES_REF.iter().take(tri.len()).enumerate() {
            let (b, _, _) = bmat(&xy, ord, xi, eta, axi);
            let mut eps = [0.0; 4];
            for (i, c) in b.iter().enumerate() {
                let ui = u[2 * tri[i / 2] as usize + i % 2];
                for r in 0..4 {
                    eps[r] += c[r] * ui;
                }
            }
            // Stress comes from the strain beyond the free thermal strain.
            let dtn = dt.as_ref().map(|v| v[tri[q] as usize]).unwrap_or(0.0);
            let e0 = eps0(mt, dtn);
            for r in 0..4 {
                eps[r] -= e0[r];
            }
            let s4 = dot4(&d, eps);
            let s = [s4[0], s4[1], s4[2]];
            // The stress across the plane: plane strain holds the length along the body
            // (σz = ν(σx + σy) − EαΔT); a turned solid has its hoop stress σθ.
            let sz = match m.analysis {
                Analysis::Strain => mt.nu * (s[0] + s[1]) - mt.e * mt.alpha * dtn,
                Analysis::Axi => s4[3],
                _ => 0.0,
            };
            let n = tri[q] as usize;
            for r in 0..3 {
                acc[n][r] += s[r] * area;
            }
            acc[n][3] += sz * area;
            wsum[n] += area;
            for (r, v) in [s[0], s[1], s[2], sz].iter().enumerate() {
                es[r] += v / tri.len() as f64;
            }
        }
        for (c, v) in ecols.iter_mut().zip(stress_set(es, mt.fy)) {
            c.1.push(v);
        }
        centres.push([(xy[0][0] + xy[1][0] + xy[2][0]) / 3.0, (xy[0][1] + xy[1][1] + xy[2][1]) / 3.0]);
    }
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
        let st = stress_set([acc[n][0] / w, acc[n][1] / w, acc[n][2] / w, acc[n][3] / w], fy_node[n]);
        let vals = [u[2 * n], u[2 * n + 1], u[2 * n].hypot(u[2 * n + 1]), st[0], st[1], st[2], st[3], st[4], st[5], st[6], st[7]];
        for (k, v) in vals.iter().enumerate() {
            cols[k].1.push(*v);
        }
    }
    let mut fields: Vec<Field> = cols.into_iter().map(|(key, v)| field(key, v, &ms.nodes)).collect();
    if let Some(t) = temps {
        fields.push(field("t", t, &ms.nodes));
    }
    let draw = draw_tris(&ms);
    let elem = ecols.into_iter().map(|(key, v)| field(key, v, &centres)).collect();
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
    // In a turned solid, radial forces round the circle cancel by themselves and moments about the
    // origin mean nothing: only the forces along the axis must balance.
    let imbalance = if axi {
        if applied.scale[0] > 0.0 { (applied.sum[1] + reacted.sum[1]).abs() / applied.scale[0] } else { 0.0 }
    } else {
        imbalance(applied.sum, reacted.sum, applied.scale)
    };
    Ok(PlaneResults { mesh: ms, draw, disp, fields, elem, reactions, applied: applied.sum, reacted: reacted.sum, scale: applied.scale, imbalance, stats, warnings })
}

/// Temperatures for thermal stress: the nodal temperatures when the model has fixed temperatures
/// or convection (solved as heat conduction on the same mesh), and the change from the
/// stress-free temperature at every node (with any uniform change added). None when nothing warms.
fn temperatures(m: &Model, ms: &Mesh) -> Result<(Option<Vec<f64>>, Option<Vec<f64>>), String> {
    let uniform: f64 = m.loads.iter().filter_map(|l| if let (LoadKind::Warming { dt }, Target::Body) = (&l.kind, &l.at) { Some(*dt) } else { None }).sum();
    let heat = m.supports.iter().any(|s| matches!(s.kind, SupportKind::Temperature { .. })) || m.loads.iter().any(|l| matches!(l.kind, LoadKind::Convection { h, .. } if h > 0.0));
    let temps = if heat {
        let mut hm = m.clone();
        // Heat conduction on the same mesh (weighted round the axis for a turned solid).
        hm.analysis = if m.analysis == Analysis::Axi { Analysis::Axi } else { Analysis::Heat };
        hm.supports.retain(|s| matches!(s.kind, SupportKind::Temperature { .. }));
        hm.loads.retain(|l| matches!(l.kind, LoadKind::Flux { .. } | LoadKind::Convection { .. } | LoadKind::Heat { .. }));
        let r = crate::heat::solve_mesh(&hm, ms.clone()).map_err(|e| format!("The temperatures for thermal stress could not be found: {e}"))?;
        Some(r.fields.into_iter().find(|f| f.key == "t").map(|f| f.v).unwrap_or_default())
    } else {
        None
    };
    let dt = match (&temps, uniform != 0.0) {
        (Some(t), _) => Some(t.iter().map(|x| x - m.t_ref + uniform).collect()),
        (None, true) => Some(vec![uniform; ms.nodes.len()]),
        (None, false) => None,
    };
    Ok((temps, dt))
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

/// σx, σy, τxy, σz → σx, σy, τxy, σz, von Mises, σ1, σ2 and the safety factor against yield
/// (0 where the yield strength is not known or nothing is stressed; at most 10⁶).
fn stress_set(s: [f64; 4], fy: f64) -> [f64; 8] {
    let (sx, sy, txy, sz) = (s[0], s[1], s[2], s[3]);
    let (c, r) = ((sx + sy) / 2.0, (((sx - sy) / 2.0).powi(2) + txy * txy).sqrt());
    let vm = von_mises(sx, sy, txy, sz);
    [sx, sy, txy, sz, vm, c + r, c - r, if fy.is_finite() && fy > 0.0 && vm > 0.0 { (fy / vm).min(1e6) } else { 0.0 }]
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
    fn an_edge_force_is_the_force_whatever_the_thickness() {
        // 0.1.3 bug found in the app: a total edge force was multiplied by the thickness, so the
        // L-bracket carried 15 N instead of 1.5 kN. Pressure acts per area: times the thickness.
        let mut d = patch(Analysis::Stress, 2, 0.03);
        d.m.thickness = 0.01;
        let s = d.m.shapes[0].id;
        d.m.loads.clear();
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Traction { fx: 1.5e3, fy: 0.0, total: true } }) }).unwrap();
        let r = solve(&d.m).unwrap();
        assert!((r.applied[0] - 1.5e3).abs() < 1e-9 * 1.5e3, "{}", r.applied[0]);
        // Per length: 2 kN/m on a 0.1 m edge is 200 N.
        if let LoadKind::Traction { fx, total, .. } = &mut d.m.loads[0].kind { *fx = 2e3; *total = false; }
        assert!((solve(&d.m).unwrap().applied[0] - 200.0).abs() < 1e-9);
        // Pressure: 1 MPa on 0.1 m × 0.01 m is 1 kN.
        d.m.loads[0].kind = LoadKind::Pressure { p: -1e6 };
        let r = solve(&d.m).unwrap();
        assert!((r.applied[0] - 1e3).abs() < 1e-9 * 1e3, "{}", r.applied[0]);
        // And the stress follows: σx = F / (h t) = 1 MPa.
        assert!((r.field("sx").max - 1e6).abs() < 1e-6 * 1e6);
    }

    #[test]
    fn a_pin_bears_on_the_near_half_of_its_hole_with_the_whole_force() {
        // A plate with a round hole, fixed along its bottom; a pin pulls 10 kN upwards. The pin
        // presses on the upper half of the hole only, and the part carries exactly 10 kN.
        let mut d = Doc::new(Model::new(Analysis::Stress));
        d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.1, 0.1), ..Shape::default() }) }).unwrap();
        let hole = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, hole: true, geom: Geom::Circle { c: [0.05, 0.06], r: 0.015 }, ..Shape::default() }) }).unwrap()[0];
        let s = d.m.shapes[0].id;
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: 0 }, kind: SupportKind::Fixed }) }).unwrap();
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: hole, edge: 0 }, kind: LoadKind::Bearing { fx: 0.0, fy: 1e4 } }) }).unwrap();
        d.m.mesh.size = 0.005;
        let r = solve(&d.m).unwrap();
        assert!((r.applied[1] - 1e4).abs() < 1e-9 * 1e4 && r.applied[0].abs() < 1e-6 * 1e4, "{:?}", r.applied);
        assert!(r.imbalance < 1e-9);
        // Below the hole's centre nothing pushes: the lower half's nodes carry no load. Check
        // through the displacement: the plate above the hole moves up, toward the pull.
        let top = r.mesh.nodes.iter().enumerate().filter(|(_, p)| (p[1] - 0.1).abs() < 1e-12).map(|(i, _)| r.disp[i][1]).fold(f64::MIN, f64::max);
        assert!(top > 0.0);
        // A pin pushing the other way on the same hole loads the lower half instead: same size.
        d.m.loads[0].kind = LoadKind::Bearing { fx: 0.0, fy: -1e4 };
        let r2 = solve(&d.m).unwrap();
        assert!((r2.applied[1] + 1e4).abs() < 1e-9 * 1e4);
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
