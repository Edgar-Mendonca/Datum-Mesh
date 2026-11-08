//! Load cases and combinations (0.3.3). A case is a named set of loads; a combination adds cases
//! times factors. Every analysis here is linear, so a combination is solved as one model whose
//! loads are the cases' loads scaled by their factors (the same answer as adding the cases'
//! results), and the envelope solves each combination in turn.

use crate::model::{Analysis, CaseKind, Combo, ComboRule, Factor, LoadKind, Model};
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
        Patch { w1, w2, dir, from, to, projected } => Patch { w1: w1 * f, w2: w2 * f, dir, from, to, projected },
        Concentrated { f: p, dir, at } => Concentrated { f: p * f, dir, at },
        Pressure { p } => Pressure { p: p * f },
        Traction { fx, fy, total, fz } => Traction { fx: fx * f, fy: fy * f, total, fz: fz * f },
        Bearing { fx, fy } => Bearing { fx: fx * f, fy: fy * f },
        Warming { dt } => Warming { dt: dt * f },
        Gravity { gx, gy, gz } => Gravity { gx: gx * f, gy: gy * f, gz: gz * f },
        Flux { q } => Flux { q: q * f },
        Heat { q } => Heat { q: q * f },
        // A mass is the structure's, not a load: never scaled.
        Mass { m } => Mass { m },
        Settlement { dx, dy, dz } => Settlement { dx: dx * f, dy: dy * f, dz: dz * f },
        // A starting velocity is how the solid moves, not a load to scale.
        Velocity { v } => Velocity { v },
        Remote { at, f: fv, m } => Remote { at, f: fv.map(|v| v * f), m: m.map(|v| v * f) },
        Convection { .. } => return None,
    })
}

/// The model with every load times `f` (0.4.0: a step's factor).
pub fn scale(m: &Model, f: f64) -> Result<Model, String> {
    let mut out = m.clone();
    if f != 1.0 {
        for l in out.loads.iter_mut() {
            l.kind = scaled(&l.kind, f).ok_or("Convection cannot be scaled by a factor.")?;
        }
    }
    Ok(out)
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
            out.loads.retain(|l| m.case_of(l) == id || matches!(l.kind, LoadKind::Mass { .. }));
        }
        Which::Combo { id } => {
            let c = m.combos.iter().find(|c| c.id == id).ok_or("That combination no longer exists.")?;
            if m.analysis == Analysis::Heat {
                return Err("Combinations are for structural loads; solve a heat model one load case at a time.".into());
            }
            if c.parts.is_empty() {
                return Err(format!("{} has no load cases in it yet: give it factors (Loads › Cases and Combinations).", c.name));
            }
            // Masses are in every case, once.
            out.loads = m.loads.iter().filter(|l| matches!(l.kind, LoadKind::Mass { .. })).cloned().collect();
            for p in &c.parts {
                for l in m.loads.iter().filter(|l| m.case_of(l) == p.case && !matches!(l.kind, LoadKind::Mass { .. })) {
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

/// Combinations by rule (0.10.3): the general limit-state way of combining load cases, with the
/// user's own factors (no design code's numbers are built in). With permanent cases G and
/// variable cases Q₁…Qₙ:
/// - strength, each Qᵢ leading in turn: γG,unfav·ΣG + γQ·Qᵢ + Σⱼ≠ᵢ γQ·ψ0ⱼ·Qⱼ (and, if asked, the same
///   with γG,fav for permanent loads that help, such as against uplift);
/// - service, characteristic: ΣG + Qᵢ + Σⱼ≠ᵢ ψ0ⱼ·Qⱼ;
/// - service, long-term (quasi-permanent): ΣG + Σⱼ ψ2ⱼ·Qⱼ.
///
/// With no variable case each set is one combination of the permanent cases.
pub fn by_rule(m: &Model, r: &ComboRule) -> Result<Vec<Combo>, String> {
    let fin = |v: f64| v.is_finite() && v >= 0.0;
    if ![r.g_unfav, r.g_fav, r.q].into_iter().all(fin) || !m.cases.iter().all(|c| fin(c.psi0) && fin(c.psi2)) {
        return Err("Factors must be numbers of zero or more.".into());
    }
    let g: Vec<u32> = m.cases.iter().filter(|c| c.kind == CaseKind::Permanent).map(|c| c.id).collect();
    let q: Vec<&crate::model::Case> = m.cases.iter().filter(|c| c.kind == CaseKind::Variable).collect();
    if g.is_empty() && q.is_empty() {
        return Err("Mark the load cases permanent or variable first: none would be combined.".into());
    }
    if !(r.strength || r.strength_fav || r.service || r.long_term) {
        return Err("Pick at least one set of combinations to make.".into());
    }
    let mut out = vec![];
    let mut push = |name: String, parts: Vec<(u32, f64)>| {
        let parts: Vec<Factor> = parts.into_iter().filter(|p| p.1 != 0.0).map(|(case, f)| Factor { case, f }).collect();
        if !parts.is_empty() {
            out.push(Combo { id: 0, name, parts, auto: true });
        }
    };
    let short = |c: &crate::model::Case| c.name.clone();
    // One set: the permanent cases at `gf`, then each variable case leading at `lead` with the
    // others at `acc`·ψ0, or (with no variable case) the permanent cases alone.
    let mut set = |tag: &str, gf: f64, lead: f64, acc: f64| {
        if q.is_empty() {
            push(format!("{tag}: permanent"), g.iter().map(|c| (*c, gf)).collect());
        }
        for (i, qi) in q.iter().enumerate() {
            let mut parts: Vec<(u32, f64)> = g.iter().map(|c| (*c, gf)).collect();
            for (j, qj) in q.iter().enumerate() {
                parts.push((qj.id, if i == j { lead } else { acc * qj.psi0 }));
            }
            push(format!("{tag}: {} leading", short(qi)), parts);
        }
    };
    if r.strength {
        set("Strength", r.g_unfav, r.q, r.q);
    }
    if r.strength_fav {
        set("Strength, permanent helping", r.g_fav, r.q, r.q);
    }
    if r.service {
        set("Service", 1.0, 1.0, 1.0);
    }
    if r.long_term {
        let mut parts: Vec<(u32, f64)> = g.iter().map(|c| (*c, 1.0)).collect();
        parts.extend(q.iter().map(|c| (c.id, c.psi2)));
        push("Service, long-term".into(), parts);
    }
    if out.is_empty() {
        return Err("Every factor came out zero: no combination to make.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod rule_tests {
    use super::*;
    use crate::model::{Analysis, Case};

    fn case(id: u32, name: &str, kind: CaseKind, psi0: f64, psi2: f64) -> Case {
        Case { id, name: name.into(), kind, psi0, psi2 }
    }

    #[test]
    fn each_variable_case_leads_in_turn_with_the_others_reduced() {
        let mut m = Model::new(Analysis::Frame);
        m.cases = vec![case(1, "Dead", CaseKind::Permanent, 1.0, 1.0), case(2, "Imposed", CaseKind::Variable, 0.7, 0.3), case(3, "Wind", CaseKind::Variable, 0.6, 0.0), case(4, "Test", CaseKind::Excluded, 1.0, 1.0)];
        let r = ComboRule { g_unfav: 1.35, g_fav: 1.0, q: 1.5, strength: true, strength_fav: true, service: true, long_term: true };
        let c = by_rule(&m, &r).unwrap();
        // Two strength sets of two, the service set of two, one long-term.
        assert_eq!(c.len(), 7);
        let f = |c: &Combo, id: u32| c.parts.iter().find(|p| p.case == id).map_or(0.0, |p| p.f);
        let s1 = c.iter().find(|x| x.name == "Strength: Imposed leading").unwrap();
        assert!((f(s1, 1) - 1.35).abs() < 1e-12 && (f(s1, 2) - 1.5).abs() < 1e-12 && (f(s1, 3) - 0.9).abs() < 1e-12 && f(s1, 4) == 0.0);
        let s2 = c.iter().find(|x| x.name == "Strength, permanent helping: Wind leading").unwrap();
        assert!((f(s2, 1) - 1.0).abs() < 1e-12 && (f(s2, 3) - 1.5).abs() < 1e-12 && (f(s2, 2) - 1.05).abs() < 1e-12);
        let sv = c.iter().find(|x| x.name == "Service: Wind leading").unwrap();
        assert!((f(sv, 1) - 1.0).abs() < 1e-12 && (f(sv, 2) - 0.7).abs() < 1e-12 && (f(sv, 3) - 1.0).abs() < 1e-12);
        let lt = c.iter().find(|x| x.name == "Service, long-term").unwrap();
        assert!((f(lt, 2) - 0.3).abs() < 1e-12 && f(lt, 3) == 0.0, "a zero factor leaves the case out");
        assert!(c.iter().all(|x| x.auto));
        // Nothing to combine, or no set asked for, says so.
        m.cases.iter_mut().for_each(|c| c.kind = CaseKind::Excluded);
        assert!(by_rule(&m, &r).is_err());
    }
}
