//! The verification benchmarks (brief §5): models with known answers, solved by Datum Mesh and
//! compared with the reference. Each check is one row of the verification report (Help ›
//! Verification) and a test: `cargo test` fails if any error is over its tolerance.

use crate::doc::{Doc, Item, Op};
use crate::frame;
use crate::model::{Analysis, Dir, Load, LoadKind, Material, MemberKind, Model, Section, SectionShape, Support, SupportKind, Target};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    /// Benchmark number in the brief's table.
    pub bench: u32,
    pub name: &'static str,
    pub quantity: &'static str,
    /// Kind of value for display units: "len", "moment", "force", "stress", "temp".
    pub q: &'static str,
    pub reference: f64,
    pub result: f64,
    /// |result − reference| / |reference|.
    pub error: f64,
    pub tolerance: f64,
    pub source: &'static str,
    pub pass: bool,
}

fn check(bench: u32, name: &'static str, quantity: &'static str, q: &'static str, reference: f64, result: f64, tolerance: f64, source: &'static str) -> Check {
    let error = (result - reference).abs() / reference.abs().max(1e-300);
    Check { bench, name, quantity, q, reference, result, error, tolerance, source, pass: error <= tolerance && result.is_finite() }
}

/// A frame model with one material (E, no weight) and one section.
fn frame(e: f64, shape: SectionShape) -> Doc {
    let mut d = Doc::new(Model::new(Analysis::Frame));
    let mut mt = Material::steel(1);
    mt.e = e;
    d.m.materials = vec![mt];
    d.m.sections = vec![Section { id: 2, name: "S".into(), shape }];
    d
}
fn node(d: &Doc, x: f64, y: f64) -> u32 {
    d.m.nodes.iter().find(|n| (n.x - x).abs() < 1e-9 && (n.y - y).abs() < 1e-9).map(|n| n.id).expect("node")
}
fn add(d: &mut Doc, op: Op) {
    d.apply(op).expect("benchmark model");
}
fn member(d: &mut Doc, a: [f64; 2], b: [f64; 2], kind: MemberKind) {
    add(d, Op::AddMember { from: a, to: b, kind, section: None, material: None });
}
fn support(d: &mut Doc, x: f64, y: f64, kind: SupportKind) {
    let id = node(d, x, y);
    add(d, Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind }) });
}
fn load(d: &mut Doc, at: Target, kind: LoadKind) {
    add(d, Op::Add { item: Item::Load(Load { id: 0, at, kind }) });
}

/// Benchmark 1: a cantilever with an end load. Beam theory: tip deflection PL³/3EI, root moment PL.
fn cantilever() -> Vec<Check> {
    let (l, p, e) = (3.0, 10e3, 200e9);
    let (b, h) = (0.1, 0.2);
    let i = b * h * h * h / 12.0;
    let mut d = frame(e, SectionShape::Rect { b, h });
    for k in 0..4 {
        member(&mut d, [l * k as f64 / 4.0, 0.0], [l * (k + 1) as f64 / 4.0, 0.0], MemberKind::Beam);
    }
    support(&mut d, 0.0, 0.0, SupportKind::Fixed);
    let tip = node(&d, l, 0.0);
    load(&mut d, Target::Node { id: tip }, LoadKind::Force { fx: 0.0, fy: -p });
    let r = frame::solve(&d.m).expect("solves");
    let ti = d.m.nodes.iter().position(|n| n.id == tip).unwrap();
    let root = r.members.iter().find(|m| m.x.len() > 1 && d.m.members.iter().any(|mb| mb.id == m.id && (mb.a == node(&d, 0.0, 0.0) || mb.b == node(&d, 0.0, 0.0)))).unwrap();
    let src = "Beam theory (Euler–Bernoulli)";
    vec![
        check(1, "Cantilever, end load", "Tip deflection PL³/3EI", "len", p * l.powi(3) / (3.0 * e * i), -r.disp[ti][1], 1e-9, src),
        check(1, "Cantilever, end load", "Root moment PL (hogging)", "moment", p * l, -root.m[0], 1e-9, src),
        check(1, "Cantilever, end load", "Tip rotation PL²/2EI", "ratio", p * l * l / (2.0 * e * i), -r.disp[ti][2], 1e-9, src),
    ]
}

/// Benchmark 2: a simply supported beam under a uniform load: 5wL⁴/384EI at midspan, wL²/8.
fn simply_supported() -> Vec<Check> {
    let (l, w, e) = (6.0, 5e3, 200e9);
    let (b, h) = (0.1, 0.25f64);
    let i = b * h.powi(3) / 12.0;
    let mut d = frame(e, SectionShape::Rect { b, h });
    member(&mut d, [0.0, 0.0], [l / 2.0, 0.0], MemberKind::Beam);
    member(&mut d, [l / 2.0, 0.0], [l, 0.0], MemberKind::Beam);
    support(&mut d, 0.0, 0.0, SupportKind::Pinned);
    support(&mut d, l, 0.0, SupportKind::RollerX);
    for mb in d.m.members.clone() {
        load(&mut d, Target::Member { id: mb.id }, LoadKind::Dist { w1: -w, w2: -w, dir: Dir::Y });
    }
    let r = frame::solve(&d.m).expect("solves");
    let mid = d.m.nodes.iter().position(|n| (n.x - l / 2.0).abs() < 1e-9).unwrap();
    let mmax = r.members.iter().flat_map(|m| m.m.iter().copied()).fold(f64::MIN, f64::max);
    // A single member: the deflected shape inside the member must carry the load's curve too.
    let mut d1 = frame(e, SectionShape::Rect { b, h });
    member(&mut d1, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
    support(&mut d1, 0.0, 0.0, SupportKind::Pinned);
    support(&mut d1, l, 0.0, SupportKind::RollerX);
    let id = d1.m.members[0].id;
    load(&mut d1, Target::Member { id }, LoadKind::Dist { w1: -w, w2: -w, dir: Dir::Y });
    let r1 = frame::solve(&d1.m).expect("solves");
    let one = &r1.members[0];
    let src = "Beam theory (Euler–Bernoulli)";
    let reaction = r.reactions.iter().map(|x| x.fy).fold(0.0, f64::max);
    vec![
        check(2, "Simply supported beam, uniform load", "Midspan deflection 5wL⁴/384EI", "len", 5.0 * w * l.powi(4) / (384.0 * e * i), -r.disp[mid][1], 1e-9, src),
        check(2, "Simply supported beam, uniform load", "Largest moment wL²/8", "moment", w * l * l / 8.0, mmax, 1e-9, src),
        check(2, "Simply supported beam, uniform load", "Each reaction wL/2", "force", w * l / 2.0, reaction, 1e-9, src),
        check(2, "Simply supported beam, uniform load", "Midspan deflection inside one member", "len", 5.0 * w * l.powi(4) / (384.0 * e * i), -one.d[frame::STATIONS / 2][1], 1e-9, src),
    ]
}

/// Benchmark 3: statically determinate trusses, worked by hand in the test (method of joints and
/// method of sections).
fn trusses() -> Vec<Check> {
    let mut out = vec![];
    // A triangle: A (0, 0) pinned, B (4, 0) on a roller, C (2, 3); 10 kN down at C.
    // Joint A: reactions 5 kN up each; tan α = 3/2, sin α = 3/√13, cos α = 2/√13.
    // ΣFy at A: 5 + N_AC sin α = 0 → N_AC = −5√13/3 (compression); ΣFx: N_AB + N_AC cos α = 0 → N_AB = 10/3 kN.
    let mut d = frame(200e9, SectionShape::Rect { b: 0.05, h: 0.05 });
    for (a, b) in [([0.0, 0.0], [4.0, 0.0]), ([0.0, 0.0], [2.0, 3.0]), ([4.0, 0.0], [2.0, 3.0])] {
        member(&mut d, a, b, MemberKind::Truss);
    }
    support(&mut d, 0.0, 0.0, SupportKind::Pinned);
    support(&mut d, 4.0, 0.0, SupportKind::RollerX);
    let c = node(&d, 2.0, 3.0);
    load(&mut d, Target::Node { id: c }, LoadKind::Force { fx: 0.0, fy: -10e3 });
    let r = frame::solve(&d.m).expect("solves");
    let axial = |d: &Doc, r: &frame::FrameResults, a: [f64; 2], b: [f64; 2]| {
        let (na, nb) = (node(d, a[0], a[1]), node(d, b[0], b[1]));
        let id = d.m.members.iter().find(|m| (m.a == na && m.b == nb) || (m.a == nb && m.b == na)).unwrap().id;
        r.members.iter().find(|m| m.id == id).unwrap().n[0]
    };
    let src = "Hand calculation, method of joints";
    out.push(check(3, "Triangle truss, apex load", "Bottom chord force 10/3 kN (tension)", "force", 10e3 / 3.0, axial(&d, &r, [0.0, 0.0], [4.0, 0.0]), 1e-9, src));
    out.push(check(3, "Triangle truss, apex load", "Rafter force −5√13/3 kN (compression)", "force", -5e3 * 13f64.sqrt() / 3.0, axial(&d, &r, [0.0, 0.0], [2.0, 3.0]), 1e-9, src));
    // The roof truss example (Pratt, 12 m span, 2 m deep, 10 kN at the five top joints).
    // Reactions 25 kN. Section through the middle panel: moments about the top joint at x = 4:
    // 25·4 − 10·2 = 80 kN·m → bottom chord 80/2 = 40 kN tension. About the bottom joint at x = 6:
    // 25·6 − 10·(4 + 2) = 90 kN·m → top chord 90/2 = 45 kN compression.
    let t = crate::examples::get("truss").expect("example");
    let dt = Doc::new(t);
    let rt = frame::solve(&dt.m).expect("solves");
    let src = "Hand calculation, method of sections";
    out.push(check(3, "Pratt roof truss, 12 m", "Bottom chord, middle panel, 40 kN", "force", 40e3, axial(&dt, &rt, [4.0, 0.0], [6.0, 0.0]), 1e-9, src));
    out.push(check(3, "Pratt roof truss, 12 m", "Top chord, middle panel, −45 kN", "force", -45e3, axial(&dt, &rt, [4.0, 2.0], [6.0, 2.0]), 1e-9, src));
    out.push(check(3, "Pratt roof truss, 12 m", "Each reaction, 25 kN", "force", 25e3, rt.reactions.iter().map(|x| x.fy).fold(0.0, f64::max), 1e-9, "Statics"));
    out
}

/// Every check, in the order of the brief's table.
pub fn run() -> Vec<Check> {
    let mut v = cantilever();
    v.extend(simply_supported());
    v.extend(trusses());
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_benchmark_is_within_its_tolerance() {
        let rows = super::run();
        assert!(rows.len() >= 12);
        for r in &rows {
            assert!(r.pass, "benchmark {} {}: {} gave {} against {} (error {:.2e})", r.bench, r.name, r.quantity, r.result, r.reference, r.error);
        }
    }
}
