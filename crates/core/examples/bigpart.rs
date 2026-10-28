//! A large part's results as JSON, for timing the window (not part of the tests).
fn main() {
    let mut m = datum_core::examples::get("bracket").unwrap();
    m.mesh.size = 0.0007;
    let r = datum_core::solve(&m).unwrap();
    println!("{}", serde_json::json!({ "m": m, "r": r }));
}
