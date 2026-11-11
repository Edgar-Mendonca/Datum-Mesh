//! Large deformation of 2D parts (0.10.10): an axisymmetric or plane-strain part of rubber (the
//! compressible neo-Hookean solid of `hyper.rs`) followed through large movements in increments,
//! by the shared path solver (Newton–Raphson or arc length). Pressure turns with the surface it
//! acts on (a follower load).
//!
//! The method, from the published literature:
//! - Total Lagrangian four-node quadrilaterals (the crush's: a grid over a rectangle, else each
//!   triangle of the mesh cut into three), 2 × 2 Gauss points; in an axisymmetric part the hoop
//!   stretch is r/R (Bathe, *Finite Element Procedures*, §6.3).
//! - F-bar with the element's mean volume change (de Souza Neto, Perić, Dunne and Owen 1996;
//!   Nagtegaal, Parks and Rice 1974): each point's deformation gradient takes the element's volume
//!   ratio J̄ = v/V, F̄ = (J̄/J)^⅓ F, so nearly incompressible rubber does not lock, and a uniform
//!   stretch is reproduced exactly, round an axis too. The nodal forces are the exact derivative of the element's energy with F̄,
//!   so they stay consistent and the tangent is symmetric.
//! - Each element's tangent stiffness by central differences of its own forces (Miehe 1996,
//!   "Numerical computation of algorithmic (consistent) tangent moduli"), so Newton converges
//!   quadratically without hand-derived second derivatives.
//! - Follower pressure: on the deformed edge, normal to it and over its deformed length (and its
//!   deformed radius, turned round the axis); its load stiffness by central differences too,
//!   unsymmetric, so the system is then solved as a general one.

use crate::engine::path::{follow, Control, Problem};
use crate::model::{Analysis, LoadKind, Model, SupportKind, Target};
use crate::solids::crush::{grid, quads, shape, Quads, G};
use crate::solids::hyper::{NeoHooke, M3};
use crate::solids::mesh;
use crate::solids::plane::{field, PartPath, PlaneResults, Reaction};
use crate::sparse::System;

/// One Gauss point: shape-function derivatives against R and Z, the functions, R, and its volume.
#[derive(Clone)]
struct Gp {
    dn: [[f64; 2]; 4],
    n: [f64; 4],
    r: f64,
    dv: f64,
}

struct Elem {
    q: [usize; 4],
    gp: [Gp; 4],
    mat: NeoHooke,
}

/// A follower pressure on one piece of an edge (nodes a → b, the part on its left).
struct Press {
    a: usize,
    b: usize,
    p: f64,
}

pub struct Large2d {
    axi: bool,
    x0: Vec<[f64; 2]>,
    el: Vec<Elem>,
    press: Vec<Press>,
    held: Vec<bool>,
    /// The follower pressure's forces at rest (subtracted, as they are the step's load vector).
    p0: Vec<f64>,
}

/// F as (F11, F12, F21, F22, F33) at a point from the element's movements.
fn grad(g: &Gp, u: &[[f64; 2]; 4], axi: bool) -> [f64; 5] {
    let mut f = [1.0, 0.0, 0.0, 1.0, 1.0];
    for a in 0..4 {
        f[0] += u[a][0] * g.dn[a][0];
        f[1] += u[a][0] * g.dn[a][1];
        f[2] += u[a][1] * g.dn[a][0];
        f[3] += u[a][1] * g.dn[a][1];
        if axi {
            f[4] += g.n[a] * u[a][0] / g.r;
        }
    }
    f
}
fn jac(f: &[f64; 5]) -> f64 {
    (f[0] * f[3] - f[1] * f[2]) * f[4]
}
fn m3(f: &[f64; 5], s: f64) -> M3 {
    [[s * f[0], s * f[1], 0.0], [s * f[2], s * f[3], 0.0], [0.0, 0.0, s * f[4]]]
}
/// F⁻ᵀ : ∂F/∂u for node a's dof c (0: r, 1: z).
fn fit_dot(f: &[f64; 5], g: &Gp, a: usize, c: usize, axi: bool) -> f64 {
    let d = f[0] * f[3] - f[1] * f[2];
    // F⁻ᵀ of the r–z block: [[F22, −F21], [−F12, F11]] / d.
    let it = [[f[3] / d, -f[2] / d], [-f[1] / d, f[0] / d]];
    let (dr, dz) = (g.dn[a][0], g.dn[a][1]);
    if c == 0 {
        it[0][0] * dr + it[0][1] * dz + if axi { g.n[a] / g.r / f[4] } else { 0.0 }
    } else {
        it[1][0] * dr + it[1][1] * dz
    }
}
/// P : ∂F/∂u for node a's dof c.
fn p_dot(p: &M3, g: &Gp, a: usize, c: usize, axi: bool) -> f64 {
    let (dr, dz) = (g.dn[a][0], g.dn[a][1]);
    if c == 0 {
        p[0][0] * dr + p[0][1] * dz + if axi { p[2][2] * g.n[a] / g.r } else { 0.0 }
    } else {
        p[1][0] * dr + p[1][1] * dz
    }
}

impl Large2d {
    /// An element's forces (8: r, z of each node) at its movements; None if it turned inside out.
    /// The element's mean volume change J̄ = v/V and its derivative d(ln J̄)/du (8), from the points.
    fn mean_j(&self, e: &Elem, u: &[[f64; 2]; 4]) -> Option<(f64, [f64; 8], [[f64; 5]; 4])> {
        let fs: [[f64; 5]; 4] = std::array::from_fn(|k| grad(&e.gp[k], u, self.axi));
        let vol: f64 = e.gp.iter().map(|g| g.dv).sum();
        let mut v = 0.0;
        let mut dv = [0.0; 8];
        for (k, g) in e.gp.iter().enumerate() {
            let j = jac(&fs[k]);
            if !(j > 0.0) {
                return None;
            }
            v += j * g.dv;
            for a in 0..4 {
                for c in 0..2 {
                    dv[2 * a + c] += j * fit_dot(&fs[k], g, a, c, self.axi) * g.dv;
                }
            }
        }
        Some((v / vol, dv.map(|x| x / v), fs))
    }

    fn elem_force(&self, e: &Elem, u: &[[f64; 2]; 4]) -> Option<[f64; 8]> {
        let (j0, dl0, fs) = self.mean_j(e, u)?;
        let mut out = [0.0; 8];
        for (k, g) in e.gp.iter().enumerate() {
            let f = fs[k];
            let j = jac(&f);
            let s = (j0 / j).cbrt();
            let pb = e.mat.piola(&m3(&f, s))?;
            // P̄ : F (the F̄ term's factor).
            let pf = s * (pb[0][0] * f[0] + pb[0][1] * f[1] + pb[1][0] * f[2] + pb[1][1] * f[3] + pb[2][2] * f[4]);
            for a in 0..4 {
                for c in 0..2 {
                    let v = s * p_dot(&pb, g, a, c, self.axi) + pf / 3.0 * (dl0[2 * a + c] - fit_dot(&f, g, a, c, self.axi));
                    out[2 * a + c] += v * g.dv;
                }
            }
        }
        Some(out)
    }

    /// The forces of one piece of follower pressure on its two nodes (r, z, r, z), at movements u.
    fn press_force(&self, pr: &Press, ua: [f64; 2], ub: [f64; 2]) -> [f64; 4] {
        let (xa, xb) = ([self.x0[pr.a][0] + ua[0], self.x0[pr.a][1] + ua[1]], [self.x0[pr.b][0] + ub[0], self.x0[pr.b][1] + ub[1]]);
        let (tx, ty) = (xb[0] - xa[0], xb[1] - xa[1]);
        // The outward normal times the length: (ty, −tx); pressure pushes inwards. Each node takes
        // ∫ N 2πr along the piece (consistent loads) when turned round the axis, else half.
        let (wa, wb) = if self.axi { (std::f64::consts::TAU * (2.0 * xa[0] + xb[0]) / 6.0, std::f64::consts::TAU * (xa[0] + 2.0 * xb[0]) / 6.0) } else { (0.5, 0.5) };
        [-pr.p * ty * wa, pr.p * tx * wa, -pr.p * ty * wb, pr.p * tx * wb]
    }

    fn nodes_of(&self, e: &Elem, u: &[f64]) -> [[f64; 2]; 4] {
        std::array::from_fn(|a| [u[2 * e.q[a]], u[2 * e.q[a] + 1]])
    }

    /// The external forces of the follower pressure at movements u.
    fn press_all(&self, u: &[f64]) -> Vec<f64> {
        let mut f = vec![0.0; u.len()];
        for pr in &self.press {
            let v = self.press_force(pr, [u[2 * pr.a], u[2 * pr.a + 1]], [u[2 * pr.b], u[2 * pr.b + 1]]);
            for (k, &nd) in [pr.a, pr.a, pr.b, pr.b].iter().enumerate() {
                f[2 * nd + k % 2] += v[k];
            }
        }
        f
    }

    /// The Cauchy stress (σrr, σzz, σrz, σθθ) at each Gauss point of each element.
    fn stresses(&self, u: &[f64]) -> Vec<[[f64; 4]; 4]> {
        self.el.iter().map(|e| {
            let ue = self.nodes_of(e, u);
            let j0 = self.mean_j(e, &ue).map_or(1.0, |x| x.0);
            std::array::from_fn(|k| {
                let f = grad(&e.gp[k], &ue, self.axi);
                let s = (j0 / jac(&f)).cbrt();
                let fb = m3(&f, s);
                let p = e.mat.piola(&fb).unwrap_or([[0.0; 3]; 3]);
                // σ = P F̄ᵀ / J̄.
                let sig = |i: usize, l: usize| (0..3).map(|k| p[i][k] * fb[l][k]).sum::<f64>() / j0;
                [sig(0, 0), sig(1, 1), sig(0, 1), sig(2, 2)]
            })
        }).collect()
    }
}

impl Problem for Large2d {
    fn size(&self) -> usize {
        self.held.len()
    }
    fn held(&self) -> &[bool] {
        &self.held
    }
    fn state(&self, u: &[f64]) -> Result<(Vec<f64>, System), String> {
        self.state_at(u, 0.0)
    }
    fn state_at(&self, u: &[f64], lambda: f64) -> Result<(Vec<f64>, System), String> {
        let n = self.size();
        let mut fint = vec![0.0; n];
        let mut k = System::new(n);
        let size = self.x0.iter().fold(0.0f64, |m, p| m.max(p[0].abs()).max(p[1].abs())).max(1e-9);
        let h = 1e-7 * size;
        for e in &self.el {
            let ue = self.nodes_of(e, u);
            let f = self.elem_force(e, &ue).ok_or("An element has turned inside out: take smaller increments, or a finer mesh.")?;
            let dofs: [usize; 8] = std::array::from_fn(|i| 2 * e.q[i / 2] + i % 2);
            for i in 0..8 {
                fint[dofs[i]] += f[i];
            }
            // The tangent by central differences of the element's own forces, made symmetric.
            let mut ke = [[0.0; 8]; 8];
            for j in 0..8 {
                let (mut up, mut um) = (ue, ue);
                up[j / 2][j % 2] += h;
                um[j / 2][j % 2] -= h;
                let (fp, fm) = (self.elem_force(e, &up).ok_or("An element has turned inside out.")?, self.elem_force(e, &um).ok_or("An element has turned inside out.")?);
                for i in 0..8 {
                    ke[i][j] = (fp[i] - fm[i]) / (2.0 * h);
                }
            }
            for i in 0..8 {
                for j in 0..8 {
                    k.add(dofs[i], dofs[j], 0.5 * (ke[i][j] + ke[j][i]));
                }
            }
        }
        if !self.press.is_empty() && lambda != 0.0 {
            // Follower pressure, taken in with the internal forces: −λ (f_p(u) − f_p(u₀)), as the
            // step's load vector already carries λ f_p(u₀). Its stiffness is unsymmetric.
            k.general = true;
            for pr in &self.press {
                let ua = [u[2 * pr.a], u[2 * pr.a + 1]];
                let ub = [u[2 * pr.b], u[2 * pr.b + 1]];
                let f = self.press_force(pr, ua, ub);
                let dofs = [2 * pr.a, 2 * pr.a + 1, 2 * pr.b, 2 * pr.b + 1];
                for i in 0..4 {
                    fint[dofs[i]] -= lambda * f[i];
                }
                for j in 0..4 {
                    let (mut ap, mut am, mut bp, mut bm) = (ua, ua, ub, ub);
                    if j < 2 {
                        ap[j] += h;
                        am[j] -= h;
                    } else {
                        bp[j - 2] += h;
                        bm[j - 2] -= h;
                    }
                    let (fp, fm) = (self.press_force(pr, ap, bp), self.press_force(pr, am, bm));
                    for i in 0..4 {
                        k.add(dofs[i], dofs[j], -lambda * (fp[i] - fm[i]) / (2.0 * h));
                    }
                }
            }
            for i in 0..n {
                fint[i] += lambda * self.p0[i];
            }
        }
        Ok((fint, k))
    }
}

/// Follow an axisymmetric or plane-strain rubber part through large movements, in `increments`.
pub fn solve(m: &Model, increments: usize, arc: bool, settings: Option<&crate::model::SolverSettings>) -> Result<PlaneResults, String> {
    let axi = match m.analysis {
        Analysis::Axi => true,
        Analysis::Strain => false,
        _ => return Err("Large deformation of parts is solved for axisymmetric and plane-strain parts; a plate in plane stress thins as it stretches, which comes later.".into()),
    };
    let t0 = std::time::Instant::now();
    let mut lin = m.clone();
    lin.mesh.order = 1;
    let ms = mesh::mesh(&lin)?;
    let qd: Quads = grid(m, ms.size).unwrap_or_else(|| quads(&ms));
    let nn = qd.nodes.len();
    let x0 = qd.nodes.clone();
    let mut el = vec![];
    for (k, q) in qd.q.iter().enumerate() {
        let mt = m.materials.iter().find(|x| x.id as usize == qd.mat[k]).ok_or("A part's material is missing.")?;
        if !(mt.e > 0.0 && mt.nu > -1.0 && mt.nu < 0.5) {
            return Err(format!("{} needs a stiffness and a Poisson's ratio below 0.5 for large deformation.", mt.name));
        }
        let x: [[f64; 2]; 4] = std::array::from_fn(|a| x0[q[a]]);
        let mk = |xi: f64, eta: f64, w: f64| -> Result<Gp, String> {
            let (dn, n, det) = shape(&x, xi, eta).ok_or("A quadrilateral of the mesh is folded: use a finer or more even mesh.")?;
            let r = (0..4).map(|a| n[a] * x[a][0]).sum::<f64>();
            if axi && r <= 0.0 {
                return Err("An axisymmetric part must lie to the right of the axis (x > 0).".into());
            }
            Ok(Gp { dn, n, r, dv: w * det * if axi { std::f64::consts::TAU * r } else { 1.0 } })
        };
        let gp = [mk(-G, -G, 1.0)?, mk(G, -G, 1.0)?, mk(G, G, 1.0)?, mk(-G, G, 1.0)?];
        el.push(Elem { q: *q, gp, mat: NeoHooke::new(mt.e, mt.nu) });
    }
    // Supports.
    let mut held = vec![false; 2 * nn];
    for s in &m.supports {
        let Target::Edge { shape, edge } = s.at else { return Err("Large deformation of parts takes supports on the part's edges.".into()) };
        let ns = qd.edges.get(&(shape, edge)).cloned().unwrap_or_default();
        let h = match s.kind {
            SupportKind::Fixed | SupportKind::Pinned => [true, true],
            SupportKind::RollerX => [false, true],
            SupportKind::RollerY => [true, false],
            SupportKind::Symmetry | SupportKind::Roller => {
                let (a, b) = (x0[ns[0]], x0[*ns.last().unwrap()]);
                if (b[0] - a[0]).abs() < (b[1] - a[1]).abs() { [true, false] } else { [false, true] }
            }
            _ => return Err("Large deformation of parts takes fixed, pinned, roller and symmetry supports.".into()),
        };
        for n in ns {
            held[2 * n] |= h[0];
            held[2 * n + 1] |= h[1];
        }
    }
    // Loads: edge forces as they are (dead), pressure following the surface.
    let mut load = vec![0.0; 2 * nn];
    let mut press = vec![];
    let edge_len = |shape: u32, edge: u32| qd.bedges.iter().filter(|e| e.shape == shape && e.edge == edge).map(|e| { let (a, b) = (x0[e.n[0] as usize], x0[e.n[2] as usize]); (b[0] - a[0]).hypot(b[1] - a[1]) }).sum::<f64>();
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Pressure { p }, Target::Edge { shape, edge }) => {
                for e in qd.bedges.iter().filter(|e| e.shape == *shape && e.edge == *edge) {
                    press.push(Press { a: e.n[0] as usize, b: e.n[2] as usize, p: *p });
                }
            }
            (LoadKind::Traction { fx, fy, total, .. }, Target::Edge { shape, edge }) => {
                let len = edge_len(*shape, *edge);
                let tr = if *total { if len > 0.0 { [fx / len, fy / len] } else { [0.0, 0.0] } } else { [*fx, *fy] };
                for e in qd.bedges.iter().filter(|e| e.shape == *shape && e.edge == *edge) {
                    let (a, b) = (e.n[0] as usize, e.n[2] as usize);
                    let l = (x0[b][0] - x0[a][0]).hypot(x0[b][1] - x0[a][1]);
                    // Consistent loads: each node ∫ N 2πr along the piece when turned round the axis.
                    let (wa, wb) = if axi { (std::f64::consts::TAU * (2.0 * x0[a][0] + x0[b][0]) / 6.0, std::f64::consts::TAU * (x0[a][0] + 2.0 * x0[b][0]) / 6.0) } else { (0.5, 0.5) };
                    for (nd, w) in [(a, wa), (b, wb)] {
                        load[2 * nd] += tr[0] * l * w;
                        load[2 * nd + 1] += tr[1] * l * w;
                    }
                }
            }
            _ => return Err("Large deformation of parts takes edge forces and pressure (which turns with the surface); other loads come later.".into()),
        }
    }
    let mut pb = Large2d { axi, x0: x0.clone(), el, press, held: held.clone(), p0: vec![] };
    let zero = vec![0.0; 2 * nn];
    pb.p0 = pb.press_all(&zero);
    for i in 0..2 * nn {
        load[i] += pb.p0[i];
    }
    let ctl = Control::for_step(increments.max(1), arc, settings);
    let path = follow(&pb, &zero, &load, &zero, &ctl)?;
    let last = path.points.last().ok_or("No point was reached.")?;
    let u = last.u.clone();
    let mut warnings = vec![];
    if let Some(n) = &path.note {
        warnings.push(n.clone());
    }
    if !pb.press.is_empty() {
        warnings.push("Pressure follows the surface as it moves (normal to the deformed edge, over its deformed length).".into());
    }
    // Results at the last point reached.
    let disp: Vec<[f64; 2]> = (0..nn).map(|i| [u[2 * i], u[2 * i + 1]]).collect();
    let sig = pb.stresses(&u);
    let nodal = |f: &dyn Fn(&[f64; 4]) -> f64| -> Vec<f64> {
        let (mut s, mut w) = (vec![0.0; nn], vec![0.0; nn]);
        for (k, e) in pb.el.iter().enumerate() {
            let v = sig[k].iter().map(f).sum::<f64>() / 4.0;
            for &a in &e.q {
                s[a] += v;
                w[a] += 1.0;
            }
        }
        (0..nn).map(|i| if w[i] > 0.0 { s[i] / w[i] } else { 0.0 }).collect()
    };
    let vm = |s: &[f64; 4]| (0.5 * ((s[0] - s[1]).powi(2) + (s[1] - s[3]).powi(2) + (s[3] - s[0]).powi(2)) + 3.0 * s[2] * s[2]).sqrt();
    let (sx, sy, txy, sz, v) = (nodal(&|s| s[0]), nodal(&|s| s[1]), nodal(&|s| s[2]), nodal(&|s| s[3]), nodal(&vm));
    let p1: Vec<f64> = (0..nn).map(|i| 0.5 * (sx[i] + sy[i]) + (0.25 * (sx[i] - sy[i]).powi(2) + txy[i] * txy[i]).sqrt()).collect();
    let p2: Vec<f64> = (0..nn).map(|i| 0.5 * (sx[i] + sy[i]) - (0.25 * (sx[i] - sy[i]).powi(2) + txy[i] * txy[i]).sqrt()).collect();
    let nodes = &x0;
    let fields = vec![
        field("ux", disp.iter().map(|d| d[0]).collect(), nodes),
        field("uy", disp.iter().map(|d| d[1]).collect(), nodes),
        field("umag", disp.iter().map(|d| d[0].hypot(d[1])).collect(), nodes),
        field("sx", sx, nodes),
        field("sy", sy, nodes),
        field("txy", txy, nodes),
        field("sz", sz, nodes),
        field("vm", v.clone(), nodes),
        field("s1", p1, nodes),
        field("s2", p2, nodes),
        field("sf", vec![0.0; nn], nodes),
        field("peeq", vec![0.0; nn], nodes),
    ];
    let tris: Vec<Vec<u32>> = qd.q.iter().flat_map(|q| [vec![q[0] as u32, q[1] as u32, q[2] as u32], vec![q[0] as u32, q[2] as u32, q[3] as u32]]).collect();
    let centroids: Vec<[f64; 2]> = tris.iter().map(|t| { let p: Vec<[f64; 2]> = t.iter().map(|&i| nodes[i as usize]).collect(); [(p[0][0] + p[1][0] + p[2][0]) / 3.0, (p[0][1] + p[1][1] + p[2][1]) / 3.0] }).collect();
    let el2 = |f: &dyn Fn(&[f64; 4]) -> f64| -> Vec<f64> { sig.iter().flat_map(|e| { let x = e.iter().map(f).sum::<f64>() / 4.0; [x, x] }).collect() };
    let elem = vec![field("sx", el2(&|s| s[0]), &centroids), field("sy", el2(&|s| s[1]), &centroids), field("txy", el2(&|s| s[2]), &centroids), field("sz", el2(&|s| s[3]), &centroids), field("vm", el2(&vm), &centroids)];
    // The equilibrium check at the end: the reactions against the loads there (pressure where the
    // deformed edge now is).
    let (fint, _) = pb.state_at(&u, 0.0)?;
    let pnow = pb.press_all(&u);
    let ext: Vec<f64> = (0..2 * nn).map(|i| last.lambda * (load[i] - pb.p0[i] + pnow[i])).collect();
    let mut applied = [0.0; 3];
    let mut reacted = [0.0; 3];
    let mut scale = [0.0f64; 2];
    for i in 0..nn {
        let (fe, fr) = ([ext[2 * i], ext[2 * i + 1]], [fint[2 * i] - ext[2 * i], fint[2 * i + 1] - ext[2 * i + 1]]);
        for c in 0..2 {
            applied[c] += fe[c];
            scale[0] += fe[c].abs();
            if held[2 * i + c] {
                reacted[c] += fr[c];
            }
        }
    }
    let free_res = (0..2 * nn).filter(|i| !held[*i]).map(|i| (fint[i] - ext[i]).abs()).fold(0.0, f64::max);
    let imbalance = if scale[0] > 0.0 { ((applied[0] + reacted[0]).abs().max((applied[1] + reacted[1]).abs()) / scale[0]).max(free_res / scale[0]) } else { 0.0 };
    scale[1] = 1.0;
    let umax: Vec<f64> = path.points.iter().map(|p| (0..nn).map(|i| p.u[2 * i].hypot(p.u[2 * i + 1])).fold(0.0, f64::max)).collect();
    let pick = crate::engine::frames::pick(path.points.len());
    let frames = crate::engine::frames::Frames {
        what: "load",
        field: "vm",
        at: pick.iter().map(|&k| path.points[k].lambda).collect(),
        disp: pick.iter().map(|&k| (0..nn).map(|i| [path.points[k].u[2 * i] as f32, path.points[k].u[2 * i + 1] as f32, 0.0]).collect()).collect(),
        v: pick.iter().map(|&k| { let s = pb.stresses(&path.points[k].u); let (mut a, mut w) = (vec![0.0f32; nn], vec![0.0f32; nn]); for (e, el) in pb.el.iter().enumerate() { let x = (s[e].iter().map(vm).sum::<f64>() / 4.0) as f32; for &q in &el.q { a[q] += x; w[q] += 1.0; } } a.iter().zip(&w).map(|(x, y)| if *y > 0.0 { x / y } else { 0.0 }).collect() }).collect(),
    };
    let ppath = PartPath { load: path.points.iter().map(|p| p.lambda).collect(), umax, peeq: vec![0.0; path.points.len()], first_yield: None, reached: path.reached, iterations: path.iterations, force: vec![] };
    let mat2: Vec<u32> = qd.mat.iter().flat_map(|&m| [m as u32, m as u32]).collect();
    let equations = held.iter().filter(|h| !**h).count();
    Ok(PlaneResults {
        mesh: crate::solids::mesh::Mesh { nodes: x0.clone(), tris: tris.clone(), order: 1, mat: mat2, edges: qd.bedges.clone(), points: vec![], size: ms.size, quality: ms.quality.clone() },
        draw: tris.iter().map(|t| [t[0], t[1], t[2]]).collect(),
        disp,
        fields,
        elem,
        reactions: m.supports.iter().map(|s| Reaction { support: s.id, fx: 0.0, fy: 0.0 }).collect(),
        applied,
        reacted,
        scale,
        imbalance,
        stats: crate::sparse::Stats { equations, nonzeros: 0, residual: 0.0, ms: t0.elapsed().as_secs_f64() * 1e3 },
        warnings,
        path: Some(ppath),
        frames: Some(frames),
        large: true,
    })
}
