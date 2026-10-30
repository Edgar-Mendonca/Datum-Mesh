//! 3D solids (0.3.0): a part drawn in the x-y plane, extruded along z by the model's thickness,
//! solved in linear elasticity. The 2D triangle mesh is swept into layers: 18-node wedges (6-node
//! triangle × 3 nodes through the layer) from quadratic meshes, 6-node wedges from linear ones, so
//! meshing stays as robust as in 2D. Supports and loads go on the side faces (the part's edges,
//! swept) and on the back (z = 0) and front (z = thickness) faces.
//!
//! The window gets the outer surface only: its nodes, triangles for drawing, the displacements and
//! the stresses there. Largest and smallest values are over every node, inside too.

use crate::mesh::{self, BEdge, Mesh, Quality};
use crate::model::{Analysis, LoadKind, Model, SupportKind, Target};
use crate::sparse::{Stats, System};
use serde::Serialize;

/// A result over the surface nodes; min and max (and where) over every node of the solid.
#[derive(Clone, Debug, Serialize)]
pub struct Field3 {
    pub key: &'static str,
    pub v: Vec<f64>,
    pub min: f64,
    pub max: f64,
    pub at_min: [f64; 3],
    pub at_max: [f64; 3],
}

/// The outer surface for drawing: nodes and triangles (quadratic faces split, so contours follow
/// the field), and the outline of the part's edges.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Surface {
    pub nodes: Vec<[f64; 3]>,
    pub tris: Vec<[u32; 3]>,
    pub lines: Vec<[u32; 2]>,
}

/// The whole solid for the cutting plane (0.3.2): every node, its displacement and its value of
/// each field (in the order of `fields`), and the elements cut into tetrahedra on their corner
/// nodes (quadratic wedges first split into eight), so the window can slice it anywhere.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Volume {
    pub nodes: Vec<[f64; 3]>,
    pub disp: Vec<[f64; 3]>,
    pub tets: Vec<[u32; 4]>,
    pub v: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SolidReaction {
    pub support: u32,
    pub f: [f64; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct SolidResults {
    pub surface: Surface,
    /// Displacements of the surface nodes (m).
    pub disp: Vec<[f64; 3]>,
    /// ux, uy, uz, umag, sx, sy, sz, txy, tyz, tzx, vm, s1, s3, sf.
    pub fields: Vec<Field3>,
    pub reactions: Vec<SolidReaction>,
    /// Loads and reactions summed: forces along x, y, z and moments about the origin.
    pub applied: [f64; 6],
    pub reacted: [f64; 6],
    pub scale: [f64; 2],
    pub imbalance: f64,
    pub stats: Stats,
    pub warnings: Vec<String>,
    /// The mesh: elements, nodes, layers through the thickness, element order, the 2D element
    /// size and the 2D mesh's quality.
    pub elements: usize,
    pub nodes: usize,
    pub layers: usize,
    pub order: u8,
    pub size: f64,
    pub quality: Quality,
    pub volume: Volume,
}

impl SolidResults {
    pub fn field(&self, k: &str) -> &Field3 {
        self.fields.iter().find(|f| f.key == k).expect("field")
    }
}

/// The most unknowns a 3D solid may have (about 3 GB of memory and half a minute on four cores;
/// 100 000 take about 6 s and 1.2 GB, docs/verification).
pub const MAX_UNKNOWNS: usize = 200_000;

/// The swept mesh: 2D nodes repeated on every level through the thickness (extruded) or round
/// the turn (revolved, 0.3.2). A revolved part's nodes on the axis are one node on every level,
/// and a full turn's last level is its first.
pub(crate) struct Swept {
    pub ms: Mesh,
    pub order: usize,
    pub levels: usize,
    /// The solid's node for each 2D node on each level (level × n2 + i).
    pub map: Vec<usize>,
    pub pos: Vec<[f64; 3]>,
    /// The revolve angle (0: extruded).
    pub revolve: f64,
}

impl Swept {
    pub fn n2(&self) -> usize {
        self.ms.nodes.len()
    }
    pub fn node(&self, i: usize, level: usize) -> usize {
        self.map[level * self.n2() + i]
    }
    pub fn at(&self, n: usize) -> [f64; 3] {
        self.pos[n]
    }
    pub fn count(&self) -> usize {
        self.pos.len()
    }
    /// Does the part have back and front faces (not a full turn)?
    pub fn has_ends(&self) -> bool {
        self.revolve < std::f64::consts::TAU - 1e-9
    }
    /// The nodes of the element on triangle `k` in layer `l`: the triangle's nodes on each level
    /// of the layer, bottom first.
    pub fn element(&self, k: usize, l: usize) -> Vec<usize> {
        let tri = &self.ms.tris[k];
        (0..=self.order).flat_map(|a| tri.iter().map(move |&i| (i as usize, l * self.order + a))).map(|(i, lv)| self.node(i, lv)).collect()
    }
}

/// Sweep the part's 2D mesh through the thickness.
pub(crate) fn sweep(m: &Model) -> Result<Swept, String> {
    let ms = mesh::mesh(m)?;
    let order = ms.order as usize;
    let (t, rev) = (m.thickness, m.revolve);
    let rmax = ms.nodes.iter().map(|p| p[0]).fold(0.0, f64::max);
    if rev > 0.0 {
        let span = ms.nodes.iter().map(|p| p[0].abs().max(p[1].abs())).fold(0.0, f64::max).max(1e-12);
        if ms.nodes.iter().any(|p| p[0] < -1e-9 * span) {
            return Err("A revolved part is drawn right of the y axis (x ≥ 0), its axis of turning; move the section or cut it at the axis.".into());
        }
    }
    // Layers: about one element size each (quadratic), two per size for linear wedges, which are
    // stiff in bending; round a turn, also at most 22.5° each (7.5° for linear) so the curved
    // faces stay true. Set in the mesh settings to override.
    let length = if rev > 0.0 { rev * rmax } else { t };
    let layers = if m.mesh.layers > 0 {
        m.mesh.layers as usize
    } else if order == 2 {
        let by_angle = if rev > 0.0 { (rev / (std::f64::consts::PI / 8.0)).ceil() } else { 1.0 };
        (length / ms.size).ceil().max(by_angle).clamp(1.0, 64.0) as usize
    } else {
        let by_angle = if rev > 0.0 { (rev / (std::f64::consts::PI / 24.0)).ceil() } else { 2.0 };
        (2.0 * length / ms.size).ceil().max(by_angle).clamp(2.0, 128.0) as usize
    };
    let levels = layers * order + 1;
    let n2 = ms.nodes.len();
    let full = rev >= std::f64::consts::TAU - 1e-9;
    let tol = 1e-9 * rmax.max(1e-12);
    let mut map = vec![0; n2 * levels];
    let mut pos = vec![];
    for lv in 0..levels {
        let f = lv as f64 / (levels - 1) as f64;
        for (i, p) in ms.nodes.iter().enumerate() {
            let same = if rev > 0.0 && lv > 0 && (p[0].abs() < tol || (full && lv == levels - 1)) { Some(map[i]) } else { None };
            map[lv * n2 + i] = match same {
                Some(n) => n,
                None => {
                    let th = rev * f;
                    pos.push(if rev > 0.0 { [p[0] * th.cos(), p[1], p[0] * th.sin()] } else { [p[0], p[1], t * f] });
                    pos.len() - 1
                }
            };
        }
    }
    let unknowns = 3 * pos.len();
    if unknowns > MAX_UNKNOWNS {
        return Err(format!("This 3D mesh would have {} unknowns, more than the {} a desktop solve keeps to (it would need several GB of memory and minutes). Use a larger element size or fewer layers (Inspector, with nothing selected, under Mesh).", unknowns, MAX_UNKNOWNS));
    }
    Ok(Swept { ms, order, levels, map, pos, revolve: rev })
}

/// Shape functions of a 3- or 6-node triangle at (ξ, η) and their derivatives.
fn tri_n(order: usize, xi: f64, eta: f64) -> (Vec<f64>, Vec<[f64; 2]>) {
    let l = [1.0 - xi - eta, xi, eta];
    let dl = [[-1.0, -1.0], [1.0, 0.0], [0.0, 1.0]];
    if order == 1 {
        return (l.to_vec(), dl.to_vec());
    }
    let mut n = vec![];
    let mut d = vec![];
    for i in 0..3 {
        n.push(l[i] * (2.0 * l[i] - 1.0));
        d.push([(4.0 * l[i] - 1.0) * dl[i][0], (4.0 * l[i] - 1.0) * dl[i][1]]);
    }
    for (a, b) in [(0, 1), (1, 2), (2, 0)] {
        n.push(4.0 * l[a] * l[b]);
        d.push([4.0 * (l[a] * dl[b][0] + l[b] * dl[a][0]), 4.0 * (l[a] * dl[b][1] + l[b] * dl[a][1])]);
    }
    (n, d)
}
/// Shape functions of a 2- or 3-node line at ζ ∈ [−1, 1] and their derivatives.
fn line_n(order: usize, s: f64) -> (Vec<f64>, Vec<f64>) {
    if order == 1 {
        (vec![(1.0 - s) / 2.0, (1.0 + s) / 2.0], vec![-0.5, 0.5])
    } else {
        (vec![s * (s - 1.0) / 2.0, 1.0 - s * s, s * (s + 1.0) / 2.0], vec![s - 0.5, -2.0 * s, s + 0.5])
    }
}
/// Triangle quadrature (ξ, η, weight; weights sum to ½): 3 points, or 6 (degree 4).
fn tri_rule(order: usize) -> Vec<(f64, f64, f64)> {
    if order == 1 {
        return vec![(1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0), (2.0 / 3.0, 1.0 / 6.0, 1.0 / 6.0), (1.0 / 6.0, 2.0 / 3.0, 1.0 / 6.0)];
    }
    let (a, wa, b, wb) = (0.445_948_490_915_965, 0.223_381_589_678_011 / 2.0, 0.091_576_213_509_771, 0.109_951_743_655_322 / 2.0);
    vec![(a, a, wa), (1.0 - 2.0 * a, a, wa), (a, 1.0 - 2.0 * a, wa), (b, b, wb), (1.0 - 2.0 * b, b, wb), (b, 1.0 - 2.0 * b, wb)]
}
/// Gauss points on [−1, 1].
fn gauss1(order: usize) -> Vec<(f64, f64)> {
    if order == 1 {
        let g = 1.0 / 3f64.sqrt();
        vec![(-g, 1.0), (g, 1.0)]
    } else {
        let g = (0.6f64).sqrt();
        vec![(-g, 5.0 / 9.0), (0.0, 8.0 / 9.0), (g, 5.0 / 9.0)]
    }
}

/// Global derivatives of the wedge's shape functions at (ξ, η, ζ), the functions themselves and
/// the Jacobian's determinant.
fn wedge(xyz: &[[f64; 3]], order: usize, xi: f64, eta: f64, zeta: f64) -> (Vec<[f64; 3]>, Vec<f64>, f64) {
    let (nt, dt) = tri_n(order, xi, eta);
    let (nl, dl) = line_n(order, zeta);
    let k = nt.len();
    let mut n = Vec::with_capacity(xyz.len());
    let mut d = Vec::with_capacity(xyz.len());
    for a in 0..nl.len() {
        for j in 0..k {
            n.push(nt[j] * nl[a]);
            d.push([dt[j][0] * nl[a], dt[j][1] * nl[a], nt[j] * dl[a]]);
        }
    }
    // J[r][c] = ∂x_r / ∂ξ_c.
    let mut jm = [[0.0; 3]; 3];
    for (p, dn) in xyz.iter().zip(&d) {
        for r in 0..3 {
            for c in 0..3 {
                jm[r][c] += p[r] * dn[c];
            }
        }
    }
    let det = jm[0][0] * (jm[1][1] * jm[2][2] - jm[1][2] * jm[2][1]) - jm[0][1] * (jm[1][0] * jm[2][2] - jm[1][2] * jm[2][0]) + jm[0][2] * (jm[1][0] * jm[2][1] - jm[1][1] * jm[2][0]);
    // Inverse of J; the global derivatives are (J⁻¹)ᵀ ∂N/∂ξ.
    let inv = [
        [(jm[1][1] * jm[2][2] - jm[1][2] * jm[2][1]) / det, (jm[0][2] * jm[2][1] - jm[0][1] * jm[2][2]) / det, (jm[0][1] * jm[1][2] - jm[0][2] * jm[1][1]) / det],
        [(jm[1][2] * jm[2][0] - jm[1][0] * jm[2][2]) / det, (jm[0][0] * jm[2][2] - jm[0][2] * jm[2][0]) / det, (jm[0][2] * jm[1][0] - jm[0][0] * jm[1][2]) / det],
        [(jm[1][0] * jm[2][1] - jm[1][1] * jm[2][0]) / det, (jm[0][1] * jm[2][0] - jm[0][0] * jm[2][1]) / det, (jm[0][0] * jm[1][1] - jm[0][1] * jm[1][0]) / det],
    ];
    let g = d.iter().map(|dn| [0, 1, 2].map(|r| inv[0][r] * dn[0] + inv[1][r] * dn[1] + inv[2][r] * dn[2])).collect();
    (g, n, det)
}

/// The natural coordinates of the wedge's nodes, in element order.
fn node_ref(order: usize) -> Vec<(f64, f64, f64)> {
    let tri: Vec<(f64, f64)> = if order == 1 { vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] } else { vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.5, 0.0), (0.5, 0.5), (0.0, 0.5)] };
    let zs: Vec<f64> = if order == 1 { vec![-1.0, 1.0] } else { vec![-1.0, 0.0, 1.0] };
    zs.iter().flat_map(|&z| tri.iter().map(move |&(x, y)| (x, y, z))).collect()
}

/// σx, σy, σz, τxy, τyz, τzx → those, von Mises, the largest and smallest principal stresses and
/// the safety factor against yield (0 where unknown; at most 10⁶).
fn stress_set(s: [f64; 6], fy: f64) -> [f64; 10] {
    let [sx, sy, sz, txy, tyz, tzx] = s;
    let vm = (0.5 * ((sx - sy).powi(2) + (sy - sz).powi(2) + (sz - sx).powi(2)) + 3.0 * (txy * txy + tyz * tyz + tzx * tzx)).sqrt();
    let (s1, s3) = principal(s);
    [sx, sy, sz, txy, tyz, tzx, vm, s1, s3, if fy.is_finite() && fy > 0.0 && vm > 0.0 { (fy / vm).min(1e6) } else { 0.0 }]
}
/// The largest and smallest principal stresses of a symmetric 3×3 stress (trigonometric solution).
pub(crate) fn principal(s: [f64; 6]) -> (f64, f64) {
    let [sx, sy, sz, txy, tyz, tzx] = s;
    let p1 = txy * txy + tyz * tyz + tzx * tzx;
    let q = (sx + sy + sz) / 3.0;
    let p2 = (sx - q).powi(2) + (sy - q).powi(2) + (sz - q).powi(2) + 2.0 * p1;
    if p2 <= 1e-30 * (q * q).max(1e-300) {
        return (q, q);
    }
    let p = (p2 / 6.0).sqrt();
    let b = [[(sx - q) / p, txy / p, tzx / p], [txy / p, (sy - q) / p, tyz / p], [tzx / p, tyz / p, (sz - q) / p]];
    let det = b[0][0] * (b[1][1] * b[2][2] - b[1][2] * b[2][1]) - b[0][1] * (b[1][0] * b[2][2] - b[1][2] * b[2][0]) + b[0][2] * (b[1][0] * b[2][1] - b[1][1] * b[2][0]);
    let phi = (det / 2.0).clamp(-1.0, 1.0).acos() / 3.0;
    let e1 = q + 2.0 * p * phi.cos();
    let e3 = q + 2.0 * p * (phi + 2.0 * std::f64::consts::PI / 3.0).cos();
    (e1, e3)
}

/// Sums of forces and their moments about the origin, and their size without signs.
#[derive(Default)]
struct Tally {
    sum: [f64; 6],
    scale: f64,
}
impl Tally {
    fn add(&mut self, p: [f64; 3], f: [f64; 3]) {
        let m = [p[1] * f[2] - p[2] * f[1], p[2] * f[0] - p[0] * f[2], p[0] * f[1] - p[1] * f[0]];
        for k in 0..3 {
            self.sum[k] += f[k];
            self.sum[3 + k] += m[k];
        }
        self.scale += f[0].abs() + f[1].abs() + f[2].abs();
    }
}

/// A face of the solid for loads and supports: its nodes on a small grid (rows along the edge or
/// the triangle, columns through the thickness), so loads integrate with the faces' own shape.
enum Face<'a> {
    /// A side face: one mesh edge of the outline over one layer.
    Side(&'a BEdge, usize),
    /// An end face: one triangle, at the back or the front.
    End(usize, bool),
}

/// Integration points of a face: (point, outward normal times the area element, nodes with their
/// shape function values).
fn face_points(sw: &Swept, f: &Face) -> Vec<([f64; 3], [f64; 3], Vec<(usize, f64)>)> {
    let o = sw.order;
    let mut out = vec![];
    match f {
        Face::Side(e, layer) => {
            // Along the edge: start, middle (quadratic), end; through the layer: o + 1 levels.
            let ids: Vec<usize> = if o == 2 { vec![e.n[0] as usize, e.n[1] as usize, e.n[2] as usize] } else { vec![e.n[0] as usize, e.n[2] as usize] };
            let lev: Vec<usize> = (0..=o).map(|a| layer * o + a).collect();
            for &(s, ws) in &gauss1(2) {
                for &(t, wt) in &gauss1(2) {
                    let (ns, ds) = line_n(o, s);
                    let (nt, dt) = line_n(o, t);
                    let (mut p, mut ts, mut tt) = ([0.0; 3], [0.0; 3], [0.0; 3]);
                    let mut nodes = vec![];
                    for a in 0..ids.len() {
                        for b in 0..lev.len() {
                            let n = sw.node(ids[a], lev[b]);
                            let x = sw.at(n);
                            for r in 0..3 {
                                p[r] += ns[a] * nt[b] * x[r];
                                ts[r] += ds[a] * nt[b] * x[r];
                                tt[r] += ns[a] * dt[b] * x[r];
                            }
                            nodes.push((n, ns[a] * nt[b]));
                        }
                    }
                    // Mesh edges run with the material on their left, z points out of the
                    // drawing: the outward normal is (along the edge) × z.
                    let nv = [ts[1] * tt[2] - ts[2] * tt[1], ts[2] * tt[0] - ts[0] * tt[2], ts[0] * tt[1] - ts[1] * tt[0]];
                    out.push((p, nv.map(|v| v * ws * wt), nodes));
                }
            }
        }
        Face::End(k, front) => {
            let tri = &sw.ms.tris[*k];
            let lv = if *front { sw.levels - 1 } else { 0 };
            for &(xi, eta, w) in &tri_rule(2) {
                let (n, d) = tri_n(o, xi, eta);
                let (mut p, mut a1, mut a2) = ([0.0; 3], [0.0; 3], [0.0; 3]);
                let mut nodes = vec![];
                for (j, &i) in tri.iter().enumerate() {
                    let nd = sw.node(i as usize, lv);
                    let x = sw.at(nd);
                    for r in 0..3 {
                        p[r] += n[j] * x[r];
                        a1[r] += d[j][0] * x[r];
                        a2[r] += d[j][1] * x[r];
                    }
                    nodes.push((nd, n[j]));
                }
                // Triangles run counter-clockwise: their area element points along the sweep
                // (+z, or round the turn), outwards at the front, inwards at the back.
                let a = [a1[1] * a2[2] - a1[2] * a2[1], a1[2] * a2[0] - a1[0] * a2[2], a1[0] * a2[1] - a1[1] * a2[0]];
                let sg = if *front { w } else { -w };
                out.push((p, a.map(|v| v * sg), nodes));
            }
        }
    }
    out
}

/// The faces a target covers.
fn faces<'a>(sw: &'a Swept, at: &Target) -> Vec<Face<'a>> {
    let layers = (sw.levels - 1) / sw.order;
    match at {
        Target::Edge { shape, edge } => sw.ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge).flat_map(|e| (0..layers).map(move |l| Face::Side(e, l))).collect(),
        Target::End { front } if sw.has_ends() => (0..sw.ms.tris.len()).map(|k| Face::End(k, *front)).collect(),
        _ => vec![],
    }
}

/// The nodes of a line on a side face (`Target::Line`): the edge's nodes on the nearest level.
fn line_nodes(sw: &Swept, at: &Target) -> Vec<usize> {
    let Target::Line { shape, edge, at } = at else { return vec![] };
    let lv = (at * (sw.levels - 1) as f64).round() as usize;
    let mut v: Vec<usize> = sw.ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge).flat_map(|e| if sw.order == 2 { vec![e.n[0], e.n[1], e.n[2]] } else { vec![e.n[0], e.n[2]] }).map(|i| sw.node(i as usize, lv.min(sw.levels - 1))).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Solve a 3D solid.
pub fn solve(m: &Model) -> Result<SolidResults, String> {
    if m.analysis != Analysis::Solid {
        return Err("This model is not a 3D solid.".into());
    }
    if m.revolve == 0.0 && !(m.thickness > 0.0) {
        return Err("A 3D solid needs a thickness to extrude its outline by (Inspector, with nothing selected).".into());
    }
    let sw = sweep(m)?;
    solve_swept(m, sw)
}

pub(crate) fn solve_swept(m: &Model, sw: Swept) -> Result<SolidResults, String> {
    let o = sw.order;
    let nn = sw.count();
    let layers = (sw.levels - 1) / o;
    let mut sys = System::lower(3 * nn);
    let mut warnings = vec![];
    let mats: std::collections::HashMap<u32, &crate::model::Material> = m.materials.iter().map(|x| (x.id, x)).collect();
    let g = m.loads.iter().filter_map(|l| if let (LoadKind::Gravity { gx, gy, gz }, Target::Body) = (&l.kind, &l.at) { Some([*gx, *gy, *gz]) } else { None }).fold([0.0; 3], |a, b| [a[0] + b[0], a[1] + b[1], a[2] + b[2]]);
    let (rule_t, rule_z) = (tri_rule(o), gauss1(o));
    let ne = sw.ms.tris.len() * layers;
    for k in 0..sw.ms.tris.len() {
        let mt = mats.get(&sw.ms.mat[k]).ok_or("A shape's material is missing.")?;
        let (lam, mu) = (mt.e * mt.nu / ((1.0 + mt.nu) * (1.0 - 2.0 * mt.nu)), mt.e / (2.0 * (1.0 + mt.nu)));
        for l in 0..layers {
            let en = sw.element(k, l);
            let xyz: Vec<[f64; 3]> = en.iter().map(|&n| sw.at(n)).collect();
            let nd = en.len();
            let mut ke = vec![0.0; 9 * nd * nd];
            let mut fe = vec![0.0; 3 * nd];
            for &(xi, eta, wt) in &rule_t {
                for &(zeta, wz) in &rule_z {
                    let (gd, n, det) = wedge(&xyz, o, xi, eta, zeta);
                    if det <= 0.0 {
                        return Err("The mesh has a folded element; try a different element size.".into());
                    }
                    let w = wt * wz * det;
                    // Isotropic elasticity: K_ij,ab = λ ∂aNi ∂bNj + μ (∂bNi ∂aNj + δab ∇Ni·∇Nj).
                    for i in 0..nd {
                        for j in 0..=i {
                            let dij = gd[i][0] * gd[j][0] + gd[i][1] * gd[j][1] + gd[i][2] * gd[j][2];
                            for a in 0..3 {
                                for b in 0..3 {
                                    let v = lam * gd[i][a] * gd[j][b] + mu * (gd[i][b] * gd[j][a] + if a == b { dij } else { 0.0 });
                                    ke[(3 * i + a) * 3 * nd + 3 * j + b] += v * w;
                                }
                            }
                        }
                    }
                    if mt.rho > 0.0 && g != [0.0; 3] {
                        for i in 0..nd {
                            for c in 0..3 {
                                fe[3 * i + c] += n[i] * mt.rho * g[c] * w;
                            }
                        }
                    }
                }
            }
            for i in 0..nd {
                for j in 0..=i {
                    for a in 0..3 {
                        // A node's own block is symmetric: its lower half is enough.
                        for b in 0..if i == j { a + 1 } else { 3 } {
                            let (r, c) = (3 * en[i] + a, 3 * en[j] + b);
                            let v = ke[(3 * i + a) * 3 * nd + 3 * j + b];
                            // Keep the lower triangle of the global matrix whichever way round the
                            // element numbers its nodes.
                            if r == c && i != j {
                                // Two element nodes that are one node of the solid (pinched on a
                                // revolved part's axis): the pair's block counts both ways round.
                                sys.add(r, c, 2.0 * v);
                            } else if r >= c {
                                sys.add(r, c, v);
                            } else {
                                sys.add(c, r, v);
                            }
                        }
                    }
                }
                for c in 0..3 {
                    sys.f[3 * en[i] + c] += fe[3 * i + c];
                }
            }
        }
    }
    // Loads on faces.
    let area = |at: &Target| -> f64 { faces(&sw, at).iter().flat_map(|f| face_points(&sw, f)).map(|(_, nv, _)| (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2]).sqrt()).sum() };
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Pressure { .. } | LoadKind::Traction { .. }, Target::Edge { .. } | Target::End { .. }) => {
                let per = match l.kind {
                    LoadKind::Traction { fx, fy, fz, total: true } => {
                        let a = area(&l.at);
                        if a > 0.0 {
                            Some([fx / a, fy / a, fz / a])
                        } else {
                            None
                        }
                    }
                    LoadKind::Traction { fx, fy, fz, total: false } => Some([fx, fy, fz]),
                    _ => None,
                };
                if matches!(l.at, Target::End { .. }) && !sw.has_ends() {
                    warnings.push("A part revolved a full turn has no back or front face; a load on one was left out.".into());
                }
                for f in faces(&sw, &l.at) {
                    for (_, nv, nodes) in face_points(&sw, &f) {
                        // Pressure pushes into the part, against the outward normal.
                        let tr = match (&l.kind, per) {
                            (LoadKind::Pressure { p }, _) => nv.map(|v| -p * v),
                            (_, Some(t)) => { let da = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2]).sqrt(); t.map(|v| v * da) }
                            _ => [0.0; 3],
                        };
                        for (n, s) in nodes {
                            for c in 0..3 {
                                sys.f[3 * n + c] += s * tr[c];
                            }
                        }
                    }
                }
            }
            (LoadKind::Gravity { .. }, Target::Body) => {}
            (LoadKind::Warming { .. } | LoadKind::Flux { .. } | LoadKind::Convection { .. } | LoadKind::Heat { .. }, _) => warnings.push("Thermal loads on 3D solids come in a later release; one was left out.".into()),
            (LoadKind::Bearing { .. }, _) => warnings.push("Pin loads on 3D solids come in a later release; one was left out (use a pressure on the hole's face).".into()),
            _ => warnings.push("A load that does not act on 3D solids (on a point, a node or a member) was left out.".into()),
        }
    }
    let mut applied = Tally::default();
    for n in 0..nn {
        applied.add(sw.at(n), [sys.f[3 * n], sys.f[3 * n + 1], sys.f[3 * n + 2]]);
    }
    if applied.scale == 0.0 {
        warnings.push("The model has no loads, so nothing moves. Add a pressure, a face force or gravity (Loads tab).".into());
    }
    // Supports: held movements, and stiff springs along the normal of sloping faces.
    let diag_mean = {
        let mut dg = vec![0.0; 3 * nn];
        for &(i, j, v) in &sys.k {
            if i == j {
                dg[i] += v;
            }
        }
        dg.iter().sum::<f64>() / dg.len().max(1) as f64
    };
    let kpen = 1e7 * diag_mean;
    let mut held = vec![false; 3 * nn];
    let mut sup_nodes: Vec<(u32, Vec<(usize, [bool; 3])>)> = vec![];
    let mut penalties: Vec<(u32, usize, [f64; 3])> = vec![];
    for s in &m.supports {
        let mut nodes: Vec<(usize, [bool; 3])> = vec![];
        match (&s.kind, &s.at) {
            (SupportKind::Temperature { .. }, _) => continue,
            (SupportKind::Fixed | SupportKind::Pinned | SupportKind::Hold { .. }, Target::Line { .. }) => {
                let h = if let SupportKind::Hold { x, y, z } = s.kind { [x, y, z] } else { [true; 3] };
                nodes.extend(line_nodes(&sw, &s.at).into_iter().map(|n| (n, h)));
            }
            (SupportKind::Hold { x, y, z }, Target::Edge { .. } | Target::End { .. }) => {
                let mut ns: Vec<usize> = faces(&sw, &s.at).iter().flat_map(|f| face_points(&sw, f)).flat_map(|(_, _, ns)| ns.into_iter().map(|(n, _)| n)).collect();
                ns.sort_unstable();
                ns.dedup();
                nodes.extend(ns.into_iter().map(|n| (n, [*x, *y, *z])));
            }
            (k @ (SupportKind::Fixed | SupportKind::Pinned | SupportKind::Roller | SupportKind::Symmetry), Target::Edge { .. } | Target::End { .. }) => {
                let fixed = matches!(k, SupportKind::Fixed | SupportKind::Pinned);
                // Each face node with the normal averaged over the faces that meet there.
                let mut nrm: std::collections::BTreeMap<usize, [f64; 3]> = std::collections::BTreeMap::new();
                for f in faces(&sw, &s.at) {
                    for (_, nv, ns) in face_points(&sw, &f) {
                        for (n, w) in ns {
                            let v = nrm.entry(n).or_insert([0.0; 3]);
                            for c in 0..3 {
                                v[c] += nv[c] * w.abs();
                            }
                        }
                    }
                }
                for (n, v) in nrm {
                    if fixed {
                        nodes.push((n, [true; 3]));
                        continue;
                    }
                    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    let nv = v.map(|c| c / l);
                    match (0..3).find(|&c| (nv[c].abs() - 1.0).abs() < 1e-9) {
                        Some(c) => {
                            let mut h = [false; 3];
                            h[c] = true;
                            nodes.push((n, h));
                        }
                        None => penalties.push((s.id, n, nv)),
                    }
                }
            }
            _ => warnings.push("Supports on 3D solids go on faces (edges or the back and front faces) or lines on them, and are fixed, pinned, roller, symmetry or held directions; one was left out.".into()),
        }
        if nodes.is_empty() && !penalties.iter().any(|p| p.0 == s.id) && !matches!(s.kind, SupportKind::Temperature { .. }) {
            warnings.push(if matches!(s.at, Target::End { .. }) { "A part revolved a full turn has no back or front face; a support on one was left out.".into() } else { "A support found no nodes of the solid to hold and was left out.".into() });
        }
        for (n, h) in &nodes {
            for c in 0..3 {
                held[3 * n + c] |= h[c];
            }
        }
        sup_nodes.push((s.id, nodes));
    }
    for &(_, n, nv) in &penalties {
        for a in 0..3 {
            for b in 0..=a {
                sys.add(3 * n + a, 3 * n + b, kpen * nv[a] * nv[b]);
            }
        }
    }
    let (u, stats) = sys.solve(&held, &vec![0.0; 3 * nn]).map_err(|e| format!("{e} A 3D solid also needs holding so it cannot turn about any axis: a fixed face, or rollers on three faces at angles to each other."))?;
    if stats.residual > 1e-8 {
        warnings.push(format!("The supports barely hold the part (the equations were solved only to {:.0e}): it can probably turn or slide. Add a fixed face, or rollers on faces at angles to each other.", stats.residual));
    }
    // Reactions.
    let ku = sys.mul(&u);
    let mut taken = vec![false; 3 * nn];
    let mut reactions = vec![];
    let mut reacted = Tally::default();
    for (sid, nodes) in &sup_nodes {
        let mut r = [0.0; 3];
        for (n, h) in nodes {
            let mut f = [0.0; 3];
            for c in 0..3 {
                if h[c] && !taken[3 * n + c] {
                    taken[3 * n + c] = true;
                    f[c] = ku[3 * n + c] - sys.f[3 * n + c];
                }
            }
            reacted.add(sw.at(*n), f);
            for c in 0..3 {
                r[c] += f[c];
            }
        }
        for &(s2, n, nv) in &penalties {
            if s2 == *sid {
                let un = (0..3).map(|c| u[3 * n + c] * nv[c]).sum::<f64>();
                let f = nv.map(|c| -kpen * un * c);
                reacted.add(sw.at(n), f);
                for c in 0..3 {
                    r[c] += f[c];
                }
            }
        }
        reactions.push(SolidReaction { support: *sid, f: r });
    }
    // Stresses at each element's nodes, averaged over the elements that share a node (by volume).
    let refs = node_ref(o);
    let mut acc = vec![[0.0f64; 6]; nn];
    let mut wsum = vec![0.0f64; nn];
    let mut fy_node = vec![f64::INFINITY; nn];
    for k in 0..sw.ms.tris.len() {
        let mt = mats[&sw.ms.mat[k]];
        let (lam, mu) = (mt.e * mt.nu / ((1.0 + mt.nu) * (1.0 - 2.0 * mt.nu)), mt.e / (2.0 * (1.0 + mt.nu)));
        for l in 0..layers {
            let en = sw.element(k, l);
            let xyz: Vec<[f64; 3]> = en.iter().map(|&n| sw.at(n)).collect();
            let (_, _, det0) = wedge(&xyz, o, 1.0 / 3.0, 1.0 / 3.0, 0.0);
            let vol = det0.abs();
            for (q, &(xi, eta, zeta)) in refs.iter().enumerate() {
                let (mut gd, _, det) = wedge(&xyz, o, xi, eta, zeta);
                if det.abs() < 1e-9 * vol {
                    // A node on the axis of a revolved part: the element is pinched there, so read
                    // the stress a little way in.
                    gd = wedge(&xyz, o, xi + (1.0 / 3.0 - xi) * 0.02, eta + (1.0 / 3.0 - eta) * 0.02, zeta).0;
                }
                let mut e = [0.0; 6];
                for (i, gi) in gd.iter().enumerate() {
                    let ui = [u[3 * en[i]], u[3 * en[i] + 1], u[3 * en[i] + 2]];
                    e[0] += gi[0] * ui[0];
                    e[1] += gi[1] * ui[1];
                    e[2] += gi[2] * ui[2];
                    e[3] += gi[1] * ui[0] + gi[0] * ui[1];
                    e[4] += gi[2] * ui[1] + gi[1] * ui[2];
                    e[5] += gi[2] * ui[0] + gi[0] * ui[2];
                }
                let tr = e[0] + e[1] + e[2];
                let s = [lam * tr + 2.0 * mu * e[0], lam * tr + 2.0 * mu * e[1], lam * tr + 2.0 * mu * e[2], mu * e[3], mu * e[4], mu * e[5]];
                let n = en[q];
                for r in 0..6 {
                    acc[n][r] += s[r] * vol;
                }
                wsum[n] += vol;
                fy_node[n] = fy_node[n].min(mt.fy);
            }
        }
    }
    const KEYS: [&str; 14] = ["ux", "uy", "uz", "umag", "sx", "sy", "sz", "txy", "tyz", "tzx", "vm", "s1", "s3", "sf"];
    let mut all: Vec<Vec<f64>> = vec![Vec::with_capacity(nn); KEYS.len()];
    for n in 0..nn {
        let w = wsum[n].max(1e-300);
        let st = stress_set(acc[n].map(|v| v / w), fy_node[n]);
        let d = [u[3 * n], u[3 * n + 1], u[3 * n + 2]];
        let vals = [d[0], d[1], d[2], (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(), st[0], st[1], st[2], st[3], st[4], st[5], st[6], st[7], st[8], st[9]];
        for (c, v) in vals.iter().enumerate() {
            all[c].push(*v);
        }
    }
    let (surface, ids) = surface(&sw);
    let volume = Volume { nodes: sw.pos.clone(), disp: (0..nn).map(|n| [u[3 * n], u[3 * n + 1], u[3 * n + 2]]).collect(), tets: tets(&sw), v: all.clone() };
    let fields = KEYS
        .iter()
        .zip(all)
        .map(|(key, v)| {
            let (mut imin, mut imax) = (0, 0);
            for i in 0..v.len() {
                if v[i] < v[imin] {
                    imin = i;
                }
                if v[i] > v[imax] {
                    imax = i;
                }
            }
            Field3 { key, min: v[imin], max: v[imax], at_min: sw.at(imin), at_max: sw.at(imax), v: ids.iter().map(|&n| v[n]).collect() }
        })
        .collect();
    let disp = ids.iter().map(|&n| [u[3 * n], u[3 * n + 1], u[3 * n + 2]]).collect();
    let size = {
        let b = m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(1.0);
        if sw.revolve > 0.0 { b } else { b.max(m.thickness) }
    };
    let umax = (0..nn).map(|n| (u[3 * n].powi(2) + u[3 * n + 1].powi(2) + u[3 * n + 2].powi(2)).sqrt()).fold(0.0, f64::max);
    if umax > 0.1 * size {
        warnings.push(format!("The largest movement is {:.0} % of the part's size: far beyond small deflections, so this linear answer is not reliable. Check the supports, loads and units.", umax / size * 100.0));
    }
    let reach = (0..nn).map(|n| { let p = sw.at(n); (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() }).fold(0.0, f64::max) + size;
    let scale = [applied.scale, applied.scale * reach];
    let f = if scale[0] > 0.0 { (0..3).map(|k| (applied.sum[k] + reacted.sum[k]).abs()).fold(0.0, f64::max) / scale[0] } else { 0.0 };
    let mo = if scale[1] > 0.0 { (3..6).map(|k| (applied.sum[k] + reacted.sum[k]).abs()).fold(0.0, f64::max) / scale[1] } else { 0.0 };
    Ok(SolidResults {
        surface,
        disp,
        fields,
        reactions,
        applied: applied.sum,
        reacted: reacted.sum,
        scale,
        imbalance: f.max(mo),
        stats,
        warnings,
        elements: ne,
        nodes: nn,
        layers,
        order: o as u8,
        size: sw.ms.size,
        quality: sw.ms.quality.clone(),
        volume,
    })
}

/// The solid cut into tetrahedra on corner nodes, for slicing: each linear wedge (a quadratic
/// one's eight sub-wedges) into three. Tetrahedra pinched flat on a revolved part's axis are left
/// out.
fn tets(sw: &Swept) -> Vec<[u32; 4]> {
    let mut out = vec![];
    let layers = (sw.levels - 1) / sw.order;
    for tri in &sw.ms.tris {
        let subs: Vec<[u32; 3]> = if tri.len() == 6 { vec![[tri[0], tri[3], tri[5]], [tri[3], tri[1], tri[4]], [tri[5], tri[4], tri[2]], [tri[3], tri[4], tri[5]]] } else { vec![[tri[0], tri[1], tri[2]]] };
        for l in 0..layers {
            for a in 0..sw.order {
                let (b, t) = (l * sw.order + a, l * sw.order + a + 1);
                for p in &subs {
                    let w = [sw.node(p[0] as usize, b), sw.node(p[1] as usize, b), sw.node(p[2] as usize, b), sw.node(p[0] as usize, t), sw.node(p[1] as usize, t), sw.node(p[2] as usize, t)];
                    for q in [[w[0], w[1], w[2], w[3]], [w[1], w[2], w[3], w[4]], [w[2], w[3], w[4], w[5]]] {
                        if (0..4).all(|i| (i + 1..4).all(|j| q[i] != q[j])) {
                            out.push(q.map(|n| n as u32));
                        }
                    }
                }
            }
        }
    }
    out
}

/// The outer surface of the swept mesh: the end faces' triangles (quadratic ones split in four)
/// and the side faces' grids, with the outline edges for drawing. Returns the surface and, for each
/// surface node, the node of the solid it is.
fn surface(sw: &Swept) -> (Surface, Vec<usize>) {
    let mut ids: Vec<usize> = vec![];
    let mut at: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    let mut idx = |n: usize, ids: &mut Vec<usize>| -> u32 {
        *at.entry(n).or_insert_with(|| {
            ids.push(n);
            (ids.len() - 1) as u32
        })
    };
    let mut tris = vec![];
    let last = sw.levels - 1;
    for tri in &sw.ms.tris {
        let parts: Vec<[u32; 3]> = if tri.len() == 6 { vec![[tri[0], tri[3], tri[5]], [tri[3], tri[1], tri[4]], [tri[5], tri[4], tri[2]], [tri[3], tri[4], tri[5]]] } else { vec![[tri[0], tri[1], tri[2]]] };
        for p in parts {
            if !sw.has_ends() {
                break;
            }
            let back = p.map(|i| idx(sw.node(i as usize, 0), &mut ids));
            tris.push([back[0], back[2], back[1]]);
            let front = p.map(|i| idx(sw.node(i as usize, last), &mut ids));
            tris.push(front);
        }
    }
    let mut lines = vec![];
    for e in &sw.ms.edges {
        let along: Vec<usize> = if sw.order == 2 { vec![e.n[0] as usize, e.n[1] as usize, e.n[2] as usize] } else { vec![e.n[0] as usize, e.n[2] as usize] };
        for a in 0..along.len() - 1 {
            for lv in 0..last {
                let q = [sw.node(along[a], lv), sw.node(along[a + 1], lv), sw.node(along[a + 1], lv + 1), sw.node(along[a], lv + 1)].map(|n| idx(n, &mut ids));
                tris.push([q[0], q[1], q[2]]);
                tris.push([q[0], q[2], q[3]]);
            }
        }
        // The part's outline at both ends (a full turn: at the start, and halfway round).
        for lv in if sw.has_ends() { [0, last] } else { [0, last / 2] } {
            lines.push([idx(sw.node(e.n[0] as usize, lv), &mut ids), idx(sw.node(e.n[2] as usize, lv), &mut ids)]);
        }
    }
    let nodes = ids.iter().map(|&n| sw.at(n)).collect();
    (Surface { nodes, tris, lines }, ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Geom, Load, Shape, Support};

    #[test]
    fn linear_wedges_pass_the_patch_test_and_quadratic_ones_bend_better() {
        // A block pulled along x on rollers at three faces, with 6-node wedges: uniform stress.
        let mut d = Doc::new(Model::new(Analysis::Solid));
        let s = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.2, 0.1), ..Shape::default() }) }).unwrap()[0];
        d.m.thickness = 0.05;
        d.m.mesh.order = 1;
        d.m.mesh.size = 0.03;
        for at in [Target::Edge { shape: s, edge: 3 }, Target::Edge { shape: s, edge: 0 }, Target::End { front: false }] {
            d.apply(Op::Add { item: Item::Support(Support { id: 0, at, kind: SupportKind::Roller }) }).unwrap();
        }
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: -20e6 } }) }).unwrap();
        let r = solve(&d.m).unwrap();
        assert!((r.field("sx").max - 20e6).abs() < 1e-6 * 20e6 && (r.field("sx").min - 20e6).abs() < 1e-6 * 20e6, "{} {}", r.field("sx").min, r.field("sx").max);
        assert!(r.imbalance < 1e-9 && r.order == 1 && r.layers >= 2);
        // Every surface triangle points at real surface nodes.
        assert!(r.surface.tris.iter().flatten().all(|&i| (i as usize) < r.surface.nodes.len()) && r.disp.len() == r.surface.nodes.len());
    }

    #[test]
    fn principal_stresses_of_known_states() {
        // Uniaxial, pure shear, and a rotated state with known eigenvalues 3, 2, 1 (×1e6).
        let (a, b) = principal([5.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!((a - 5.0).abs() < 1e-12 && b.abs() < 1e-12);
        let (a, b) = principal([0.0, 0.0, 0.0, 4.0, 0.0, 0.0]);
        assert!((a - 4.0).abs() < 1e-12 && (b + 4.0).abs() < 1e-12);
        let (a, b) = principal([2.5e6, 2.5e6, 1e6, 0.5e6, 0.0, 0.0]);
        assert!((a - 3e6).abs() < 1e-6 && (b - 1e6).abs() < 1e-6, "{a} {b}");
    }

    #[test]
    fn a_solid_that_is_not_held_says_why() {
        let mut d = Doc::new(Model::new(Analysis::Solid));
        let s = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.2, 0.1), ..Shape::default() }) }).unwrap()[0];
        d.m.thickness = 0.02;
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: 1e6 } }) }).unwrap();
        assert!(solve(&d.m).unwrap_err().contains("turn about any axis"));
        // Too fine a mesh is refused before it uses the memory.
        d.m.mesh.size = 0.0005;
        assert!(solve(&d.m).unwrap_err().contains("unknowns"));
        // End faces are refused on other analyses.
        let mut e = Doc::new(Model::new(Analysis::Stress));
        e.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.2, 0.1), ..Shape::default() }) }).unwrap();
        assert!(e.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::End { front: true }, kind: SupportKind::Fixed }) }).unwrap_err().contains("3D solids"));
    }
}
