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
        e("portal", "Portal frame", "A pitched portal frame with wind on one side and load on the roof.", Analysis::Frame),
        e("truss", "Roof truss", "A 12 m roof truss of pinned bars carrying purlin loads.", Analysis::Frame),
        e("lug", "Lifting lug", "A plate with a pin hole, pulled through the pin.", Analysis::Stress),
        e("bracket", "L-bracket", "An L-bracket with a fillet in the corner, where the stress gathers.", Analysis::Stress),
        e("flange", "Flange plate", "A round plate with a pressurised bore and six bolt holes.", Analysis::Stress),
        e("fin", "Heat sink", "A heat sink with four fins cooled by air.", Analysis::Heat),
    ]
}

fn sup(at: Target, kind: SupportKind) -> Op {
    Op::Add { item: Item::Support(Support { id: 0, at, kind }) }
}
fn load(at: Target, kind: LoadKind) -> Op {
    Op::Add { item: Item::Load(Load { id: 0, at, kind }) }
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
        "portal" => {
            d.m.notes = "Span 12 m, eaves 5 m, ridge 6.5 m; fixed bases. Wind 2 kN/m on the left column, 6 kN/m down on the rafters.".into();
            let p = [[0.0, 0.0], [0.0, 5.0], [6.0, 6.5], [12.0, 5.0], [12.0, 0.0]];
            for w in p.windows(2) {
                d.apply(Op::AddMember { from: w[0], to: w[1], kind: MemberKind::Beam, section: None, material: None })?;
            }
            let mut ops = vec![sup(node(d, 0.0, 0.0), SupportKind::Fixed), sup(node(d, 12.0, 0.0), SupportKind::Fixed)];
            ops.push(load(member(d, p[0], p[1]), LoadKind::Dist { w1: 2e3, w2: 2e3, dir: Dir::X }));
            ops.push(load(member(d, p[1], p[2]), LoadKind::Dist { w1: -6e3, w2: -6e3, dir: Dir::Y }));
            ops.push(load(member(d, p[2], p[3]), LoadKind::Dist { w1: -6e3, w2: -6e3, dir: Dir::Y }));
            d.apply(Op::Batch(ops))?;
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
                d.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Truss, section: None, material: None })?;
            }
            // Truss bars: a 100 × 100 × 10 angle.
            let s = d.m.sections[0].id;
            let mut sec = d.m.sections[0].clone();
            sec.name = "Angle 100 × 100 × 10".into();
            sec.shape = crate::model::SectionShape::Angle { h: 0.1, b: 0.1, t: 0.01 };
            let mut ops = vec![Op::Set { item: Item::Section(sec) }, sup(node(d, 0.0, 0.0), SupportKind::Pinned), sup(node(d, span, 0.0), SupportKind::RollerX)];
            for i in 1..n {
                ops.push(load(node(d, i as f64 * dx, h), LoadKind::Force { fx: 0.0, fy: -10e3 }));
            }
            d.apply(Op::Batch(ops))?;
            debug_assert!(d.m.members.iter().all(|m| m.section == s));
        }
        "lug" => {
            d.m.notes = "A 20 mm steel plate, 120 mm wide with a rounded top and a 40 mm pin hole; the base is welded on and the pin pulls 60 kN upwards.".into();
            d.m.thickness = 20.0 * mm;
            d.m.grid = 5.0 * mm;
            let s = d.apply(shape(poly(&[[0.0, 0.0], [120.0, 0.0], [120.0, 120.0], [0.0, 120.0]], &[(2, 1.0)]), false))?[0];
            let h = d.apply(shape(circle(60.0, 120.0, 20.0), true))?[0];
            d.apply(Op::Batch(vec![sup(edge(s, 0), SupportKind::Fixed), load(edge(h, 0), LoadKind::Traction { fx: 0.0, fy: 60e3, total: true })]))?;
        }
        "bracket" => {
            d.m.notes = "A 10 mm steel L-bracket, legs 100 mm long and 20 mm thick, with a 10 mm fillet in the inside corner. The top is bolted to a wall; 1.5 kN pulls down on the free end.".into();
            d.m.grid = 5.0 * mm;
            let s = d.apply(shape(poly(&[[0.0, 0.0], [100.0, 0.0], [100.0, 20.0], [30.0, 20.0], [20.0, 30.0], [20.0, 100.0], [0.0, 100.0]], &[(3, -QUARTER)]), false))?[0];
            d.apply(Op::Batch(vec![sup(edge(s, 5), SupportKind::Fixed), load(edge(s, 1), LoadKind::Traction { fx: 0.0, fy: -1.5e3, total: true })]))?;
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
            // Aluminium: conductivity 167 W/m·K.
            let mut al = d.m.materials[0].clone();
            al.name = "Aluminium 6061-T6".into();
            (al.e, al.nu, al.rho, al.fy, al.k, al.alpha) = (69e9, 0.33, 2700.0, 276e6, 167.0, 23.6e-6);
            al.source = "Typical handbook values (ASM)".into();
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
                assert!(!m.supports.is_empty() && !m.loads.is_empty(), "{} has supports and loads", e.id);
            }
        }
        assert!(get("nope").is_none());
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
