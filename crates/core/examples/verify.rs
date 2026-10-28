//! The verification record as Markdown, from the benchmarks this version solves:
//! cargo run -q -p datum-core --example verify > docs/verification/<date>.md
fn main() {
    let rows = datum_core::bench::run();
    println!("# Datum Mesh {}: verification record\n", env!("CARGO_PKG_VERSION"));
    println!("Every row was solved by Datum Mesh's engine and compared with a known answer. The same checks run as tests (`cargo test`, `bench.rs`) and in the app (Help › Verification). SI units.\n");
    println!("| # | Benchmark | Quantity | Datum Mesh | Reference | Error | Source | |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for r in &rows {
        println!("| {} | {} | {} | {:.6e} | {:.6e} | {:.1e} | {} | {} |", r.bench, r.name, r.quantity, r.result, r.reference, r.error, r.source, if r.pass { "pass" } else { "FAIL" });
    }
    println!("\n{} of {} checks pass.", rows.iter().filter(|r| r.pass).count(), rows.len());
    println!("\n## Mesh convergence\n\nThe same model solved on finer meshes: the error falls as the elements get smaller.\n");
    for r in rows.iter().filter(|r| !r.steps.is_empty()) {
        println!("**{}: {}**\n\n| Elements | Error |\n| --- | --- |", r.name, r.quantity);
        for s in &r.steps {
            println!("| {} | {:.2e} |", s[0], s[1]);
        }
        println!();
    }
}
