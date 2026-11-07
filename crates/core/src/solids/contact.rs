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
    let start: Vec<State> = sl.iter().map(|s| if s.g0 <= 1e-9 * scale { State::Stick } else { State::Open }).collect();
    // One run of rounds: with friction (`mu_on`) or without, from the states and sliding forces
    // given, until they settle.
    #[allow(clippy::type_complexity)]
    let run = |mu_on: bool, mut state: Vec<State>, mut slide: Vec<[f64; 3]>, warnings: &mut Vec<String>| -> Result<(Vec<f64>, crate::engine::sparse::Stats, Vec<f64>, Vec<f64>, Vec<[f64; 3]>, Vec<State>, Vec<[f64; 3]>), String> {
        let mu = |s: &Slave| if mu_on { s.mu } else { 0.0 };
        let mut last = None;
        for it in 0..80 {
            let mut s2 = System { n: sys.n, k: sys.k.clone(), f: sys.f.clone(), half: sys.half, general: sys.general };
            for (i, s) in sl.iter().enumerate() {
                if state[i] == State::Open {
                    continue;
                }
                spring(&mut s2, s, s.n, eps);
                push(&mut s2.f, s, s.n.map(|v| -eps * s.g0 * v));
                match state[i] {
                    State::Stick if mu(s) > 0.0 => {
                        for t in s.t {
                            spring(&mut s2, s, t, eps);
                        }
                    }
                    State::Slide => push(&mut s2.f, s, slide[i]),
                    _ => {}
                }
            }
            let (u, st) = s2.solve(held, presc).map_err(|e| format!("{e} With contact, each solid must be held on its own or pressed onto another: a solid held only by contact without friction can slide away."))?;
            // The forces now, and the new states.
            let mut next = state.clone();
            let mut nslide = slide.clone();
            let mut fnode = vec![0.0; 3 * ms.nodes.len()];
            let mut fn_ = vec![0.0; sl.len()];
            let mut fs = vec![[0.0; 3]; sl.len()];
            for (i, s) in sl.iter().enumerate() {
                let g = s.g0 + along(s, &u, s.n);
                if state[i] == State::Open {
                    // A node closing: it sticks where it has not moved along the face, and
                    // slides (its friction found next round) where it has.
                    let st_: [f64; 3] = std::array::from_fn(|c| s.t[0][c] * along(s, &u, s.t[0]) + s.t[1][c] * along(s, &u, s.t[1]));
                    let moved = dot(st_, st_).sqrt() > 1e-12 * scale;
                    next[i] = if g >= 0.0 { State::Open } else if mu(s) > 0.0 && !moved { State::Stick } else { State::Slide };
                    nslide[i] = [0.0; 3];
                    continue;
                }
                let pn = -eps * g;
                if pn <= 0.0 {
                    next[i] = State::Open;
                    continue;
                }
                fn_[i] = pn;
                let mut f = s.n.map(|v| pn * v);
                if mu(s) > 0.0 {
                    let st_: [f64; 3] = std::array::from_fn(|c| s.t[0][c] * along(s, &u, s.t[0]) + s.t[1][c] * along(s, &u, s.t[1]));
                    let trial = st_.map(|v| -eps * v);
                    let tn = dot(trial, trial).sqrt();
                    // Sliding stays sliding while the load goes on (it only grows); its friction,
                    // μ times the pressing force against the way it slides, is eased towards the
                    // new value round by round so the pressing and the friction settle together.
                    let sl_ = dot(st_, st_).sqrt();
                    let limit = if sl_ < 1e-300 { [0.0; 3] } else { st_.map(|v| -mu(s) * pn * v / sl_) };
                    let sticks = state[i] == State::Stick && tn <= mu(s) * pn * (1.0 + 1e-9);
                    let ft = if sticks { trial } else if state[i] == State::Slide { slide[i] } else { limit };
                    next[i] = if sticks { State::Stick } else { State::Slide };
                    if !sticks {
                        nslide[i] = if state[i] == State::Slide { std::array::from_fn(|c| 0.5 * (slide[i][c] + limit[c])) } else { limit };
                    }
                    for c in 0..3 {
                        f[c] += ft[c];
                    }
                } else {
                    next[i] = State::Slide;
                    nslide[i] = [0.0; 3];
                }
                push(&mut fnode, s, f);
                fs[i] = f;
            }
            let changed = next != state;
            let moved = nslide.iter().zip(&slide).map(|(a, b)| dot(sub(*a, *b), sub(*a, *b)).sqrt()).fold(0.0, f64::max);
            let big = nslide.iter().map(|a| dot(*a, *a).sqrt()).fold(0.0, f64::max).max(1e-300);
            state = next;
            slide = nslide;
            let done = !changed && moved <= 1e-8 * big && it > 0;
            last = Some((u, st, fnode, fn_, fs));
            if done {
                break;
            }
            if it == 79 {
                warnings.push("Contact did not settle in 80 rounds: which nodes touch or slide is still changing. The results are the last round's.".into());
            }
        }
        let (u, st, fnode, fn_, fs) = last.ok_or("Contact could not be solved.")?;
        Ok((u, st, fnode, fn_, fs, state, slide))
    };
    let zero = vec![[0.0; 3]; sl.len()];
    let (u, st, fnode, fn_, fs, state, _) = if sl.iter().any(|s| s.mu > 0.0) {
        // With friction: first where the faces press, without it; then friction from there,
        // sliding at μ times that pressure the way they slid (sticking where they did not).
        let mut quiet = vec![];
        match run(false, start.clone(), zero.clone(), &mut quiet) {
            Ok((u0, _, _, fn0, _, st0, _)) => {
                let mut state = st0.clone();
                let mut slide = zero.clone();
                for (i, s) in sl.iter().enumerate() {
                    if st0[i] == State::Open || s.mu <= 0.0 {
                        continue;
                    }
                    let st_: [f64; 3] = std::array::from_fn(|c| s.t[0][c] * along(s, &u0, s.t[0]) + s.t[1][c] * along(s, &u0, s.t[1]));
                    let l = dot(st_, st_).sqrt();
                    if l > 1e-12 * scale {
                        state[i] = State::Slide;
                        slide[i] = st_.map(|v| -s.mu * fn0[i] * v / l);
                    } else {
                        state[i] = State::Stick;
                    }
                }
                run(true, state, slide, warnings)?
            }
            Err(_) => run(true, start, zero, warnings)?,
        }
    } else {
        run(false, start, zero, warnings)?
    };
    // What each contact carried, and the pressure at each node of the first faces.
    let mut outs: Vec<ContactOut> = m.contacts.iter().map(|c| ContactOut { id: c.id, ..Default::default() }).collect();
    let mut cp = vec![0.0f64; ms.nodes.len()];
    for (i, s) in sl.iter().enumerate() {
        let o = &mut outs[s.pair];
        o.nodes += 1;
        if state[i] == State::Open || fn_[i] <= 0.0 {
            continue;
        }
        o.touching += 1;
        if state[i] == State::Slide && s.mu > 0.0 {
            o.sliding += 1;
        }
        let f = fs[i];
        for c in 0..3 {
            o.force[c] += f[c];
        }
        o.normal += fn_[i];
        let ft = sub(f, s.n.map(|v| v * dot(f, s.n)));
        o.friction += dot(ft, ft).sqrt();
        let p = fn_[i] / s.area.max(1e-300);
        cp[s.q] = cp[s.q].max(p);
        o.max_pressure = o.max_pressure.max(p);
        o.max_overlap = o.max_overlap.max(fn_[i] / eps);
    }
    Ok((u, st, fnode, outs, cp))
}
