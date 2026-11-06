//! Lists the geometry engine's commands with their parameter schemas (for development).
fn main() {
    let mut s = solidcraft_engine::Session::new(solidcraft_doc::Document::default());
    let v = s.execute("engine.commands", &serde_json::json!({})).expect("commands");
    println!("{}", serde_json::to_string_pretty(&v).expect("json"));
}
