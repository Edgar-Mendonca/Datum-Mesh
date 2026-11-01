//! The linear solver every analysis shares: stiffness entries as triplets, supports by elimination
//! (held degrees of freedom are removed, springs and prescribed values handled by the caller), and
//! a sparse Cholesky factorisation from faer (MIT) with a fill-reducing ordering.

use faer::sparse::{SparseColMat, Triplet};
use faer::linalg::solvers::Solve;
use faer::{Mat, Side};

/// A symmetric system K u = f with n unknowns, built from (row, col, value) entries; repeated
/// entries add up.
#[derive(Default)]
pub struct System {
    pub n: usize,
    pub k: Vec<(usize, usize, f64)>,
    pub f: Vec<f64>,
    /// Only the lower triangle (row ≥ column) is kept; the upper half is implied (0.3.0: 3D
    /// solids, whose elements add many entries, store half as much).
    pub half: bool,
    /// The matrix is not symmetric (0.5.8: the tangent of large 3D rotations, taken with respect
    /// to rotation vectors): `solve_many` factorises it by LU, and judges stability from its
    /// symmetric part.
    pub general: bool,
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
        System { n, k: Vec::new(), f: vec![0.0; n], half: false, general: false }
    }
    /// A system that keeps only the lower triangle: entries above the diagonal are dropped as
    /// they are added (add both halves, or only the lower one).
    pub fn lower(n: usize) -> System {
        System { half: true, ..System::new(n) }
    }
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        if v != 0.0 && !(self.half && i < j) {
            self.k.push((i, j, v));
        }
    }

    /// K u for the whole system (reactions are K u − f at held unknowns).
    pub fn mul(&self, u: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; self.n];
        for &(i, j, v) in &self.k {
            out[i] += v * u[j];
            if self.half && i != j {
                out[j] += v * u[i];
            }
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
                (a, b) => {
                    if a != usize::MAX && b == usize::MAX && fixed[j] != 0.0 {
                        rhs[a] -= v * fixed[j];
                    }
                    // The implied upper entry (j, i) of a half-stored system.
                    if self.half && b != usize::MAX && a == usize::MAX && fixed[i] != 0.0 {
                        rhs[b] -= v * fixed[i];
                    }
                }
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

impl System {
    /// Solve K x = b for several right-hand sides at once, with the held unknowns at zero, for a
    /// tangent stiffness that may not be positive (past a limit point, 0.4.0): a Cholesky
    /// factorisation when it is positive, else a sparse LU. Returns the answers and whether the
    /// matrix was positive definite (the structure is stable there). `f` is not read.
    pub fn solve_many(&self, held: &[bool], rhs: &[&[f64]]) -> Result<(Vec<Vec<f64>>, bool), String> {
        let mut map = vec![usize::MAX; self.n];
        let mut m = 0;
        for i in 0..self.n {
            if !held[i] {
                map[i] = m;
                m += 1;
            }
        }
        let mut out = vec![vec![0.0; self.n]; rhs.len()];
        if m == 0 {
            return Ok((out, true));
        }
        let mut tri = Vec::with_capacity(self.k.len() * if self.half { 2 } else { 1 });
        let mut diag = vec![0.0f64; m];
        for &(i, j, v) in &self.k {
            let (a, b) = (map[i], map[j]);
            if a == usize::MAX || b == usize::MAX {
                continue;
            }
            tri.push(Triplet::new(a, b, v));
            if self.half && a != b {
                tri.push(Triplet::new(b, a, v));
            }
            if a == b {
                diag[a] += v;
            }
        }
        if !diag.iter().all(|d| d.is_finite()) {
            return Err(mechanism());
        }
        let a = SparseColMat::<usize, f64>::try_new_from_triplets(m, m, &tri).map_err(|e| format!("Could not build the stiffness matrix: {e:?}"))?;
        let mut b = Mat::<f64>::zeros(m, rhs.len() + 1);
        for (c, r) in rhs.iter().enumerate() {
            for i in 0..self.n {
                if map[i] != usize::MAX {
                    b[(map[i], c)] = r[i];
                }
            }
        }
        // The probe column, as in `solve`: enormous answers mean a zero pivot (a mechanism).
        let scale = diag.iter().fold(0.0f64, |x, d| x.max(d.abs())).max(f64::MIN_POSITIVE);
        for i in 0..m {
            b[(i, rhs.len())] = if diag[i].abs() > 0.0 { diag[i] } else { scale };
        }
        // A general matrix: the symmetric part (A + Aᵀ)/2 for the stability check, LU to solve.
        let lower: Vec<Triplet<usize, usize, f64>> = if self.general {
            tri.iter().flat_map(|t| [Triplet::new(t.row, t.col, 0.5 * t.val), Triplet::new(t.col, t.row, 0.5 * t.val)]).filter(|t| t.row >= t.col).collect()
        } else {
            tri.iter().filter(|t| t.row >= t.col).copied().collect()
        };
        let pd = diag.iter().all(|d| *d > 0.0);
        let llt = if pd { SparseColMat::<usize, f64>::try_new_from_triplets(m, m, &lower).ok().and_then(|l| l.sp_cholesky(Side::Lower).ok()) } else { None };
        let (x, stable) = match (llt, self.general) {
            (Some(l), false) => (l.solve(&b), true),
            (l, true) => (a.sp_lu().map_err(|_| mechanism())?.solve(&b), l.is_some()),
            (None, false) => (a.sp_lu().map_err(|_| mechanism())?.solve(&b), false),
        };
        if (0..m).any(|i| !(x[(i, rhs.len())].abs() < 1e15)) {
            return Err(mechanism());
        }
        for (c, o) in out.iter_mut().enumerate() {
            for i in 0..self.n {
                if map[i] != usize::MAX {
                    o[i] = x[(map[i], c)];
                }
            }
            if !o.iter().all(|v| v.is_finite()) {
                return Err(mechanism());
            }
        }
        Ok((out, stable))
    }
}

/// A system factorised once and solved for many right-hand sides (0.4.2: time stepping), the
/// held unknowns at zero. Needs a positive definite matrix.
pub struct Factor {
    n: usize,
    map: Vec<usize>,
    llt: faer::sparse::linalg::solvers::Llt<usize, f64>,
}

impl Factor {
    /// Solve K x = b; b is read on the free unknowns only, x is zero at held ones.
    pub fn solve(&self, b: &[f64]) -> Vec<f64> {
        let m = self.map.iter().filter(|x| **x != usize::MAX).count();
        let mut r = Mat::<f64>::zeros(m, 1);
        for i in 0..self.n {
            if self.map[i] != usize::MAX {
                r[(self.map[i], 0)] = b[i];
            }
        }
        let x = self.llt.solve(&r);
        (0..self.n).map(|i| if self.map[i] != usize::MAX { x[(self.map[i], 0)] } else { 0.0 }).collect()
    }
}

impl System {
    /// Factorise K (with the extra entries `plus`, e.g. a multiple of the mass) once, the
    /// unknowns in `held` removed.
    pub fn factor(&self, held: &[bool], plus: &[(usize, usize, f64)]) -> Result<Factor, String> {
        let mut map = vec![usize::MAX; self.n];
        let mut m = 0;
        for i in 0..self.n {
            if !held[i] {
                map[i] = m;
                m += 1;
            }
        }
        let mut tri = vec![];
        for &(i, j, v) in self.k.iter().chain(plus.iter()) {
            let (a, b) = (map[i], map[j]);
            if a == usize::MAX || b == usize::MAX {
                continue;
            }
            // The lower triangle (a half-stored K has only that; `plus` gives both halves).
            if a >= b {
                tri.push(Triplet::new(a, b, v));
            }
        }
        let a = SparseColMat::<usize, f64>::try_new_from_triplets(m, m, &tri).map_err(|e| format!("Could not build the matrix: {e:?}"))?;
        let llt = a.sp_cholesky(Side::Lower).map_err(|_| mechanism())?;
        Ok(Factor { n: self.n, map, llt })
    }
}

fn mechanism() -> String {
    "The model can move without resistance: add supports so it cannot slide or turn as a whole, and check that every part is connected (a truss needs triangles).".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_half_stored_system_solves_as_the_full_one() {
        // A small chain of springs with one end held at a prescribed movement.
        let build = |s: &mut System| {
            for i in 0..5 {
                s.add(i, i, 2.0);
                s.add(i + 1, i + 1, 2.0);
                s.add(i, i + 1, -1.5);
                s.add(i + 1, i, -1.5);
            }
            s.f[3] = 1.0;
        };
        let (mut a, mut b) = (System::new(6), System::lower(6));
        build(&mut a);
        build(&mut b);
        assert!(b.k.len() < a.k.len());
        let held = [true, false, false, false, false, true];
        let fixed = [0.25, 0.0, 0.0, 0.0, 0.0, -0.5];
        let (ua, _) = a.solve(&held, &fixed).unwrap();
        let (ub, _) = b.solve(&held, &fixed).unwrap();
        assert!(ua.iter().zip(&ub).all(|(x, y)| (x - y).abs() < 1e-12), "{ua:?} {ub:?}");
        assert!(a.mul(&ua).iter().zip(b.mul(&ub)).all(|(x, y)| (x - y).abs() < 1e-12));
    }

    #[test]
    fn an_indefinite_system_solves_and_says_so() {
        // Springs 1 and −0.5 in a row (a negative stiffness, as past a limit point): K = [[0.5, 0.5], [0.5, −0.5]] after the held end.
        let mut s = System::new(3);
        for (a, k) in [(0, 1.0), (1, -0.5)] {
            s.add(a, a, k);
            s.add(a + 1, a + 1, k);
            s.add(a, a + 1, -k);
            s.add(a + 1, a, -k);
        }
        let (x, stable) = s.solve_many(&[true, false, false], &[&[0.0, 0.0, 1.0]]).unwrap();
        assert!(!stable);
        // Series springs: 1/(1/1 + 1/−0.5) = −1, so the end moves −1; the middle node moves the force over spring 1.
        assert!((x[0][2] + 1.0).abs() < 1e-12 && (x[0][1] - 1.0).abs() < 1e-12, "{:?}", x[0]);
        // A positive one says stable and agrees with `solve`.
        let mut p = System::new(3);
        for (a, k) in [(0, 1.0), (1, 2.0)] {
            p.add(a, a, k);
            p.add(a + 1, a + 1, k);
            p.add(a, a + 1, -k);
            p.add(a + 1, a, -k);
        }
        let (x, stable) = p.solve_many(&[true, false, false], &[&[0.0, 0.0, 1.0], &[0.0, 1.0, 0.0]]).unwrap();
        assert!(stable && (x[0][2] - 1.5).abs() < 1e-12 && (x[1][2] - 1.0).abs() < 1e-12);
        // A loose chain is a mechanism.
        assert!(p.solve_many(&[false; 3], &[&[0.0, 0.0, 1.0]]).is_err());
    }

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
