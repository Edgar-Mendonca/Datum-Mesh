//! Load cases and combinations (0.3.3). A case is a named set of loads; a combination adds cases
//! times factors. Every analysis here is linear, so a combination is solved as one model whose
//! loads are the cases' loads scaled by their factors (the same answer as adding the cases'
//! results), and the envelope solves each combination in turn.

use crate::model::{Analysis, LoadKind, Model};
use serde::{Deserialize, Serialize};

/// What to solve: one load case or one combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "on", rename_all = "lowercase")]
pub enum Which {
    Case { id: u32 },
    Combo { id: u32 },
}

/// A load times a factor; None for one that is not a load's size (convection's film).
fn scaled(k: &LoadKind, f: f64) -> Option<LoadKind> {
    use LoadKind::*;
    Some(match *k {
        Force { fx, fy, fz } => Force { fx: fx * f, fy: fy * f, fz: fz * f },
        Moment { m, mx, my } => Moment { m: m * f, mx: mx * f, my: my * f },
        Dist { w1, w2, dir } => Dist { w1: w1 * f, w2: w2 * f, dir },
        Pressure { p } => Pressure { p: p * f },
        Traction { fx, fy, total, fz } => Traction { fx: fx * f, fy: fy * f, total, fz: fz * f },
        Bearing { fx, fy } => Bearing { fx: fx * f, fy: fy * f },
        Warming { dt } => Warming { dt: dt * f },
        Gravity { gx, gy, gz } => Gravity { gx: gx * f, gy: gy * f, gz: gz * f },
        Flux { q } => Flux { q: q * f },
        Heat { q } => Heat { q: q * f },
        Convection { .. } => return None,
    })
}

/// The model with only the loads of a case, or of a combination scaled by its factors. None:
/// every load as it is.
pub fn select(m: &Model, which: Option<Which>) -> Result<Model, String> {
    let Some(w) = which else { return Ok(m.clone()) };
    let mut out = m.clone();
    match w {
        Which::Case { id } => {
            if !m.cases.iter().any(|c| c.id == id) {
                return Err("That load case no longer exists.".into());
            }
            out.loads.retain(|l| m.case_of(l) == id);
        }
        Which::Combo { id } => {
            let c = m.combos.iter().find(|c| c.id == id).ok_or("That combination no longer exists.")?;
            if m.analysis == Analysis::Heat {
                return Err("Combinations are for structural loads; solve a heat model one load case at a time.".into());
            }
            if c.parts.is_empty() {
                return Err(format!("{} has no load cases in it yet: give it factors (Loads › Cases and Combinations).", c.name));
            }
            out.loads = vec![];
            for p in &c.parts {
                for l in m.loads.iter().filter(|l| m.case_of(l) == p.case) {
                    let kind = scaled(&l.kind, p.f).ok_or("Convection cannot be scaled by a factor; leave it out of combinations.")?;
                    out.loads.push(crate::model::Load { kind, ..l.clone() });
                }
            }
        }
    }
    Ok(out)
}

/// What a combination's results are called, for the window and the report.
pub fn label(m: &Model, which: Option<Which>) -> String {
    let name = |id: u32| m.cases.iter().find(|c| c.id == id).map_or("?".to_string(), |c| c.name.clone());
    match which {
        None => "All loads".into(),
        Some(Which::Case { id }) => name(id),
        Some(Which::Combo { id }) => m.combos.iter().find(|c| c.id == id).map_or("?".into(), |c| {
            let parts: Vec<String> = c.parts.iter().map(|p| format!("{} × {}", fmt(p.f), name(p.case))).collect();
            format!("{}: {}", c.name, parts.join(" + "))
        }),
    }
}
fn fmt(f: f64) -> String {
    let s = format!("{f:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The largest and smallest member forces and reactions over every combination (or every case,
/// when there are no combinations), with the one that gives each member's largest utilisation.
#[derive(Clone, Debug, Serialize)]
pub struct Envelope {
    /// The combinations (or cases) solved, by name.
    pub over: Vec<String>,
    pub members: Vec<EnvMember>,
    pub reactions: Vec<EnvReaction>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct EnvMember {
    pub id: u32,
    pub n_max: f64,
    pub n_min: f64,
    /// Largest shear and bending moment (both ways in 3D) and twisting moment, without signs.
    pub v: f64,
    pub m: f64,
    pub t: f64,
    pub deflection: f64,
    pub util: f64,
    /// The combination giving the largest utilisation.
    pub governs: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct EnvReaction {
    pub node: u32,
    /// Fx, Fy, Fz, Mx, My, Mz (plane frames: Fx, Fy and M about z), largest and smallest.
    pub max: [f64; 6],
    pub min: [f64; 6],
}

/// The envelope of a frame (plane or 3D).
pub fn envelope(m: &Model) -> Result<Envelope, String> {
    let list: Vec<(String, Which)> = if !m.combos.is_empty() {
        m.combos.iter().map(|c| (c.name.clone(), Which::Combo { id: c.id })).collect()
    } else {
        m.cases.iter().map(|c| (c.name.clone(), Which::Case { id: c.id })).collect()
    };
    if list.is_empty() {
        return Err("There are no load cases or combinations to go over: add them on the Loads tab.".into());
    }
    let mut members: Vec<EnvMember> = vec![];
    let mut reactions: Vec<EnvReaction> = vec![];
    let mut warnings = vec![];
    let mut over = vec![];
    for (name, w) in list {
        let one = select(m, Some(w))?;
        // (id, N along, |V|, |M|, |T|, deflection, util) and (node, six reactions).
        let (rows, reac): (Vec<(u32, Vec<f64>, f64, f64, f64, f64, f64)>, Vec<(u32, [f64; 6])>) = match m.analysis {
            Analysis::Frame => {
                let r = crate::frame::solve(&one).map_err(|e| format!("{name}: {e}"))?;
                warnings.extend(r.warnings.iter().map(|x| format!("{name}: {x}")));
                let big = |v: &[f64]| v.iter().fold(0.0f64, |a, b| a.max(b.abs()));
                (
                    r.members.iter().map(|x| (x.id, x.n.clone(), big(&x.v), big(&x.m), 0.0, x.d.iter().fold(0.0f64, |a, d| a.max(d[0].hypot(d[1]))), x.util)).collect(),
                    r.reactions.iter().map(|x| (x.node, [x.fx, x.fy, 0.0, 0.0, 0.0, x.m])).collect(),
                )
            }
            Analysis::Space => {
                let r = crate::space::solve(&one).map_err(|e| format!("{name}: {e}"))?;
                warnings.extend(r.warnings.iter().map(|x| format!("{name}: {x}")));
                let big = |v: &[f64]| v.iter().fold(0.0f64, |a, b| a.max(b.abs()));
                (
                    r.members.iter().map(|x| (x.id, x.n.clone(), big(&x.vy).max(big(&x.vz)), big(&x.mz).max(big(&x.my)), big(&x.t), x.d.iter().fold(0.0f64, |a, d| a.max((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())), x.util)).collect(),
                    r.reactions.iter().map(|x| (x.node, x.f)).collect(),
                )
            }
            _ => return Err("The envelope is for frames and trusses.".into()),
        };
        for (id, n, v, mm, t, d, u) in rows {
            let (nmax, nmin) = (n.iter().cloned().fold(f64::MIN, f64::max), n.iter().cloned().fold(f64::MAX, f64::min));
            match members.iter_mut().find(|x| x.id == id) {
                Some(x) => {
                    x.n_max = x.n_max.max(nmax);
                    x.n_min = x.n_min.min(nmin);
                    x.v = x.v.max(v);
                    x.m = x.m.max(mm);
                    x.t = x.t.max(t);
                    x.deflection = x.deflection.max(d);
                    if u > x.util {
                        x.util = u;
                        x.governs = name.clone();
                    }
                }
                None => members.push(EnvMember { id, n_max: nmax, n_min: nmin, v, m: mm, t, deflection: d, util: u, governs: name.clone() }),
            }
        }
        for (node, f) in reac {
            match reactions.iter_mut().find(|x| x.node == node) {
                Some(x) => {
                    for k in 0..6 {
                        x.max[k] = x.max[k].max(f[k]);
                        x.min[k] = x.min[k].min(f[k]);
                    }
                }
                None => reactions.push(EnvReaction { node, max: f, min: f }),
            }
        }
        over.push(name);
    }
    warnings.dedup();
    Ok(Envelope { over, members, reactions, warnings })
}
