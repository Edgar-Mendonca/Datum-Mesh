#![recursion_limit = "256"]
//! Writes the engine's real output for the UI tests (tests_ui/fixture.json):
//! cargo run -q -p datum-core --example dump > tests_ui/fixture.json
use datum_core::{bench, examples, materials, mesh, wire};
use serde_json::json;

/// What the shell sends: results with their long arrays packed (`wire`).
fn sent<T: serde::Serialize>(r: &T) -> serde_json::Value {
    wire::encode(serde_json::to_value(r).unwrap())
}

fn main() {
    let out = |id: &str| {
        let m = examples::get(id).expect("example");
        json!({ "m": datum_core::solids::tet::for_window(&m), "path": null, "dirty": false, "undo": false, "redo": false, "created": [], "warnings": [] })
    };
    // A fixed beam with no loads (0.1.1 drew a garbage deflected shape × 2e+299).
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Frame));
    d.apply(datum_core::doc::Op::AddMember { from: [0.0, 0.0], to: [6.0, 0.0], kind: datum_core::model::MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    let id = d.m.nodes[0].id;
    d.apply(datum_core::doc::Op::Add { item: datum_core::doc::Item::Support(datum_core::model::Support { id: 0, at: datum_core::model::Target::Node { id }, kind: datum_core::model::SupportKind::Fixed }) }).unwrap();
    let noload = d.m;
    // A member whose start is off the grid (0.3 m on a 0.5 m grid): a drag must land it on the grid.
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Frame));
    d.apply(datum_core::doc::Op::AddMember { from: [0.3, 0.0], to: [4.0, 0.0], kind: datum_core::model::MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    let id = d.m.nodes[1].id;
    d.apply(datum_core::doc::Op::Add { item: datum_core::doc::Item::Load(datum_core::model::Load { case: 0, id: 0, at: datum_core::model::Target::Node { id }, kind: datum_core::model::LoadKind::Force { fx: 0.0, fy: -1e4, fz: 0.0 } }) }).unwrap();
    let offgrid = d.m;
    // A beam with a point load and a tapering load over part of it (0.10.3).
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Frame));
    d.apply(datum_core::doc::Op::AddMember { from: [0.0, 0.0], to: [8.0, 0.0], kind: datum_core::model::MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    let (n0, n1, mb) = (d.m.nodes[0].id, d.m.nodes[1].id, d.m.members[0].id);
    use datum_core::model::{Dir, Load, LoadKind, Support, SupportKind, Target};
    use datum_core::doc::{Item, Op};
    for (n, kind) in [(n0, SupportKind::Pinned), (n1, SupportKind::RollerX)] {
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: n }, kind }) }).unwrap();
    }
    for kind in [LoadKind::Concentrated { f: -20e3, dir: Dir::Y, at: 2.5 }, LoadKind::Patch { w1: -8e3, w2: -12e3, dir: Dir::Y, from: 4.5, to: 7.5, projected: false }] {
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Member { id: mb }, kind }) }).unwrap();
    }
    let beamloads = d.m;
    // A frame with faults for the model check (0.10.3): a loose node, two nodes 3 mm apart, a
    // piece on its own, two members on the same nodes.
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Frame));
    for (a, b) in [([0.0, 0.0], [0.0, 4.0]), ([0.0, 4.0], [6.0, 4.0]), ([6.0, 4.0], [6.0, 0.0]), ([6.003, 4.0], [9.0, 4.0])] {
        d.apply(Op::AddMember { from: a, to: b, kind: datum_core::model::MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    }
    for p in [[0.0, 0.0], [6.0, 0.0]] {
        let n = d.m.nodes.iter().find(|n| n.x == p[0] && n.y == p[1]).unwrap().id;
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: n }, kind: SupportKind::Fixed }) }).unwrap();
    }
    d.apply(Op::Add { item: Item::Node(datum_core::model::Node { id: 0, x: 3.0, y: 6.5, z: 0.0 }) }).unwrap();
    let mut dup = d.m.members[1].clone();
    dup.id = 900;
    d.m.members.push(dup);
    let checkdemo = d.m;
    // A design member (0.10.4): a beam of four members, 8 m on two supports, restrained at 4 m.
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Frame));
    for k in 0..4 {
        d.apply(Op::AddMember { from: [2.0 * k as f64, 0.0], to: [2.0 * (k + 1) as f64, 0.0], kind: datum_core::model::MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    }
    for x in [0.0, 8.0] {
        let n = d.m.nodes.iter().find(|n| n.x == x).unwrap().id;
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: n }, kind: if x == 0.0 { SupportKind::Pinned } else { SupportKind::RollerX } }) }).unwrap();
    }
    let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
    let dmid = d.apply(Op::FormDesignMember { ids, name: "Beam B1".into() }).unwrap()[0];
    let mut dm = d.m.design_members[0].clone();
    dm.restraints = vec![datum_core::model::Restraint { at: 4.0, holds: datum_core::model::Holds::Both }];
    d.apply(Op::Set { item: Item::DesignMember(dm) }).unwrap();
    for b in d.m.members.iter().map(|b| b.id).collect::<Vec<_>>() {
        d.apply(Op::Add { item: Item::Load(Load { id: 0, case: 0, at: Target::Member { id: b }, kind: LoadKind::Dist { w1: -10e3, w2: -10e3, dir: Dir::Y } }) }).unwrap();
    }
    let n4 = d.m.nodes.iter().find(|n| n.x == 4.0).unwrap().id;
    d.apply(Op::Add { item: Item::Load(Load { id: 0, case: 0, at: Target::Node { id: n4 }, kind: LoadKind::Force { fx: 0.0, fy: -20e3, fz: 0.0 } }) }).unwrap();
    let dmdemo_checks = datum_core::frames::mech::run(&d.m).unwrap();
    let dmdemo_values = datum_core::frames::mech::along(&d.m, None, dmid, 4.0).unwrap();
    let dmdemo_info = datum_core::frames::design::info(&d.m, dmid).unwrap();
    let dmdemo = d.m;
    // Storey drift (0.10.4): a two-storey portal, 6 m wide, storeys of 3.5 m, pushed sideways.
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Frame));
    for (f, t) in [([0.0, 0.0], [0.0, 3.5]), ([0.0, 3.5], [0.0, 7.0]), ([6.0, 0.0], [6.0, 3.5]), ([6.0, 3.5], [6.0, 7.0]), ([0.0, 3.5], [6.0, 3.5]), ([0.0, 7.0], [6.0, 7.0])] {
        d.apply(Op::AddMember { from: f, to: t, kind: datum_core::model::MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    }
    for x in [0.0, 6.0] {
        let n = d.m.nodes.iter().find(|n| n.x == x && n.y == 0.0).unwrap().id;
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: n }, kind: SupportKind::Fixed }) }).unwrap();
    }
    for y in [3.5, 7.0] {
        let n = d.m.nodes.iter().find(|n| n.x == 0.0 && n.y == y).unwrap().id;
        d.apply(Op::Add { item: Item::Load(Load { id: 0, case: 0, at: Target::Node { id: n }, kind: LoadKind::Force { fx: 30e3, fy: 0.0, fz: 0.0 } }) }).unwrap();
    }
    let left: Vec<u32> = d.m.members.iter().filter(|b| d.m.node(b.a).unwrap().x == 0.0 && d.m.node(b.b).unwrap().x == 0.0).map(|b| b.id).collect();
    d.apply(Op::FormDesignMember { ids: left, name: "Column C1".into() }).unwrap();
    let driftdemo_checks = datum_core::frames::mech::run(&d.m).unwrap();
    let driftdemo = d.m;
    // The braced frame with its cases marked for combinations by rule (0.10.3): demonstration
    // factors typed as a user would, not any design code's.
    let mut braced_rule = examples::get("braced").unwrap();
    for c in braced_rule.cases.iter_mut() {
        match c.name.as_str() {
            "Imposed" => { c.kind = datum_core::model::CaseKind::Variable; c.psi0 = 0.7; c.psi2 = 0.3; }
            "Wind" => { c.kind = datum_core::model::CaseKind::Variable; c.psi0 = 0.6; c.psi2 = 0.0; }
            _ => {}
        }
    }
    let rule = datum_core::model::ComboRule { g_unfav: 1.35, g_fav: 1.0, q: 1.5, strength: true, strength_fav: true, service: true, long_term: true };
    let braced_rule_preview = datum_core::cases::by_rule(&braced_rule, &rule).unwrap();
    braced_rule.combo_rule = Some(rule);
    // A part with the sample DXF imported (0.3.5), as the shell sends it back.
    let imp = datum_core::dxf::read(datum_core::dxf::SAMPLE_PUB, None).unwrap();
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Stress));
    let created = d.apply(datum_core::doc::Op::Batch(imp.shapes.into_iter().map(|(geom, hole)| datum_core::doc::Op::Add { item: datum_core::doc::Item::Shape(datum_core::model::Shape { id: 0, name: String::new(), hole, material: 1, geom, plate: None, part: 0 }) }).collect())).unwrap();
    let dxf_out = json!({ "m": d.m, "path": null, "dirty": true, "undo": true, "redo": false, "created": created, "warnings": std::iter::once("Imported 1 shape and 2 holes from plate.dxf.".to_string()).chain(imp.warnings).collect::<Vec<_>>() });
    let out_m = |m: &datum_core::model::Model| json!({ "m": m, "path": null, "dirty": false, "undo": false, "redo": false, "created": [], "warnings": [] });
    let fix = json!({
        "examples_list": examples::list(),
        "frame": out("blank-frame"),
        "portal": out("portal"),
        "truss": out("truss"),
        "bracket": out("bracket"),
        "lug": out("lug"),
        "fin": out("fin"),
        "hotplate": out("hotplate"),
        "vessel": out("vessel"),
        "platform": out("platform"),
        "bracket3d": out("bracket3d"),
        "solid": out("blank-solid"),
        "bracket3d_results": sent(&datum_core::solve(&examples::get("bracket3d").unwrap()).unwrap()),
        "braced": out("braced"),
        "lee": out("lee"),
        "slab": out("slab"),
        "quake": out("quake"),
        "xbrace": out("xbrace"),
        "building": out("building"),
        "tube": out("tube"),
        "tube_results": sent(&datum_core::solve_for(&examples::get("tube").unwrap(), None).unwrap()),
        "yield3d": out("yield3d"),
        "rubber": out("rubber"),
        "elbow": out("elbow"),
        "tubeplates": out("tubeplates"),
        "tubeplates_results": sent(&datum_core::solve_for(&examples::get("tubeplates").unwrap(), None).unwrap()),
        "elbow_results": sent(&datum_core::solve_for(&examples::get("elbow").unwrap(), None).unwrap()),
        "elbowheat": out("elbowheat"),
        "elbowheat_results": sent(&datum_core::solve_for(&examples::get("elbowheat").unwrap(), None).unwrap()),
        "elbow_modes": sent(&datum_core::modes(&examples::get("elbow").unwrap(), 4).unwrap()),
        "rubber_results": sent(&datum_core::solve_for(&examples::get("rubber").unwrap(), None).unwrap()),
        "yield3d_results": sent(&datum_core::solve_for(&examples::get("yield3d").unwrap(), None).unwrap()),
        "building_results": sent(&datum_core::solve_for(&examples::get("building").unwrap(), None).unwrap()),
        "xbrace_results": sent(&datum_core::solve(&examples::get("xbrace").unwrap()).unwrap()),
        "quake_results": sent(&datum_core::solve_for(&examples::get("quake").unwrap(), None).unwrap()),
        "quake_history": sent(&datum_core::solve_for(&{
            let mut m = examples::get("quake").unwrap();
            let st = &mut m.steps[0];
            st.kind = datum_core::model::StepKind::History;
            st.ground = true;
            st.time = vec![[0.0, 0.0], [0.2, 3.0], [0.4, -3.0], [0.6, 2.0], [0.8, 0.0]];
            st.dt = 0.005;
            st.duration = 3.0;
            m
        }, None).unwrap()),
        "slab_results": sent(&datum_core::solve(&examples::get("slab").unwrap()).unwrap()),
        "slab_modes": sent(&datum_core::modes(&examples::get("slab").unwrap(), 4).unwrap()),
        "bend45": out("bend45"),
        "bend45_results": sent(&datum_core::solve_for(&examples::get("bend45").unwrap(), None).unwrap()),
        "lee_results": sent(&datum_core::solve_for(&examples::get("lee").unwrap(), None).unwrap()),
        "braced_results": sent(&datum_core::solve(&examples::get("braced").unwrap()).unwrap()),
        "braced_envelope": datum_core::cases::envelope(&examples::get("braced").unwrap()).unwrap(),
        "sections": datum_core::steel::library(),
        "dxf_import": dxf_out,
        "fin3d": out("fin3d"),
        "fin3d_results": sent(&datum_core::solve(&examples::get("fin3d").unwrap()).unwrap()),
        "bracket3d_modes": sent(&datum_core::modes(&examples::get("bracket3d").unwrap(), 6).unwrap()),
        "vessel3d": out("vessel3d"),
        "vessel3d_results": sent(&datum_core::solve(&examples::get("vessel3d").unwrap()).unwrap()),
        "space": out("blank-space"),
        "platform_results": sent(&datum_core::solve(&examples::get("platform").unwrap()).unwrap()),
        "vessel_results": sent(&datum_core::solve(&examples::get("vessel").unwrap()).unwrap()),
        "hotplate_results": sent(&datum_core::solve(&examples::get("hotplate").unwrap()).unwrap()),
        "portal_results": sent(&datum_core::solve(&examples::get("portal").unwrap()).unwrap()),
        "truss_results": sent(&datum_core::solve(&examples::get("truss").unwrap()).unwrap()),
        "bracket_results": sent(&datum_core::solve(&examples::get("bracket").unwrap()).unwrap()),
        "fin_results": sent(&datum_core::solve(&examples::get("fin").unwrap()).unwrap()),
        "bracket_mesh": sent(&mesh::mesh(&examples::get("bracket").unwrap()).unwrap()),
        "library": materials::library(),
        "portal_modes": sent(&datum_core::modes(&examples::get("portal").unwrap(), 6).unwrap()),
        "portal_buckle": sent(&datum_core::buckling(&examples::get("portal").unwrap(), 6).unwrap()),
        "truss_buckle": sent(&datum_core::buckling(&examples::get("truss").unwrap(), 6).unwrap()),
        "platform_modes": sent(&datum_core::modes(&examples::get("platform").unwrap(), 6).unwrap()),
        "platform_buckle": sent(&datum_core::buckling(&examples::get("platform").unwrap(), 6).unwrap()),
        "bracket_modes": sent(&datum_core::modes(&examples::get("bracket").unwrap(), 6).unwrap()),
        "verification": bench::run(),
        "noload": out_m(&noload),
        "offgrid": out_m(&offgrid),
        "noload_results": sent(&datum_core::solve(&noload).unwrap()),
        "beamloads": out_m(&beamloads),
        "checkdemo": out_m(&checkdemo),
        "dmdemo": out_m(&dmdemo),
        "dmdemo_info": dmdemo_info,
        "dmdemo_checks": dmdemo_checks,
        "dmdemo_values": dmdemo_values,
        "driftdemo_checks": driftdemo_checks,
        "driftdemo": out_m(&driftdemo),
        "section_table_i": datum_core::sections::table(&datum_core::model::SectionShape::I { h: 0.3, b: 0.15, tw: 0.0071, tf: 0.0107, r: 0.0 }).unwrap(),
        "braced_rule": out_m(&braced_rule),
        "braced_rule_preview": braced_rule_preview,
        "checkdemo_findings": datum_core::frames::check::model_check(&checkdemo, 0.005),
        "beamloads_results": sent(&datum_core::solve(&beamloads).unwrap()),
    });
    println!("{}", serde_json::to_string(&fix).unwrap());
}
