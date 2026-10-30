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
    add(d, Op::AddMember { from: a, to: b, kind, section: None, material: None, z: [0.0; 2] });
}
fn support(d: &mut Doc, x: f64, y: f64, kind: SupportKind) {
    let id = node(d, x, y);
    add(d, Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind }) });
}
fn load(d: &mut Doc, at: Target, kind: LoadKind) {
    add(d, Op::Add { item: Item::Load(Load { case: 0, id: 0, at, kind }) });
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
    load(&mut d, Target::Node { id: tip }, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
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
    load(&mut d, Target::Node { id: c }, LoadKind::Force { fx: 0.0, fy: -10e3, fz: 0.0 });
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
    add_id(d, Item::Load(Load { case: 0, id: 0, at: Target::Edge { shape, edge }, kind }));
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
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Warming { dt } }));
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
    add(&mut f, Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Warming { dt } }) });
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
    add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Warming { dt: 80.0 } }));
    d.m.mesh.size = 0.01;
    let r = plane::solve(&d.m).expect("solves");
    let src = "Thermoelasticity: free, even warming";
    out.push(check(10, "Free tube warmed 80 K, axisymmetric", "Outside radius grows by αΔT R", "len", 12e-6 * 80.0 * 0.08, r.field("ux").max, 1e-9, src));
    out.push(check(10, "Free tube warmed 80 K, axisymmetric", "No stress: 1 + largest σvm/EαΔT", "ratio", 1.0, 1.0 + r.field("vm").max / (e * 12e-6 * 80.0), 1e-9, src));
    out
}

/// Benchmark 11 (0.2.2): natural frequencies of beams, against Euler–Bernoulli theory:
/// f = (βL)² / 2πL² · √(EI/ρA).
fn frequencies() -> Vec<Check> {
    let mut out = vec![];
    let (l, e, rho) = (2.0, 200e9, 7850.0);
    let (b, h) = (0.05, 0.1f64);
    let (a, i) = (b * h, b * h.powi(3) / 12.0);
    let base = (e * i / (rho * a)).sqrt() / (std::f64::consts::TAU * l * l);
    let beam = |ends: [Option<SupportKind>; 2], pieces: usize| {
        let mut d = frame(e, SectionShape::Rect { b, h });
        d.m.materials[0].rho = rho;
        for k in 0..pieces {
            member(&mut d, [l * k as f64 / pieces as f64, 0.0], [l * (k + 1) as f64 / pieces as f64, 0.0], MemberKind::Beam);
        }
        for (x, s) in [(0.0, &ends[0]), (l, &ends[1])] {
            if let Some(s) = s {
                support(&mut d, x, 0.0, s.clone());
            }
        }
        frame::modes(&d.m, 3).expect("modes")
    };
    let src = "Euler–Bernoulli beam vibration (Blevins, Formulas for Natural Frequency and Mode Shape, table 8-1)";
    let fm = beam([Some(SupportKind::Fixed), None], 10);
    for (k, bl) in [1.875_104_068_711_961, 4.694_091_132_974_175, 7.854_757_438_237_613].iter().enumerate() {
        out.push(check(11, "Cantilever, 10 beam elements", ["First natural frequency (βL = 1.8751)", "Second (βL = 4.6941)", "Third (βL = 7.8548)"][k], "hz", bl * bl * base, fm.modes[k].f, 1e-3, src));
    }
    let fm = beam([Some(SupportKind::Pinned), Some(SupportKind::RollerX)], 10);
    for k in 0..3 {
        let n = (k + 1) as f64;
        out.push(check(11, "Simply supported beam, 10 beam elements", ["First natural frequency (βL = π)", "Second (βL = 2π)", "Third (βL = 3π)"][k], "hz", (n * std::f64::consts::PI).powi(2) * base, fm.modes[k].f, 1e-3, src));
    }
    // A slender strip 1 m × 50 mm, plane stress, ν = 0, fixed along one end: its first bending
    // frequency against beam theory (shear and rotary inertia lower it by about 0.3 % at L/h = 20),
    // and its first stretching mode against the bar's f = √(E/ρ) / 4L.
    let (sl, sh) = (1.0, 0.05f64);
    let (mut d, ids) = part(Analysis::Stress, e, 0.0, vec![(Geom::rect(0.0, 0.0, sl, sh), false)]);
    d.m.materials[0].rho = rho;
    edge_support(&mut d, ids[0], 3, SupportKind::Fixed);
    d.m.mesh.size = sh / 4.0;
    let pm = plane::modes(&d.m, 6).expect("modes");
    let bend = 1.875_104_068_711_961f64.powi(2) / (std::f64::consts::TAU * sl * sl) * (e * sh * sh / (12.0 * rho)).sqrt();
    out.push(check(11, "Plate strip as a cantilever, plane stress, ν = 0", "First bending frequency (beam theory)", "hz", bend, pm.modes[0].f, 1e-2, "Euler–Bernoulli beam vibration; the plate adds shear and rotary inertia (about −0.3 %)"));
    let axial = pm.modes.iter().find(|md| { let (sx, sy) = md.disp.iter().fold((0.0, 0.0), |a, u| (a.0 + u[0] * u[0], a.1 + u[1] * u[1])); sx > 10.0 * sy }).map(|md| md.f).unwrap_or(f64::NAN);
    out.push(check(11, "Plate strip as a cantilever, plane stress, ν = 0", "First stretching frequency √(E/ρ) / 4L", "hz", (e / rho).sqrt() / (4.0 * sl), axial, 1e-3, "Longitudinal vibration of a bar fixed at one end"));
    out
}

/// Benchmark 12 (0.2.3): buckling of a column drawn as one member, against Euler's loads for four
/// ways of holding its ends, a column held at both ends and warmed, and a truss bar on its own.
fn buckling() -> Vec<Check> {
    let mut out = vec![];
    let (l, e, p) = (3.0, 200e9, 10e3);
    let (b, h) = (0.05, 0.1f64);
    let ei = e * b * h.powi(3) / 12.0;
    let euler = std::f64::consts::PI.powi(2) * ei / (l * l);
    let column = |bottom: SupportKind, top: Option<SupportKind>, kind: MemberKind, warm: f64| {
        let mut d = frame(e, SectionShape::Rect { b, h });
        member(&mut d, [0.0, 0.0], [0.0, l], kind);
        support(&mut d, 0.0, 0.0, bottom);
        if let Some(t) = top {
            support(&mut d, 0.0, l, t);
        }
        if warm != 0.0 {
            add(&mut d, Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Warming { dt: warm } }) });
        } else {
            let top = node(&d, 0.0, l);
            load(&mut d, Target::Node { id: top }, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
        }
        frame::buckling(&d.m, 2).expect("buckling")
    };
    let src = "Euler buckling loads (Timoshenko & Gere, Theory of Elastic Stability, §2.1–2.5)";
    // Sliding but not turning at the top: stiff springs across and in rotation, none along.
    let slide = SupportKind::Spring { kx: 1e15, ky: 0.0, kr: 1e15, kz: 0.0, krx: 0.0, kry: 0.0 };
    let cases = [
        ("Column pinned at both ends, one member", "Load factor π²EI/L² / P", SupportKind::Pinned, Some(SupportKind::RollerY), 1.0),
        ("Column fixed at the foot, free at the top (flagpole)", "Load factor π²EI/4L² / P", SupportKind::Fixed, None, 0.25),
        ("Column fixed at the foot, pinned at the top", "Load factor 20.19 EI/L² / P", SupportKind::Fixed, Some(SupportKind::RollerY), 4.493_409_457_909_064f64.powi(2) / std::f64::consts::PI.powi(2)),
        ("Column fixed at both ends (top slides along its axis)", "Load factor 4π²EI/L² / P", SupportKind::Fixed, Some(slide), 4.0),
    ];
    for (name, q, bottom, top, k) in cases {
        let r = column(bottom, top, MemberKind::Beam, 0.0);
        out.push(check(12, name, q, "ratio", k * euler / p, r.modes[0].lambda, 1e-3, src));
    }
    // Pinned–pinned again: the second mode is the S-shape at four times the first.
    let r = column(SupportKind::Pinned, Some(SupportKind::RollerY), MemberKind::Beam, 0.0);
    out.push(check(12, "Column pinned at both ends, one member", "Second load factor 4π²EI/L² / P", "ratio", 4.0 * euler / p, r.modes[1].lambda, 1e-3, src));
    // Held at both ends and warmed 10 K: N = EAαΔT, so λ = 4π²EI/L² / EAαΔT.
    let r = column(SupportKind::Fixed, Some(SupportKind::Fixed), MemberKind::Beam, 10.0);
    out.push(check(12, "Column fixed at both ends, warmed 10 K", "Load factor 4π²EI/L² / EAαΔT", "ratio", 4.0 * euler / (e * b * h * 12e-6 * 10.0), r.modes[0].lambda, 1e-3, "Thermal buckling of a restrained bar (Timoshenko & Gere §2.1)"));
    // A truss bar pinned at both ends is checked on its own as a strut.
    let r = column(SupportKind::Pinned, Some(SupportKind::RollerY), MemberKind::Truss, 0.0);
    out.push(check(12, "Truss bar pinned at both ends", "Its own load factor π²EI/L² / P", "ratio", euler / p, r.bars.first().map(|b| b.lambda).unwrap_or(f64::NAN), 1e-9, src));
    out
}

/// A 3D frame model with one material (E, ν = 0.3, no weight) and one section.
fn space_frame(e: f64, shape: SectionShape) -> Doc {
    let mut d = frame(e, shape);
    d.m.analysis = Analysis::Space;
    d
}
fn member3(d: &mut Doc, a: [f64; 3], b: [f64; 3], kind: MemberKind) {
    add(d, Op::AddMember { from: [a[0], a[1]], to: [b[0], b[1]], kind, section: None, material: None, z: [a[2], b[2]] });
}
fn node3(d: &Doc, p: [f64; 3]) -> u32 {
    d.m.nodes.iter().find(|n| crate::model::dist3(n.at(), p) < 1e-9).map(|n| n.id).expect("node")
}
fn support3(d: &mut Doc, p: [f64; 3], kind: SupportKind) {
    let id = node3(d, p);
    add(d, Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind }) });
}

/// Benchmark 13 (0.2.4): 3D frames and trusses against hand answers.
fn space() -> Vec<Check> {
    let mut out = vec![];
    let (l, p, e) = (2.0, 1e3, 200e9);
    let (b, h) = (0.05, 0.1);
    let pr = crate::sections::props(&SectionShape::Rect { b, h }).unwrap();
    let g = e / (2.0 * (1.0 + Material::steel(1).nu));
    let src = "Beam theory: tip deflection PL³/3EI, twist TL/GJ";
    // A cantilever along x, loaded at the tip one way at a time.
    let cant = |roll: f64, tip: LoadKind| {
        let mut d = space_frame(e, SectionShape::Rect { b, h });
        member3(&mut d, [0.0; 3], [l, 0.0, 0.0], MemberKind::Beam);
        d.m.members[0].roll = roll;
        support3(&mut d, [0.0; 3], SupportKind::Fixed);
        let id = node3(&d, [l, 0.0, 0.0]);
        load(&mut d, Target::Node { id }, tip);
        crate::space::solve(&d.m).expect("space")
    };
    let r = cant(0.0, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
    out.push(check(13, "3D cantilever along x", "Tip deflection in y, PL³/3EI (strong axis)", "len", -p * l.powi(3) / (3.0 * e * pr.i), r.disp[1][1], 1e-9, src));
    let r = cant(0.0, LoadKind::Force { fx: 0.0, fy: 0.0, fz: p });
    out.push(check(13, "3D cantilever along x", "Tip deflection in z, PL³/3EIy (weak axis)", "len", p * l.powi(3) / (3.0 * e * pr.iy), r.disp[1][2], 1e-9, src));
    out.push(check(13, "3D cantilever along x", "Root moment about y, PL", "moment", p * l, r.members[0].my[0].abs(), 1e-9, src));
    let r = cant(0.0, LoadKind::Moment { m: 0.0, mx: p, my: 0.0 });
    out.push(check(13, "3D cantilever along x", "Tip twist under a torque, TL/GJ", "rad", p * l / (g * pr.j), r.disp[1][3], 1e-9, src));
    let r = cant(std::f64::consts::FRAC_PI_2, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
    out.push(check(13, "3D cantilever along x, section rolled 90°", "Tip deflection in y, PL³/3EIy (now the weak axis)", "len", -p * l.powi(3) / (3.0 * e * pr.iy), r.disp[1][1], 1e-9, src));
    // The same cantilever under a uniform load along z: tip qL⁴/8EIy, root moment qL²/2; and a
    // load rising from zero along y: tip 11qL⁴/120EI.
    let mut d = space_frame(e, SectionShape::Rect { b, h });
    member3(&mut d, [0.0; 3], [l, 0.0, 0.0], MemberKind::Beam);
    support3(&mut d, [0.0; 3], SupportKind::Fixed);
    let mid = d.m.members[0].id;
    load(&mut d, Target::Member { id: mid }, LoadKind::Dist { w1: p, w2: p, dir: Dir::Z });
    load(&mut d, Target::Member { id: mid }, LoadKind::Dist { w1: 0.0, w2: -p, dir: Dir::Y });
    let r = crate::space::solve(&d.m).expect("space");
    out.push(check(13, "3D cantilever along x, member loads", "Tip deflection in z under uniform q, qL⁴/8EIy", "len", p * l.powi(4) / (8.0 * e * pr.iy), r.disp[1][2], 1e-9, src));
    out.push(check(13, "3D cantilever along x, member loads", "Root moment about y, qL²/2", "moment", p * l * l / 2.0, r.members[0].my[0].abs(), 1e-9, src));
    out.push(check(13, "3D cantilever along x, member loads", "Tip deflection in y under a load rising to q, 11qL⁴/120EI", "len", -11.0 * p * l.powi(4) / (120.0 * e * pr.i), r.disp[1][1], 1e-9, "Beam theory (Roark, table 8.1 case 2)"));
    let tipd = r.members[0].d.last().unwrap();
    out.push(check(13, "3D cantilever along x, member loads", "Deflection drawn at the tip meets the node, in z", "len", r.disp[1][2], tipd[2], 1e-9, "The diagram's own integration"));
    // A cantilever running along z (into the depth): its depth stands upright.
    let mut d = space_frame(e, SectionShape::Rect { b, h });
    member3(&mut d, [0.0; 3], [0.0, 0.0, l], MemberKind::Beam);
    support3(&mut d, [0.0; 3], SupportKind::Fixed);
    let id = node3(&d, [0.0, 0.0, l]);
    load(&mut d, Target::Node { id }, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
    let r = crate::space::solve(&d.m).expect("space");
    out.push(check(13, "3D cantilever along z", "Tip deflection in y, PL³/3EI (depth upright)", "len", -p * l.powi(3) / (3.0 * e * pr.i), r.disp[1][1], 1e-9, src));
    // An L-shaped cantilever in plan: a along x from the fixed end, then b along z, with P down at
    // the free end. δ = Pa³/3EI + Pb³/3EI + Pb²a/GJ (the first arm twists under P·b).
    let (a, bb) = (2.0, 1.0);
    let mut d = space_frame(e, SectionShape::Rect { b, h });
    member3(&mut d, [0.0; 3], [a, 0.0, 0.0], MemberKind::Beam);
    member3(&mut d, [a, 0.0, 0.0], [a, 0.0, bb], MemberKind::Beam);
    support3(&mut d, [0.0; 3], SupportKind::Fixed);
    let id = node3(&d, [a, 0.0, bb]);
    load(&mut d, Target::Node { id }, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
    let r = crate::space::solve(&d.m).expect("space");
    let tip = d.m.nodes.iter().position(|n| n.id == id).unwrap();
    let delta = p * a.powi(3) / (3.0 * e * pr.i) + p * bb.powi(3) / (3.0 * e * pr.i) + p * bb * bb * a / (g * pr.j);
    out.push(check(13, "L-shaped cantilever bent in plan", "Tip deflection Pa³/3EI + Pb³/3EI + Pb²a/GJ", "len", -delta, r.disp[tip][1], 1e-9, "Bending and torsion of a bent cantilever (Roark, ch. 8)"));
    out.push(check(13, "L-shaped cantilever bent in plan", "Torque in the first arm, P·b", "moment", p * bb, r.members[0].t[0].abs(), 1e-9, "Statics"));
    // A tripod of truss bars: apex at height hh above three feet on a circle of radius rr, P down
    // on the apex: each leg carries P L / 3h in compression.
    let (hh, rr) = (3.0, 2.0);
    let mut d = space_frame(e, SectionShape::Tube { d: 0.06, t: 0.004 });
    let feet: Vec<[f64; 3]> = (0..3).map(|k| { let t = std::f64::consts::TAU * k as f64 / 3.0 + 0.3; [rr * t.cos(), 0.0, rr * t.sin()] }).collect();
    for f in &feet {
        member3(&mut d, *f, [0.0, hh, 0.0], MemberKind::Truss);
        support3(&mut d, *f, SupportKind::Pinned);
    }
    let id = node3(&d, [0.0, hh, 0.0]);
    load(&mut d, Target::Node { id }, LoadKind::Force { fx: 0.0, fy: -10e3, fz: 0.0 });
    let r = crate::space::solve(&d.m).expect("space");
    let leg = (hh * hh + rr * rr).sqrt();
    out.push(check(13, "Tripod of truss bars", "Force in each leg, −P L / 3h", "force", -10e3 * leg / (3.0 * hh), r.members.iter().map(|m| m.n[0]).fold(0.0, |a: f64, v| if v.abs() > a.abs() { v } else { a }), 1e-9, "Statics of a symmetric tripod"));
    // The plane portal frame, solved as a 3D frame, gives the plane solver's answers.
    let mut pm = crate::examples::get("portal").unwrap();
    let r2 = frame::solve(&pm).unwrap();
    pm.analysis = Analysis::Space;
    let r3 = crate::space::solve(&pm).unwrap();
    let du = r2.disp.iter().zip(&r3.disp).map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[5]).abs())).fold(0.0, f64::max);
    let umax = r2.disp.iter().map(|a| a[0].abs().max(a[1].abs())).fold(0.0, f64::max);
    out.push(check(13, "Portal frame example, solved in 3D", "Largest difference from the plane solve, over the largest movement", "ratio", 1.0, 1.0 + du / umax, 1e-9, "The plane frame is a special case of the 3D frame"));
    let dm = r2.members.iter().zip(&r3.members).map(|(a, b)| a.m.iter().zip(&b.mz).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)).fold(0.0, f64::max);
    let mmax = r2.members.iter().flat_map(|a| a.m.iter()).fold(0.0f64, |a, v| a.max(v.abs()));
    out.push(check(13, "Portal frame example, solved in 3D", "Largest difference in bending moment, over the largest moment", "ratio", 1.0, 1.0 + dm / mmax, 1e-9, "The plane frame is a special case of the 3D frame"));
    out
}

/// Benchmark 14 (0.2.6): natural frequencies and buckling of 3D frames.
fn space_dynamics() -> Vec<Check> {
    let mut out = vec![];
    let (l, e, rho) = (2.0, 200e9, 7850.0);
    let (b, h) = (0.05, 0.1);
    let pr = crate::sections::props(&SectionShape::Rect { b, h }).unwrap();
    let g = e / (2.0 * (1.0 + Material::steel(1).nu));
    let src = "Euler–Bernoulli beam vibration (Blevins, table 8-1); torsional vibration of a shaft";
    // A cantilever along x drawn as one member: it bends both ways and twists.
    let mut d = space_frame(e, SectionShape::Rect { b, h });
    d.m.materials[0].rho = rho;
    member3(&mut d, [0.0; 3], [l, 0.0, 0.0], MemberKind::Beam);
    support3(&mut d, [0.0; 3], SupportKind::Fixed);
    let r = crate::space::modes(&d.m, 8).expect("modes");
    let a = b * h;
    let bend = |i: f64| 1.875_104_068_711_961f64.powi(2) / (std::f64::consts::TAU * l * l) * (e * i / (rho * a)).sqrt();
    let tip = d.m.nodes.iter().position(|n| n.x == l).unwrap();
    let pick = |k: usize| r.modes.iter().find(|md| { let t = md.disp[tip]; let others = (0..4).filter(|j| *j != k).map(|j| t[j].abs()).fold(0.0, f64::max); t[k].abs() > 10.0 * others }).map(|md| md.f).unwrap_or(f64::NAN);
    out.push(check(14, "3D cantilever, one member", "First frequency across the width (weak axis)", "hz", bend(pr.iy), pick(2), 1e-3, src));
    out.push(check(14, "3D cantilever, one member", "First frequency across the depth (strong axis)", "hz", bend(pr.i), pick(1), 1e-3, src));
    out.push(check(14, "3D cantilever, one member", "First twisting frequency √(GJ/ρIp) / 4L", "hz", (g * pr.j / (rho * (pr.i + pr.iy))).sqrt() / (4.0 * l), pick(3), 1e-2, "Torsional vibration of a shaft fixed at one end; linear twisting elements are 0.3 % high with eight pieces"));
    // A column along y pinned at both ends (held from spinning at the foot), 80 × 100: it buckles
    // first about its weak axis, then about its strong one.
    let (lc, p) = (3.0, 10e3);
    let pc = crate::sections::props(&SectionShape::Rect { b: 0.08, h: 0.1 }).unwrap();
    let mut d = space_frame(e, SectionShape::Rect { b: 0.08, h: 0.1 });
    member3(&mut d, [0.0; 3], [0.0, lc, 0.0], MemberKind::Beam);
    let big = 1e15;
    support3(&mut d, [0.0; 3], SupportKind::Spring { kx: big, ky: big, kr: 0.0, kz: big, krx: 0.0, kry: big });
    support3(&mut d, [0.0, lc, 0.0], SupportKind::Spring { kx: big, ky: 0.0, kr: 0.0, kz: big, krx: 0.0, kry: 0.0 });
    let top = node3(&d, [0.0, lc, 0.0]);
    load(&mut d, Target::Node { id: top }, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
    let r = crate::space::buckling(&d.m, 3).expect("buckling");
    let euler = |i: f64| std::f64::consts::PI.powi(2) * e * i / (lc * lc) / p;
    let src = "Euler buckling loads (Timoshenko & Gere, §2.1)";
    out.push(check(14, "3D column pinned at both ends, 80 × 100", "First load factor, about the weak axis", "ratio", euler(pc.i.min(pc.iy)), r.modes[0].lambda, 1e-3, src));
    out.push(check(14, "3D column pinned at both ends, 80 × 100", "Second load factor, about the strong axis", "ratio", euler(pc.i.max(pc.iy)), r.modes[1].lambda, 1e-3, src));
    // The plane portal frame in 3D: its plane frequencies and buckling factors are among the 3D
    // ones (the 3D frame also sways and twists out of its plane).
    let mut pm = crate::examples::get("portal").unwrap();
    let f2 = frame::modes(&pm, 3).unwrap();
    let b2 = frame::buckling(&pm, 2).unwrap();
    pm.analysis = Analysis::Space;
    let f3 = crate::space::modes(&pm, 30).unwrap();
    let b3 = crate::space::buckling(&pm, 30).unwrap();
    let near = |v: f64, xs: &[f64]| xs.iter().map(|x| (x - v).abs() / v).fold(f64::INFINITY, f64::min);
    let ff: Vec<f64> = f3.modes.iter().map(|m| m.f).collect();
    let fb: Vec<f64> = b3.modes.iter().map(|m| m.lambda).collect();
    let worst_f = f2.modes.iter().map(|m| near(m.f, &ff)).fold(0.0, f64::max);
    let worst_b = b2.modes.iter().map(|m| near(m.lambda, &fb)).fold(0.0, f64::max);
    out.push(check(14, "Portal frame example, in 3D", "Its three plane frequencies are among the 3D ones (largest difference)", "ratio", 1.0, 1.0 + worst_f, 1e-6, "The plane frame is a special case of the 3D frame"));
    out.push(check(14, "Portal frame example, in 3D", "Its two plane buckling factors are among the 3D ones (largest difference)", "ratio", 1.0, 1.0 + worst_b, 1e-6, "The plane frame is a special case of the 3D frame"));
    out
}

/// The value of a 3D solid's field at the surface node nearest a point.
fn at_solid(r: &crate::solid::SolidResults, key: &str, p: [f64; 3]) -> f64 {
    let n = r.surface.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, p).total_cmp(&crate::model::dist3(*b.1, p))).map(|x| x.0).unwrap();
    r.field(key).v[n]
}

/// Benchmark 15 (0.3.0): 3D solids, extruded from the drawing, against exact answers.
fn solids() -> Vec<Check> {
    let mut out = vec![];
    let e = 200e9;
    let face = |d: &mut Doc, front: bool, kind: SupportKind| { add_id(d, Item::Support(Support { id: 0, at: Target::End { front }, kind })); };
    // A patch test: a block on rollers at three faces, pulled at the fourth. Every element must
    // carry exactly the uniform stress, and the block thins by ν σ t / E.
    let (p, nu, t) = (50e6, 0.3, 0.05);
    let (mut d, ids) = part(Analysis::Solid, e, nu, vec![(Geom::rect(0.0, 0.0, 0.2, 0.1), false)]);
    d.m.thickness = t;
    edge_support(&mut d, ids[0], 3, SupportKind::Roller);
    edge_support(&mut d, ids[0], 0, SupportKind::Roller);
    face(&mut d, false, SupportKind::Roller);
    edge_load(&mut d, ids[0], 1, LoadKind::Pressure { p: -p });
    d.m.mesh.size = 0.04;
    let r = crate::solid::solve(&d.m).expect("solid");
    let src = "Patch test: a uniform stress must be reproduced exactly";
    out.push(check(15, "3D patch test: a block pulled, 18-node wedges", "Largest von Mises stress (uniform σx)", "stress", p, r.field("vm").max, 1e-9, src));
    out.push(check(15, "3D patch test: a block pulled, 18-node wedges", "Smallest von Mises stress (uniform σx)", "stress", p, r.field("vm").min, 1e-9, src));
    out.push(check(15, "3D patch test: a block pulled, 18-node wedges", "Thinning of the block −νσt/E", "len", -nu * p * t / e, r.field("uz").min, 1e-9, src));
    // A cantilever 1 m long, 100 deep and 50 wide, fixed at one end, 1 kN down at the other.
    // Timoshenko beam theory with ν = 0 (no sideways spread at the fixed end): PL³/3EI + PL/κGA.
    let (l, h, b, pf) = (1.0, 0.1, 0.05, 1e3);
    let (mut d, ids) = part(Analysis::Solid, e, 0.0, vec![(Geom::rect(0.0, 0.0, l, h), false)]);
    d.m.thickness = b;
    edge_support(&mut d, ids[0], 3, SupportKind::Fixed);
    edge_load(&mut d, ids[0], 1, LoadKind::Traction { fx: 0.0, fy: -pf, total: true, fz: 0.0 });
    d.m.mesh.size = h / 4.0;
    let r = crate::solid::solve(&d.m).expect("solid");
    let i = b * h.powi(3) / 12.0;
    let tip = pf * l.powi(3) / (3.0 * e * i) + pf * l / (5.0 / 6.0 * (e / 2.0) * b * h);
    out.push(check(15, "3D cantilever, 1 m × 100 × 50, ν = 0", "Tip deflection PL³/3EI + PL/κGA", "len", -tip, at_solid(&r, "uy", [l, h / 2.0, b / 2.0]), 5e-3, "Timoshenko beam theory (κ = 5/6)"));
    out.push(check(15, "3D cantilever, 1 m × 100 × 50, ν = 0", "Bending stress at mid-span, top, M c / I", "stress", pf * l / 2.0 * (h / 2.0) / i, at_solid(&r, "sx", [l / 2.0, h, b / 2.0]), 1e-2, "Beam theory"));
    out.push(check(15, "3D cantilever, 1 m × 100 × 50, ν = 0", "Reaction at the fixed face equals the load", "force", pf, r.reactions[0].f[1], 1e-9, "Statics"));
    // A thick cylinder, a quarter ring extruded and held at both ends along z (plane strain):
    // Lamé's hoop stress at the bore.
    let (a, bo, pr, nu) = (0.1, 0.2, 100e6, 0.3);
    let q = (std::f64::consts::FRAC_PI_8).tan();
    let ring = Geom::Poly { pts: vec![[a, 0.0], [bo, 0.0], [0.0, bo], [0.0, a]], bulge: vec![0.0, q, 0.0, -q] };
    let (mut d, ids) = part(Analysis::Solid, e, nu, vec![(ring, false)]);
    d.m.thickness = 0.02;
    edge_support(&mut d, ids[0], 0, SupportKind::Symmetry);
    edge_support(&mut d, ids[0], 2, SupportKind::Symmetry);
    face(&mut d, false, SupportKind::Symmetry);
    face(&mut d, true, SupportKind::Symmetry);
    edge_load(&mut d, ids[0], 3, LoadKind::Pressure { p: pr });
    d.m.mesh.size = 0.0125;
    let r = crate::solid::solve(&d.m).expect("solid");
    let k = a * a / (bo * bo - a * a);
    let src = "Lamé thick-cylinder equations, plane strain";
    out.push(check(15, "Thick cylinder in 3D, held at both ends", "Hoop stress at the bore p(b²+a²)/(b²−a²)", "stress", pr * k * (1.0 + bo * bo / (a * a)), at_solid(&r, "sy", [a, 0.0, 0.01]), 1e-2, src));
    out.push(check(15, "Thick cylinder in 3D, held at both ends", "Axial stress ν(σr + σθ) = 2νpa²/(b²−a²)", "stress", 2.0 * nu * pr * k, at_solid(&r, "sz", [a, 0.0, 0.01]), 2e-2, src));
    out.push(check(15, "Thick cylinder in 3D, held at both ends", "Bore radial movement", "len", (1.0 + nu) / e * pr * k * ((1.0 - 2.0 * nu) * a + bo * bo / a), at_solid(&r, "ux", [a, 0.0, 0.01]), 1e-3, src));
    out
}

/// Benchmark 17 (0.3.2): revolved 3D solids against Lamé and statics, and NAFEMS LE10.
fn revolved() -> Vec<Check> {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let mut out = vec![];
    let e = 200e9;
    let face = |d: &mut Doc, front: bool, kind: SupportKind| { add_id(d, Item::Support(Support { id: 0, at: Target::End { front }, kind })); };
    let line = |d: &mut Doc, shape: u32, edge: u32, at: f64, h: [bool; 3]| { add_id(d, Item::Support(Support { id: 0, at: Target::Line { shape, edge, at }, kind: SupportKind::Hold { x: h[0], y: h[1], z: h[2] } })); };
    // A closed-end thick cylinder: the wall's section revolved, a slice of it held on its lower
    // face and pulled on its upper by the end cap's load. Lamé, with the uniform axial stress of
    // closed ends.
    let (a, b, h, p, nu) = (0.1, 0.2, 0.05, 100e6, 0.3);
    let k = a * a / (b * b - a * a);
    let (hoop, axial) = (p * k * (1.0 + b * b / (a * a)), p * k);
    let bore = a / e * (hoop - nu * (-p + axial));
    let src = "Lamé thick-cylinder equations, closed ends";
    for (name, turn) in [("Closed-end thick cylinder, revolved a quarter turn", FRAC_PI_2), ("Closed-end thick cylinder, revolved a full turn", TAU)] {
        let (mut d, ids) = part(Analysis::Solid, e, nu, vec![(Geom::rect(a, 0.0, b, h), false)]);
        let s = ids[0];
        d.m.revolve = turn;
        edge_support(&mut d, s, 0, SupportKind::Symmetry);
        if turn < TAU {
            face(&mut d, false, SupportKind::Symmetry);
            face(&mut d, true, SupportKind::Symmetry);
        } else {
            // Round a full turn nothing stops it turning or sliding sideways: hold the movement
            // round the turn on two lines a quarter turn apart (zero in the exact answer).
            line(&mut d, s, 0, 0.0, [false, false, true]);
            line(&mut d, s, 0, 0.25, [true, false, false]);
        }
        edge_load(&mut d, s, 3, LoadKind::Pressure { p });
        edge_load(&mut d, s, 2, LoadKind::Traction { fx: 0.0, fy: axial, total: false, fz: 0.0 });
        d.m.mesh.size = 0.0125;
        d.m.mesh.layers = if turn < TAU { 6 } else { 16 };
        let r = crate::solid::solve(&d.m).expect("revolved solid");
        out.push(check(17, name, "Hoop stress at the bore p(b²+a²)/(b²−a²)", "stress", hoop, at_solid(&r, "sz", [a, h / 2.0, 0.0]), 1e-2, src));
        out.push(check(17, name, "Axial stress pa²/(b²−a²)", "stress", axial, at_solid(&r, "sy", [a, h / 2.0, 0.0]), 1e-2, src));
        out.push(check(17, name, "Bore radial movement", "len", bore, at_solid(&r, "ux", [a, h / 2.0, 0.0]), 1e-3, src));
    }
    // A round bar through the axis squeezed end to end: uniform stress, and the sides bulge by
    // ν p R / E.
    let (rr, l, p) = (0.05, 0.2, 50e6);
    let (mut d, ids) = part(Analysis::Solid, e, nu, vec![(Geom::rect(0.0, 0.0, rr, l), false)]);
    let s = ids[0];
    d.m.revolve = TAU;
    edge_support(&mut d, s, 0, SupportKind::Symmetry);
    line(&mut d, s, 0, 0.0, [false, false, true]);
    line(&mut d, s, 0, 0.25, [true, false, false]);
    edge_load(&mut d, s, 2, LoadKind::Pressure { p });
    d.m.mesh.size = 0.025;
    d.m.mesh.layers = 16;
    let r = crate::solid::solve(&d.m).expect("revolved bar");
    let name = "Round bar through the axis, squeezed (a full turn)";
    out.push(check(17, name, "Axial stress −p, everywhere", "stress", -p, r.field("sy").min, 1e-6, "Statics: a uniform stress"));
    out.push(check(17, name, "Axial stress −p, everywhere", "stress", -p, r.field("sy").max, 1e-6, "Statics: a uniform stress"));
    out.push(check(17, name, "End movement −pL/E", "len", -p * l / e, at_solid(&r, "uy", [0.0, l, 0.0]), 1e-6, "Hooke's law"));
    out.push(check(17, name, "Side bulge νpR/E", "len", nu * p * rr / e, at_solid(&r, "ux", [rr, l / 2.0, 0.0]), 1e-6, "Hooke's law"));
    out.extend(le10());
    out
}

/// NAFEMS LE10: a thick plate, a quarter of an elliptic ring 0.6 m thick, under 1 MPa on its top
/// face; held along x and y on its outer curved face and vertically along that face's middle
/// line. The target is σyy = −5.38 MPa at D, the inner edge on the x axis, on the top face.
fn le10() -> Vec<Check> {
    let (size, near, layers) = (0.2, 0.1, 4);
    let n = 48;
    let ell = |ax: f64, ay: f64, t: f64| [ax * t.cos(), ay * t.sin()];
    let mut pts = vec![[2.0, 0.0]];
    for i in 0..=n {
        pts.push(ell(3.25, 2.75, std::f64::consts::FRAC_PI_2 * i as f64 / n as f64));
    }
    for i in (1..=n).rev() {
        pts.push(ell(2.0, 1.0, std::f64::consts::FRAC_PI_2 * i as f64 / n as f64));
    }
    let k = pts.len();
    let (mut d, ids) = part(Analysis::Solid, 210e9, 0.3, vec![(Geom::Poly { pts, bulge: vec![0.0; k] }, false)]);
    let s = ids[0];
    d.m.thickness = 0.6;
    // Edge 0: D–C on y = 0; edges 1–n: the outer ellipse; n + 1: B–A on x = 0; the rest: inner.
    edge_support(&mut d, s, 0, SupportKind::Symmetry);
    edge_support(&mut d, s, n as u32 + 1, SupportKind::Symmetry);
    for e in 1..=n as u32 {
        add_id(&mut d, Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: e }, kind: SupportKind::Hold { x: true, y: true, z: false } }));
        add_id(&mut d, Item::Support(Support { id: 0, at: Target::Line { shape: s, edge: e, at: 0.5 }, kind: SupportKind::Hold { x: false, y: false, z: true } }));
    }
    add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::End { front: true }, kind: LoadKind::Pressure { p: 1e6 } }));
    d.m.mesh.size = size;
    d.m.mesh.layers = layers;
    for e in n as u32 + 2..k as u32 {
        d.m.mesh.refine.push(Refine { at: Target::Edge { shape: s, edge: e }, size: near });
    }
    let r = crate::solid::solve(&d.m).expect("LE10");
    vec![check(17, "NAFEMS LE10: thick plate under pressure", "σyy at D (inner edge, top face)", "stress", -5.38e6, at_solid(&r, "sy", [2.0, 0.0, 0.6]), 2e-2, "NAFEMS LE10 (Linear elastic benchmarks, 1990)")]
}

/// Benchmark 18 (0.3.3): member end releases (hinges), load cases and combinations, and the steel
/// section tables, against beam theory, statics, Euler and the tables' own values.
fn frames_more() -> Vec<Check> {
    use crate::cases::{envelope, Which};
    use crate::model::{Case, Combo, Factor};
    let mut out = vec![];
    let (e, b, h) = (200e9, 0.1, 0.25f64);
    let i = b * h.powi(3) / 12.0;
    let hinge = |d: &mut Doc, k: usize, hg: [bool; 2]| {
        let mut mb = d.m.members[k].clone();
        mb.hinge = hg;
        add(d, Op::Set { item: Item::Member(mb) });
    };
    let src = "Beam theory (Euler–Bernoulli)";
    // A beam hinged at both ends between fixed supports carries its load as if simply supported.
    let (l, w) = (6.0, 5e3);
    let mut d = frame(e, SectionShape::Rect { b, h });
    member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
    support(&mut d, 0.0, 0.0, SupportKind::Fixed);
    support(&mut d, l, 0.0, SupportKind::Fixed);
    hinge(&mut d, 0, [true, true]);
    let id = d.m.members[0].id;
    load(&mut d, Target::Member { id }, LoadKind::Dist { w1: -w, w2: -w, dir: Dir::Y });
    let r = frame::solve(&d.m).expect("solves");
    let one = &r.members[0];
    let name = "Beam hinged at both ends between fixed supports, uniform load";
    out.push(check(18, name, "Largest moment wL²/8, as simply supported", "moment", w * l * l / 8.0, one.m.iter().cloned().fold(f64::MIN, f64::max), 1e-9, src));
    out.push(check(18, name, "Midspan deflection 5wL⁴/384EI", "len", 5.0 * w * l.powi(4) / (384.0 * e * i), -one.d[frame::STATIONS / 2][1], 1e-9, src));
    // Its natural frequency and, as a column, its buckling load: those of a pinned member.
    let rho = d.m.materials[0].rho;
    let f1 = std::f64::consts::PI.powi(2) / (std::f64::consts::TAU * l * l) * (e * i / (rho * b * h)).sqrt();
    let md = frame::modes(&d.m, 1).expect("modes");
    out.push(check(18, name, "Lowest natural frequency π²√(EI/ρA) / 2πL²", "hz", f1, md.modes[0].f, 1e-3, "Euler–Bernoulli beam vibration (Blevins, table 8-1)"));
    let (lc, p) = (3.0, 10e3);
    let mut c = frame(e, SectionShape::Rect { b, h });
    member(&mut c, [0.0, 0.0], [0.0, lc], MemberKind::Beam);
    support(&mut c, 0.0, 0.0, SupportKind::Fixed);
    support(&mut c, 0.0, lc, SupportKind::RollerY);
    hinge(&mut c, 0, [true, true]);
    let top = node(&c, 0.0, lc);
    load(&mut c, Target::Node { id: top }, LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 });
    let bk = frame::buckling(&c.m, 1).expect("buckling");
    out.push(check(18, "Column hinged at both ends to fixed supports", "Load factor π²EI/L² / P", "ratio", std::f64::consts::PI.powi(2) * e * i / (lc * lc) / p, bk.modes[0].lambda, 1e-3, "Euler buckling (Timoshenko & Gere §2.1)"));
    // A beam on a fixed support with a hinge partway (a Gerber beam): the span beyond the hinge is
    // simply supported and hangs its reaction on the cantilever.
    let (a, sb) = (2.0, 4.0);
    let mut g = frame(e, SectionShape::Rect { b, h });
    member(&mut g, [0.0, 0.0], [a, 0.0], MemberKind::Beam);
    member(&mut g, [a, 0.0], [a + sb, 0.0], MemberKind::Beam);
    support(&mut g, 0.0, 0.0, SupportKind::Fixed);
    support(&mut g, a + sb, 0.0, SupportKind::RollerX);
    hinge(&mut g, 1, [true, false]);
    let id = g.m.members[1].id;
    load(&mut g, Target::Member { id }, LoadKind::Dist { w1: -w, w2: -w, dir: Dir::Y });
    let r = frame::solve(&g.m).expect("solves");
    let (na, nc) = (node(&g, 0.0, 0.0), node(&g, a + sb, 0.0));
    let ra = r.reactions.iter().find(|x| x.node == na).unwrap();
    let rc = r.reactions.iter().find(|x| x.node == nc).unwrap();
    let name = "Beam with a hinge partway (Gerber beam), uniform load beyond it";
    out.push(check(18, name, "Reaction at the far support wb/2", "force", w * sb / 2.0, rc.fy, 1e-9, "Statics"));
    out.push(check(18, name, "Moment at the fixed support (wb/2)·a", "moment", w * sb / 2.0 * a, ra.m.abs(), 1e-9, "Statics"));
    // In 3D: hinged at both ends, loaded both ways: wL²/8 about both axes.
    let mut s3 = space_frame(e, SectionShape::Rect { b, h });
    member3(&mut s3, [0.0; 3], [l, 0.0, 0.0], MemberKind::Beam);
    support3(&mut s3, [0.0; 3], SupportKind::Fixed);
    support3(&mut s3, [l, 0.0, 0.0], SupportKind::Fixed);
    hinge(&mut s3, 0, [true, true]);
    let id = s3.m.members[0].id;
    load(&mut s3, Target::Member { id }, LoadKind::Dist { w1: -w, w2: -w, dir: Dir::Y });
    load(&mut s3, Target::Member { id }, LoadKind::Dist { w1: -w, w2: -w, dir: Dir::Z });
    let r = crate::space::solve(&s3.m).expect("solves");
    let big = |v: &[f64]| v.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    let name = "3D beam hinged at both ends, loaded across both ways";
    out.push(check(18, name, "Largest Mz wL²/8", "moment", w * l * l / 8.0, big(&r.members[0].mz), 1e-9, src));
    out.push(check(18, name, "Largest My wL²/8", "moment", w * l * l / 8.0, big(&r.members[0].my), 1e-9, src));
    // Load cases and combinations: a cantilever with a dead and an imposed load at its tip.
    let (pg, pq) = (10e3, 5e3);
    let mut k = frame(e, SectionShape::Rect { b, h });
    member(&mut k, [0.0, 0.0], [lc, 0.0], MemberKind::Beam);
    support(&mut k, 0.0, 0.0, SupportKind::Fixed);
    let gid = k.apply(Op::Add { item: Item::Case(Case { id: 0, name: "Dead".into() }) }).expect("case")[0];
    let qid = k.apply(Op::Add { item: Item::Case(Case { id: 0, name: "Imposed".into() }) }).expect("case")[0];
    let tip = node(&k, lc, 0.0);
    for (case, p) in [(gid, pg), (qid, pq)] {
        add(&mut k, Op::Add { item: Item::Load(Load { case, id: 0, at: Target::Node { id: tip }, kind: LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 } }) });
    }
    let uls = k.apply(Op::Add { item: Item::Combo(Combo { id: 0, name: "ULS".into(), parts: vec![Factor { case: gid, f: 1.35 }, Factor { case: qid, f: 1.5 }] }) }).expect("combo")[0];
    add(&mut k, Op::Add { item: Item::Combo(Combo { id: 0, name: "SLS".into(), parts: vec![Factor { case: gid, f: 1.0 }, Factor { case: qid, f: 1.0 }] }) });
    let Ok(crate::Results::Frame(r)) = crate::solve_for(&k.m, Some(Which::Combo { id: uls })) else { panic!("combination") };
    let ti = k.m.nodes.iter().position(|n| n.id == tip).unwrap();
    let pu = 1.35 * pg + 1.5 * pq;
    let name = "Cantilever, dead and imposed tip loads, 1.35 G + 1.5 Q";
    out.push(check(18, name, "Tip deflection (1.35 P_G + 1.5 P_Q) L³/3EI", "len", pu * lc.powi(3) / (3.0 * e * i), -r.disp[ti][1], 1e-9, "Beam theory, superposition"));
    let Ok(crate::Results::Frame(rg)) = crate::solve_for(&k.m, Some(Which::Case { id: gid })) else { panic!("case") };
    out.push(check(18, name, "Dead load alone: tip deflection P_G L³/3EI", "len", pg * lc.powi(3) / (3.0 * e * i), -rg.disp[ti][1], 1e-9, "Beam theory"));
    let env = envelope(&k.m).expect("envelope");
    out.push(check(18, name, "Envelope: largest moment (1.35 P_G + 1.5 P_Q) L", "moment", pu * lc, env.members[0].m, 1e-9, "Statics, the larger of the two combinations"));
    // The section tables: second moments worked out from the dimensions against the tables.
    let tsrc = "EN 10365 section tables (second moment of area)";
    for (n, iy) in [("IPE 300", 8356e-8), ("HEA 200", 3692e-8), ("HEB 200", 5696e-8)] {
        let p = crate::sections::props(&crate::steel::find(n).expect("section").shape).expect("props");
        out.push(check(18, "Steel section tables, from the dimensions", Box::leak(format!("{n}: Iy, strong axis").into_boxed_str()), "inertia", iy, p.i, 1e-3, tsrc));
    }
    let worst = crate::steel::library().into_iter().map(|s| (crate::sections::props(&s.shape).expect("props").a, s)).max_by(|x, y| ((x.0 - x.1.table_area).abs() / x.1.table_area).total_cmp(&((y.0 - y.1.table_area).abs() / y.1.table_area))).expect("tables");
    out.push(check(18, "Steel section tables, from the dimensions", Box::leak(format!("Area of {}, the furthest of all {} from its table", worst.1.name, crate::steel::library().len()).into_boxed_str()), "area", worst.1.table_area, worst.0, 5e-3, "EN 10365 section tables (area)"));
    out
}

/// Every check, in the order of the brief's table.
/// Benchmark 16 (0.3.1): the sketch tools. Areas of shapes made by rounding, combining,
/// offsetting and the ready-made shapes, against hand formulas; then Kirsch's plate again, its
/// hole cut by subtracting a circle from a rectangle, meshed and solved.
fn sketch() -> Vec<Check> {
    use crate::sketch::{combine, corner, make, offset, How, Make};
    use std::f64::consts::PI;
    let sq = Geom::rect(0.0, 0.0, 1.0, 1.0);
    let r = 0.2;
    let round = corner(&sq, 2, r, true).expect("round").area();
    let bevel = corner(&sq, 2, r, false).expect("bevel").area();
    let bite: f64 = combine(&sq, &Geom::Circle { c: [1.0, 1.0], r: 0.5 }, How::Subtract).expect("subtract").iter().map(|p| p.geom.area()).sum();
    let lens = combine(&Geom::Circle { c: [0.0, 0.0], r: 1.0 }, &Geom::Circle { c: [1.0, 0.0], r: 1.0 }, How::Intersect).expect("intersect")[0].geom.area();
    let joined = combine(&sq, &Geom::Circle { c: [1.0, 0.5], r: 0.5 }, How::Union).expect("union")[0].geom.area();
    let rr = make(&Make::Rounded { a: [0.0, 0.0], b: [1.0, 1.0], r }).expect("rounded");
    let inset = offset(&rr, -0.1).expect("offset").area();
    let slot = make(&Make::Slot { a: [0.0, 0.0], b: [1.0, 0.0], w: 0.2 }).expect("slot").area();
    let hex = make(&Make::Polygon { c: [0.0, 0.0], r: 1.0, n: 6, turn: 0.0 }).expect("polygon").area();
    let src = "Hand formulas (circle segments and sectors)";
    let mut v = vec![
        check(16, "Sketch tools: areas", "Unit square, one corner rounded r = 0.2: 1 − r² + πr²/4", "ratio", 1.0 - r * r + PI * r * r / 4.0, round, 1e-9, src),
        check(16, "Sketch tools: areas", "Unit square, one corner bevelled 0.2: 1 − 0.2²/2", "ratio", 1.0 - r * r / 2.0, bevel, 1e-9, src),
        check(16, "Sketch tools: areas", "Unit square less a circle r = 0.5 on a corner: 1 − π/16", "ratio", 1.0 - PI / 16.0, bite, 1e-9, src),
        check(16, "Sketch tools: areas", "Two unit circles 1 apart, in common: 2π/3 − √3/2", "ratio", 2.0 * PI / 3.0 - 3f64.sqrt() / 2.0, lens, 1e-9, src),
        check(16, "Sketch tools: areas", "Unit square and a circle r = 0.5 on its side, joined: 1 + π/8", "ratio", 1.0 + PI / 8.0, joined, 1e-9, src),
        check(16, "Sketch tools: areas", "Unit square rounded r = 0.2, offset in 0.1: 0.8² − (4 − π)·0.1²", "ratio", 0.64 - (4.0 - PI) * 0.01, inset, 1e-9, src),
        check(16, "Sketch tools: areas", "Slot 1 between centres, 0.2 wide: 0.2 + π·0.1²", "ratio", 0.2 + PI * 0.01, slot, 1e-9, src),
        check(16, "Sketch tools: areas", "Regular hexagon, corners 1 from the centre: 3√3/2", "ratio", 1.5 * 3f64.sqrt(), hex, 1e-9, src),
    ];
    // Kirsch's quarter plate, the hole cut by Subtract (as benchmark 4, finest mesh).
    let (a, w, s) = (0.01, 4.0, 100e6);
    let (mut d, ids) = part(Analysis::Stress, 210e9, 0.3, vec![(Geom::rect(0.0, 0.0, w / 2.0, w / 2.0), false), (Geom::Circle { c: [0.0, 0.0], r: a }, false)]);
    add(&mut d, Op::Combine { a: ids[0], b: ids[1], how: How::Subtract });
    let p = ids[0];
    let Geom::Poly { pts, bulge } = d.m.shapes[0].geom.clone() else { panic!("a polygon") };
    let n = pts.len() as u32;
    let edge = |f: &dyn Fn([f64; 2], [f64; 2]) -> bool| (0..n).find(|&i| f(pts[i as usize], pts[((i + 1) % n) as usize])).expect("edge");
    let hole = (0..n).find(|&i| bulge[i as usize] != 0.0).expect("the arc");
    edge_support(&mut d, p, edge(&|p, q| p[1].abs() < 1e-12 && q[1].abs() < 1e-12), SupportKind::Symmetry);
    edge_support(&mut d, p, edge(&|p, q| p[0].abs() < 1e-12 && q[0].abs() < 1e-12), SupportKind::Symmetry);
    edge_load(&mut d, p, edge(&|p, q| (p[0] - w / 2.0).abs() < 1e-12 && (q[0] - w / 2.0).abs() < 1e-12), LoadKind::Pressure { p: -s });
    d.m.mesh.size = 0.2;
    d.m.mesh.refine.push(Refine { at: Target::Edge { shape: p, edge: hole }, size: a / 16.0 });
    let res = plane::solve(&d.m).expect("benchmark solves");
    v.push(check(16, "Sketch tools: a hole cut by Subtract", "Stress concentration at the hole 3.0 (as benchmark 4)", "ratio", 3.0, at(&res, "sx", [0.0, a]) / s, 1e-2, "Kirsch (1898), infinite plate"));
    v
}

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
    v.extend(frequencies());
    v.extend(buckling());
    v.extend(space());
    v.extend(space_dynamics());
    v.extend(solids());
    v.extend(sketch());
    v.extend(revolved());
    v.extend(frames_more());
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
        for b in 1..=18 {
            assert!(rows.iter().any(|r| r.bench == b), "benchmark {b} has no rows");
        }
    }
}

/// How long a solve took (0.2.6): typical and large models, against the speed budgets in
/// docs/MEMORY.md. Times depend on the machine; the record says which one ran them.
#[derive(Clone, Debug, Serialize)]
pub struct Timing {
    pub name: String,
    pub unknowns: usize,
    pub ms: f64,
    pub budget_ms: f64,
}

/// A building frame of `nx` × `nz` bays and `ny` storeys (3 m bays, 3 m storeys), fixed at the
/// ground, gravity and a push along x at every floor node. Built directly (not edit by edit).
pub fn building(nx: usize, ny: usize, nz: usize) -> Model {
    use crate::model::{Member, Node};
    let mut m = Model::new(Analysis::Space);
    let id = |i: usize, j: usize, k: usize| (100 + (j * (nz + 1) + k) * (nx + 1) + i) as u32;
    for j in 0..=ny {
        for k in 0..=nz {
            for i in 0..=nx {
                m.nodes.push(Node { id: id(i, j, k), x: 3.0 * i as f64, y: 3.0 * j as f64, z: 3.0 * k as f64 });
            }
        }
    }
    let mut next = 100 + ((ny + 1) * (nz + 1) * (nx + 1)) as u32;
    let mut member = |m: &mut Model, a: u32, b: u32| {
        m.members.push(Member { id: next, a, b, kind: MemberKind::Beam, section: 2, material: 1, roll: 0.0, hinge: [false; 2] });
        next += 1;
    };
    for j in 0..=ny {
        for k in 0..=nz {
            for i in 0..=nx {
                if j > 0 {
                    member(&mut m, id(i, j - 1, k), id(i, j, k));
                    if i < nx {
                        member(&mut m, id(i, j, k), id(i + 1, j, k));
                    }
                    if k < nz {
                        member(&mut m, id(i, j, k), id(i, j, k + 1));
                    }
                }
            }
        }
    }
    let mut sid = next;
    for k in 0..=nz {
        for i in 0..=nx {
            m.supports.push(Support { id: sid, at: Target::Node { id: id(i, 0, k) }, kind: SupportKind::Fixed });
            sid += 1;
        }
    }
    m.loads.push(Load { case: 0, id: sid, at: Target::Body, kind: LoadKind::Gravity { gx: 0.0, gy: -9.81, gz: 0.0 } });
    sid += 1;
    for j in 1..=ny {
        m.loads.push(Load { case: 0, id: sid, at: Target::Node { id: id(0, j, 0) }, kind: LoadKind::Force { fx: 5e3, fy: 0.0, fz: 0.0 } });
        sid += 1;
    }
    m.next = sid;
    m
}

/// Typical and large solves, timed.
pub fn timings() -> Vec<Timing> {
    let mut out = vec![];
    let mut time = |name: &str, budget_ms: f64, f: &mut dyn FnMut() -> usize| {
        let t0 = std::time::Instant::now();
        let unknowns = f();
        out.push(Timing { name: name.into(), unknowns, ms: t0.elapsed().as_secs_f64() * 1e3, budget_ms });
    };
    let portal = crate::examples::get("portal").unwrap();
    time("Portal frame example: solve", 1000.0, &mut || frame::solve(&portal).unwrap().stats.equations);
    let platform = crate::examples::get("platform").unwrap();
    time("Steel platform (3D) example: solve", 1000.0, &mut || crate::space::solve(&platform).unwrap().stats.equations);
    let bracket = crate::examples::get("bracket").unwrap();
    time("L-bracket example: mesh and solve", 1000.0, &mut || plane::solve(&bracket).unwrap().stats.equations);
    let mut fine = bracket.clone();
    fine.mesh.size = 0.0006;
    time("L-bracket, fine mesh: mesh and solve", 10_000.0, &mut || plane::solve(&fine).unwrap().stats.equations);
    let b3 = crate::examples::get("bracket3d").unwrap();
    time("L-bracket in 3D example: mesh and solve", 1000.0, &mut || crate::solid::solve(&b3).unwrap().stats.equations);
    let mut b3f = b3.clone();
    b3f.mesh.size = 0.003;
    b3f.thickness = 0.015;
    b3f.mesh.layers = 0;
    time("L-bracket in 3D, 15 deep, fine mesh: mesh and solve", 10_000.0, &mut || crate::solid::solve(&b3f).unwrap().stats.equations);
    let v3 = crate::examples::get("vessel3d").unwrap();
    time("Vessel in 3D (revolved) example: mesh and solve", 2000.0, &mut || crate::solid::solve(&v3).unwrap().stats.equations);
    let tall = building(10, 12, 8);
    time("3D building frame, 10 × 8 bays, 12 storeys: solve", 10_000.0, &mut || crate::space::solve(&tall).unwrap().stats.equations);
    time("3D building frame, 10 × 8 bays, 12 storeys: 6 natural frequencies", 10_000.0, &mut || crate::space::modes(&tall, 6).unwrap().equations);
    out
}
