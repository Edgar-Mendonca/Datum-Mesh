//! Cross-section properties for frame members: area, second moment of area about the axis of
//! in-plane bending, and the distance from the neutral axis to the extreme fibre.

use crate::model::SectionShape;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Props {
    /// Area (m²).
    pub a: f64,
    /// Second moment of area for in-plane bending (m⁴): about the section's strong axis, bending
    /// along its depth h.
    pub i: f64,
    /// Distance from the neutral axis to the extreme fibre (m).
    pub c: f64,
    /// 3D frames (0.2.4): the second moment about the other axis (bending across the width b),
    /// its extreme fibre, and the torsion constant J (m⁴).
    pub iy: f64,
    pub cy: f64,
    pub j: f64,
}

impl Props {
    /// Elastic section modulus (m³).
    pub fn z(&self) -> f64 {
        self.i / self.c
    }
    /// Elastic section modulus about the other axis (m³).
    pub fn zy(&self) -> f64 {
        self.iy / self.cy
    }
}

/// Torsion constant of a solid rectangle (Roark, table 10.7 case 4): J = a b³ (1/3 − 0.21 (b/a)
/// (1 − b⁴/12a⁴)), a the longer side; within 0.5 % of the series solution.
fn rect_j(b: f64, h: f64) -> f64 {
    let (a, b) = (b.max(h), b.min(h));
    a * b.powi(3) * (1.0 / 3.0 - 0.21 * b / a * (1.0 - b.powi(4) / (12.0 * a.powi(4))))
}

/// One root fillet of radius r (a square of side r less a quarter circle centred at its far
/// corner): its area, its centroid's distance from the two faces it lies against, and its second
/// moment about its own centroid (the same both ways).
fn fillet(r: f64) -> (f64, f64, f64) {
    if r == 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let pi = std::f64::consts::PI;
    let (aq, cq) = (pi * r * r / 4.0, r - 4.0 * r / (3.0 * pi));
    let a = r * r - aq;
    let c = (r * r * r / 2.0 - aq * cq) / a;
    // About the face: the square's r⁴/3, less the quarter circle's (about its own centroid, then moved).
    let iq = pi * r.powi(4) / 16.0 - aq * (4.0 * r / (3.0 * pi)).powi(2);
    let i_face = r.powi(4) / 3.0 - (iq + aq * cq * cq);
    (a, c, i_face - a * c * c)
}

/// The properties of a section shape, or why the dimensions do not make a section.
pub fn props(s: &SectionShape) -> Result<Props, String> {
    use SectionShape::*;
    let dims: Vec<f64> = match s {
        Rect { b, h } => vec![*b, *h],
        Circle { d } => vec![*d],
        Tube { d, t } => vec![*d, *t],
        Box { b, h, t } => vec![*b, *h, *t],
        I { h, b, tw, tf, .. } | Channel { h, b, tw, tf } => vec![*h, *b, *tw, *tf],
        Angle { h, b, t } => vec![*h, *b, *t],
        Custom { a, i, c, .. } => vec![*a, *i, *c],
    };
    if dims.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return Err("every dimension must be greater than zero".into());
    }
    let pi = std::f64::consts::PI;
    let p = match *s {
        Rect { b, h } => Props { a: b * h, i: b * h.powi(3) / 12.0, c: h / 2.0, iy: h * b.powi(3) / 12.0, cy: b / 2.0, j: rect_j(b, h) },
        Circle { d } => {
            let i = pi * d.powi(4) / 64.0;
            Props { a: pi * d * d / 4.0, i, c: d / 2.0, iy: i, cy: d / 2.0, j: 2.0 * i }
        }
        Tube { d, t } => {
            if 2.0 * t >= d {
                return Err("the wall must be thinner than half the diameter".into());
            }
            let di = d - 2.0 * t;
            let i = pi * (d.powi(4) - di.powi(4)) / 64.0;
            Props { a: pi * (d * d - di * di) / 4.0, i, c: d / 2.0, iy: i, cy: d / 2.0, j: 2.0 * i }
        }
        Box { b, h, t } => {
            if 2.0 * t >= b || 2.0 * t >= h {
                return Err("the wall must be thinner than half the width and depth".into());
            }
            let (bi, hi) = (b - 2.0 * t, h - 2.0 * t);
            // Thin-walled closed section (Bredt): J = 4 A_m² t / perimeter of the mid-line.
            let j = 2.0 * t * (b - t).powi(2) * (h - t).powi(2) / (b + h - 2.0 * t);
            Props { a: b * h - bi * hi, i: (b * h.powi(3) - bi * hi.powi(3)) / 12.0, c: h / 2.0, iy: (h * b.powi(3) - hi * bi.powi(3)) / 12.0, cy: b / 2.0, j }
        }
        I { h, b, tw, tf, .. } | Channel { h, b, tw, tf } => {
            if 2.0 * tf >= h || tw >= b {
                return Err("the flanges and web do not fit the depth and width".into());
            }
            let r = if let I { r, .. } = *s { r } else { 0.0 };
            if !(r.is_finite() && r >= 0.0) || 2.0 * (tf + r) >= h || tw + 2.0 * r >= b {
                return Err("the root radius does not fit between the flanges and the web".into());
            }
            let hw = h - 2.0 * tf;
            let a = 2.0 * b * tf + hw * tw;
            // Open thin-walled section: J = Σ b t³ / 3.
            let j = (2.0 * b * tf.powi(3) + hw * tw.powi(3)) / 3.0;
            // About the axis along the depth: the I is symmetric; the channel's web is at one side.
            let (iy, cy) = if matches!(s, I { .. }) {
                (2.0 * tf * b.powi(3) / 12.0 + hw * tw.powi(3) / 12.0, b / 2.0)
            } else {
                let x = (2.0 * b * tf * b / 2.0 + hw * tw * tw / 2.0) / a;
                (2.0 * (tf * b.powi(3) / 12.0 + b * tf * (b / 2.0 - x).powi(2)) + hw * tw.powi(3) / 12.0 + hw * tw * (tw / 2.0 - x).powi(2), x.max(b - x))
            };
            let i = (b * h.powi(3) - (b - tw) * hw.powi(3)) / 12.0;
            // Rolled sections (0.3.3): the four fillets where the web meets the flanges, each a
            // square of side r less a quarter circle, at their own distances from both axes.
            let (fa, fc, fi) = fillet(r);
            let (dy, dx) = (h / 2.0 - tf - fc, tw / 2.0 + fc);
            Props { a: a + 4.0 * fa, i: i + 4.0 * (fi + fa * dy * dy), c: h / 2.0, iy: iy + 4.0 * (fi + fa * dx * dx), cy, j }
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
            // About the axis parallel to h (the legs' own axes; the product of inertia is left out).
            let (x1, x2) = (t / 2.0, t + (b - t) / 2.0);
            let x = (a1 * x1 + a2 * x2) / a;
            let iy = h * t.powi(3) / 12.0 + a1 * (x1 - x).powi(2) + t * (b - t).powi(3) / 12.0 + a2 * (x2 - x).powi(2);
            Props { a, i: i1 + a1 * (y1 - y).powi(2) + i2 + a2 * (y2 - y).powi(2), c: y.max(h - y), iy, cy: x.max(b - x), j: (h + b - t) * t.powi(3) / 3.0 }
        }
        Custom { a, i, c, iy, j } => Props { a, i, c, iy: if iy > 0.0 { iy } else { i }, cy: c, j: if j > 0.0 { j } else { 2.0 * i } },
    };
    if let Custom { iy, j, .. } = *s {
        if !(iy.is_finite() && iy >= 0.0 && j.is_finite() && j >= 0.0) {
            return Err("the second moment and torsion constant must be zero or more".into());
        }
    }
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
        let i = props(&I { h: 0.3, b: 0.15, tw: 0.0071, tf: 0.0107, r: 0.0 }).unwrap();
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
    fn the_other_axis_and_torsion() {
        // Rectangle 100 × 200: Iy = 0.2·0.1³/12; J for b/a = 1/2 is 0.2287·a·b³ (Roark's series
        // value 0.229), within 0.5 %.
        let r = props(&Rect { b: 0.1, h: 0.2 }).unwrap();
        assert!(close(r.iy, 0.2 * 0.1f64.powi(3) / 12.0) && close(r.cy, 0.05));
        assert!((r.j / (0.229 * 0.2 * 0.1f64.powi(3)) - 1.0).abs() < 5e-3, "{}", r.j);
        // A square: J = 0.1406 a⁴ (Timoshenko).
        let q = props(&Rect { b: 0.1, h: 0.1 }).unwrap();
        assert!((q.j / (0.1406 * 1e-4) - 1.0).abs() < 5e-3, "{}", q.j);
        // Round bars and tubes: J = 2I = Ip, and the same I both ways.
        let t = props(&Tube { d: 0.1, t: 0.005 }).unwrap();
        assert!(close(t.j, 2.0 * t.i) && close(t.iy, t.i));
        // A square box of thin wall: J = b³ t for the mid-line b (Bredt).
        let bx = props(&Box { b: 0.2, h: 0.2, t: 0.002 }).unwrap();
        assert!((bx.j / (0.198f64.powi(3) * 0.002) - 1.0).abs() < 1e-9);
        // An I: Iy from the two flanges and the web; J = Σ b t³/3.
        let i = props(&I { h: 0.3, b: 0.15, tw: 0.0071, tf: 0.0107, r: 0.0 }).unwrap();
        assert!(close(i.iy, 2.0 * 0.0107 * 0.15f64.powi(3) / 12.0 + (0.3 - 0.0214) * 0.0071f64.powi(3) / 12.0));
        assert!(close(i.j, (2.0 * 0.15 * 0.0107f64.powi(3) + (0.3 - 0.0214) * 0.0071f64.powi(3)) / 3.0));
        // Custom without the 3D values: a round bar's.
        let c = props(&Custom { a: 1e-3, i: 2e-6, c: 0.05, iy: 0.0, j: 0.0 }).unwrap();
        assert!(c.iy == 2e-6 && c.j == 4e-6);
    }

    #[test]
    fn impossible_sections_are_refused() {
        assert!(props(&Rect { b: 0.0, h: 0.2 }).is_err());
        assert!(props(&Tube { d: 0.1, t: 0.05 }).is_err());
        assert!(props(&I { h: 0.1, b: 0.1, tw: 0.2, tf: 0.01, r: 0.0 }).is_err());
        assert!(props(&Custom { a: 1.0, i: f64::NAN, c: 1.0, iy: 0.0, j: 0.0 }).is_err());
        assert!(props(&Custom { a: 1.0, i: 1.0, c: 1.0, iy: -1.0, j: 0.0 }).is_err());
    }
}
