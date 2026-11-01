//! Large rotations in 3D frames (0.5.8): the corotational beam in 3D (Crisfield, *Non-linear
//! Finite Element Analysis of Solids and Structures*, vol. 2, ch. 17; Battini and Pacoste 2002).
//! Each piece of a beam carries a frame that turns with it: its x axis along the chord between
//! its ends, its y axis set by the mean of the two nodes' turned y axes. Measured in that frame,
//! the piece only stretches and turns its ends a little, so the linear 3D beam stiffness serves
//! there; all the large rotation is in the frame.
//!
//! Each node keeps its total rotation as a rotation vector. The internal forces at a node's
//! rotations are moments, found from small turns (spins) applied on top of the current rotation,
//! and the tangent is their derivative with respect to such spins; the path solver's increments
//! are spins too, composed onto the rotations (`Problem::add`), never added to the vectors. At
//! balance the tangent is then symmetric for conservative loads, so its positive definiteness
//! says whether the frame is stable, as for plane frames. Both derivatives are central
//! differences of the piece's local movements, which are exact formulas.

use super::space::{assemble, cond, k_local, results_at, scatter, Assembly, Elem, SpaceResults};
use crate::engine::path::Problem;
use crate::engine::sparse::{Stats, System};
use crate::model::Model;

type M3 = [[f64; 3]; 3];

/// The rotation matrix of a rotation vector (Rodrigues).
pub fn exp(t: [f64; 3]) -> M3 {
    let a = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
    let (s, c) = if a < 1e-8 { (1.0 - a * a / 6.0, 0.5 - a * a / 24.0) } else { (a.sin() / a, (1.0 - a.cos()) / (a * a)) };
    let k = [[0.0, -t[2], t[1]], [t[2], 0.0, -t[0]], [-t[1], t[0], 0.0]];
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk: f64 = (0..3).map(|m| k[i][m] * k[m][j]).sum();
            r[i][j] = if i == j { 1.0 } else { 0.0 } + s * k[i][j] + c * kk;
        }
    }
    r
}

/// The rotation vector of a rotation matrix (through the quaternion, well behaved up to a half turn
/// and beyond).
pub fn log(r: &M3) -> [f64; 3] {
    let tr = r[0][0] + r[1][1] + r[2][2];
    let (w, x, y, z) = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        (0.25 * s, (r[2][1] - r[1][2]) / s, (r[0][2] - r[2][0]) / s, (r[1][0] - r[0][1]) / s)
    } else if r[0][0] > r[1][1] && r[0][0] > r[2][2] {
        let s = (1.0 + r[0][0] - r[1][1] - r[2][2]).sqrt() * 2.0;
        ((r[2][1] - r[1][2]) / s, 0.25 * s, (r[0][1] + r[1][0]) / s, (r[0][2] + r[2][0]) / s)
    } else if r[1][1] > r[2][2] {
        let s = (1.0 + r[1][1] - r[0][0] - r[2][2]).sqrt() * 2.0;
        ((r[0][2] - r[2][0]) / s, (r[0][1] + r[1][0]) / s, 0.25 * s, (r[1][2] + r[2][1]) / s)
    } else {
        let s = (1.0 + r[2][2] - r[0][0] - r[1][1]).sqrt() * 2.0;
        ((r[1][0] - r[0][1]) / s, (r[0][2] + r[2][0]) / s, (r[1][2] + r[2][1]) / s, 0.25 * s)
    };
    let (w, x, y, z) = if w < 0.0 { (-w, -x, -y, -z) } else { (w, x, y, z) };
    let v = (x * x + y * y + z * z).sqrt();
    if v < 1e-14 {
        return [2.0 * x, 2.0 * y, 2.0 * z];
    }
    let a = 2.0 * v.atan2(w);
    [x / v * a, y / v * a, z / v * a]
}

/// A rotation vector after a small turn `w` on top of it: log(exp(w)·exp(t)), taken on the branch
/// nearest `t` so a node's rotation runs on smoothly past a half turn.
fn turn(t: [f64; 3], w: [f64; 3]) -> [f64; 3] {
    let v = log(&mul(&exp(w), &exp(t)));
    let a = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if a < 1e-12 {
        return v;
    }
    let n = v.map(|x| x / a);
    (-3..=3)
        .map(|k| { let s = a + 2.0 * std::f64::consts::PI * k as f64; [n[0] * s, n[1] * s, n[2] * s] })
        .min_by(|p, q| { let d = |x: &[f64; 3]| (0..3).map(|i| (x[i] - t[i]).powi(2)).sum::<f64>(); d(p).total_cmp(&d(q)) })
        .unwrap_or(v)
}

fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    a.map(|v| v / l)
}

/// A piece's local movements in its turned frame (u, v, w, θx, θy, θz at each end; only the
/// stretch and the end rotations are not zero), from its ends' places and turns.
fn local(e: &Elem, xa: [f64; 3], xb: [f64; 3], ra: &M3, rb: &M3) -> [f64; 12] {
    // The original local axes as columns, turned with each node.
    let c0: M3 = std::array::from_fn(|i| std::array::from_fn(|j| e.r[j][i]));
    let (ta, tb) = (mul(ra, &c0), mul(rb, &c0));
    let d = [xb[0] - xa[0], xb[1] - xa[1], xb[2] - xa[2]];
    let ln = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let e1 = d.map(|v| v / ln);
    let q = [0, 1, 2].map(|i| (ta[i][1] + tb[i][1]) / 2.0);
    let e3 = unit(cross(e1, q));
    let e2 = cross(e3, e1);
    // Each end's turn relative to the frame: E^T T.
    let rel = |t: &M3| -> M3 { let ee = [e1, e2, e3]; std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| ee[i][k] * t[k][j]).sum())) };
    let (qa, qb) = (log(&rel(&ta)), log(&rel(&tb)));
    [0.0, 0.0, 0.0, qa[0], qa[1], qa[2], ln - e.l, 0.0, 0.0, qb[0], qb[1], qb[2]]
}

/// The 3D frame for the path solver: unknowns as the frame assembly has them (x, y, z, θx, θy,
/// θz at each node), the rotations as total rotation vectors.
pub struct Corot3 {
    pub(crate) asm: Assembly,
    kc: Vec<[[f64; 12]; 12]>,
    /// What the assembly holds besides the pieces (spring supports, rigid floor ties): linear.
    extra: Vec<(usize, usize, f64)>,
}

impl Corot3 {
    pub fn new(m: &Model, pieces: usize) -> Result<(Corot3, Vec<f64>), String> {
        let asm = assemble(m, pieces.max(1))?;
        if asm.plated() {
            return Err("Large rotations are for 3D frames without plates in this release; with plates, a large-deflection step is second order (P-delta).".into());
        }
        let kc: Vec<[[f64; 12]; 12]> = asm.parts.iter().map(|e| { let mut e0 = e.clone(); e0.pn = 0.0; cond(e, k_local(&e0)) }).collect();
        let mut extra = asm.sys.k.clone();
        for (e, k) in asm.parts.iter().zip(&kc) {
            scatter(e, k, |i, j, v| extra.push((i, j, -v)));
        }
        let mut sum: std::collections::HashMap<(usize, usize), f64> = std::collections::HashMap::new();
        for (i, j, v) in extra {
            *sum.entry((i, j)).or_insert(0.0) += v;
        }
        let big = asm.sys.k.iter().map(|e| e.2.abs()).fold(0.0, f64::max);
        let mut extra: Vec<(usize, usize, f64)> = sum.into_iter().filter(|e| e.1.abs() > 1e-12 * big).map(|((i, j), v)| (i, j, v)).collect();
        extra.sort_by_key(|e| (e.0, e.1));
        let f = asm.sys.f.clone();
        Ok((Corot3 { asm, kc, extra }, f))
    }

    /// A piece's ends' places and turns at movements u, moved on by `b` and then `q` (movements,
    /// and spins on top of the rotations).
    fn ends(&self, e: &Elem, u: &[f64], b: &[f64; 12], q: &[f64; 12]) -> ([f64; 3], [f64; 3], M3, M3) {
        let p = &self.asm.pos;
        let rot = |n: usize, o: usize| mul(&exp([q[o + 3], q[o + 4], q[o + 5]]), &mul(&exp([b[o + 3], b[o + 4], b[o + 5]]), &exp([u[6 * n + 3], u[6 * n + 4], u[6 * n + 5]])));
        let xa = [0, 1, 2].map(|k| p[e.a][k] + u[6 * e.a + k] + b[k] + q[k]);
        let xb = [0, 1, 2].map(|k| p[e.b][k] + u[6 * e.b + k] + b[6 + k] + q[6 + k]);
        (xa, xb, rot(e.a, 0), rot(e.b, 6))
    }

    /// A piece's local movements, and its end forces (forces and moments at its two nodes) in
    /// global terms, at movements u.
    fn piece(&self, k: usize, u: &[f64], b: &[f64; 12]) -> ([f64; 12], [f64; 12]) {
        let e = &self.asm.parts[k];
        let at = |q: &[f64; 12]| { let (xa, xb, ra, rb) = self.ends(e, u, b, q); local(e, xa, xb, &ra, &rb) };
        let d = at(&[0.0; 12]);
        let kc = &self.kc[k];
        let fl: [f64; 12] = std::array::from_fn(|i| (0..12).map(|j| kc[i][j] * d[j]).sum());
        // f = Bᵀ fl, B = ∂d/∂(movements and spins), by central differences.
        let mut f = [0.0; 12];
        for c in 0..12 {
            // A step that balances rounding (ε/h) against truncation (h²): 1e-5.
            let h = if c % 6 < 3 { 1e-5 * e.l } else { 1e-5 };
            let mut qp = [0.0; 12];
            qp[c] = h;
            let mut qm = [0.0; 12];
            qm[c] = -h;
            let (dp, dm) = (at(&qp), at(&qm));
            f[c] = (0..12).map(|i| fl[i] * (dp[i] - dm[i]) / (2.0 * h)).sum();
        }
        (d, f)
    }

    /// Every piece's local end movements at u (for the results).
    pub fn locals(&self, u: &[f64]) -> Vec<[f64; 12]> {
        (0..self.asm.parts.len()).map(|k| self.piece(k, u, &[0.0; 12]).0).collect()
    }

    pub fn results(&self, m: &Model, u: &[f64], f: &[f64], stats: Stats, warnings: Vec<String>) -> Result<SpaceResults, String> {
        let (fint, _) = self.state(u)?;
        let ls = self.locals(u);
        let mut r = results_at(m, &self.asm, u, f, 1.0, true, Some((&fint, &ls)));
        r.stats = stats;
        r.warnings.extend(warnings);
        r.warnings.dedup();
        Ok(r)
    }
}

impl Problem for Corot3 {
    fn size(&self) -> usize {
        self.asm.sys.n
    }
    fn held(&self) -> &[bool] {
        &self.asm.held
    }
    fn state(&self, u: &[f64]) -> Result<(Vec<f64>, System), String> {
        let n = self.asm.sys.n;
        let mut fint = vec![0.0; n];
        let mut kt = System::new(n);
        kt.general = true;
        for (k, e) in self.asm.parts.iter().enumerate() {
            let dofs: [usize; 12] = std::array::from_fn(|i| 6 * if i < 6 { e.a } else { e.b } + i % 6);
            let (_, f) = self.piece(k, u, &[0.0; 12]);
            for i in 0..12 {
                fint[dofs[i]] += f[i];
            }
            // The tangent: ∂f/∂(movements and spins).
            let mut ke = [[0.0; 12]; 12];
            for c in 0..12 {
                let h = if c % 6 < 3 { 1e-6 * e.l } else { 1e-6 };
                let at = |s: f64| { let mut b = [0.0; 12]; b[c] = s; self.piece(k, u, &b).1 };
                let (fp, fm) = (at(h), at(-h));
                for i in 0..12 {
                    ke[i][c] = (fp[i] - fm[i]) / (2.0 * h);
                }
            }
            for i in 0..12 {
                for j in 0..12 {
                    if ke[i][j] != 0.0 {
                        kt.add(dofs[i], dofs[j], ke[i][j]);
                    }
                }
            }
        }
        // Springs and supports' own stiffness the assembly carries outside the pieces (spring
        // supports, rigid floor ties) stay linear.
        for &(i, j, v) in &self.extra {
            fint[i] += v * u[j];
            kt.add(i, j, v);
        }
        Ok((fint, kt))
    }
    fn add(&self, u: &[f64], du: &[f64]) -> Vec<f64> {
        let mut out: Vec<f64> = u.iter().zip(du).map(|(a, b)| a + b).collect();
        for nd in 0..u.len() / 6 {
            let r = 6 * nd + 3;
            if du[r..r + 3].iter().any(|x| *x != 0.0) {
                out[r..r + 3].copy_from_slice(&turn([u[r], u[r + 1], u[r + 2]], [du[r], du[r + 1], du[r + 2]]));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_and_log_are_inverse() {
        for t in [[0.0, 0.0, 0.0], [0.3, -0.2, 0.1], [0.0, 0.0, 3.0], [1.0, 2.0, -0.5], [1e-9, 0.0, 2e-9]] {
            let back = log(&exp(t));
            assert!((0..3).all(|i| (back[i] - t[i]).abs() < 1e-10), "{t:?} {back:?}");
        }
    }
}
