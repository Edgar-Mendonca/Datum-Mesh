//! The worked examples (File › Examples), built in code: the six chosen for the convention, and
//! blank models of each kind.

use crate::doc::{Doc, Item, Op};
use crate::model::{Analysis, Dir, Geom, Load, LoadKind, MemberKind, Model, Shape, Support, SupportKind, Target};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Example {
    pub id: &'static str,
    pub name: &'static str,
    pub what: &'static str,
    pub analysis: Analysis,
}

pub fn list() -> Vec<Example> {
    let e = |id, name, what, analysis| Example { id, name, what, analysis };
    vec![
        e("blank-frame", "Blank frame", "Nodes and members: beams and trusses in 2D.", Analysis::Frame),
        e("blank-stress", "Blank part", "Plane stress: a thin plate loaded in its plane.", Analysis::Stress),
        e("blank-space", "Blank 3D frame", "Nodes and members in 3D: beams that bend both ways and twist, and truss bars.", Analysis::Space),
        e("blank-solid", "Blank 3D solid", "Draw a part's outline; it is extruded by its thickness into a 3D solid.", Analysis::Solid),
        e("portal", "Portal frame", "A pitched portal frame with wind on one side and load on the roof.", Analysis::Frame),
        e("truss", "Roof truss", "A 12 m roof truss of pinned bars carrying purlin loads.", Analysis::Frame),
        e("braced", "Braced frame", "A two-storey steel frame: HEB columns, IPE beams pinned at their ends, one braced bay, and dead, imposed and wind load cases with combinations.", Analysis::Frame),
        e("lug", "Lifting lug", "A plate with a pin hole, pulled through the pin.", Analysis::Stress),
        e("bracket", "L-bracket", "An L-bracket with a fillet in the corner, where the stress gathers.", Analysis::Stress),
        e("flange", "Flange plate", "A round plate with a pressurised bore and six bolt holes.", Analysis::Stress),
        e("fin", "Heat sink", "A heat sink with four fins cooled by air.", Analysis::Heat),
        e("vessel", "Pressure vessel", "A cylinder with a round head under 2 MPa inside, as a half-section turned about its axis.", Analysis::Axi),
        e("vessel3d", "Vessel in 3D", "The pressure vessel revolved a quarter turn about its axis: a 3D solid with symmetry on its cut faces.", Analysis::Solid),
        e("bracket3d", "L-bracket in 3D", "The L-bracket as a solid 10 mm thick: the stress through the thickness, in 3D.", Analysis::Solid),
        e("platform", "Steel platform", "A 6 × 4 m platform on four columns, 3 m up, braced on two sides, carrying a floor load and a push from the side: a 3D frame.", Analysis::Space),
        e("hotplate", "Heated plate", "A steel plate with a hole, fixed at its cool edge and heated at the other: thermal stress.", Analysis::Stress),
    ]
}

fn sup(at: Target, kind: SupportKind) -> Op {
    Op::Add { item: Item::Support(Support { id: 0, at, kind }) }
}
fn load(at: Target, kind: LoadKind) -> Op {
    Op::Add { item: Item::Load(Load { case: 0, id: 0, at, kind }) }
}
fn shape(geom: Geom, hole: bool) -> Op {
    Op::Add { item: Item::Shape(Shape { material: 1, geom, hole, ..Shape::default() }) }
}
fn poly(pts: &[[f64; 2]], bulge: &[(usize, f64)]) -> Geom {
    let mut b = vec![0.0; pts.len()];
    for (i, v) in bulge {
        b[*i] = *v;
    }
    Geom::Poly { pts: pts.iter().map(|p| [p[0] / 1000.0, p[1] / 1000.0]).collect(), bulge: b }
}
fn circle(x: f64, y: f64, r: f64) -> Geom {
    Geom::Circle { c: [x / 1000.0, y / 1000.0], r: r / 1000.0 }
}
fn edge(shape: u32, edge: u32) -> Target {
    Target::Edge { shape, edge }
}
fn node(d: &Doc, x: f64, y: f64) -> Target {
    Target::Node { id: d.m.nodes.iter().find(|n| (n.x - x).abs() < 1e-9 && (n.y - y).abs() < 1e-9).map(|n| n.id).unwrap_or(0) }
}
fn member(d: &Doc, a: [f64; 2], b: [f64; 2]) -> Target {
    let id = |p: [f64; 2]| d.m.nodes.iter().find(|n| (n.x - p[0]).abs() < 1e-9 && (n.y - p[1]).abs() < 1e-9).map(|n| n.id).unwrap_or(0);
    let (a, b) = (id(a), id(b));
    Target::Member { id: d.m.members.iter().find(|m| (m.a == a && m.b == b) || (m.a == b && m.b == a)).map(|m| m.id).unwrap_or(0) }
}
const QUARTER: f64 = 0.414_213_562_373_095; // tan(90°/4)

/// A worked example by id (None if there is no such example).
pub fn get(id: &str) -> Option<Model> {
    let ex = list().into_iter().find(|e| e.id == id)?;
    let mut d = Doc::new(Model::new(ex.analysis));
    d.m.name = ex.name.into();
    let r = build(id, &mut d);
    debug_assert!(r.is_ok(), "{id}: {r:?}");
    r.ok()?;
    Some(d.m)
}

fn build(id: &str, d: &mut Doc) -> Result<(), String> {
    let mm = 0.001;
    match id {
        "platform" => {
            d.m.notes = "Four fixed columns 3 m high at the corners of a 6 × 4 m plan; edge beams round the top and a middle beam across; one diagonal brace on the back and one on the left side. Floor 8 kN/m on the three cross beams, 4 kN pushing along x at the two left corners, and self-weight.".into();
            let (w, dp, h) = (6.0, 4.0, 3.0);
            let mut bars: Vec<([f64; 3], [f64; 3], MemberKind)> = vec![];
            for [x, z] in [[0.0, 0.0], [w, 0.0], [0.0, dp], [w, dp]] {
                bars.push(([x, 0.0, z], [x, h, z], MemberKind::Beam));
            }
            for z in [0.0, dp] {
                bars.push(([0.0, h, z], [w / 2.0, h, z], MemberKind::Beam));
                bars.push(([w / 2.0, h, z], [w, h, z], MemberKind::Beam));
            }
            for x in [0.0, w / 2.0, w] {
                bars.push(([x, h, 0.0], [x, h, dp], MemberKind::Beam));
            }
            bars.push(([0.0, 0.0, 0.0], [w / 2.0, h, 0.0], MemberKind::Truss));
            bars.push(([0.0, 0.0, dp], [0.0, h, 0.0], MemberKind::Truss));
            for (a, b, kind) in &bars {
                d.apply(Op::AddMember { from: [a[0], a[1]], to: [b[0], b[1]], kind: *kind, section: None, material: None, z: [a[2], b[2]] })?;
            }
            // Braces: a 76.1 × 4 tube.
            let mut brace = d.m.sections[0].clone();
            brace.id = 0;
            brace.name = "Tube 76.1 × 4".into();
            brace.shape = crate::model::SectionShape::Tube { d: 0.0761, t: 0.004 };
            let bs = d.apply(Op::Add { item: Item::Section(brace) })?[0];
            let at = |d: &Doc, p: [f64; 3]| d.m.nodes.iter().find(|n| crate::model::dist3(n.at(), p) < 1e-9).map(|n| n.id).unwrap_or(0);
            let mut ops = vec![];
            for mb in d.m.members.iter().filter(|m| m.kind == MemberKind::Truss) {
                ops.push(Op::Set { item: Item::Member(crate::model::Member { section: bs, ..mb.clone() }) });
            }
            for [x, z] in [[0.0, 0.0], [w, 0.0], [0.0, dp], [w, dp]] {
                ops.push(sup(Target::Node { id: at(d, [x, 0.0, z]) }, SupportKind::Fixed));
            }
            for x in [0.0, w / 2.0, w] {
                let (a, b) = (at(d, [x, h, 0.0]), at(d, [x, h, dp]));
                let id = d.m.members.iter().find(|m| (m.a == a && m.b == b) || (m.a == b && m.b == a)).map(|m| m.id).unwrap_or(0);
                ops.push(load(Target::Member { id }, LoadKind::Dist { w1: -8e3, w2: -8e3, dir: Dir::Y }));
            }
            for z in [0.0, dp] {
                ops.push(load(Target::Node { id: at(d, [0.0, h, z]) }, LoadKind::Force { fx: 4e3, fy: 0.0, fz: 0.0 }));
            }
            ops.push(load(Target::Body, LoadKind::Gravity { gx: 0.0, gy: -9.81, gz: 0.0 }));
            d.apply(Op::Batch(ops))?;
        }
        "portal" => {
            d.m.notes = "Span 12 m, eaves 5 m, ridge 6.5 m; fixed bases. Wind 2 kN/m on the left column, 6 kN/m down on the rafters.".into();
            let p = [[0.0, 0.0], [0.0, 5.0], [6.0, 6.5], [12.0, 5.0], [12.0, 0.0]];
            for w in p.windows(2) {
                d.apply(Op::AddMember { from: w[0], to: w[1], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] })?;
            }
            let mut ops = vec![sup(node(d, 0.0, 0.0), SupportKind::Fixed), sup(node(d, 12.0, 0.0), SupportKind::Fixed)];
            ops.push(load(member(d, p[0], p[1]), LoadKind::Dist { w1: 2e3, w2: 2e3, dir: Dir::X }));
            ops.push(load(member(d, p[1], p[2]), LoadKind::Dist { w1: -6e3, w2: -6e3, dir: Dir::Y }));
            ops.push(load(member(d, p[2], p[3]), LoadKind::Dist { w1: -6e3, w2: -6e3, dir: Dir::Y }));
            d.apply(Op::Batch(ops))?;
        }
        "braced" => {
            d.m.notes = "Two storeys of 3.5 m and two bays of 6 m. Columns HEB 200, continuous; floor and roof beams IPE 300, pinned to the columns at both ends (simple connections); the left bay braced with 30 mm round bars. Pinned bases. Load cases: dead (8 kN/m on the floor, 5 kN/m on the roof), imposed (10 and 3 kN/m) and wind from the left (12 kN and 6 kN at the floors). The combinations' factors are an example only: use your own.".into();
            let sec = |d: &mut Doc, name: &str, shape: crate::model::SectionShape| -> Result<u32, String> { Ok(d.apply(Op::Add { item: Item::Section(crate::model::Section { id: 0, name: name.into(), shape }) })?[0]) };
            let col = sec(d, "HEB 200", crate::steel::find("HEB 200").ok_or("table")?.shape)?;
            let beam = sec(d, "IPE 300", crate::steel::find("IPE 300").ok_or("table")?.shape)?;
            let rod = sec(d, "Round bar 30", crate::model::SectionShape::Circle { d: 0.03 })?;
            let (xs, ys) = ([0.0, 6.0, 12.0], [0.0, 3.5, 7.0]);
            let add = |d: &mut Doc, a: [f64; 2], b: [f64; 2], kind: MemberKind, section: u32| d.apply(Op::AddMember { from: a, to: b, kind, section: Some(section), material: None, z: [0.0; 2] }).map(|_| ());
            for x in xs {
                add(d, [x, 0.0], [x, 3.5], MemberKind::Beam, col)?;
                add(d, [x, 3.5], [x, 7.0], MemberKind::Beam, col)?;
            }
            for y in [3.5, 7.0] {
                add(d, [0.0, y], [6.0, y], MemberKind::Beam, beam)?;
                add(d, [6.0, y], [12.0, y], MemberKind::Beam, beam)?;
            }
            add(d, [0.0, 0.0], [6.0, 3.5], MemberKind::Truss, rod)?;
            add(d, [0.0, 3.5], [6.0, 7.0], MemberKind::Truss, rod)?;
            d.apply(Op::Delete { ids: vec![2] })?;
            // The beams are pinned to the columns.
            let beams: Vec<crate::model::Member> = d.m.members.iter().filter(|b| b.section == beam).cloned().collect();
            d.apply(Op::Batch(beams.into_iter().map(|b| Op::Set { item: Item::Member(crate::model::Member { hinge: [true, true], ..b }) }).collect()))?;
            let mut ops: Vec<Op> = xs.iter().map(|x| sup(node(d, *x, ys[0]), SupportKind::Pinned)).collect();
            d.apply(Op::Batch(std::mem::take(&mut ops)))?;
            let case = |d: &mut Doc, name: &str| -> Result<u32, String> { Ok(d.apply(Op::Add { item: Item::Case(crate::model::Case { id: 0, name: name.into() }) })?[0]) };
            let (g, q, w) = (case(d, "Dead")?, case(d, "Imposed")?, case(d, "Wind")?);
            let mut on = |c: u32, at: Target, kind: LoadKind| ops.push(Op::Add { item: Item::Load(Load { case: c, id: 0, at, kind }) });
            for (y, dead, imposed) in [(3.5, 8e3, 10e3), (7.0, 5e3, 3e3)] {
                for (a, b) in [([0.0, y], [6.0, y]), ([6.0, y], [12.0, y])] {
                    let t = member(d, a, b);
                    on(g, t.clone(), LoadKind::Dist { w1: -dead, w2: -dead, dir: Dir::Y });
                    on(q, t, LoadKind::Dist { w1: -imposed, w2: -imposed, dir: Dir::Y });
                }
            }
            on(w, node(d, 0.0, 3.5), LoadKind::Force { fx: 12e3, fy: 0.0, fz: 0.0 });
            on(w, node(d, 0.0, 7.0), LoadKind::Force { fx: 6e3, fy: 0.0, fz: 0.0 });
            d.apply(Op::Batch(ops))?;
            let combo = |name: &str, parts: &[(u32, f64)]| Op::Add { item: Item::Combo(crate::model::Combo { id: 0, name: name.into(), parts: parts.iter().map(|&(case, f)| crate::model::Factor { case, f }).collect() }) };
            d.apply(Op::Batch(vec![combo("Combination 1", &[(g, 1.35), (q, 1.5)]), combo("Combination 2", &[(g, 1.0), (w, 1.5)]), combo("Combination 3", &[(g, 1.35), (q, 1.5), (w, 0.9)])]))?;
        }
        "truss" => {
            d.m.notes = "A 12 m Pratt truss, 2 m deep, six 2 m panels; pinned at the left, on a roller at the right; 10 kN at each top-chord joint.".into();
            let (span, h, n) = (12.0, 2.0, 6);
            let dx = span / n as f64;
            let mut ops = vec![];
            for i in 0..n {
                let (x0, x1) = (i as f64 * dx, (i + 1) as f64 * dx);
                ops.push([[x0, 0.0], [x1, 0.0]]);
                if i > 0 && i < n - 1 {
                    ops.push([[x0, h], [x1, h]]);
                }
            }
            for i in 1..n {
                let x = i as f64 * dx;
                ops.push([[x, 0.0], [x, h]]);
            }
            ops.push([[0.0, 0.0], [dx, h]]);
            ops.push([[span, 0.0], [span - dx, h]]);
            for i in 1..n / 2 {
                let x = i as f64 * dx;
                ops.push([[x, h], [x + dx, 0.0]]);
                ops.push([[span - x, h], [span - x - dx, 0.0]]);
            }
            for [a, b] in ops {
                d.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Truss, section: None, material: None, z: [0.0; 2] })?;
            }
            // Truss bars: a 100 × 100 × 10 angle.
            let s = d.m.sections[0].id;
            let mut sec = d.m.sections[0].clone();
            sec.name = "Angle 100 × 100 × 10".into();
            sec.shape = crate::model::SectionShape::Angle { h: 0.1, b: 0.1, t: 0.01 };
            let mut ops = vec![Op::Set { item: Item::Section(sec) }, sup(node(d, 0.0, 0.0), SupportKind::Pinned), sup(node(d, span, 0.0), SupportKind::RollerX)];
            for i in 1..n {
                ops.push(load(node(d, i as f64 * dx, h), LoadKind::Force { fx: 0.0, fy: -10e3, fz: 0.0 }));
            }
            d.apply(Op::Batch(ops))?;
            debug_assert!(d.m.members.iter().all(|m| m.section == s));
        }
        "lug" => {
            d.m.notes = "A 20 mm steel plate, 120 mm wide with a rounded top and a 40 mm pin hole; the base is welded on and a pin bears on the top of the hole, pulling 60 kN upwards.".into();
            d.m.thickness = 20.0 * mm;
            d.m.grid = 5.0 * mm;
            let s = d.apply(shape(poly(&[[0.0, 0.0], [120.0, 0.0], [120.0, 120.0], [0.0, 120.0]], &[(2, 1.0)]), false))?[0];
            let h = d.apply(shape(circle(60.0, 120.0, 20.0), true))?[0];
            d.apply(Op::Batch(vec![sup(edge(s, 0), SupportKind::Fixed), load(edge(h, 0), LoadKind::Bearing { fx: 0.0, fy: 60e3 })]))?;
        }
        "bracket3d" => {
            d.m.notes = "The L-bracket as a 3D solid: legs 100 mm long and 20 mm thick, 10 mm deep (the extrusion), a 10 mm fillet inside the corner. The top end is bolted to a wall (fixed face); 1.5 kN pulls down on the free end's face.".into();
            d.m.grid = 5.0 * mm;
            d.m.thickness = 10.0 * mm;
            let s = d.apply(shape(poly(&[[0.0, 0.0], [100.0, 0.0], [100.0, 20.0], [30.0, 20.0], [20.0, 30.0], [20.0, 100.0], [0.0, 100.0]], &[(3, -QUARTER)]), false))?[0];
            d.apply(Op::Batch(vec![sup(edge(s, 5), SupportKind::Fixed), load(edge(s, 1), LoadKind::Traction { fx: 0.0, fy: -1.5e3, total: true, fz: 0.0 })]))?;
            // Elements of 5 mm, 2.5 mm round the fillet, two quadratic layers: under a second.
            d.m.mesh.size = 5.0 * mm;
            d.m.mesh.layers = 2;
            d.m.mesh.refine.push(crate::model::Refine { at: edge(s, 3), size: 2.5 * mm });
        }
        "bracket" => {
            d.m.notes = "A 10 mm steel L-bracket, legs 100 mm long and 20 mm thick, with a 10 mm fillet in the inside corner. The top is bolted to a wall; 1.5 kN pulls down on the free end.".into();
            d.m.grid = 5.0 * mm;
            let s = d.apply(shape(poly(&[[0.0, 0.0], [100.0, 0.0], [100.0, 20.0], [30.0, 20.0], [20.0, 30.0], [20.0, 100.0], [0.0, 100.0]], &[(3, -QUARTER)]), false))?[0];
            d.apply(Op::Batch(vec![sup(edge(s, 5), SupportKind::Fixed), load(edge(s, 1), LoadKind::Traction { fx: 0.0, fy: -1.5e3, total: true, fz: 0.0 })]))?;
            // Finer elements round the fillet, where the stress peaks.
            d.m.mesh.refine.push(crate::model::Refine { at: edge(s, 3), size: 1.0 * mm });
        }
        "flange" => {
            d.m.notes = "A 200 mm round steel plate, 15 mm thick, with an 80 mm bore under 20 MPa and six 16 mm bolt holes on a 140 mm circle.".into();
            d.m.thickness = 15.0 * mm;
            d.m.grid = 5.0 * mm;
            d.apply(shape(circle(0.0, 0.0, 100.0), false))?;
            let bore = d.apply(shape(circle(0.0, 0.0, 40.0), true))?[0];
            let mut ops = vec![load(edge(bore, 0), LoadKind::Pressure { p: 20e6 })];
            for i in 0..6 {
                let t = i as f64 * std::f64::consts::PI / 3.0;
                let b = d.apply(shape(circle(70.0 * t.cos(), 70.0 * t.sin(), 8.0), true))?[0];
                ops.push(sup(edge(b, 0), SupportKind::Fixed));
            }
            d.apply(Op::Batch(ops))?;
        }
        "vessel" => {
            d.m.notes = "A steel vessel: inside radius 100 mm, wall 10 mm, a 200 mm cylinder closed by a hemispherical head, 2 MPa inside. Drawn as the half-section right of the axis (x is the radius) and cut in half at y = 0 by symmetry. The membrane hoop stress in the cylinder is pr/t ≈ 21 MPa; in the head about half that.".into();
            d.m.grid = 5.0 * mm;
            let q = (std::f64::consts::FRAC_PI_8).tan();
            let s = d.apply(shape(poly(&[[100.0, 0.0], [110.0, 0.0], [110.0, 200.0], [0.0, 310.0], [0.0, 300.0], [100.0, 200.0]], &[(2, q), (4, -q)]), false))?[0];
            d.apply(Op::Batch(vec![sup(edge(s, 0), SupportKind::Symmetry), load(edge(s, 4), LoadKind::Pressure { p: 2e6 }), load(edge(s, 5), LoadKind::Pressure { p: 2e6 })]))?;
        }
        "vessel3d" => {
            d.m.notes = "The steel pressure vessel as a 3D solid: the half-section (inside radius 100 mm, wall 10 mm, a hemispherical head) revolved a quarter turn about the y axis, with symmetry on the two cut faces and the cut at y = 0; 2 MPa inside. The hoop stress in the cylinder is about pr/t ≈ 21 MPa, in the head about half, as in the axisymmetric example.".into();
            d.m.grid = 5.0 * mm;
            d.m.revolve = std::f64::consts::FRAC_PI_2;
            let q = (std::f64::consts::FRAC_PI_8).tan();
            let s = d.apply(shape(poly(&[[100.0, 0.0], [110.0, 0.0], [110.0, 200.0], [0.0, 310.0], [0.0, 300.0], [100.0, 200.0]], &[(2, q), (4, -q)]), false))?[0];
            d.apply(Op::Batch(vec![
                sup(edge(s, 0), SupportKind::Symmetry),
                sup(Target::End { front: false }, SupportKind::Symmetry),
                sup(Target::End { front: true }, SupportKind::Symmetry),
                load(edge(s, 4), LoadKind::Pressure { p: 2e6 }),
                load(edge(s, 5), LoadKind::Pressure { p: 2e6 }),
            ]))?;
            // 5 mm elements in the section, six quadratic layers (15° each) round the quarter turn.
            d.m.mesh.size = 5.0 * mm;
            d.m.mesh.layers = 6;
        }
        "hotplate" => {
            d.m.notes = "A 200 × 60 mm steel plate, 10 mm thick, with a 20 mm hole. The left edge is welded to a cool wall at 20 °C; the right edge is heated to 220 °C. Solve finds the temperatures first, then the stress from the plate's growth held back at the wall and bent round the hole.".into();
            d.m.grid = 5.0 * mm;
            let s = d.apply(shape(poly(&[[0.0, 0.0], [200.0, 0.0], [200.0, 60.0], [0.0, 60.0]], &[]), false))?[0];
            d.apply(shape(circle(100.0, 30.0, 10.0), true))?;
            d.apply(Op::Batch(vec![
                sup(edge(s, 3), SupportKind::Fixed),
                sup(edge(s, 3), SupportKind::Temperature { t: 293.15 }),
                sup(edge(s, 1), SupportKind::Temperature { t: 493.15 }),
            ]))?;
        }
        "fin" => {
            d.m.notes = "An aluminium heat sink: a 60 × 6 mm base held at 80 °C, four 3 mm fins 40 mm tall, cooled by air at 25 °C (h = 25 W/m²·K).".into();
            d.m.grid = 1.0 * mm;
            let mut pts = vec![[0.0, 0.0], [60.0, 0.0]];
            for i in (0..4).rev() {
                let x = 4.5 + i as f64 * 17.0;
                pts.extend([[x + 3.0, 6.0], [x + 3.0, 46.0], [x, 46.0], [x, 6.0]]);
            }
            pts.insert(2, [60.0, 6.0]);
            pts.push([0.0, 6.0]);
            let s = d.apply(shape(poly(&pts, &[]), false))?[0];
            // Aluminium 6061-T6 from the materials library.
            let al = crate::materials::get("al-6061-t6").expect("library").to_material(d.m.materials[0].id);
            let mut ops = vec![Op::Set { item: Item::Material(al) }, sup(edge(s, 0), SupportKind::Temperature { t: 353.15 })];
            for e in 1..pts.len() as u32 {
                ops.push(load(edge(s, e), LoadKind::Convection { h: 25.0, t_inf: 298.15 }));
            }
            d.apply(Op::Batch(ops))?;
        }
        _ => {}
    }
    d.m.check()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_builds_and_is_valid() {
        for e in list() {
            let m = get(e.id).unwrap_or_else(|| panic!("{}", e.id));
            m.check().unwrap();
            assert_eq!(m.analysis, e.analysis);
            if !e.id.starts_with("blank") {
                // Something to hold it and something to load it (temperatures load the heated plate).
                let warmed = m.supports.iter().any(|s| matches!(s.kind, SupportKind::Temperature { .. }));
                assert!(!m.supports.is_empty() && (!m.loads.is_empty() || warmed), "{} has supports and loads", e.id);
            }
        }
        assert!(get("nope").is_none());
        // The heated plate keeps both its fixed support and the wall's temperature on one edge.
        assert_eq!(get("hotplate").unwrap().supports.len(), 3);
        let t = get("truss").unwrap();
        assert_eq!(t.members.len(), 6 + 4 + 5 + 2 + 4);
        assert!(t.members.iter().all(|m| m.kind == MemberKind::Truss));
        // The bracket's polygon runs counter-clockwise and its fillet takes a little area away.
        let b = get("bracket").unwrap();
        let a = b.shapes[0].geom.area();
        let square = 100.0 * 20.0 + 80.0 * 20.0 + 10.0 * 10.0;
        let fillet = 10.0 * 10.0 - std::f64::consts::PI * 100.0 / 4.0;
        assert!((a * 1e6 - (square - 100.0 + fillet)).abs() < 1e-6, "{}", a * 1e6);
    }
}
