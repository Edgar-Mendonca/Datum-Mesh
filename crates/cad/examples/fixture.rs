//! Scenes from the real geometry engine for the window's checks (tests_ui): a sketch with each
//! way of drawing, the sketch finished, a block extruded from it, and a revolved ring.
//! cargo run -q -p datum-cad --example fixture > tests_ui/cad.json
use serde_json::{json, Value};

fn main() {
    let mut out = serde_json::Map::new();
    let mut c = datum_cad::Cad::new();
    let run = |c: &mut datum_cad::Cad, id: &str, p: Value| c.run(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"));
    out.insert("empty".into(), serde_json::to_value(c.scene().expect("scene")).expect("json"));
    run(&mut c, "sketch.create", json!({"plane": "XY"}));
    out.insert("sketch".into(), serde_json::to_value(c.scene().expect("scene")).expect("json"));
    run(&mut c, "sketch.rectangle.center", json!({"center": [0, 0], "corner": [60, 40]}));
    run(&mut c, "sketch.circle.center", json!({"center": [-35, 0], "radius": 10}));
    run(&mut c, "sketch.polygon.inscribed", json!({"center": [30, 0], "radius": 14, "sides": 6, "angle": 0}));
    run(&mut c, "sketch.slot.center_to_center", json!({"p0": [-10, 25], "p1": [20, 25], "width": 8}));
    out.insert("drawn".into(), serde_json::to_value(c.scene().expect("scene")).expect("json"));
    run(&mut c, "sketch.finish", json!({}));
    out.insert("finished".into(), serde_json::to_value(c.scene().expect("scene")).expect("json"));
    run(&mut c, "solid.extrude", json!({"distance": 15.0}));
    out.insert("extruded".into(), serde_json::to_value(c.scene().expect("scene")).expect("json"));
    let ex = c.scene().expect("scene").features[1].id;
    out.insert("inspect_extrude".into(), run(&mut c, "timeline.inspect", json!({"feature": ex})));
    let mut r = datum_cad::Cad::new();
    run(&mut r, "sketch.create", json!({"plane": "XZ"}));
    run(&mut r, "sketch.rectangle.two_point", json!({"p0": [30, 0], "p1": [45, 50]}));
    run(&mut r, "sketch.finish", json!({}));
    run(&mut r, "solid.revolve", json!({"axis": "y", "angle": 360}));
    out.insert("revolved".into(), serde_json::to_value(r.scene().expect("scene")).expect("json"));
    println!("{}", Value::Object(out));
}
