//! Member end releases (hinges, 0.3.3) by static condensation. A hinged end's rotation is not the
//! node's: it is whatever makes the member's moment there zero. With u_k the member's other end
//! movements and u_r the released rotations, zero end moment gives u_r = T_rk u_k + K_rr⁻¹ f_r
//! (f the member's own fixed-end loads), so u = T u_k + d. The member then enters the frame as
//! Tᵀ K T, Tᵀ f, Tᵀ M T and Tᵀ K_G T: one transform for the static, modal and buckling solves.

/// Solve A X = B for small dense matrices (Gaussian elimination with partial pivoting).
fn solve_small(mut a: Vec<Vec<f64>>, mut b: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let n = a.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap_or(c);
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            for k in c..n {
                a[r][k] -= f * a[c][k];
            }
            for k in 0..b[r].len() {
                b[r][k] -= f * b[c][k];
            }
        }
    }
    for c in (0..n).rev() {
        for k in 0..b[c].len() {
            let s: f64 = (c + 1..n).map(|j| a[c][j] * b[j][k]).sum();
            b[c][k] = (b[c][k] - s) / a[c][c];
        }
    }
    b
}

/// The condensation of one member: its transform and the released rotations its own loads give.
pub(crate) struct Release<const N: usize> {
    t: [[f64; N]; N],
    d: [f64; N],
}

impl<const N: usize> Release<N> {
    /// From the member's full stiffness `k`, its fixed-end loads `f` and the released dofs.
    pub fn new(k: &[[f64; N]; N], f: &[f64; N], rel: &[usize]) -> Self {
        let kept: Vec<usize> = (0..N).filter(|i| !rel.contains(i)).collect();
        let krr: Vec<Vec<f64>> = rel.iter().map(|&r| rel.iter().map(|&c| k[r][c]).collect()).collect();
        // Right-hand sides: −K_rk (one column per kept dof) and f_r.
        let rhs: Vec<Vec<f64>> = rel.iter().map(|&r| kept.iter().map(|&c| -k[r][c]).chain(std::iter::once(f[r])).collect()).collect();
        let x = solve_small(krr, rhs);
        let mut t = [[0.0; N]; N];
        let mut d = [0.0; N];
        for &c in &kept {
            t[c][c] = 1.0;
        }
        for (i, &r) in rel.iter().enumerate() {
            for (j, &c) in kept.iter().enumerate() {
                t[r][c] = x[i][j];
            }
            d[r] = x[i][kept.len()];
        }
        Release { t, d }
    }
    /// Tᵀ A T.
    pub fn mat(&self, a: &[[f64; N]; N]) -> [[f64; N]; N] {
        let mut at = [[0.0; N]; N];
        for i in 0..N {
            for j in 0..N {
                at[i][j] = (0..N).map(|q| a[i][q] * self.t[q][j]).sum();
            }
        }
        let mut out = [[0.0; N]; N];
        for i in 0..N {
            for j in 0..N {
                out[i][j] = (0..N).map(|q| self.t[q][i] * at[q][j]).sum();
            }
        }
        out
    }
    /// Tᵀ f.
    pub fn load(&self, f: &[f64; N]) -> [f64; N] {
        std::array::from_fn(|i| (0..N).map(|q| self.t[q][i] * f[q]).sum())
    }
    /// The member's full end movements from the nodes' (the released ones are not the nodes'),
    /// with or without the part its own loads give.
    pub fn full(&self, u: &[f64; N], loaded: bool) -> [f64; N] {
        std::array::from_fn(|i| (0..N).map(|q| self.t[i][q] * u[q]).sum::<f64>() + if loaded { self.d[i] } else { 0.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hinged_end_carries_no_moment() {
        // A 2D beam (EI = 1, L = 1) hinged at b: its stiffness against end a's rotation drops from
        // 4EI/L to 3EI/L, and the fixed-end moment of a uniform load wL²/12 becomes wL²/8.
        let mut k = [[0.0; 6]; 6];
        let s = [[12.0, 6.0, -12.0, 6.0], [6.0, 4.0, -6.0, 2.0], [-12.0, -6.0, 12.0, -6.0], [6.0, 2.0, -6.0, 4.0]];
        let idx = [1, 2, 4, 5];
        for i in 0..4 {
            for j in 0..4 {
                k[idx[i]][idx[j]] = s[i][j];
            }
        }
        let f = [0.0, 0.5, 1.0 / 12.0, 0.0, 0.5, -1.0 / 12.0];
        let r = Release::new(&k, &f, &[5]);
        let kc = r.mat(&k);
        assert!((kc[2][2] - 3.0).abs() < 1e-12 && kc[5][5] == 0.0);
        assert!((r.load(&f)[2] - 1.0 / 8.0).abs() < 1e-12);
        // The end rotation that makes the moment at b zero, with a held still.
        let u = r.full(&[0.0; 6], true);
        let mb: f64 = (0..6).map(|j| k[5][j] * u[j]).sum::<f64>() - f[5];
        assert!(mb.abs() < 1e-12);
    }
}
