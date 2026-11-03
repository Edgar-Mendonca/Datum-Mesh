//! Basic modelling of parts (0.6.3). A part is a sketch swept along its z (or turned about its y)
//! and placed by a turn and a move (`Part::place`). Here: the turn as a matrix and back, a point
//! of the assembly in a part's own space, moving, turning and mirroring parts (`Placing`), and a
//! part's exact volume (area × depth, or Pappus' theorem for a turned part).

use crate::model::{Geom, Model, Part};
use serde::{Deserialize, Serialize};

pub type M3 = [[f64; 3]; 3];

/// How parts are placed anew (`Op::PlaceParts`): moved by a vector, turned about an axis through
/// a point (radians, right-handed), or mirrored in a plane through a point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Placing {
    Move { by: [f64; 3] },
    Turn { about: [f64; 3], axis: [f64; 3], angle: f64 },
    Mirror { point: [f64; 3], normal: [f64; 3] },
}

fn mul(a: &M3, b: &M3) -> M3 {
    let mut c = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    c
}
fn mulv(a: &M3, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}
fn tr(a: &M3) -> M3 {
    [0, 1, 2].map(|i| [a[0][i], a[1][i], a[2][i]])
}
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (n > 1e-12 && n.is_finite()).then(|| v.map(|c| c / n))
}

/// The turn of a part (about x, then y, then z) as a matrix: R = Rz · Ry · Rx.
pub fn rot(t: [f64; 3]) -> M3 {
    let (sx, cx, sy, cy, sz, cz) = (t[0].sin(), t[0].cos(), t[1].sin(), t[1].cos(), t[2].sin(), t[2].cos());
    let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    mul(&rz, &mul(&ry, &rx))
}

/// The turn (about x, then y, then z) of a rotation matrix; at a quarter turn about y (gimbal
/// lock) the turn about z is taken as zero.
pub fn euler(r: &M3) -> [f64; 3] {
    let sy = (-r[2][0]).clamp(-1.0, 1.0);
    let y = sy.asin();
    if sy.abs() > 1.0 - 1e-12 {
        // cos y = 0: only x − z (or x + z) is fixed; put it all in x.
        let x = (r[0][1] * sy).atan2(r[1][1]);
        return [x, y, 0.0].map(|v| if v.abs() < 1e-15 { 0.0 } else { v });
    }
    [r[2][1].atan2(r[2][2]), y, r[1][0].atan2(r[0][0])].map(|v| if v.abs() < 1e-15 { 0.0 } else { v })
}

/// A point of the assembly in a part's own space: sketch x, y, and z along the extrusion (the
/// inverse of `Part::place`).
pub fn local(p: &Part, q: [f64; 3]) -> [f64; 3] {
    mulv(&tr(&rot(p.turn)), [q[0] - p.at[0], q[1] - p.at[1], q[2] - p.at[2]])
}

/// The sketch point a point of the part lies over: straight back along the extrusion, or turned
/// back about the sketch's y axis for a turned part.
pub fn sketch_point(p: &Part, q: [f64; 3]) -> [f64; 2] {
    let l = local(p, q);
    if p.angle > 0.0 {
        [l[0].hypot(l[2]), l[1]]
    } else {
        [l[0], l[1]]
    }
}

/// The length of a part's sweep: its depth, or its angle when it is turned.
pub fn sweep(p: &Part) -> f64 {
    if p.angle > 0.0 {
        p.angle
    } else {
        p.depth
    }
}

/// A rigid change of place q → R q + t; `flip` when it is a mirror image (det R = −1).
struct Change {
    r: M3,
    t: [f64; 3],
    flip: bool,
}

fn change(how: &Placing) -> Result<Change, String> {
    let ok = |v: &[f64]| v.iter().all(|c| c.is_finite());
    match how {
        Placing::Move { by } => {
            if !ok(by) {
                return Err("Give the move as numbers.".into());
            }
            Ok(Change { r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: *by, flip: false })
        }
        Placing::Turn { about, axis, angle } => {
            let k = unit(*axis).ok_or("Give the axis to turn about (a direction that is not zero).")?;
            if !ok(about) || !angle.is_finite() {
                return Err("Give the turn as numbers.".into());
            }
            // Rodrigues: R = cos θ I + sin θ [k]× + (1 − cos θ) k kᵀ.
            let (s, c) = angle.sin_cos();
            let mut r = [[0.0; 3]; 3];
            let kx = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]];
            for i in 0..3 {
                for j in 0..3 {
                    r[i][j] = if i == j { c } else { 0.0 } + s * kx[i][j] + (1.0 - c) * k[i] * k[j];
                }
            }
            let ra = mulv(&r, *about);
            Ok(Change { r, t: [0, 1, 2].map(|i| about[i] - ra[i]), flip: false })
        }
        Placing::Mirror { point, normal } => {
            let n = unit(*normal).ok_or("Give the mirror plane's normal (a direction that is not zero).")?;
            if !ok(point) {
                return Err("Give the mirror plane's point as numbers.".into());
            }
            let r = [0, 1, 2].map(|i| [0, 1, 2].map(|j| if i == j { 1.0 } else { 0.0 } - 2.0 * n[i] * n[j]));
            let d = 2.0 * (n[0] * point[0] + n[1] * point[1] + n[2] * point[2]);
            Ok(Change { r, t: n.map(|c| d * c), flip: true })
        }
    }
}

/// A part placed anew by `how`, applied `times` times. A mirror image of a part is the same sketch
/// swept the other way: the sweep runs back from the sketch instead of forward (z → −z), which
/// keeps the part's turn a true rotation.
pub fn placed(p: &Part, how: &Placing, times: u32) -> Result<Part, String> {
    let c = change(how)?;
    let mut q = p.clone();
    for _ in 0..times {
        let mut r = mul(&c.r, &rot(q.turn));
        if c.flip {
            for row in r.iter_mut() {
                row[2] = -row[2];
            }
            q.from = -(q.from + sweep(&q));
        }
        q.turn = euler(&r);
        let a = mulv(&c.r, q.at);
        q.at = [0, 1, 2].map(|i| a[i] + c.t[i]);
    }
    Ok(q)
}

/// Is the point inside the shape's outline (arcs followed closely)?
pub fn contains(g: &Geom, p: [f64; 2]) -> bool {
    if let Geom::Circle { c, r } = g {
        return (p[0] - c[0]).hypot(p[1] - c[1]) < *r;
    }
    let (pts, _) = g.outline(f64::MAX, 0);
    let size = pts.iter().fold(0.0_f64, |s, a| s.max(a[0].abs()).max(a[1].abs())).max(1e-9);
    let (pts, _) = g.outline(size / 200.0, 64);
    let mut inside = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + n - 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// How near these points come to a shape's outline (the shortest distance to any of its edges,
/// arcs followed closely).
pub fn gap(g: &Geom, pts: &[[f64; 2]]) -> f64 {
    let (o, _) = g.outline(f64::MAX, 0);
    let size = o.iter().fold(0.0_f64, |s, a| s.max(a[0].abs()).max(a[1].abs())).max(1e-9);
    let (o, _) = g.outline(size / 400.0, 128);
    let n = o.len();
    let seg = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
    };
    pts.iter().map(|p| (0..n).map(|i| seg(*p, o[i], o[(i + 1) % n])).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min)
}

/// The first moment ∫x dA of a shape's area (signed as its area is): polygons by Green's theorem,
/// each arc edge adding its circular segment at the segment's centroid.
pub fn moment_x(g: &Geom) -> f64 {
    match g {
        Geom::Circle { c, r } => std::f64::consts::PI * r * r * c[0],
        Geom::Poly { pts, bulge } => {
            let n = pts.len();
            let mut s = 0.0;
            for i in 0..n {
                let (p, q) = (pts[i], pts[(i + 1) % n]);
                s += (p[0] + q[0]) * (p[0] * q[1] - q[0] * p[1]) / 6.0;
                let b = bulge.get(i).copied().unwrap_or(0.0);
                if b != 0.0 {
                    let th = 4.0 * b.abs().atan();
                    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
                    let c = dx.hypot(dy);
                    let r = c / (2.0 * (th / 2.0).sin());
                    let area = 0.5 * r * r * (th - th.sin());
                    // From the chord's middle towards the arc: the right of p → q for a positive
                    // bulge. The centre is r cos(θ/2) back from the chord, the segment's centroid
                    // 4 r sin³(θ/2) / 3(θ − sin θ) forward from the centre.
                    let ux = if b > 0.0 { dy / c } else { -dy / c };
                    let off = 4.0 * r * (th / 2.0).sin().powi(3) / (3.0 * (th - th.sin())) - r * (th / 2.0).cos();
                    let cx = (p[0] + q[0]) / 2.0 + ux * off;
                    s += b.signum() * area * cx;
                }
            }
            s
        }
    }
}

/// A part's volume (m³): its sketch's area (outlines less holes) times its depth, or, turned, the
/// area's first moment about the axis times the angle (Pappus).
pub fn volume(m: &Model, part: u32) -> Option<f64> {
    let p = m.parts.iter().find(|p| p.id == part)?;
    let mut area = 0.0;
    let mut first = 0.0;
    for s in m.shapes.iter().filter(|s| s.part == part && s.plate.is_none()) {
        let (a, q) = (s.geom.area(), moment_x(&s.geom));
        // Outlines may run either way round: take each with the sign of its area.
        let sg = if a < 0.0 { -1.0 } else { 1.0 };
        let k = if s.hole { -1.0 } else { 1.0 };
        area += k * a.abs();
        first += k * sg * q;
    }
    Some(if p.angle > 0.0 { p.angle * first } else { p.depth * area })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Geom;

    fn close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < tol)
    }

    #[test]
    fn a_turn_goes_to_a_matrix_and_back() {
        for t in [[0.3, -0.2, 0.5], [0.0, 0.0, 0.0], [-1.2, 1.0, 2.9], [0.0, std::f64::consts::FRAC_PI_2, 0.0], [0.4, -std::f64::consts::FRAC_PI_2, 0.0]] {
            let p = Part { turn: t, ..Default::default() };
            let q = Part { turn: euler(&rot(t)), ..Default::default() };
            for v in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.3, -0.7, 0.2]] {
                assert!(close(p.place(v), q.place(v), 1e-12), "{t:?}");
                assert!(close(mulv(&rot(t), v), p.place(v), 1e-12), "{t:?}");
            }
        }
    }

    #[test]
    fn local_undoes_place() {
        let p = Part { turn: [0.3, -0.2, 0.5], at: [1.0, 2.0, 3.0], ..Default::default() };
        for v in [[0.1, 0.2, 0.3], [-1.0, 0.0, 2.0]] {
            assert!(close(local(&p, p.place(v)), v, 1e-12));
        }
    }

    #[test]
    fn a_turned_moved_or_mirrored_part_lies_where_its_points_go() {
        let p = Part { depth: 0.3, turn: [0.3, -0.2, 0.5], at: [1.0, 2.0, 3.0], ..Default::default() };
        // Turned a third of a turn about an axis through (1, 0, 0) along (1, 1, 1), twice.
        let how = Placing::Turn { about: [1.0, 0.0, 0.0], axis: [1.0, 1.0, 1.0], angle: std::f64::consts::TAU / 3.0 };
        let q = placed(&p, &how, 3).unwrap();
        // Three thirds of a turn: back where it was.
        for v in [[0.1, 0.2, 0.0], [0.0, 0.0, 0.3]] {
            assert!(close(q.place(v), p.place(v), 1e-12));
        }
        let q = placed(&p, &Placing::Move { by: [0.5, 0.0, -1.0] }, 2).unwrap();
        let (a, b) = (p.place([0.1, 0.2, 0.1]), q.place([0.1, 0.2, 0.1]));
        assert!(close(b, [a[0] + 1.0, a[1], a[2] - 2.0], 1e-12));
        // Mirrored in the plane x = 2: every point of the sweep goes to its mirror image.
        let q = placed(&p, &Placing::Mirror { point: [2.0, 0.0, 0.0], normal: [3.0, 0.0, 0.0] }, 1).unwrap();
        assert!((q.from + 0.3).abs() < 1e-15);
        for v in [[0.1, 0.2, 0.0], [0.05, 0.0, 0.3], [0.2, 0.1, 0.15]] {
            let a = p.place(v);
            let b = q.place([v[0], v[1], -v[2]]);
            assert!(close(b, [4.0 - a[0], a[1], a[2]], 1e-12), "{a:?} {b:?}");
        }
        assert!(placed(&p, &Placing::Turn { about: [0.0; 3], axis: [0.0; 3], angle: 1.0 }, 1).is_err());
    }

    #[test]
    fn first_moments_and_volumes_match_hand_formulas() {
        use std::f64::consts::PI;
        // A circle at x = 2 and the same circle as two half-circle arcs, either way round.
        let c = Geom::Circle { c: [2.0, 1.0], r: 0.5 };
        let two = Geom::Poly { pts: vec![[2.5, 1.0], [1.5, 1.0]], bulge: vec![1.0, 1.0] };
        let back = Geom::Poly { pts: vec![[2.5, 1.0], [1.5, 1.0]], bulge: vec![-1.0, -1.0] };
        let want = PI * 0.25 * 2.0;
        assert!((moment_x(&c) - want).abs() < 1e-12);
        assert!((moment_x(&two) - want).abs() < 1e-12, "{}", moment_x(&two));
        assert!((moment_x(&back) + want).abs() < 1e-12, "{}", moment_x(&back));
        // A rectangle with one corner rounded (a quarter arc), against the polygon of its outline.
        let g = Geom::Poly { pts: vec![[1.0, 0.0], [3.0, 0.0], [3.0, 0.6], [2.6, 1.0], [1.0, 1.0]], bulge: vec![0.0, 0.0, (PI / 8.0).tan(), 0.0, 0.0] };
        let (pts, _) = g.outline(1e-4, 64);
        let fine = Geom::Poly { bulge: vec![0.0; pts.len()], pts };
        assert!((moment_x(&g) - moment_x(&fine)).abs() < 1e-7, "{} {}", moment_x(&g), moment_x(&fine));
        assert!((g.area() - (2.0 - 0.16 * (1.0 - PI / 4.0))).abs() < 1e-12);
    }

    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, Shape};
    use std::f64::consts::PI;

    /// A model with one part: a sketch of these outlines (hole or not), extruded by `depth`.
    fn one(gs: &[(Geom, bool)], depth: f64, at: [f64; 3], turn: [f64; 3]) -> (Doc, u32) {
        let mut d = Doc::new(Model::new(Analysis::Solid));
        d.m.mesh.size = 0.02;
        let p = d.apply(Op::Add { item: Item::Part(Part { depth, at, turn, ..Default::default() }) }).unwrap()[0];
        for (g, hole) in gs {
            d.apply(Op::Add { item: Item::Shape(Shape { geom: g.clone(), hole: *hole, material: d.m.materials[0].id, part: p, ..Default::default() }) }).unwrap();
        }
        (d, p)
    }
    fn mesh_volume(m: &Model) -> f64 {
        let a = crate::solids::assembly::mesh(m).unwrap();
        (0..a.mesh.tets.len()).map(|e| {
            let el = &a.mesh.tets[e];
            let p = |i: usize| a.mesh.nodes[el[i] as usize];
            let s = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
            let (u, v, w) = (s(p(1), p(0)), s(p(2), p(0)), s(p(3), p(0)));
            (u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])) / 6.0
        }).sum()
    }

    #[test]
    fn a_reversed_sweep_runs_back_from_the_sketch() {
        let (mut d, p) = one(&[(Geom::rect(0.0, 0.0, 0.2, 0.1), false)], 0.3, [0.0; 3], [0.0; 3]);
        let mut q = d.m.parts[0].clone();
        q.from = -0.3;
        d.apply(Op::Set { item: Item::Part(q) }).unwrap();
        let a = crate::solids::assembly::mesh(&d.m).unwrap();
        let (lo, hi) = a.mesh.nodes.iter().fold((f64::MAX, f64::MIN), |(l, h), n| (l.min(n[2]), h.max(n[2])));
        assert!((lo + 0.3).abs() < 1e-12 && hi.abs() < 1e-12, "{lo} {hi}");
        assert!((volume(&d.m, p).unwrap() - 0.006).abs() < 1e-15);
        assert!((mesh_volume(&d.m) - 0.006).abs() < 1e-12);
    }

    #[test]
    fn parts_are_copied_in_a_ring_and_mirrored() {
        let (mut d, p) = one(&[(Geom::rect(0.5, 0.0, 0.7, 0.1), false)], 0.1, [0.0; 3], [0.0; 3]);
        // Three copies a quarter turn apart about the sketch's y axis (through the origin).
        let made = d.apply(Op::PlaceParts { ids: vec![p], how: Placing::Turn { about: [0.0; 3], axis: [0.0, 1.0, 0.0], angle: PI / 2.0 }, copies: 3 }).unwrap();
        assert_eq!(d.m.parts.len(), 4);
        assert_eq!(made.len(), 6, "three parts and their sketches");
        assert!((mesh_volume(&d.m) - 4.0 * 0.002).abs() < 1e-12);
        // The second copy is half a turn round: the point (0.6, 0.05, 0.05) goes to (−0.6, 0.05, −0.05).
        let c3 = d.m.parts.iter().find(|x| x.name.ends_with("copy 2")).unwrap();
        assert!(close(c3.place([0.6, 0.05, 0.05]), [-0.6, 0.05, -0.05], 1e-12), "{:?}", c3.place([0.6, 0.05, 0.05]));
        // A mirror copy in the plane x = 0: the original's mesh, mirrored.
        let (mut d, p) = one(&[(Geom::rect(0.5, 0.0, 0.7, 0.1), false)], 0.1, [0.1, 0.2, 0.3], [0.3, 0.2, -0.4]);
        d.apply(Op::PlaceParts { ids: vec![p], how: Placing::Mirror { point: [0.0; 3], normal: [1.0, 0.0, 0.0] }, copies: 1 }).unwrap();
        let a = crate::solids::assembly::mesh(&d.m).unwrap();
        let half = a.mesh.nodes.len() / 2;
        for n in &a.mesh.nodes[..half] {
            assert!(a.mesh.nodes[half..].iter().any(|m| close(*m, [-n[0], n[1], n[2]], 1e-12)), "{n:?}");
        }
        assert!((mesh_volume(&d.m) - 0.004).abs() < 1e-12);
        assert!(d.apply(Op::PlaceParts { ids: vec![p], how: Placing::Mirror { point: [0.0; 3], normal: [1.0, 0.0, 0.0] }, copies: 2 }).is_err());
        // Moved in place: no new part.
        let before = d.m.parts[0].place([0.0; 3]);
        d.apply(Op::PlaceParts { ids: vec![p], how: Placing::Move { by: [1.0, 0.0, 0.0] }, copies: 0 }).unwrap();
        assert_eq!(d.m.parts.len(), 2);
        assert!(close(d.m.parts[0].place([0.0; 3]), [before[0] + 1.0, before[1], before[2]], 1e-12));
    }

    #[test]
    fn a_cut_and_a_hole_go_through_a_plate_and_an_edge_is_rounded() {
        // A plate 0.2 × 0.1, 0.02 thick; a disc r = 0.02 sketched on its front face (its own part,
        // turned half a turn so its sketch faces the other way) cuts through it.
        let (mut d, plate) = one(&[(Geom::rect(0.0, 0.0, 0.2, 0.1), false)], 0.02, [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let tool = d.apply(Op::Add { item: Item::Part(Part { depth: 0.01, at: [1.0, 0.0, 0.02], turn: [0.0, PI, 0.0], ..Default::default() }) }).unwrap()[0];
        // Turned half a turn about y, sketch x runs along −x: x = −0.05 lies over plate x = 0.05.
        d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::Circle { c: [-0.05, 0.05], r: 0.02 }, material: d.m.materials[0].id, part: tool, ..Default::default() }) }).unwrap();
        d.apply(Op::Cut { tool, from: plate }).unwrap();
        assert_eq!(d.m.parts.len(), 1);
        let h = d.m.shapes.iter().find(|s| s.hole).unwrap();
        assert!(matches!(h.geom, Geom::Circle { c, r } if (c[0] - 0.05).abs() < 1e-12 && (c[1] - 0.05).abs() < 1e-12 && r == 0.02), "{:?}", h.geom);
        let want = (0.02 - PI * 4e-4) * 0.02;
        assert!((volume(&d.m, plate).unwrap() - want).abs() < 1e-15);
        // A hole 20 mm across at (0.15, 0.05) on the back face.
        d.apply(Op::Hole { part: plate, at: [1.15, 0.05, 0.0], d: 0.02 }).unwrap();
        assert!((volume(&d.m, plate).unwrap() - (want - PI * 1e-4 * 0.02)).abs() < 1e-15);
        assert!(d.apply(Op::Hole { part: plate, at: [1.15, 0.05, 0.01], d: 0.02 }).unwrap_err().contains("front or back face"));
        assert!(d.apply(Op::Hole { part: plate, at: [1.05, 0.05, 0.0], d: 0.01 }).unwrap_err().contains("centre must be on the part"));
        assert!(d.apply(Op::Hole { part: plate, at: [1.3, 0.05, 0.0], d: 0.01 }).is_err());
        // The edge along the depth at the corner (0.2, 0.1), clicked a little off it, rounded 10 mm.
        d.apply(Op::PartEdge { part: plate, near: [1.199, 0.1, 0.012], size: 0.01, round: true }).unwrap();
        let want = want - PI * 1e-4 * 0.02 - 1e-4 * (1.0 - PI / 4.0) * 0.02;
        assert!((volume(&d.m, plate).unwrap() - want).abs() < 1e-15, "{} {want}", volume(&d.m, plate).unwrap());
        // A hole, or a cut, that just touches the edge is refused (it would leave a sliver).
        assert!(d.apply(Op::Hole { part: plate, at: [1.18, 0.05, 0.0], d: 0.04 }).unwrap_err().contains("edge"));
        let touch = d.apply(Op::Add { item: Item::Part(Part { depth: 0.01, at: [1.0, 0.0, 0.02], ..Default::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::Circle { c: [0.1, 0.09], r: 0.01 }, material: d.m.materials[0].id, part: touch, ..Default::default() }) }).unwrap();
        assert!(d.apply(Op::Cut { tool: touch, from: plate }).unwrap_err().contains("touches"));
        d.apply(Op::Delete { ids: vec![touch] }).unwrap();
        // A hole that would reach the edge is refused.
        assert!(d.apply(Op::Hole { part: plate, at: [1.15, 0.09, 0.0], d: 0.03 }).unwrap_err().contains("edge"));
        // A disc across the plate's edge cuts a notch out of the outline; one beside it misses.
        let before = volume(&d.m, plate).unwrap();
        let notch = d.apply(Op::Add { item: Item::Part(Part { depth: 0.01, at: [1.0, 0.0, 0.02], ..Default::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::Circle { c: [0.1, 0.0], r: 0.01 }, material: d.m.materials[0].id, part: notch, ..Default::default() }) }).unwrap();
        d.apply(Op::Cut { tool: notch, from: plate }).unwrap();
        assert!((volume(&d.m, plate).unwrap() - (before - PI * 1e-4 / 2.0 * 0.02)).abs() < 1e-9, "{} {}", volume(&d.m, plate).unwrap(), before);
        assert_eq!(d.m.shapes.iter().filter(|s| s.part == plate && s.hole).count(), 2, "the notch is in the outline, not a hole");
        let miss = d.apply(Op::Add { item: Item::Part(Part { depth: 0.01, at: [1.0, 0.0, 0.02], ..Default::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::Circle { c: [0.4, 0.05], r: 0.01 }, material: d.m.materials[0].id, part: miss, ..Default::default() }) }).unwrap();
        assert!(d.apply(Op::Cut { tool: miss, from: plate }).unwrap_err().contains("misses"));
        // A cut whose sketch is not parallel to the plate's is refused.
        let side = d.apply(Op::Add { item: Item::Part(Part { depth: 0.01, turn: [0.0, PI / 2.0, 0.0], ..Default::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::Circle { c: [0.0, 0.05], r: 0.01 }, material: d.m.materials[0].id, part: side, ..Default::default() }) }).unwrap();
        assert!(d.apply(Op::Cut { tool: side, from: plate }).unwrap_err().contains("along its depth"));
    }

    #[test]
    fn a_turned_part_has_its_volume_by_pappus() {
        // A ring: the rectangle 0.1 ≤ x ≤ 0.2, 0 ≤ y ≤ 0.05 turned a full turn.
        let (mut d, p) = one(&[(Geom::rect(0.1, 0.0, 0.2, 0.05), false)], 0.1, [0.0; 3], [0.0; 3]);
        let mut q = d.m.parts[0].clone();
        q.angle = 2.0 * PI;
        d.apply(Op::Set { item: Item::Part(q) }).unwrap();
        assert!((volume(&d.m, p).unwrap() - PI * (0.04 - 0.01) * 0.05).abs() < 1e-15);
    }
}
