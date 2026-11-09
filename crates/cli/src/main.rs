//! `datum-mesh-cli` (0.10.5): Datum Mesh from the command line, and an MCP server for AI agents.
//!
//!   datum-mesh-cli commands                     every command and its parameters
//!   datum-mesh-cli call NAME [JSON] [--file F]  one command (on the model in F), its answer as JSON
//!   datum-mesh-cli run SCRIPT.json              a list of {"command": …, "params": {…}} in turn
//!   datum-mesh-cli solve F.dmesh                the solve's summary
//!   datum-mesh-cli verify                       the verification benchmarks
//!   datum-mesh-cli mcp                          the MCP server on standard input and output

use datum_mesh_cli::{catalogue, mcp_reply, Session};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn print(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn fail(e: &str) -> ! {
    eprintln!("{e}");
    std::process::exit(1);
}

fn with_file(s: &mut Session, args: &[String]) {
    if let Some(k) = args.iter().position(|a| a == "--file") {
        let path = args.get(k + 1).unwrap_or_else(|| fail("--file needs a path"));
        s.run("open", &json!({ "path": path })).unwrap_or_else(|e| fail(&e));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut s = Session::default();
    match args.first().map(String::as_str) {
        Some("commands") => print(&Value::Array(catalogue().iter().map(|c| json!({ "name": c.name, "what": c.what, "params": (c.params)() })).collect())),
        Some("call") => {
            let name = args.get(1).unwrap_or_else(|| fail("call NAME [JSON] [--file F]"));
            let p: Value = match args.get(2).filter(|a| !a.starts_with("--")) {
                Some(t) => serde_json::from_str(t).unwrap_or_else(|e| fail(&format!("The parameters are not JSON: {e}"))),
                None => json!({}),
            };
            with_file(&mut s, &args);
            match s.run(name, &p) {
                Ok(v) => print(&v),
                Err(e) => fail(&e),
            }
        }
        Some("run") => {
            let path = args.get(1).unwrap_or_else(|| fail("run SCRIPT.json"));
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
            let script: Vec<Value> = serde_json::from_str(&text).unwrap_or_else(|e| fail(&format!("{path}: not a list of commands ({e})")));
            for (k, step) in script.iter().enumerate() {
                let name = step["command"].as_str().unwrap_or_else(|| fail(&format!("Step {}: no \"command\"", k + 1)));
                match s.run(name, step.get("params").unwrap_or(&json!({}))) {
                    Ok(v) => print(&json!({ "step": k + 1, "command": name, "result": v })),
                    Err(e) => fail(&format!("Step {} ({name}): {e}", k + 1)),
                }
            }
        }
        Some("solve") => {
            let path = args.get(1).unwrap_or_else(|| fail("solve FILE.dmesh"));
            s.run("open", &json!({ "path": path })).unwrap_or_else(|e| fail(&e));
            print(&s.run("solve", &json!({})).unwrap_or_else(|e| fail(&e)));
        }
        Some("verify") => {
            let v = s.run("verify", &json!({})).unwrap_or_else(|e| fail(&e));
            print(&v);
            if v["failed"].as_array().is_some_and(|f| !f.is_empty()) {
                std::process::exit(2);
            }
        }
        Some("mcp") => {
            let stdin = std::io::stdin();
            let mut out = std::io::stdout();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let reply = match serde_json::from_str::<Value>(&line) {
                    Ok(msg) => mcp_reply(&mut s, &msg),
                    Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
                };
                if let Some(r) = reply {
                    let _ = writeln!(out, "{r}");
                    let _ = out.flush();
                }
            }
        }
        _ => {
            println!("Datum Mesh {} from the command line\n", env!("CARGO_PKG_VERSION"));
            println!("  datum-mesh-cli commands                     every command and its parameters");
            println!("  datum-mesh-cli call NAME [JSON] [--file F]  one command (on the model in F), its answer as JSON");
            println!("  datum-mesh-cli run SCRIPT.json              a list of {{\"command\": …, \"params\": {{…}}}} in turn");
            println!("  datum-mesh-cli solve F.dmesh                the solve's summary");
            println!("  datum-mesh-cli verify                       the verification benchmarks");
            println!("  datum-mesh-cli mcp                          the MCP server for AI agents (standard input and output)");
        }
    }
}
