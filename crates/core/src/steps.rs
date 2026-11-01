//! Analysis steps (0.4.0): the sequence a model is solved in. No steps, or only linear ones: the
//! linear solve of each analysis, as before. Nonlinear steps (large deflection, by Newton or by
//! arc length) on plane frames and on 3D frames (large rotations, 0.5.8; second order with
//! plates): each starts from where the one before ended and takes the loads from there to its
//! own. Parts and solids: elastic-plastic or large-deformation steps.

use crate::cases::{self, Which};
use crate::engine::path::{follow, Control};
use crate::frames::corot::{Corot, Curve};
use crate::engine::dynamics::{self, DynInfo, Structure};
use crate::model::{Analysis, LoadKind, Model, Step, StepKind};
use crate::sparse::Stats;
use crate::Results;

/// Solve the model by its steps, for the load case or combination chosen in the window.
pub fn run(m: &Model, which: Option<Which>) -> Result<Results, String> {
    if let Some(st) = m.steps.iter().find(|s| matches!(s.kind, StepKind::Spectrum | StepKind::History)) {
        if m.steps.len() > 1 {
            return Err("A dynamic step (response spectrum or time history) stands on its own: make it the only step (Solve › Steps).".into());
        }
        return dynamic(m, which, st);
    }
    let nonlinear = m.steps.iter().any(|s| s.kind != StepKind::Linear);
    if !nonlinear {
        // Linear: the last step's load times its factor (or the window's choice as it is).
        return match m.steps.last() {
            Some(st) => crate::solve(&cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?),
            None => crate::solve(&cases::select(m, which)?),
        };
    }
    if m.analysis != Analysis::Solid && m.steps.iter().any(|s| s.kind == StepKind::Large) {
        return Err("Large-deformation steps (rubber) are for 3D solids in this release; frames use large-deflection steps.".into());
    }
    if m.analysis == Analysis::Space && !m.shapes.iter().any(|s| s.plate.is_some()) {
        // 3D frames (0.5.8): large rotations by corotational beams, Newton or arc length.
        if m.steps.iter().any(|s| s.kind == StepKind::Linear) {
            return Err("A linear step cannot follow or lead a large-deflection one: make every step large-deflection, or every step linear (Solve › Steps).".into());
        }
        return space(m, which);
    }
    if m.analysis == Analysis::Space {
        // 3D frames with plates (0.4.3): second order (P-delta) for the last step's load; no arc length.
        if m.steps.iter().any(|s| s.kind == StepKind::Arc) {
            return Err("Following a 3D frame past its limit load (arc length) comes later; use a large-deflection step (second order, P-delta) for 3D frames.".into());
        }
        let st = m.steps.last().expect("steps");
        let mm = cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?;
        return crate::space::pdelta(&mm, st.pieces as usize).map(Results::Space);
    }
    if matches!(m.analysis, Analysis::Stress | Analysis::Strain | Analysis::Axi) {
        // Parts (0.5.0): an elastic-plastic solve (von Mises with each material's hardening),
        // small strains, for the last step's load; no arc length.
        if m.steps.iter().any(|s| s.kind == StepKind::Arc) {
            return Err("Following a part past its collapse load (arc length) comes later; use a nonlinear step (load in increments) for plasticity.".into());
        }
        let st = m.steps.last().expect("steps");
        let mm = cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?;
        return crate::plane::solve_plastic(&mm, st.increments as usize).map(Results::Plane);
    }
    if m.analysis == Analysis::Solid {
        // 3D solids (0.5.1): elastic-plastic, as for plane parts.
        if m.steps.iter().any(|s| s.kind == StepKind::Arc) {
            return Err("Following a part past its collapse load (arc length) comes later; use a nonlinear step (load in increments) for plasticity.".into());
        }
        let st = m.steps.last().expect("steps");
        let mm = cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?;
        if st.kind == StepKind::Large {
            return crate::solid::solve_large(&mm, st.increments as usize).map(Results::Solid);
        }
        return crate::solid::solve_plastic(&mm, st.increments as usize).map(Results::Solid);
    }
    if m.analysis != Analysis::Frame {
        return Err("Nonlinear steps are for frames, plane parts and 3D solids.".into());
    }
    if m.steps.iter().any(|s| s.kind == StepKind::Linear) {
        return Err("A linear step cannot follow or lead a large-deflection one: make every step large-deflection, or every step linear (Home › Steps).".into());
    }
    frame(m, which)
}

fn frame(m: &Model, which: Option<Which>) -> Result<Results, String> {
    let t0 = std::time::Instant::now();
    let pieces = m.steps[0].pieces as usize;
    let mut loads = vec![];
    for st in &m.steps {
        let mm = cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?;
        loads.push(Corot::new(&mm, pieces)?);
    }
    let (c, _) = &loads[0];
    let n = c.asm.sys.n;
    let mut warnings = c.asm.warnings.clone();
    if m.members.iter().any(|b| b.only != crate::model::Only::Both) {
        warnings.push("Tension-only and compression-only members act both ways in large-deflection steps in this release.".into());
    }
    if m.loads.iter().any(|l| matches!(l.kind, LoadKind::Settlement { .. })) {
        warnings.push("Support settlements are left out of large-deflection steps in this release.".into());
    }
    let mut u = vec![0.0; n];
    let mut base = vec![0.0; n];
    let mut curve = Curve::default();
    let mut points: Vec<(u32, f64, Vec<f64>, bool)> = vec![(1, 0.0, u.clone(), true)];
    let mut iterations = 0;
    let mut stopped = false;
    for (k, st) in m.steps.iter().enumerate() {
        let target = &loads[k].1;
        let load: Vec<f64> = (0..n).map(|i| target[i] - base[i]).collect();
        let ctl = Control { increments: st.increments as usize, arc: st.kind == StepKind::Arc, ..Default::default() };
        let path = follow(c, &base, &load, &u, &ctl)?;
        iterations += path.iterations;
        for p in path.points.iter().skip(1) {
            points.push((k as u32 + 1, st.factor * p.lambda, p.u.clone(), p.stable));
        }
        let last = path.points.last().expect("the path starts somewhere");
        u = last.u.clone();
        for i in 0..n {
            base[i] += last.lambda * load[i];
        }
        if !path.reached {
            warnings.push(format!("{}: {}", st.name, path.note.unwrap_or_default()));
            stopped = true;
            break;
        }
    }
    // The node the curve follows: the one asked for, or the model node that moved most.
    let nn = m.nodes.len();
    let at = match m.steps.iter().find_map(|s| s.monitor) {
        Some(id) => m.nodes.iter().position(|x| x.id == id).ok_or("The node a step follows no longer exists.")?,
        None => (0..nn).max_by(|&a, &b| u[3 * a].hypot(u[3 * a + 1]).total_cmp(&u[3 * b].hypot(u[3 * b + 1]))).unwrap_or(0),
    };
    curve.node = m.nodes.get(at).map_or(0, |x| x.id);
    for (step, load, uu, stable) in &points {
        curve.step.push(*step);
        curve.load.push(*load);
        curve.ux.push(uu[3 * at]);
        curve.uy.push(uu[3 * at + 1]);
        curve.rz.push(uu[3 * at + 2]);
        curve.stable.push(*stable);
    }
    // The first limit: the largest load before it first falls (the whole path's largest if it
    // never does).
    let fall = curve.load.windows(2).position(|w| w[1] < w[0] - 1e-12).map_or(curve.load.len(), |i| i + 1);
    for i in 0..fall {
        if curve.load[i] > curve.peak {
            curve.peak = curve.load[i];
            curve.peak_step = curve.step[i];
        }
    }
    curve.iterations = iterations;
    if !stopped && !points.last().is_some_and(|p| p.3) {
        warnings.push("The structure is not stable at the end of the last step (its stiffness is not positive): it is held only by the exact balance of the loads.".into());
    }
    // The solver report: the out-of-balance force left at the end, over the load.
    use crate::engine::path::Problem;
    let (fint, k) = c.state(&u)?;
    let held = &c.asm.held;
    let (mut r2, mut f2) = (0.0, 0.0);
    for i in 0..n {
        if !held[i] {
            r2 += (base[i] - fint[i]).powi(2);
            f2 += base[i].powi(2);
        }
    }
    let residual = if f2 > 0.0 { (r2 / f2).sqrt() } else { r2.sqrt() };
    let nonzeros = k.k.iter().filter(|e| e.0 >= e.1 && !held[e.0] && !held[e.1]).count();
    let stats = Stats { equations: held.iter().filter(|h| !**h).count(), nonzeros, residual, ms: t0.elapsed().as_secs_f64() * 1e3 };
    let mut r = c.results(m, &u, &base, stats, warnings);
    r.path = Some(curve);
    Ok(Results::Frame(r))
}

/// Large rotations of a 3D frame (0.5.8), step after step, by the corotational 3D beam: as the
/// plane frame's driver, with the curve's movement along z too.
fn space(m: &Model, which: Option<Which>) -> Result<Results, String> {
    use crate::frames::corot3::Corot3;
    let t0 = std::time::Instant::now();
    let pieces = m.steps[0].pieces as usize;
    let mut loads = vec![];
    for st in &m.steps {
        let mm = cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?;
        loads.push(Corot3::new(&mm, pieces)?);
    }
    let (c, _) = &loads[0];
    let n = c.asm.sys.n;
    let mut warnings = vec![];
    if m.members.iter().any(|b| b.only != crate::model::Only::Both) {
        warnings.push("Tension-only and compression-only members act both ways in large-deflection steps in this release.".into());
    }
    if m.loads.iter().any(|l| matches!(l.kind, LoadKind::Settlement { .. })) {
        warnings.push("Support settlements are left out of large-deflection steps in this release.".into());
    }
    let mut u = vec![0.0; n];
    let mut base = vec![0.0; n];
    let mut points: Vec<(u32, f64, Vec<f64>, bool)> = vec![(1, 0.0, u.clone(), true)];
    let mut iterations = 0;
    let mut stopped = false;
    for (k, st) in m.steps.iter().enumerate() {
        let target = &loads[k].1;
        let load: Vec<f64> = (0..n).map(|i| target[i] - base[i]).collect();
        let ctl = Control { increments: st.increments as usize, arc: st.kind == StepKind::Arc, ..Default::default() };
        let path = follow(c, &base, &load, &u, &ctl)?;
        iterations += path.iterations;
        for p in path.points.iter().skip(1) {
            points.push((k as u32 + 1, st.factor * p.lambda, p.u.clone(), p.stable));
        }
        let last = path.points.last().expect("the path starts somewhere");
        u = last.u.clone();
        for i in 0..n {
            base[i] += last.lambda * load[i];
        }
        if !path.reached {
            warnings.push(format!("{}: {}", st.name, path.note.unwrap_or_default()));
            stopped = true;
            break;
        }
    }
    let nn = m.nodes.len();
    let mag = |i: usize| (u[6 * i].powi(2) + u[6 * i + 1].powi(2) + u[6 * i + 2].powi(2)).sqrt();
    let at = match m.steps.iter().find_map(|s| s.monitor) {
        Some(id) => m.nodes.iter().position(|x| x.id == id).ok_or("The node a step follows no longer exists.")?,
        None => (0..nn).max_by(|&a, &b| mag(a).total_cmp(&mag(b))).unwrap_or(0),
    };
    let mut curve = Curve { node: m.nodes.get(at).map_or(0, |x| x.id), ..Default::default() };
    for (step, load, uu, stable) in &points {
        curve.step.push(*step);
        curve.load.push(*load);
        curve.ux.push(uu[6 * at]);
        curve.uy.push(uu[6 * at + 1]);
        curve.uz.push(uu[6 * at + 2]);
        curve.rz.push((uu[6 * at + 3].powi(2) + uu[6 * at + 4].powi(2) + uu[6 * at + 5].powi(2)).sqrt());
        curve.stable.push(*stable);
    }
    let fall = curve.load.windows(2).position(|w| w[1] < w[0] - 1e-12).map_or(curve.load.len(), |i| i + 1);
    for i in 0..fall {
        if curve.load[i] > curve.peak {
            curve.peak = curve.load[i];
            curve.peak_step = curve.step[i];
        }
    }
    curve.iterations = iterations;
    if !stopped && !points.last().is_some_and(|p| p.3) {
        warnings.push("The structure is not stable at the end of the last step (its stiffness is not positive): it is held only by the exact balance of the loads.".into());
    }
    use crate::engine::path::Problem;
    let (fint, k) = c.state(&u)?;
    let held = &c.asm.held;
    let (mut r2, mut f2) = (0.0, 0.0);
    for i in 0..n {
        if !held[i] {
            r2 += (base[i] - fint[i]).powi(2);
            f2 += base[i].powi(2);
        }
    }
    let residual = if f2 > 0.0 { (r2 / f2).sqrt() } else { r2.sqrt() };
    let nonzeros = k.k.iter().filter(|e| e.0 >= e.1 && !held[e.0] && !held[e.1]).count();
    let stats = Stats { equations: held.iter().filter(|h| !**h).count(), nonzeros, residual, ms: t0.elapsed().as_secs_f64() * 1e3 };
    let mut r = c.results(m, &u, &base, stats, warnings)?;
    r.path = Some(curve);
    Ok(Results::Space(r))
}

/// A dynamic step (0.4.2) on a plane or 3D frame.
fn dynamic(m: &Model, which: Option<Which>, st: &Step) -> Result<Results, String> {
    let t0 = std::time::Instant::now();
    let loaded = cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?;
    // The shaking of the ground and the spectrum move the masses only: the static loads are left
    // out (solve them on their own and add). A load history applies the step's loads.
    let masses = Model { loads: loaded.loads.iter().filter(|l| matches!(l.kind, LoadKind::Mass { .. })).cloned().collect(), ..loaded.clone() };
    let base = if st.kind == StepKind::History && !st.ground { &loaded } else { &masses };
    let pieces = st.pieces as usize;
    match m.analysis {
        Analysis::Frame => {
            let s = crate::frame::Dyn::new(base, pieces)?;
            let (mut r, info) = if st.kind == StepKind::Spectrum { spectrum(&s, st, |o, rho| s.combine(o, rho))? } else { history(&s, st, s.load(), m)? };
            r.warnings.extend(info.warnings.iter().cloned());
            r.stats.ms = t0.elapsed().as_secs_f64() * 1e3;
            r.dynamic = Some(info);
            Ok(Results::Frame(r))
        }
        Analysis::Space => {
            let s = crate::space::Dyn::new(base, pieces)?;
            let (mut r, info) = if st.kind == StepKind::Spectrum { spectrum(&s, st, |o, rho| s.combine(o, rho))? } else { history(&s, st, s.load(), m)? };
            r.warnings.extend(info.warnings.iter().cloned());
            r.stats.ms = t0.elapsed().as_secs_f64() * 1e3;
            r.dynamic = Some(info);
            Ok(Results::Space(r))
        }
        _ => Err("Dynamic steps are for frames, plates and trusses in this release; solids come later.".into()),
    }
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

fn spectrum<S: Structure>(s: &S, st: &Step, combine: impl Fn(&[S::Out], &[Vec<f64>]) -> S::Out) -> Result<(S::Out, DynInfo), String> {
    let free = s.held().iter().filter(|h| !**h).count();
    let (modes, _) = crate::modal::lowest(s.k(), s.mass(), s.held(), (st.modes as usize).min(free))?;
    let (outs, rho, mut info) = dynamics::spectrum(s, &modes, unit(st.along), &st.spectrum, st.damping, st.cqc);
    if info.share < 0.9 {
        info.warnings.push(format!("The {} modes reach {:.0} % of the mass along the shaking; add modes (Solve › Steps) until they reach 90 % or more.", info.modes.len(), info.share * 100.0));
    }
    info.notes.push(format!("Peak values of the modes combined by {}: each number is a largest size, without its sign, and the reactions and loads do not balance as a static solve's do.", info.combined));
    Ok((combine(&outs, &rho), info))
}

fn history<S: Structure>(s: &S, st: &Step, load: &[f64], m: &Model) -> Result<(S::Out, DynInfo), String> {
    let n = s.k().n;
    let free = s.held().iter().filter(|h| !**h).count();
    let (modes, _) = crate::modal::lowest(s.k(), s.mass(), s.held(), 2.min(free))?;
    let w: Vec<f64> = modes.iter().map(|x| x.lambda.max(0.0).sqrt()).collect();
    let (alpha, beta) = dynamics::rayleigh(st.damping, w[0], *w.get(1).unwrap_or(&w[0]));
    let mr = dynamics::mul(s.mass(), &s.ground(unit(st.along)));
    let ground = st.ground;
    let force = |t: f64| -> Vec<f64> {
        let g = dynamics::interp(&st.time, t);
        if ground { mr.iter().map(|x| -x * g).collect() } else { load.iter().map(|x| x * g).collect() }
    };
    let steps = (st.duration / st.dt).round().max(1.0) as usize;
    let nn = m.nodes.len();
    let dofs: Vec<[usize; 3]> = (0..nn).map(|i| s.node(i)).collect();
    let at = |u: &[f64], d: [usize; 3]| d.map(|k| if k == usize::MAX { 0.0 } else { u[k] });
    let keep_all = nn * (steps + 1) <= 2_000_000;
    let mon = st.monitor.and_then(|id| m.nodes.iter().position(|x| x.id == id));
    let mut series: Vec<Vec<[f64; 3]>> = vec![vec![]; nn];
    let mut times = vec![];
    let mut peak = vec![0.0f64; nn];
    let mut best = (-1.0f64, 0.0, vec![0.0; n], vec![0.0; n], 0.0);
    // The time of the largest movement of the node followed (or of any node): the results are
    // shown then, with the inertia and damping forces among the loads so they balance.
    let mut each = |_: usize, t: f64, u: &[f64], v: &[f64], a: &[f64], f: &[f64]| {
        times.push(t);
        for i in 0..nn {
            let d = at(u, dofs[i]);
            let size = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            peak[i] = peak[i].max(size);
            if keep_all || Some(i) == mon {
                series[i].push(d);
            }
            if (mon.is_none() || mon == Some(i)) && size > best.0 {
                let ma = dynamics::mul(s.mass(), a);
                let (mv, kv) = (dynamics::mul(s.mass(), v), s.k().mul(v));
                let feff: Vec<f64> = (0..n).map(|k| f[k] - ma[k] - alpha * mv[k] - beta * kv[k]).collect();
                best = (size, t, u.to_vec(), feff, if ground { 0.0 } else { dynamics::interp(&st.time, t) });
            }
        }
    };
    dynamics::history(s, (alpha, beta), st.dt, steps, &force, &mut each)?;
    let node = mon.unwrap_or_else(|| (0..nn).max_by(|&a, &b| peak[a].total_cmp(&peak[b])).unwrap_or(0));
    let sr = &series[node];
    let mut info = DynInfo { kind: "history", node: m.nodes.get(node).map_or(0, |x| x.id), t: times, ux: sr.iter().map(|d| d[0]).collect(), uy: sr.iter().map(|d| d[1]).collect(), uz: sr.iter().map(|d| d[2]).collect(), peak_t: best.1, peak, steps, ..Default::default() };
    if info.t.len() != info.ux.len() {
        info.t.truncate(info.ux.len());
    }
    let tp = std::f64::consts::TAU / w[0];
    if st.dt > tp / 10.0 {
        info.warnings.push(format!("The time step is over a tenth of the first period ({:.3} s): the motion is followed coarsely; use a smaller step.", tp));
    }
    info.notes.push(format!("The results shown are at t = {:.3} s, when {} moves most{}; the largest movement of every node over the whole history is in the record.", best.1, m.nodes.get(node).map_or("the model".to_string(), |_| format!("node {}", node + 1)), if ground { " (relative to the ground)" } else { "" }));
    let out = s.results(&best.2, &best.3, best.4);
    Ok((out, info))
}

#[cfg(test)]
mod tests {
    use crate::model::{Step, StepKind};

    #[test]
    fn the_slab_shaken_by_a_spectrum_reaches_most_of_its_mass_and_names_its_modes() {
        let mut m = crate::examples::get("slab").unwrap();
        m.mesh.size = 0.5;
        m.steps = vec![Step { id: m.next, kind: StepKind::Spectrum, along: [1.0, 0.0, 0.0], modes: 12, pieces: 4, ..Default::default() }];
        m.next += 1;
        let Ok(crate::Results::Space(r)) = crate::solve_for(&m, None) else { panic!() };
        let d = r.dynamic.unwrap();
        assert!(d.share > 0.9 && d.share <= 1.0 + 1e-9, "{}", d.share);
        assert_eq!(d.modes.len(), 12);
        assert!(d.modes.windows(2).all(|w| w[1].f >= w[0].f * (1.0 - 1e-9)));
        // Every number a peak: nothing negative.
        assert!(r.members.iter().all(|mr| mr.n.iter().chain(&mr.mz).all(|v| *v >= 0.0)));
        assert!(r.plates.unwrap().fields.iter().all(|f| f.min >= 0.0));
        // A dynamic step with another step is refused, and says why.
        m.steps.push(Step { id: m.next, ..Default::default() });
        m.next += 1;
        assert!(crate::solve_for(&m, None).unwrap_err().contains("on its own"));
    }

    /// Arc length in 3D (0.5.8): Lee's frame, built as a 3D frame (a stiff spring holds it in
    /// its plane at the knee and the load), snaps through at the same first limit as in 2D.
    #[test]
    fn lees_frame_in_3d_snaps_through_at_the_plane_frames_limit() {
        use crate::doc::Item;
        use crate::model::{Support, SupportKind, Target};
        let plane = crate::examples::get("lee").unwrap();
        let Ok(crate::Results::Frame(r2)) = crate::solve_for(&plane, None) else { panic!() };
        let mut m = plane.clone();
        m.analysis = crate::model::Analysis::Space;
        for p in [[0.0, 1.2], [0.24, 1.2]] {
            let id = m.nodes.iter().find(|n| (n.x - p[0]).abs() < 1e-9 && (n.y - p[1]).abs() < 1e-9).unwrap().id;
            let mut d = crate::doc::Doc::new(m);
            d.apply(crate::doc::Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind: SupportKind::Spring { kx: 0.0, ky: 0.0, kr: 0.0, kz: 1e9, krx: 0.0, kry: 0.0 } }) }).unwrap();
            m = d.m;
        }
        let Ok(crate::Results::Space(r3)) = crate::solve_for(&m, None) else { panic!("{:?}", crate::solve_for(&m, None).err()) };
        let (c2, c3) = (r2.path.unwrap(), r3.path.unwrap());
        assert!((c3.peak - c2.peak).abs() < 0.01 * c2.peak, "3D {} 2D {}", c3.peak, c2.peak);
        // Past the limit: the load falls on the way down, and the path reaches the full load.
        assert!(c3.stable.iter().any(|s| !s) && (c3.load.last().unwrap() - 1.0).abs() < 1e-9);
    }

    /// 0.5.8: with large rotations the members' drawn movements follow the nodes (they were the
    /// pieces' small local movements, so the bend looked as if it hardly moved).
    #[test]
    fn a_3d_frame_with_large_rotations_draws_its_members_where_the_nodes_went() {
        let m = crate::examples::get("bend45").unwrap();
        let Ok(crate::Results::Space(r)) = crate::solve_for(&m, None) else { panic!() };
        for (mb, mr) in m.members.iter().zip(&r.members) {
            for (end, d) in [(mb.a, mr.d[0]), (mb.b, *mr.d.last().unwrap())] {
                let i = m.nodes.iter().position(|n| n.id == end).unwrap();
                assert!((0..3).all(|k| (d[k] - r.disp[i][k]).abs() < 1e-9), "{d:?} {:?}", r.disp[i]);
            }
        }
        assert!(r.members.iter().flat_map(|mr| &mr.d).any(|d| d[1] < -1.3));
    }
}
