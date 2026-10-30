//! Sketch tools (0.3.1): rounding and bevelling corners, combining shapes, offsetting outlines and
//! the ready-made shapes (slots, rounded rectangles, regular polygons). Pure geometry on `Geom`;
//! `doc.rs` applies the results to the model and moves supports and loads onto the new edges.
//!
//! Combining keeps arcs as arcs: the outlines are cut into short straight pieces for the boolean
//! (i_overlay), and each edge of the result is then traced back to the line or circle it lies on,
//! so a run of pieces on one circle becomes one arc again, with its ends where the exact curves
//! meet.

use crate::model::{arc, Geom};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

type P = [f64; 2];
fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn mul(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn len(a: P) -> f64 {
    a[0].hypot(a[1])
}
fn dist(a: P, b: P) -> f64 {
    len(sub(a, b))
}
/// An angle wrapped into (−π, π].
fn wrap(a: f64) -> f64 {
    let a = a.rem_euclid(TAU);
    if a > PI {
        a - TAU
    } else {
        a
    }
}

/// Round (`round`, `size` = radius) or bevel (`size` = distance along each edge) corner `i` of a
/// polygon. Old edge e becomes e for e < i and e + 1 otherwise; the new arc or bevel is edge i.
pub fn corner(g: &Geom, i: usize, size: f64, round: bool) -> Result<Geom, String> {
    let Geom::Poly { pts, bulge } = g else { return Err("A circle has no corners.".into()) };
    let n = pts.len();
    if i >= n {
        return Err("That corner does not exist.".into());
    }
    if !(size.is_finite() && size > 0.0) {
        return Err(format!("The {} must be greater than zero.", if round { "radius" } else { "size" }));
    }
    let h = (i + n - 1) % n;
    if bulge[h] != 0.0 || bulge[i] != 0.0 {
        return Err("Only a corner between two straight edges can be rounded or bevelled.".into());
    }
    let (a, p, c) = (pts[h], pts[i], pts[(i + 1) % n]);
    let (la, lc) = (dist(a, p), dist(c, p));
    let (u, v) = (mul(sub(a, p), 1.0 / la), mul(sub(c, p), 1.0 / lc));
    // The angle between the two edges at the corner.
    let th = dot(u, v).clamp(-1.0, 1.0).acos();
    if th > PI - 1e-9 {
        return Err("The edges at this corner are in line: there is no corner to round.".into());
    }
    let t = if round { size / (th / 2.0).tan() } else { size };
    if t >= la.min(lc) * (1.0 - 1e-9) {
        return Err(format!("That is too big for this corner: it would use up a whole edge ({} ≥ {:.4} m along it).", if round { "the round reaches" } else { "the bevel reaches" }, la.min(lc)));
    }
    // Turning left (counter-clockwise) at the corner, the arc turns left too.
    let turn = cross(sub(p, a), sub(c, p)).signum();
    let b = if round { turn * ((PI - th) / 4.0).tan() } else { 0.0 };
    let (mut pts, mut bulge) = (pts.clone(), bulge.clone());
    pts[i] = add(p, mul(u, t));
    pts.insert(i + 1, add(p, mul(v, t)));
    bulge[i] = b;
    bulge.insert(i + 1, 0.0);
    Ok(Geom::Poly { pts, bulge })
}

/// How two shapes combine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    Union,
    Subtract,
    Intersect,
}

/// The line or circle an edge lies on.
#[derive(Clone, Copy, Debug)]
enum Carrier {
    Line { p: P, d: P },
    Circle { c: P, r: f64 },
}

/// One edge of an input shape: its carrier, and the piece of it (a segment, or an arc from angle
/// `a0` turning through `th`).
#[derive(Clone, Copy, Debug)]
struct Edge {
    src: usize,
    edge: u32,
    carrier: usize,
    p: P,
    q: P,
    /// For arcs: centre, radius, start angle and swept angle.
    arc: Option<(P, f64, f64, f64)>,
}

impl Edge {
    /// Is `x` on this edge (within `tol`)?
    fn holds(&self, x: P, tol: f64) -> bool {
        match self.arc {
            None => {
                let d = sub(self.q, self.p);
                let l2 = dot(d, d);
                let t = (dot(sub(x, self.p), d) / l2).clamp(0.0, 1.0);
                dist(x, add(self.p, mul(d, t))) < tol
            }
            Some((c, r, a0, th)) => {
                if (dist(x, c) - r).abs() > tol {
                    return false;
                }
                let a = (x[1] - c[1]).atan2(x[0] - c[0]);
                // How far along the sweep, in the sweep's direction.
                let s = if th >= 0.0 { (a - a0).rem_euclid(TAU) } else { (a0 - a).rem_euclid(TAU) };
                let slack = tol / r;
                s <= th.abs() + slack || s >= TAU - slack
            }
        }
    }
}

/// One result shape of a combination: its outline, and for each edge the input edges it came from
/// (input 0 is `a`, 1 is `b`).
#[derive(Clone, Debug)]
pub struct Piece {
    pub geom: Geom,
    /// True for a hole in the result (a ring inside an outline).
    pub inner: bool,
    pub from: Vec<Vec<(usize, u32)>>,
}

fn edges_of(g: &Geom, src: usize, carriers: &mut Vec<Carrier>, tol: f64, out: &mut Vec<Edge>) {
    let mut find = |c: Carrier| -> usize {
        let same = |k: &Carrier| match (k, &c) {
            (Carrier::Line { p, d }, Carrier::Line { p: p2, d: d2 }) => cross(*d, *d2).abs() < 1e-9 && cross(*d, sub(*p2, *p)).abs() < tol,
            (Carrier::Circle { c, r }, Carrier::Circle { c: c2, r: r2 }) => dist(*c, *c2) < tol && (r - r2).abs() < tol,
            _ => false,
        };
        match carriers.iter().position(same) {
            Some(k) => k,
            None => {
                carriers.push(c);
                carriers.len() - 1
            }
        }
    };
    match g {
        Geom::Circle { c, r } => {
            let k = find(Carrier::Circle { c: *c, r: *r });
            let p = [c[0] + r, c[1]];
            out.push(Edge { src, edge: 0, carrier: k, p, q: p, arc: Some((*c, *r, 0.0, TAU)) });
        }
        Geom::Poly { pts, bulge } => {
            let n = pts.len();
            for i in 0..n {
                let (p, q) = (pts[i], pts[(i + 1) % n]);
                let b = bulge.get(i).copied().unwrap_or(0.0);
                if b == 0.0 {
                    let d = sub(q, p);
                    let k = find(Carrier::Line { p, d: mul(d, 1.0 / len(d)) });
                    out.push(Edge { src, edge: i as u32, carrier: k, p, q, arc: None });
                } else {
                    let (c, r, a0, th) = arc(p, q, b);
                    let k = find(Carrier::Circle { c, r });
                    out.push(Edge { src, edge: i as u32, carrier: k, p, q, arc: Some((c, r, a0, th)) });
                }
            }
        }
    }
}

/// Where carriers `a` and `b` meet, nearest to `x` (None if they do not meet near it).
fn meet(a: Carrier, b: Carrier, x: P) -> Option<P> {
    fn line_circle(p: P, d: P, c: P, r: f64) -> Vec<P> {
        // |p + t d − c|² = r², d a unit vector.
        let w = sub(p, c);
        let (bb, cc) = (dot(w, d), dot(w, w) - r * r);
        let disc = bb * bb - cc;
        if disc < -1e-12 * r * r {
            return vec![];
        }
        let s = disc.max(0.0).sqrt();
        vec![add(p, mul(d, -bb - s)), add(p, mul(d, -bb + s))]
    }
    let cands: Vec<P> = match (a, b) {
        (Carrier::Line { p, d }, Carrier::Line { p: q, d: e }) => {
            let den = cross(d, e);
            if den.abs() < 1e-12 {
                return None;
            }
            vec![add(p, mul(d, cross(sub(q, p), e) / den))]
        }
        (Carrier::Line { p, d }, Carrier::Circle { c, r }) | (Carrier::Circle { c, r }, Carrier::Line { p, d }) => line_circle(p, d, c, r),
        (Carrier::Circle { c: c1, r: r1 }, Carrier::Circle { c: c2, r: r2 }) => {
            let dd = dist(c1, c2);
            if dd < 1e-12 {
                return None;
            }
            let a = (dd * dd + r1 * r1 - r2 * r2) / (2.0 * dd);
            let h2 = r1 * r1 - a * a;
            if h2 < -1e-9 * r1 * r1 {
                return None;
            }
            let u = mul(sub(c2, c1), 1.0 / dd);
            let m = add(c1, mul(u, a));
            let h = h2.max(0.0).sqrt();
            let nrm = [-u[1], u[0]];
            vec![add(m, mul(nrm, h)), sub(m, mul(nrm, h))]
        }
    };
    cands.into_iter().min_by(|p, q| dist(*p, x).total_cmp(&dist(*q, x)))
}

/// Union, subtract (`a` − `b`) or intersect two shapes. Arcs and circles stay arcs and circles.
pub fn combine(a: &Geom, b: &Geom, how: How) -> Result<Vec<Piece>, String> {
    let (oa, _) = a.outline(1.0, 8);
    let (ob, _) = b.outline(1.0, 8);
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in oa.iter().chain(ob.iter()) {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-9);
    // The boolean works on short straight pieces; `eps` is how close a corner of the result lies
    // to the pieces it is on (the boolean's rounding), `tol` how close the pieces lie to the
    // true curves.
    let (seg, min) = (span / 1000.0, 360);
    let fa = a.outline(seg, min).0;
    let fb = b.outline(seg, min).0;
    let eps = span * 1e-7;
    let tol = span * 2e-5;

    let mut carriers = vec![];
    let mut edges = vec![];
    edges_of(a, 0, &mut carriers, eps, &mut edges);
    edges_of(b, 1, &mut carriers, eps, &mut edges);
    // The pieces each outline was cut into, with the edge each belongs to.
    let pieces = |g: &Geom, flat: &[P], src: usize| -> Vec<(P, P, usize)> {
        let (_, starts) = g.outline(seg, min);
        let n = flat.len();
        let base = edges.iter().position(|e| e.src == src).unwrap_or(0);
        (0..n)
            .map(|i| {
                let e = starts.iter().rposition(|s| *s <= i).unwrap_or(0);
                (flat[i], flat[(i + 1) % n], base + e)
            })
            .collect()
    };
    let mut all = pieces(a, &fa, 0);
    all.extend(pieces(b, &fb, 1));
    let corners: Vec<P> = [a, b].iter().flat_map(|g| match g {
        Geom::Poly { pts, .. } => pts.clone(),
        Geom::Circle { .. } => vec![],
    }).collect();

    let rule = match how {
        How::Union => OverlayRule::Union,
        How::Subtract => OverlayRule::Difference,
        How::Intersect => OverlayRule::Intersect,
    };
    let shapes = fa.overlay_as::<i64>(&fb, rule, FillRule::NonZero);

    let on_piece = |x: P, (p, q, _): (P, P, usize)| {
        let d = sub(q, p);
        let t = (dot(sub(x, p), d) / dot(d, d)).clamp(0.0, 1.0);
        dist(x, add(p, mul(d, t))) < eps * 10.0
    };
    let mut out = vec![];
    for shape in shapes {
        for (k, ring) in shape.into_iter().enumerate() {
            let mut ring: Vec<P> = ring;
            ring.dedup_by(|x, y| dist(*x, *y) < eps);
            while ring.len() > 1 && dist(ring[0], ring[ring.len() - 1]) < eps {
                ring.pop();
            }
            if ring.len() < 3 {
                continue;
            }
            // Counter-clockwise, outlines and holes alike (holes become shapes of their own).
            let area: f64 = (0..ring.len()).map(|i| cross(ring[i], ring[(i + 1) % ring.len()])).sum();
            if area < 0.0 {
                ring.reverse();
            }
            let n = ring.len();
            // The carrier of each segment of the ring: one that a piece under both its ends has.
            // (A segment can span several pieces when edges of both shapes lie in line.)
            let under = |x: P| -> Vec<usize> {
                let mut v: Vec<usize> = all.iter().filter(|pc| on_piece(x, **pc)).map(|pc| edges[pc.2].carrier).collect();
                v.sort_unstable();
                v.dedup();
                v
            };
            let at: Vec<Vec<usize>> = ring.iter().map(|x| under(*x)).collect();
            let mut car = Vec::with_capacity(n);
            for i in 0..n {
                let mid = under(mul(add(ring[i], ring[(i + 1) % n]), 0.5));
                let c = at[i].iter().find(|c| at[(i + 1) % n].contains(c) && mid.contains(c));
                car.push(*c.ok_or("These shapes could not be combined (an edge of the result was not on either shape).")?);
            }
            // Start the ring where the carrier changes, so no run wraps around the start.
            let Some(s0) = (0..n).find(|&i| car[i] != car[(i + n - 1) % n]) else {
                // The whole ring is on one carrier: a circle.
                let Carrier::Circle { c, r } = carriers[car[0]] else { return Err("These shapes could not be combined.".into()) };
                let from = edges.iter().filter(|e| e.carrier == car[0]).map(|e| (e.src, e.edge)).collect();
                out.push(Piece { geom: Geom::Circle { c, r }, inner: k > 0, from: vec![from] });
                continue;
            };
            ring.rotate_left(s0);
            car.rotate_left(s0);
            // Runs of segments on one carrier become one edge.
            let mut runs: Vec<(usize, usize, usize)> = vec![]; // (carrier, first point, last point)
            let mut i = 0;
            while i < n {
                let mut j = i;
                while j + 1 < n && car[j + 1] == car[i] {
                    j += 1;
                }
                runs.push((car[i], i, (j + 1) % n));
                i = j + 1;
            }
            // Corners: an input corner if the point is one, else where the exact curves meet.
            let nr = runs.len();
            let mut cpts: Vec<P> = Vec::with_capacity(nr);
            for r in 0..nr {
                let (ck, first, _) = runs[r];
                let x = ring[first];
                let prev = runs[(r + nr - 1) % nr].0;
                let p = match corners.iter().find(|c| dist(**c, x) < tol) {
                    Some(c) => *c,
                    None => meet(carriers[prev], carriers[ck], x).filter(|m| dist(*m, x) < seg).unwrap_or(x),
                };
                cpts.push(p);
            }
            let mut pts = vec![];
            let mut bulge = vec![];
            let mut from = vec![];
            for r in 0..nr {
                let (ck, first, last) = runs[r];
                let (p, q) = (cpts[r], cpts[(r + 1) % nr]);
                let b = match carriers[ck] {
                    Carrier::Line { .. } => 0.0,
                    Carrier::Circle { c, .. } => {
                        // The angle swept along the run, summed piece by piece.
                        let mut path = vec![p];
                        let mut t = first + 1;
                        while t % n != last {
                            path.push(ring[t % n]);
                            t += 1;
                        }
                        path.push(q);
                        let ang = |x: P| (x[1] - c[1]).atan2(x[0] - c[0]);
                        let sweep: f64 = path.windows(2).map(|w| wrap(ang(w[1]) - ang(w[0]))).sum();
                        (sweep / 4.0).tan()
                    }
                };
                // The input edges under this edge: those holding points along it.
                let probe: Vec<P> = {
                    let mut v = vec![];
                    let mut t = first;
                    loop {
                        let (x, y) = (ring[t % n], ring[(t + 1) % n]);
                        v.push(mul(add(x, y), 0.5));
                        t += 1;
                        if t % n == last {
                            break;
                        }
                    }
                    v
                };
                let mut src: Vec<(usize, u32)> = edges.iter().filter(|e| e.carrier == ck && probe.iter().any(|x| e.holds(*x, tol))).map(|e| (e.src, e.edge)).collect();
                src.dedup();
                pts.push(p);
                bulge.push(b);
                from.push(src);
            }
            // An arc turns at most half a circle (longer ones are split in two), and a polygon needs
            // three corners (a two-edged ring, a lens, has its longest edge split).
            let mut i = 0;
            while i < pts.len() {
                if bulge[i].abs() > 1.0 + 1e-9 {
                    split(&mut pts, &mut bulge, &mut from, i);
                } else {
                    bulge[i] = bulge[i].clamp(-1.0, 1.0);
                    i += 1;
                }
            }
            while pts.len() < 3 {
                let m = pts.len();
                let i = (0..m).max_by(|x, y| dist(pts[*x], pts[(*x + 1) % m]).total_cmp(&dist(pts[*y], pts[(*y + 1) % m]))).unwrap_or(0);
                split(&mut pts, &mut bulge, &mut from, i);
            }
            // Start where the first shape started (its first corner still there), so its edges keep
            // their numbers where they can.
            if let Geom::Poly { pts: first, .. } = a {
                if let Some(j) = first.iter().find_map(|c| pts.iter().position(|p| dist(*p, *c) < tol)) {
                    pts.rotate_left(j);
                    bulge.rotate_left(j);
                    from.rotate_left(j);
                }
            }
            out.push(Piece { geom: Geom::Poly { pts, bulge }, inner: k > 0, from });
        }
    }
    if out.iter().all(|p| p.inner) {
        return Err(match how {
            How::Intersect => "These shapes do not overlap, so they have nothing in common.",
            How::Subtract => "Nothing is left: the second shape covers the whole of the first.",
            How::Union => "These shapes could not be combined.",
        }
        .into());
    }
    Ok(out)
}

/// Split edge `i` at its middle (on the arc, for an arc); both halves come from the same edges.
fn split(pts: &mut Vec<P>, bulge: &mut Vec<f64>, from: &mut Vec<Vec<(usize, u32)>>, i: usize) {
    let m = pts.len();
    let (p, q, b) = (pts[i], pts[(i + 1) % m], bulge[i]);
    let (mid, half) = if b == 0.0 {
        (mul(add(p, q), 0.5), 0.0)
    } else {
        let (c, r, a0, th) = arc(p, q, b);
        let t = a0 + th / 2.0;
        ([c[0] + r * t.cos(), c[1] + r * t.sin()], (th / 8.0).tan())
    };
    pts.insert(i + 1, mid);
    bulge[i] = half;
    bulge.insert(i + 1, half);
    from.insert(i + 1, from[i].clone());
}

/// The outline moved out by `d` (in by a negative `d`), each edge parallel to its own: corners
/// stay sharp, arcs keep their centres. Arcs must meet their neighbours smoothly.
pub fn offset(g: &Geom, d: f64) -> Result<Geom, String> {
    if !(d.is_finite() && d != 0.0) {
        return Err("Offset needs a distance (positive outwards, negative inwards).".into());
    }
    match g {
        Geom::Circle { c, r } => {
            if r + d <= 0.0 {
                return Err("That offset is bigger than the circle's radius.".into());
            }
            Ok(Geom::Circle { c: *c, r: r + d })
        }
        Geom::Poly { pts, bulge } => {
            let n = pts.len();
            // Outwards is to the right of the edges of a counter-clockwise outline.
            let s = if g.area() >= 0.0 { d } else { -d };
            // Each edge's offset: a line (point, direction) or a circle (centre, radius).
            let mut off: Vec<Carrier> = vec![];
            for i in 0..n {
                let (p, q) = (pts[i], pts[(i + 1) % n]);
                if bulge[i] == 0.0 {
                    let t = mul(sub(q, p), 1.0 / dist(p, q));
                    let out = [t[1], -t[0]];
                    off.push(Carrier::Line { p: add(p, mul(out, s)), d: t });
                } else {
                    let (c, r, _, th) = arc(p, q, bulge[i]);
                    // A left-turning arc has its centre on the left: outwards grows it.
                    let r2 = if th > 0.0 { r + s } else { r - s };
                    if r2 <= 1e-12 * r {
                        return Err("That offset is bigger than the radius of an arc.".into());
                    }
                    off.push(Carrier::Circle { c, r: r2 });
                }
            }
            let mut np = vec![];
            for i in 0..n {
                let h = (i + n - 1) % n;
                let x = pts[i];
                let smooth = |k: usize, at_end: bool| -> P {
                    // Where edge k's offset passes the corner: the corner moved along the normal.
                    let (p, q) = (pts[k], pts[(k + 1) % n]);
                    let e = if at_end { q } else { p };
                    match off[k] {
                        Carrier::Line { p: lp, d: dd } => add(lp, mul(dd, dot(sub(e, p), dd))),
                        Carrier::Circle { c, r } => add(c, mul(sub(e, c), r / dist(e, c))),
                    }
                };
                let (ea, eb) = (smooth(h, true), smooth(i, false));
                let p = if dist(ea, eb) < 1e-9 * (1.0 + len(x)) {
                    ea
                } else if bulge[h] == 0.0 && bulge[i] == 0.0 {
                    meet(off[h], off[i], x).ok_or("Two edges of this outline are in line; remove the corner between them first.")?
                } else {
                    return Err("Offset works on outlines whose arcs meet their neighbours smoothly (a rounded corner), not at a sharp corner.".into());
                };
                np.push(p);
            }
            // Each edge keeps its direction and arc; one that turned round has been used up.
            let mut nb = vec![];
            for i in 0..n {
                let (p, q) = (pts[i], pts[(i + 1) % n]);
                let (p2, q2) = (np[i], np[(i + 1) % n]);
                if dist(p2, q2) < 1e-9 * span_of(pts) {
                    return Err("That offset uses up an edge of the outline; try a smaller one.".into());
                }
                if bulge[i] == 0.0 {
                    if dot(sub(q, p), sub(q2, p2)) <= 0.0 {
                        return Err("That offset uses up an edge of the outline; try a smaller one.".into());
                    }
                    nb.push(0.0);
                } else {
                    let (_, _, _, th) = arc(p, q, bulge[i]);
                    nb.push((th / 4.0).tan());
                }
            }
            let out = Geom::Poly { pts: np, bulge: nb };
            if out.area().signum() != g.area().signum() || crosses_itself(&out) {
                return Err("That offset folds the outline over itself; try a smaller one.".into());
            }
            Ok(out)
        }
    }
}

fn span_of(pts: &[P]) -> f64 {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in pts {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-12)
}

/// Does the outline cross itself? (Checked on its straight pieces.)
pub fn crosses_itself(g: &Geom) -> bool {
    let (pts, _) = g.outline(span_of(&g.outline(1.0, 8).0) / 200.0, 64);
    let n = pts.len();
    let hit = |a: P, b: P, c: P, d: P| {
        let (r, s) = (sub(b, a), sub(d, c));
        let den = cross(r, s);
        if den.abs() < 1e-30 {
            return false;
        }
        let t = cross(sub(c, a), s) / den;
        let u = cross(sub(c, a), r) / den;
        t > 1e-9 && t < 1.0 - 1e-9 && u > 1e-9 && u < 1.0 - 1e-9
    };
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if hit(pts[i], pts[(i + 1) % n], pts[j], pts[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

/// A ready-made shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Make {
    /// A slot: the two centres of its round ends and its width.
    Slot { a: P, b: P, w: f64 },
    /// A rectangle between two corners with its corners rounded to radius `r`.
    Rounded { a: P, b: P, r: f64 },
    /// A regular polygon of `n` sides: its centre, the distance to each corner, and the angle of
    /// its first corner (radians).
    Polygon { c: P, r: f64, n: u32, turn: f64 },
}

pub fn make(k: &Make) -> Result<Geom, String> {
    let fin = |v: &[f64]| v.iter().all(|x| x.is_finite());
    match *k {
        Make::Slot { a, b, w } => {
            if !(fin(&[a[0], a[1], b[0], b[1], w]) && w > 0.0) {
                return Err("A slot needs its two end centres and a width greater than zero.".into());
            }
            let r = w / 2.0;
            let l = dist(a, b);
            if l < 1e-9 * r {
                return Ok(Geom::Circle { c: a, r });
            }
            let u = mul(sub(b, a), 1.0 / l);
            let nrm = [-u[1], u[0]];
            Ok(Geom::Poly { pts: vec![sub(a, mul(nrm, r)), sub(b, mul(nrm, r)), add(b, mul(nrm, r)), add(a, mul(nrm, r))], bulge: vec![0.0, 1.0, 0.0, 1.0] })
        }
        Make::Rounded { a, b, r } => {
            let (x0, x1, y0, y1) = (a[0].min(b[0]), a[0].max(b[0]), a[1].min(b[1]), a[1].max(b[1]));
            let (w, h) = (x1 - x0, y1 - y0);
            if !(fin(&[x0, x1, y0, y1, r]) && w > 0.0 && h > 0.0 && r >= 0.0) {
                return Err("A rounded rectangle needs two corners apart and a radius of zero or more.".into());
            }
            if r > w.min(h) / 2.0 * (1.0 + 1e-9) {
                return Err(format!("The radius can be at most half the shorter side ({:.4} m).", w.min(h) / 2.0));
            }
            let r = r.min(w.min(h) / 2.0);
            if r == 0.0 {
                return Ok(Geom::rect(x0, y0, x1, y1));
            }
            // Rounds that use up the short sides: a slot (a circle when square).
            if (2.0 * r - h).abs() < 1e-9 * h {
                return make(&Make::Slot { a: [x0 + r, y0 + r], b: [x1 - r, y0 + r], w: h });
            }
            if (2.0 * r - w).abs() < 1e-9 * w {
                return make(&Make::Slot { a: [x0 + r, y0 + r], b: [x0 + r, y1 - r], w });
            }
            let q = (PI / 8.0).tan();
            let pts = vec![[x0 + r, y0], [x1 - r, y0], [x1, y0 + r], [x1, y1 - r], [x1 - r, y1], [x0 + r, y1], [x0, y1 - r], [x0, y0 + r]];
            let bulge = vec![0.0, q, 0.0, q, 0.0, q, 0.0, q];
            Ok(Geom::Poly { pts, bulge })
        }
        Make::Polygon { c, r, n, turn } => {
            if !(fin(&[c[0], c[1], r, turn]) && r > 0.0) || !(3..=1000).contains(&n) {
                return Err("A regular polygon needs 3 to 1000 sides and a size greater than zero.".into());
            }
            let pts = (0..n).map(|i| {
                let t = turn + TAU * i as f64 / n as f64;
                [c[0] + r * t.cos(), c[1] + r * t.sin()]
            });
            Ok(Geom::Poly { pts: pts.collect(), bulge: vec![0.0; n as usize] })
        }
    }
}

/// Turn a shape through `ang` (radians, counter-clockwise) about `o`.
pub fn turn(g: &mut Geom, o: P, ang: f64) {
    let (s, c) = ang.sin_cos();
    for p in g.points_mut() {
        let v = sub(*p, o);
        *p = [o[0] + c * v[0] - s * v[1], o[1] + s * v[0] + c * v[1]];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    #[test]
    fn rounding_a_square_corner_takes_off_the_hand_area() {
        let g = Geom::rect(0.0, 0.0, 1.0, 1.0);
        let r = 0.2;
        let f = corner(&g, 2, r, true).unwrap();
        // A square corner rounded to r loses r² − πr²/4.
        assert!(close(f.area(), 1.0 - (r * r - PI * r * r / 4.0), 1e-12), "{}", f.area());
        assert_eq!(f.edges(), 5);
        let c = corner(&g, 0, 0.1, false).unwrap();
        assert!(close(c.area(), 1.0 - 0.005, 1e-12));
        // All four corners, highest first (each adds a corner after itself).
        let mut h = g.clone();
        for i in (0..4).rev() {
            h = corner(&h, i, 0.25, true).unwrap();
        }
        assert!(close(h.area(), 1.0 - 4.0 * (0.0625 - PI * 0.0625 / 4.0), 1e-12));
        // A corner of a clockwise outline, and a re-entrant (inside) corner of an L.
        let cw = Geom::Poly { pts: vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]], bulge: vec![0.0; 4] };
        assert!(close(corner(&cw, 1, r, true).unwrap().area(), -(1.0 - (r * r - PI * r * r / 4.0)), 1e-12));
        let l = Geom::Poly { pts: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.0, 2.0], [0.0, 2.0]], bulge: vec![0.0; 6] };
        assert!(close(corner(&l, 3, r, true).unwrap().area(), 3.0 + (r * r - PI * r * r / 4.0), 1e-12));
        assert!(corner(&g, 1, 1.2, true).is_err() && corner(&f, 2, 0.1, true).is_err() && corner(&g, 9, 0.1, true).is_err());
    }

    #[test]
    fn combining_keeps_arcs_and_matches_hand_areas() {
        let sq = Geom::rect(0.0, 0.0, 2.0, 2.0);
        let hole = Geom::Circle { c: [1.0, 1.0], r: 0.5 };
        // A hole inside: the outline stays, the circle comes back as a hole.
        let p = combine(&sq, &hole, How::Subtract).unwrap();
        assert_eq!(p.len(), 2);
        let outer = p.iter().find(|x| !x.inner).unwrap();
        assert!(close(outer.geom.area(), 4.0, 1e-9) && outer.geom.edges() == 4);
        assert_eq!(outer.geom, sq, "the outline comes back as it was, edges in the same order");
        assert!(outer.from.iter().enumerate().all(|(e, f)| f == &vec![(0, e as u32)]));
        assert!(matches!(p.iter().find(|x| x.inner).unwrap().geom, Geom::Circle { r, .. } if close(r, 0.5, 1e-9)));
        // A circle over a corner: three-quarters of the square's area… a quarter of the disc goes.
        let bite = combine(&sq, &Geom::Circle { c: [2.0, 2.0], r: 0.5 }, How::Subtract).unwrap();
        assert_eq!(bite.len(), 1);
        assert!(close(bite[0].geom.area(), 4.0 - PI * 0.25 / 4.0, 1e-9), "{}", bite[0].geom.area());
        assert_eq!(bite[0].geom.edges(), 5, "four straight edges and one arc");
        let joined = combine(&sq, &Geom::Circle { c: [2.0, 2.0], r: 0.5 }, How::Union).unwrap();
        assert!(close(joined[0].geom.area(), 4.0 + 3.0 * PI * 0.25 / 4.0, 1e-9));
        let common = combine(&sq, &Geom::Circle { c: [2.0, 2.0], r: 0.5 }, How::Intersect).unwrap();
        assert!(close(common[0].geom.area(), PI * 0.25 / 4.0, 1e-9));
        assert_eq!(common[0].geom.edges(), 3);
        // Two overlapping circles: the lens, by hand 2r²acos(d/2r) − (d/2)√(4r² − d²).
        let (r, d) = (1.0f64, 1.0f64);
        let lens = combine(&Geom::Circle { c: [0.0, 0.0], r }, &Geom::Circle { c: [d, 0.0], r }, How::Intersect).unwrap();
        let want = 2.0 * r * r * (d / (2.0 * r)).acos() - d / 2.0 * (4.0 * r * r - d * d).sqrt();
        assert!(close(lens[0].geom.area(), want, 1e-9), "{} {}", lens[0].geom.area(), want);
        // Side by side: one rectangle of four edges, each traced to the edges under it.
        let u = combine(&Geom::rect(0.0, 0.0, 1.0, 1.0), &Geom::rect(1.0, 0.0, 2.0, 1.0), How::Union).unwrap();
        assert_eq!(u.len(), 1);
        assert!(close(u[0].geom.area(), 2.0, 1e-12) && u[0].geom.edges() == 4);
        let bottom = u[0].from.iter().find(|f| f.len() == 2).expect("the bottom lies on both bottoms");
        assert!(bottom.contains(&(0, 0)) && bottom.contains(&(1, 0)));
        // Apart: union gives two shapes; intersection nothing; subtracting all leaves nothing.
        assert_eq!(combine(&sq, &Geom::rect(5.0, 5.0, 6.0, 6.0), How::Union).unwrap().len(), 2);
        assert!(combine(&sq, &Geom::rect(5.0, 5.0, 6.0, 6.0), How::Intersect).is_err());
        assert!(combine(&hole, &sq, How::Subtract).is_err());
        // A slot cut across a plate: arcs stay arcs.
        let slot = make(&Make::Slot { a: [0.5, 1.0], b: [3.0, 1.0], w: 0.4 }).unwrap();
        let cut = combine(&sq, &slot, How::Subtract).unwrap();
        let want = 4.0 - (1.5 * 0.4 + PI * 0.04 / 2.0);
        let got: f64 = cut.iter().map(|p| p.geom.area()).sum();
        assert!(close(got, want, 1e-9), "{got} {want}");
    }

    #[test]
    fn offsets_and_ready_made_shapes() {
        let sq = Geom::rect(0.0, 0.0, 2.0, 1.0);
        assert!(close(offset(&sq, 0.1).unwrap().area(), 2.2 * 1.2, 1e-12));
        assert!(close(offset(&sq, -0.1).unwrap().area(), 1.8 * 0.8, 1e-12));
        assert!(offset(&sq, -0.6).is_err());
        let rr = make(&Make::Rounded { a: [0.0, 0.0], b: [2.0, 1.0], r: 0.2 }).unwrap();
        assert!(close(rr.area(), 2.0 - 4.0 * (0.04 - PI * 0.04 / 4.0), 1e-12));
        // Offsetting a rounded rectangle in by t: the rounds shrink with it.
        let t = 0.1;
        let want = 1.8 * 0.8 - 4.0 * (0.01 - PI * 0.01 / 4.0);
        assert!(close(offset(&rr, -t).unwrap().area(), want, 1e-12));
        assert!(offset(&rr, -0.25).is_err(), "the rounds are used up");
        assert!(matches!(make(&Make::Rounded { a: [0.0, 0.0], b: [1.0, 1.0], r: 0.5 }).unwrap(), Geom::Circle { .. }));
        assert_eq!(make(&Make::Rounded { a: [0.0, 0.0], b: [2.0, 1.0], r: 0.5 }).unwrap().edges(), 4, "a stadium");
        let slot = make(&Make::Slot { a: [0.0, 0.0], b: [1.0, 0.0], w: 0.2 }).unwrap();
        assert!(close(slot.area(), 0.2 + PI * 0.01, 1e-12));
        let hex = make(&Make::Polygon { c: [0.0, 0.0], r: 1.0, n: 6, turn: 0.0 }).unwrap();
        assert!(close(hex.area(), 1.5 * 3f64.sqrt(), 1e-12));
        assert!(make(&Make::Polygon { c: [0.0, 0.0], r: 1.0, n: 2, turn: 0.0 }).is_err());
        let mut g = Geom::rect(1.0, 0.0, 2.0, 1.0);
        turn(&mut g, [0.0, 0.0], PI / 2.0);
        let Geom::Poly { pts, .. } = &g else { panic!() };
        assert!(dist(pts[0], [0.0, 1.0]) < 1e-12);
        assert!(crosses_itself(&Geom::Poly { pts: vec![[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]], bulge: vec![0.0; 4] }));
    }
}
