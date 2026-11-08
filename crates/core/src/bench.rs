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
    let gid = k.apply(Op::Add { item: Item::Case(Case { id: 0, name: "Dead".into(), ..Default::default() }) }).expect("case")[0];
    let qid = k.apply(Op::Add { item: Item::Case(Case { id: 0, name: "Imposed".into(), ..Default::default() }) }).expect("case")[0];
    let tip = node(&k, lc, 0.0);
    for (case, p) in [(gid, pg), (qid, pq)] {
        add(&mut k, Op::Add { item: Item::Load(Load { case, id: 0, at: Target::Node { id: tip }, kind: LoadKind::Force { fx: 0.0, fy: -p, fz: 0.0 } }) });
    }
    let uls = k.apply(Op::Add { item: Item::Combo(Combo { id: 0, name: "ULS".into(), parts: vec![Factor { case: gid, f: 1.35 }, Factor { case: qid, f: 1.5 }], auto: false }) }).expect("combo")[0];
    add(&mut k, Op::Add { item: Item::Combo(Combo { id: 0, name: "SLS".into(), parts: vec![Factor { case: gid, f: 1.0 }, Factor { case: qid, f: 1.0 }], auto: false }) });
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

/// The value of a 3D heat field at the surface node nearest a point.
fn at_heat3(r: &crate::solid::SolidHeatResults, key: &str, p: [f64; 3]) -> f64 {
    let n = r.surface.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, p).total_cmp(&crate::model::dist3(*b.1, p))).map(|x| x.0).unwrap();
    r.field(key).v[n]
}

/// Benchmark 19 (0.3.4): heat conduction and natural frequencies of 3D solids, against exact
/// answers: a wall, a revolved pipe wall, a fin, a slab heating itself; a bar's axial and
/// bending vibration, and a revolved shaft's.
fn solids_more() -> Vec<Check> {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    let mut out = vec![];
    let tsup = |d: &mut Doc, at: Target, t: f64| { add_id(d, Item::Support(Support { id: 0, at, kind: SupportKind::Temperature { t } })); };
    let tload = |d: &mut Doc, at: Target, kind: LoadKind| { add_id(d, Item::Load(Load { case: 0, id: 0, at, kind })); };
    let heat_part = |k: f64, g: Geom| {
        let (mut d, ids) = part(Analysis::SolidHeat, 200e9, 0.3, vec![(g, false)]);
        d.m.materials[0].k = k;
        (d, ids[0])
    };
    // A wall 100 mm thick, 0 °C on one face and 100 °C on the other: a straight line of
    // temperature, and the flux kΔT/L everywhere.
    let (l, k, t0) = (0.1, 50.0, 273.15);
    let (mut d, s) = heat_part(k, Geom::rect(0.0, 0.0, l, 0.04));
    d.m.thickness = 0.02;
    tsup(&mut d, Target::Edge { shape: s, edge: 3 }, t0 + 100.0);
    tsup(&mut d, Target::Edge { shape: s, edge: 1 }, t0);
    d.m.mesh.size = 0.02;
    let r = crate::solid::heat(&d.m).expect("heat");
    let name = "3D wall, temperatures fixed on two faces";
    out.push(check(19, name, "Temperature halfway through", "dtemp", 50.0, at_heat3(&r, "t", [l / 2.0, 0.0, 0.0]) - t0, 1e-9, "Fourier's law (a patch test)"));
    out.push(check(19, name, "Heat flux kΔT/L, everywhere", "flux", k * 100.0 / l, r.field("qmag").max, 1e-9, "Fourier's law (a patch test)"));
    out.push(check(19, name, "Heat balance: the flow in equals the flow out", "flux", k * 100.0 / l, r.supports[0].q / (0.04 * 0.02), 1e-9, "Conservation of energy"));
    // A pipe wall, its section revolved a quarter turn: T(r) = Ti − ΔT ln(r/ri)/ln(ro/ri).
    let (ri, ro, h) = (0.05, 0.1, 0.02);
    let (mut d, s) = heat_part(k, Geom::rect(ri, 0.0, ro, h));
    d.m.revolve = FRAC_PI_2;
    tsup(&mut d, Target::Edge { shape: s, edge: 3 }, t0 + 100.0);
    tsup(&mut d, Target::Edge { shape: s, edge: 1 }, t0);
    d.m.mesh.size = 0.0125;
    d.m.mesh.layers = 6;
    let r = crate::solid::heat(&d.m).expect("heat");
    let rm = 0.075;
    let name = "Pipe wall, revolved a quarter turn, inside hotter";
    out.push(check(19, name, "Temperature halfway through the wall", "dtemp", 100.0 - 100.0 * (rm / ri).ln() / (ro / ri).ln(), at_heat3(&r, "t", [rm, h / 2.0, 0.0]) - t0, 2e-3, "Radial conduction through a cylinder wall (Incropera, §3.3)"));
    out.push(check(19, name, "Heat flow through the quarter, 2πkhΔT / 4 ln(ro/ri)", "power", TAU * k * h * 100.0 / (4.0 * (ro / ri).ln()), r.supports[0].q, 2e-3, "Radial conduction through a cylinder wall (Incropera, §3.3)"));
    // A pin fin, 10 mm square and 100 mm long, its base at 100 °C, cooled by air on its sides,
    // its tip sealed: the tip at θ_b / cosh(mL), m = √(hP/kA).
    let (lf, w, kf, hc) = (0.1, 0.01, 200.0, 25.0);
    let (mut d, s) = heat_part(kf, Geom::rect(0.0, 0.0, lf, w));
    d.m.thickness = w;
    tsup(&mut d, Target::Edge { shape: s, edge: 3 }, t0 + 100.0);
    for at in [Target::Edge { shape: s, edge: 0 }, Target::Edge { shape: s, edge: 2 }, Target::End { front: false }, Target::End { front: true }] {
        tload(&mut d, at, LoadKind::Convection { h: hc, t_inf: t0 });
    }
    d.m.mesh.size = 0.005;
    let r = crate::solid::heat(&d.m).expect("heat");
    let mf = (hc * 4.0 * w / (kf * w * w)).sqrt();
    let name = "Pin fin, convection on its sides, tip sealed";
    out.push(check(19, name, "Tip temperature rise θ_b / cosh(mL)", "dtemp", 100.0 / (mf * lf).cosh(), at_heat3(&r, "t", [lf, w / 2.0, w / 2.0]) - t0, 5e-3, "Fin equation (Incropera, §3.6), Biot number 0.0006"));
    out.push(check(19, name, "Heat balance over the fin", "ratio", 1.0, 1.0 + r.imbalance, 1e-9, "Conservation of energy"));
    // A slab heating itself, both faces at 0 °C: the middle rises by qL²/8k.
    let (ls, qg) = (0.1, 1e6);
    let (mut d, s) = heat_part(k, Geom::rect(0.0, 0.0, ls, 0.04));
    d.m.thickness = 0.02;
    tsup(&mut d, Target::Edge { shape: s, edge: 3 }, t0);
    tsup(&mut d, Target::Edge { shape: s, edge: 1 }, t0);
    tload(&mut d, Target::Body, LoadKind::Heat { q: qg });
    d.m.mesh.size = 0.02;
    let r = crate::solid::heat(&d.m).expect("heat");
    out.push(check(19, "Slab heating itself, both faces at 0 °C", "Rise in the middle qL²/8k", "dtemp", qg * ls * ls / (8.0 * k), r.field("t").max - t0, 1e-9, "Conduction with generation (a parabola, exact for quadratic elements)"));
    // Vibration: a steel bar 1 m long, 50 mm square, fixed at one end (ν = 0).
    let (e, rho) = (200e9, 7850.0);
    let bar = |lb: f64, a: f64, size: f64| {
        let (mut d, ids) = part(Analysis::Solid, e, 0.0, vec![(Geom::rect(0.0, 0.0, lb, a), false)]);
        d.m.materials[0].rho = rho;
        d.m.thickness = a;
        edge_support(&mut d, ids[0], 3, SupportKind::Fixed);
        d.m.mesh.size = size;
        d
    };
    let d = bar(1.0, 0.05, 0.05);
    let md = crate::solid::modes(&d.m, 8).expect("modes");
    let axial = (e / rho).sqrt() / 4.0;
    let fa = md.modes.iter().map(|x| x.f).min_by(|a, b| (a - axial).abs().total_cmp(&(b - axial).abs())).unwrap();
    out.push(check(19, "Bar 1 m long fixed at one end, ν = 0", "Lowest axial frequency √(E/ρ) / 4L", "hz", axial, fa, 1e-3, "Axial vibration of a bar (Blevins, table 8-16)"));
    // A slender cantilever, 1 m × 20 × 20: its lowest bending frequency.
    let d = bar(1.0, 0.02, 0.01);
    let md = crate::solid::modes(&d.m, 1).expect("modes");
    let bend = 1.875_104_068_711_96f64.powi(2) / TAU * (e * 0.02f64.powi(4) / 12.0 / (rho * 0.02 * 0.02)).sqrt();
    out.push(check(19, "Cantilever 1 m × 20 × 20 in 3D, ν = 0", "Lowest bending frequency (1.875²/2π)√(EI/ρAL⁴)", "hz", bend, md.modes[0].f, 1e-2, "Euler–Bernoulli beam vibration (Blevins, table 8-1); shear and rotary inertia lower it slightly"));
    // A round shaft 40 mm across, revolved a full turn, fixed at its foot: its bending frequency.
    let (rs, ls2) = (0.02, 1.0);
    let (mut d, ids) = part(Analysis::Solid, e, 0.0, vec![(Geom::rect(0.0, 0.0, rs, ls2), false)]);
    d.m.materials[0].rho = rho;
    d.m.revolve = TAU;
    edge_support(&mut d, ids[0], 0, SupportKind::Fixed);
    d.m.mesh.size = 0.02;
    d.m.mesh.layers = 8;
    let md = crate::solid::modes(&d.m, 1).expect("modes");
    let shaft = 1.875_104_068_711_96f64.powi(2) / TAU * (e * PI * rs.powi(4) / 4.0 / (rho * PI * rs * rs)).sqrt() / (ls2 * ls2);
    out.push(check(19, "Round shaft 40 mm across, 1 m long, revolved a full turn", "Lowest bending frequency (1.875²/2π)√(EI/ρAL⁴)", "hz", shaft, md.modes[0].f, 1e-2, "Euler–Bernoulli beam vibration (Blevins, table 8-1)"));
    out
}

/// Benchmark 20 (0.3.5): DXF import. Every example's section written to a DXF and read back, and a
/// drawing of loose lines and arcs joined into a plate with a slot and a hole, against hand areas.
fn dxf() -> Vec<Check> {
    use std::f64::consts::PI;
    let mut worst = 0.0f64;
    let mut n = 0;
    for e in crate::examples::list() {
        let m = crate::examples::get(e.id).expect("example");
        if m.shapes.is_empty() {
            continue;
        }
        let back = crate::dxf::read(&crate::dxf::write(&m), None).expect("dxf");
        let total = |v: &mut dyn Iterator<Item = (f64, bool)>| v.map(|(a, h)| if h { -a.abs() } else { a.abs() }).sum::<f64>();
        let (a0, a1) = (total(&mut m.shapes.iter().map(|s| (s.geom.area(), s.hole))), total(&mut back.shapes.iter().map(|(g, h)| (g.area(), *h))));
        worst = worst.max((a1 - a0).abs() / a0.abs());
        n += 1;
    }
    let r = crate::dxf::read(crate::dxf::SAMPLE, None).expect("dxf");
    let inch = 0.0254f64;
    let solid: f64 = r.shapes.iter().filter(|s| !s.1).map(|s| s.0.area()).sum();
    let holes: f64 = r.shapes.iter().filter(|s| s.1).map(|s| s.0.area()).sum();
    let src = "Hand areas";
    vec![
        check(20, "DXF round trip of every example's section", Box::leak(format!("Net area after writing and reading back ({n} sections, the largest difference)").into_boxed_str()), "ratio", 1.0, 1.0 + worst, 1e-9, "The sections as drawn"),
        check(20, "DXF of loose lines and arcs, in inches", "Plate joined from four lines: 100 × 40 in", "area", 4000.0 * inch * inch, solid, 1e-9, src),
        check(20, "DXF of loose lines and arcs, in inches", "Holes: a slot of two lines and two arcs, and a circle", "area", (200.0 + PI * 25.0 + PI * 64.0) * inch * inch, holes, 1e-9, src),
    ]
}

/// Solve a frame by its steps (0.4.0) and give the results.
fn stepped(d: &Doc) -> frame::FrameResults {
    match crate::solve_for(&d.m, None).expect("benchmark steps") {
        crate::Results::Frame(r) => r,
        _ => unreachable!(),
    }
}
fn step(kind: crate::model::StepKind, factor: f64, increments: u32, pieces: u32, monitor: Option<u32>) -> Item {
    Item::Step(crate::model::Step { kind, factor, increments, pieces, monitor, ..Default::default() })
}

/// K(m) and E(m), the complete elliptic integrals (parameter m = k²), by the arithmetic–geometric
/// mean (Abramowitz and Stegun 17.6).
fn elliptic(m: f64) -> (f64, f64) {
    let (mut a, mut b, mut c) = (1.0f64, (1.0 - m).sqrt(), m.sqrt());
    let (mut sum, mut pw) = (0.5 * c * c, 0.5);
    for _ in 0..40 {
        let (a1, b1) = ((a + b) / 2.0, (a * b).sqrt());
        c = (a - b) / 2.0;
        a = a1;
        b = b1;
        pw *= 2.0;
        sum += pw * c * c;
    }
    let k = std::f64::consts::PI / (2.0 * a);
    (k, k * (1.0 - sum))
}

/// The elastica of a cantilever under an end load along its axis, past buckling: for
/// P* = PL²/EI, K(k) = √P* with k = sin(α/2), α the tip rotation; the tip moves 2k/K·L sideways
/// and (2 − 2E/K)·L along the axis (Timoshenko and Gere, *Theory of Elastic Stability*, §2.7).
fn elastica(p: f64) -> (f64, f64, f64) {
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if elliptic(mid).0 < p.sqrt() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (k, e) = elliptic(lo);
    let s = lo.sqrt();
    (2.0 * s / k, 2.0 - 2.0 * e / k, 2.0 * s.asin())
}

/// Benchmark 21 (0.4.0): large deflection and second-order (P-delta) frames, by corotational
/// beams and the Newton and arc-length path solver.
fn large() -> Vec<Check> {
    use crate::model::StepKind::{Arc, Newton};
    use std::f64::consts::PI;
    let rect = SectionShape::Rect { b: 0.1, h: 0.2 };
    let (e, i) = (200e9, 0.1 * 0.2f64.powi(3) / 12.0);
    let ei = e * i;
    let mut v = vec![];
    // AISC 360 Commentary C2, case 1: a pinned beam-column, uniform load w and axial load P.
    // u = (L/2)√(P/EI): M_mid = (wEI/P)(sec u − 1), δ_mid = (5wL⁴/384EI)·12(2 sec u − 2 − u²)/(5u⁴).
    {
        let (l, w) = (8.0, -10e3);
        let p = 0.4 * PI * PI * ei / (l * l);
        let mut d = frame(e, rect.clone());
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [l / 2.0, 0.0], MemberKind::Beam);
        member(&mut d, [l / 2.0, 0.0], [l, 0.0], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Pinned);
        support(&mut d, l, 0.0, SupportKind::RollerX);
        for id in d.m.members.iter().map(|m| m.id).collect::<Vec<_>>() {
            load(&mut d, Target::Member { id }, LoadKind::Dist { w1: w, w2: w, dir: Dir::Y });
        }
        let end = node(&d, l, 0.0);
        load(&mut d, Target::Node { id: end }, LoadKind::Force { fx: -p, fy: 0.0, fz: 0.0 });
        add(&mut d, Op::Add { item: step(Newton, 1.0, 5, 20, None) });
        let r = stepped(&d);
        let u = l / 2.0 * (p / ei).sqrt();
        let sec = 1.0 / u.cos();
        let mm = w.abs() * ei / p * (sec - 1.0);
        let dd = 5.0 * w.abs() * l.powi(4) / (384.0 * ei) * 12.0 * (2.0 * sec - 2.0 - u * u) / (5.0 * u.powi(4));
        let mid = node(&d, l / 2.0, 0.0);
        let k = d.m.nodes.iter().position(|n| n.id == mid).unwrap();
        let src = "AISC 360-16 Commentary C2, benchmark case 1 (closed form)";
        v.push(check(21, "Second-order beam-column, P = 0.4 P_e", "Mid-span moment, amplified", "moment", mm, r.members[0].m.last().unwrap().abs(), 0.005, src));
        v.push(check(21, "Second-order beam-column, P = 0.4 P_e", "Mid-span deflection, amplified", "len", dd, r.disp[k][1].abs(), 0.005, src));
    }
    // Case 2: a flagpole (fixed base, free top) with an axial load P and a side load H at the top.
    // k = √(P/EI): Δ = (H/(kP))(tan kL − kL), M_base = H tan(kL)/k.
    {
        let (l, h) = (5.0, 10e3);
        let p = 0.5 * PI * PI * ei / (4.0 * l * l);
        let mut d = frame(e, rect.clone());
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [0.0, l], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let top = node(&d, 0.0, l);
        load(&mut d, Target::Node { id: top }, LoadKind::Force { fx: h, fy: -p, fz: 0.0 });
        add(&mut d, Op::Add { item: step(Newton, 1.0, 5, 20, None) });
        let r = stepped(&d);
        let k = (p / ei).sqrt();
        let src = "AISC 360-16 Commentary C2, benchmark case 2 (closed form)";
        v.push(check(21, "Second-order flagpole, P = 0.5 P_cr", "Top deflection, amplified", "len", h / (k * p) * ((k * l).tan() - k * l), r.disp[1][0], 0.005, src));
        v.push(check(21, "Second-order flagpole, P = 0.5 P_cr", "Base moment, amplified", "moment", h * (k * l).tan() / k, r.reactions[0].m.abs(), 0.005, src));
    }
    // NAFEMS NL5: a cantilever rolled into a circle by an end moment. κ = M/EI; with φ = ML/EI
    // the tip is at (L sin φ/φ, L(1 − cos φ)/φ): at φ = π it is 2L/π above the root line, right
    // above the root; at φ = 2π the cantilever is a full circle, the tip back at the root.
    // Two steps: to φ = π, then on to 2π.
    {
        let l = 1.0;
        let mut d = frame(e, rect.clone());
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let tip = node(&d, l, 0.0);
        load(&mut d, Target::Node { id: tip }, LoadKind::Moment { m: 2.0 * PI * ei / l, mx: 0.0, my: 0.0 });
        add(&mut d, Op::Add { item: step(Newton, 0.5, 10, 40, Some(tip)) });
        add(&mut d, Op::Add { item: step(Newton, 1.0, 10, 40, Some(tip)) });
        let r = stepped(&d);
        let c = r.path.as_ref().expect("a curve");
        let half = c.step.iter().rposition(|s| *s == 1).unwrap();
        let src = "NAFEMS NL5; Euler's elastica, κ = M/EI";
        v.push(check(21, "Cantilever rolled up by an end moment (NAFEMS NL5)", "Half turn: tip height 2L/π", "len", 2.0 * l / PI, c.uy[half], 0.005, src));
        v.push(check(21, "Cantilever rolled up by an end moment (NAFEMS NL5)", "Full turn: tip movement back to the root, −L", "len", -l, r.disp[1][0], 0.001, src));
        v.push(check(21, "Cantilever rolled up by an end moment (NAFEMS NL5)", "Full turn: tip rotation 2π", "rad", 2.0 * PI, r.disp[1][2], 0.001, src));
    }
    // NAFEMS NL6: a cantilever under an end load along its axis, bent far past buckling (a side
    // load of 10⁻³ P starts it off), at P* = PL²/EI = 3.190 and on to 22.493.
    {
        let l = 10.0; // slender (L/r = 173), so its axial strain (P/EA, 0.08 % at most) is small as the elastica assumes
        let mut d = frame(e, rect.clone());
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [0.0, l], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let tip = node(&d, 0.0, l);
        let p = 22.493 * ei / (l * l);
        load(&mut d, Target::Node { id: tip }, LoadKind::Force { fx: 1e-3 * p, fy: -p, fz: 0.0 });
        add(&mut d, Op::Add { item: step(Newton, 3.190 / 22.493, 20, 40, Some(tip)) });
        add(&mut d, Op::Add { item: step(Newton, 1.0, 40, 40, Some(tip)) });
        let r = stepped(&d);
        let c = r.path.as_ref().expect("a curve");
        let first = c.step.iter().rposition(|s| *s == 1).unwrap();
        let src = "NAFEMS NL6; the elastica in elliptic integrals (Timoshenko and Gere §2.7)";
        let (s1, a1, t1) = elastica(3.190);
        let (s2, a2, t2) = elastica(22.493);
        let name = "Post-buckled cantilever, axial end load (NAFEMS NL6)";
        v.push(check(21, name, "P* = 3.190: tip sideways", "len", s1 * l, c.ux[first], 0.005, src));
        v.push(check(21, name, "P* = 3.190: tip shortening", "len", -a1 * l, c.uy[first], 0.005, src));
        v.push(check(21, name, "P* = 3.190: tip rotation", "rad", -t1, c.rz[first], 0.005, src));
        v.push(check(21, name, "P* = 22.493: tip sideways", "len", s2 * l, r.disp[1][0], 0.005, src));
        v.push(check(21, name, "P* = 22.493: tip shortening", "len", -a2 * l, r.disp[1][1], 0.005, src));
        v.push(check(21, name, "P* = 22.493: tip rotation", "rad", -t2, r.disp[1][2], 0.005, src));
    }
    // NAFEMS NL7, Lee's frame: two 1.2 m members at right angles, pinned at both far ends, a
    // load down on the beam 0.24 m from the knee. E = 7.2 GPa, A = 6 cm², I = 2 cm⁴. Arc length
    // follows it over its first limit (snap-through), PL²/EI = 18.54 to 18.55 in the literature.
    {
        let l = 1.2;
        let mut d = frame(7.2e9, SectionShape::Rect { b: 0.03, h: 0.02 });
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [0.0, l], MemberKind::Beam);
        member(&mut d, [0.0, l], [0.24, l], MemberKind::Beam);
        member(&mut d, [0.24, l], [l, l], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Pinned);
        support(&mut d, l, l, SupportKind::Pinned);
        let eil = 7.2e9 * 2e-8 / (l * l);
        let at = node(&d, 0.24, l);
        load(&mut d, Target::Node { id: at }, LoadKind::Force { fx: 0.0, fy: -1.0, fz: 0.0 });
        add(&mut d, Op::Add { item: step(Arc, 30.0 * eil, 60, 20, Some(at)) });
        let r = stepped(&d);
        let c = r.path.as_ref().expect("a curve");
        let src = "NAFEMS NL7 (Lee, Manuel and Rossow 1968; Schweizerhof and Wriggers 1986): PL²/EI ≈ 18.54";
        v.push(check(21, "Lee's frame, snap-through by arc length (NAFEMS NL7)", "First limit load PL²/EI", "ratio", 18.54, c.peak / eil, 0.005, src));
        v.push(check(21, "Lee's frame, snap-through by arc length (NAFEMS NL7)", "Equilibrium at the end of the path (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics on the deflected frame"));
    }
    v
}

/// Navier's series for a simply supported a × b plate under uniform q: the deflection at (x, y)
/// times D/q (Timoshenko and Woinowsky-Krieger, *Theory of Plates and Shells*, §30).
fn navier(a: f64, b: f64, x: f64, y: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let mut w = 0.0;
    for m in (1..200).step_by(2) {
        for n in (1..200).step_by(2) {
            let (mf, nf) = (m as f64, n as f64);
            w += 16.0 / (pi.powi(6) * mf * nf * ((mf / a).powi(2) + (nf / b).powi(2)).powi(2)) * (mf * pi * x / a).sin() * (nf * pi * y / b).sin();
        }
    }
    w
}

/// Benchmark 22 (0.4.1): plates and shells. The flat shell triangle (Allman's membrane with
/// drilling rotations, the Discrete Kirchhoff plate) on its own and in 3D frame models.
fn plates() -> Vec<Check> {
    use crate::frames::shell::cases;
    use crate::model::{Plate, PlaneKind};
    let mut v = vec![];
    let tw = "Timoshenko and Woinowsky-Krieger, Theory of Plates and Shells (Navier's series)";
    v.push(check(22, "Square plate, simply supported, uniform pressure (32 × 32 × 2 triangles)", "Deflection at the middle × D/qa⁴", "ratio", navier(1.0, 1.0, 0.5, 0.5), cases::plate(32, false), 0.005, tw));
    v.push(check(22, "Square plate, clamped, uniform pressure (32 × 32 × 2 triangles)", "Deflection at the middle × D/qa⁴", "ratio", 0.00126, cases::plate(32, true), 0.01, "Timoshenko and Woinowsky-Krieger, Table 35 (0.00126)"));
    v.push(check(22, "Panel bent in its plane by an end moment (8 triangles deep)", "End deflection over ML²/2EI (plane-stress elasticity)", "ratio", 1.0, cases::bending(8), 0.01, "Plane-stress elasticity, pure bending (exact)"));
    let steps: Vec<[f64; 2]> = [8usize, 16, 32].iter().map(|&n| [(2 * n * n) as f64, (cases::scordelis(n) - 0.3024).abs() / 0.3024]).collect();
    v.push(with_steps(check(22, "Scordelis–Lo roof under its own weight (a quarter, 32 × 32 × 2 triangles)", "Vertical movement at the middle of the free edge", "len", 0.3024, cases::scordelis(32), 0.01, "MacNeal and Harder 1985, standard set of problems"), steps));
    let steps: Vec<[f64; 2]> = [8usize, 16, 32].iter().map(|&n| [(2 * n * n) as f64, (cases::pinched(n) - 1.8248e-5).abs() / 1.8248e-5]).collect();
    v.push(with_steps(check(22, "Pinched cylinder with end diaphragms (an eighth, 32 × 32 × 2 triangles)", "Movement under the load", "len", 1.8248e-5, cases::pinched(32), 0.01, "MacNeal and Harder 1985, standard set of problems"), steps));
    // A slab drawn on the Plan plane of a 3D frame, meshed, its edges pinned, under pressure.
    let slab = |size: f64, rho: f64| {
        let mut d = Doc::new(Model::new(Analysis::Space));
        d.m.materials[0].rho = rho;
        d.m.mesh.size = size;
        let id = d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::rect(0.0, 0.0, 4.0, 4.0), material: d.m.materials[0].id, plate: Some(Plate { plane: PlaneKind::Xz, depth: 3.0, t: 0.2 }), ..Shape::default() }) }).expect("slab")[0];
        for e in 0..4 {
            add(&mut d, Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: id, edge: e }, kind: SupportKind::Pinned }) });
        }
        (d, id)
    };
    let (mut d, id) = slab(0.25, 0.0);
    let q = 10e3;
    load(&mut d, Target::Plate { shape: id }, LoadKind::Pressure { p: q });
    let r = crate::space::solve(&d.m).expect("slab");
    let p = r.plates.as_ref().expect("plates");
    let mat = d.m.materials[0].clone();
    let dd = mat.e * 0.2f64.powi(3) / (12.0 * (1.0 - mat.nu * mat.nu));
    let k = (0..p.nodes.len()).min_by(|&i, &j| (p.nodes[i][0] - 2.0).hypot(p.nodes[i][2] + 2.0).total_cmp(&(p.nodes[j][0] - 2.0).hypot(p.nodes[j][2] + 2.0))).expect("node");
    let name = "Slab on the Plan plane of a 3D frame, edges pinned, 10 kPa (automatic mesh)";
    v.push(check(22, name, "Deflection near the middle (Navier's series at that node)", "len", q / dd * navier(4.0, 4.0, p.nodes[k][0], -p.nodes[k][2]), -p.disp[k][1], 0.01, tw));
    let fy: f64 = r.reactions.iter().map(|x| x.f[1]).sum();
    v.push(check(22, name, "Edge reactions, all together, against the load", "force", q * 16.0, fy, 1e-9, "Statics"));
    // Its first natural frequency: f = (π/2)(2/a²)√(D/ρt).
    let (d, _) = slab(0.25, 2500.0);
    let f11 = std::f64::consts::PI / 2.0 * 2.0 / 16.0 * (dd / (2500.0 * 0.2)).sqrt();
    let modes = crate::space::modes(&d.m, 1).expect("modes");
    v.push(check(22, "The slab's first natural frequency (lumped masses)", "Frequency", "hz", f11, modes.modes[0].f, 0.02, "Timoshenko, Young and Weaver, Vibration Problems in Engineering (simply supported plate)"));
    v
}

/// Benchmark 23 (0.4.2): dynamics of structures, against the closed forms of one and two masses.
fn dynamics() -> Vec<Check> {
    use crate::model::{Step, StepKind};
    use std::f64::consts::TAU;
    let mut v = vec![];
    // One mass on a weightless cantilever column: lateral stiffness k = 3EI/L³, ω = √(k/M).
    let (l, mass) = (3.0f64, 2000.0);
    let rect = SectionShape::Rect { b: 0.1, h: 0.2 };
    let ei = 200e9 * 0.1 * 0.2f64.powi(3) / 12.0;
    let k = 3.0 * ei / l.powi(3);
    let w = (k / mass).sqrt();
    let tp = TAU / w;
    let column = |steps: Vec<Step>, extra: Option<LoadKind>| {
        let mut d = frame(200e9, rect.clone());
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [0.0, l], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let top = node(&d, 0.0, l);
        load(&mut d, Target::Node { id: top }, LoadKind::Mass { m: mass });
        if let Some(k) = extra {
            add(&mut d, Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Node { id: top }, kind: k }) });
        }
        for st in steps {
            add(&mut d, Op::Add { item: Item::Step(Step { monitor: Some(top), ..st }) });
        }
        d
    };
    let sdof = "One mass on a weightless cantilever column: ω² = 3EI/(ML³)";
    let d = column(vec![], None);
    let fm = crate::frame::modes(&d.m, 1).expect("modes");
    v.push(check(23, "Mass on a cantilever column", "Natural frequency √(k/M)/2π", "hz", w / TAU, fm.modes[0].f, 1e-6, sdof));
    let solve = |d: &Doc| match crate::solve_for(&d.m, None).expect("dynamic step") {
        crate::Results::Frame(r) => r,
        _ => unreachable!(),
    };
    // A step load H at the top, undamped: u = (H/k)(1 − cos ωt), peak 2H/k at t = π/ω.
    let h = 1e3;
    let st = Step { kind: StepKind::History, damping: 0.0, time: vec![[0.0, 1.0], [100.0, 1.0]], dt: tp / 200.0, duration: 1.5 * tp, pieces: 1, ..Default::default() };
    let r = solve(&column(vec![st], Some(LoadKind::Force { fx: h, fy: 0.0, fz: 0.0 })));
    let dy = r.dynamic.as_ref().expect("record");
    let pk = dy.ux.iter().fold(0.0f64, |a, b| a.max(*b));
    v.push(check(23, "Step load on the mass, undamped (Newmark, 200 steps a period)", "Largest movement 2H/k", "len", 2.0 * h / k, pk, 2e-3, "Single degree of freedom, step load (Chopra, Dynamics of Structures, §4.3)"));
    v.push(check(23, "Step load on the mass, undamped (Newmark, 200 steps a period)", "When it comes: t = T/2", "ratio", 0.5, dy.peak_t / tp, 0.02, "Single degree of freedom, step load"));
    v.push(check(23, "Step load on the mass, undamped (Newmark, 200 steps a period)", "Dynamic balance at that time (loads, inertia, reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics with d'Alembert's inertia forces"));
    // The ground suddenly accelerating at 1 m/s²: relative movement −(M a/k)(1 − cos ωt).
    let st = Step { kind: StepKind::History, ground: true, along: [1.0, 0.0, 0.0], damping: 0.0, time: vec![[0.0, 1.0], [100.0, 1.0]], dt: tp / 200.0, duration: 1.5 * tp, pieces: 1, ..Default::default() };
    let r = solve(&column(vec![st], None));
    let pk = r.dynamic.as_ref().expect("record").ux.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    v.push(check(23, "The ground stepping to 1 m/s², undamped", "Largest movement relative to the ground 2Ma/k", "len", 2.0 * mass * 1.0 / k, pk, 2e-3, "Single degree of freedom, ground acceleration (Chopra §6.4)"));
    // Resonance with 5 % damping: F sin ωt from rest, the exact transient and steady parts.
    let z = 0.05f64;
    let wd = w * (1.0 - z * z).sqrt();
    let exact = |t: f64| h / k / (2.0 * z) * ((-z * w * t).exp() * ((wd * t).cos() + z / (1.0 - z * z).sqrt() * (wd * t).sin()) - (w * t).cos());
    let pts: Vec<[f64; 2]> = (0..=4000).map(|i| { let t = 12.0 * tp * i as f64 / 4000.0; [t, (w * t).sin()] }).collect();
    let st = Step { kind: StepKind::History, damping: z, time: pts, dt: tp / 200.0, duration: 10.0 * tp, pieces: 1, ..Default::default() };
    let r = solve(&column(vec![st], Some(LoadKind::Force { fx: h, fy: 0.0, fz: 0.0 })));
    let dy = r.dynamic.as_ref().expect("record");
    let late: f64 = dy.ux.iter().zip(&dy.t).filter(|(_, t)| **t > 9.0 * tp).map(|(u, _)| u.abs()).fold(0.0, f64::max);
    let want: f64 = (0..2000).map(|i| exact(9.0 * tp + tp * i as f64 / 2000.0).abs()).fold(0.0, f64::max);
    v.push(check(23, "Harmonic load at resonance, 5 % damping (Rayleigh)", "Largest movement in the tenth cycle", "len", want, late, 0.01, "Single degree of freedom, harmonic load from rest (Chopra §3.2)"));
    // A flat response spectrum Sa: the movement Sa/ω² and the base shear M·Sa.
    let sa = 2.5;
    let st = Step { kind: StepKind::Spectrum, along: [1.0, 0.0, 0.0], spectrum: vec![[0.0, sa], [10.0, sa]], modes: 2, pieces: 1, ..Default::default() };
    let r = solve(&column(vec![st], None));
    v.push(check(23, "Response spectrum, flat 2.5 m/s²", "Peak movement Sa/ω²", "len", sa / (w * w), r.disp[1][0], 1e-6, "Single degree of freedom, spectrum"));
    v.push(check(23, "Response spectrum, flat 2.5 m/s²", "Base shear M·Sa", "force", mass * sa, r.reactions[0].fx, 1e-6, "Single degree of freedom, spectrum"));
    // A two-storey shear frame: equal masses m and storey stiffnesses k (two fixed-ended columns,
    // 12EI/h³ each, under rigid floors): ω² = (k/m)(3 ∓ √5)/2.
    let (hs, bw, mf) = (3.0f64, 6.0, 10_000.0);
    // Columns that bend but do not stretch (a large area), as the shear-frame formula assumes.
    let col = SectionShape::Custom { a: 10.0, i: 0.1 * 0.2f64.powi(3) / 12.0, c: 0.1, iy: 1e-4, j: 1e-4 };
    let shear = |steps: Vec<Step>| {
        let mut d = frame(200e9, col.clone());
        d.m.materials[0].rho = 0.0;
        let stiff = d.apply(Op::Add { item: Item::Section(Section { id: 0, name: "Rigid".into(), shape: SectionShape::Custom { a: 10.0, i: 10.0, c: 1.0, iy: 10.0, j: 10.0 } }) }).expect("section")[0];
        for y in [0.0, hs] {
            member(&mut d, [0.0, y], [0.0, y + hs], MemberKind::Beam);
            member(&mut d, [bw, y], [bw, y + hs], MemberKind::Beam);
        }
        for y in [hs, 2.0 * hs] {
            add(&mut d, Op::AddMember { from: [0.0, y], to: [bw, y], kind: MemberKind::Beam, section: Some(stiff), material: None, z: [0.0; 2] });
        }
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        support(&mut d, bw, 0.0, SupportKind::Fixed);
        for y in [hs, 2.0 * hs] {
            for x in [0.0, bw] {
                let id = node(&d, x, y);
                load(&mut d, Target::Node { id }, LoadKind::Mass { m: mf / 2.0 });
            }
        }
        for st in steps {
            add(&mut d, Op::Add { item: Item::Step(st) });
        }
        d
    };
    let ks = 2.0 * 12.0 * ei / hs.powi(3);
    let (w1, w2) = ((ks / mf * (3.0 - 5f64.sqrt()) / 2.0).sqrt(), (ks / mf * (3.0 + 5f64.sqrt()) / 2.0).sqrt());
    let d = shear(vec![]);
    let fm = crate::frame::modes(&d.m, 2).expect("modes");
    let src2 = "Two masses on two storeys, equal m and k: ω² = (k/m)(3 ∓ √5)/2";
    v.push(check(23, "Two-storey shear frame (rigid floors, columns that do not stretch)", "First frequency", "hz", w1 / TAU, fm.modes[0].f, 1e-4, src2));
    v.push(check(23, "Two-storey shear frame (rigid floors, columns that do not stretch)", "Second frequency", "hz", w2 / TAU, fm.modes[1].f, 1e-4, src2));
    // Spectrum, SRSS: the modes are φ = (1, (1 ± √5)/2), participation Γ = (1 + φ₂)/(1 + φ₂²).
    let st = Step { kind: StepKind::Spectrum, along: [1.0, 0.0, 0.0], spectrum: vec![[0.0, sa], [10.0, sa]], modes: 2, cqc: false, pieces: 1, ..Default::default() };
    let r = solve(&shear(vec![st]));
    let mut top2 = 0.0;
    for (wi, top) in [(w1, (1.0 + 5f64.sqrt()) / 2.0), (w2, (1.0 - 5f64.sqrt()) / 2.0)] {
        // φ = (1, top); Γ = m(1 + top)/(m(1 + top²)).
        let g = (1.0 + top) / (1.0 + top * top);
        let u = g * top * sa / (wi * wi);
        top2 += u * u;
    }
    let ti = node(&d, 0.0, 2.0 * hs);
    let k2 = d.m.nodes.iter().position(|n| n.id == ti).expect("top");
    v.push(check(23, "Two-storey shear frame, flat spectrum, SRSS", "Peak movement of the roof", "len", top2.sqrt(), r.disp[k2][0], 1e-4, src2));
    let dyn_ = r.dynamic.as_ref().expect("modes");
    v.push(check(23, "Two-storey shear frame, flat spectrum, SRSS", "The two modes' effective masses, all of the moving mass", "ratio", 1.0, dyn_.share, 1e-3, "Σ Γ² φᵀMφ = total mass"));
    // The floor slab example (plates and beams in 3D) shaken along x: a time history's results
    // balance with the inertia forces.
    let mut slab = crate::examples::get("slab").expect("slab");
    slab.mesh.size = 0.5;
    let st = Step { id: slab.next, kind: StepKind::History, ground: true, along: [1.0, 0.0, 0.0], damping: 0.05, time: vec![[0.0, 0.0], [0.1, 2.0], [0.2, -2.0], [0.3, 0.0]], dt: 0.005, duration: 0.6, pieces: 4, ..Default::default() };
    slab.next += 1;
    slab.steps = vec![st];
    let Ok(crate::Results::Space(r)) = crate::solve_for(&slab, None) else { panic!("slab history") };
    v.push(check(23, "Floor slab (plates and beams) shaken along x, time history", "Dynamic balance at the time of the largest movement", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics with d'Alembert's inertia forces"));

    v
}

/// Benchmark 24 (0.4.3): structural details. Tension-only braces, a compression-only post, and
/// support settlement, in plane and 3D frames, against statics and beam theory.
fn details() -> Vec<Check> {
    use crate::model::{Analysis, Only};
    let mut v = vec![];
    let rect = SectionShape::Rect { b: 0.1, h: 0.2 };
    // An X-braced square panel of pinned bars, a push H at the top: with the braces tension-only
    // the squeezed one goes slack and the other carries H√2; statics, exactly.
    let (a, h) = (4.0, 10e3);
    let mut d = frame(200e9, rect.clone());
    d.m.materials[0].rho = 0.0;
    for (p, q) in [([0.0, 0.0], [0.0, a]), ([a, 0.0], [a, a]), ([0.0, a], [a, a])] {
        member(&mut d, p, q, MemberKind::Truss);
    }
    member(&mut d, [0.0, 0.0], [a, a], MemberKind::Truss);
    member(&mut d, [a, 0.0], [0.0, a], MemberKind::Truss);
    let n = d.m.members.len();
    for k in [n - 2, n - 1] {
        d.m.members[k].only = Only::Tension;
    }
    support(&mut d, 0.0, 0.0, SupportKind::Pinned);
    support(&mut d, a, 0.0, SupportKind::Pinned);
    let top = node(&d, 0.0, a);
    load(&mut d, Target::Node { id: top }, LoadKind::Force { fx: h, fy: 0.0, fz: 0.0 });
    let Ok(crate::Results::Frame(r)) = crate::solve(&d.m) else { panic!("braced panel") };
    let src = "Statics of the panel with one brace";
    v.push(check(24, "X-braced panel, tension-only braces, a push at the top", "Force in the stretched brace, H√2", "force", h * 2f64.sqrt(), r.members[n - 2].n[20], 1e-9, src));
    v.push(check(24, "X-braced panel, tension-only braces, a push at the top", "Force in the slack brace, 0 (against H)", "ratio", 1.0, 1.0 + r.members[n - 1].n[20].abs() / h, 1e-12, src));
    // The push reversed: the other brace takes it.
    d.m.loads[0].kind = LoadKind::Force { fx: -h, fy: 0.0, fz: 0.0 };
    let Ok(crate::Results::Frame(r)) = crate::solve(&d.m) else { panic!("braced panel") };
    v.push(check(24, "X-braced panel, the push reversed", "Force in the other brace, H√2", "force", h * 2f64.sqrt(), r.members[n - 1].n[20], 1e-9, src));
    // A beam on end supports and a compression-only post at mid-span, lifted at the middle: the
    // post lets go, and the beam deflects as simply supported, PL³/48EI.
    let (l, p) = (6.0, 5e3);
    let ei = 200e9 * 0.1 * 0.2f64.powi(3) / 12.0;
    let mut d = frame(200e9, rect.clone());
    d.m.materials[0].rho = 0.0;
    member(&mut d, [0.0, 1.0], [l / 2.0, 1.0], MemberKind::Beam);
    member(&mut d, [l / 2.0, 1.0], [l, 1.0], MemberKind::Beam);
    member(&mut d, [l / 2.0, 0.0], [l / 2.0, 1.0], MemberKind::Truss);
    d.m.members[2].only = Only::Compression;
    support(&mut d, 0.0, 1.0, SupportKind::Pinned);
    support(&mut d, l, 1.0, SupportKind::RollerX);
    support(&mut d, l / 2.0, 0.0, SupportKind::Pinned);
    let mid = node(&d, l / 2.0, 1.0);
    load(&mut d, Target::Node { id: mid }, LoadKind::Force { fx: 0.0, fy: p, fz: 0.0 });
    let Ok(crate::Results::Frame(r)) = crate::solve(&d.m) else { panic!("post") };
    let k = d.m.nodes.iter().position(|x| x.id == mid).unwrap();
    v.push(check(24, "Beam on a compression-only post, lifted at mid-span", "Mid-span rise PL³/48EI (the post lets go)", "len", p * l.powi(3) / (48.0 * ei), r.disp[k][1], 1e-9, "Beam theory, simply supported"));
    // Support settlement: a propped cantilever whose prop sinks by δ: the prop's reaction
    // 3EIδ/L³ and the fixed end's moment 3EIδ/L². Plane and 3D.
    let delta = -0.01;
    for (an, name) in [(Analysis::Frame, "Propped cantilever, the prop sinks 10 mm"), (Analysis::Space, "Propped cantilever in 3D, the prop sinks 10 mm")] {
        let mut d = frame(200e9, rect.clone());
        d.m.analysis = an;
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        support(&mut d, l, 0.0, if an == Analysis::Frame { SupportKind::RollerX } else { SupportKind::Pinned });
        let end = node(&d, l, 0.0);
        load(&mut d, Target::Node { id: end }, LoadKind::Settlement { dx: 0.0, dy: delta, dz: 0.0 });
        let (prop, root) = match crate::solve(&d.m).expect("settlement") {
            crate::Results::Frame(r) => (r.reactions[1].fy, r.reactions[0].m),
            crate::Results::Space(r) => (r.reactions[1].f[1], r.reactions[0].f[5]),
            _ => unreachable!(),
        };
        let src = "Beam theory: a propped cantilever with its prop moved (slope–deflection)";
        v.push(check(24, name, "Prop reaction 3EIδ/L³", "force", 3.0 * ei * delta / l.powi(3), prop, 1e-9, src));
        v.push(check(24, name, "Fixed-end moment 3EIδ/L²", "moment", (3.0 * ei * delta / (l * l)).abs(), root.abs(), 1e-9, src));
    }
    // Second order in 3D frames (P-delta): the AISC 360 C2 cases again, bending about the
    // section's weak axis (EIy = E h b³/12), out of the drawing plane.
    {
        use crate::model::{Step, StepKind};
        use std::f64::consts::PI;
        let eiy = 200e9 * 0.2 * 0.1f64.powi(3) / 12.0;
        let step = |d: &mut Doc| add(d, Op::Add { item: Item::Step(Step { kind: StepKind::Newton, pieces: 10, ..Default::default() }) });
        // Flagpole along y, the side load along z.
        let (lf, hf) = (5.0, 2e3);
        let p = 0.5 * PI * PI * eiy / (4.0 * lf * lf);
        let mut d = frame(200e9, rect.clone());
        d.m.analysis = Analysis::Space;
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [0.0, lf], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let top = node(&d, 0.0, lf);
        load(&mut d, Target::Node { id: top }, LoadKind::Force { fx: 0.0, fy: -p, fz: hf });
        step(&mut d);
        let Ok(crate::Results::Space(r)) = crate::solve_for(&d.m, None) else { panic!("3D flagpole") };
        let k = (p / eiy).sqrt();
        let src = "AISC 360-16 Commentary C2, benchmark case 2 (closed form), in 3D";
        v.push(check(24, "3D flagpole, second order (P-delta), P = 0.5 P_cr about the weak axis", "Top deflection, amplified", "len", hf / (k * p) * ((k * lf).tan() - k * lf), r.disp[1][2], 0.005, src));
        v.push(check(24, "3D flagpole, second order (P-delta), P = 0.5 P_cr about the weak axis", "Base moment, amplified", "moment", hf * (k * lf).tan() / k, r.reactions[0].f[3].abs(), 0.005, src));
        v.push(check(24, "3D flagpole, second order (P-delta), P = 0.5 P_cr about the weak axis", "Balance of loads and reactions in the deflected shape", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics on the deflected frame"));
        let _ = step;
    }
    // Rigid floors (0.4.4): four cantilever columns at the corners of an a × a square, tied only
    // by a rigid floor, pushed along x at one corner. The floor moves H/4k and turns by
    // M/(2ka² + 4GJ/h) about its middle (k = 3EI/h³ each way, GJ/h each column's twist), so a
    // corner moves H/4k ± Ha²/4Kθ (rigid-diaphragm statics).
    {
        let (a, hh, h) = (4.0f64, 3.0f64, 10e3);
        let sq = SectionShape::Rect { b: 0.2, h: 0.2 };
        let pr = crate::sections::props(&sq).expect("props");
        let e = 200e9;
        let g = e / (2.0 * 1.3);
        let k = 3.0 * e * pr.i / hh.powi(3);
        let kt = 2.0 * k * a * a + 4.0 * g * pr.j / hh;
        let mut d = frame(e, sq);
        d.m.analysis = Analysis::Space;
        d.m.materials[0].rho = 0.0;
        d.m.rigid_floors = true;
        for (x, z) in [(0.0, 0.0), (a, 0.0), (0.0, a), (a, a)] {
            add(&mut d, Op::AddMember { from: [x, 0.0], to: [x, hh], kind: MemberKind::Beam, section: None, material: None, z: [z, z] });
        }
        let at = |d: &Doc, p: [f64; 3]| d.m.nodes.iter().find(|n| crate::model::dist3(n.at(), p) < 1e-9).map(|n| n.id).expect("node");
        for (x, z) in [(0.0, 0.0), (a, 0.0), (0.0, a), (a, a)] {
            let id = at(&d, [x, 0.0, z]);
            add(&mut d, Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind: SupportKind::Fixed }) });
        }
        let c0 = at(&d, [0.0, hh, 0.0]);
        let c3 = at(&d, [a, hh, a]);
        load(&mut d, Target::Node { id: c0 }, LoadKind::Force { fx: h, fy: 0.0, fz: 0.0 });
        let Ok(crate::Results::Space(r)) = crate::solve(&d.m) else { panic!("rigid floor") };
        let k0 = d.m.nodes.iter().position(|n| n.id == c0).expect("c0");
        let k3 = d.m.nodes.iter().position(|n| n.id == c3).expect("c3");
        let src = "Rigid-diaphragm statics: translation H/4k, twist M/Σ(k r² + GJ/h)";
        let name = "Rigid floor on four cantilever columns, pushed at one corner";
        v.push(check(24, name, "Movement of the corner pushed, H/4k + Ha²/4Kθ", "len", h / (4.0 * k) + h * a * a / (4.0 * kt), r.disp[k0][0], 1e-5, src));
        v.push(check(24, name, "Movement of the opposite corner, H/4k − Ha²/4Kθ", "len", h / (4.0 * k) - h * a * a / (4.0 * kt), r.disp[k3][0], 1e-5, src));
        v.push(check(24, name, "Balance of the loads and reactions (the floor's ties are internal)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics"));
    }
    // Member end offsets (0.4.5): rigid zones bend nothing, so a member bends as if it were
    // only its flexible length.
    {
        let (l, o, pp) = (4.0f64, 0.5f64, 5e3);
        let mut d = frame(200e9, rect.clone());
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
        d.m.members[0].offset = [o, 0.0];
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let tip = node(&d, l, 0.0);
        load(&mut d, Target::Node { id: tip }, LoadKind::Force { fx: 0.0, fy: -pp, fz: 0.0 });
        let Ok(crate::Results::Frame(r)) = crate::solve(&d.m) else { panic!("offset cantilever") };
        let lf = l - o;
        let src = "Beam theory over the flexible length (the rigid zone, 10⁵ times stiffer, bends about 10⁻⁵ as much)";
        v.push(check(24, "Cantilever with a 0.5 m rigid zone at its root", "Tip deflection P(L − o)³/3EI", "len", pp * lf.powi(3) / (3.0 * ei), -r.disp[1][1], 2e-5, src));
        v.push(check(24, "Cantilever with a 0.5 m rigid zone at its root", "Root moment PL (the whole lever)", "moment", pp * l, r.reactions[0].m.abs(), 1e-9, "Statics"));
        // 3D: a beam fixed at both ends with 0.4 m rigid zones, a uniform load: it deflects as a
        // fixed-ended beam of the flexible length, wLf⁴/384EI.
        let (l, o, w) = (6.0f64, 0.4f64, -10e3);
        let mut d = frame(200e9, rect.clone());
        d.m.analysis = Analysis::Space;
        d.m.materials[0].rho = 0.0;
        member(&mut d, [0.0, 0.0], [l / 2.0, 0.0], MemberKind::Beam);
        member(&mut d, [l / 2.0, 0.0], [l, 0.0], MemberKind::Beam);
        d.m.members[0].offset = [o, 0.0];
        d.m.members[1].offset = [0.0, o];
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        support(&mut d, l, 0.0, SupportKind::Fixed);
        for id in d.m.members.iter().map(|b| b.id).collect::<Vec<_>>() {
            load(&mut d, Target::Member { id }, LoadKind::Dist { w1: w, w2: w, dir: Dir::Y });
        }
        let Ok(crate::Results::Space(r)) = crate::solve(&d.m) else { panic!("offset beam") };
        let mid = d.m.nodes.iter().position(|n| (n.x - l / 2.0).abs() < 1e-9).expect("mid");
        let lf = l - 2.0 * o;
        v.push(check(24, "3D beam fixed at both ends, 0.4 m rigid zones, uniform load", "Mid-span deflection wLf⁴/384EI", "len", w.abs() * lf.powi(4) / (384.0 * ei), -r.disp[mid][1], 2e-5, src));
        v.push(check(24, "3D beam fixed at both ends, 0.4 m rigid zones, uniform load", "Balance of the loads and reactions", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics"));
    }
    v
}

/// Benchmark 25 (0.5.0): plasticity of parts, von Mises (J2) with Newton's method and the
/// consistent tangent, against the closed forms of limit analysis and of a pulled strip.
fn plasticity() -> Vec<Check> {
    use crate::model::{Step, StepKind};
    let mut v = vec![];
    let fy = 250e6;
    let q = 0.414_213_562_373_095; // tan(90°/4): a quarter-circle arc
    let ring = |a: f64, b: f64| Geom::Poly { pts: vec![[a, 0.0], [b, 0.0], [0.0, b], [0.0, a]], bulge: vec![0.0, q, 0.0, -q] };
    let collapse = |analysis: Analysis, over: f64, size: f64| -> (f64, usize) {
        let (mut d, ids) = part(analysis, 200e9, 0.3, vec![(ring(0.1, 0.2), false)]);
        d.m.materials[0].fy = fy;
        d.m.materials[0].rho = 0.0;
        d.m.mesh.size = size;
        let id = ids[0];
        edge_support(&mut d, id, 0, SupportKind::Roller);
        if analysis != Analysis::Axi {
            edge_support(&mut d, id, 2, SupportKind::Roller);
        }
        edge_load(&mut d, id, 3, LoadKind::Pressure { p: over });
        add(&mut d, Op::Add { item: Item::Step(Step { kind: StepKind::Newton, increments: 12, ..Default::default() }) });
        let Ok(crate::Results::Plane(r)) = crate::solve_for(&d.m, None) else { panic!("plastic part") };
        (r.path.as_ref().map_or(0.0, |p| p.load.last().copied().unwrap_or(0.0)) * over, r.mesh.tris.len())
    };
    // A thick cylinder (a = 0.1, b = 0.2 m) in plane strain under internal pressure, perfectly
    // plastic: it collapses at p = 2k ln(b/a), k = σy/√3 (Hill, The Mathematical Theory of
    // Plasticity, 1950, ch. V).
    let pc = 2.0 * fy / 3f64.sqrt() * 2f64.ln();
    let src = "Hill 1950, The Mathematical Theory of Plasticity, ch. V (limit analysis: independent of E and ν); the largest load carried, so it is reached from below";
    v.push(check(25, "Thick cylinder under internal pressure, plane strain, perfectly plastic (b/a = 2)", "Collapse pressure 2k ln(b/a), k = σy/√3", "stress", pc, collapse(Analysis::Strain, 1.1 * pc, 0.02).0, 0.01, src));
    // A thick sphere, as an axisymmetric solid: it collapses at p = 2σy ln(b/a).
    let ps = 2.0 * fy * 2f64.ln();
    let runs: Vec<(f64, usize)> = [0.025, 0.0125].iter().map(|&h| collapse(Analysis::Axi, 1.1 * ps, h)).collect();
    let steps = runs.iter().map(|(p, n)| [*n as f64, (p - ps).abs() / ps]).collect();
    v.push(with_steps(check(25, "Thick sphere under internal pressure, axisymmetric, perfectly plastic (b/a = 2)", "Collapse pressure 2σy ln(b/a)", "stress", ps, runs[1].0, 0.01, src), steps));
    // A strip 1 m × 0.1 m in plane stress pulled to 1.5 σy, hardening H = 2 GPa: it stretches
    // L(σ/E + (σ − σy)/H), exactly (a uniform state).
    let (mut d, ids) = part(Analysis::Stress, 200e9, 0.3, vec![(Geom::rect(0.0, 0.0, 1.0, 0.1), false)]);
    let h = 2e9;
    d.m.materials[0].fy = fy;
    d.m.materials[0].hardening = h;
    d.m.materials[0].rho = 0.0;
    d.m.mesh.size = 0.05;
    let id = ids[0];
    edge_support(&mut d, id, 3, SupportKind::Roller);
    edge_support(&mut d, id, 0, SupportKind::Roller);
    let sig = 1.5 * fy;
    let force = sig * 0.1 * d.m.thickness;
    edge_load(&mut d, id, 1, LoadKind::Traction { fx: force, fy: 0.0, total: true, fz: 0.0 });
    add(&mut d, Op::Add { item: Item::Step(Step { kind: StepKind::Newton, increments: 6, ..Default::default() }) });
    let Ok(crate::Results::Plane(r)) = crate::solve_for(&d.m, None) else { panic!("strip") };
    let ux = r.field("ux").max;
    let e = 200e9;
    v.push(check(25, "Strip pulled past yield, plane stress, hardening H = 2 GPa", "Stretch L(σ/E + (σ − σy)/H)", "len", sig / e + (sig - fy) / h, ux, 1e-6, "Uniaxial stress with linear hardening (exact)"));
    v.push(check(25, "Strip pulled past yield, plane stress, hardening H = 2 GPa", "Equivalent plastic strain (σ − σy)/H", "ratio", (sig - fy) / h, r.field("peeq").max, 1e-6, "Uniaxial stress with linear hardening (exact)"));
    v
}

/// Benchmark 26 (0.5.1): plasticity in 3D solids. The same collapse loads as benchmark 25, now
/// from wedge elements: a thick cylinder extruded and held between its end faces (plane strain),
/// a thick sphere revolved a slice, and a bar pulled past yield with hardening.
fn solid_plasticity() -> Vec<Check> {
    use crate::model::{Step, StepKind};
    let mut v = vec![];
    let fy = 250e6;
    let q = 0.414_213_562_373_095;
    let ring = |a: f64, b: f64| Geom::Poly { pts: vec![[a, 0.0], [b, 0.0], [0.0, b], [0.0, a]], bulge: vec![0.0, q, 0.0, -q] };
    let face = |d: &mut Doc, front: bool| { add_id(d, Item::Support(Support { id: 0, at: Target::End { front }, kind: SupportKind::Symmetry })); };
    let newton = |d: &mut Doc, n: u32| { add(d, Op::Add { item: Item::Step(Step { kind: StepKind::Newton, increments: n, ..Default::default() }) }); };
    let src = "Hill 1950, The Mathematical Theory of Plasticity, ch. V (limit analysis: independent of E and ν); the largest load carried, so it is reached from below";
    let collapse = |revolve: f64, over: f64| -> f64 {
        let (mut d, ids) = part(Analysis::Solid, 200e9, 0.3, vec![(ring(0.1, 0.2), false)]);
        d.m.materials[0].fy = fy;
        d.m.materials[0].rho = 0.0;
        d.m.mesh.size = if revolve > 0.0 { 0.0125 } else { 0.02 };
        d.m.mesh.layers = 1;
        d.m.thickness = 0.02;
        d.m.revolve = revolve;
        let id = ids[0];
        edge_support(&mut d, id, 0, SupportKind::Symmetry);
        if revolve == 0.0 {
            edge_support(&mut d, id, 2, SupportKind::Symmetry);
        }
        face(&mut d, false);
        face(&mut d, true);
        edge_load(&mut d, id, 3, LoadKind::Pressure { p: over });
        newton(&mut d, 12);
        let Ok(crate::Results::Solid(r)) = crate::solve_for(&d.m, None) else { panic!("plastic solid") };
        r.path.as_ref().map_or(0.0, |p| p.load.last().copied().unwrap_or(0.0)) * over
    };
    let pc = 2.0 * fy / 3f64.sqrt() * 2f64.ln();
    v.push(check(26, "Thick cylinder, 3D solid held between its end faces, perfectly plastic (b/a = 2)", "Collapse pressure 2k ln(b/a), k = σy/√3", "stress", pc, collapse(0.0, 1.1 * pc), 0.01, src));
    let ps = 2.0 * fy * 2f64.ln();
    v.push(check(26, "Thick sphere, 3D solid revolved 10°, perfectly plastic (b/a = 2)", "Collapse pressure 2σy ln(b/a)", "stress", ps, collapse(10f64.to_radians(), 1.1 * ps), 0.01, src));
    // A bar 1 × 0.1 × 0.1 m pulled to 1.5 σy, hardening H = 2 GPa: exact, a uniform state.
    let (mut d, ids) = part(Analysis::Solid, 200e9, 0.3, vec![(Geom::rect(0.0, 0.0, 1.0, 0.1), false)]);
    let h = 2e9;
    d.m.materials[0].fy = fy;
    d.m.materials[0].hardening = h;
    d.m.materials[0].rho = 0.0;
    d.m.mesh.size = 0.05;
    d.m.mesh.layers = 2;
    d.m.thickness = 0.1;
    let id = ids[0];
    edge_support(&mut d, id, 3, SupportKind::Symmetry);
    edge_support(&mut d, id, 0, SupportKind::Symmetry);
    face(&mut d, false);
    let sig = 1.5 * fy;
    edge_load(&mut d, id, 1, LoadKind::Traction { fx: sig * 0.1 * 0.1, fy: 0.0, total: true, fz: 0.0 });
    newton(&mut d, 6);
    let Ok(crate::Results::Solid(r)) = crate::solve_for(&d.m, None) else { panic!("bar") };
    let e = 200e9;
    let name = "Bar pulled past yield, 3D solid, hardening H = 2 GPa";
    let exact = "Uniaxial stress with linear hardening (exact)";
    v.push(check(26, name, "Stretch L(σ/E + (σ − σy)/H)", "len", sig / e + (sig - fy) / h, r.field("ux").max, 1e-6, exact));
    v.push(check(26, name, "Equivalent plastic strain (σ − σy)/H", "ratio", (sig - fy) / h, r.field("peeq").max, 1e-6, exact));
    v.push(check(26, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics"));
    v
}

/// The elastica of a cantilever under a dead transverse end load (0.5.2): θ'' = −α cos θ along
/// s ∈ [0, 1] with α = PL²/EI, θ(0) = 0 and θ'(1) = 0, shot on θ'(0) by bisection with a
/// fourth-order Runge–Kutta march. Returns the tip's movement across and along (over L) and its
/// rotation. Mattiasson (1981) tabulates the same: for α = 2, 0.49346, 0.16064 and 0.78175.
fn elastica_tip(alpha: f64) -> (f64, f64, f64) {
    let march = |k0: f64| -> (f64, f64, f64, f64) {
        let n = 20_000;
        let h = 1.0 / n as f64;
        let (mut t, mut k, mut x, mut y) = (0.0f64, k0, 0.0f64, 0.0f64);
        let f = |t: f64, k: f64| (k, -alpha * t.cos(), t.cos(), t.sin());
        for _ in 0..n {
            let a = f(t, k);
            let b = f(t + h / 2.0 * a.0, k + h / 2.0 * a.1);
            let c = f(t + h / 2.0 * b.0, k + h / 2.0 * b.1);
            let d = f(t + h * c.0, k + h * c.1);
            x += h / 6.0 * (a.2 + 2.0 * b.2 + 2.0 * c.2 + d.2);
            y += h / 6.0 * (a.3 + 2.0 * b.3 + 2.0 * c.3 + d.3);
            t += h / 6.0 * (a.0 + 2.0 * b.0 + 2.0 * c.0 + d.0);
            k += h / 6.0 * (a.1 + 2.0 * b.1 + 2.0 * c.1 + d.1);
        }
        (k, x, y, t)
    };
    let (mut lo, mut hi) = (0.0, alpha);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if march(mid).0 > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let (_, x, y, t) = march((lo + hi) / 2.0);
    (y, 1.0 - x, t)
}

/// Benchmark 27 (0.5.2): large deformation of 3D solids. A rubber bar stretched to twice its
/// length and squeezed to 0.6 of it, against the neo-Hookean model's exact uniaxial answer; and a
/// slender steel cantilever bent far by an end load, against the elastica.
fn rubber() -> Vec<Check> {
    use crate::model::{Step, StepKind};
    use crate::solids::hyper::NeoHooke;
    let mut v = vec![];
    let large = |d: &mut Doc, n: u32| { add(d, Op::Add { item: Item::Step(Step { kind: StepKind::Large, increments: n, ..Default::default() }) }); };
    // A rubber bar 0.2 × 0.02 × 0.02 m (a quarter of it by symmetry is not needed: rollers on
    // three faces leave it free to thin), μ = 1 MPa, ν = 0.49.
    let (e, nu) = (2.98e6, 0.49);
    let nh = NeoHooke::new(e, nu);
    let src = "Neo-Hookean uniaxial tension, exact (Bonet and Wood §6.5): the sides' stretch found so the stress across is zero; Rivlin's P = μ(λ − λ⁻²) in the incompressible limit";
    for (lam, name) in [(2.0, "Rubber bar stretched to twice its length (neo-Hookean, ν = 0.49)"), (0.6, "Rubber bar squeezed to 0.6 of its length (neo-Hookean, ν = 0.49)")] {
        let (p, side) = nh.uniaxial(lam);
        let (mut d, ids) = part(Analysis::Solid, e, nu, vec![(Geom::rect(0.0, 0.0, 0.2, 0.02), false)]);
        d.m.materials[0].rho = 0.0;
        d.m.thickness = 0.02;
        d.m.mesh.size = 0.01;
        d.m.mesh.layers = 1;
        let id = ids[0];
        edge_support(&mut d, id, 3, SupportKind::Symmetry);
        edge_support(&mut d, id, 0, SupportKind::Symmetry);
        add_id(&mut d, Item::Support(Support { id: 0, at: Target::End { front: false }, kind: SupportKind::Symmetry }));
        edge_load(&mut d, id, 1, LoadKind::Traction { fx: p * 0.02 * 0.02, fy: 0.0, total: true, fz: 0.0 });
        large(&mut d, 10);
        let Ok(crate::Results::Solid(r)) = crate::solve_for(&d.m, None) else { panic!("rubber bar") };
        v.push(check(27, name, "End movement (λ − 1) L", "len", (lam - 1.0) * 0.2, at_solid(&r, "ux", [0.2, 0.01, 0.01]), 1e-6, src));
        v.push(check(27, name, "Side movement (λ⊥ − 1) b", "len", (side - 1.0) * 0.02, at_solid(&r, "uy", [0.1, 0.02, 0.01]), 1e-6, src));
        v.push(check(27, name, "True stress along it, force over the deformed area", "stress", p / (side * side), at_solid(&r, "sx", [0.1, 0.01, 0.01]), 1e-6, src));
        v.push(check(27, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics on the deformed solid"));
    }
    // A steel cantilever 1 m long, 50 × 50 mm, under a dead end load P = 2 EI/L²: it bends to
    // half its length down.
    let (e, l, h) = (200e9, 1.0, 0.05f64);
    let ei = e * h * h.powi(3) / 12.0;
    let alpha = 2.0;
    let (mut d, ids) = part(Analysis::Solid, e, 0.3, vec![(Geom::rect(0.0, 0.0, l, h), false)]);
    d.m.materials[0].rho = 0.0;
    d.m.thickness = h;
    d.m.mesh.size = 0.025;
    d.m.mesh.layers = 1;
    let id = ids[0];
    edge_support(&mut d, id, 3, SupportKind::Fixed);
    edge_load(&mut d, id, 1, LoadKind::Traction { fx: 0.0, fy: -alpha * ei / (l * l), total: true, fz: 0.0 });
    large(&mut d, 10);
    let Ok(crate::Results::Solid(r)) = crate::solve_for(&d.m, None) else { panic!("cantilever") };
    let (dv, du, _) = elastica_tip(alpha);
    let name = "Steel cantilever bent far by a dead end load, 3D solid (PL²/EI = 2)";
    let src = "The elastica of a cantilever under an end load (Bisshopp and Drucker 1945; Mattiasson 1981: 0.49346 L down, 0.16064 L in)";
    v.push(check(27, name, "Tip movement down", "len", -dv * l, at_solid(&r, "uy", [l, h / 2.0, h / 2.0]), 0.01, src));
    v.push(check(27, name, "Tip movement in towards the root", "len", -du * l, at_solid(&r, "ux", [l, h / 2.0, h / 2.0]), 0.01, src));
    v.push(check(27, name, "Equilibrium (load over reactions, moments about the bent shape)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics on the deformed solid"));
    v
}

/// Benchmark 28 (0.5.5): tetrahedra (4- and 10-node) on meshes made the way an imported one is:
/// a bar pulled (exact), a cantilever (beam theory with shear), and a thick cylinder (Lamé).
fn tetra() -> Vec<Check> {
    use crate::solids::tet::{grid, TetMesh};
    let mut v = vec![];
    let solid = |ms: TetMesh, e: f64, nu: f64| { let mut d = Doc::new(Model::new(Analysis::Solid)); d.m.materials[0].e = e; d.m.materials[0].nu = nu; d.m.materials[0].rho = 0.0; d.m.volume = Some(Box::new(ms)); d };
    let on = |d: &Doc, f: &dyn Fn([f64; 3]) -> bool| Target::Face { face: d.m.volume.as_ref().unwrap().face_where(f).expect("face") };
    let sup = |d: &mut Doc, at: Target, kind: SupportKind| { add_id(d, Item::Support(Support { id: 0, at, kind })); };
    let load = |d: &mut Doc, at: Target, kind: LoadKind| { add_id(d, Item::Load(Load { case: 0, id: 0, at, kind })); };
    let at3 = |r: &crate::solid::SolidResults, k: &str, p: [f64; 3]| { let i = r.volume.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, p).total_cmp(&crate::model::dist3(*b.1, p))).unwrap().0; r.volume.v[r.fields.iter().position(|f| f.key == k).unwrap()][i] };
    // A bar 1 × 0.1 × 0.1 m pulled by 100 MPa, symmetry on three faces: a uniform state, exact for both elements.
    let (e, nu, p) = (200e9, 0.3, 100e6);
    for (order, name) in [(1u8, "Bar pulled, 4-node tetrahedra"), (2, "Bar pulled, 10-node tetrahedra")] {
        let mut d = solid(grid([10, 2, 2], order, |q| [q[0], 0.1 * q[1], 0.1 * q[2]]).unwrap(), e, nu);
        for (c, val) in [(0, 0.0), (1, 0.0), (2, 0.0)] {
            let t = on(&d, &move |q: [f64; 3]| (q[c] - val).abs() < 1e-9);
            sup(&mut d, t, SupportKind::Symmetry);
        }
        let end = on(&d, &|q| (q[0] - 1.0).abs() < 1e-9);
        load(&mut d, end, LoadKind::Traction { fx: p, fy: 0.0, fz: 0.0, total: false });
        let r = crate::solids::tet::solve(&d.m).expect("bar");
        let src = "Uniaxial stress (exact)";
        v.push(check(28, name, "Stress along the bar", "stress", p, r.field("sx").max, 1e-9, src));
        v.push(check(28, name, "Stretch pL/E", "len", p / e, r.field("ux").max, 1e-9, src));
        v.push(check(28, name, "Thinning −νpb/E", "len", -nu * p * 0.1 / e, r.field("uy").min, 1e-9, src));
    }
    // A cantilever 1 m long, 0.1 × 0.1 m, fixed at its root, 10 kN down on its tip face: the tip
    // moves PL³/3EI + PL/κGA (shear, κ = 5/6), 10-node tetrahedra.
    {
        let (l, h, pf) = (1.0, 0.1, 10e3);
        let mut d = solid(grid([40, 5, 5], 2, |q| [l * q[0], h * q[1], h * q[2]]).unwrap(), e, nu);
        let root = on(&d, &|q| q[0].abs() < 1e-9);
        sup(&mut d, root, SupportKind::Fixed);
        let tip = on(&d, &|q| (q[0] - l).abs() < 1e-9);
        load(&mut d, tip, LoadKind::Traction { fx: 0.0, fy: -pf, fz: 0.0, total: true });
        let r = crate::solids::tet::solve(&d.m).expect("cantilever");
        let i = h.powi(4) / 12.0;
        let g = e / (2.0 * (1.0 + nu));
        let want = pf * l.powi(3) / (3.0 * e * i) + pf * l / (5.0 / 6.0 * g * h * h);
        let name = "Cantilever, end load, 10-node tetrahedra";
        v.push(check(28, name, "Tip deflection PL³/3EI + PL/κGA", "len", -want, at3(&r, "uy", [l, h / 2.0, h / 2.0]), 0.01, "Timoshenko beam theory (a fixed root is a little stiffer)"));
        v.push(check(28, name, "Root bending stress 6PL/bh², at the top", "stress", 6.0 * pf * l / h.powi(3), at3(&r, "sx", [0.25 * l, h, h / 2.0]) / 0.75, 0.02, "Beam theory, a quarter of the way along (away from the root)"));
        v.push(check(28, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics"));
    }
    // A thick cylinder, a = 0.1, b = 0.2 m, a quarter of it held between its end faces (plane
    // strain), 100 MPa inside: Lamé's hoop stress and bore growth, 10-node tetrahedra.
    {
        let (a, b, hh, pi) = (0.1, 0.2, 0.05, 100e6);
        let ms = grid([6, 8, 1], 2, |q| { let (r, t) = (a + (b - a) * q[0], q[1] * std::f64::consts::FRAC_PI_2); [r * t.cos(), r * t.sin(), hh * q[2]] }).unwrap();
        let mut d = solid(ms, e, nu);
        for (c, val) in [(1, 0.0), (0, 0.0), (2, 0.0), (2, hh)] {
            let t = on(&d, &move |q: [f64; 3]| (q[c] - val).abs() < 1e-9);
            sup(&mut d, t, SupportKind::Symmetry);
        }
        let bore = on(&d, &|q| ((q[0] * q[0] + q[1] * q[1]).sqrt() - a).abs() < 1e-9);
        load(&mut d, bore, LoadKind::Pressure { p: pi });
        let r = crate::solids::tet::solve(&d.m).expect("cylinder");
        let k = a * a / (b * b - a * a);
        let hoop = pi * k * (1.0 + b * b / (a * a));
        let grow = (1.0 + nu) * pi * k * a / e * ((1.0 - 2.0 * nu) + b * b / (a * a));
        let name = "Thick cylinder in plane strain, 10-node tetrahedra (a quarter)";
        let src = "Lamé thick-cylinder equations, plane strain";
        v.push(check(28, name, "Hoop stress at the bore p(b²+a²)/(b²−a²)", "stress", hoop, at3(&r, "sy", [a, 0.0, hh / 2.0]), 0.01, src));
        v.push(check(28, name, "Bore growth", "len", grow, at3(&r, "ux", [a, 0.0, hh / 2.0]), 0.005, src));
    }
    v
}

/// Benchmark 35 (0.7.7): a remote force and a remote moment carried onto a face by a
/// distributing coupling, on a 10-node tetrahedral cantilever 1 × 0.1 × 0.1 m fixed at its root.
fn remote_load() -> Vec<Check> {
    use crate::solids::tet::{grid, TetMesh};
    let mut v = vec![];
    let (l, h, e, nu) = (1.0, 0.1, 200e9, 0.3);
    let g = e / (2.0 * (1.0 + nu));
    let model = |ms: TetMesh| {
        let mut d = Doc::new(Model::new(Analysis::Solid));
        d.m.materials[0].e = e;
        d.m.materials[0].nu = nu;
        d.m.materials[0].rho = 0.0;
        d.m.volume = Some(Box::new(ms));
        let root = Target::Face { face: d.m.volume.as_ref().unwrap().face_where(|q| q[0].abs() < 1e-9).expect("root") };
        add_id(&mut d, Item::Support(Support { id: 0, at: root, kind: SupportKind::Fixed }));
        d
    };
    let tip = |d: &Doc| Target::Face { face: d.m.volume.as_ref().unwrap().face_where(|q| (q[0] - l).abs() < 1e-9).expect("tip") };
    let near = |r: &crate::solid::SolidResults, k: &str, p: [f64; 3]| {
        let i = r.volume.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, p).total_cmp(&crate::model::dist3(*b.1, p))).unwrap().0;
        r.volume.v[r.fields.iter().position(|f| f.key == k).unwrap()][i]
    };
    // 10 kN down at a point half a metre beyond the tip, on its axis: the tip carries the force
    // and a moment of half a metre times it, both bending it down.
    {
        let pf = 10e3;
        let mut d = model(grid([40, 5, 5], 2, |q| [l * q[0], h * q[1], h * q[2]]).unwrap());
        let t = tip(&d);
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: t, kind: LoadKind::Remote { at: [1.5 * l, h / 2.0, h / 2.0], f: [0.0, -pf, 0.0], m: [0.0; 3] } }));
        let r = crate::solids::tet::solve(&d.m).expect("remote force");
        let i = h.powi(4) / 12.0;
        let want = pf * l.powi(3) / (3.0 * e * i) + 0.5 * pf * l * l * l / (2.0 * e * i) + pf * l / (5.0 / 6.0 * g * h * h);
        let name = "Cantilever, a force 0.5 m beyond its tip carried by a remote load, 10-node tetrahedra";
        v.push(check(35, name, "Tip deflection PL³/3EI + (Pa)L²/2EI + PL/κGA", "len", -want, near(&r, "uy", [l, h / 2.0, h / 2.0]), 0.01, "Timoshenko beam theory (a fixed root is a little stiffer)"));
        v.push(check(35, name, "Bending stress 6P(L + a − x)/bh² at a quarter span, top", "stress", 6.0 * pf * (1.5 * l - 0.25 * l) / h.powi(3), near(&r, "sx", [0.25 * l, h, h / 2.0]), 0.02, "Beam theory: the remote force's moment carried down the bar"));
        v.push(check(35, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics"));
    }
    // A torque of 1 kN·m about the bar's axis at its tip: the tip turns TL/GJ, J = 0.1406 b⁴.
    {
        let tq = 1e3;
        let mut d = model(grid([40, 6, 6], 2, |q| [l * q[0], h * q[1], h * q[2]]).unwrap());
        let t = tip(&d);
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: t, kind: LoadKind::Remote { at: [l, h / 2.0, h / 2.0], f: [0.0; 3], m: [tq, 0.0, 0.0] } }));
        let r = crate::solids::tet::solve(&d.m).expect("remote torque");
        let x = 0.75 * l;
        let turn = (near(&r, "uz", [x, h, h / 2.0]) - near(&r, "uz", [x, 0.0, h / 2.0])) / h;
        let name = "Square bar twisted by a remote moment on its tip, 10-node tetrahedra";
        v.push(check(35, name, "Twist three quarters along, Tx/GJ (J = 0.1406 b⁴)", "ratio", tq * x / (g * 0.1406 * h.powi(4)), turn, 0.03, "Saint-Venant torsion of a square bar (Timoshenko and Goodier); the root's held warping makes it a little stiffer"));
        v.push(check(35, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics"));
    }
    v
}

/// Benchmark 36 (0.8.0): contact between two blocks meshed apart, 10-node tetrahedra. A block
/// pressed onto another through a frictionless contact whose meshes do not match, and a block
/// pressed down and dragged sideways over a fixed one with friction.
fn contact() -> Vec<Check> {
    use crate::model::Contact;
    use crate::solids::tet::{grid, TetMesh};
    let mut v = vec![];
    let (e, nu) = (200e9, 0.3);
    // Two meshes in one, apart; and the face of either that lies where `inside` says.
    let join = |a: TetMesh, b: TetMesh| {
        let (na, ta, fa) = (a.nodes.len() as u32, a.tets.len() as u32, a.faces.len());
        let mut m = a;
        m.nodes.extend(b.nodes);
        m.tets.extend(b.tets.into_iter().map(|t| t.into_iter().map(|q| q + na).collect()));
        m.faces.extend(b.faces.into_iter().map(|mut f| {
            f.facets.iter_mut().for_each(|x| x.0 += ta);
            f
        }));
        (m, fa)
    };
    let face_of = |ms: &TetMesh, from: usize, to: usize, inside: &dyn Fn([f64; 3]) -> bool| {
        (from..to).find(|&f| ms.faces[f].facets.iter().all(|&(el, k)| ms.facet(el as usize, k as usize)[..3].iter().all(|&i| inside(ms.nodes[i as usize])))).map(|f| f as u32).expect("face")
    };
    let solid = |ms: TetMesh| {
        let mut d = Doc::new(Model::new(Analysis::Solid));
        d.m.materials[0].e = e;
        d.m.materials[0].nu = nu;
        d.m.materials[0].rho = 0.0;
        d.m.volume = Some(Box::new(ms));
        d
    };
    let sup = |d: &mut Doc, face: u32, kind: SupportKind| add_id(d, Item::Support(Support { id: 0, at: Target::Face { face }, kind }));
    // A 1 m cube on another, rollers on two sides and under the lower one; 10 MPa on top. The
    // stress is uniform, −p, in both, and the top moves down 2pL/E with no sideways restraint
    // (uniaxial stress).
    {
        let p = 10e6;
        let lower = grid([3, 3, 3], 2, |q| q).unwrap();
        let upper = grid([4, 4, 4], 2, |q| [q[0], q[1], 1.0 + q[2]]).unwrap();
        let nf = lower.faces.len();
        let (ms, fa) = join(lower, upper);
        let nall = ms.faces.len();
        let mut d = solid(ms);
        let m = d.m.volume.as_ref().unwrap().clone();
        for (lo, hi) in [(0, fa), (fa, nall)] {
            sup(&mut d, face_of(&m, lo, hi, &|q| q[0].abs() < 1e-9), SupportKind::Symmetry);
            sup(&mut d, face_of(&m, lo, hi, &|q| q[1].abs() < 1e-9), SupportKind::Symmetry);
        }
        sup(&mut d, face_of(&m, 0, nf, &|q| q[2].abs() < 1e-9), SupportKind::Symmetry);
        let top = face_of(&m, fa, nall, &|q| (q[2] - 2.0).abs() < 1e-9);
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Face { face: top }, kind: LoadKind::Pressure { p } }));
        let a = face_of(&m, fa, nall, &|q| (q[2] - 1.0).abs() < 1e-9);
        let b = face_of(&m, 0, nf, &|q| (q[2] - 1.0).abs() < 1e-9);
        add_id(&mut d, Item::Contact(Contact { id: 0, a: Target::Face { face: a }, b: Target::Face { face: b }, friction: 0.0 }));
        let r = crate::solids::tet::solve(&d.m).expect("blocks in contact");
        let name = "A block pressed onto another, frictionless contact, meshes not matching (4 × 4 on 3 × 3), 10-node tetrahedra";
        let c = &r.contact[0];
        let src = "Uniaxial stress through the contact (exact; the penalty lets the faces overlap by about a thousandth of the movement)";
        v.push(check(36, name, "Force through the contact pA", "force", p, c.normal, 1e-6, src));
        let mid = |z: f64| { let i = r.volume.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, [0.5, 0.5, z]).total_cmp(&crate::model::dist3(*b.1, [0.5, 0.5, z]))).unwrap().0; r.volume.v[r.fields.iter().position(|f| f.key == "sz").unwrap()][i] };
        v.push(check(36, name, "Stress in the lower block, mid-height", "stress", -p, mid(0.5), 0.02, src));
        v.push(check(36, name, "Stress in the upper block, mid-height", "stress", -p, mid(1.5), 0.02, src));
        v.push(check(36, name, "Top moves down 2pL/E", "len", -2.0 * p / e, r.field("uz").min, 0.01, src));
        v.push(check(36, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics"));
    }
    // The upper block pressed down by 1 MPa and its top face moved 1 mm sideways: it slides, and
    // the friction is μ times the force pressing, μ = 0.3.
    {
        let (p, mu) = (1e6, 0.3);
        let lower = grid([3, 3, 2], 2, |q| [q[0], q[1], 0.5 * q[2]]).unwrap();
        let upper = grid([3, 3, 2], 2, |q| [q[0], q[1], 0.5 + 0.5 * q[2]]).unwrap();
        let nf = lower.faces.len();
        let (ms, fa) = join(lower, upper);
        let nall = ms.faces.len();
        let mut d = solid(ms);
        let m = d.m.volume.as_ref().unwrap().clone();
        sup(&mut d, face_of(&m, 0, nf, &|q| q[2].abs() < 1e-9), SupportKind::Fixed);
        sup(&mut d, face_of(&m, fa, nall, &|q| q[1].abs() < 1e-9), SupportKind::Symmetry);
        let top = face_of(&m, fa, nall, &|q| (q[2] - 1.0).abs() < 1e-9);
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Face { face: top }, kind: LoadKind::Pressure { p } }));
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Face { face: top }, kind: LoadKind::Settlement { dx: 1e-3, dy: 0.0, dz: 0.0 } }));
        let a = face_of(&m, fa, nall, &|q| (q[2] - 0.5).abs() < 1e-9);
        let b = face_of(&m, 0, nf, &|q| (q[2] - 0.5).abs() < 1e-9);
        add_id(&mut d, Item::Contact(Contact { id: 0, a: Target::Face { face: a }, b: Target::Face { face: b }, friction: mu }));
        let r = crate::solids::tet::solve(&d.m).expect("block dragged");
        let c = &r.contact[0];
        let name = "A block pressed down and dragged over another, Coulomb friction μ = 0.3, 10-node tetrahedra";
        let src = "Coulomb friction: sliding, the friction is μ N (exact; the penalty and the rounds that settle the sliding stop within 0.2 %)";
        v.push(check(36, name, "Force pressing N = pA", "force", p, c.normal, 2e-3, src));
        v.push(check(36, name, "Friction μN while it slides", "force", mu * p, -c.force[0], 2e-3, src));
        v.push(check(36, name, "Share of the touching nodes sliding", "ratio", 1.0, c.sliding as f64 / c.touching.max(1) as f64, 1e-12, src));
    }
    // The same block pushed sideways by a force a third of μN on its side: it holds, and the
    // friction is the push (an edge the push's moment unloads may slip a little).
    {
        let (p, mu, t) = (1e6, 0.3, 1e5);
        let lower = grid([3, 3, 2], 2, |q| [q[0], q[1], 0.5 * q[2]]).unwrap();
        let upper = grid([3, 3, 2], 2, |q| [q[0], q[1], 0.5 + 0.5 * q[2]]).unwrap();
        let nf = lower.faces.len();
        let (ms, fa) = join(lower, upper);
        let nall = ms.faces.len();
        let mut d = solid(ms);
        let m = d.m.volume.as_ref().unwrap().clone();
        sup(&mut d, face_of(&m, 0, nf, &|q| q[2].abs() < 1e-9), SupportKind::Fixed);
        sup(&mut d, face_of(&m, fa, nall, &|q| q[1].abs() < 1e-9), SupportKind::Symmetry);
        let top = face_of(&m, fa, nall, &|q| (q[2] - 1.0).abs() < 1e-9);
        let side = face_of(&m, fa, nall, &|q| q[0].abs() < 1e-9);
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Face { face: top }, kind: LoadKind::Pressure { p } }));
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Face { face: side }, kind: LoadKind::Traction { fx: t, fy: 0.0, fz: 0.0, total: true } }));
        let a = face_of(&m, fa, nall, &|q| (q[2] - 0.5).abs() < 1e-9);
        let b = face_of(&m, 0, nf, &|q| (q[2] - 0.5).abs() < 1e-9);
        add_id(&mut d, Item::Contact(Contact { id: 0, a: Target::Face { face: a }, b: Target::Face { face: b }, friction: mu }));
        let r = crate::solids::tet::solve(&d.m).expect("block pushed");
        let c = &r.contact[0];
        let name = "The block pushed sideways by a third of μN, Coulomb friction μ = 0.3, 10-node tetrahedra";
        let src = "Coulomb friction: holding, the friction is the push (statics, exact; the rounds that settle the edge's slip stop within 0.01 %)";
        v.push(check(36, name, "Friction equals the push", "force", t, -c.force[0], 1e-4, src));
        v.push(check(36, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-4, "Statics"));
    }
    v
}

/// Benchmark 37 (0.9.0): explicit dynamics of meshed solids, 10-node tetrahedra, central
/// differences with lumped masses.
fn explicit() -> Vec<Check> {
    use crate::solids::tet::{grid, solve_explicit, Ex};
    let mut v = vec![];
    let (e, rho) = (200e9, 7850.0);
    let solid = |nu: f64| {
        let mut d = Doc::new(Model::new(Analysis::Solid));
        d.m.materials[0].e = e;
        d.m.materials[0].nu = nu;
        d.m.materials[0].rho = rho;
        d
    };
    // A bar 1 m long fixed at one end, a pressure p put on at once on the other (ν = 0, so the
    // bar is exactly one-dimensional): a wave runs along at c = √(E/ρ), reflects, and the end
    // moves out to twice the static movement, 2pL/E, at t = 2L/c, then back.
    {
        let (l, h, p) = (1.0, 0.05, 50e6);
        let mut d = solid(0.0);
        d.m.volume = Some(Box::new(grid([40, 1, 1], 2, |q| [l * q[0], h * q[1], h * q[2]]).unwrap()));
        let ms = d.m.volume.as_ref().unwrap().clone();
        let fixed = Target::Face { face: ms.face_where(|q| q[0].abs() < 1e-9).expect("root") };
        let end = Target::Face { face: ms.face_where(|q| (q[0] - l).abs() < 1e-9).expect("end") };
        add_id(&mut d, Item::Support(Support { id: 0, at: fixed, kind: SupportKind::Fixed }));
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: end, kind: LoadKind::Pressure { p: -p } }));
        let c = (e / rho).sqrt();
        let r = solve_explicit(&d.m, &Ex { duration: 3.0 * l / c, dt: 0.0, curve: vec![] }).expect("bar wave");
        let hs = r.history.as_ref().unwrap();
        let name = "A bar hit by a sudden end load: the stress wave, ν = 0, 10-node tetrahedra";
        let src = "One-dimensional wave in a bar (exact: the end moves 2pL/E at t = 2L/c; Graff, Wave Motion in Elastic Solids, ch. 2)";
        v.push(check(37, name, "Largest end movement 2pL/E (twice the static)", "len", 2.0 * p * l / e, r.field("ux").max, 0.02, src));
        v.push(check(37, name, "When it moves most, 2L/c", "ratio", 2.0 * l / c, hs.peak, 0.03, src));
        let k = hs.t.len() - 1;
        v.push(check(37, name, "Energy kept: kinetic + strain over the work done, at the end", "ratio", 1.0, (hs.kinetic[k] + hs.strain[k]) / hs.work[k].max(1e-300), 0.01, "Central differences conserve energy to the order of the time step"));
    }
    // A block flying at 10 m/s, held by nothing: it moves 10 t, as one piece.
    {
        let mut d = solid(0.3);
        d.m.volume = Some(Box::new(grid([2, 2, 2], 2, |q| [0.1 * q[0], 0.1 * q[1], 0.1 * q[2]]).unwrap()));
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Velocity { v: [10.0, 0.0, 0.0] } }));
        let t = 1e-3;
        let r = solve_explicit(&d.m, &Ex { duration: t, dt: 0.0, curve: vec![] }).expect("flying block");
        let name = "A block flying at 10 m/s with nothing on it, 10-node tetrahedra";
        v.push(check(37, name, "How far it moves in 1 ms, vt", "len", 10.0 * t, r.field("ux").max, 1e-9, "Newton's first law (exact)"));
        v.push(check(37, name, "Still one piece: the most any point moves over the least", "ratio", 1.0, r.field("ux").max / r.field("ux").min, 1e-9, "Rigid motion (exact)"));
    }
    v
}

/// Benchmark 29 (0.5.6): natural frequencies and heat conduction on tetrahedra.
fn tetra_more() -> Vec<Check> {
    use crate::solids::tet::{grid, TetMesh};
    let mut v = vec![];
    let doc = |ms: TetMesh, a: Analysis| { let mut d = Doc::new(Model::new(a)); d.m.volume = Some(Box::new(ms)); d };
    let on = |d: &Doc, c: usize, val: f64| Target::Face { face: d.m.volume.as_ref().unwrap().face_where(|q| (q[c] - val).abs() < 1e-9).expect("face") };
    let sup = |d: &mut Doc, at: Target, kind: SupportKind| { add_id(d, Item::Support(Support { id: 0, at, kind })); };
    let lowest = |m: &Model, n: usize| -> Vec<f64> { match crate::modes(m, n).expect("modes") { crate::Modes::Solid(x) => x.modes.iter().map(|q| q.f).collect(), _ => vec![] } };
    let (e, rho) = (200e9, 7850.0);
    // A bar 1 m long held on three faces by rollers, so it can only stretch: its first mode is the
    // quarter wave, f = √(E/ρ) / 4L (lateral inertia is negligible for so slender a bar).
    {
        let mut d = doc(grid([20, 2, 2], 2, |q| [q[0], 0.05 * q[1], 0.05 * q[2]]).unwrap(), Analysis::Solid);
        d.m.materials[0].e = e;
        d.m.materials[0].rho = rho;
        d.m.materials[0].nu = 0.0;
        for (c, val) in [(0, 0.0), (1, 0.0), (2, 0.0)] {
            let t = on(&d, c, val);
            sup(&mut d, t, SupportKind::Symmetry);
        }
        let f = lowest(&d.m, 1);
        v.push(check(29, "Bar stretching and shortening, 10-node tetrahedra", "First natural frequency √(E/ρ)/4L", "hz", (e / rho).sqrt() / 4.0, f[0], 1e-3, "One-dimensional wave equation (ν = 0, so no lateral inertia)"));
    }
    // A cantilever 1 m long, 50 × 50 mm, fixed at its root: first bending frequency
    // (1.8751)²/2π √(EI/ρAL⁴), Euler–Bernoulli (L/h = 20: shear and rotary inertia lower it a little).
    {
        let h = 0.05f64;
        let mut d = doc(grid([24, 2, 2], 2, |q| [q[0], h * q[1], h * q[2]]).unwrap(), Analysis::Solid);
        d.m.materials[0].e = e;
        d.m.materials[0].rho = rho;
        d.m.materials[0].nu = 0.3;
        let root = on(&d, 0, 0.0);
        sup(&mut d, root, SupportKind::Fixed);
        let f = lowest(&d.m, 2);
        let want = 1.875_104_068_7f64.powi(2) / (2.0 * std::f64::consts::PI) * (e * h.powi(4) / 12.0 / (rho * h * h)).sqrt();
        v.push(check(29, "Cantilever vibrating, 10-node tetrahedra", "First bending frequency 1.8751²/2π √(EI/ρAL⁴)", "hz", want, f[0], 0.01, "Euler–Bernoulli beam (Timoshenko effects about 0.3 % at L/h = 20)"));
        v.push(check(29, "Cantilever vibrating, 10-node tetrahedra", "Second mode: the same bending the other way (a square section)", "hz", f[0], f[1], 1e-3, "Symmetry of the square section"));
    }
    // A plate 1 m thick (along x), 50 °C on one face, 20 °C on the other, with heat made inside at
    // 10 kW/m³: T(x) = T₀ + (T₁ − T₀) x/L + q x (L − x) / 2k, exact for 10-node elements.
    {
        let mut d = doc(grid([4, 2, 2], 2, |q| [q[0], 0.2 * q[1], 0.2 * q[2]]).unwrap(), Analysis::SolidHeat);
        let k = 50.0;
        d.m.materials[0].k = k;
        let (t0, t1, q) = (323.15, 293.15, 1e4);
        let (a, b) = (on(&d, 0, 0.0), on(&d, 0, 1.0));
        sup(&mut d, a, SupportKind::Temperature { t: t0 });
        sup(&mut d, b, SupportKind::Temperature { t: t1 });
        add_id(&mut d, Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Heat { q } }));
        let Ok(crate::Results::SolidHeat(r)) = crate::solve(&d.m) else { panic!("slab heat") };
        let mid = r.volume.nodes.iter().position(|p| (p[0] - 0.5).abs() < 1e-9 && (p[1] - 0.1).abs() < 1e-9 && (p[2] - 0.1).abs() < 1e-9).expect("middle node");
        let want = t0 + (t1 - t0) * 0.5 + q * 0.25 / (2.0 * k);
        let name = "Plate with heat made inside, 10-node tetrahedra";
        v.push(check(29, name, "Temperature halfway through", "temp", want, r.volume.v[0][mid], 1e-9, "Steady conduction with uniform generation (exact: a parabola)"));
        v.push(check(29, name, "Heat balance (heat out over heat made and in)", "ratio", 1.0, 1.0 + r.imbalance, 1e-9, "Conservation of energy"));
    }
    // A thick cylinder wall (a = 0.1, b = 0.2 m), 100 °C inside, 20 °C outside, a quarter, ends
    // insulated: T(r) = Tᵢ + (Tₒ − Tᵢ) ln(r/a)/ln(b/a); heat through it 2πkh(Tᵢ − Tₒ)/ln(b/a).
    {
        let (a, b, hh, k) = (0.1f64, 0.2f64, 0.05, 45.0);
        let ms = grid([6, 8, 1], 2, |q| { let (r, t) = (a + (b - a) * q[0], q[1] * std::f64::consts::FRAC_PI_2); [r * t.cos(), r * t.sin(), hh * q[2]] }).unwrap();
        let mut d = doc(ms, Analysis::SolidHeat);
        d.m.materials[0].k = k;
        let fin = |d: &Doc, r: f64| Target::Face { face: d.m.volume.as_ref().unwrap().face_where(|p| ((p[0] * p[0] + p[1] * p[1]).sqrt() - r).abs() < 1e-9).expect("face") };
        let (inner, outer) = (fin(&d, a), fin(&d, b));
        sup(&mut d, inner, SupportKind::Temperature { t: 373.15 });
        sup(&mut d, outer, SupportKind::Temperature { t: 293.15 });
        let Ok(crate::Results::SolidHeat(r)) = crate::solve(&d.m) else { panic!("wall heat") };
        let rm = 0.15;
        let node = r.volume.nodes.iter().position(|p| ((p[0] * p[0] + p[1] * p[1]).sqrt() - rm).abs() < 1e-9 && p[1].abs() < 1e-9 && p[2].abs() < 1e-9).expect("node at r = 0.15");
        let want = 373.15 + (293.15 - 373.15) * (rm / a).ln() / (b / a).ln();
        let flow = std::f64::consts::FRAC_PI_2 * k * hh * 80.0 / (b / a).ln();
        let name = "Thick cylinder wall conducting heat, 10-node tetrahedra (a quarter)";
        v.push(check(29, name, "Temperature at r = 0.15 m", "temp", want, r.volume.v[0][node], 1e-3, "Radial conduction: T(r) logarithmic"));
        v.push(check(29, name, "Heat through the wall πkh(Tᵢ − Tₒ)/2 ln(b/a)", "power", flow, r.supports[0].q.abs(), 5e-3, "Radial conduction (a quarter of 2πkhΔT/ln(b/a))"));
    }
    v
}

/// Benchmark 30 (0.5.7): plasticity and large deformation on 10-node tetrahedra (the swept
/// solids' solvers, fed the tetrahedra's points), against the answers of benchmarks 26 and 27.
fn tetra_nonlinear() -> Vec<Check> {
    use crate::model::{Step, StepKind};
    use crate::solids::hyper::NeoHooke;
    use crate::solids::tet::{grid, TetMesh};
    let mut v = vec![];
    let doc = |ms: TetMesh, e: f64, nu: f64| { let mut d = Doc::new(Model::new(Analysis::Solid)); d.m.materials[0].e = e; d.m.materials[0].nu = nu; d.m.materials[0].rho = 0.0; d.m.volume = Some(Box::new(ms)); d };
    let on = |d: &Doc, c: usize, val: f64| Target::Face { face: d.m.volume.as_ref().unwrap().face_where(|q| (q[c] - val).abs() < 1e-9).expect("face") };
    let sup = |d: &mut Doc, at: Target, kind: SupportKind| { add_id(d, Item::Support(Support { id: 0, at, kind })); };
    let load = |d: &mut Doc, at: Target, kind: LoadKind| { add_id(d, Item::Load(Load { case: 0, id: 0, at, kind })); };
    let step = |d: &mut Doc, kind: StepKind, n: u32| { add(d, Op::Add { item: Item::Step(Step { kind, increments: n, ..Default::default() }) }); };
    let solid = |d: &Doc| { let Ok(crate::Results::Solid(r)) = crate::solve_for(&d.m, None) else { panic!("{:?}", crate::solve_for(&d.m, None).err()) }; r };
    let fy = 250e6;
    // The thick cylinder in plane strain (a quarter, b/a = 2), perfectly plastic: collapse at 2k ln 2.
    {
        let (a, b, hh) = (0.1f64, 0.2f64, 0.02);
        let ms = grid([6, 8, 1], 2, |q| { let (r, t) = (a + (b - a) * q[0], q[1] * std::f64::consts::FRAC_PI_2); [r * t.cos(), r * t.sin(), hh * q[2]] }).unwrap();
        let mut d = doc(ms, 200e9, 0.3);
        d.m.materials[0].fy = fy;
        for (c, val) in [(1, 0.0), (0, 0.0), (2, 0.0), (2, hh)] {
            let t = on(&d, c, val);
            sup(&mut d, t, SupportKind::Symmetry);
        }
        let bore = Target::Face { face: d.m.volume.as_ref().unwrap().face_where(|p| ((p[0] * p[0] + p[1] * p[1]).sqrt() - a).abs() < 1e-9).expect("bore") };
        let pc = 2.0 * fy / 3f64.sqrt() * 2f64.ln();
        load(&mut d, bore, LoadKind::Pressure { p: 1.1 * pc });
        step(&mut d, StepKind::Newton, 12);
        let r = solid(&d);
        let got = r.path.as_ref().map_or(0.0, |p| p.load.last().copied().unwrap_or(0.0)) * 1.1 * pc;
        v.push(check(30, "Thick cylinder, 10-node tetrahedra, perfectly plastic (b/a = 2)", "Collapse pressure 2k ln(b/a), k = σy/√3", "stress", pc, got, 0.01, "Hill 1950 (limit analysis); the largest load carried, reached from below"));
    }
    // A bar pulled to 1.5 σy with hardening H = 2 GPa: exact.
    {
        let mut d = doc(grid([8, 2, 2], 2, |q| [q[0], 0.1 * q[1], 0.1 * q[2]]).unwrap(), 200e9, 0.3);
        let h = 2e9;
        d.m.materials[0].fy = fy;
        d.m.materials[0].hardening = h;
        for (c, val) in [(0, 0.0), (1, 0.0), (2, 0.0)] {
            let t = on(&d, c, val);
            sup(&mut d, t, SupportKind::Symmetry);
        }
        let end = on(&d, 0, 1.0);
        load(&mut d, end, LoadKind::Traction { fx: 1.5 * fy, fy: 0.0, fz: 0.0, total: false });
        step(&mut d, StepKind::Newton, 6);
        let r = solid(&d);
        let name = "Bar pulled past yield, 10-node tetrahedra, hardening H = 2 GPa";
        v.push(check(30, name, "Stretch L(σ/E + (σ − σy)/H)", "len", 1.5 * fy / 200e9 + 0.5 * fy / h, r.field("ux").max, 1e-6, "Uniaxial stress with linear hardening (exact)"));
        v.push(check(30, name, "Equivalent plastic strain (σ − σy)/H", "ratio", 0.5 * fy / h, r.field("peeq").max, 1e-6, "Uniaxial stress with linear hardening (exact)"));
    }
    // A rubber bar stretched to twice its length: the neo-Hookean answer, exact.
    {
        let (e, nu) = (2.98e6, 0.49);
        let (p, side) = NeoHooke::new(e, nu).uniaxial(2.0);
        let mut d = doc(grid([10, 1, 1], 2, |q| [0.2 * q[0], 0.02 * q[1], 0.02 * q[2]]).unwrap(), e, nu);
        for (c, val) in [(0, 0.0), (1, 0.0), (2, 0.0)] {
            let t = on(&d, c, val);
            sup(&mut d, t, SupportKind::Symmetry);
        }
        let end = on(&d, 0, 0.2);
        load(&mut d, end, LoadKind::Traction { fx: p * 0.02 * 0.02, fy: 0.0, fz: 0.0, total: true });
        step(&mut d, StepKind::Large, 10);
        let r = solid(&d);
        let name = "Rubber bar stretched to twice its length, 10-node tetrahedra";
        let src = "Neo-Hookean uniaxial tension, exact";
        v.push(check(30, name, "End movement (λ − 1) L", "len", 0.2, r.field("ux").max, 1e-6, src));
        v.push(check(30, name, "True stress along it", "stress", p / (side * side), r.field("sx").max, 1e-6, src));
    }
    // A steel cantilever bent far by a dead end load (PL²/EI = 2): the elastica.
    {
        let (e, l, h) = (200e9, 1.0, 0.05f64);
        let ei = e * h * h.powi(3) / 12.0;
        let mut d = doc(grid([40, 2, 2], 2, |q| [l * q[0], h * q[1], h * q[2]]).unwrap(), e, 0.3);
        let root = on(&d, 0, 0.0);
        sup(&mut d, root, SupportKind::Fixed);
        let tip = on(&d, 0, l);
        load(&mut d, tip, LoadKind::Traction { fx: 0.0, fy: -2.0 * ei / (l * l), fz: 0.0, total: true });
        step(&mut d, StepKind::Large, 10);
        let r = solid(&d);
        let (dv, _, _) = elastica_tip(2.0);
        let i = r.volume.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, [l, h / 2.0, h / 2.0]).total_cmp(&crate::model::dist3(*b.1, [l, h / 2.0, h / 2.0]))).unwrap().0;
        let name = "Steel cantilever bent far by a dead end load, 10-node tetrahedra";
        v.push(check(30, name, "Tip movement down", "len", -dv * l, r.volume.disp[i][1], 0.01, "The elastica (Mattiasson 1981: 0.49346 L)"));
        v.push(check(30, name, "Equilibrium (moments about the bent shape)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics on the deformed solid"));
    }
    v
}

/// Benchmark 31 (0.5.8): large rotations in 3D frames, by the corotational 3D beam, against
/// exact answers: cantilevers rolled up by end moments about either axis, a skew cantilever bent
/// far by an end load (the elastica in a plane at an angle to every axis), and a large twist.
fn space_large() -> Vec<Check> {
    use crate::model::{Step, StepKind};
    use std::f64::consts::PI;
    let mut v = vec![];
    let e = 200e9;
    let solve = |d: &Doc| -> crate::space::SpaceResults { let Ok(crate::Results::Space(r)) = crate::solve_for(&d.m, None) else { panic!("{:?}", crate::solve_for(&d.m, None).err()) }; r };
    let step = |d: &mut Doc, n: u32, pieces: u32| { add(d, Op::Add { item: Item::Step(Step { kind: StepKind::Newton, increments: n, pieces, ..Default::default() }) }); };
    let tip_of = |d: &Doc, p: [f64; 3]| d.m.nodes.iter().position(|n| crate::model::dist3(n.at(), p) < 1e-9).expect("tip");
    // A cantilever 1 m along x, rolled a quarter turn by an end moment: about z (bending in x-y)
    // with M = (π/2) EIz/L, and about y (bending in x-z) with M = (π/2) EIy/L. The tip moves to
    // (2L/π, ±2L/π) and turns π/2 (Euler's elastica: κ = M/EI).
    let (b, h, l) = (0.05, 0.1, 1.0);
    let shape = SectionShape::Rect { b, h };
    let (eiz, eiy) = (e * b * h.powi(3) / 12.0, e * h * b.powi(3) / 12.0);
    for (axis, name) in [(2usize, "Cantilever rolled a quarter turn about z, 3D frame"), (1, "Cantilever rolled a quarter turn about y, 3D frame")] {
        let mut d = space_frame(e, shape.clone());
        member3(&mut d, [0.0; 3], [l, 0.0, 0.0], MemberKind::Beam);
        support3(&mut d, [0.0; 3], SupportKind::Fixed);
        let tip = node3(&d, [l, 0.0, 0.0]);
        let m = PI / 2.0 * if axis == 2 { eiz } else { eiy } / l;
        let kind = if axis == 2 { LoadKind::Moment { m, mx: 0.0, my: 0.0 } } else { LoadKind::Moment { m: 0.0, mx: 0.0, my: m } };
        add(&mut d, Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Node { id: tip }, kind }) });
        step(&mut d, 6, 20);
        let r = solve(&d);
        let i = tip_of(&d, [l, 0.0, 0.0]);
        let src = "NAFEMS NL5 in 3D; Euler's elastica, κ = M/EI";
        v.push(check(31, name, "Tip moves in (2/π − 1) L", "len", (2.0 / PI - 1.0) * l, r.disp[i][0], 2e-3, src));
        let across = if axis == 2 { r.disp[i][1] } else { -r.disp[i][2] };
        v.push(check(31, name, "Tip moves across 2L/π", "len", 2.0 * l / PI, across, 2e-3, src));
        let turn = if axis == 2 { r.disp[i][5] } else { r.disp[i][4] };
        v.push(check(31, name, "Tip turns π/2", "rad", PI / 2.0, turn, 2e-3, src));
    }
    // A square cantilever 1 m long along the diagonal (1, 1, 1)/√3, a dead end load across it
    // (along (1, −1, 0)/√2) with PL²/EI = 2: the elastica, in a plane at an angle to every axis.
    {
        let a = 0.05;
        let mut d = space_frame(e, SectionShape::Rect { b: a, h: a });
        let ei = e * a.powi(4) / 12.0;
        let dir = [1.0 / 3f64.sqrt(); 3];
        let across = [1.0 / 2f64.sqrt(), -1.0 / 2f64.sqrt(), 0.0];
        let end = dir.map(|c| c * l);
        member3(&mut d, [0.0; 3], end, MemberKind::Beam);
        support3(&mut d, [0.0; 3], SupportKind::Fixed);
        let tip = node3(&d, end);
        let p = 2.0 * ei / (l * l);
        add(&mut d, Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Node { id: tip }, kind: LoadKind::Force { fx: p * across[0], fy: p * across[1], fz: p * across[2] } }) });
        step(&mut d, 10, 20);
        let r = solve(&d);
        let i = tip_of(&d, end);
        let u = [r.disp[i][0], r.disp[i][1], r.disp[i][2]];
        let (dv, du, _) = elastica_tip(2.0);
        let name = "Skew cantilever bent far by an end load across it, 3D frame (PL²/EI = 2)";
        let src = "The elastica (Mattiasson 1981: 0.49346 L across, 0.16064 L in)";
        v.push(check(31, name, "Tip moves across, along the load", "len", dv * l, u.iter().zip(&across).map(|(a, b)| a * b).sum::<f64>(), 5e-3, src));
        v.push(check(31, name, "Tip moves in towards the root", "len", -du * l, u.iter().zip(&dir).map(|(a, b)| a * b).sum::<f64>(), 5e-3, src));
        v.push(check(31, name, "Equilibrium (moments about the bent shape)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Statics on the deflected frame"));
    }
    // A round bar twisted through 2 rad by an end torque: θ = TL/GJ, exactly (St Venant torsion
    // of a circle stays linear however far it turns).
    {
        let dia = 0.05;
        let mut d = space_frame(e, SectionShape::Circle { d: dia });
        member3(&mut d, [0.0; 3], [l, 0.0, 0.0], MemberKind::Beam);
        support3(&mut d, [0.0; 3], SupportKind::Fixed);
        let tip = node3(&d, [l, 0.0, 0.0]);
        let gj = e / 2.6 * PI * dia.powi(4) / 32.0;
        add(&mut d, Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Node { id: tip }, kind: LoadKind::Moment { m: 0.0, mx: 2.0 * gj / l, my: 0.0 } }) });
        step(&mut d, 4, 4);
        let r = solve(&d);
        let i = tip_of(&d, [l, 0.0, 0.0]);
        v.push(check(31, "Round bar twisted far by an end torque, 3D frame", "Twist TL/GJ = 2 rad", "rad", 2.0, r.disp[i][3], 1e-6, "St Venant torsion of a circular bar (exact)"));
    }
    // The 45° bend (Bathe and Bolourchi 1979): a curved cantilever pushed out of its plane, bending
    // and twisting far. The tip's place at the full load, as published (Simo and Vu-Quoc 1986:
    // 47.23, 15.79, 53.37 in, from the root, radius 100 in); published solutions differ among
    // themselves by about 1 % (the bar's torsion constant), hence the tolerance.
    {
        let m = crate::examples::get("bend45").expect("bend45");
        let Ok(crate::Results::Space(r)) = crate::solve_for(&m, None) else { panic!("bend45") };
        let i = m.nodes.iter().position(|n| n.x > 1.7).expect("tip");
        let p = m.nodes[i].at();
        let name = "45° bend pushed out of its plane, 3D frame (large rotations)";
        let src = "Bathe and Bolourchi 1979; Simo and Vu-Quoc 1986";
        let inch = 0.0254;
        v.push(check(31, name, "Tip along the first tangent", "len", 47.23 * inch, p[0] + r.disp[i][0], 2e-2, src));
        v.push(check(31, name, "Tip across, in the bend's plane", "len", 15.79 * inch, p[2] + r.disp[i][2], 2e-2, src));
        v.push(check(31, name, "Tip down, out of the plane", "len", 53.37 * inch, -r.disp[i][1], 2e-2, src));
    }
    v
}

/// Benchmark 32 (0.6.0): assemblies of parts. Parts sketched, extruded or revolved, turned and
/// placed, meshed on their own and tied where they touch: a cantilever as one part and cut into
/// two, a bar of two materials, a revolved thick cylinder, and heat through two parts.
/// Benchmark 33 (0.6.3): basic modelling. Parts reversed, cut, drilled, rounded, turned,
/// mirrored and copied in a ring; their exact volumes (area × depth, Pappus) and their meshes'
/// volumes against hand formulas, and a mirror copy against the mirror image.
fn modelling() -> Vec<Check> {
    use crate::model::Part;
    use crate::solids::modelling::{volume, Placing};
    use std::f64::consts::PI;
    let mut v = vec![];
    let new = |size: f64| { let mut d = Doc::new(Model::new(Analysis::Solid)); d.m.mesh.size = size; d.m.mesh.order = 1; d };
    let part = |d: &mut Doc, p: Part, gs: &[Geom]| -> u32 {
        let id = add_id(d, Item::Part(p));
        for g in gs {
            add_id(d, Item::Shape(Shape { geom: g.clone(), material: d.m.materials[0].id, part: id, ..Default::default() }));
        }
        id
    };
    let meshed = |m: &Model| -> f64 {
        let a = crate::solids::assembly::mesh(m).expect("meshes");
        a.mesh.tets.iter().map(|el| {
            let p = |i: usize| a.mesh.nodes[el[i] as usize];
            let s = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
            let (u, w, z) = (s(p(1), p(0)), s(p(2), p(0)), s(p(3), p(0)));
            (u[0] * (w[1] * z[2] - w[2] * z[1]) - u[1] * (w[0] * z[2] - w[2] * z[0]) + u[2] * (w[0] * z[1] - w[1] * z[0])) / 6.0
        }).sum()
    };
    let src = "Volume of a prism: area × depth";
    // A block 0.2 × 0.1 extruded 0.3 back from its sketch (reversed).
    {
        let mut d = new(0.05);
        part(&mut d, Part { depth: 0.3, from: -0.3, ..Default::default() }, &[Geom::rect(0.0, 0.0, 0.2, 0.1)]);
        v.push(check(33, "Block 0.2 × 0.1 m extruded 0.3 m the other way", "Meshed volume abh", "ratio", 0.006, meshed(&d.m), 1e-12, src));
    }
    // A plate 0.2 × 0.1 × 0.02: a disc r = 20 mm sketched on its front face cut through it, a
    // hole 20 mm across drilled at a point on its back face, one edge along its depth rounded
    // to r = 10 mm.
    {
        let mut d = new(0.005);
        let plate = part(&mut d, Part { depth: 0.02, ..Default::default() }, &[Geom::rect(0.0, 0.0, 0.2, 0.1)]);
        let tool = part(&mut d, Part { depth: 0.01, at: [0.0, 0.0, 0.02], ..Default::default() }, &[Geom::Circle { c: [0.05, 0.05], r: 0.02 }]);
        add(&mut d, Op::Cut { tool, from: plate });
        let cut = (0.02 - PI * 4e-4) * 0.02;
        v.push(check(33, "Plate 0.2 × 0.1 × 0.02 m, a disc of r = 20 mm cut through", "Volume (ab − πr²)t", "ratio", cut, volume(&d.m, plate).unwrap_or(0.0), 1e-12, src));
        add(&mut d, Op::Hole { part: plate, at: [0.15, 0.05, 0.0], d: 0.02 });
        let drilled = cut - PI * 1e-4 * 0.02;
        v.push(check(33, "… and a hole 20 mm across drilled through", "Volume (ab − πr₁² − πr₂²)t", "ratio", drilled, volume(&d.m, plate).unwrap_or(0.0), 1e-12, src));
        add(&mut d, Op::PartEdge { part: plate, near: [0.2, 0.1, 0.01], size: 0.01, round: true });
        let rounded = drilled - 1e-4 * (1.0 - PI / 4.0) * 0.02;
        v.push(check(33, "… and one edge rounded to r = 10 mm", "Volume, less r²(1 − π/4)t at the edge", "ratio", rounded, volume(&d.m, plate).unwrap_or(0.0), 1e-12, src));
        let tool = part(&mut d, Part { depth: 0.01, at: [0.0, 0.0, 0.02], ..Default::default() }, &[Geom::Circle { c: [0.1, 0.0], r: 0.01 }]);
        add(&mut d, Op::Cut { tool, from: plate });
        let notched = rounded - PI * 1e-4 / 2.0 * 0.02;
        v.push(check(33, "… and a disc of r = 10 mm across its edge cut as a notch", "Volume, less half the disc × t", "ratio", notched, volume(&d.m, plate).unwrap_or(0.0), 1e-12, src));
        v.push(check(33, "… meshed (circles as straight pieces)", "Meshed volume against the exact one", "ratio", notched, meshed(&d.m), 0.01, "The exact volume above"));
    }
    // Turned parts: a ring of rectangular section and a torus of round section (Pappus).
    {
        let mut d = new(0.01);
        let ring = part(&mut d, Part { angle: 2.0 * PI, ..Default::default() }, &[Geom::rect(0.1, 0.0, 0.2, 0.05)]);
        v.push(check(33, "Ring, rectangle 0.1 ≤ r ≤ 0.2, 0.05 high, turned a full turn", "Volume π(r₂² − r₁²)h", "ratio", PI * 0.03 * 0.05, volume(&d.m, ring).unwrap_or(0.0), 1e-12, "Pappus' theorem"));
        let mut d = new(0.01);
        let torus = part(&mut d, Part { angle: 2.0 * PI, ..Default::default() }, &[Geom::Circle { c: [0.2, 0.0], r: 0.05 }]);
        let want = 2.0 * PI * PI * 0.2 * 0.05 * 0.05;
        v.push(check(33, "Torus, a circle r = 0.05 m at R = 0.2 m turned a full turn", "Volume 2π²Rr²", "ratio", want, volume(&d.m, torus).unwrap_or(0.0), 1e-12, "Pappus' theorem"));
        v.push(check(33, "Torus, meshed", "Meshed volume against 2π²Rr²", "ratio", want, meshed(&d.m), 0.02, "Pappus' theorem (circle and turn as straight pieces)"));
    }
    // A block turned and moved, mirrored in the plane x = 0; copies in a ring of four.
    {
        let mut d = new(0.05);
        let p = part(&mut d, Part { depth: 0.1, at: [0.1, 0.2, 0.3], turn: [0.3, 0.2, -0.4], ..Default::default() }, &[Geom::rect(0.5, 0.0, 0.7, 0.1)]);
        add(&mut d, Op::PlaceParts { ids: vec![p], how: Placing::Mirror { point: [0.0; 3], normal: [1.0, 0.0, 0.0] }, copies: 1 });
        let a = crate::solids::assembly::mesh(&d.m).expect("meshes");
        let half = a.mesh.nodes.len() / 2;
        let worst = a.mesh.nodes[..half].iter().map(|n| a.mesh.nodes[half..].iter().map(|m| crate::model::dist3(*m, [-n[0], n[1], n[2]])).fold(f64::MAX, f64::min)).fold(0.0, f64::max);
        v.push(check(33, "Block turned, moved and mirrored in x = 0", "Mirror copy's nodes on the mirror image (1 + farthest miss, m)", "ratio", 1.0, 1.0 + worst, 1e-12, "Geometry: a mirror image"));
        let mut d = new(0.05);
        let p = part(&mut d, Part { depth: 0.1, ..Default::default() }, &[Geom::rect(0.5, 0.0, 0.7, 0.1)]);
        add(&mut d, Op::PlaceParts { ids: vec![p], how: Placing::Turn { about: [0.0; 3], axis: [0.0, 1.0, 0.0], angle: PI / 2.0 }, copies: 3 });
        v.push(check(33, "Block copied three times a quarter turn apart", "Meshed volume of the four, 4abh", "ratio", 4.0 * 0.002, meshed(&d.m), 1e-12, src));
    }
    v
}

fn assembly() -> Vec<Check> {
    use crate::model::Part;
    use std::f64::consts::FRAC_PI_2;
    let mut v = vec![];
    // A model of parts, each a rectangle sketch [x0, y0, x1, y1] with its depth or angle, place,
    // turn and material (index into the model's materials); returns the shape of each part.
    let build = |an: Analysis, size: f64, list: &[([f64; 4], f64, f64, [f64; 3], [f64; 3], usize)], mats: &[(f64, f64, f64)]| -> (Doc, Vec<(u32, u32)>) {
        let mut d = Doc::new(Model::new(an));
        d.m.mesh.size = size;
        let mut ids = vec![];
        for (k, &(e, nu, kc)) in mats.iter().enumerate() {
            if k == 0 {
                d.m.materials[0].e = e;
                d.m.materials[0].nu = nu;
                d.m.materials[0].k = kc;
                d.m.materials[0].rho = 7850.0;
            } else {
                let mut mt = d.m.materials[0].clone();
                mt.e = e;
                mt.nu = nu;
                mt.k = kc;
                mt.name = format!("Material {}", k + 1);
                add_id(&mut d, Item::Material(mt));
            }
        }
        let mids: Vec<u32> = d.m.materials.iter().map(|m| m.id).collect();
        for (k, &(r, depth, angle, at, turn, mi)) in list.iter().enumerate() {
            let p = d.apply(Op::Add { item: Item::Part(Part { name: format!("Part {}", k + 1), depth, angle, at, turn, material: mids[mi], ..Default::default() }) }).unwrap()[0];
            let sh = d.apply(Op::Add { item: Item::Shape(Shape { geom: Geom::rect(r[0], r[1], r[2], r[3]), material: mids[mi], part: p, ..Default::default() }) }).unwrap()[0];
            ids.push((p, sh));
        }
        (d, ids)
    };
    let sup = |d: &mut Doc, at: Target, kind: SupportKind| { add_id(d, Item::Support(Support { id: 0, at, kind })); };
    let load = |d: &mut Doc, at: Target, kind: LoadKind| { add_id(d, Item::Load(Load { case: 0, id: 0, at, kind })); };
    let at3 = |r: &crate::solid::SolidResults, k: &str, p: [f64; 3]| { let i = r.volume.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, p).total_cmp(&crate::model::dist3(*b.1, p))).unwrap().0; r.volume.v[r.fields.iter().position(|f| f.key == k).unwrap()][i] };
    let (e, nu) = (200e9, 0.3);
    // A cantilever 1 m long, 0.1 × 0.1 m: its section sketched and extruded 1 m, turned a quarter
    // turn about y so it lies along x; fixed at its root, 10 kN down on its tip. Then the same
    // cantilever as two parts of 0.5 m, placed end to end and tied where they meet.
    let turn = [0.0, FRAC_PI_2, 0.0];
    let (l, h, pf): (f64, f64, f64) = (1.0, 0.1, 10e3);
    let i = h.powi(4) / 12.0;
    let want = pf * l.powi(3) / (3.0 * e * i) + pf * l / (5.0 / 6.0 * e / (2.0 * (1.0 + nu)) * h * h);
    let tip_of = |r: &crate::solid::SolidResults| at3(r, "uy", [l, h / 2.0, -h / 2.0]);
    let one = {
        let (mut d, ids) = build(Analysis::Solid, 0.025, &[([0.0, 0.0, h, h], l, 0.0, [0.0; 3], turn, 0)], &[(e, nu, 50.0)]);
        sup(&mut d, Target::PartEnd { part: ids[0].0, front: false }, SupportKind::Fixed);
        load(&mut d, Target::PartEnd { part: ids[0].0, front: true }, LoadKind::Traction { fx: 0.0, fy: -pf, fz: 0.0, total: true });
        let r = crate::solve_for(&d.m, None).map(|x| match x { crate::Results::Solid(r) => r, _ => unreachable!() }).expect("one part");
        let name = "Cantilever sketched, extruded and turned into place, one part";
        v.push(check(32, name, "Tip deflection PL³/3EI + PL/κGA", "len", -want, tip_of(&r), 0.01, "Timoshenko beam theory (a fixed root is a little stiffer)"));
        v.push(check(32, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics"));
        tip_of(&r)
    };
    {
        let (mut d, ids) = build(Analysis::Solid, 0.025, &[([0.0, 0.0, h, h], l / 2.0, 0.0, [0.0; 3], turn, 0), ([0.0, 0.0, h, h], l / 2.0, 0.0, [l / 2.0, 0.0, 0.0], turn, 0)], &[(e, nu, 50.0)]);
        sup(&mut d, Target::PartEnd { part: ids[0].0, front: false }, SupportKind::Fixed);
        load(&mut d, Target::PartEnd { part: ids[1].0, front: true }, LoadKind::Traction { fx: 0.0, fy: -pf, fz: 0.0, total: true });
        let r = crate::solve_for(&d.m, None).map(|x| match x { crate::Results::Solid(r) => r, _ => unreachable!() }).expect("two parts");
        let name = "The cantilever as two parts tied end to end";
        v.push(check(32, name, "Tip deflection, as one part", "len", one, tip_of(&r), 2e-3, "The one-part model above (the tie joins them fully)"));
        v.push(check(32, name, "Tip deflection PL³/3EI + PL/κGA", "len", -want, tip_of(&r), 0.012, "Timoshenko beam theory"));
        v.push(check(32, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics"));
    }
    // A bar 1 m along x, 0.1 × 0.1 m, of steel for its first half and aluminium for the second
    // (ν = 0), pulled by 100 MPa, symmetry on three faces: the stretch adds up exactly.
    {
        let (e2, p) = (70e9, 100e6);
        let (mut d, ids) = build(Analysis::Solid, 0.05, &[([0.0, 0.0, h, h], 0.5, 0.0, [0.0; 3], turn, 0), ([0.0, 0.0, h, h], 0.5, 0.0, [0.5, 0.0, 0.0], turn, 1)], &[(e, 0.0, 50.0), (e2, 0.0, 200.0)]);
        sup(&mut d, Target::PartEnd { part: ids[0].0, front: false }, SupportKind::Symmetry);
        for &(_, sh) in &ids {
            // Sketch edge 0 (y = 0) stays the face y = 0; edge 3 (sketch x = 0) turns into z = 0.
            sup(&mut d, Target::Edge { shape: sh, edge: 0 }, SupportKind::Symmetry);
            sup(&mut d, Target::Edge { shape: sh, edge: 3 }, SupportKind::Symmetry);
        }
        load(&mut d, Target::PartEnd { part: ids[1].0, front: true }, LoadKind::Traction { fx: p, fy: 0.0, fz: 0.0, total: false });
        let r = crate::solve_for(&d.m, None).map(|x| match x { crate::Results::Solid(r) => r, _ => unreachable!() }).expect("two materials");
        let name = "Bar of two parts, steel and aluminium, pulled";
        v.push(check(32, name, "Stretch p·L₁/E₁ + p·L₂/E₂", "len", p * 0.5 / e + p * 0.5 / e2, r.field("ux").max, 1e-3, "Uniaxial stress, springs in series (exact)"));
        v.push(check(32, name, "Stress along the bar", "stress", p, at3(&r, "sx", [0.75, h / 2.0, -h / 2.0]), 1e-3, "Uniaxial stress (exact)"));
    }
    // A thick cylinder a = 0.1, b = 0.2 m: its wall sketched and revolved a quarter turn, held
    // between its end planes and on its cut faces (plane strain), 100 MPa inside: Lamé.
    {
        let (a, b, hh, pi) = (0.1, 0.2, 0.05, 100e6);
        let (mut d, ids) = build(Analysis::Solid, 0.0125, &[([a, 0.0, b, hh], 0.0, FRAC_PI_2, [0.0; 3], [0.0; 3], 0)], &[(e, nu, 50.0)]);
        let (p, sh) = ids[0];
        for t in [Target::PartEnd { part: p, front: false }, Target::PartEnd { part: p, front: true }, Target::Edge { shape: sh, edge: 0 }, Target::Edge { shape: sh, edge: 2 }] {
            sup(&mut d, t, SupportKind::Symmetry);
        }
        load(&mut d, Target::Edge { shape: sh, edge: 3 }, LoadKind::Pressure { p: pi });
        let r = crate::solve_for(&d.m, None).map(|x| match x { crate::Results::Solid(r) => r, _ => unreachable!() }).expect("cylinder");
        let name = "Thick cylinder, its wall revolved a quarter turn as a part";
        let ua = (1.0 + nu) * a * pi / (e * (b * b - a * a)) * ((1.0 - 2.0 * nu) * a * a + b * b);
        v.push(check(32, name, "Bore grows (Lamé, plane strain)", "len", ua, at3(&r, "ux", [a, hh / 2.0, 0.0]), 0.01, "Lamé's thick cylinder (exact)"));
        v.push(check(32, name, "Equilibrium (load over reactions)", "ratio", 1.0, 1.0 + r.imbalance, 1e-8, "Statics"));
    }
    // Heat along a bar of two parts, k = 50 and 200 W/m·K, 0.5 m each, 100 °C at one end and 0 at
    // the other: the joint settles where the heat through both is the same.
    {
        let (k1, k2) = (50.0, 200.0);
        let (mut d, ids) = build(Analysis::SolidHeat, 0.05, &[([0.0, 0.0, h, h], 0.5, 0.0, [0.0; 3], turn, 0), ([0.0, 0.0, h, h], 0.5, 0.0, [0.5, 0.0, 0.0], turn, 1)], &[(e, nu, k1), (e, nu, k2)]);
        sup(&mut d, Target::PartEnd { part: ids[0].0, front: false }, SupportKind::Temperature { t: 100.0 });
        sup(&mut d, Target::PartEnd { part: ids[1].0, front: true }, SupportKind::Temperature { t: 0.0 });
        let r = crate::solve_for(&d.m, None).map(|x| match x { crate::Results::SolidHeat(r) => r, _ => unreachable!() }).expect("heat");
        let joint = 100.0 * k1 / (k1 + k2);
        let ti = r.fields.iter().position(|f| f.key == "t").unwrap();
        let j = r.volume.nodes.iter().enumerate().min_by(|x, y| crate::model::dist3(*x.1, [0.5, h / 2.0, -h / 2.0]).total_cmp(&crate::model::dist3(*y.1, [0.5, h / 2.0, -h / 2.0]))).unwrap().0;
        let name = "Heat through two parts in series, k = 50 and 200 W/m·K";
        v.push(check(32, name, "Temperature at the joint", "temp", joint, r.volume.v[ti][j], 1e-3, "Conduction in series (exact)"));
        v.push(check(32, name, "Heat balance (in over out)", "ratio", 1.0, 1.0 + r.imbalance, 1e-6, "Conservation of energy"));
    }
    v
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

/// Benchmark 34 (0.7.4): yielding with large movements of meshed solids (total Lagrangian, small
/// strains, large rotations; Bathe §6.6). A slender cantilever bent far with no yield, against the
/// elastica (the large-rotation part); and a bar pulled past yield by moving its end face, the
/// force it takes against the exact answer of this formulation, F·S·A₀ = λ σy A₀ (the plastic part
/// with a moved face).
fn yielding_large() -> Vec<Check> {
    use crate::model::{Step, StepKind};
    use crate::solids::tet::{grid, TetMesh};
    let mut v = vec![];
    let doc = |ms: TetMesh, e: f64, nu: f64| { let mut d = Doc::new(Model::new(Analysis::Solid)); d.m.materials[0].e = e; d.m.materials[0].nu = nu; d.m.materials[0].rho = 0.0; d.m.materials[0].fy = 0.0; d.m.volume = Some(Box::new(ms)); d };
    let on = |d: &Doc, c: usize, val: f64| Target::Face { face: d.m.volume.as_ref().unwrap().face_where(|q| (q[c] - val).abs() < 1e-9).expect("face") };
    let sup = |d: &mut Doc, at: Target, kind: SupportKind| { add_id(d, Item::Support(Support { id: 0, at, kind })); };
    let load = |d: &mut Doc, at: Target, kind: LoadKind| { add_id(d, Item::Load(Load { case: 0, id: 0, at, kind })); };
    let step = |d: &mut Doc, n: u32| { add(d, Op::Add { item: Item::Step(Step { kind: StepKind::LargePlastic, increments: n, ..Default::default() }) }); };
    let solid = |d: &Doc| { let Ok(crate::Results::Solid(r)) = crate::solve_for(&d.m, None) else { panic!("{:?}", crate::solve_for(&d.m, None).err()) }; r };
    // A steel cantilever bent far by a dead end load (PL²/EI = 2), nothing yields: the elastica.
    {
        let (e, l, h) = (200e9, 1.0, 0.05f64);
        let ei = e * h * h.powi(3) / 12.0;
        let mut d = doc(grid([40, 2, 2], 2, |q| [l * q[0], h * q[1], h * q[2]]).unwrap(), e, 0.3);
        let root = on(&d, 0, 0.0);
        sup(&mut d, root, SupportKind::Fixed);
        let tip = on(&d, 0, l);
        load(&mut d, tip, LoadKind::Traction { fx: 0.0, fy: -2.0 * ei / (l * l), fz: 0.0, total: true });
        step(&mut d, 10);
        let r = solid(&d);
        let (dv, _, _) = elastica_tip(2.0);
        let i = r.volume.nodes.iter().enumerate().min_by(|a, b| crate::model::dist3(*a.1, [l, h / 2.0, h / 2.0]).total_cmp(&crate::model::dist3(*b.1, [l, h / 2.0, h / 2.0]))).unwrap().0;
        v.push(check(34, "Cantilever bent far (PL²/EI = 2), yielding with large movements, nothing yields", "Tip deflection (elastica)", "len", dv * l, -r.volume.disp[i][1], 0.01, "Elastica of a cantilever with a dead end load (Bisshopp and Drucker 1945)"));
    }
    // A bar 1 × 0.1 × 0.1 m pulled 1 % past its length by its end face, perfectly plastic.
    {
        let fy = 250e6;
        let mut d = doc(grid([8, 2, 2], 2, |q| [q[0], 0.1 * q[1], 0.1 * q[2]]).unwrap(), 200e9, 0.3);
        d.m.materials[0].fy = fy;
        for (c, val) in [(0, 0.0), (1, 0.0), (2, 0.0)] {
            let t = on(&d, c, val);
            sup(&mut d, t, SupportKind::Symmetry);
        }
        let end = on(&d, 0, 1.0);
        load(&mut d, end, LoadKind::Settlement { dx: 0.01, dy: 0.0, dz: 0.0 });
        step(&mut d, 10);
        let r = solid(&d);
        let p = r.path.as_ref().expect("path");
        let name = "Bar pulled 1 % past its length by its end face, perfectly plastic, yielding with large movements";
        v.push(check(34, name, "Force at the end, λ σy A₀", "force", 1.01 * fy * 0.01, p.force.last().copied().unwrap_or(0.0), 1e-4, "Uniaxial stress, total Lagrangian (S at yield, P = F S), exact"));
        v.push(check(34, name, "Equivalent plastic strain (Green–Lagrange less yield)", "ratio", 0.5 * (1.01f64 * 1.01 - 1.0) - fy / 200e9, r.field("peeq").max, 1e-3, "Uniaxial stress, total Lagrangian, exact"));
    }
    v
}

/// Benchmark 38 (0.10.3): point and part-length loads on members, against beam theory.
fn member_loads() -> Vec<Check> {
    let mut out = vec![];
    let (l, p, w, e) = (6.0, 10e3, 4e3, 200e9);
    let (b, h) = (0.1, 0.25f64);
    let i = b * h.powi(3) / 12.0;
    let src = "Beam theory (Euler–Bernoulli; Roark, table 8.1)";
    let name = "Simply supported beam, point load off centre";
    // One member, pinned and on a roller, a point load P at a = 2 m (b = 4 m).
    let simple = |extra: &dyn Fn(&mut Doc, u32)| {
        let mut d = frame(e, SectionShape::Rect { b, h });
        member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Pinned);
        support(&mut d, l, 0.0, SupportKind::RollerX);
        let id = d.m.members[0].id;
        extra(&mut d, id);
        d
    };
    let (a, bb) = (2.0, l - 2.0);
    let d = simple(&|d, id| load(d, Target::Member { id }, LoadKind::Concentrated { f: -p, dir: Dir::Y, at: a }));
    let r = frame::solve(&d.m).expect("solves");
    let mmax = r.members[0].m.iter().copied().fold(f64::MIN, f64::max);
    let ra = r.reactions.iter().find(|x| x.node == node(&d, 0.0, 0.0)).unwrap().fy;
    let under = r.members[0].x.iter().position(|x| (x - a).abs() < 1e-9).expect("a station at the load");
    out.push(check(38, name, "Moment under the load Pab/L", "moment", p * a * bb / l, mmax, 1e-9, src));
    out.push(check(38, name, "Reaction at the near end Pb/L", "force", p * bb / l, ra, 1e-9, src));
    out.push(check(38, name, "Deflection under the load Pa²b²/3EIL", "len", p * a * a * bb * bb / (3.0 * e * i * l), -r.members[0].d[under][1], 1e-9, src));
    // The shear drops by P under the load: both values are drawn at that place.
    let at_load: Vec<usize> = r.members[0].x.iter().enumerate().filter(|(_, x)| (*x - a).abs() < 1e-9).map(|(k, _)| k).collect();
    out.push(check(38, name, "Shear just before the load Pb/L", "force", p * bb / l, r.members[0].v[at_load[0]], 1e-9, src));
    out.push(check(38, name, "Shear just after the load −Pa/L", "force", -p * a / l, r.members[0].v[*at_load.last().unwrap()], 1e-9, src));
    // A uniform load over the middle half (c = 3 m, centred): midspan wc(8L³ − 4Lc² + c³)/384EI
    // and moment (wc/2)(L/2) − (wc/2)(c/4).
    let c = 3.0;
    let name = "Simply supported beam, uniform load over the middle";
    let d = simple(&|d, id| load(d, Target::Member { id }, LoadKind::Patch { w1: -w, w2: -w, dir: Dir::Y, from: (l - c) / 2.0, to: (l + c) / 2.0, projected: false }));
    let r = frame::solve(&d.m).expect("solves");
    let mid = r.members[0].x.iter().position(|x| (x - l / 2.0).abs() < 1e-9).expect("a station at midspan");
    out.push(check(38, name, "Midspan moment wc(2L − c)/8", "moment", w * c * (2.0 * l - c) / 8.0, r.members[0].m[mid], 1e-9, src));
    out.push(check(38, name, "Midspan deflection wc(8L³ − 4Lc² + c³)/384EI", "len", w * c * (8.0 * l.powi(3) - 4.0 * l * c * c + c.powi(3)) / (384.0 * e * i), -r.members[0].d[mid][1], 1e-9, src));
    // A load rising from zero over the last third: the reactions from statics.
    let name = "Simply supported beam, load rising over the last third";
    let d = simple(&|d, id| load(d, Target::Member { id }, LoadKind::Patch { w1: 0.0, w2: -w, dir: Dir::Y, from: 4.0, to: 6.0, projected: false }));
    let r = frame::solve(&d.m).expect("solves");
    let total = w * 2.0 / 2.0;
    let rb = r.reactions.iter().find(|x| x.node == node(&d, l, 0.0)).unwrap().fy;
    out.push(check(38, name, "Far reaction W·x̄/L, x̄ = 4 + 2·2/3 m", "force", total * (4.0 + 4.0 / 3.0) / l, rb, 1e-9, "Statics"));
    // A rafter from (0, 0) to (4, 3): a vertical load per metre of plan (projected) against per
    // metre of rafter.
    let name = "Inclined rafter, vertical load per plan length";
    for (projected, run, what) in [(true, 4.0, "Vertical reactions sum to w × plan length 4 m"), (false, 5.0, "Vertical reactions sum to w × rafter length 5 m")] {
        let mut d = frame(e, SectionShape::Rect { b, h });
        member(&mut d, [0.0, 0.0], [4.0, 3.0], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Pinned);
        support(&mut d, 4.0, 3.0, SupportKind::RollerX);
        let id = d.m.members[0].id;
        load(&mut d, Target::Member { id }, LoadKind::Patch { w1: -w, w2: -w, dir: Dir::Y, from: 0.0, to: 5.0, projected });
        let r = frame::solve(&d.m).expect("solves");
        out.push(check(38, name, what, "force", w * run, r.reactions.iter().map(|x| x.fy).sum(), 1e-9, "Statics"));
    }
    // A cantilever, P at a from the root: tip Pa²(3L − a)/6EI.
    let (lc, ac) = (3.0, 2.0);
    let mut d = frame(e, SectionShape::Rect { b, h });
    member(&mut d, [0.0, 0.0], [lc, 0.0], MemberKind::Beam);
    support(&mut d, 0.0, 0.0, SupportKind::Fixed);
    let id = d.m.members[0].id;
    load(&mut d, Target::Member { id }, LoadKind::Concentrated { f: -p, dir: Dir::Local, at: ac });
    let r = frame::solve(&d.m).expect("solves");
    let tip = d.m.nodes.iter().position(|n| n.id == node(&d, lc, 0.0)).unwrap();
    out.push(check(38, "Cantilever, point load inside the span", "Tip deflection Pa²(3L − a)/6EI", "len", p * ac * ac * (3.0 * lc - ac) / (6.0 * e * i), -r.disp[tip][1], 1e-9, src));
    // The same in 3D, the load along z (the weak axis).
    let pr = crate::sections::props(&SectionShape::Rect { b, h }).unwrap();
    let mut d = space_frame(e, SectionShape::Rect { b, h });
    member3(&mut d, [0.0; 3], [lc, 0.0, 0.0], MemberKind::Beam);
    support3(&mut d, [0.0; 3], SupportKind::Fixed);
    let id = d.m.members[0].id;
    load(&mut d, Target::Member { id }, LoadKind::Concentrated { f: p, dir: Dir::Z, at: ac });
    load(&mut d, Target::Member { id }, LoadKind::Patch { w1: w, w2: w, dir: Dir::Z, from: 0.0, to: 1.0, projected: false });
    let r = crate::space::solve(&d.m).expect("space");
    // The patch: w over [0, c] gives a tip deflection wc³(4L − c)/24EI.
    let (c3, ei) = (1.0f64, e * pr.iy);
    let want = p * ac * ac * (3.0 * lc - ac) / (6.0 * ei) + w * c3.powi(3) * (4.0 * lc - c3) / (24.0 * ei);
    out.push(check(38, "3D cantilever, point and part-length loads along z", "Tip deflection Pa²(3L − a)/6EIy + wc³(4L − c)/24EIy", "len", want, r.disp[1][2], 1e-9, src));
    // A member cut after it was loaded keeps its loads where they were.
    let name = "Loaded beam cut in two by a new member";
    let d0 = simple(&|d, id| {
        load(d, Target::Member { id }, LoadKind::Concentrated { f: -p, dir: Dir::Y, at: 4.5 });
        load(d, Target::Member { id }, LoadKind::Patch { w1: -w, w2: -2.0 * w, dir: Dir::Y, from: 1.0, to: 5.0, projected: false });
    });
    let mut d1 = d0.clone();
    member(&mut d1, [3.0, 0.0], [3.0, 1.0], MemberKind::Beam);
    let (r0, r1) = (frame::solve(&d0.m).expect("solves"), frame::solve(&d1.m).expect("solves"));
    // The deflection and moment under the point load (4.5 m from the start, 1.5 m into the far piece).
    let under = |r: &frame::FrameResults, k: usize, x: f64| {
        let mr = &r.members[k];
        let i = mr.x.iter().position(|s| (s - x).abs() < 1e-9).expect("a station at the load");
        (mr.d[i][1], mr.m[i])
    };
    let far = d1.m.members.iter().position(|mb| mb.a == node(&d1, 3.0, 0.0) && mb.b == node(&d1, l, 0.0)).expect("the far piece");
    let (u0, m0) = under(&r0, 0, 4.5);
    let (u1, m1) = under(&r1, far, 1.5);
    out.push(check(38, name, "Deflection under the point load, before and after the cut", "len", u0, u1, 1e-9, "The same model"));
    out.push(check(38, name, "Moment under the point load, before and after the cut", "moment", m0, m1, 1e-9, "The same model"));
    let ra = |r: &frame::FrameResults, d: &Doc| r.reactions.iter().find(|x| x.node == node(d, 0.0, 0.0)).unwrap().fy;
    out.push(check(38, name, "Reaction at the near end, before and after the cut", "force", ra(&r0, &d0), ra(&r1, &d1), 1e-9, "The same model"));
    out
}

/// Benchmark 39 (0.10.3): rows asked for by the structural specification, each against a hand
/// formula: an axial bar, a fixed–fixed beam whose end moves, a force on an arm, a round shaft in
/// torsion, CQC with close and far modes, and a tube's volume.
fn spec_rows() -> Vec<Check> {
    let mut out = vec![];
    let (e, b, h) = (200e9, 0.05, 0.1f64);
    let (a, i) = (b * h, b * h.powi(3) / 12.0);
    let src = "Mechanics of materials";
    // An axial bar: P along a 3 m bar fixed at one end: δ = PL/EA, N = P, σ = P/A.
    {
        let (l, p) = (3.0, 100e3);
        let mut d = frame(e, SectionShape::Rect { b, h });
        member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let tip = node(&d, l, 0.0);
        load(&mut d, Target::Node { id: tip }, LoadKind::Force { fx: p, fy: 0.0, fz: 0.0 });
        let r = frame::solve(&d.m).expect("solves");
        let ti = d.m.nodes.iter().position(|n| n.id == tip).unwrap();
        out.push(check(39, "Axial bar, 100 kN on 3 m", "Extension PL/EA", "len", p * l / (e * a), r.disp[ti][0], 1e-9, src));
        out.push(check(39, "Axial bar, 100 kN on 3 m", "Axial force P (tension)", "force", p, r.members[0].n[0], 1e-9, src));
        out.push(check(39, "Axial bar, 100 kN on 3 m", "Stress P/A", "stress", p / a, r.members[0].stress, 1e-9, src));
    }
    // A fixed–fixed beam whose right end sinks δ: end moments 6EIδ/L², end shears 12EIδ/L³.
    {
        let (l, dl) = (6.0, -0.01);
        let mut d = frame(e, SectionShape::Rect { b, h });
        member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        support(&mut d, l, 0.0, SupportKind::Fixed);
        let end = node(&d, l, 0.0);
        load(&mut d, Target::Node { id: end }, LoadKind::Settlement { dx: 0.0, dy: dl, dz: 0.0 });
        let r = frame::solve(&d.m).expect("solves");
        let left = r.reactions.iter().find(|x| x.node == node(&d, 0.0, 0.0)).unwrap();
        let src = "Beam theory: slope–deflection, a support moved";
        out.push(check(39, "Fixed–fixed beam, one end sinks 10 mm", "End moment 6EIδ/L²", "moment", 6.0 * e * i * dl.abs() / (l * l), left.m.abs(), 1e-9, src));
        out.push(check(39, "Fixed–fixed beam, one end sinks 10 mm", "End shear 12EIδ/L³", "force", 12.0 * e * i * dl.abs() / l.powi(3), left.fy.abs(), 1e-9, src));
    }
    // A force on an arm: a cantilever 4 m along x, an arm 1 m up at its tip, 10 kN along x at the
    // arm's end: the root takes −F and the moment F·e, and the cantilever carries N = F.
    {
        let (l, ea, f) = (4.0, 1.0, 10e3);
        let mut d = frame(e, SectionShape::Rect { b, h });
        member(&mut d, [0.0, 0.0], [l, 0.0], MemberKind::Beam);
        member(&mut d, [l, 0.0], [l, ea], MemberKind::Beam);
        support(&mut d, 0.0, 0.0, SupportKind::Fixed);
        let top = node(&d, l, ea);
        load(&mut d, Target::Node { id: top }, LoadKind::Force { fx: f, fy: 0.0, fz: 0.0 });
        let r = frame::solve(&d.m).expect("solves");
        let root = r.reactions.iter().find(|x| x.node == node(&d, 0.0, 0.0)).unwrap();
        let src = "Statics: a force moved to a point is the force and its moment, M = F·e";
        out.push(check(39, "Force of 10 kN on a 1 m arm at a cantilever's tip", "Root moment F·e", "moment", f * ea, root.m.abs(), 1e-9, src));
        out.push(check(39, "Force of 10 kN on a 1 m arm at a cantilever's tip", "Root reaction along x, F", "force", f, root.fx.abs(), 1e-9, src));
        out.push(check(39, "Force of 10 kN on a 1 m arm at a cantilever's tip", "Axial force in the cantilever, F", "force", f, r.members[0].n[0].abs(), 1e-9, src));
    }
    // A round shaft 50 mm across, 1 m long, twisted by 1 kN·m: θ = TL/GJ, J = πd⁴/32.
    {
        let (d0, l, t) = (0.05f64, 1.0, 1e3);
        let jp = std::f64::consts::PI * d0.powi(4) / 32.0;
        let g = e / (2.0 * (1.0 + Material::steel(1).nu));
        let pr = crate::sections::props(&SectionShape::Circle { d: d0 }).unwrap();
        out.push(check(39, "Round shaft 50 mm across", "Torsion constant J = πd⁴/32", "ratio", jp, pr.j, 1e-12, src));
        let mut d = space_frame(e, SectionShape::Circle { d: d0 });
        member3(&mut d, [0.0; 3], [l, 0.0, 0.0], MemberKind::Beam);
        support3(&mut d, [0.0; 3], SupportKind::Fixed);
        let id = node3(&d, [l, 0.0, 0.0]);
        load(&mut d, Target::Node { id }, LoadKind::Moment { m: 0.0, mx: t, my: 0.0 });
        let r = crate::space::solve(&d.m).expect("space");
        out.push(check(39, "Round shaft 50 mm across, 1 kN·m on 1 m", "Twist TL/GJ", "rad", t * l / (g * jp), r.disp[1][3], 1e-9, src));
        out.push(check(39, "Round shaft 50 mm across, 1 kN·m on 1 m", "Root torque T", "moment", t, r.reactions[0].f[3].abs(), 1e-9, "Statics"));
    }
    // CQC (Der Kiureghian 1981): two modes at the same frequency are fully correlated, so
    // their peaks add; far apart they are not, and CQC falls to SRSS; between, the closed form.
    {
        use crate::engine::dynamics::{combine, cqc};
        let src = "Der Kiureghian (1981), the CQC correlation of two modes";
        let z = 0.05;
        out.push(check(39, "CQC, two modes at the same frequency", "Correlation ρ = 1", "ratio", 1.0, cqc(10.0, 10.0, z), 1e-12, src));
        let rho = |w2: f64| vec![vec![1.0, cqc(10.0, w2, z)], vec![cqc(w2, 10.0, z), 1.0]];
        out.push(check(39, "CQC, two modes at the same frequency, values 3 and 4", "Combined 3 + 4", "ratio", 7.0, combine(&[3.0, 4.0], &rho(10.0)), 1e-12, src));
        out.push(check(39, "CQC, modes 1 : 20 apart, values 3 and 4", "Combined as SRSS, √(3² + 4²)", "ratio", 5.0, combine(&[3.0, 4.0], &rho(200.0)), 2e-3, src));
        // Frequency ratio 0.9 and 5 % damping, the closed form worked by hand:
        // 8·0.05²·1.9·0.9^1.5 / ((1 − 0.81)² + 4·0.05²·0.9·1.9²) = 0.0324446 / 0.068590 = 0.473024.
        let hand = 8.0 * 0.0025 * 1.9 * 0.9f64.powf(1.5) / (0.19f64.powi(2) + 4.0 * 0.0025 * 0.9 * 3.61);
        out.push(check(39, "CQC, frequency ratio 0.9, 5 % damping", "Correlation ρ ≈ 0.473", "ratio", hand, cqc(10.0, 9.0, z), 1e-12, src));
    }
    // A tube: a ring of R = 50 mm and r = 40 mm extruded 0.5 m: π(R² − r²)L.
    {
        let mut d = Doc::new(Model::new(Analysis::Solid));
        let pid = add_id(&mut d, Item::Part(crate::model::Part { depth: 0.5, ..Default::default() }));
        let mat = d.m.materials[0].id;
        add_id(&mut d, Item::Shape(Shape { geom: Geom::Circle { c: [0.0, 0.0], r: 0.05 }, material: mat, part: pid, ..Default::default() }));
        add_id(&mut d, Item::Shape(Shape { geom: Geom::Circle { c: [0.0, 0.0], r: 0.04 }, material: mat, part: pid, hole: true, ..Default::default() }));
        let want = std::f64::consts::PI * (0.05f64.powi(2) - 0.04f64.powi(2)) * 0.5;
        out.push(check(39, "Tube R = 50 mm, r = 40 mm, 0.5 m long", "Volume π(R² − r²)L", "ratio", want, crate::solids::modelling::volume(&d.m, pid).unwrap_or(0.0), 1e-12, "Volume of a prism: area × depth"));
    }
    out
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
    v.extend(solids_more());
    v.extend(dxf());
    v.extend(large());
    v.extend(plates());
    v.extend(dynamics());
    v.extend(details());
    v.extend(plasticity());
    v.extend(solid_plasticity());
    v.extend(rubber());
    v.extend(tetra());
    v.extend(tetra_more());
    v.extend(tetra_nonlinear());
    v.extend(space_large());
    v.extend(assembly());
    v.extend(modelling());
    v.extend(yielding_large());
    v.extend(remote_load());
    v.extend(contact());
    v.extend(explicit());
    v.extend(member_loads());
    v.extend(spec_rows());
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn spec_rows_on_their_own() {
        for r in super::spec_rows() {
            assert!(r.pass, "{}: {} gave {} against {} (error {:.2e})", r.name, r.quantity, r.result, r.reference, r.error);
        }
    }

    #[test]
    fn member_loads_on_their_own() {
        for r in super::member_loads() {
            assert!(r.pass, "{}: {} gave {} against {} (error {:.2e})", r.name, r.quantity, r.result, r.reference, r.error);
        }
    }

    #[test]
    fn explicit_on_its_own() {
        for r in super::explicit() {
            assert!(r.pass, "{}: {} gave {} against {} (error {:.2e})", r.name, r.quantity, r.result, r.reference, r.error);
        }
    }

    #[test]
    fn contact_on_its_own() {
        for r in super::contact() {
            assert!(r.pass, "{}: {} gave {} against {} (error {:.2e})", r.name, r.quantity, r.result, r.reference, r.error);
        }
    }

    #[test]
    fn remote_load_on_its_own() {
        for r in super::remote_load() {
            assert!(r.pass, "{}: {} gave {} against {} (error {:.2e})", r.name, r.quantity, r.result, r.reference, r.error);
        }
    }

    #[test]
    fn yielding_with_large_movements_on_its_own() {
        for r in super::yielding_large() {
            assert!(r.pass, "{}: {} gave {} against {} (error {:.2e})", r.name, r.quantity, r.result, r.reference, r.error);
        }
    }

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
        for b in 1..=25 {
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
        m.members.push(Member { id: next, a, b, kind: MemberKind::Beam, section: 2, material: 1, roll: 0.0, hinge: [false; 2], only: Default::default(), offset: [0.0; 2] });
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
    time("L-bracket in 3D example: 6 natural frequencies", 3000.0, &mut || crate::solid::modes(&b3, 6).unwrap().equations);
    let f3 = crate::examples::get("fin3d").unwrap();
    time("Heat sink in 3D example: mesh and solve", 1000.0, &mut || crate::solid::heat(&f3).unwrap().stats.equations);
    let v3 = crate::examples::get("vessel3d").unwrap();
    time("Vessel in 3D (revolved) example: mesh and solve", 2000.0, &mut || crate::solid::solve(&v3).unwrap().stats.equations);
    let tall = building(10, 12, 8);
    time("3D building frame, 10 × 8 bays, 12 storeys: solve", 10_000.0, &mut || crate::space::solve(&tall).unwrap().stats.equations);
    time("3D building frame, 10 × 8 bays, 12 storeys: 6 natural frequencies", 10_000.0, &mut || crate::space::modes(&tall, 6).unwrap().equations);
    out
}
