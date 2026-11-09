//! The path solver (0.4.0): follows a structure's equilibrium as its loads grow, for any module
//! that gives its internal forces and tangent stiffness. Two ways:
//!
//! - **Load control** (Newton–Raphson): the load factor λ rises in increments, each corrected by
//!   Newton iterations until the out-of-balance force is negligible; an increment that will not
//!   converge is halved.
//! - **Arc length** (Crisfield's cylindrical method, *Non-linear Finite Element Analysis of Solids
//!   and Structures*, vol. 1, 1991, ch. 9): λ is an unknown too, and each increment moves a fixed
//!   distance along the path, so it can pass limit points where the load falls (snap-through) and
//!   follow the path back up. The root of the constraint closest to the last direction is taken;
//!   the predictor's sign follows the last increment.
//!
//! The external load is `base + λ·load`, λ from 0 to 1 over the step.

use crate::sparse::System;

/// A nonlinear problem a module gives the path solver.
pub trait Problem {
    /// Unknowns (all, held included).
    fn size(&self) -> usize;
    /// Unknowns held at zero by supports.
    fn held(&self) -> &[bool];
    /// The internal forces and the tangent stiffness at movements `u`.
    fn state(&self, u: &[f64]) -> Result<(Vec<f64>, System), String>;
    /// Movements `u` moved on by an increment `du` the tangent was solved for: a plain sum, unless
    /// the module's unknowns compose otherwise (finite rotations in 3D, 0.5.8).
    fn add(&self, u: &[f64], du: &[f64]) -> Vec<f64> {
        u.iter().zip(du).map(|(a, b)| a + b).collect()
    }
}

/// How to follow the path.
#[derive(Clone, Debug)]
pub struct Control {
    /// Increments to reach λ = 1 if all goes well (load control), or the first increment's share
    /// of the load (arc length).
    pub increments: usize,
    pub arc: bool,
    /// Out-of-balance force over the load's size at which an increment has converged.
    pub tol: f64,
    pub max_iter: usize,
    /// Arc length: the most increments before giving up on reaching λ = 1.
    pub max_increments: usize,
    /// The smallest increment, as a share of the first, before the solve stops (0.10.3).
    pub min_step: f64,
    /// Grow the increment again after an easy one (few iterations) (0.10.3).
    pub grow: bool,
}

impl Default for Control {
    fn default() -> Self {
        Control { increments: 10, arc: false, tol: 1e-8, max_iter: 30, max_increments: 400, min_step: 1e-4, grow: true }
    }
}

impl Control {
    /// The control for a step: its kind and increments, and the user's solver settings (0.10.3).
    pub fn for_step(increments: usize, arc: bool, s: Option<&crate::model::SolverSettings>) -> Control {
        let d = Control { increments, arc, ..Default::default() };
        match s {
            None => d,
            Some(s) => Control { tol: s.tol, max_iter: s.max_iter as usize, max_increments: s.max_increments as usize, min_step: s.min_step, grow: s.grow, ..d },
        }
    }
}

/// One converged point on the path.
#[derive(Clone, Debug)]
pub struct Point {
    pub lambda: f64,
    pub u: Vec<f64>,
    pub iterations: usize,
    /// The tangent stiffness was positive definite here (stable).
    pub stable: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Path {
    /// Converged points from the start (λ = 0) on.
    pub points: Vec<Point>,
    /// λ = 1 was reached.
    pub reached: bool,
    /// Why the path stopped short, in words.
    pub note: Option<String>,
    /// Newton iterations in all.
    pub iterations: usize,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm_free(v: &[f64], held: &[bool]) -> f64 {
    v.iter().zip(held).filter(|(_, h)| !**h).map(|(x, _)| x * x).sum::<f64>().sqrt()
}

/// The out-of-balance force base + λ·load − internal on the free unknowns (zero at held ones).
fn residual(base: &[f64], load: &[f64], lambda: f64, fint: &[f64], held: &[bool]) -> Vec<f64> {
    (0..fint.len()).map(|i| if held[i] { 0.0 } else { base[i] + lambda * load[i] - fint[i] }).collect()
}

/// Follow the path from movements `u0` (in equilibrium with `base`) as λ goes from 0 to 1.
pub fn follow(p: &dyn Problem, base: &[f64], load: &[f64], u0: &[f64], ctl: &Control) -> Result<Path, String> {
    let held = p.held().to_vec();
    let n = p.size();
    let scale = norm_free(base, &held).max(norm_free(load, &held));
    if scale == 0.0 {
        let (_, k) = p.state(u0)?;
        let (_, stable) = k.solve_many(&held, &[&vec![0.0; n]])?;
        return Ok(Path { points: vec![Point { lambda: 0.0, u: u0.to_vec(), iterations: 0, stable }, Point { lambda: 1.0, u: u0.to_vec(), iterations: 0, stable }], reached: true, note: None, iterations: 0 });
    }
    let tol = ctl.tol * scale;
    let mut path = Path { points: vec![Point { lambda: 0.0, u: u0.to_vec(), iterations: 0, stable: true }], ..Default::default() };
    if ctl.arc {
        arc(p, base, load, &held, tol, ctl, &mut path)?;
    } else {
        newton(p, base, load, &held, tol, ctl, &mut path)?;
    }
    Ok(path)
}

/// Newton iterations at a fixed λ from `u`; Some((u, iterations, stable)) when converged.
///
/// With a line search (0.10.8; Crisfield 1991, §9.3): a correction that would leave more force
/// out of balance than before is halved, up to four times, and the best of the tries is taken.
/// Near the answer the whole correction is always better, so Newton keeps its quadratic rate and
/// the state found there is reused for the next iteration (no extra work).
fn correct(p: &dyn Problem, base: &[f64], load: &[f64], held: &[bool], tol: f64, lambda: f64, mut u: Vec<f64>, max_iter: usize) -> Result<Option<(Vec<f64>, usize, bool)>, String> {
    let scale = (norm_free(base, held).max(norm_free(load, held))).max(f64::MIN_POSITIVE);
    let mut cached: Option<(Vec<f64>, crate::sparse::System)> = None;
    for it in 0..=max_iter {
        let (fint, k) = match cached.take() {
            Some(s) => s,
            None => p.state(&u)?,
        };
        let r = residual(base, load, lambda, &fint, held);
        let rn = norm_free(&r, held);
        if !rn.is_finite() {
            return Ok(None);
        }
        crate::progress::iteration(it, rn / scale);
        let (x, stable) = match k.solve_many(held, &[&r]) {
            Ok(x) => x,
            Err(_) => return Ok(None),
        };
        // Converged: the force out of balance is negligible, or the corrections have stopped
        // changing anything but the last digits while it is small.
        let dn = dot(&x[0], &x[0]).sqrt();
        if it > 0 && (rn <= tol || (dn <= 1e-12 * dot(&u, &u).sqrt() && rn <= 1e3 * tol)) {
            return Ok(Some((u, it, stable)));
        }
        if it == max_iter {
            break;
        }
        // The line search: the whole correction, then halves while that leaves more out of balance.
        let mut best: Option<(f64, Vec<f64>, (Vec<f64>, crate::sparse::System))> = None;
        let mut a = 1.0;
        for _ in 0..5 {
            let trial = p.add(&u, &x[0].iter().map(|v| a * v).collect::<Vec<f64>>());
            if let Ok(st) = p.state(&trial) {
                let rt = norm_free(&residual(base, load, lambda, &st.0, held), held);
                if rt.is_finite() && best.as_ref().is_none_or(|b| rt < b.0) {
                    let done = rt <= rn;
                    best = Some((rt, trial, st));
                    if done {
                        break;
                    }
                }
            }
            a *= 0.5;
        }
        match best {
            Some((_, un, st)) => {
                u = un;
                cached = Some(st);
            }
            None => return Ok(None),
        }
    }
    Ok(None)
}

fn newton(p: &dyn Problem, base: &[f64], load: &[f64], held: &[bool], tol: f64, ctl: &Control, path: &mut Path) -> Result<(), String> {
    let first = 1.0 / ctl.increments.max(1) as f64;
    let mut step = first;
    let mut lambda = 0.0;
    let mut u = path.points[0].u.clone();
    while lambda < 1.0 - 1e-12 {
        if let Some(w) = crate::progress::halted(lambda) {
            path.note = Some(w);
            return Ok(());
        }
        let target = (lambda + step).min(1.0);
        crate::progress::increment(target);
        match correct(p, base, load, held, tol, target, u.clone(), ctl.max_iter)? {
            // A loaded structure follows its stable path: an unstable balance reached from a stable
            // one (a straight column past buckling) means the increment jumped a bifurcation, so
            // it is cut until the stable branch is found, or no smaller step helps.
            Some((_, _, false)) if path.points.last().is_some_and(|p| p.stable) && step > first * 1e-3 => {
                step /= 2.0;
                crate::progress::cut((lambda + step).min(1.0), "it jumped past a buckling point; the increment is halved to find the stable path");
            }
            Some((un, it, stable)) => {
                lambda = target;
                u = un;
                path.iterations += it;
                crate::progress::converged(lambda, it);
                path.points.push(Point { lambda, u: u.clone(), iterations: it, stable });
                if ctl.grow && it <= 4 {
                    step = (step * 1.5).min(first);
                }
            }
            None => {
                step /= 2.0;
                crate::progress::cut((lambda + step).min(1.0), format!("it did not converge in {} iterations; the increment is halved", ctl.max_iter));
                if step < first * ctl.min_step {
                    let last = path.points.last().map_or(0.0, |x| x.lambda);
                    path.note = Some(format!("The solution stopped at {:.1} % of the load: it would not converge beyond. The structure may have reached its limit load (snap-through or collapse); try arc length to follow it further.", last * 100.0));
                    return Ok(());
                }
            }
        }
    }
    path.reached = true;
    Ok(())
}

fn arc(p: &dyn Problem, base: &[f64], load: &[f64], held: &[bool], tol: f64, ctl: &Control, path: &mut Path) -> Result<(), String> {
    let n = p.size();
    let mut u = path.points[0].u.clone();
    let mut lambda = 0.0;
    // The first arc length: the first increment's share of the load on the starting tangent.
    let (_, k0) = p.state(&u)?;
    let (t0, _) = k0.solve_many(held, &[load])?;
    let tn = dot(&t0[0], &t0[0]).sqrt();
    if !(tn > 0.0) {
        return Err("The load does not move the structure.".into());
    }
    let dl0 = tn / ctl.increments.max(1) as f64;
    let mut dl = dl0;
    let mut prev: Option<(Vec<f64>, f64)> = None;
    let target_iter = 6.0;
    let scale = norm_free(base, held).max(norm_free(load, held)).max(f64::MIN_POSITIVE);
    for _ in 0..ctl.max_increments {
        if let Some(w) = crate::progress::halted(lambda) {
            path.note = Some(w);
            return Ok(());
        }
        crate::progress::increment(lambda);
        let (_, k) = p.state(&u)?;
        let (t, _) = k.solve_many(held, &[load])?;
        let t = &t[0];
        let tt = dot(t, t).sqrt();
        let sign = match &prev {
            Some((du, dlam)) => {
                if dot(du, t) + dlam * 0.0 >= 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
            None => 1.0,
        };
        let mut dlam = sign * dl / tt;
        // Close to the end of the step on a rising stable path: finish exactly at λ = 1.
        if lambda + dlam >= 1.0 - 1e-12 && dlam > 0.0 {
            let guess = p.add(&u, &t.iter().map(|x| (1.0 - lambda) * x).collect::<Vec<f64>>());
            if let Some((un, it, stable)) = correct(p, base, load, held, tol, 1.0, guess, ctl.max_iter)? {
                path.iterations += it;
                crate::progress::converged(1.0, it);
                path.points.push(Point { lambda: 1.0, u: un, iterations: it, stable });
                path.reached = true;
                return Ok(());
            }
            // Load control would not converge there: carry on along the arc.
        }
        let mut du: Vec<f64> = t.iter().map(|x| dlam * x).collect();
        let mut done = None;
        for it in 1..=ctl.max_iter {
            let ut = p.add(&u, &du);
            let (fint, k) = p.state(&ut)?;
            let r = residual(base, load, lambda + dlam, &fint, held);
            let rn = norm_free(&r, held);
            if !rn.is_finite() {
                break;
            }
            crate::progress::iteration(it, rn / scale);
            let Ok((x, stable)) = k.solve_many(held, &[&r, load]) else { break };
            if rn <= tol {
                done = Some((ut, it, stable));
                break;
            }
            let (dr, dt) = (&x[0], &x[1]);
            let w: Vec<f64> = (0..n).map(|i| du[i] + dr[i]).collect();
            let a = dot(dt, dt);
            let b = 2.0 * dot(dt, &w);
            let c = dot(&w, &w) - dl * dl;
            let disc = b * b - 4.0 * a * c;
            if !(disc >= 0.0) || a == 0.0 {
                break;
            }
            let roots = [(-b + disc.sqrt()) / (2.0 * a), (-b - disc.sqrt()) / (2.0 * a)];
            // The root that keeps the direction closest to this increment's so far.
            let pick = roots
                .iter()
                .map(|&g| {
                    let nd: Vec<f64> = (0..n).map(|i| w[i] + g * dt[i]).collect();
                    (dot(&nd, &du), g)
                })
                .fold((f64::NEG_INFINITY, 0.0), |m, x| if x.0 > m.0 { x } else { m })
                .1;
            for i in 0..n {
                du[i] = w[i] + pick * dt[i];
            }
            dlam += pick;
        }
        match done {
            Some((un, it, stable)) => {
                let step: Vec<f64> = (0..n).map(|i| un[i] - u[i]).collect();
                u = un;
                lambda += dlam;
                path.iterations += it;
                path.points.push(Point { lambda, u: u.clone(), iterations: it, stable });
                prev = Some((step, dlam));
                crate::progress::converged(lambda, it);
                dl = (dl * (target_iter / it.max(1) as f64).sqrt().clamp(0.5, 2.0)).min(dl0 * 10.0);
                if lambda >= 1.0 - 1e-12 {
                    path.reached = true;
                    return Ok(());
                }
            }
            None => {
                dl /= 2.0;
                crate::progress::cut(lambda, format!("it did not converge in {} iterations; the arc length is halved", ctl.max_iter));
                if dl < dl0 * ctl.min_step * 0.1 {
                    path.note = Some(format!("The arc-length solution would not converge beyond a load factor of {:.3}.", lambda));
                    return Ok(());
                }
            }
        }
    }
    let top = path.points.iter().map(|x| x.lambda).fold(f64::NEG_INFINITY, f64::max);
    path.note = Some(format!("The path did not reach the full load in {} increments; the largest load reached was {:.1} % of it.", ctl.max_increments, top * 100.0));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shallow two-bar truss (von Mises truss): bars from (±a, 0) to the apex (0, h), pinned
    /// at their feet, a load P down on the apex. With axial force N = EA (ℓ − ℓ₀)/ℓ₀ in a straight
    /// bar of length ℓ, equilibrium of the apex at drop w is P = 2N (h − w)/ℓ, exactly: the load
    /// rises to a limit, falls below zero as the apex passes the feet, and rises again.
    struct Mises {
        a: f64,
        h: f64,
        ea: f64,
        held: Vec<bool>,
    }
    impl Mises {
        fn force(&self, w: f64) -> (f64, f64) {
            let (l0, y) = ((self.a * self.a + self.h * self.h).sqrt(), self.h - w);
            let l = (self.a * self.a + y * y).sqrt();
            let nf = self.ea * (l - l0) / l0;
            // Internal force on the apex (downward positive) and its tangent dF/dw.
            let f = -2.0 * nf * y / l;
            let dn = self.ea / l0 * (-y / l);
            let dydl = -(l * l - y * y) / (l * l * l);
            (f, -2.0 * (dn * y / l + nf * dydl))
        }
    }
    impl Problem for Mises {
        fn size(&self) -> usize {
            1
        }
        fn held(&self) -> &[bool] {
            &self.held
        }
        fn state(&self, u: &[f64]) -> Result<(Vec<f64>, System), String> {
            let (f, k) = self.force(u[0]);
            let mut s = System::new(1);
            s.k.push((0, 0, k));
            Ok((vec![f], s))
        }
    }

    /// A spring whose force is atan(u): from u = 3 back to no load in one increment, plain Newton
    /// overshoots further each time (u = 3 → −9.5 → …); the line search brings it home.
    struct Atan {
        held: Vec<bool>,
    }
    impl Problem for Atan {
        fn size(&self) -> usize {
            1
        }
        fn held(&self) -> &[bool] {
            &self.held
        }
        fn state(&self, u: &[f64]) -> Result<(Vec<f64>, System), String> {
            let mut k = System::new(1);
            k.add(0, 0, 1.0 / (1.0 + u[0] * u[0]));
            Ok((vec![u[0].atan()], k))
        }
    }

    #[test]
    fn the_line_search_brings_an_overshooting_newton_home() {
        let p = Atan { held: vec![false] };
        let f0 = 3f64.atan();
        let path = follow(&p, &[f0], &[-f0], &[3.0], &Control { increments: 1, grow: false, ..Default::default() }).unwrap();
        assert!(path.reached, "{:?}", path.note);
        assert_eq!(path.points.len(), 2, "one increment, not cut");
        assert!(path.points[1].u[0].abs() < 1e-8, "{}", path.points[1].u[0]);
    }

    #[test]
    fn the_tangent_of_the_two_bar_truss_is_its_derivative() {
        let m = Mises { a: 1.0, h: 0.1, ea: 1e6, held: vec![false] };
        for w in [0.0, 0.03, 0.1, 0.17, 0.25] {
            let d = 1e-7;
            let fd = (m.force(w + d).0 - m.force(w - d).0) / (2.0 * d);
            assert!((fd - m.force(w).1).abs() < 1e-5 * fd.abs().max(1.0), "{w}: {fd} {}", m.force(w).1);
        }
    }

    #[test]
    fn solver_settings_change_how_the_path_is_followed() {
        // 0.10.3: the user's settings reach the solver. Half the limit load, well within reach.
        let m = Mises { a: 1.0, h: 0.1, ea: 1e6, held: vec![false] };
        let top = (0..=10_000).map(|i| m.force(0.1 * i as f64 / 10_000.0).0).fold(0.0, f64::max);
        let pl = 0.5 * top;
        let s = |tol: f64, max_iter: u32, min_step: f64, grow: bool| crate::model::SolverSettings { tol, max_iter, max_increments: 400, min_step, grow };
        let usual = follow(&m, &[0.0], &[pl], &[0.0], &Control::for_step(4, false, None)).unwrap();
        assert!(usual.reached);
        // A tighter tolerance takes at least as many iterations and still balances.
        let tight = follow(&m, &[0.0], &[pl], &[0.0], &Control::for_step(4, false, Some(&s(1e-13, 30, 1e-4, true)))).unwrap();
        assert!(tight.reached && tight.iterations >= usual.iterations);
        let last = tight.points.last().unwrap();
        assert!((m.force(last.u[0]).0 - pl).abs() < 1e-9 * pl);
        // One iteration allowed and no cutting below half the first increment: it stops, and says why.
        let stuck = follow(&m, &[0.0], &[pl], &[0.0], &Control::for_step(1, false, Some(&s(1e-12, 1, 0.5, true)))).unwrap();
        assert!(!stuck.reached && stuck.note.is_some());
        // Increments not grown: exactly the four asked for when each converges.
        let even = follow(&m, &[0.0], &[pl], &[0.0], &Control::for_step(4, false, Some(&s(1e-8, 30, 1e-4, false)))).unwrap();
        assert!(even.reached && even.points.len() == 5, "{}", even.points.len());
    }

    #[test]
    fn arc_length_follows_the_two_bar_truss_through_snap_through() {
        let m = Mises { a: 1.0, h: 0.1, ea: 1e6, held: vec![false] };
        // The exact limit load: the largest P on w in (0, h).
        let top = (0..=100_000).map(|i| m.force(0.1 * i as f64 / 100_000.0).0).fold(0.0, f64::max);
        // Load past the limit: 1.5 times; arc length goes over the top, down and back up.
        let pl = 1.5 * top;
        let path = follow(&m, &[0.0], &[pl], &[0.0], &Control { increments: 20, arc: true, ..Default::default() }).unwrap();
        assert!(path.reached, "{:?}", path.note);
        // Every point is in equilibrium with its own load.
        for pt in &path.points {
            assert!((m.force(pt.u[0]).0 - pt.lambda * pl).abs() < 1e-6 * pl, "{} {}", pt.u[0], pt.lambda);
        }
        // The path found the limit (within the increments' spacing) and passed below zero load.
        // The first maximum along the path (it rises again at the end, past the first limit).
        let first_fall = path.points.windows(2).position(|w| w[1].lambda < w[0].lambda).unwrap();
        let peak = path.points[..=first_fall].iter().map(|x| x.lambda * pl).fold(0.0, f64::max);
        assert!(peak > 0.97 * top && peak <= top * (1.0 + 1e-9), "{peak} {top}");
        assert!(path.points.iter().any(|x| x.lambda < 0.0));
        assert!(path.points.iter().any(|x| !x.stable));
        // It ends on the far side, the apex below its feet.
        assert!(path.points.last().unwrap().u[0] > 0.2);
        // Load control stops at the limit, and says so.
        let lc = follow(&m, &[0.0], &[pl], &[0.0], &Control { increments: 20, ..Default::default() }).unwrap();
        assert!(!lc.reached && lc.note.as_deref().unwrap().contains("limit"));
        let last = lc.points.last().unwrap().lambda * pl;
        assert!(last < top && last > 0.95 * top, "{last} {top}");
    }

    #[test]
    fn below_the_limit_both_ways_agree() {
        let m = Mises { a: 1.0, h: 0.1, ea: 1e6, held: vec![false] };
        let pl = 300.0;
        let a = follow(&m, &[0.0], &[pl], &[0.0], &Control { increments: 4, ..Default::default() }).unwrap();
        let b = follow(&m, &[0.0], &[pl], &[0.0], &Control { increments: 4, arc: true, ..Default::default() }).unwrap();
        assert!(a.reached && b.reached);
        let (ua, ub) = (a.points.last().unwrap().u[0], b.points.last().unwrap().u[0]);
        assert!((ua - ub).abs() < 1e-9, "{ua} {ub}");
        assert!((m.force(ua).0 - pl).abs() < 1e-6 * pl);
    }
}
