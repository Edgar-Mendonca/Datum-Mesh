//! Writes the engine's real output for the UI tests (tests_ui/fixture.json):
//! cargo run -q -p datum-core --example dump > tests_ui/fixture.json
use datum_core::{bench, examples, frame};
use serde_json::json;

fn main() {
    let out = |id: &str| {
        let m = examples::get(id).expect("example");
        json!({ "m": m, "path": null, "dirty": false, "undo": false, "redo": false, "created": [], "warnings": [] })
    };
    let fix = json!({
        "examples_list": examples::list(),
        "frame": out("blank-frame"),
        "portal": out("portal"),
        "truss": out("truss"),
        "bracket": out("bracket"),
        "lug": out("lug"),
        "fin": out("fin"),
        "portal_results": frame::solve(&examples::get("portal").unwrap()).unwrap(),
        "truss_results": frame::solve(&examples::get("truss").unwrap()).unwrap(),
        "verification": bench::run(),
    });
    println!("{}", serde_json::to_string(&fix).unwrap());
}
