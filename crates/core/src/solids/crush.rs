//! Explicit dynamics of 2D parts (0.10.7): an axisymmetric or plane-strain part stepped in time
//! with large deformation, yielding, its folds pressing on each other and on rigid platens. Made
//! for crushing: a tube's wall section, its foot held and its top moved down, folds over and over
//! as a crushed tube does (concertina folding).
//!
//! The method, from the published literature:
//! - Four-node quadrilaterals made by cutting each triangle of the part's mesh into three (from
//!   its edges' midpoints to its centroid), with 2 × 2 Gauss points, total Lagrangian; in an
//!   axisymmetric part the hoop stretch is r/R (Bathe, *Finite Element Procedures*, §6.3).
//! - Selective reduced integration (Hughes 1980): the deviatoric stress at the four Gauss points,
//!   the pressure once, at the element's centre (mean dilatation), so yielding's incompressible
//!   flow does not lock the elements and no spurious modes appear under large compression.
//! - Yielding: J2 plasticity at finite strain with the exponential return map on the principal
//!   logarithmic elastic strains (Simo 1992; Simo and Hughes, *Computational Inelasticity*, §9),
//!   perfectly plastic or with linear hardening.
//! - Time: central differences with lumped masses (Belytschko, Liu and Moran, *Nonlinear Finite
//!   Elements for Continua and Structures*, §6.2); the time step below the smallest element's
//!   wave transit time. Where the step asked for is larger, the density is scaled up so it is
//!   stable (mass scaling, said in the warnings with the kinetic energy it brings).
//! - Contact: every boundary node against every boundary segment of the part not next to it (the
//!   folds pressing on each other) and against platens along the held and moved straight edges,
//!   by penalty forces normal to the surface, without friction.

use crate::model::{Analysis, LoadKind, Model, SupportKind, Target};
use crate::solids::mesh::{self, Mesh};
use crate::solids::plane::{field, PartPath, PlaneResults, Reaction};
use std::collections::HashMap;

/// The settings of a crush (from the explicit step): its duration, the time step asked for (0:
/// the stable one), and the factor on the moved edges against time.
pub struct Crush {
    pub duration: f64,
    pub dt: f64,
    pub curve: Vec<[f64; 2]>,
}

fn factor_at(c: &[[f64; 2]], t: f64, duration: f64) -> f64 {
    if c.len() < 2 {
        // A smooth start and a steady crush: the factor rises as t/T with zero speed at the start
        // over the first tenth, then straight.
        let s = (t / duration).clamp(0.0, 1.0);
        let a = 0.1;
        return if s < a { s * s / (2.0 * a) / (1.0 - a / 2.0) } else { (s - a / 2.0) / (1.0 - a / 2.0) };
    }
    let k = c.windows(2).position(|w| t >= w[0][0] && t <= w[1][0]);
    match k {
        Some(k) => {
            let (a, b) = (c[k], c[k + 1]);
            if b[0] > a[0] { a[1] + (b[1] - a[1]) * (t - a[0]) / (b[0] - a[0]) } else { b[1] }
        }
        None => if t < c[0][0] { c[0][1] } else { c[c.len() - 1][1] },
    }
}

/// The part cut into quadrilaterals: nodes, the four corners of each (counter-clockwise), its
/// material, and for each outline edge (shape, edge) its nodes.
pub(crate) struct Quads {
    pub(crate) nodes: Vec<[f64; 2]>,
    pub(crate) q: Vec<[usize; 4]>,
    pub(crate) mat: Vec<usize>,
    pub(crate) edges: HashMap<(u32, u32), Vec<usize>>,
    /// The outline's pieces, for drawing supports and loads.
    pub(crate) bedges: Vec<crate::solids::mesh::BEdge>,
}

/// A part that is one upright rectangle gets a grid of near-square quadrilaterals about `h`
/// across: even elements, so the time step is not held down by a sliver.
pub(crate) fn grid(m: &Model, h: f64) -> Option<Quads> {
    use crate::model::Geom;
    use crate::solids::mesh::BEdge;
    let [sh] = &m.shapes[..] else { return None };
    let Geom::Poly { pts, bulge } = &sh.geom else { return None };
    if pts.len() != 4 || bulge.iter().any(|b| *b != 0.0) || sh.hole {
        return None;
    }
    let (a, c) = (pts[0][0], pts[0][1]);
    let (b, d) = (pts[2][0], pts[2][1]);
    if !(b > a && d > c && pts[1] == [b, c] && pts[3] == [a, d]) {
        return None;
    }
    let nx = ((b - a) / h).round().max(1.0) as usize;
    let ny = ((d - c) / h).round().max(1.0) as usize;
    let id = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = vec![];
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a + (b - a) * i as f64 / nx as f64, c + (d - c) * j as f64 / ny as f64]);
        }
    }
    let mut q = vec![];
    for j in 0..ny {
        for i in 0..nx {
            q.push([id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)]);
        }
    }
    let lines: [Vec<usize>; 4] = [(0..=nx).map(|i| id(i, 0)).collect(), (0..=ny).map(|j| id(nx, j)).collect(), (0..=nx).rev().map(|i| id(i, ny)).collect(), (0..=ny).rev().map(|j| id(0, j)).collect()];
    let mut edges = HashMap::new();
    let mut bedges = vec![];
    for (e, l) in lines.iter().enumerate() {
        edges.insert((sh.id, e as u32), l.clone());
        for w in l.windows(2) {
            bedges.push(BEdge { shape: sh.id, edge: e as u32, n: [w[0] as u32, u32::MAX, w[1] as u32] });
        }
    }
    let mat = vec![sh.material as usize; q.len()];
    Some(Quads { nodes, q, mat, edges, bedges })
}

pub(crate) fn quads(ms: &Mesh) -> Quads {
    let mut nodes = ms.nodes.clone();
    let mut mid: HashMap<(u32, u32), usize> = HashMap::new();
    let mut at_mid = |a: u32, b: u32, nodes: &mut Vec<[f64; 2]>| -> usize {
        let key = (a.min(b), a.max(b));
        *mid.entry(key).or_insert_with(|| {
            let (p, q) = (nodes[a as usize], nodes[b as usize]);
            nodes.push([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]);
            nodes.len() - 1
        })
    };
    let mut q = vec![];
    let mut mat = vec![];
    for (k, t) in ms.tris.iter().enumerate() {
        let (a, b, c) = (t[0], t[1], t[2]);
        let (mab, mbc, mca) = (at_mid(a, b, &mut nodes), at_mid(b, c, &mut nodes), at_mid(c, a, &mut nodes));
        let (pa, pb, pc) = (nodes[a as usize], nodes[b as usize], nodes[c as usize]);
        nodes.push([(pa[0] + pb[0] + pc[0]) / 3.0, (pa[1] + pb[1] + pc[1]) / 3.0]);
        let g = nodes.len() - 1;
        let m = ms.mat.get(k).copied().unwrap_or(1) as usize;
        for quad in [[a as usize, mab, g, mca], [b as usize, mbc, g, mab], [c as usize, mca, g, mbc]] {
            q.push(quad);
            mat.push(m);
        }
    }
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    let mut bedges = vec![];
    for e in &ms.edges {
        let (a, b) = (e.n[0], e.n[2]);
        let m = at_mid(a, b, &mut nodes);
        bedges.push(crate::solids::mesh::BEdge { shape: e.shape, edge: e.edge, n: [a, u32::MAX, m as u32] });
        bedges.push(crate::solids::mesh::BEdge { shape: e.shape, edge: e.edge, n: [m as u32, u32::MAX, b] });
        let v = edges.entry((e.shape, e.edge)).or_default();
        for n in [a as usize, m, b as usize] {
            if !v.contains(&n) {
                v.push(n);
            }
        }
    }
    Quads { nodes, q, mat, edges, bedges }
}

/// One Gauss point's geometry and state.
#[derive(Clone)]
struct Gp {
    dn: [[f64; 2]; 4],
    n: [f64; 4],
    r: f64,
    dv: f64,
    /// Elastic left Cauchy–Green tensor (b11, b22, b12, b33) and the deformation gradient at the
    /// last step (F11, F12, F21, F22, F33), the plastic strain and the last Cauchy stress
    /// (σrr, σzz, σrz, σθθ).
    be: [f64; 4],
    f: [f64; 5],
    ep: f64,
    sig: [f64; 4],
}

pub(crate) const G: f64 = 0.577_350_269_189_625_8;

/// Shape-function derivatives of a quadrilateral at (ξ, η), against x and y, with N and det J.
pub(crate) fn shape(x: &[[f64; 2]; 4], xi: f64, eta: f64) -> Option<([[f64; 2]; 4], [f64; 4], f64)> {
    let s = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    let mut n = [0.0; 4];
    let mut dxi = [[0.0; 2]; 4];
    for a in 0..4 {
        n[a] = 0.25 * (1.0 + s[a][0] * xi) * (1.0 + s[a][1] * eta);
        dxi[a] = [0.25 * s[a][0] * (1.0 + s[a][1] * eta), 0.25 * s[a][1] * (1.0 + s[a][0] * xi)];
    }
    let mut j = [[0.0; 2]; 2];
    for a in 0..4 {
        for i in 0..2 {
            for k in 0..2 {
                j[i][k] += dxi[a][k] * x[a][i];
            }
        }
    }
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    if !(det > 0.0) {
        return None;
    }
    let inv = [[j[1][1] / det, -j[0][1] / det], [-j[1][0] / det, j[0][0] / det]];
    let mut dn = [[0.0; 2]; 4];
    for a in 0..4 {
        for i in 0..2 {
            dn[a][i] = dxi[a][0] * inv[0][i] + dxi[a][1] * inv[1][i];
        }
    }
    Some((dn, n, det))
}

/// Eigenvalues and the first eigenvector's angle of a symmetric 2 × 2 tensor (a11, a22, a12).
fn eig2(a11: f64, a22: f64, a12: f64) -> (f64, f64, f64) {
    let m = 0.5 * (a11 + a22);
    let d = (0.25 * (a11 - a22).powi(2) + a12 * a12).sqrt();
    let th = 0.5 * (2.0 * a12).atan2(a11 - a22);
    (m + d, m - d, th)
}

/// The material of an element: bulk and shear moduli, density, yield stress and hardening.
#[derive(Clone, Copy)]
struct Mat {
    k: f64,
    mu: f64,
    rho: f64,
    fy: f64,
    h: f64,
}

/// The deviatoric stress at a Gauss point for the deformation gradient `f1` (F11, F12, F21, F22,
/// F33), with the element's pressure `pk` (Kirchhoff, from its centre) and volume change `jbar` for
/// the stress shown; updates the point's state. Returns the deviatoric first Piola–Kirchhoff
/// stress (P11, P12, P21, P22, P33).
fn stress(g: &mut Gp, f1: [f64; 5], jbar: f64, pk: f64, m: &Mat) -> [f64; 5] {
    let f0 = g.f;
    let d0 = f0[0] * f0[3] - f0[1] * f0[2];
    // f = F₁ F₀⁻¹ (the step's own deformation).
    let i0 = [f0[3] / d0, -f0[1] / d0, -f0[2] / d0, f0[0] / d0];
    let fr = [f1[0] * i0[0] + f1[1] * i0[2], f1[0] * i0[1] + f1[1] * i0[3], f1[2] * i0[0] + f1[3] * i0[2], f1[2] * i0[1] + f1[3] * i0[3]];
    let f33 = f1[4] / f0[4];
    let b = g.be;
    // b_trial = f b fᵀ, in the r–z plane and the hoop direction.
    let fb = [fr[0] * b[0] + fr[1] * b[2], fr[0] * b[2] + fr[1] * b[1], fr[2] * b[0] + fr[3] * b[2], fr[2] * b[2] + fr[3] * b[1]];
    let bt = [fb[0] * fr[0] + fb[1] * fr[1], fb[2] * fr[2] + fb[3] * fr[3], fb[0] * fr[2] + fb[1] * fr[3], b[3] * f33 * f33];
    let (l1, l2, th) = eig2(bt[0], bt[1], bt[2]);
    let mut e = [0.5 * l1.max(1e-300).ln(), 0.5 * l2.max(1e-300).ln(), 0.5 * bt[3].max(1e-300).ln()];
    // The point's own volume change stays in b_e; only the deviatoric part makes stress here.
    let ev = e[0] + e[1] + e[2];
    let mut s: [f64; 3] = std::array::from_fn(|i| 2.0 * m.mu * (e[i] - ev / 3.0));
    let q = (1.5 * s.iter().map(|x| x * x).sum::<f64>()).sqrt();
    let sy = m.fy + m.h * g.ep;
    if m.fy > 0.0 && q > sy {
        let dg = (q - sy) / (3.0 * m.mu + m.h);
        let k = 1.0 - 3.0 * m.mu * dg / q;
        for (i, x) in s.iter_mut().enumerate() {
            *x *= k;
            e[i] = *x / (2.0 * m.mu) + ev / 3.0;
        }
        g.ep += dg;
    }
    let tau = s;
    let (c, sn) = (th.cos(), th.sin());
    // Back to r–z: τ = Σ τᵢ nᵢ ⊗ nᵢ; b_e likewise from exp(2 eᵢ).
    let t11 = tau[0] * c * c + tau[1] * sn * sn;
    let t22 = tau[0] * sn * sn + tau[1] * c * c;
    let t12 = (tau[0] - tau[1]) * c * sn;
    let (x1, x2) = ((2.0 * e[0]).exp(), (2.0 * e[1]).exp());
    g.be = [x1 * c * c + x2 * sn * sn, x1 * sn * sn + x2 * c * c, (x1 - x2) * c * sn, (2.0 * e[2]).exp()];
    g.f = f1;
    g.sig = [(t11 + pk) / jbar, (t22 + pk) / jbar, t12 / jbar, (tau[2] + pk) / jbar];
    // P = τ F⁻ᵀ.
    let d1 = f1[0] * f1[3] - f1[1] * f1[2];
    let it = [f1[3] / d1, -f1[2] / d1, -f1[1] / d1, f1[0] / d1]; // F⁻ᵀ: (11, 12, 21, 22)
    [t11 * it[0] + t12 * it[2], t11 * it[1] + t12 * it[3], t12 * it[0] + t22 * it[2], t12 * it[1] + t22 * it[3], tau[2] / f1[4]]
}

/// What a crush gives: the plane results with the force against the crush, and frames to play.
pub fn solve(m: &Model, cr: &Crush) -> Result<PlaneResults, String> {
    let axi = match m.analysis {
        Analysis::Axi => true,
        Analysis::Strain => false,
        _ => return Err("Crushing in time is for axisymmetric and plane-strain parts.".into()),
    };
    if !(cr.duration > 0.0) {
        return Err("A crush in time needs a length of time above zero (Solve › Steps).".into());
    }
    let t0 = std::time::Instant::now();
    let mut lin = m.clone();
    lin.mesh.order = 1;
    let ms = mesh::mesh(&lin)?;
    let qd = grid(m, ms.size).unwrap_or_else(|| quads(&ms));
    let nn = qd.nodes.len();
    let x0 = &qd.nodes;
    // Materials.
    let mat_of = |id: usize| -> Result<Mat, String> {
        let mt = m.materials.iter().find(|x| x.id as usize == id).ok_or("A part's material is missing.")?;
        let (e, nu) = (mt.e, mt.nu);
        if !(e > 0.0 && nu > -1.0 && nu < 0.5 && mt.rho > 0.0) {
            return Err(format!("{} needs a stiffness, a Poisson's ratio below 0.5 and a density to be crushed in time.", mt.name));
        }
        Ok(Mat { k: e / (3.0 * (1.0 - 2.0 * nu)), mu: e / (2.0 * (1.0 + nu)), rho: mt.rho, fy: mt.fy, h: mt.hardening })
    };
    let mats: Vec<Mat> = qd.mat.iter().map(|&k| mat_of(k)).collect::<Result<_, _>>()?;
    let thick = if axi { 1.0 } else { m.thickness.max(1e-12) };
    // Gauss points, masses.
    let mut gps: Vec<[Gp; 4]> = vec![];
    let mut centre: Vec<([[f64; 2]; 4], [f64; 4])> = vec![];
    let mut mass = vec![0.0; nn];
    let mut hmin = f64::INFINITY;
    let mut lens = vec![];
    for (k, q) in qd.q.iter().enumerate() {
        let x: [[f64; 2]; 4] = std::array::from_fn(|a| x0[q[a]]);
        let mut e4: Vec<Gp> = vec![];
        for (xi, eta) in [(-G, -G), (G, -G), (G, G), (-G, G)] {
            let (dn, n, det) = shape(&x, xi, eta).ok_or("A quadrilateral of the crush mesh is folded: use a finer or more even mesh.")?;
            let r = (0..4).map(|a| n[a] * x[a][0]).sum::<f64>();
            if axi && r <= 0.0 {
                return Err("An axisymmetric part must lie to the right of the axis (x > 0).".into());
            }
            let dv = det * if axi { 2.0 * std::f64::consts::PI * r } else { thick };
            for a in 0..4 {
                mass[q[a]] += mats[k].rho * n[a] * dv;
            }
            e4.push(Gp { dn, n, r, dv, be: [1.0, 1.0, 0.0, 1.0], f: [1.0, 0.0, 0.0, 1.0, 1.0], ep: 0.0, sig: [0.0; 4] });
        }
        let (dn, n, _) = shape(&x, 0.0, 0.0).ok_or("A quadrilateral of the crush mesh is folded.")?;
        centre.push((dn, n));
        gps.push([e4[0].clone(), e4[1].clone(), e4[2].clone(), e4[3].clone()]);
        // The element's length for the time step: its area over its longest edge.
        let area = 0.5 * ((x[2][0] - x[0][0]) * (x[3][1] - x[1][1]) - (x[3][0] - x[1][0]) * (x[2][1] - x[0][1])).abs();
        let longest = (0..4).map(|a| (x[a][0] - x[(a + 1) % 4][0]).hypot(x[a][1] - x[(a + 1) % 4][1])).fold(0.0, f64::max);
        hmin = hmin.min((area / longest.max(1e-12)).max(1e-12));
        lens.push(area.sqrt());
    }
    // Supports and moved edges.
    let mut held = vec![[false; 2]; nn];
    let mut moved: Vec<(usize, [f64; 2])> = vec![];
    let mut moved_edges: Vec<(u32, u32, [f64; 2])> = vec![];
    let mut held_edges: Vec<(u32, u32)> = vec![];
    let nodes_of = |t: &Target| -> Result<Vec<usize>, String> {
        match t {
            Target::Edge { shape, edge } => qd.edges.get(&(*shape, *edge)).cloned().ok_or("A support or moved edge is on an edge the mesh does not have.".to_string()),
            _ => Err("Crushing in time takes supports and moved amounts on the part's edges.".into()),
        }
    };
    for s in &m.supports {
        let ns = nodes_of(&s.at)?;
        let h = match s.kind {
            SupportKind::Fixed | SupportKind::Pinned => [true, true],
            SupportKind::RollerX => [false, true],
            SupportKind::RollerY => [true, false],
            SupportKind::Symmetry | SupportKind::Roller => {
                // Across the edge: x for an upright edge, y for a level one.
                let (a, b) = (x0[ns[0]], x0[*ns.last().unwrap()]);
                if (b[0] - a[0]).abs() < (b[1] - a[1]).abs() { [true, false] } else { [false, true] }
            }
            _ => return Err("Crushing in time takes fixed, pinned, roller and symmetry supports.".into()),
        };
        if let Target::Edge { shape, edge } = s.at {
            if h == [true, true] {
                held_edges.push((shape, edge));
            }
        }
        for n in ns {
            held[n][0] |= h[0];
            held[n][1] |= h[1];
        }
    }
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Settlement { dx, dy, .. }, Target::Edge { shape, edge }) => {
                for n in nodes_of(&l.at)? {
                    moved.push((n, [*dx, *dy]));
                }
                moved_edges.push((*shape, *edge, [*dx, *dy]));
            }
            _ => return Err("Crushing in time takes only edges moved by set amounts (Supports & Loads › Moved Edge); other loads are not yet solved in time.".into()),
        }
    }
    if moved.is_empty() {
        return Err("Nothing moves: give an edge a set movement (the platen's travel), and hold the opposite edge.".into());
    }
    // A wall for each held or moved straight edge: the platen it is pressed on.
    let mut platens: Vec<([f64; 2], [f64; 2], [f64; 2])> = vec![]; // (point, outward normal, its travel)
    let outward = |ns: &[usize]| -> Option<([f64; 2], [f64; 2])> {
        let (a, b) = (x0[ns[0]], x0[*ns.last().unwrap()]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        if len == 0.0 {
            return None;
        }
        let t = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
        // Straight: every node on the line.
        if ns.iter().any(|&n| ((x0[n][0] - a[0]) * t[1] - (x0[n][1] - a[1]) * t[0]).abs() > 1e-9 * len) {
            return None;
        }
        // Outward: away from the part's centre of mass.
        let (cx, cy) = (x0.iter().map(|p| p[0]).sum::<f64>() / nn as f64, x0.iter().map(|p| p[1]).sum::<f64>() / nn as f64);
        let mut nrm = [t[1], -t[0]];
        if (a[0] - cx) * nrm[0] + (a[1] - cy) * nrm[1] < 0.0 {
            nrm = [-nrm[0], -nrm[1]];
        }
        Some((a, nrm))
    };
    for (s, e) in &held_edges {
        if let Some((p, n)) = qd.edges.get(&(*s, *e)).and_then(|ns| outward(ns)) {
            platens.push((p, n, [0.0, 0.0]));
        }
    }
    for (s, e, d) in &moved_edges {
        if let Some((p, n)) = qd.edges.get(&(*s, *e)).and_then(|ns| outward(ns)) {
            platens.push((p, n, *d));
        }
    }
    let is_moved: Vec<bool> = { let mut v = vec![false; nn]; for (n, _) in &moved { v[*n] = true; } v };
    // A moved node follows its movement in the directions it moves in (a support there does not
    // hold it back); across, it is free unless a support holds it.
    for &(n, d) in &moved {
        for c in 0..2 {
            if d[c] != 0.0 {
                held[n][c] = false;
            }
        }
    }
    // The boundary: edges of one quadrilateral, counter-clockwise, and who neighbours whom.
    let mut count: HashMap<(usize, usize), (usize, usize, usize)> = HashMap::new();
    for q in &qd.q {
        for a in 0..4 {
            let (i, j) = (q[a], q[(a + 1) % 4]);
            let e = count.entry((i.min(j), i.max(j))).or_insert((i, j, 0));
            e.2 += 1;
        }
    }
    let segs: Vec<(usize, usize)> = count.values().filter(|v| v.2 == 1).map(|v| (v.0, v.1)).collect();
    let mut bnodes: Vec<usize> = segs.iter().flat_map(|s| [s.0, s.1]).collect();
    bnodes.sort_unstable();
    bnodes.dedup();
    let mut trib = vec![0.0; nn];
    for &(a, b) in &segs {
        let l = (x0[a][0] - x0[b][0]).hypot(x0[a][1] - x0[b][1]);
        trib[a] += l / 2.0;
        trib[b] += l / 2.0;
    }
    // Who shares an element with whom (contact skips them).
    let mut near: Vec<Vec<usize>> = vec![vec![]; nn];
    for q in &qd.q {
        for &a in q {
            for &b in q {
                if !near[a].contains(&b) {
                    near[a].push(b);
                }
            }
        }
    }
    // Time step: the stable one, and mass scaling to reach the one asked for.
    let cmax = mats.iter().map(|m| ((m.k + 4.0 / 3.0 * m.mu) / m.rho).sqrt()).fold(0.0, f64::max);
    let dt_stable = 0.6 * hmin / cmax;
    let mut warnings = vec![];
    let mut scale = 1.0;
    let dt = if cr.dt > 0.0 && cr.dt > dt_stable {
        scale = (cr.dt / dt_stable).powi(2);
        warnings.push(format!("Mass scaling: the density is taken {scale:.0} times larger so the time step asked for ({:.2e} s) is stable (the stable one is {:.2e} s). Fine for a slow crush while the kinetic energy stays small against the work done (see below).", cr.dt, dt_stable));
        cr.dt
    } else if cr.dt > 0.0 {
        cr.dt
    } else {
        dt_stable
    };
    for x in &mut mass {
        *x *= scale;
    }
    // Each element's own mass factor: raised where squashing makes it too short for the step
    // (selective mass scaling), and the mass that adds.
    let mut escale = vec![scale; qd.q.len()];
    let evol: Vec<f64> = gps.iter().map(|e| e.iter().map(|g| g.dv).sum()).collect();
    let total_mass: f64 = mass.iter().sum();
    let mut added = 0.0;
    // The platen's travel against time: the step's curve when it starts from nothing, else a
    // smooth start and a steady crush (the usual curve of a step is a load held at full value).
    let curve: Vec<[f64; 2]> = if cr.curve.len() >= 2 && cr.curve[0][1] == 0.0 { cr.curve.clone() } else { vec![] };
    let steps = (cr.duration / dt).ceil() as usize;
    if steps > 2_000_000 {
        return Err(format!("{steps} time steps would take too long: shorten the time, use a larger element size, or set a larger time step (it is made stable by mass scaling)."));
    }
    let emin = mats.iter().map(|m| 9.0 * m.k * m.mu / (3.0 * m.k + m.mu)).fold(f64::INFINITY, f64::min);
    // Contact: pressure per penetration and the depth it is looked for, from the typical element.
    lens.sort_by(f64::total_cmp);
    let htyp = lens[lens.len() / 2];
    let kpen = 0.5 * emin / htyp;
    let dmax = 0.5 * htyp;
    let mut u = vec![[0.0f64; 2]; nn];
    let mut v = vec![[0.0f64; 2]; nn];
    let mut acc = vec![[0.0f64; 2]; nn];
    let mut fint = vec![[0.0f64; 2]; nn];
    let mut fc = vec![[0.0f64; 2]; nn];
    let mut snaps: Vec<(f64, Vec<[f64; 2]>, Vec<f64>)> = vec![];
    let (mut crush, mut force, mut ke_rec, mut work_rec, mut peq_rec, mut times) = (vec![0.0], vec![0.0], vec![0.0], vec![0.0], vec![0.0], vec![0.0]);
    let mut work = 0.0;
    let travel = moved.iter().map(|(_, d)| d[0].hypot(d[1])).fold(0.0, f64::max);
    let dir = { let d = moved[0].1; let l = d[0].hypot(d[1]).max(1e-300); [d[0] / l, d[1] / l] };
    let every = (steps / 400).max(1);
    let snap_every = (steps / 60).max(1);
    let mut bottom_force = 0.0;
    for step in 1..=steps {
        if step % 50 == 0 && crate::progress::stopping() {
            warnings.push(format!("Stopped at {:.3e} s of {:.3e} s.", step as f64 * dt, cr.duration));
            break;
        }
        let t = step as f64 * dt;
        // Velocities at the half step, then the movements.
        let fa = factor_at(&curve, t, cr.duration);
        let fa0 = factor_at(&curve, t - dt, cr.duration);
        for i in 0..nn {
            for c in 0..2 {
                v[i][c] += dt * acc[i][c];
                if held[i][c] {
                    v[i][c] = 0.0;
                }
            }
        }
        for &(n, d) in &moved {
            for c in 0..2 {
                if d[c] != 0.0 {
                    v[n][c] = d[c] * (fa - fa0) / dt;
                }
            }
        }
        for i in 0..nn {
            for c in 0..2 {
                u[i][c] += dt * v[i][c];
            }
        }
        for &(n, d) in &moved {
            for c in 0..2 {
                if d[c] != 0.0 {
                    u[n][c] = d[c] * fa;
                }
            }
        }
        // Squashed elements: keep each stable at this step by giving it mass where it needs it.
        if step % 10 == 1 {
            for (k, q) in qd.q.iter().enumerate() {
                let x: [[f64; 2]; 4] = std::array::from_fn(|a| [x0[q[a]][0] + u[q[a]][0], x0[q[a]][1] + u[q[a]][1]]);
                let area = 0.5 * ((x[2][0] - x[0][0]) * (x[3][1] - x[1][1]) - (x[3][0] - x[1][0]) * (x[2][1] - x[0][1])).abs();
                let longest = (0..4).map(|a| (x[a][0] - x[(a + 1) % 4][0]).hypot(x[a][1] - x[(a + 1) % 4][1])).fold(0.0, f64::max);
                let le = area / longest.max(1e-300);
                let c = ((mats[k].k + 4.0 / 3.0 * mats[k].mu) / (mats[k].rho * escale[k])).sqrt();
                let dte = 0.6 * le / c;
                if dte < dt {
                    let s2 = escale[k] * (dt / dte).powi(2) * 1.1;
                    let dm = mats[k].rho * evol[k] * (s2 - escale[k]);
                    for &a in q {
                        mass[a] += dm / 4.0;
                    }
                    added += dm;
                    escale[k] = s2;
                }
            }
        }
        // Internal forces.
        for f in fint.iter_mut() {
            *f = [0.0; 2];
        }
        let mut bad = false;
        let mut bad_at = [0.0f64; 2];
        for (k, q) in qd.q.iter().enumerate() {
            let grad = |dn: &[[f64; 2]; 4], n: &[f64; 4], r: f64| -> [f64; 5] {
                let mut f = [1.0, 0.0, 0.0, 1.0, 1.0];
                for a in 0..4 {
                    let ua = u[q[a]];
                    f[0] += ua[0] * dn[a][0];
                    f[1] += ua[0] * dn[a][1];
                    f[2] += ua[1] * dn[a][0];
                    f[3] += ua[1] * dn[a][1];
                    if axi {
                        f[4] += n[a] * ua[0] / r;
                    }
                }
                f
            };
            let (cdn, cn) = &centre[k];
            let rc = (0..4).map(|a| cn[a] * x0[q[a]][0]).sum::<f64>();
            let fc0 = grad(cdn, cn, rc);
            let jc = (fc0[0] * fc0[3] - fc0[1] * fc0[2]) * fc0[4];
            if !(jc > 0.0) {
                bad = true;
                bad_at = [(0..4).map(|a| x0[q[a]][0] + u[q[a]][0]).sum::<f64>() / 4.0, (0..4).map(|a| x0[q[a]][1] + u[q[a]][1]).sum::<f64>() / 4.0];
                continue;
            }
            // The pressure once, at the centre (mean dilatation): Kirchhoff pressure K ln J there.
            let pk = mats[k].k * jc.ln();
            {
                let d = fc0[0] * fc0[3] - fc0[1] * fc0[2];
                let it = [fc0[3] / d, -fc0[2] / d, -fc0[1] / d, fc0[0] / d];
                let vol: f64 = gps[k].iter().map(|g| g.dv).sum();
                let (p11, p12, p21, p22, p33) = (pk * it[0], pk * it[1], pk * it[2], pk * it[3], pk / fc0[4]);
                for a in 0..4 {
                    let (dr, dz) = (cdn[a][0], cdn[a][1]);
                    fint[q[a]][0] += (p11 * dr + p12 * dz + if axi { p33 * cn[a] / rc } else { 0.0 }) * vol;
                    fint[q[a]][1] += (p21 * dr + p22 * dz) * vol;
                }
            }
            for g in gps[k].iter_mut() {
                let f1 = grad(&g.dn, &g.n, g.r);
                let j = (f1[0] * f1[3] - f1[1] * f1[2]) * f1[4];
                if !(j > 0.0) {
                    bad = true;
                    bad_at = [(0..4).map(|a| x0[q[a]][0] + u[q[a]][0]).sum::<f64>() / 4.0, (0..4).map(|a| x0[q[a]][1] + u[q[a]][1]).sum::<f64>() / 4.0];
                    continue;
                }
                let p = stress(g, f1, jc, pk, &mats[k]);
                for a in 0..4 {
                    let (dr, dz) = (g.dn[a][0], g.dn[a][1]);
                    fint[q[a]][0] += (p[0] * dr + p[1] * dz + if axi { p[4] * g.n[a] / g.r } else { 0.0 }) * g.dv;
                    fint[q[a]][1] += (p[2] * dr + p[3] * dz) * g.dv;
                }
            }
        }
        if bad {
            let pe = gps.iter().flatten().map(|g| g.ep).fold(0.0, f64::max);
            warnings.push(format!("An element turned inside out at {:.3e} s, at x = {:.2} mm, y = {:.2} mm (the largest plastic strain then {:.2}): the solve stopped there. Use a finer mesh or a slower crush.", t, bad_at[0] * 1e3, bad_at[1] * 1e3, pe));
            break;
        }
        // Contact: the folds on each other and on the platens.
        for f in fc.iter_mut() {
            *f = [0.0; 2];
        }
        let xc = |i: usize| [x0[i][0] + u[i][0], x0[i][1] + u[i][1]];
        let area = |i: usize| trib[i] * if axi { 2.0 * std::f64::consts::PI * xc(i)[0].max(1e-9) } else { thick };
        // Segments by the cells of a grid they reach (with the contact distance round them).
        let cur: Vec<([f64; 2], [f64; 2])> = segs.iter().map(|&(a, b)| (xc(a), xc(b))).collect();
        let cell = cur.iter().map(|(p, q)| (p[0] - q[0]).hypot(p[1] - q[1])).fold(0.0, f64::max) + 2.0 * dmax;
        let key = |x: f64, y: f64| ((x / cell).floor() as i64, (y / cell).floor() as i64);
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (k, (p, q)) in cur.iter().enumerate() {
            let (i0, j0) = key(p[0].min(q[0]) - dmax, p[1].min(q[1]) - dmax);
            let (i1, j1) = key(p[0].max(q[0]) + dmax, p[1].max(q[1]) + dmax);
            for i in i0..=i1 {
                for j in j0..=j1 {
                    grid.entry((i, j)).or_default().push(k);
                }
            }
        }
        for &p in &bnodes {
            let xp = xc(p);
            let Some(cands) = grid.get(&key(xp[0], xp[1])) else { continue };
            for &k in cands {
                let (a, b) = segs[k];
                if a == p || b == p || near[p].contains(&a) || near[p].contains(&b) {
                    continue;
                }
                let (xa, xb) = (xc(a), xc(b));
                let (tx, ty) = (xb[0] - xa[0], xb[1] - xa[1]);
                let l2 = tx * tx + ty * ty;
                if l2 == 0.0 {
                    continue;
                }
                let s = ((xp[0] - xa[0]) * tx + (xp[1] - xa[1]) * ty) / l2;
                if !(0.0..=1.0).contains(&s) {
                    continue;
                }
                let l = l2.sqrt();
                let nrm = [ty / l, -tx / l];
                let g = (xp[0] - xa[0]) * nrm[0] + (xp[1] - xa[1]) * nrm[1];
                if g < 0.0 && g > -dmax {
                    // Penalty capped by the masses and the time step, so contact stays stable.
                    let k = (kpen * area(p)).min(0.2 * mass[p].min(0.5 * (mass[a] + mass[b])) / (dt * dt));
                    let fmag = k * (-g);
                    fc[p][0] += fmag * nrm[0];
                    fc[p][1] += fmag * nrm[1];
                    fc[a][0] -= (1.0 - s) * fmag * nrm[0];
                    fc[a][1] -= (1.0 - s) * fmag * nrm[1];
                    fc[b][0] -= s * fmag * nrm[0];
                    fc[b][1] -= s * fmag * nrm[1];
                }
            }
        }
        let mut platen_force = 0.0;
        for &(p0, nrm, d) in &platens {
            let at = [p0[0] + d[0] * fa, p0[1] + d[1] * fa];
            for &p in &bnodes {
                if held[p] == [true, true] || is_moved[p] {
                    continue;
                }
                let xp = xc(p);
                let g = (xp[0] - at[0]) * nrm[0] + (xp[1] - at[1]) * nrm[1];
                if g > 0.0 {
                    let fmag = (kpen * area(p)).min(0.2 * mass[p] / (dt * dt)) * g;
                    fc[p][0] -= fmag * nrm[0];
                    fc[p][1] -= fmag * nrm[1];
                    if d != [0.0, 0.0] {
                        platen_force += fmag * (-(nrm[0] * dir[0] + nrm[1] * dir[1]));
                    }
                }
            }
        }
        // Accelerations.
        for i in 0..nn {
            for c in 0..2 {
                acc[i][c] = (fc[i][c] - fint[i][c]) / mass[i];
            }
        }
        // The force on the moved edges along their travel, and the work done.
        let fmoved: f64 = moved.iter().map(|&(n, _)| fint[n][0] * dir[0] + fint[n][1] * dir[1]).sum::<f64>() + platen_force;
        let du = travel * (fa - fa0);
        work += fmoved * du;
        if step % every == 0 || step == steps {
            let ke: f64 = (0..nn).map(|i| 0.5 * mass[i] * (v[i][0] * v[i][0] + v[i][1] * v[i][1])).sum();
            bottom_force = (0..nn).filter(|&i| held[i] == [true, true]).map(|i| fint[i][0] * dir[0] + fint[i][1] * dir[1]).sum::<f64>();
            crush.push(travel * fa);
            force.push(fmoved);
            ke_rec.push(ke);
            work_rec.push(work);
            peq_rec.push(gps.iter().flatten().map(|g| g.ep).fold(0.0, f64::max));
            times.push(t);
            crate::progress::time(t, cr.duration, step);
        }
        if step % snap_every == 0 || step == steps {
            let vm: Vec<f64> = nodal(&qd, &gps, nn, |g| vm_of(&g.sig));
            snaps.push((t, u.clone(), vm));
        }
    }
    let _ = bottom_force;
    // Results at the end.
    let tris: Vec<Vec<u32>> = qd.q.iter().flat_map(|q| [vec![q[0] as u32, q[1] as u32, q[2] as u32], vec![q[0] as u32, q[2] as u32, q[3] as u32]]).collect();
    let mat2: Vec<u32> = qd.mat.iter().flat_map(|&m| [m as u32, m as u32]).collect();
    let mesh = Mesh { nodes: qd.nodes.clone(), tris: tris.clone(), order: 1, mat: mat2, edges: qd.bedges.clone(), points: vec![], size: ms.size, quality: ms.quality.clone() };
    let draw: Vec<[u32; 3]> = tris.iter().map(|t| [t[0], t[1], t[2]]).collect();
    let nodes = &qd.nodes;
    let s = |i: usize| nodal(&qd, &gps, nn, |g| g.sig[i]);
    let (sx, sy, txy, sz) = (s(0), s(1), s(2), s(3));
    let vm = nodal(&qd, &gps, nn, |g| vm_of(&g.sig));
    let peeq = nodal(&qd, &gps, nn, |g| g.ep);
    let p1: Vec<f64> = (0..nn).map(|i| 0.5 * (sx[i] + sy[i]) + (0.25 * (sx[i] - sy[i]).powi(2) + txy[i] * txy[i]).sqrt()).collect();
    let p2: Vec<f64> = (0..nn).map(|i| 0.5 * (sx[i] + sy[i]) - (0.25 * (sx[i] - sy[i]).powi(2) + txy[i] * txy[i]).sqrt()).collect();
    let fy_of = |i: usize| -> f64 { let _ = i; m.materials.first().map_or(0.0, |x| x.fy) };
    let sf: Vec<f64> = (0..nn).map(|i| if fy_of(i) > 0.0 && vm[i] > 0.0 { fy_of(i) / vm[i] } else { 0.0 }).collect();
    let fields = vec![
        field("ux", u.iter().map(|d| d[0]).collect(), nodes),
        field("uy", u.iter().map(|d| d[1]).collect(), nodes),
        field("umag", u.iter().map(|d| d[0].hypot(d[1])).collect(), nodes),
        field("sx", sx, nodes),
        field("sy", sy, nodes),
        field("txy", txy, nodes),
        field("sz", sz, nodes),
        field("vm", vm.clone(), nodes),
        field("s1", p1, nodes),
        field("s2", p2, nodes),
        field("sf", sf, nodes),
        field("peeq", peeq, nodes),
    ];
    let centroids: Vec<[f64; 2]> = tris.iter().map(|t| { let p: Vec<[f64; 2]> = t.iter().map(|&i| nodes[i as usize]).collect(); [(p[0][0] + p[1][0] + p[2][0]) / 3.0, (p[0][1] + p[1][1] + p[2][1]) / 3.0] }).collect();
    let el = |f: &dyn Fn(&Gp) -> f64| -> Vec<f64> { gps.iter().flat_map(|e| { let v = e.iter().map(f).sum::<f64>() / 4.0; [v, v] }).collect() };
    let elem = vec![
        field("sx", el(&|g| g.sig[0]), &centroids),
        field("sy", el(&|g| g.sig[1]), &centroids),
        field("txy", el(&|g| g.sig[2]), &centroids),
        field("sz", el(&|g| g.sig[3]), &centroids),
        field("vm", el(&|g| vm_of(&g.sig)), &centroids),
    ];
    let last_ke = ke_rec.last().copied().unwrap_or(0.0);
    let last_w = work_rec.last().copied().unwrap_or(0.0);
    let peak_ke = ke_rec.iter().cloned().fold(0.0, f64::max);
    warnings.push(format!(
        "Explicit crush: {} time steps of {:.2e} s, {} quadrilaterals{}. Work done {:.4} J; kinetic energy at most {:.4} J ({:.1} % of the work at the end): {}",
        times.len().max(1).saturating_sub(1) * every,
        dt,
        qd.q.len(),
        if grid(m, ms.size).is_some() { " (a grid over the rectangle)" } else { " (each triangle of the mesh cut into three)" },
        last_w,
        peak_ke,
        if last_w > 0.0 { 100.0 * last_ke / last_w } else { 0.0 },
        if last_w > 0.0 && last_ke < 0.05 * last_w { "the crush is slow enough to stand for a quasi-static one." } else { "inertia matters: the crush is fast (or the mass scaled too far) to stand for a slow one." }
    ));
    if added > 0.0 {
        warnings.push(format!("Squashed elements were given mass to stay stable at the time step: {:.2} % of the part's mass added (selective mass scaling).", 100.0 * added / total_mass));
    }
    warnings.push("Contact between the folds and with the platens is frictionless (penalty forces).".into());
    let fr = crate::engine::frames::Frames {
        what: "time",
        field: "vm",
        at: snaps.iter().map(|s| s.0).collect(),
        disp: snaps.iter().map(|s| s.1.iter().map(|d| [d[0] as f32, d[1] as f32, 0.0]).collect()).collect(),
        v: snaps.iter().map(|s| s.2.iter().map(|x| *x as f32).collect()).collect(),
    };
    let path = PartPath { load: times.iter().map(|t| t / cr.duration).collect(), umax: crush, peeq: peq_rec, first_yield: None, reached: true, iterations: 0, force };
    let reactions = m.supports.iter().map(|s| Reaction { support: s.id, fx: 0.0, fy: 0.0 }).collect();
    Ok(PlaneResults {
        mesh,
        draw,
        disp: u,
        fields,
        elem,
        reactions,
        applied: [0.0; 3],
        reacted: [0.0; 3],
        scale: [1.0, 1.0],
        imbalance: 0.0,
        stats: crate::sparse::Stats { equations: nn * 2 - held.iter().map(|h| h.iter().filter(|x| **x).count()).sum::<usize>(), nonzeros: 0, residual: 0.0, ms: t0.elapsed().as_secs_f64() * 1e3 },
        warnings,
        path: Some(path),
        frames: Some(fr),
        large: true,
    })
}

fn vm_of(s: &[f64; 4]) -> f64 {
    (0.5 * ((s[0] - s[1]).powi(2) + (s[1] - s[3]).powi(2) + (s[3] - s[0]).powi(2)) + 3.0 * s[2] * s[2]).sqrt()
}

/// A Gauss-point value averaged onto the nodes (each element's mean to its corners).
fn nodal(qd: &Quads, gps: &[[Gp; 4]], nn: usize, f: impl Fn(&Gp) -> f64) -> Vec<f64> {
    let (mut s, mut w) = (vec![0.0; nn], vec![0.0; nn]);
    for (k, q) in qd.q.iter().enumerate() {
        let v = gps[k].iter().map(&f).sum::<f64>() / 4.0;
        for &a in q {
            s[a] += v;
            w[a] += 1.0;
        }
    }
    (0..nn).map(|i| if w[i] > 0.0 { s[i] / w[i] } else { 0.0 }).collect()
}

/// A tube crushed between two platens (0.10.7): its wall's half-section, mean radius `r`, wall
/// `t`, height `h` (m), the foot held and the top moved down `crush` over `duration` seconds, in
/// aluminium (perfectly plastic at `fy`), meshed about `size`; the time step `dt` (mass scaling).
pub fn tube(r: f64, t: f64, h: f64, crush: f64, fy: f64, size: f64, duration: f64, dt: f64) -> Model {
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Geom, Load, Shape, Step, StepKind, Support};
    let mut d = Doc::new(Model::new(Analysis::Axi));
    let mt = &mut d.m.materials[0];
    mt.name = "Aluminium, perfectly plastic".into();
    mt.e = 70e9;
    mt.nu = 0.3;
    mt.rho = 2700.0;
    mt.fy = fy;
    mt.hardening = 0.0;
    let id = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(r - t / 2.0, 0.0, r + t / 2.0, h), hole: false, ..Shape::default() }) }).expect("shape")[0];
    d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: id, edge: 0 }, kind: SupportKind::Fixed }) }).expect("foot");
    d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: id, edge: 2 }, kind: SupportKind::RollerY }) }).expect("top");
    d.apply(Op::Add { item: Item::Load(Load { id: 0, case: 0, at: Target::Edge { shape: id, edge: 2 }, kind: LoadKind::Settlement { dx: 0.0, dy: -crush, dz: 0.0 } }) }).expect("platen");
    d.apply(Op::Add { item: Item::Step(Step { kind: StepKind::Explicit, duration, dt, name: "Crush".into(), ..Default::default() }) }).expect("step");
    d.m.mesh.size = size;
    d.m
}
