//! J2 (von Mises) plasticity with linear isotropic hardening (0.5.0), one material point:
//! the radial return of Simo and Hughes, *Computational Inelasticity* (1998), Box 3.1, in the
//! components the plane and axisymmetric solids have (xx, yy, zz, xy; tensor strains).
//!
//! Trial stress from the elastic strain, its deviator s and q = √(3/2 s:s); if q exceeds the yield
//! stress σy + H ᾱ, the plastic multiplier Δγ = (q − σy − H ᾱ)/(3G + H) scales s back onto the
//! yield surface and the plastic strain grows by (3/2) Δγ s/q.

/// The material: Young's modulus, Poisson's ratio, yield stress and hardening modulus H (the
/// slope of stress against plastic strain; 0 for perfectly plastic).
#[derive(Clone, Copy, Debug)]
pub struct J2 {
    pub e: f64,
    pub nu: f64,
    pub fy: f64,
    pub h: f64,
}

/// A material point's history: plastic strain (xx, yy, zz, xy, tensor) and the equivalent
/// plastic strain ᾱ.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub ep: [f64; 4],
    pub alpha: f64,
}

impl J2 {
    fn lame(&self) -> (f64, f64) {
        let g = self.e / (2.0 * (1.0 + self.nu));
        (self.e * self.nu / ((1.0 + self.nu) * (1.0 - 2.0 * self.nu)), g)
    }

    /// Stress (xx, yy, zz, xy) and the new history for total strain `eps` (xx, yy, zz, xy:
    /// tensor), from the history `st` at the start of the increment.
    pub fn stress(&self, eps: [f64; 4], st: &State) -> ([f64; 4], State) {
        let (lam, g) = self.lame();
        let ee = [eps[0] - st.ep[0], eps[1] - st.ep[1], eps[2] - st.ep[2], eps[3] - st.ep[3]];
        let tr = ee[0] + ee[1] + ee[2];
        let sig = [lam * tr + 2.0 * g * ee[0], lam * tr + 2.0 * g * ee[1], lam * tr + 2.0 * g * ee[2], 2.0 * g * ee[3]];
        let p = (sig[0] + sig[1] + sig[2]) / 3.0;
        let s = [sig[0] - p, sig[1] - p, sig[2] - p, sig[3]];
        let q = (1.5 * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2] + 2.0 * s[3] * s[3])).sqrt();
        let f = q - (self.fy + self.h * st.alpha);
        if self.fy <= 0.0 || f <= 0.0 {
            return (sig, *st);
        }
        let dg = f / (3.0 * g + self.h);
        let k = 1.0 - 3.0 * g * dg / q;
        let mut out = *st;
        for i in 0..4 {
            out.ep[i] += 1.5 * dg * s[i] / q;
        }
        out.alpha += dg;
        ([s[0] * k + p, s[1] * k + p, s[2] * k + p, s[3] * k], out)
    }

    /// Plane stress: the stress with σzz = 0, the free εzz found by Newton's method (the slope
    /// from a small step), from in-plane strain (xx, yy, xy: tensor). Returns the stress, the
    /// history and εzz.
    pub fn plane_stress(&self, eps: [f64; 3], st: &State) -> ([f64; 4], State, f64) {
        // Start from the elastic guess εzz = −ν/(1 − ν)(εxx + εyy) shifted by the plastic part.
        let mut ez = -self.nu / (1.0 - self.nu) * (eps[0] - st.ep[0] + eps[1] - st.ep[1]) + st.ep[2];
        let at = |ez: f64| self.stress([eps[0], eps[1], ez, eps[2]], st);
        for _ in 0..50 {
            let (s0, _) = at(ez);
            if s0[2].abs() <= 1e-12 * (s0[0].abs() + s0[1].abs() + s0[3].abs()).max(self.fy * 1e-6).max(1e-30) {
                break;
            }
            let h = 1e-7 * (ez.abs() + eps[0].abs() + eps[1].abs()).max(1e-12);
            let (s1, _) = at(ez + h);
            let slope = (s1[2] - s0[2]) / h;
            if !(slope.abs() > 0.0) {
                break;
            }
            ez -= s0[2] / slope;
        }
        let (s, st2) = at(ez);
        (s, st2, ez)
    }

    /// The von Mises stress of (xx, yy, zz, xy).
    pub fn mises(s: [f64; 4]) -> f64 {
        (0.5 * ((s[0] - s[1]).powi(2) + (s[1] - s[2]).powi(2) + (s[2] - s[0]).powi(2)) + 3.0 * s[3] * s[3]).sqrt()
    }
}

/// A 3D material point's history (0.5.1): plastic strain (xx, yy, zz, xy, yz, zx, tensor) and ᾱ.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State6 {
    pub ep: [f64; 6],
    pub alpha: f64,
}

impl J2 {
    /// The radial return in all six components: stress (xx, yy, zz, xy, yz, zx) from total
    /// strain (tensor components), from the history at the start of the increment.
    pub fn stress6(&self, eps: [f64; 6], st: &State6) -> ([f64; 6], State6) {
        let (lam, g) = self.lame();
        let ee: [f64; 6] = std::array::from_fn(|i| eps[i] - st.ep[i]);
        let tr = ee[0] + ee[1] + ee[2];
        let sig: [f64; 6] = std::array::from_fn(|i| if i < 3 { lam * tr + 2.0 * g * ee[i] } else { 2.0 * g * ee[i] });
        let p = (sig[0] + sig[1] + sig[2]) / 3.0;
        let s: [f64; 6] = std::array::from_fn(|i| if i < 3 { sig[i] - p } else { sig[i] });
        let ss = s[0] * s[0] + s[1] * s[1] + s[2] * s[2] + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]);
        let q = (1.5 * ss).sqrt();
        let f = q - (self.fy + self.h * st.alpha);
        if self.fy <= 0.0 || f <= 0.0 {
            return (sig, *st);
        }
        let dg = f / (3.0 * g + self.h);
        let k = 1.0 - 3.0 * g * dg / q;
        let mut out = *st;
        for i in 0..6 {
            out.ep[i] += 1.5 * dg * s[i] / q;
        }
        out.alpha += dg;
        (std::array::from_fn(|i| if i < 3 { s[i] * k + p } else { s[i] * k }), out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniaxial_stress_follows_the_hardening_line() {
        // Plane stress, pulled along x with σyy = 0 (εyy free, found by bisection on it here).
        let m = J2 { e: 200e9, nu: 0.3, fy: 250e6, h: 2e9 };
        let et = m.e * m.h / (m.e + m.h);
        let mut st = State::default();
        for k in 1..=40 {
            let ex = 0.0005 * k as f64;
            // Find εyy giving σyy = 0 (plane stress also gives σzz = 0).
            let (mut lo, mut hi) = (-0.05, 0.05);
            let mut out = (st, [0.0; 4]);
            for _ in 0..200 {
                let mid = (lo + hi) / 2.0;
                let (s, s2, _) = m.plane_stress([ex, mid, 0.0], &st);
                out = (s2, s);
                if s[1] > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let sx = out.1[0];
            let want = if ex * m.e <= m.fy { ex * m.e } else { m.fy + et * (ex - m.fy / m.e) };
            assert!((sx - want).abs() < 1e-6 * want, "{ex}: {sx} {want}");
            assert!(out.1[2].abs() < 1e-3, "σzz {}", out.1[2]);
            st = out.0;
        }
        // The stress stays on the yield surface with its hardening.
        assert!((st.alpha - (0.02 - (m.fy + m.h * st.alpha) / m.e)).abs() < 1e-9, "{}", st.alpha);
    }

    #[test]
    fn the_six_component_return_agrees_with_the_four() {
        let m = J2 { e: 70e9, nu: 0.33, fy: 100e6, h: 1e9 };
        let (s4, a4) = m.stress([0.004, -0.001, 0.0005, 0.002], &State::default());
        let (s6, a6) = m.stress6([0.004, -0.001, 0.0005, 0.002, 0.0, 0.0], &State6::default());
        assert!((0..4).all(|i| (s4[i] - s6[i]).abs() < 1e-6 * m.fy) && (a4.alpha - a6.alpha).abs() < 1e-15);
        // Shear on another plane yields the same way.
        let (s, _) = m.stress6([0.0, 0.0, 0.0, 0.0, 0.003, 0.0], &State6::default());
        assert!((s[4] - (m.fy + m.h * 0.0) / 3f64.sqrt()).abs() < 0.05 * m.fy, "{}", s[4]);
    }

    #[test]
    fn a_return_lands_on_the_yield_surface_and_unloads_elastically() {
        let m = J2 { e: 70e9, nu: 0.33, fy: 100e6, h: 0.0 };
        let st = State::default();
        let (s, st1) = m.stress([0.004, -0.001, 0.0, 0.002], &st);
        assert!((J2::mises(s) - m.fy).abs() < 1e-6 * m.fy, "{}", J2::mises(s));
        // Taking a little strain back off is elastic: the history does not change.
        let (s2, st2) = m.stress([0.0039, -0.001, 0.0, 0.002], &st1);
        assert_eq!(st2, st1);
        assert!(J2::mises(s2) < m.fy);
        let _ = s2;
    }
}
