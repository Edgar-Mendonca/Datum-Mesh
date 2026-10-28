//! Cross-section properties for frame members: area, second moment of area about the axis of
//! in-plane bending, and the distance from the neutral axis to the extreme fibre.

use crate::model::SectionShape;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Props {
    /// Area (m²).
    pub a: f64,
    /// Second moment of area for in-plane bending (m⁴).
    pub i: f64,
    /// Distance from the neutral axis to the extreme fibre (m).
    pub c: f64,
}

impl Props {
    /// Elastic section modulus (m³).
    pub fn z(&self) -> f64 {
        self.i / self.c
    }
}

/// The properties of a section shape, or why the dimensions do not make a section.
pub fn props(s: &SectionShape) -> Result<Props, String> {
    use SectionShape::*;
    let dims: Vec<f64> = match s {
        Rect { b, h } => vec![*b, *h],
        Circle { d } => vec![*d],
        Tube { d, t } => vec![*d, *t],
        Box { b, h, t } => vec![*b, *h, *t],
        I { h, b, tw, tf } | Channel { h, b, tw, tf } => vec![*h, *b, *tw, *tf],
        Angle { h, b, t } => vec![*h, *b, *t],
        Custom { a, i, c } => vec![*a, *i, *c],
    };
    if dims.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return Err("every dimension must be greater than zero".into());
    }
    let pi = std::f64::consts::PI;
    let p = match *s {
        Rect { b, h } => Props { a: b * h, i: b * h.powi(3) / 12.0, c: h / 2.0 },
        Circle { d } => Props { a: pi * d * d / 4.0, i: pi * d.powi(4) / 64.0, c: d / 2.0 },
        Tube { d, t } => {
            if 2.0 * t >= d {
                return Err("the wall must be thinner than half the diameter".into());
            }
            let di = d - 2.0 * t;
            Props { a: pi * (d * d - di * di) / 4.0, i: pi * (d.powi(4) - di.powi(4)) / 64.0, c: d / 2.0 }
        }
        Box { b, h, t } => {
            if 2.0 * t >= b || 2.0 * t >= h {
                return Err("the wall must be thinner than half the width and depth".into());
            }
            let (bi, hi) = (b - 2.0 * t, h - 2.0 * t);
            Props { a: b * h - bi * hi, i: (b * h.powi(3) - bi * hi.powi(3)) / 12.0, c: h / 2.0 }
        }
        I { h, b, tw, tf } | Channel { h, b, tw, tf } => {
            if 2.0 * tf >= h || tw >= b {
                return Err("the flanges and web do not fit the depth and width".into());
            }
            let hw = h - 2.0 * tf;
            Props { a: 2.0 * b * tf + hw * tw, i: (b * h.powi(3) - (b - tw) * hw.powi(3)) / 12.0, c: h / 2.0 }
        }
        Angle { h, b, t } => {
            if t >= h || t >= b {
                return Err("the thickness must be less than both legs".into());
            }
            // The upright leg (t × h) and the rest of the bottom leg ((b − t) × t).
            let (a1, y1, i1) = (t * h, h / 2.0, t * h.powi(3) / 12.0);
            let (a2, y2, i2) = ((b - t) * t, t / 2.0, (b - t) * t.powi(3) / 12.0);
            let a = a1 + a2;
            let y = (a1 * y1 + a2 * y2) / a;
            Props { a, i: i1 + a1 * (y1 - y).powi(2) + i2 + a2 * (y2 - y).powi(2), c: y.max(h - y) }
        }
        Custom { a, i, c } => Props { a, i, c },
    };
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use SectionShape::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b.abs().max(1e-30)
    }

    #[test]
    fn hand_worked_sections() {
        // Rectangle 100 × 200 mm: A = 0.02 m², I = 0.1·0.2³/12 = 6.6667e-5 m⁴.
        let r = props(&Rect { b: 0.1, h: 0.2 }).unwrap();
        assert!(close(r.a, 0.02) && close(r.i, 6.666_666_666_666_667e-5) && close(r.c, 0.1));
        // Circle d = 50 mm: I = π·0.05⁴/64 = 3.0680e-7.
        let c = props(&Circle { d: 0.05 }).unwrap();
        assert!(close(c.i, std::f64::consts::PI * 0.05f64.powi(4) / 64.0));
        // An I-section equals the outer rectangle minus the two side voids.
        let i = props(&I { h: 0.3, b: 0.15, tw: 0.0071, tf: 0.0107 }).unwrap();
        let outer = 0.15 * 0.3f64.powi(3) / 12.0;
        let voids = (0.15 - 0.0071) * (0.3 - 2.0 * 0.0107f64).powi(3) / 12.0;
        assert!(close(i.i, outer - voids));
        assert!(close(i.a, 2.0 * 0.15 * 0.0107 + (0.3 - 0.0214) * 0.0071));
        // An angle 100 × 100 × 10: centroid 28.68 mm from the heel (by hand: (1000·50 + 900·5)/1900).
        let a = props(&Angle { h: 0.1, b: 0.1, t: 0.01 }).unwrap();
        let y = (0.001 * 0.05 + 0.0009 * 0.005) / 0.0019;
        assert!(close(a.c, 0.1 - y));
        assert!(close(a.a, 0.0019));
        // A tube with a thin wall tends to π dm³ t / 8 (dm the mean diameter).
        let t = props(&Tube { d: 0.2, t: 0.0002 }).unwrap();
        assert!((t.i / (std::f64::consts::PI * 0.1998f64.powi(3) * 0.0002 / 8.0) - 1.0).abs() < 2e-6);
    }

    #[test]
    fn impossible_sections_are_refused() {
        assert!(props(&Rect { b: 0.0, h: 0.2 }).is_err());
        assert!(props(&Tube { d: 0.1, t: 0.05 }).is_err());
        assert!(props(&I { h: 0.1, b: 0.1, tw: 0.2, tf: 0.01 }).is_err());
        assert!(props(&Custom { a: 1.0, i: f64::NAN, c: 1.0 }).is_err());
    }
}
