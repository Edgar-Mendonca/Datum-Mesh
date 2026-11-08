//! Mechanics checks (0.10.4): design members checked from mechanics formulas, not a design code.
//! Stress against yield, axial force against the Euler load with an effective length, deflection
//! from the line between the member's ends against span over a ratio, and storey drift. Each
//! combination is solved on its own and its forces are kept together: a station's N and M always
//! come from one combination at one place, never peaks from different ones (the specification's
//! F11). Every check says what it found (pass, fail, not checked, missing data) and shows its
//! formula and values; the report lists what is not checked at all.

use crate::cases::Which;
use crate::frames::design::{chain, segments, Chain};
use crate::model::{DesignMember, Holds, Model};
use crate::Results;
use serde::Serialize;
use std::f64::consts::PI;

/// One station's values, the same for plane and 3D frames: distance from end a (m), axial force
/// (N, tension positive), shear and moments (N, N·m), the movement (m) and the deflection from
/// the line between the design member's moved ends (m).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Sv {
    s: f64,
    n: f64,
    v: f64,
    mz: f64,
    my: f64,
    d: [f64; 3],
    w: f64,
}

fn unit(m: &Model, dm: &DesignMember) -> [f64; 3] {
    let (a, b) = (m.node(dm.a).map(|n| n.at()).unwrap_or_default(), m.node(dm.b).map(|n| n.at()).unwrap_or_default());
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-300);
    [d[0] / l, d[1] / l, d[2] / l]
}

/// The deflection of every station from the chord between the moved ends: the movement less
/// the straight line between the ends' movements, across the member (F09).
fn chord(st: &mut [Sv], u: [f64; 3], length: f64) {
    let (Some(f), Some(l)) = (st.first().copied(), st.last().copied()) else { return };
    for p in st.iter_mut() {
        let t = p.s / length;
        let r: [f64; 3] = std::array::from_fn(|k| p.d[k] - f.d[k] - (l.d[k] - f.d[k]) * t);
        let along = r[0] * u[0] + r[1] * u[1] + r[2] * u[2];
        p.w = (0..3).map(|k| (r[k] - along * u[k]).powi(2)).sum::<f64>().sqrt();
    }
}

/// The values along a design member under one solve, every station of its chain in order from
/// end a. A member that runs the other way is turned round: its moments change sign (the cut's
/// faces swap), its axial force and shear do not.
fn stations(m: &Model, r: &Results, dm: &DesignMember, c: &Chain) -> Result<Vec<Sv>, String> {
    let mut out = vec![];
    for l in &c.members {
        let missing = || "A member of the design member has no results.".to_string();
        let mut part: Vec<Sv> = match r {
            Results::Frame(f) => {
                let x = f.members.iter().find(|x| x.id == l.member).ok_or_else(missing)?;
                (0..x.x.len()).map(|i| Sv { s: x.x[i], n: x.n[i], v: x.v[i], mz: x.m[i], my: 0.0, d: [x.d[i][0], x.d[i][1], 0.0], w: 0.0 }).collect()
            }
            Results::Space(f) => {
                let x = f.members.iter().find(|x| x.id == l.member).ok_or_else(missing)?;
                (0..x.x.len()).map(|i| Sv { s: x.x[i], n: x.n[i], v: x.vy[i], mz: x.mz[i], my: x.my[i], d: x.d[i], w: 0.0 }).collect()
            }
            _ => return Err("Design members are checked in frames.".into()),
        };
        if !l.forward {
            part.reverse();
            for p in &mut part {
                p.s = l.length - p.s;
                p.mz = -p.mz;
                p.my = -p.my;
            }
        }
        out.extend(part.into_iter().map(|p| Sv { s: p.s + l.start, ..p }));
    }
    chord(&mut out, unit(m, dm), c.length);
    Ok(out)
}

/// What a design member carries at one place: from the left and from the right (they differ
/// where a point load or a joint makes a jump).
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Values {
    pub n: f64,
    pub v: f64,
    pub m: f64,
    /// 3D frames: the moment about the section's other axis.
    pub m_minor: f64,
    /// The movement (m) and the deflection from the line between the moved ends (m).
    pub d: [f64; 3],
    pub deflection: f64,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Probe {
    pub at: f64,
    pub length: f64,
    pub left: Values,
    pub right: Values,
    pub jump: bool,
    pub case: String,
}

fn values(p: &Sv) -> Values {
    Values { n: p.n, v: p.v, m: p.mz, m_minor: p.my, d: p.d, deflection: p.w }
}

fn lerp(a: &Sv, b: &Sv, s: f64) -> Sv {
    let t = if b.s > a.s { (s - a.s) / (b.s - a.s) } else { 0.0 };
    let f = |x: f64, y: f64| x + (y - x) * t;
    Sv { s, n: f(a.n, b.n), v: f(a.v, b.v), mz: f(a.mz, b.mz), my: f(a.my, b.my), d: std::array::from_fn(|k| f(a.d[k], b.d[k])), w: f(a.w, b.w) }
}

/// The values at `at` (m from end a) from the stations: at a station both sides of it (the
/// first and last of stations at one place), else in between, straight-line.
fn pick(st: &[Sv], at: f64, length: f64) -> (Sv, Sv) {
    let tol = 1e-9 * length.max(1.0);
    let here: Vec<&Sv> = st.iter().filter(|p| (p.s - at).abs() <= tol).collect();
    if let (Some(a), Some(b)) = (here.first(), here.last()) {
        return (**a, **b);
    }
    let k = st.windows(2).position(|w| w[0].s < at && at < w[1].s).unwrap_or(0);
    let p = lerp(&st[k], &st[(k + 1).min(st.len() - 1)], at);
    (p, p)
}

/// Values along a design member (0.10.4): at `at` m from its end a, under a load case or
/// combination (None: the loads as applied). Both sides where there is a jump (F08).
pub fn along(m: &Model, which: Option<Which>, id: u32, at: f64) -> Result<Probe, String> {
    let dm = m.design_members.iter().find(|x| x.id == id).ok_or("That design member does not exist.")?;
    let c = chain(m, dm.a, dm.b)?;
    if !(at >= 0.0 && at <= c.length * (1.0 + 1e-12)) {
        return Err(format!("Give a distance from 0 to {:.3} m, along the design member.", c.length));
    }
    let r = crate::solve_for(m, which)?;
    let st = stations(m, &r, dm, &c)?;
    let (l, rt) = pick(&st, at, c.length);
    let size = l.n.abs().max(l.v.abs()).max(l.mz.abs()).max(rt.n.abs()).max(rt.v.abs()).max(rt.mz.abs()).max(1e-30);
    let jump = [(l.n, rt.n), (l.v, rt.v), (l.mz, rt.mz), (l.my, rt.my)].iter().any(|(a, b)| (a - b).abs() > 1e-9 * size);
    Ok(Probe { at, length: c.length, left: values(&l), right: values(&rt), jump, case: crate::cases::label(m, which) })
}

/// One check of one design member.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Check {
    /// "stress", "buckling" or "deflection".
    pub kind: &'static str,
    /// "pass", "fail", "not checked" or "missing data".
    pub status: &'static str,
    /// What it uses over what it may take (pass up to 1).
    pub ratio: Option<f64>,
    /// The combination and the place that govern.
    pub governing: String,
    /// The formula and values behind it, line by line.
    pub trace: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MemberReport {
    pub id: u32,
    pub name: String,
    /// "valid", or why the design member cannot be checked.
    pub input: String,
    /// How the solves went.
    pub solve: String,
    pub checks: Vec<Check>,
    /// The worst of its checks: fail, missing data, not checked, pass.
    pub status: &'static str,
    pub ratio: Option<f64>,
    pub governing: String,
}

/// One storey's drift: the movement of its level against the level below, over its height.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Storey {
    /// The level (m, along y, up) and the height from the level below.
    pub level: f64,
    pub height: f64,
    /// Drift along x and along z, as shares of the height.
    pub dx: f64,
    pub dz: f64,
    pub ratio: f64,
    pub status: &'static str,
    pub governing: String,
}

/// A combination the checks solved, and how.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Used {
    pub name: String,
    /// Used for deflection and drift (service) rather than stress and buckling (strength).
    pub service: bool,
    /// "solved", or why not.
    pub solve: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Report {
    pub settings: crate::model::CheckSettings,
    pub combos: Vec<Used>,
    pub members: Vec<MemberReport>,
    pub storeys: Vec<Storey>,
    /// Said once for the whole report.
    pub notes: Vec<String>,
    /// What these checks never look at.
    pub not_checked: Vec<String>,
}

fn kn(x: f64) -> String {
    format!("{:.2} kN", x / 1e3 + 0.0)
}
fn knm(x: f64) -> String {
    format!("{:.2} kN·m", x / 1e3 + 0.0)
}
fn mpa(x: f64) -> String {
    format!("{:.1} MPa", x / 1e6)
}
fn mm(x: f64) -> String {
    format!("{:.2} mm", x * 1e3)
}
fn worst<'a>(xs: impl Iterator<Item = &'a str>) -> &'static str {
    let mut out = "pass";
    for s in xs {
        let rank = |x: &str| ["pass", "not checked", "missing data", "fail"].iter().position(|y| *y == x).unwrap_or(1);
        if rank(s) > rank(out) {
            out = ["pass", "not checked", "missing data", "fail"][rank(s)];
        }
    }
    out
}
fn verdict(ratio: f64) -> &'static str {
    if ratio <= 1.0 {
        "pass"
    } else {
        "fail"
    }
}

/// The solves the checks use: every combination, or the loads as applied when there is none.
/// Combinations named "Service…" (as the rule names them) are for deflection and drift; the
/// others for stress and buckling.
type Solve = (String, bool, Result<Results, String>);

fn solves(m: &Model) -> Vec<Solve> {
    if m.combos.is_empty() {
        return vec![("The loads as applied".into(), true, crate::solve_for(m, None))];
    }
    m.combos.iter().map(|c| (c.name.clone(), c.name.trim_start().to_lowercase().starts_with("service"), crate::solve_for(m, Some(Which::Combo { id: c.id })))).collect()
}

/// The strength and the service solves (by index): each the marked ones, or every one when
/// none is marked that way; and what the report should say about it.
fn sets(m: &Model, all: &[Solve]) -> (Vec<usize>, Vec<usize>, Vec<String>) {
    let every: Vec<usize> = (0..all.len()).collect();
    if m.combos.is_empty() {
        return (every.clone(), every, vec!["No combinations: every check uses the loads as applied, factored by nothing.".into()]);
    }
    let mut notes = vec![];
    let mut pick = |service: bool, note: &str| {
        let x: Vec<usize> = every.iter().copied().filter(|i| all[*i].1 == service).collect();
        if x.is_empty() {
            notes.push(note.to_string());
            every.clone()
        } else {
            x
        }
    };
    let strength = pick(false, "No strength combinations: stress and buckling are checked under every combination.");
    let service = pick(true, "No combination named Service…: deflection and drift are checked under every combination.");
    (strength, service, notes)
}

/// Run the mechanics checks on every design member, and the storey drift.
pub fn run(m: &Model) -> Result<Report, String> {
    if !matches!(m.analysis, crate::model::Analysis::Frame | crate::model::Analysis::Space) {
        return Err("Mechanics checks are for 2D and 3D frames.".into());
    }
    let s = m.check_settings.clone();
    let all = solves(m);
    let (strength, service, mut notes) = sets(m, &all);
    let combos = all.iter().map(|(n, sv, r)| Used { name: n.clone(), service: *sv, solve: match r { Ok(_) => "solved".into(), Err(e) => e.clone() } }).collect();
    let mut members = vec![];
    for dm in &m.design_members {
        members.push(member(m, dm, &s, &all, &strength, &service));
    }
    if m.design_members.is_empty() {
        notes.push("No design members: form them under Geometry › Design Members to check members.".into());
    }
    let storeys = drift(m, &s, &all, &service);
    Ok(Report {
        settings: s,
        combos,
        members,
        storeys,
        notes,
        not_checked: vec![
            "Lateral–torsional buckling of beams".into(),
            "Shear, and shear buckling of webs".into(),
            "Local buckling of the flanges and webs (section class)".into(),
            "Torsion".into(),
            "Buckling and bending acting together (interaction)".into(),
            "Connections and joints".into(),
            "Fatigue, fire and vibration".into(),
            "Any design standard's rules and factors (planned for 1.0, from a standard that is free to use)".into(),
        ],
    })
}

fn not_checked(kind: &'static str, why: &str) -> Check {
    Check { kind, status: "not checked", ratio: None, governing: String::new(), trace: vec![why.to_string()] }
}

fn member(m: &Model, dm: &DesignMember, s: &crate::model::CheckSettings, all: &[Solve], strength: &[usize], service: &[usize]) -> MemberReport {
    let done = |input: String, solve: String, checks: Vec<Check>| {
        let status = worst(checks.iter().map(|c| c.status));
        let gov = checks.iter().filter(|c| c.ratio.is_some()).max_by(|a, b| a.ratio.unwrap().total_cmp(&b.ratio.unwrap()));
        MemberReport { id: dm.id, name: dm.name.clone(), input, solve, ratio: gov.and_then(|c| c.ratio), governing: gov.map_or(String::new(), |c| format!("{} · {}", c.kind, c.governing)), status, checks }
    };
    let all3 = |why: &str| vec![not_checked("stress", why), not_checked("buckling", why), not_checked("deflection", why)];
    let c = match chain(m, dm.a, dm.b) {
        Ok(c) => c,
        Err(e) => return done(format!("broken: {e}"), "not solved for it".into(), all3("The design member is broken, so it is not checked.")),
    };
    let first = m.members.iter().find(|b| b.id == c.members[0].member);
    let (Some(sec), Some(mat)) = (first.and_then(|b| m.section(b.section)), first.and_then(|b| m.material(b.material))) else {
        return done("missing data: no section or material".into(), "not solved for it".into(), all3("The members have no section or material."));
    };
    let p = match crate::sections::props(&sec.shape) {
        Ok(p) => p,
        Err(e) => return done(format!("missing data: {e}"), "not solved for it".into(), all3("The section's properties could not be found.")),
    };
    // Each solve's stations, or why there are none.
    let mut st: Vec<Option<Vec<Sv>>> = vec![None; all.len()];
    let mut failed = vec![];
    for (i, (name, _, r)) in all.iter().enumerate() {
        match r.as_ref().map_err(|e| e.clone()).and_then(|r| stations(m, r, dm, &c)) {
            Ok(x) => st[i] = Some(x),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    let ok = st.iter().filter(|x| x.is_some()).count();
    let solve = if failed.is_empty() { format!("solved ({ok} of {})", all.len()) } else { format!("solved {ok} of {}; not solved: {}", all.len(), failed.join("; ")) };
    let space = matches!(m.analysis, crate::model::Analysis::Space);
    let mut checks = vec![];

    // Stress: σ = |N|/A + |M|/Z (+ |My|/Zy in 3D), one combination at one place.
    let mut best: Option<(f64, usize, Sv)> = None;
    for &i in strength {
        for q in st[i].iter().flatten() {
            let sg = q.n.abs() / p.a + q.mz.abs() / p.z() + if space && p.zy() > 0.0 { q.my.abs() / p.zy() } else { 0.0 };
            if best.is_none_or(|b| sg > b.0) {
                best = Some((sg, i, *q));
            }
        }
    }
    checks.push(match best {
        None => not_checked("stress", "No combination for strength was solved."),
        Some((sg, i, q)) => {
            let mut t = vec![
                if space { "σ = |N|/A + |Mz|/Z + |My|/Zy: the forces of one combination at one place (never peaks of different combinations)".into() } else { "σ = |N|/A + |M|/Z: the forces of one combination at one place (never peaks of different combinations)".into() },
                format!("At {:.3} m under {}: N = {}, M = {}{}", q.s, all[i].0, kn(q.n), knm(q.mz), if space { format!(", My = {}", knm(q.my)) } else { String::new() }),
                format!("A = {:.2} cm², Z = {:.1} cm³{}", p.a * 1e4, p.z() * 1e6, if space { format!(", Zy = {:.1} cm³", p.zy() * 1e6) } else { String::new() }),
                format!("σ = {}", mpa(sg)),
            ];
            if mat.fy > 0.0 {
                let r = sg / mat.fy;
                t.push(format!("fy = {} ({}); σ / fy = {r:.3}", mpa(mat.fy), mat.name));
                Check { kind: "stress", status: verdict(r), ratio: Some(r), governing: format!("{} at {:.3} m", all[i].0, q.s), trace: t }
            } else {
                t.push(format!("{} has no yield strength: give one under Materials to finish this check.", mat.name));
                Check { kind: "stress", status: "missing data", ratio: None, governing: format!("{} at {:.3} m", all[i].0, q.s), trace: t }
            }
        }
    });

    // Buckling: the largest compression against the Euler load, both axes.
    let mut ned: Option<(f64, usize, f64)> = None;
    for &i in strength {
        for q in st[i].iter().flatten() {
            if q.n < 0.0 && ned.is_none_or(|b| -q.n > b.0) {
                ned = Some((-q.n, i, q.s));
            }
        }
    }
    let ran = strength.iter().any(|i| st[*i].is_some());
    checks.push(if !ran {
        not_checked("buckling", "No combination for strength was solved.")
    } else if let Some((nc, i, at)) = ned {
        let sideways = DesignMember { restraints: dm.restraints.iter().filter(|r| r.holds != Holds::Twist).cloned().collect(), ..dm.clone() };
        let ls = segments(&sideways, c.length).iter().map(|x| x.length).fold(0.0, f64::max);
        let euler = |i: f64, l: f64| PI * PI * mat.e * i / (s.k * l).powi(2);
        let n1 = euler(p.i, c.length);
        let mut t = vec![
            "N_cr = π² E I / (K L)²: the Euler load of a straight strut, no imperfection and no yielding (a mechanics check, not a design rule)".into(),
            format!("Largest compression: {} under {} at {at:.3} m", kn(nc), all[i].0),
            format!("Strong axis: L = {:.3} m (end to end; restraints hold sideways, not in this plane), K = {}, I = {:.1} cm⁴ → N_cr = {}", c.length, s.k, p.i * 1e8, kn(n1)),
        ];
        let mut ncr = n1;
        if p.iy > 0.0 {
            let n2 = euler(p.iy, ls);
            t.push(format!("Weak axis: L = {ls:.3} m (the longest length between sideways restraints), K = {}, Iy = {:.1} cm⁴ → N_cr = {}", s.k, p.iy * 1e8, kn(n2)));
            ncr = ncr.min(n2);
        } else {
            t.push("Weak axis: the section gives no second moment that way, so it is not checked.".into());
        }
        let r = nc / ncr;
        t.push(format!("N / N_cr = {r:.3}"));
        Check { kind: "buckling", status: verdict(r), ratio: Some(r), governing: format!("{} at {at:.3} m", all[i].0), trace: t }
    } else {
        Check { kind: "buckling", status: "pass", ratio: Some(0.0), governing: String::new(), trace: vec!["No compression under any combination for strength: nothing to buckle.".into()] }
    });

    // Deflection from the line between the moved ends, against span / ratio.
    let mut wmax: Option<(f64, usize, f64)> = None;
    for &i in service {
        for q in st[i].iter().flatten() {
            if wmax.is_none_or(|b| q.w > b.0) {
                wmax = Some((q.w, i, q.s));
            }
        }
    }
    checks.push(match wmax {
        None => not_checked("deflection", "No combination for service was solved."),
        Some((w, i, at)) => {
            let lim = c.length / s.span_ratio;
            let r = w / lim;
            Check {
                kind: "deflection",
                status: verdict(r),
                ratio: Some(r),
                governing: format!("{} at {at:.3} m", all[i].0),
                trace: vec![
                    "δ: the movement across the member less the straight line between its moved ends (the ends' own movement and turning are left out)".into(),
                    format!("δ = {} at {at:.3} m under {}", mm(w), all[i].0),
                    format!("Limit: span / {} = {:.3} m / {} = {}", s.span_ratio, c.length, s.span_ratio, mm(lim)),
                    format!("δ / limit = {r:.3}"),
                ],
            }
        }
    });
    done("valid".into(), solve, checks)
}

/// Storey drift: the levels (y, up) where members meet, the mean movement of each level's nodes,
/// and each storey's drift over its height, under the service combinations.
fn drift(m: &Model, s: &crate::model::CheckSettings, all: &[Solve], service: &[usize]) -> Vec<Storey> {
    let used: std::collections::HashSet<u32> = m.members.iter().flat_map(|b| [b.a, b.b]).collect();
    let ys: Vec<f64> = m.nodes.iter().filter(|n| used.contains(&n.id)).map(|n| n.y).collect();
    let span = ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
    let tol = 1e-6 * span.max(1.0);
    let mut levels: Vec<f64> = vec![];
    let mut sorted = ys.clone();
    sorted.sort_by(f64::total_cmp);
    for y in sorted {
        if levels.last().is_none_or(|l| y - l > tol) {
            levels.push(y);
        }
    }
    if levels.len() < 2 {
        return vec![];
    }
    let mean = |r: &Results, y: f64| -> [f64; 2] {
        let idx: Vec<usize> = m.nodes.iter().enumerate().filter(|(_, n)| used.contains(&n.id) && (n.y - y).abs() <= tol).map(|(i, _)| i).collect();
        let k = idx.len().max(1) as f64;
        match r {
            Results::Frame(f) => [idx.iter().map(|i| f.disp[*i][0]).sum::<f64>() / k, 0.0],
            Results::Space(f) => [idx.iter().map(|i| f.disp[*i][0]).sum::<f64>() / k, idx.iter().map(|i| f.disp[*i][2]).sum::<f64>() / k],
            _ => [0.0, 0.0],
        }
    };
    levels.windows(2).map(|w| {
        let h = w[1] - w[0];
        let mut best = (0.0, 0.0, 0.0, String::new());
        for &i in service {
            if let Ok(r) = &all[i].2 {
                let (a, b) = (mean(r, w[0]), mean(r, w[1]));
                let (dx, dz) = ((b[0] - a[0]) / h, (b[1] - a[1]) / h);
                let r = dx.abs().max(dz.abs()) / s.drift;
                if r >= best.2 {
                    best = (dx, dz, r, all[i].0.clone());
                }
            }
        }
        Storey { level: w[1], height: h, dx: best.0, dz: best.1, ratio: best.2, status: if best.3.is_empty() { "not checked" } else { verdict(best.2) }, governing: best.3 }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, Case, CaseKind, Combo, Dir, Factor, Load, LoadKind, MemberKind, Restraint, Support, SupportKind, Target};

    fn beam(d: &mut Doc, a: [f64; 2], b: [f64; 2]) {
        d.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    }
    fn node(d: &Doc, x: f64, y: f64) -> u32 {
        d.m.nodes.iter().find(|n| (n.x - x).abs() < 1e-9 && (n.y - y).abs() < 1e-9).unwrap().id
    }
    fn support(d: &mut Doc, n: u32, kind: SupportKind) {
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: n }, kind }) }).unwrap();
    }
    fn load(d: &mut Doc, at: Target, kind: LoadKind, case: u32) {
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at, kind, case }) }).unwrap();
    }
    /// A simply supported beam of `pieces` members from 0 to `l`, formed into one design member.
    fn simple(l: f64, pieces: usize) -> (Doc, u32) {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        for k in 0..pieces {
            beam(&mut d, [l * k as f64 / pieces as f64, 0.0], [l * (k + 1) as f64 / pieces as f64, 0.0]);
        }
        let (a, b) = (node(&d, 0.0, 0.0), node(&d, l, 0.0));
        support(&mut d, a, SupportKind::Pinned);
        support(&mut d, b, SupportKind::RollerX);
        let ids = d.m.members.iter().map(|b| b.id).collect();
        let id = d.apply(Op::FormDesignMember { ids, name: "B1".into() }).unwrap()[0];
        (d, id)
    }
    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1e-30)
    }

    #[test]
    fn values_at_any_distance_and_both_sides_of_a_jump() {
        // F08: 8 m, two members, 10 kN down at the middle node. Shear ±5 kN either side of it,
        // M = PL/4 = 20 kN·m there and PL/8 = 10 kN·m at a quarter, between nodes.
        let (mut d, id) = simple(8.0, 2);
        let mid = node(&d, 4.0, 0.0);
        load(&mut d, Target::Node { id: mid }, LoadKind::Force { fx: 0.0, fy: -10e3, fz: 0.0 }, 0);
        let p = along(&d.m, None, id, 4.0).unwrap();
        assert!(p.jump && close((p.left.v - p.right.v).abs(), 10e3, 1e-9), "{p:?}");
        assert!(close(p.left.v.abs(), 5e3, 1e-9) && close(p.right.v.abs(), 5e3, 1e-9));
        assert!(close(p.left.m.abs(), 20e3, 1e-9) && close(p.right.m.abs(), 20e3, 1e-9));
        let q = along(&d.m, None, id, 2.0).unwrap();
        assert!(!q.jump && close(q.left.m.abs(), 10e3, 1e-6), "{q:?}");
        // The same load inside one member (a point load on it): the same jump, at a station.
        let (mut d1, id1) = simple(8.0, 1);
        let b = d1.m.members[0].id;
        load(&mut d1, Target::Member { id: b }, LoadKind::Concentrated { f: -10e3, dir: Dir::Y, at: 4.0 }, 0);
        let p1 = along(&d1.m, None, id1, 4.0).unwrap();
        assert!(p1.jump && close((p1.left.v - p1.right.v).abs(), 10e3, 1e-9) && close(p1.left.m.abs(), 20e3, 1e-9), "{p1:?}");
        // A member turned round in the chain gives the same values (its moment's sign follows the design member).
        let second = d.m.members[1].id;
        d.apply(Op::ReverseMember { ids: vec![second] }).unwrap();
        let r = along(&d.m, None, id, 6.0).unwrap();
        let r0 = along(&Doc::new(simple_loaded()).m, None, d.m.design_members[0].id, 6.0).unwrap();
        assert!(close(r.left.m, r0.left.m, 1e-9) && close(r.left.v, r0.left.v, 1e-9) && close(r.left.n.abs() + 1.0, r0.left.n.abs() + 1.0, 1e-9), "{r:?} {r0:?}");
        assert!(along(&d.m, None, id, 9.0).is_err());
    }
    fn simple_loaded() -> Model {
        let (mut d, _) = simple(8.0, 2);
        let mid = node(&d, 4.0, 0.0);
        load(&mut d, Target::Node { id: mid }, LoadKind::Force { fx: 0.0, fy: -10e3, fz: 0.0 }, 0);
        d.m
    }

    #[test]
    fn deflection_is_measured_from_the_line_between_the_moved_ends() {
        // F09, first: moving and turning the whole member gives no deflection; a sag stays a sag.
        let u = [1.0, 0.0, 0.0];
        let mut st: Vec<Sv> = (0..=10).map(|k| { let s = 0.6 * k as f64; Sv { s, d: [0.002, 0.01 + 0.003 * s, 0.0], ..Default::default() } }).collect();
        chord(&mut st, u, 6.0);
        assert!(st.iter().all(|p| p.w < 1e-15));
        for p in &mut st {
            p.d[1] -= 0.004 * (PI * p.s / 6.0).sin();
        }
        chord(&mut st, u, 6.0);
        assert!(close(st[5].w, 0.004, 1e-12));
        // A cantilever's tip moves PL³/3EI; from the chord the deflection is v(s) − v(L)·s/L.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        beam(&mut d, [0.0, 0.0], [3.0, 0.0]);
        let (a, b) = (node(&d, 0.0, 0.0), node(&d, 3.0, 0.0));
        support(&mut d, a, SupportKind::Fixed);
        load(&mut d, Target::Node { id: b }, LoadKind::Force { fx: 0.0, fy: -5e3, fz: 0.0 }, 0);
        let ids = d.m.members.iter().map(|b| b.id).collect();
        d.apply(Op::FormDesignMember { ids, name: String::new() }).unwrap();
        let rep = run(&d.m).unwrap();
        let defl = rep.members[0].checks.iter().find(|c| c.kind == "deflection").unwrap();
        let p = crate::sections::props(&d.m.sections[0].shape).unwrap();
        let ei = d.m.materials[0].e * p.i;
        let v = |s: f64| 5e3 * s * s * (9.0 - s) / (6.0 * ei);
        let want = (0..=40).map(|k| 3.0 * k as f64 / 40.0).map(|s| (v(s) - v(3.0) * s / 3.0).abs()).fold(0.0, f64::max);
        let got: f64 = defl.trace[1].split("δ = ").nth(1).unwrap().split(" mm").next().unwrap().parse().unwrap();
        assert!((got - want * 1e3).abs() < 0.006, "{got} {}", want * 1e3);
        assert!(defl.ratio.unwrap() > 0.0 && defl.ratio.unwrap() < 1.0, "{defl:?}");
    }

    #[test]
    fn a_combination_keeps_its_forces_together() {
        // F11: one combination pushes the beam end-on (N only), another bends it (M only). The
        // governing stress is the larger of the two, never N of one with M of the other.
        let (mut d, _) = simple(6.0, 2);
        let mut add = |item: Item| d.apply(Op::Add { item }).unwrap()[0];
        let push = add(Item::Case(Case { id: 0, name: "Push".into(), kind: CaseKind::Variable, psi0: 1.0, psi2: 1.0 }));
        let bend = add(Item::Case(Case { id: 0, name: "Bend".into(), kind: CaseKind::Variable, psi0: 1.0, psi2: 1.0 }));
        add(Item::Combo(Combo { id: 0, name: "C1".into(), parts: vec![Factor { case: push, f: 1.0 }], auto: false }));
        add(Item::Combo(Combo { id: 0, name: "C2".into(), parts: vec![Factor { case: bend, f: 1.0 }], auto: false }));
        let (end, mid) = (node(&d, 6.0, 0.0), node(&d, 3.0, 0.0));
        load(&mut d, Target::Node { id: end }, LoadKind::Force { fx: -200e3, fy: 0.0, fz: 0.0 }, push);
        load(&mut d, Target::Node { id: mid }, LoadKind::Force { fx: 0.0, fy: -20e3, fz: 0.0 }, bend);
        let rep = run(&d.m).unwrap();
        let st = &rep.members[0].checks[0];
        let p = crate::sections::props(&d.m.sections[0].shape).unwrap();
        let (s1, s2) = (200e3 / p.a, 20e3 * 6.0 / 4.0 / p.z());
        let got = st.ratio.unwrap() * d.m.materials[0].fy;
        assert!(close(got, s1.max(s2), 1e-6), "{got} {s1} {s2}");
        assert!(got < 0.99 * (s1 + s2));
        assert!(st.governing.starts_with(if s1 > s2 { "C1" } else { "C2" }), "{st:?}");
        // Buckling: 200 kN against the weak axis's Euler load over 6 m; a brace at 3 m halves its length.
        let b = rep.members[0].checks.iter().find(|c| c.kind == "buckling").unwrap();
        let ncr = PI * PI * d.m.materials[0].e * p.iy / 36.0;
        assert!(close(b.ratio.unwrap(), 200e3 / ncr.min(PI * PI * d.m.materials[0].e * p.i / 36.0), 1e-9), "{b:?}");
        let mut dm = d.m.design_members[0].clone();
        dm.restraints = vec![Restraint { at: 3.0, holds: Holds::Sideways }];
        d.apply(Op::Set { item: Item::DesignMember(dm.clone()) }).unwrap();
        let b2 = run(&d.m).unwrap().members[0].checks[1].ratio.unwrap();
        assert!(close(b2, b.ratio.unwrap() / 4.0, 1e-9), "{b2}");
        // A brace that holds only twisting does not shorten it.
        dm.restraints[0].holds = Holds::Twist;
        d.apply(Op::Set { item: Item::DesignMember(dm) }).unwrap();
        assert!(close(run(&d.m).unwrap().members[0].checks[1].ratio.unwrap(), b.ratio.unwrap(), 1e-9));
        // No yield strength: the stress check says so instead of passing.
        d.m.materials[0].fy = 0.0;
        assert_eq!(run(&d.m).unwrap().members[0].checks[0].status, "missing data");
    }

    #[test]
    fn storey_drift_of_a_cantilever_column() {
        // A 3 m column fixed at the foot, 10 kN across at the top: drift = (PL³/3EI) / L.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        beam(&mut d, [0.0, 0.0], [0.0, 3.0]);
        let (a, b) = (node(&d, 0.0, 0.0), node(&d, 0.0, 3.0));
        support(&mut d, a, SupportKind::Fixed);
        load(&mut d, Target::Node { id: b }, LoadKind::Force { fx: 10e3, fy: 0.0, fz: 0.0 }, 0);
        let rep = run(&d.m).unwrap();
        let p = crate::sections::props(&d.m.sections[0].shape).unwrap();
        let want = 10e3 * 27.0 / (3.0 * d.m.materials[0].e * p.i) / 3.0;
        assert_eq!(rep.storeys.len(), 1);
        assert!(close(rep.storeys[0].dx, want, 1e-6), "{:?} {want}", rep.storeys);
        assert_eq!(rep.storeys[0].status, verdict(want / 0.005));
        assert!(!rep.not_checked.is_empty() && rep.notes.iter().any(|n| n.contains("design members")));
    }
}
