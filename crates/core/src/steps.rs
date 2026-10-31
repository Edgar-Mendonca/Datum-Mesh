//! Analysis steps (0.4.0): the sequence a model is solved in. No steps, or only linear ones: the
//! linear solve of each analysis, as before. Nonlinear steps (large deflection, by Newton or by
//! arc length) are for plane frames in this release: each starts from where the one before
//! ended and takes the loads from there to its own.

use crate::cases::{self, Which};
use crate::engine::path::{follow, Control};
use crate::frames::corot::{Corot, Curve};
use crate::model::{Analysis, Model, StepKind};
use crate::sparse::Stats;
use crate::Results;

/// Solve the model by its steps, for the load case or combination chosen in the window.
pub fn run(m: &Model, which: Option<Which>) -> Result<Results, String> {
    let nonlinear = m.steps.iter().any(|s| s.kind != StepKind::Linear);
    if !nonlinear {
        // Linear: the last step's load times its factor (or the window's choice as it is).
        return match m.steps.last() {
            Some(st) => crate::solve(&cases::scale(&cases::select(m, st.load.or(which))?, st.factor)?),
            None => crate::solve(&cases::select(m, which)?),
        };
    }
    if m.analysis != Analysis::Frame {
        return Err("Large-deflection steps are for plane frames in this release; 3D frames and solids come later. Make the steps linear (Home › Steps).".into());
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
    let stats = Stats { equations: c.asm.held.iter().filter(|h| !**h).count(), nonzeros: 0, residual: 0.0, ms: t0.elapsed().as_secs_f64() * 1e3 };
    let mut r = c.results(m, &u, &base, stats, warnings);
    r.path = Some(curve);
    Ok(Results::Frame(r))
}
