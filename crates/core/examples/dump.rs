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
        json!({ "m": m, "path": null, "dirty": false, "undo": false, "redo": false, "created": [], "warnings": [] })
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
    d.apply(datum_core::doc::Op::Add { item: datum_core::doc::Item::Load(datum_core::model::Load { id: 0, at: datum_core::model::Target::Node { id }, kind: datum_core::model::LoadKind::Force { fx: 0.0, fy: -1e4, fz: 0.0 } }) }).unwrap();
    let offgrid = d.m;
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
        "portal_buckle": sent(&datum_core::Modes::Buckle(datum_core::buckling(&examples::get("portal").unwrap(), 6).unwrap())),
        "truss_buckle": sent(&datum_core::Modes::Buckle(datum_core::buckling(&examples::get("truss").unwrap(), 6).unwrap())),
        "bracket_modes": sent(&datum_core::modes(&examples::get("bracket").unwrap(), 6).unwrap()),
        "verification": bench::run(),
        "noload": out_m(&noload),
        "offgrid": out_m(&offgrid),
        "noload_results": sent(&datum_core::solve(&noload).unwrap()),
    });
    println!("{}", serde_json::to_string(&fix).unwrap());
}
