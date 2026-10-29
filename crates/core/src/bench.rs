//! The verification benchmarks (brief §5): models with known answers, solved by Datum Mesh and
//! compared with the reference. Each check is one row of the verification report (Help ›
//! Verification) and a test: `cargo test` fails if any error is over its tolerance.

use crate::doc::{Doc, Item, Op};
use crate::frame;
use crate::model::{Analysis, Dir, Geom, Load, LoadKind, Material, MemberKind, Model, Refine, Section, SectionShape, Shape, Support, SupportKind, Target};
use crate::{heat, plane};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    /// Benchmark number in the brief's table.
    pub bench: u32,
    pub name: &'static str,
    pub quantity: &'static str,
    /// Kind of value for display units: "len", "moment", "force", "stress", "dtemp", "flux",
    /// "flowlen" (W/m), "rad", or "ratio" (a plain number).
    pub q: &'static str,
    pub reference: f64,
    pub result: f64,
    /// |result − reference| / |reference|.
    pub error: f64,
    pub tolerance: f64,
    pub source: &'static str,
    pub pass: bool,
    /// Mesh convergence: (elements, error) for a coarser-to-finer series ending at this mesh.
    pub steps: Vec<[f64; 2]>,
}

fn check(bench: u32, name: &'static str, quantity: &'static str, q: &'static str, reference: f64, result: f64, tolerance: f64, source: &'static str) -> Check {
    let error = (result - reference).abs() / reference.abs().max(1e-300);
    Check { bench, name, quantity, q, reference, result, error, tolerance, source, pass: error <= tolerance && result.is_finite(), steps: vec![] }
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
        check(1, "Cantilever, end load", "Tip rotation PL²/2EI", "rad", p * l * l / (2.0 * e * i), -r.disp[ti][2], 1e-9, src),
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

/// A plane model with one material and the given shapes (solid, hole).
fn part(analysis: Analysis, e: f64, nu: f64, shapes: Vec<(Geom, bool)>) -> (Doc, Vec<u32>) {
    let mut d = Doc::new(Model::new(analysis));
    d.m.materials[0].e = e;
    d.m.materials[0].nu = nu;
    let mut ids = vec![];
    for (g, hole) in shapes {
        ids.push(add_id(&mut d, Item::Shape(Shape { material: 1, geom: g, hole, ..Shape::default() })));
    }
    (d, ids)
}
fn add_id(d: &mut Doc, item: Item) -> u32 {
    d.apply(Op::Add { item }).expect("benchmark model")[0]
}
fn edge_support(d: &mut Doc, shape: u32, edge: u32, kind: SupportKind) {
    add_id(d, Item::Support(Support { id: 0, at: Target::Edge { shape, edge }, kind }));
}
fn edge_load(d: &mut Doc, shape: u32, edge: u32, kind: LoadKind) {
    add_id(d, Item::Load(Load { id: 0, at: Target::Edge { shape, edge }, kind }));
}
/// The value of a nodal field at the mesh node nearest a point.
fn at(r: &plane::PlaneResults, key: &str, p: [f64; 2]) -> f64 {
    let n = r.mesh.nodes.iter().enumerate().min_by(|a, b| (a.1[0] - p[0]).hypot(a.1[1] - p[1]).total_cmp(&(b.1[0] - p[0]).hypot(b.1[1] - p[1]))).map(|x| x.0).unwrap();
    r.field(key).v[n]
}
/// Solve a model at a series of element sizes (coarse to fine) and keep each step's error for one
/// quantity; the check reports the finest.
fn converge(sizes: &[f64], build: impl Fn(f64) -> Doc, value: impl Fn(&plane::PlaneResults) -> f64, reference: f64) -> (f64, Vec<[f64; 2]>) {
    let mut steps = vec![];
    let mut last = f64::NAN;
    for &h in sizes {
        let d = build(h);
        let r = plane::solve(&d.m).expect("benchmark solves");
        last = value(&r);
        steps.push([r.mesh.tris.len() as f64, (last - reference).abs() / reference.abs()]);
    }
    (last, steps)
}
fn with_steps(mut c: Check, steps: Vec<[f64; 2]>) -> Check {
    c.steps = steps;
    c
}

/// Benchmark 4: a plate with a small circular hole pulled in one direction. Kirsch: the stress at
/// the side of the hole is 3σ in an infinite plate. Quarter model of a plate 200 hole diameters
/// wide (the finite width then changes the factor by under 0.01 %), the hole's edge refined.
fn kirsch() -> Vec<Check> {
    let (a, w, s) = (0.01, 4.0, 100e6);
    let build = |h: f64| {
        let quarter = Geom::Poly { pts: vec![[a, 0.0], [w / 2.0, 0.0], [w / 2.0, w / 2.0], [0.0, w / 2.0], [0.0, a]], bulge: vec![0.0, 0.0, 0.0, 0.0, -(std::f64::consts::FRAC_PI_8).tan()] };
        let (mut d, ids) = part(Analysis::Stress, 210e9, 0.3, vec![(quarter, false)]);
        let p = ids[0];
        edge_support(&mut d, p, 0, SupportKind::Symmetry);
        edge_support(&mut d, p, 3, SupportKind::Symmetry);
        edge_load(&mut d, p, 1, LoadKind::Pressure { p: -s });
        d.m.mesh.size = 0.2;
        d.m.mesh.refine.push(Refine { at: Target::Edge { shape: p, edge: 4 }, size: h });
        d
    };
    let (sy, steps) = converge(&[a / 2.0, a / 4.0, a / 8.0, a / 16.0], build, |r| at(r, "sx", [0.0, a]) / s, 3.0);
    vec![with_steps(check(4, "Plate with a hole in tension", "Stress concentration at the hole 3.0", "ratio", 3.0, sy, 1e-2, "Kirsch (1898), infinite plate"), steps)]
}

/// Benchmark 5: a thick cylinder under internal pressure, plane strain, quarter model. Lamé.
fn lame() -> Vec<Check> {
    let (a, b, p, e, nu) = (0.1, 0.2, 100e6, 210e9, 0.3);
    let q = (std::f64::consts::FRAC_PI_8).tan();
    let build = |h: f64| {
        let ring = Geom::Poly { pts: vec![[a, 0.0], [b, 0.0], [0.0, b], [0.0, a]], bulge: vec![0.0, q, 0.0, -q] };
        let (mut d, ids) = part(Analysis::Strain, e, nu, vec![(ring, false)]);
        let r = ids[0];
        edge_support(&mut d, r, 0, SupportKind::Symmetry);
        edge_support(&mut d, r, 2, SupportKind::Symmetry);
        edge_load(&mut d, r, 3, LoadKind::Pressure { p });
        d.m.mesh.size = h;
        d
    };
    let k = a * a / (b * b - a * a);
    let hoop_in = p * k * (1.0 + b * b / (a * a));
    let hoop_out = p * k * 2.0;
    let ur = (1.0 + nu) / e * p * k * ((1.0 - 2.0 * nu) * a + b * b / a);
    let sizes = [0.025, 0.0125, 0.00625];
    let src = "Lamé thick-cylinder equations, plane strain";
    let (v1, s1) = converge(&sizes, build, |r| at(r, "sy", [a, 0.0]), hoop_in);
    let (v2, s2) = converge(&sizes, build, |r| at(r, "sy", [b, 0.0]), hoop_out);
    let (v3, s3) = converge(&sizes, build, |r| at(r, "ux", [a, 0.0]), ur);
    let (v4, s4) = converge(&sizes, build, |r| at(r, "sx", [a, 0.0]), -p);
    vec![
        with_steps(check(5, "Thick cylinder, internal pressure", "Hoop stress at the bore p(b²+a²)/(b²−a²)", "stress", hoop_in, v1, 1e-2, src), s1),
        with_steps(check(5, "Thick cylinder, internal pressure", "Hoop stress outside 2pa²/(b²−a²)", "stress", hoop_out, v2, 1e-2, src), s2),
        with_steps(check(5, "Thick cylinder, internal pressure", "Radial stress at the bore −p", "stress", -p, v4, 1e-2, src), s4),
        with_steps(check(5, "Thick cylinder, internal pressure", "Bore radial movement", "len", ur, v3, 1e-3, src), s3),
    ]
}

/// The bulge of the circular arc from p through m to q.
fn bulge3(p: [f64; 2], m: [f64; 2], q: [f64; 2]) -> f64 {
    // The angle at m between the chord's ends; the arc through m sweeps 2(π − φ).
    let (u, v) = ([p[0] - m[0], p[1] - m[1]], [q[0] - m[0], q[1] - m[1]]);
    let phi = (u[0] * v[1] - u[1] * v[0]).abs().atan2(u[0] * v[0] + u[1] * v[1]);
    // m to the right of p→q: the arc turns counter-clockwise (positive bulge).
    let side = (q[0] - p[0]) * (m[1] - p[1]) - (q[1] - p[1]) * (m[0] - p[0]);
    let b = ((std::f64::consts::PI - phi) / 2.0).tan();
    if side < 0.0 { b } else { -b }
}

/// LE1's quarter, counter-clockwise: D (2, 0) → C (3.25, 0) (edge 0), the outer ellipse to
/// B (0, 2.75) (edges 1..=n), down to A (0, 1) (edge n + 1), the inner ellipse back to D
/// (edges n + 2..=2n + 1); each ellipse as `n` circular arcs through points on it.
fn le1_geom(n: usize) -> Geom {
    let half = std::f64::consts::FRAC_PI_2;
    let ell = |ax: f64, by: f64, t: f64| [ax * t.cos(), by * t.sin()];
    let t = |k: usize| half * k as f64 / n as f64;
    let tm = |k: usize| half * (k as f64 + 0.5) / n as f64;
    let (mut pts, mut bulge) = (vec![[2.0, 0.0]], vec![0.0]);
    for k in 0..n {
        pts.push(ell(3.25, 2.75, t(k)));
        bulge.push(bulge3(ell(3.25, 2.75, t(k)), ell(3.25, 2.75, tm(k)), ell(3.25, 2.75, t(k + 1))));
    }
    pts.push([0.0, 2.75]);
    bulge.push(0.0);
    for k in (0..n).rev() {
        pts.push(ell(2.0, 1.0, t(k + 1)));
        bulge.push(bulge3(ell(2.0, 1.0, t(k + 1)), ell(2.0, 1.0, tm(k)), ell(2.0, 1.0, t(k))));
    }
    Geom::Poly { pts, bulge }
}

/// Benchmark 6: NAFEMS LE1, the elliptic membrane. Plane stress, 0.1 m thick, 10 MPa outward on
/// the outer ellipse; σyy at D (2, 0) is 92.7 MPa.
fn le1() -> Vec<Check> {
    let n = 16;
    let build = |h: f64| {
        let (mut d, ids) = part(Analysis::Stress, 210e9, 0.3, vec![(le1_geom(n), false)]);
        let s = ids[0];
        d.m.thickness = 0.1;
        edge_support(&mut d, s, 0, SupportKind::Symmetry);
        edge_support(&mut d, s, n as u32 + 1, SupportKind::Symmetry);
        for k in 1..=n as u32 {
            edge_load(&mut d, s, k, LoadKind::Pressure { p: -10e6 });
        }
        d.m.mesh.size = h;
        d
    };
    let (v, steps) = converge(&[0.25, 0.125, 0.0625], build, |r| at(r, "sy", [2.0, 0.0]), 92.7e6);
    vec![with_steps(check(6, "NAFEMS LE1, elliptic membrane", "σyy at D (2, 0), 92.7 MPa", "stress", 92.7e6, v, 1e-2, "NAFEMS LE1 (The Standard NAFEMS Benchmarks, 1990)"), steps)]
}

/// Benchmark 7: steady conduction. A plane wall, hot on one face and cooled by a fluid on the
/// other; and a thin fin, its base hot, cooled on both sides, its tip insulated.
fn conduction() -> Vec<Check> {
    let mut out = vec![];
    let (t0, tinf) = (373.15, 293.15);
    let bar = |w: f64, l: f64, k: f64, h: f64, size: f64, fin: bool| {
        let (mut d, ids) = part(Analysis::Heat, 70e9, 0.33, vec![(Geom::rect(0.0, 0.0, l, w), false)]);
        let s = ids[0];
        d.m.materials[0].k = k;
        add_id(&mut d, Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: SupportKind::Temperature { t: t0 } }));
        for e in if fin { vec![0, 2] } else { vec![1] } {
            edge_load(&mut d, s, e, LoadKind::Convection { h, t_inf: tinf });
        }
        d.m.mesh.size = size;
        d
    };
    // Wall: 0.2 m of concrete-like k = 1.4, h = 10. q = ΔT / (L/k + 1/h); the cooled face is q/h above the fluid.
    let (l, k, h) = (0.2, 1.4, 10.0);
    let d = bar(0.05, l, k, h, 0.02, false);
    let r = heat::solve(&d.m).expect("solves");
    let q = (t0 - tinf) / (l / k + 1.0 / h);
    let src = "One-dimensional conduction, thermal resistances in series";
    out.push(check(7, "Plane wall, convection on one face", "Cooled face above the fluid q/h", "dtemp", q / h, r.field("t").min - tinf, 1e-9, src));
    out.push(check(7, "Plane wall, convection on one face", "Heat flux through the wall", "flux", q, r.field("qx").min, 1e-6, src));
    // Fin: aluminium k = 200, 2 mm thick, 50 mm long, h = 25. θ_tip/θ_0 = 1/cosh(mL), m = √(2h/kt).
    let (k, tf, lf, h) = (200.0f64, 0.002, 0.05, 25.0);
    let mm = (2.0 * h / (k * tf)).sqrt();
    let tip = (t0 - tinf) / (mm * lf).cosh();
    let base = k * tf * mm * (t0 - tinf) * (mm * lf).tanh();
    let src = "Fin equation, insulated tip (Incropera, Fundamentals of Heat and Mass Transfer, §3.6)";
    // The fin equation treats the fin as one-dimensional; across 2 mm the temperature varies by
    // about Bi = ht/2k ≈ 1e-4 of the excess, which bounds the agreement.
    let d = bar(tf, lf, k, h, 0.002, true);
    let r = heat::solve(&d.m).expect("solves");
    let tip_v = at_heat(&r, [lf, tf / 2.0]) - tinf;
    let base_v = r.supports[0].q / d.m.thickness;
    out.push(check(7, "Fin with convection, insulated tip", "Tip above the fluid θ₀/cosh(mL)", "dtemp", tip, tip_v, 1e-3, src));
    out.push(check(7, "Fin with convection, insulated tip", "Heat into the base, per metre of depth", "flowlen", base, base_v, 2e-3, src));
    out
}
fn at_heat(r: &heat::HeatResults, p: [f64; 2]) -> f64 {
    let n = r.mesh.nodes.iter().enumerate().min_by(|a, b| (a.1[0] - p[0]).hypot(a.1[1] - p[1]).total_cmp(&(b.1[0] - p[0]).hypot(b.1[1] - p[1]))).map(|x| x.0).unwrap();
    r.field("t").v[n]
}

/// Benchmark 8: patch tests. A constant stress state (50 MPa pull, rollers on two sides) must be
/// reproduced exactly by any mesh of 3- or 6-node triangles; a constant heat flux likewise.
fn patch_tests() -> Vec<Check> {
    let mut out = vec![];
    for (analysis, order, name) in [(Analysis::Stress, 1, "Patch test, 3-node triangles, plane stress"), (Analysis::Stress, 2, "Patch test, 6-node triangles, plane stress"), (Analysis::Strain, 2, "Patch test, 6-node triangles, plane strain")] {
        let (mut d, ids) = part(analysis, 210e9, 0.3, vec![(Geom::rect(0.0, 0.0, 0.2, 0.1), false)]);
        let s = ids[0];
        edge_support(&mut d, s, 3, SupportKind::Roller);
        edge_support(&mut d, s, 0, SupportKind::Roller);
        edge_load(&mut d, s, 1, LoadKind::Pressure { p: -50e6 });
        d.m.mesh.order = order;
        d.m.mesh.size = 0.023;
        let r = plane::solve(&d.m).expect("solves");
        let sx = r.field("sx");
        let worst = if (sx.max - 50e6).abs() > (sx.min - 50e6).abs() { sx.max } else { sx.min };
        out.push(check(8, name, "σx everywhere 50 MPa (the worst node)", "stress", 50e6, worst, 1e-9, "FE theory: constant strain is reproduced exactly"));
    }
    for order in [1, 2] {
        let (mut d, ids) = part(Analysis::Heat, 210e9, 0.3, vec![(Geom::rect(0.0, 0.0, 0.1, 0.03), false)]);
        let s = ids[0];
        add_id(&mut d, Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: SupportKind::Temperature { t: 300.0 } }));
        edge_load(&mut d, s, 1, LoadKind::Flux { q: 1e4 });
        d.m.mesh.order = order;
        d.m.mesh.size = 0.013;
        let r = heat::solve(&d.m).expect("solves");
        let k = d.m.materials[0].k;
        let worst = r.mesh.nodes.iter().enumerate().map(|(n, p)| r.field("t").v[n] - 1e4 * p[0] / k - 300.0).fold(0.0f64, |a, b| if b.abs() > a.abs() { b } else { a });
        let name = if order == 1 { "Patch test, 3-node triangles, heat" } else { "Patch test, 6-node triangles, heat" };
        out.push(check(8, name, "Hot end above the fixed end qL/k", "dtemp", 1e4 * 0.1 / k, 1e4 * 0.1 / k + worst, 1e-9, "FE theory: a linear temperature is reproduced exactly"));
    }
    out
}

/// Benchmark 9 (0.2.0): thermal stress, against hand calculations.
fn thermal() -> Vec<Check> {
    let mut out = vec![];
    let (e, nu, a, dt) = (210e9, 0.3, 12e-6, 50.0);
    let src = "Hand calculation, thermal strain held back";
    let warmed = |analysis: Analysis, sides: &[u32]| {
        let (mut d, ids) = part(analysis, e, nu, vec![(Geom::rect(0.0, 0.0, 0.2, 0.05), false)]);
        d.m.materials[0].alpha = a;
        for &k in sides {
            edge_support(&mut d, ids[0], k, SupportKind::Roller);
        }
        add_id(&mut d, Item::Load(Load { id: 0, at: Target::Body, kind: LoadKind::Warming { dt } }));
        d.m.mesh.size = 0.02;
        plane::solve(&d.m).expect("solves")
    };
    // A bar held at both ends, free to grow across: σx = −EαΔT, σy = 0.
    let r = warmed(Analysis::Stress, &[3, 1, 0]);
    out.push(check(9, "Bar held at both ends, warmed 50 K", "σx = −EαΔT", "stress", -e * a * dt, r.field("sx").min, 1e-9, src));
    // A block held on every side in plane strain: σx = σy = σz = −EαΔT/(1 − 2ν).
    let r = warmed(Analysis::Strain, &[0, 1, 2, 3]);
    out.push(check(9, "Block held on every side, plane strain, warmed 50 K", "σx = −EαΔT/(1 − 2ν)", "stress", -e * a * dt / (1.0 - 2.0 * nu), r.field("sx").max, 1e-9, src));
    out.push(check(9, "Block held on every side, plane strain, warmed 50 K", "σz = −EαΔT/(1 − 2ν)", "stress", -e * a * dt / (1.0 - 2.0 * nu), r.field("sz").max, 1e-9, src));
    // A free plate with a temperature varying linearly through it has no stress at all: the
    // temperature comes from fixed temperatures on two edges (a heat solve on the same mesh).
    let (mut d, ids) = part(Analysis::Stress, e, nu, vec![(Geom::rect(0.0, 0.0, 0.2, 0.05), false)]);
    d.m.materials[0].alpha = a;
    add_id(&mut d, Item::Support(Support { id: 0, at: Target::Edge { shape: ids[0], edge: 0 }, kind: SupportKind::Temperature { t: 393.15 } }));
    add_id(&mut d, Item::Support(Support { id: 0, at: Target::Edge { shape: ids[0], edge: 2 }, kind: SupportKind::Temperature { t: 293.15 } }));
    add_id(&mut d, Item::Support(Support { id: 0, at: Target::Point { x: 0.0, y: 0.0 }, kind: SupportKind::Pinned }));
    add_id(&mut d, Item::Support(Support { id: 0, at: Target::Point { x: 0.2, y: 0.0 }, kind: SupportKind::RollerX }));
    d.m.mesh.size = 0.01;
    let r = plane::solve(&d.m).expect("solves");
    let worst = r.field("vm").max / (e * a * 100.0);
    out.push(check(9, "Free plate, temperature linear through it (100 K)", "No stress: 1 + largest σvm/EαΔT", "ratio", 1.0, 1.0 + worst, 1e-9, "Thermoelasticity: a linear temperature leaves a free plate unstressed"));
    // A frame member fixed at both ends, warmed: N = −EAαΔT.
    let mut f = frame(e, SectionShape::Rect { b: 0.1, h: 0.2 });
    f.m.materials[0].alpha = a;
    member(&mut f, [0.0, 0.0], [4.0, 0.0], MemberKind::Beam);
    support(&mut f, 0.0, 0.0, SupportKind::Fixed);
    support(&mut f, 4.0, 0.0, SupportKind::Fixed);
    add(&mut f, Op::Add { item: Item::Load(Load { id: 0, at: Target::Body, kind: LoadKind::Warming { dt } }) });
    let r = frame::solve(&f.m).expect("solves");
    out.push(check(9, "Beam fixed at both ends, warmed 50 K", "Axial force N = −EAαΔT", "force", -e * 0.02 * a * dt, r.members[0].n[0], 1e-9, src));
    out
}

/// Benchmark 10 (0.2.1): axisymmetric solids, drawn as a half-section right of the axis.
fn axisymmetric() -> Vec<Check> {
    let mut out = vec![];
    let (e, nu) = (210e9, 0.3);
    // A long thick cylinder, its ends held from moving along the axis (plane strain): Lamé.
    let (a, b, p) = (0.1, 0.2, 100e6);
    let build = |h: f64| {
        let (mut d, ids) = part(Analysis::Axi, e, nu, vec![(Geom::rect(a, 0.0, b, 0.05), false)]);
        edge_support(&mut d, ids[0], 0, SupportKind::Roller);
        edge_support(&mut d, ids[0], 2, SupportKind::Roller);
        edge_load(&mut d, ids[0], 3, LoadKind::Pressure { p });
        d.m.mesh.size = h;
        d
    };
    let k = a * a / (b * b - a * a);
    let hoop = p * k * (1.0 + b * b / (a * a));
    let (v, steps) = converge(&[0.025, 0.0125, 0.00625], build, |r| at(r, "sz", [a, 0.025]), hoop);
    out.push(with_steps(check(10, "Long thick cylinder, axisymmetric, internal pressure", "Hoop stress σθ at the bore (Lamé)", "stress", hoop, v, 1e-2, "Lamé thick-cylinder equations"), steps));
    // A thick hollow sphere under internal pressure: a quarter ring turned about the y axis.
    let q = (std::f64::consts::FRAC_PI_8).tan();
    let build = |h: f64| {
        let ring = Geom::Poly { pts: vec![[a, 0.0], [b, 0.0], [0.0, b], [0.0, a]], bulge: vec![0.0, q, 0.0, -q] };
        let (mut d, ids) = part(Analysis::Axi, e, nu, vec![(ring, false)]);
        edge_support(&mut d, ids[0], 0, SupportKind::Symmetry);
        edge_load(&mut d, ids[0], 3, LoadKind::Pressure { p });
        d.m.mesh.size = h;
        d
    };
    let c3 = b.powi(3) - a.powi(3);
    let hoop = p * (b.powi(3) + 2.0 * a.powi(3)) / (2.0 * c3);
    let u = p * a.powi(3) / (e * c3) * ((1.0 - 2.0 * nu) * a + (1.0 + nu) * b.powi(3) / (2.0 * a * a));
    let src = "Lamé thick-sphere equations (Timoshenko and Goodier, Theory of Elasticity, §137)";
    let (v, steps) = converge(&[0.04, 0.02, 0.01], build, |r| at(r, "sz", [a, 0.0]), hoop);
    out.push(with_steps(check(10, "Thick hollow sphere, internal pressure", "Hoop stress at the inside p(b³ + 2a³)/2(b³ − a³)", "stress", hoop, v, 1e-2, src), steps));
    let (v, steps) = converge(&[0.04, 0.02, 0.01], build, |r| at(r, "ux", [a, 0.0]), u);
    out.push(with_steps(check(10, "Thick hollow sphere, internal pressure", "Growth of the inside radius", "len", u, v, 1e-3, src), steps));
    // A solid round bar pulled along its axis: σ along it everywhere, no hoop stress, and the
    // radius shrinks by νσR/E. Exact for any mesh (a patch test with the axis in it).
    let (r0, s) = (0.03, 80e6);
    let (mut d, ids) = part(Analysis::Axi, e, nu, vec![(Geom::rect(0.0, 0.0, r0, 0.1), false)]);
    edge_support(&mut d, ids[0], 0, SupportKind::Roller);
    edge_load(&mut d, ids[0], 2, LoadKind::Pressure { p: -s });
    d.m.mesh.size = 0.011;
    let r = plane::solve(&d.m).expect("solves");
    let src = "FE theory: constant stress is reproduced exactly";
    out.push(check(10, "Round bar pulled along its axis (patch test)", "σ along the axis, the worst node", "stress", s, if (r.field("sy").max - s).abs() > (r.field("sy").min - s).abs() { r.field("sy").max } else { r.field("sy").min }, 1e-9, src));
    out.push(check(10, "Round bar pulled along its axis (patch test)", "Radius shrinks by νσR/E", "len", -nu * s * r0 / e, r.field("ux").min, 1e-9, src));
    // A free cylinder warmed evenly: it grows by αΔT R and has no stress.
    let (mut d, ids) = part(Analysis::Axi, e, nu, vec![(Geom::rect(0.05, 0.0, 0.08, 0.1), false)]);
    d.m.materials[0].alpha = 12e-6;
    edge_support(&mut d, ids[0], 0, SupportKind::Roller);
    add_id(&mut d, Item::Load(Load { id: 0, at: Target::Body, kind: LoadKind::Warming { dt: 80.0 } }));
    d.m.mesh.size = 0.01;
    let r = plane::solve(&d.m).expect("solves");
    let src = "Thermoelasticity: free, even warming";
    out.push(check(10, "Free tube warmed 80 K, axisymmetric", "Outside radius grows by αΔT R", "len", 12e-6 * 80.0 * 0.08, r.field("ux").max, 1e-9, src));
    out.push(check(10, "Free tube warmed 80 K, axisymmetric", "No stress: 1 + largest σvm/EαΔT", "ratio", 1.0, 1.0 + r.field("vm").max / (e * 12e-6 * 80.0), 1e-9, src));
    out
}

/// Every check, in the order of the brief's table.
pub fn run() -> Vec<Check> {
    let mut v = cantilever();
    v.extend(simply_supported());
    v.extend(trusses());
    v.extend(kirsch());
    v.extend(lame());
    v.extend(le1());
    v.extend(conduction());
    v.extend(patch_tests());
    v.extend(thermal());
    v.extend(axisymmetric());
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
            // Convergence: finer meshes, and the error falls from the coarsest to the finest.
            if let (Some(a), Some(b)) = (r.steps.first(), r.steps.last()) {
                assert!(r.steps.windows(2).all(|w| w[1][0] > w[0][0]), "{}: {:?}", r.quantity, r.steps);
                assert!(b[1] < a[1], "{}: {:?}", r.quantity, r.steps);
            }
        }
        for b in 1..=10 {
            assert!(rows.iter().any(|r| r.bench == b), "benchmark {b} has no rows");
        }
    }
}
