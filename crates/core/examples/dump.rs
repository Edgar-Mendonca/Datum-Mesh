//! Writes the engine's real output for the UI tests (tests_ui/fixture.json):
//! cargo run -q -p datum-core --example dump > tests_ui/fixture.json
use datum_core::{bench, examples, materials, mesh};
use serde_json::json;

fn main() {
    let out = |id: &str| {
        let m = examples::get(id).expect("example");
        json!({ "m": m, "path": null, "dirty": false, "undo": false, "redo": false, "created": [], "warnings": [] })
    };
    // A fixed beam with no loads (0.1.1 drew a garbage deflected shape × 2e+299).
    let mut d = datum_core::doc::Doc::new(datum_core::model::Model::new(datum_core::model::Analysis::Frame));
    d.apply(datum_core::doc::Op::AddMember { from: [0.0, 0.0], to: [6.0, 0.0], kind: datum_core::model::MemberKind::Beam, section: None, material: None }).unwrap();
    let id = d.m.nodes[0].id;
    d.apply(datum_core::doc::Op::Add { item: datum_core::doc::Item::Support(datum_core::model::Support { id: 0, at: datum_core::model::Target::Node { id }, kind: datum_core::model::SupportKind::Fixed }) }).unwrap();
    let noload = d.m;
    let out_m = |m: &datum_core::model::Model| json!({ "m": m, "path": null, "dirty": false, "undo": false, "redo": false, "created": [], "warnings": [] });
    let fix = json!({
        "examples_list": examples::list(),
        "frame": out("blank-frame"),
        "portal": out("portal"),
        "truss": out("truss"),
        "bracket": out("bracket"),
        "lug": out("lug"),
        "fin": out("fin"),
        "portal_results": datum_core::solve(&examples::get("portal").unwrap()).unwrap(),
        "truss_results": datum_core::solve(&examples::get("truss").unwrap()).unwrap(),
        "bracket_results": datum_core::solve(&examples::get("bracket").unwrap()).unwrap(),
        "fin_results": datum_core::solve(&examples::get("fin").unwrap()).unwrap(),
        "bracket_mesh": mesh::mesh(&examples::get("bracket").unwrap()).unwrap(),
        "library": materials::library(),
        "verification": bench::run(),
        "noload": out_m(&noload),
        "noload_results": datum_core::solve(&noload).unwrap(),
    });
    println!("{}", serde_json::to_string(&fix).unwrap());
}
