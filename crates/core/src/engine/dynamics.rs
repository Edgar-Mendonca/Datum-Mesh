//! Dynamics of structures (0.4.2), for any module that gives its stiffness, mass and results:
//!
//! - **Response spectrum**: the lowest modes, each one's participation in the ground's shaking
//!   Γ = φᵀ M r / φᵀ M φ and effective mass Γ² φᵀ M φ, its peak response Γ φ Sa(T)/ω², and the
//!   modes' results combined by CQC (Der Kiureghian 1981, the correlation of two modes with the
//!   same damping) or by SRSS (the square root of the sum of squares).
//! - **Time history**: M ü + C u̇ + K u = f(t) stepped by Newmark's average acceleration
//!   (γ = ½, β = ¼: unconditionally stable, no numerical damping; Bathe, *Finite Element
//!   Procedures*, §9.2.4), with Rayleigh damping C = αM + βK set to the damping ratio at the first
//!   two modes; the load a time function times the step's load, or the ground's acceleration
//!   (f = −M r a_g, movements relative to the ground).

use crate::modal::Mode;
use crate::sparse::System;

/// A structure as the dynamics see it.
pub trait Structure {
    type Out;
    fn k(&self) -> &System;
    fn held(&self) -> &[bool];
    /// Mass entries, both halves.
    fn mass(&self) -> &[(usize, usize, f64)];
    /// The influence vector of a rigid movement of the ground along `dir` (1 on the movements
    /// along it, projected).
    fn ground(&self, dir: [f64; 3]) -> Vec<f64>;
    /// The unknowns of a model node's movement (x, y, z; usize::MAX where there is none).
    fn node(&self, i: usize) -> [usize; 3];
    /// Results for movements `u` under the nodal loads `f` (inertia and damping forces in them),
    /// with the members' own loads times `p`.
    fn results(&self, u: &[f64], f: &[f64], p: f64) -> Self::Out;
}

/// M x.
pub fn mul(mass: &[(usize, usize, f64)], x: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; x.len()];
    for &(i, j, v) in mass {
        out[i] += v * x[j];
    }
    out
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A table of (x, y) points read straight between them, held flat beyond its ends.
pub fn interp(t: &[[f64; 2]], x: f64) -> f64 {
    match t {
        [] => 0.0,
        [p] => p[1],
        _ => {
            if x <= t[0][0] {
                return t[0][1];
            }
            for w in t.windows(2) {
                if x <= w[1][0] {
                    let s = (x - w[0][0]) / (w[1][0] - w[0][0]);
                    return w[0][1] + (w[1][1] - w[0][1]) * s;
                }
            }
            t[t.len() - 1][1]
        }
    }
}

/// The CQC correlation of two modes with circular frequencies wi, wj and damping ratio z.
pub fn cqc(wi: f64, wj: f64, z: f64) -> f64 {
    let r = wj / wi;
    let num = 8.0 * z * z * (1.0 + r) * r.powf(1.5);
    let den = (1.0 - r * r).powi(2) + 4.0 * z * z * r * (1.0 + r).powi(2);
    if den > 0.0 { num / den } else { 1.0 }
}

/// Combine one quantity's modal values: √(Σ ρij vi vj).
pub fn combine(v: &[f64], rho: &[Vec<f64>]) -> f64 {
    let mut s = 0.0;
    for i in 0..v.len() {
        for j in 0..v.len() {
            s += rho[i][j] * v[i] * v[j];
        }
    }
    s.max(0.0).sqrt()
}

/// One mode of a spectrum analysis.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ModeRow {
    pub f: f64,
    pub period: f64,
    /// Spectral acceleration read at its period (m/s²), participation factor, effective mass (kg)
    /// and its share of the total mass along the shaking.
    pub sa: f64,
    pub gamma: f64,
    pub mass: f64,
    pub share: f64,
}

/// What a dynamic solve adds to the results (0.4.2).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct DynInfo {
    /// "spectrum" or "history".
    pub kind: &'static str,
    /// Spectrum: the modes, the total mass along the shaking, and the share of it the modes
    /// reach; how they were combined.
    pub modes: Vec<ModeRow>,
    pub total_mass: f64,
    pub share: f64,
    pub combined: &'static str,
    /// History: the node followed, times and its movements (x, y, z), the time of the largest
    /// movement (the results shown are at that time), and every node's largest movement.
    pub node: u32,
    pub t: Vec<f64>,
    pub ux: Vec<f64>,
    pub uy: Vec<f64>,
    pub uz: Vec<f64>,
    pub peak_t: f64,
    pub peak: Vec<f64>,
    pub steps: usize,
    pub notes: Vec<String>,
    /// Warnings for the results (too few modes, too coarse a step).
    #[serde(skip)]
    pub warnings: Vec<String>,
}

/// Mass-normalised participation of a mode in the shaking `r`: (Γ, generalised mass φᵀMφ).
fn participation(mass: &[(usize, usize, f64)], shape: &[f64], r: &[f64]) -> (f64, f64) {
    let mphi = mul(mass, shape);
    let gm = dot(shape, &mphi);
    (dot(&mphi, r) / gm, gm)
}

/// A response spectrum analysis: the modes' results, their correlation, and the table.
pub fn spectrum<S: Structure>(s: &S, modes: &[Mode], dir: [f64; 3], table: &[[f64; 2]], z: f64, use_cqc: bool) -> (Vec<S::Out>, Vec<Vec<f64>>, DynInfo) {
    let r = s.ground(dir);
    let total = dot(&mul(s.mass(), &r), &r);
    let mut outs = vec![];
    let mut rows = vec![];
    let mut ws = vec![];
    for md in modes {
        let w = md.lambda.max(0.0).sqrt();
        let (gamma, gm) = participation(s.mass(), &md.shape, &r);
        let period = if w > 0.0 { std::f64::consts::TAU / w } else { f64::INFINITY };
        let sa = interp(table, period);
        let amp = if w > 0.0 { gamma * sa / (w * w) } else { 0.0 };
        let u: Vec<f64> = md.shape.iter().map(|x| x * amp).collect();
        // The equivalent static forces of the mode: K u (= ω² M u), the loads the results balance.
        let f = s.k().mul(&u);
        let f: Vec<f64> = f.iter().zip(s.held()).map(|(v, h)| if *h { 0.0 } else { *v }).collect();
        outs.push(s.results(&u, &f, 0.0));
        let em = gamma * gamma * gm;
        rows.push(ModeRow { f: md.f, period, sa, gamma, mass: em, share: if total > 0.0 { em / total } else { 0.0 } });
        ws.push(w);
    }
    let n = ws.len();
    let rho: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else if use_cqc && ws[i] > 0.0 && ws[j] > 0.0 { cqc(ws[i], ws[j], z) } else { 0.0 }).collect()).collect();
    let share = rows.iter().map(|x| x.share).sum();
    let info = DynInfo { kind: "spectrum", modes: rows, total_mass: total, share, combined: if use_cqc { "CQC" } else { "SRSS" }, ..Default::default() };
    (outs, rho, info)
}

/// Rayleigh damping coefficients (α on M, β on K) for ratio z at circular frequencies w1, w2.
pub fn rayleigh(z: f64, w1: f64, w2: f64) -> (f64, f64) {
    if w2 > w1 * (1.0 + 1e-9) {
        (2.0 * z * w1 * w2 / (w1 + w2), 2.0 * z / (w1 + w2))
    } else {
        (z * w1, z / w1)
    }
}

/// A time history by Newmark's average acceleration from rest: `load(t)` gives the nodal loads at
/// time t and the factor on the members' own loads. Calls `each(step, t, u, v, a, f)` after
/// every step (and at t = 0).
pub fn history<S: Structure>(s: &S, damping: (f64, f64), dt: f64, steps: usize, load: &dyn Fn(f64) -> Vec<f64>, each: &mut dyn FnMut(usize, f64, &[f64], &[f64], &[f64], &[f64])) -> Result<(), String> {
    let (alpha, beta) = damping;
    let n = s.k().n;
    let held = s.held();
    let (c0, c1) = (4.0 / (dt * dt), 2.0 / dt);
    // K̂ = (1 + c1 β) K + (c0 + c1 α) M.
    let mut kh = System::new(n);
    kh.k = s.k().k.iter().map(|&(i, j, v)| (i, j, v * (1.0 + c1 * beta))).collect();
    let plus: Vec<(usize, usize, f64)> = s.mass().iter().map(|&(i, j, v)| (i, j, v * (c0 + c1 * alpha))).collect();
    let fac = kh.factor(held, &plus)?;
    let mut u = vec![0.0; n];
    let mut v = vec![0.0; n];
    // At rest at t = 0: M a = f(0) (the stiffness and damping give nothing yet).
    let f0 = load(0.0);
    let mut a = if f0.iter().any(|x| *x != 0.0) {
        // Only the movements that carry mass accelerate; a weightless one (the rotation under a
        // point mass) follows statically, and has no load on it at t = 0 in a consistent start.
        let mut md = vec![0.0f64; n];
        for &(i, j, v) in s.mass() {
            if i == j {
                md[i] += v;
            }
        }
        let big = md.iter().fold(0.0f64, |x, y| x.max(*y));
        let still: Vec<bool> = (0..n).map(|i| held[i] || md[i] <= 1e-12 * big).collect();
        if (0..n).any(|i| still[i] && !held[i] && f0[i].abs() > 1e-12 * f0.iter().fold(0.0f64, |x, y| x.max(y.abs()))) {
            return Err("A load acts at time zero where there is no mass (a point mass's rotation, or a weightless member's node): start the time function at zero, or give the members a density.".into());
        }
        let mut mm = System::new(n);
        mm.k = s.mass().to_vec();
        mm.factor(&still, &[])?.solve(&f0)
    } else {
        vec![0.0; n]
    };
    each(0, 0.0, &u, &v, &a, &f0);
    let cmul = |x: &[f64]| -> Vec<f64> {
        let (mx, kx) = (mul(s.mass(), x), s.k().mul(x));
        (0..n).map(|i| alpha * mx[i] + beta * kx[i]).collect()
    };
    for step in 1..=steps {
        let t = step as f64 * dt;
        let f = load(t);
        let mterm: Vec<f64> = (0..n).map(|i| c0 * u[i] + 4.0 / dt * v[i] + a[i]).collect();
        let cterm: Vec<f64> = (0..n).map(|i| c1 * u[i] + v[i]).collect();
        let (mm, cc) = (mul(s.mass(), &mterm), cmul(&cterm));
        let rhs: Vec<f64> = (0..n).map(|i| f[i] + mm[i] + cc[i]).collect();
        let un = fac.solve(&rhs);
        let an: Vec<f64> = (0..n).map(|i| c0 * (un[i] - u[i]) - 4.0 / dt * v[i] - a[i]).collect();
        let vn: Vec<f64> = (0..n).map(|i| v[i] + dt / 2.0 * (a[i] + an[i])).collect();
        u = un;
        v = vn;
        a = an;
        each(step, t, &u, &v, &a, &f);
        if !u.iter().all(|x| x.is_finite()) {
            return Err("The time history did not stay finite.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One mass on one spring, for the formulas.
    struct Sdof {
        k: System,
        m: Vec<(usize, usize, f64)>,
        held: Vec<bool>,
    }
    impl Structure for Sdof {
        type Out = f64;
        fn k(&self) -> &System {
            &self.k
        }
        fn held(&self) -> &[bool] {
            &self.held
        }
        fn mass(&self) -> &[(usize, usize, f64)] {
            &self.m
        }
        fn ground(&self, _: [f64; 3]) -> Vec<f64> {
            vec![1.0]
        }
        fn node(&self, _: usize) -> [usize; 3] {
            [0, usize::MAX, usize::MAX]
        }
        fn results(&self, u: &[f64], _: &[f64], _: f64) -> f64 {
            u[0]
        }
    }
    fn sdof(k: f64, m: f64) -> Sdof {
        let mut s = System::new(1);
        s.add(0, 0, k);
        Sdof { k: s, m: vec![(0, 0, m)], held: vec![false] }
    }

    #[test]
    fn a_step_load_doubles_the_static_movement() {
        let (k, m, f) = (4e4, 10.0, 100.0);
        let s = sdof(k, m);
        let w = (k / m).sqrt();
        let dt = std::f64::consts::TAU / w / 200.0;
        let mut peak: f64 = 0.0;
        let mut err: f64 = 0.0;
        history(&s, (0.0, 0.0), dt, 400, &|_| vec![f], &mut |_, t, u, _, _, _| {
            peak = peak.max(u[0]);
            err = err.max((u[0] - f / k * (1.0 - (w * t).cos())).abs());
        })
        .unwrap();
        assert!((peak - 2.0 * f / k).abs() < 1e-3 * 2.0 * f / k, "{peak}");
        // Newmark lengthens the period a little: (πΔt/T)²/3 ≈ 8e-5 here.
        assert!(err < 2e-3 * f / k, "{err}");
    }

    #[test]
    fn damped_free_vibration_decays_at_its_ratio() {
        // Released from a ramped load: after it is gone the amplitude falls by e^(−2πζ) a cycle.
        let (k, m, z) = (4e4, 10.0, 0.05);
        let s = sdof(k, m);
        let w = (k / m).sqrt();
        let (alpha, beta) = rayleigh(z, w, w);
        let tp = std::f64::consts::TAU / w;
        let dt = tp / 400.0;
        let mut peaks: Vec<f64> = vec![];
        let mut last = [0.0f64; 2];
        history(&s, (alpha, beta), dt, 4000, &|t| vec![if t < tp { 100.0 * t / tp } else { 0.0 }], &mut |_, t, u, _, _, _| {
            if t > 2.0 * tp && last[1] > last[0] && last[1] > u[0] {
                peaks.push(last[1]);
            }
            last = [last[1], u[0]];
        })
        .unwrap();
        assert!(peaks.len() > 4);
        let ratio = peaks[3] / peaks[2];
        let want = (-std::f64::consts::TAU * z / (1.0 - z * z).sqrt()).exp();
        assert!((ratio - want).abs() < 5e-3, "{ratio} {want}");
    }

    #[test]
    fn cqc_is_one_on_the_diagonal_and_small_apart() {
        assert!((cqc(10.0, 10.0, 0.05) - 1.0).abs() < 1e-12);
        // Frequencies a factor of two apart, 5 %: 8ζ²(1+r)r^1.5 / ((1−r²)² + 4ζ²r(1+r)²) = 0.0185.
        assert!((cqc(10.0, 20.0, 0.05) - 0.01849).abs() < 1e-4, "{}", cqc(10.0, 20.0, 0.05));
        assert!((cqc(10.0, 11.0, 0.05) - cqc(11.0, 10.0, 0.05)).abs() < 1e-12);
    }
}
