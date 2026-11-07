//! Progress of a solve, and stopping it (0.9.1).
//!
//! The solver loops tell whoever is watching what they are doing: the stage (meshing, assembling,
//! factorising), each load increment, each Newton iteration with its out-of-balance force, the
//! increments cut back and why, and the time reached in a time-stepped solve. The window shows
//! these in its solver monitor; the engine itself never waits for them.
//!
//! Whoever starts a solve wraps it in [`watch`] with a function that receives the events (on the
//! solving thread, so it must be quick) and a flag of its own; setting the flag (from any thread)
//! asks the solve to end: the loops check [`halted`] at the start of each increment or time step
//! and end there, keeping what has converged, as when a structure collapses.

use serde::Serialize;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// One thing a solve reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "e", rename_all = "snake_case")]
pub enum Event {
    /// What the solve is doing, in words ("Factorising 19,395 equations").
    Stage { text: String },
    /// A load increment starts, aiming at load factor `lambda` (0 to 1 of the step).
    Increment { lambda: f64 },
    /// A Newton iteration: the force out of balance over the load's size.
    Iteration { it: usize, residual: f64 },
    /// The increment reached balance at `lambda` in `iterations` iterations.
    Converged { lambda: f64, iterations: usize },
    /// The increment would not settle and was cut back; it tries `lambda` next.
    Cut { lambda: f64, why: String },
    /// A time-stepped solve has reached time `t` of `of` (seconds), after `steps` steps.
    Time { t: f64, of: f64, steps: usize },
    /// Anything else worth a line in the log.
    Note { text: String },
}

type Sink = Box<dyn Fn(&Event)>;

thread_local! {
    static SINK: RefCell<Option<(Sink, Arc<AtomicBool>)>> = const { RefCell::new(None) };
}

/// Run `f` with `sink` receiving its events (on this thread); setting `stop` asks it to end early.
pub fn watch<T>(sink: impl Fn(&Event) + 'static, stop: Arc<AtomicBool>, f: impl FnOnce() -> T) -> T {
    let old = SINK.with(|s| s.replace(Some((Box::new(sink), stop))));
    let out = f();
    SINK.with(|s| *s.borrow_mut() = old);
    out
}

/// Report one event (nothing happens when no one watches).
pub fn emit(e: Event) {
    SINK.with(|s| {
        if let Some((f, _)) = s.borrow().as_ref() {
            f(&e)
        }
    });
}

pub fn stage(text: impl Into<String>) {
    emit(Event::Stage { text: text.into() });
}
pub fn increment(lambda: f64) {
    emit(Event::Increment { lambda });
}
pub fn iteration(it: usize, residual: f64) {
    emit(Event::Iteration { it, residual });
}
pub fn converged(lambda: f64, iterations: usize) {
    emit(Event::Converged { lambda, iterations });
}
pub fn cut(lambda: f64, why: impl Into<String>) {
    emit(Event::Cut { lambda, why: why.into() });
}
pub fn time(t: f64, of: f64, steps: usize) {
    emit(Event::Time { t, of, steps });
}
pub fn note(text: impl Into<String>) {
    emit(Event::Note { text: text.into() });
}

/// Whether the solve running on this thread was asked to stop.
pub fn stopping() -> bool {
    SINK.with(|s| s.borrow().as_ref().is_some_and(|(_, f)| f.load(Ordering::SeqCst)))
}

/// When a stop was asked for: the warning that says where the solve ended (`lambda` of the load,
/// or `None` for a time-stepped solve that words its own).
pub fn halted(lambda: f64) -> Option<String> {
    stopping().then(|| {
        note(format!("Stopped at your request at {:.1} % of the load.", lambda * 100.0));
        format!("Stopped at your request at {:.1} % of the load: the results shown are at the last load reached.", lambda * 100.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn events_reach_the_watcher_in_order_and_only_while_watched() {
        let seen = Rc::new(RefCell::new(vec![]));
        let s2 = seen.clone();
        let r = watch(move |e| s2.borrow_mut().push(e.clone()), Default::default(), || {
            increment(0.5);
            iteration(1, 0.1);
            converged(0.5, 1);
            7
        });
        assert_eq!(r, 7);
        increment(1.0); // no one watches now
        assert_eq!(*seen.borrow(), vec![Event::Increment { lambda: 0.5 }, Event::Iteration { it: 1, residual: 0.1 }, Event::Converged { lambda: 0.5, iterations: 1 }]);
        let j = serde_json::to_string(&seen.borrow()[1]).unwrap();
        assert_eq!(j, r#"{"e":"iteration","it":1,"residual":0.1}"#);
    }

    #[test]
    fn a_stop_is_seen_by_the_solve_it_was_given_to() {
        let flag = Arc::new(AtomicBool::new(false));
        watch(|_| {}, flag.clone(), || {
            assert!(halted(0.3).is_none());
            flag.store(true, Ordering::SeqCst);
            assert!(halted(0.3).unwrap().contains("30.0 %"));
        });
        assert!(!stopping(), "no one watches here");
    }
}
