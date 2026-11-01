//! Rubber (0.5.2): the compressible neo-Hookean solid, for large strains,
//!   W = μ/2 (J^(−2/3) I₁ − 3) + κ/2 (J − 1)²,
//! with the first Piola–Kirchhoff stress
//!   P = μ J^(−2/3) (F − I₁/3 F⁻ᵀ) + κ (J − 1) J F⁻ᵀ
//! (Bonet and Wood, *Nonlinear Continuum Mechanics for Finite Element Analysis*, §6.5; Simo and
//! Hughes §9). At small strains it is Hooke's law with μ = E/2(1 + ν) and κ = E/3(1 − 2ν), so
//! steel and rubber alike can be followed through large movements.

pub type M3 = [[f64; 3]; 3];

#[derive(Clone, Copy, Debug)]
pub struct NeoHooke {
    pub mu: f64,
    pub kappa: f64,
}

pub fn det(f: &M3) -> f64 {
    f[0][0] * (f[1][1] * f[2][2] - f[1][2] * f[2][1]) - f[0][1] * (f[1][0] * f[2][2] - f[1][2] * f[2][0]) + f[0][2] * (f[1][0] * f[2][1] - f[1][1] * f[2][0])
}

/// The inverse transpose F⁻ᵀ.
fn inv_t(f: &M3, j: f64) -> M3 {
    // F⁻ᵀ is the cofactor matrix over det F.
    [
        [(f[1][1] * f[2][2] - f[1][2] * f[2][1]) / j, (f[1][2] * f[2][0] - f[1][0] * f[2][2]) / j, (f[1][0] * f[2][1] - f[1][1] * f[2][0]) / j],
        [(f[0][2] * f[2][1] - f[0][1] * f[2][2]) / j, (f[0][0] * f[2][2] - f[0][2] * f[2][0]) / j, (f[0][1] * f[2][0] - f[0][0] * f[2][1]) / j],
        [(f[0][1] * f[1][2] - f[0][2] * f[1][1]) / j, (f[0][2] * f[1][0] - f[0][0] * f[1][2]) / j, (f[0][0] * f[1][1] - f[0][1] * f[1][0]) / j],
    ]
}

impl NeoHooke {
    pub fn new(e: f64, nu: f64) -> NeoHooke {
        NeoHooke { mu: e / (2.0 * (1.0 + nu)), kappa: e / (3.0 * (1.0 - 2.0 * nu)) }
    }

    /// The first Piola–Kirchhoff stress P[a][J] of the deformation gradient F[a][J]; None where
    /// the element has turned inside out (det F ≤ 0).
    pub fn piola(&self, f: &M3) -> Option<M3> {
        let j = det(f);
        if !(j > 0.0) {
            return None;
        }
        let fit = inv_t(f, j);
        let i1: f64 = f.iter().flatten().map(|v| v * v).sum();
        let c = self.mu * j.powf(-2.0 / 3.0);
        let v = self.kappa * (j - 1.0) * j;
        Some(std::array::from_fn(|a| std::array::from_fn(|k| c * (f[a][k] - i1 / 3.0 * fit[a][k]) + v * fit[a][k])))
    }

    /// The tangent A = ∂P/∂F (A[3a + J][3b + L]), by central differences of P.
    pub fn tangent(&self, f: &M3) -> Option<[[f64; 9]; 9]> {
        let h = 1e-6;
        let mut out = [[0.0; 9]; 9];
        for b in 0..3 {
            for l in 0..3 {
                let (mut fp, mut fm) = (*f, *f);
                fp[b][l] += h;
                fm[b][l] -= h;
                let (pp, pm) = (self.piola(&fp)?, self.piola(&fm)?);
                for a in 0..3 {
                    for k in 0..3 {
                        out[3 * a + k][3 * b + l] = (pp[a][k] - pm[a][k]) / (2.0 * h);
                    }
                }
            }
        }
        Some(out)
    }

    /// The Cauchy (true) stress σ = P Fᵀ / J as (xx, yy, zz, xy, yz, zx).
    pub fn cauchy(&self, f: &M3) -> Option<[f64; 6]> {
        let p = self.piola(f)?;
        let j = det(f);
        let s = |a: usize, b: usize| (0..3).map(|k| p[a][k] * f[b][k]).sum::<f64>() / j;
        Some([s(0, 0), s(1, 1), s(2, 2), s(0, 1), s(1, 2), s(2, 0)])
    }

    /// Uniaxial tension (exact, 0.5.2's benchmark): the nominal stress (force over the original
    /// area) at stretch λ along the bar, the sides free; their stretch found by Newton's method so
    /// the stress across is zero. Returns (nominal stress, side stretch).
    pub fn uniaxial(&self, lam: f64) -> (f64, f64) {
        let mut s = 1.0 / lam.sqrt();
        let across = |s: f64| self.piola(&[[lam, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s]]).map_or(f64::NAN, |p| p[1][1]);
        for _ in 0..100 {
            let r = across(s);
            let d = (across(s + 1e-9) - across(s - 1e-9)) / 2e-9;
            if !(d.abs() > 0.0) {
                break;
            }
            let ds = r / d;
            s -= ds;
            if ds.abs() < 1e-15 {
                break;
            }
        }
        let p = self.piola(&[[lam, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s]]).map_or(f64::NAN, |p| p[0][0]);
        (p, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_strains_give_hookes_law() {
        let m = NeoHooke::new(200e9, 0.3);
        let e = 1e-7;
        let f = [[1.0 + e, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let s = m.cauchy(&f).unwrap();
        let (lam, mu) = (200e9 * 0.3 / (1.3 * 0.4), 200e9 / 2.6);
        assert!((s[0] - (lam + 2.0 * mu) * e).abs() < 1e-5 * (lam + 2.0 * mu) * e, "{}", s[0]);
        assert!((s[1] - lam * e).abs() < 1e-5 * lam * e);
    }

    #[test]
    fn a_rigid_rotation_carries_no_stress() {
        let m = NeoHooke::new(5e6, 0.49);
        let (c, s) = (0.6f64, 0.8f64);
        let f = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
        assert!(m.cauchy(&f).unwrap().iter().all(|v| v.abs() < 1e-6));
    }

    #[test]
    fn nearly_incompressible_rubber_follows_rivlin() {
        // Incompressible neo-Hookean uniaxial: P = μ (λ − λ⁻²) (2 C10 = μ).
        let m = NeoHooke { mu: 1e6, kappa: 1e10 };
        for lam in [0.5, 1.5, 2.0, 3.0] {
            let (p, s) = m.uniaxial(lam);
            let want = m.mu * (lam - 1.0 / (lam * lam));
            assert!((p - want).abs() < 1e-3 * want.abs(), "{lam}: {p} {want}");
            assert!((s - 1.0 / lam.sqrt()).abs() < 1e-3);
        }
    }

    #[test]
    fn the_tangent_matches_the_stress() {
        let m = NeoHooke::new(10e6, 0.45);
        let f = [[1.3, 0.2, 0.0], [-0.1, 0.9, 0.05], [0.0, 0.1, 1.1]];
        let a = m.tangent(&f).unwrap();
        // Symmetric (major symmetry of a hyperelastic tangent).
        for r in 0..9 {
            for c in 0..9 {
                assert!((a[r][c] - a[c][r]).abs() < 1e-5 * m.mu, "{r} {c}");
            }
        }
    }
}
