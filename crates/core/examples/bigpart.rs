//! A large part's results as JSON, for timing the window by hand (not a test):
//! cargo run --release -p datum-core --example bigpart [wire] > big.json
fn main() {
    let mut m = datum_core::examples::get("bracket").unwrap();
    m.mesh.size = 0.0007;
    let r = serde_json::to_value(datum_core::solve(&m).unwrap()).unwrap();
    let r = if std::env::args().any(|a| a == "wire") { datum_core::wire::encode(r) } else { r };
    println!("{}", serde_json::json!({ "m": m, "r": r }));
}
