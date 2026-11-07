//! Contact between faces of meshed solids (0.8.0): node to surface, by penalty (Wriggers,
//! *Computational Contact Mechanics*, ch. 5 and 6; Bathe §6.7). Each node of the contact's face
//! `a` is projected onto the nearest facet of face `b`; its gap, measured along that facet's
//! outward normal, may close but not go below zero: where it would, a stiff spring pushes it out
//! (and the facet's nodes back, shared by the facet's shape functions). Friction is Coulomb's,
//! regularised by a stiff tangential spring while the node sticks; once the spring's force would
//! pass the friction coefficient times the normal force, the node slides and the friction force
//! is that limit, against the sliding. Which nodes touch, stick or slide is found by repeating
//! the solve until it stops changing (an active set). Small movements: the gaps and normals are
//! the unloaded ones, so the load is put on in one go.

use super::tet::{facet_points, TetMesh, FACES};
use crate::engine::sparse::System;
use crate::model::{Model, Target};
use serde::{Deserialize, Serialize};

/// One node of a contact's face `a`, where it meets face `b`.
pub(crate) struct Slave {
    /// The contact it belongs to (its index in `Model::contacts`).
    pub pair: usize,
    pub q: usize,
    /// The node's share of its face's area (m²), for the pressure.
    pub area: f64,
    /// The nodes of the facet of `b` it meets, and their shape functions there.
    pub master: Vec<(usize, f64)>,
    /// The facet's outward unit normal and two unit tangents.
    pub n: [f64; 3],
    pub t: [[f64; 3]; 2],
    /// The gap before loading (m): positive open, negative overlapping.
    pub g0: f64,
    pub mu: f64,
}

/// What a contact carried, for the results.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContactOut {
    pub id: u32,
    /// The whole force face `b` puts on face `a` (N), pressing and friction.
    pub force: [f64; 3],
    /// Of it, the pressing part (N), and the friction part's size (N).
    pub normal: f64,
    pub friction: f64,
    /// Nodes of face `a` touching, and of those sliding; all its nodes over face `b`.
    pub touching: usize,
    pub sliding: usize,
    pub nodes: usize,
    /// The largest contact pressure (Pa) and overlap (m, how far the penalty lets a node in).
    pub max_pressure: f64,
    pub max_overlap: f64,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt().max(1e-300);
    a.map(|v| v / l)
}

/// The slave nodes of every contact of the model, each with the facet it meets.
pub(crate) fn slaves(m: &Model, ms: &TetMesh) -> Result<Vec<Slave>, String> {
    let face = |t: &Target| match t {
        Target::Face { face } => ms.faces.get(*face as usize).ok_or("A contact is on a face the mesh no longer has.".to_string()),
        _ => Err("Contact is between faces of a meshed solid.".to_string()),
    };
    let o = if ms.order == 2 { 2 } else { 1 };
    let (lo, hi) = ms.nodes.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(l, h), p| (std::array::from_fn(|k| l[k].min(p[k])), std::array::from_fn(|k| h[k].max(p[k]))));
    let reach = 0.1 * dot(sub(hi, lo), sub(hi, lo)).sqrt();
    let mut out = vec![];
    for (pi, c) in m.contacts.iter().enumerate() {
        let (fa, fb) = (face(&c.a)?, face(&c.b)?);
        // Each node's share of face a's area (lumped: by the size of its shape function).
        let mut area: std::collections::BTreeMap<usize, f64> = Default::default();
        for &(e, k) in &fa.facets {
            for (_, nv, ns) in facet_points(ms, e as usize, k as usize) {
                let da = dot(nv, nv).sqrt();
                let tot: f64 = ns.iter().map(|x| x.1.abs()).sum::<f64>().max(1e-300);
                for (q, s) in ns {
                    *area.entry(q).or_default() += s.abs() / tot * da;
                }
            }
        }
        // Face b's facets: their nodes, corners and outward normals.
        let facets: Vec<(Vec<u32>, [[f64; 3]; 3], [f64; 3])> = fb
            .facets
            .iter()
            .map(|&(e, k)| {
                let f = ms.facet(e as usize, k as usize);
                let cn = [0, 1, 2].map(|i| ms.nodes[f[i] as usize]);
                let el = &ms.tets[e as usize];
                let other = (0..4).find(|i| !FACES[k as usize].contains(i)).unwrap_or(0);
                let mut n = unit(cross(sub(cn[1], cn[0]), sub(cn[2], cn[0])));
                if dot(n, sub(ms.nodes[el[other] as usize], cn[0])) > 0.0 {
                    n = n.map(|v| -v);
                }
                (f, cn, n)
            })
            .collect();
        for (&q, &a) in &area {
            let x = ms.nodes[q];
            // The nearest facet of b whose triangle the node lies over.
            let mut best: Option<(f64, usize, f64, f64)> = None;
            for (fi, (_, cn, n)) in facets.iter().enumerate() {
                let (e1, e2, d) = (sub(cn[1], cn[0]), sub(cn[2], cn[0]), sub(x, cn[0]));
                let (a11, a12, a22, b1, b2) = (dot(e1, e1), dot(e1, e2), dot(e2, e2), dot(e1, d), dot(e2, d));
                let det = a11 * a22 - a12 * a12;
                if det <= 0.0 {
                    continue;
                }
                let (l1, l2) = ((b1 * a22 - b2 * a12) / det, (a11 * b2 - a12 * b1) / det);
                let tol = -1e-6;
                if l1 < tol || l2 < tol || 1.0 - l1 - l2 < tol {
                    continue;
                }
                let g = dot(d, *n);
                if g.abs() > reach {
                    continue;
                }
                if best.is_none_or(|b| g.abs() < b.0) {
                    best = Some((g.abs(), fi, l1.clamp(0.0, 1.0), l2.clamp(0.0, 1.0)));
                }
            }
            let Some((_, fi, l1, l2)) = best else { continue };
            let (f, cn, n) = &facets[fi];
            if f.contains(&(q as u32)) {
                continue; // the same node: faces that share it are joined already
            }
            let (sn, _) = super::solid::tri_n(o, l1, l2);
            let p: [f64; 3] = std::array::from_fn(|k| f.iter().zip(&sn).map(|(&j, s)| s * ms.nodes[j as usize][k]).sum());
            let t1 = unit(sub(sub(cn[1], cn[0]), n.map(|v| v * dot(sub(cn[1], cn[0]), *n))));
            out.push(Slave { pair: pi, q, area: a, master: f.iter().zip(sn).map(|(&j, s)| (j as usize, s)).collect(), n: *n, t: [t1, cross(*n, t1)], g0: dot(sub(x, p), *n), mu: c.friction });
        }
    }
    Ok(out)
}

/// The node and its facet's nodes, each with its weight in the relative movement (slave less
/// master).
fn terms(s: &Slave) -> Vec<(usize, f64)> {
    std::iter::once((s.q, 1.0)).chain(s.master.iter().map(|&(j, w)| (j, -w))).collect()
}

/// The relative movement of the node from its facet along `v`.
fn along(s: &Slave, u: &[f64], v: [f64; 3]) -> f64 {
    terms(s).iter().map(|&(j, w)| w * (0..3).map(|c| u[3 * j + c] * v[c]).sum::<f64>()).sum()
}

/// A spring `k` on the relative movement along `v` (the lower triangle).
fn spring(sys: &mut System, s: &Slave, v: [f64; 3], k: f64) {
    let tm = terms(s);
    for &(p, wp) in &tm {
        for &(r, wr) in &tm {
            for a in 0..3 {
                for b in 0..3 {
                    let (i, j) = (3 * p + a, 3 * r + b);
                    if i >= j {
                        sys.add(i, j, k * wp * wr * v[a] * v[b]);
                    }
                }
            }
        }
    }
}

/// A force `f` on the node and its reaction on the facet's nodes, into `out`.
fn push(out: &mut [f64], s: &Slave, f: [f64; 3]) {
    for (j, w) in terms(s) {
        for c in 0..3 {
            out[3 * j + c] += w * f[c];
        }
    }
}

/// Where each slave is: apart, touching and sticking, or touching and sliding.
#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Open,
    Stick,
    Slide,
}

/// The solve with contact: the linear system `sys` (supports `held`, moved values `presc`) with
/// the contact springs and forces, repeated until which nodes touch, stick and slide settles.
/// Returns the movements, the solver's statistics, the contact forces on the nodes (to take out of
/// the reactions), what each contact carried and the contact pressure at each node.
#[allow(clippy::type_complexity)]
pub(crate) fn solve(m: &Model, ms: &TetMesh, sys: &System, held: &[bool], presc: &[f64], warnings: &mut Vec<String>) -> Result<(Vec<f64>, crate::engine::sparse::Stats, Vec<f64>, Vec<ContactOut>, Vec<f64>), String> {
    let sl = slaves(m, ms)?;
    if sl.is_empty() {
        warnings.push("No node of a contact's first face lies over its second face, so the contact does nothing: check the faces chosen.".into());
    }
    // The springs: a thousand times the mean stiffness on a node's movement, so a node's overlap is
    // about a thousandth of how far the load moves it.
    let mut dg = vec![0.0; sys.n];
    for &(i, j, v) in &sys.k {
        if i == j {
            dg[i] += v;
        }
    }
    let eps = 1e3 * dg.iter().sum::<f64>() / dg.len().max(1) as f64;
    let scale = ms.nodes.iter().fold(0.0f64, |a, p| a.max(p[0].abs()).max(p[1].abs()).max(p[2].abs())).max(1e-9);
    // Each node: apart or touching; touching with friction it has a tangential spring whose
    // rest point (its slip so far, `sp`) moves when the spring would pull harder than μ times the
    // pressing force (a return mapping, as in plasticity: Wriggers ch. 5.2; Simo and Hughes
    // ch. 1): so a sliding node keeps a stiffness and a solid held only by friction stays held.
    let mut state: Vec<State> = sl.iter().map(|s| if s.g0 <= 1e-9 * scale { State::Stick } else { State::Open }).collect();
    let mut sp: Vec<[f64; 3]> = vec![[0.0; 3]; sl.len()];
    // The sliding nodes' spring: soft, so the friction it gives hardly changes as the node moves
    // in a round (no overshoot), yet the node is never free.
    let ks = 1e-3 * eps;
    let tang = |s: &Slave, u: &[f64]| -> [f64; 3] { std::array::from_fn(|c| s.t[0][c] * along(s, u, s.t[0]) + s.t[1][c] * along(s, u, s.t[1])) };
    let mut last = None;
    // Friction comes in a second phase: first where the faces press, without it (phase 0); then
    // each node that slid starts sliding at μ times that pressure along its slide. If the solids
    // cannot be held without friction, friction is on from the start, every node sticking.
    let any_mu = sl.iter().any(|s| s.mu > 0.0);
    let mut phase = if any_mu { 0 } else { 1 };
    for it in 0..120 {
        let mu_of = |s: &Slave| if phase == 0 { 0.0 } else { s.mu };
        let mut s2 = System { n: sys.n, k: sys.k.clone(), f: sys.f.clone(), half: sys.half, general: sys.general };
        for (i, s) in sl.iter().enumerate() {
            if state[i] == State::Open {
                continue;
            }
            spring(&mut s2, s, s.n, eps);
            push(&mut s2.f, s, s.n.map(|v| -eps * s.g0 * v));
            if mu_of(s) > 0.0 {
                // Sticking, the stiff spring; sliding, a soft one set to pull μ pn (see below).
                let k = if state[i] == State::Slide { ks } else { eps };
                for t in s.t {
                    spring(&mut s2, s, t, k);
                }
                // The spring pulls towards its rest point: f = −k (s_t − sp) = −k s_t + k sp.
                push(&mut s2.f, s, sp[i].map(|v| k * v));
            }
        }
        let (u, st) = match s2.solve(held, presc) {
            Ok(x) => x,
            Err(_) if phase == 0 => {
                phase = 1;
                continue;
            }
            Err(e) => return Err(format!("{e} With contact, each solid must be held on its own or pressed onto another: a solid held only by contact without friction can slide away.")),
        };
        let mut next = state.clone();
        let mut nsp = sp.clone();
        let mut fnode = vec![0.0; 3 * ms.nodes.len()];
        let mut fn_ = vec![0.0; sl.len()];
        let mut fs = vec![[0.0; 3]; sl.len()];
        // A dead band against flip-flopping at the edge of a contact: a hundredth of the deepest
        // overlap; a node opens past it, and closes past it the other way.
        let deepest = sl.iter().enumerate().filter(|(i, _)| state[*i] != State::Open).map(|(_, s)| -(s.g0 + along(s, &u, s.n))).fold(0.0f64, f64::max);
        let band = 1e-2 * deepest;
        for (i, s) in sl.iter().enumerate() {
            let g = s.g0 + along(s, &u, s.n);
            let stv = tang(s, &u);
            if state[i] == State::Open {
                // A node closing starts with its spring at rest where it touched.
                if g < -band || (deepest == 0.0 && g < 0.0) {
                    next[i] = State::Stick;
                    nsp[i] = stv;
                }
                continue;
            }
            if g > band {
                next[i] = State::Open;
                continue;
            }
            // Inside the band a node may pull a very little; it is counted as it is.
            let pn = -eps * g;
            fn_[i] = pn;
            let mut f = s.n.map(|v| pn * v);
            if mu_of(s) > 0.0 {
                let el = sub(stv, sp[i]);
                let trial = el.map(|v| -eps * v);
                let tn = dot(trial, trial).sqrt();
                let lim = s.mu * pn.max(0.0);
                if state[i] == State::Stick && tn <= lim * (1.0 + 1e-9) {
                    next[i] = State::Stick;
                    for c in 0..3 {
                        f[c] += trial[c];
                    }
                } else {
                    // Slides: the rest point moves so the spring pulls exactly μ pn, against the
                    // way it slides.
                    next[i] = State::Slide;
                    // The load goes on at once, so it slides along its whole slide (the way it has
                    // moved along the face), or along this round's pull if it has not moved.
                    let w = if dot(stv, stv).sqrt() > 1e-12 * scale { stv } else { el };
                    let d = w.map(|v| v / dot(w, w).sqrt().max(1e-300));
                    nsp[i] = std::array::from_fn(|c| stv[c] - lim / ks * d[c]);
                    for c in 0..3 {
                        f[c] -= lim * d[c];
                    }
                }
            } else if s.mu <= 0.0 {
                next[i] = State::Slide;
            }
            push(&mut fnode, s, f);
            fs[i] = f;
        }
        let changed = next.iter().zip(&state).any(|(a, b)| (*a == State::Open) != (*b == State::Open));
        let moved = nsp.iter().zip(&sp).map(|(a, b)| dot(sub(*a, *b), sub(*a, *b)).sqrt()).fold(0.0, f64::max);
        let big = sl.iter().zip(&nsp).map(|(_, a)| dot(*a, *a).sqrt()).fold(0.0, f64::max);
        let tiny = 1e-9 * u.iter().fold(0.0f64, |a, v| a.max(v.abs())).max(1e-300);
        state = next;
        sp = nsp;
        last = Some((u, st, fnode, fn_, fs));
        if phase == 0 && !changed && it > 0 {
            // Pressing settled without friction: friction on, each touching node sliding at μ pn
            // along its slide (sticking if it has not moved).
            let (u0, _, _, fn0, _) = last.as_ref().ok_or("Contact could not be solved.")?;
            for (i, s) in sl.iter().enumerate() {
                if state[i] == State::Open || s.mu <= 0.0 {
                    continue;
                }
                let stv = tang(s, u0);
                let l = dot(stv, stv).sqrt();
                if l > 1e-12 * scale && fn0[i] > 0.0 {
                    state[i] = State::Slide;
                    sp[i] = std::array::from_fn(|c| stv[c] - s.mu * fn0[i] / ks * stv[c] / l);
                } else {
                    state[i] = State::Stick;
                    sp[i] = stv;
                }
            }
            phase = 1;
            continue;
        }
        if phase == 1 && !changed && it > 0 && moved <= (1e-4 * big).max(tiny) {
            break;
        }
        if it == 119 {
            warnings.push("Contact did not settle in 120 rounds: which nodes touch or slide is still changing a little. The results are the last round's.".into());
        }
    }
    let (u, st, fnode, fn_, fs) = last.ok_or("Contact could not be solved.")?;
    // What each contact carried, and the pressure at each node of the first faces.
    let mut outs: Vec<ContactOut> = m.contacts.iter().map(|c| ContactOut { id: c.id, ..Default::default() }).collect();
    let mut cp = vec![0.0f64; ms.nodes.len()];
    for (i, s) in sl.iter().enumerate() {
        let o = &mut outs[s.pair];
        o.nodes += 1;
        if state[i] == State::Open {
            continue;
        }
        if fn_[i] > 0.0 {
            o.touching += 1;
            if state[i] == State::Slide && s.mu > 0.0 {
                o.sliding += 1;
            }
        }
        let f = fs[i];
        for c in 0..3 {
            o.force[c] += f[c];
        }
        o.normal += fn_[i];
        let ft = sub(f, s.n.map(|v| v * dot(f, s.n)));
        o.friction += dot(ft, ft).sqrt();
        let p = fn_[i].max(0.0) / s.area.max(1e-300);
        cp[s.q] = cp[s.q].max(p);
        o.max_pressure = o.max_pressure.max(p);
        o.max_overlap = o.max_overlap.max(fn_[i] / eps);
    }
    Ok((u, st, fnode, outs, cp))
}

/// The pairs of faces, one from `a` and one from `b`, that touch: a corner of one within `tol`
/// (m) of a facet of the other, over it (0.8.0: contact between two solids found by itself). Each
/// pair comes with its finer face first (the one whose nodes are kept out of the other).
pub fn touching(ms: &TetMesh, a: &[u32], b: &[u32], tol: f64) -> Vec<(u32, u32)> {
    let corners = |f: u32| -> Vec<usize> {
        let mut v: Vec<usize> = ms.faces[f as usize].facets.iter().flat_map(|&(e, k)| ms.facet(e as usize, k as usize)[..3].iter().map(|&i| i as usize).collect::<Vec<_>>()).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let near = |nodes: &[usize], f: u32| {
        ms.faces[f as usize].facets.iter().any(|&(e, k)| {
            let fc = ms.facet(e as usize, k as usize);
            let cn = [0, 1, 2].map(|i| ms.nodes[fc[i] as usize]);
            let (e1, e2) = (sub(cn[1], cn[0]), sub(cn[2], cn[0]));
            let n = unit(cross(e1, e2));
            let (a11, a12, a22) = (dot(e1, e1), dot(e1, e2), dot(e2, e2));
            let det = a11 * a22 - a12 * a12;
            det > 0.0
                && nodes.iter().any(|&q| {
                    let d = sub(ms.nodes[q], cn[0]);
                    let (b1, b2) = (dot(e1, d), dot(e2, d));
                    let (l1, l2) = ((b1 * a22 - b2 * a12) / det, (a11 * b2 - a12 * b1) / det);
                    l1 > 1e-6 && l2 > 1e-6 && l1 + l2 < 1.0 - 1e-6 && dot(d, n).abs() <= tol
                })
        })
    };
    let mut out = vec![];
    for &fa in a {
        let ca = corners(fa);
        for &fb in b {
            let cb = corners(fb);
            if fa != fb && (near(&ca, fb) || near(&cb, fa)) {
                out.push(if ca.len() >= cb.len() { (fa, fb) } else { (fb, fa) });
            }
        }
    }
    out
}
