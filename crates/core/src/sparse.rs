//! The linear solver every analysis shares: stiffness entries as triplets, supports by elimination
//! (held degrees of freedom are removed, springs and prescribed values handled by the caller), and
//! a sparse Cholesky factorisation from faer (MIT) with a fill-reducing ordering.

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::{Mat, Side};

/// A symmetric system K u = f with n unknowns, built from (row, col, value) entries; repeated
/// entries add up.
#[derive(Default)]
pub struct System {
    pub n: usize,
    pub k: Vec<(usize, usize, f64)>,
    pub f: Vec<f64>,
}

/// How the solve went, for the solver report.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Stats {
    /// Unknowns solved for (after supports are removed).
    pub equations: usize,
    /// Stored entries of the reduced stiffness matrix (lower triangle).
    pub nonzeros: usize,
    /// ‖K u − f‖ / ‖f‖ on the free unknowns.
    pub residual: f64,
    pub ms: f64,
}

impl System {
    pub fn new(n: usize) -> System {
        System { n, k: Vec::new(), f: vec![0.0; n] }
    }
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        if v != 0.0 {
            self.k.push((i, j, v));
        }
    }

    /// K u for the whole system (reactions are K u − f at held unknowns).
    pub fn mul(&self, u: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; self.n];
        for &(i, j, v) in &self.k {
            out[i] += v * u[j];
        }
        out
    }

    /// Solve with the unknowns in `held` fixed at the values in `fixed` (same length as n; only
    /// held entries are read). Returns the full solution. An error means the model can move
    /// without resistance (a mechanism) or the stiffness is not positive.
    pub fn solve(&self, held: &[bool], fixed: &[f64]) -> Result<(Vec<f64>, Stats), String> {
        let t0 = std::time::Instant::now();
        let mut map = vec![usize::MAX; self.n];
        let mut m = 0;
        for i in 0..self.n {
            if !held[i] {
                map[i] = m;
                m += 1;
            }
        }
        let mut u: Vec<f64> = (0..self.n).map(|i| if held[i] { fixed[i] } else { 0.0 }).collect();
        if m == 0 {
            return Ok((u, Stats { ms: t0.elapsed().as_secs_f64() * 1e3, ..Default::default() }));
        }
        // Right-hand side: f minus the effect of prescribed values.
        let mut rhs = vec![0.0; m];
        for i in 0..self.n {
            if map[i] != usize::MAX {
                rhs[map[i]] += self.f[i];
            }
        }
        let mut tri = Vec::with_capacity(self.k.len());
        let mut diag = vec![0.0f64; m];
        for &(i, j, v) in &self.k {
            match (map[i], map[j]) {
                (a, b) if a != usize::MAX && b != usize::MAX => {
                    if a >= b {
                        tri.push(Triplet::new(a, b, v));
                    }
                    if a == b {
                        diag[a] += v;
                    }
                }
                (a, usize::MAX) if a != usize::MAX && fixed[j] != 0.0 => rhs[a] -= v * fixed[j],
                _ => {}
            }
        }
        if !diag.iter().all(|d| *d > 0.0 && d.is_finite()) {
            return Err(mechanism());
        }
        let a = SparseColMat::<usize, f64>::try_new_from_triplets(m, m, &tri).map_err(|e| format!("Could not build the stiffness matrix: {e:?}"))?;
        let nonzeros = a.compute_nnz();
        let llt = a.sp_cholesky(Side::Lower).map_err(|_| mechanism())?;
        // Column 0 is the load; column 1 a probe load scaled by the diagonal, whose answer is about 1
        // in a compact model, larger in long chains of members, and enormous when rounding hid a zero
        // pivot (a mechanism).
        let mut b = Mat::<f64>::zeros(m, 2);
        for i in 0..m {
            b[(i, 0)] = rhs[i];
            b[(i, 1)] = diag[i];
        }
        let x = llt.solve(&b);
        // A member split into n pieces answers about 3n⁴ (2.4e14 at 3 000); a singular matrix far more.
        if (0..m).any(|i| !(x[(i, 1)].abs() < 1e15)) {
            return Err(mechanism());
        }
        for i in 0..self.n {
            if map[i] != usize::MAX {
                u[i] = x[(map[i], 0)];
            }
        }
        if !u.iter().all(|v| v.is_finite()) {
            return Err(mechanism());
        }
        // Iterative refinement with the same factors: long chains of members lose digits to
        // rounding (a cantilever in 1 000 pieces, 6e-5), and two steps win them back.
        for _ in 0..2 {
            let ku = self.mul(&u);
            let mut r = Mat::<f64>::zeros(m, 1);
            for i in 0..self.n {
                if map[i] != usize::MAX {
                    r[(map[i], 0)] = self.f[i] - ku[i];
                }
            }
            let dx = llt.solve(&r);
            for i in 0..self.n {
                if map[i] != usize::MAX {
                    u[i] += dx[(map[i], 0)];
                }
            }
        }
        // Residual on the free unknowns.
        let ku = self.mul(&u);
        let (mut r2, mut f2) = (0.0, 0.0);
        for i in 0..self.n {
            if map[i] != usize::MAX {
                r2 += (ku[i] - self.f[i]).powi(2);
                f2 += self.f[i].powi(2);
            }
        }
        let residual = if f2 > 0.0 { (r2 / f2).sqrt() } else { r2.sqrt() };
        Ok((u, Stats { equations: m, nonzeros, residual, ms: t0.elapsed().as_secs_f64() * 1e3 }))
    }
}

fn mechanism() -> String {
    "The model can move without resistance: add supports so it cannot slide or turn as a whole, and check that every part is connected (a truss needs triangles).".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_of_springs() {
        // Three springs in a row, k = 1, 2, 4; the left end held, 1 N pulling the right end:
        // extensions 1, 1/2, 1/4, so the end moves 1.75.
        let mut s = System::new(4);
        for (a, k) in [(0, 1.0), (1, 2.0), (2, 4.0)] {
            s.add(a, a, k);
            s.add(a + 1, a + 1, k);
            s.add(a, a + 1, -k);
            s.add(a + 1, a, -k);
        }
        s.f[3] = 1.0;
        let held = [true, false, false, false];
        let (u, st) = s.solve(&held, &[0.0; 4]).unwrap();
        assert!((u[3] - 1.75).abs() < 1e-12 && st.equations == 3 && st.residual < 1e-12);
        // A prescribed end movement instead of the force.
        let mut s2 = System { f: vec![0.0; 4], ..s };
        s2.f[3] = 0.0;
        let (u, _) = s2.solve(&[true, false, false, true], &[0.0, 0.0, 0.0, 1.75]).unwrap();
        assert!((u[1] - 1.0).abs() < 1e-12);
        // Nothing held: the chain floats away.
        assert!(s2.solve(&[false; 4], &[0.0; 4]).is_err());
    }
}
