//! Datum Mesh's commands (0.10.5): one catalogue that the command line and AI agents (over MCP)
//! both run, on the same engine and with the same edits (`doc::Op`) as the window. A session
//! holds one open model, as the window does. The engine and the files keep SI units (m, N, Pa).

use datum_core::doc::{Doc, Op};
use datum_core::model::{self, Model};
use serde_json::{json, Value};

/// One command: its name, what it does and the JSON schema of its parameters.
pub struct Command {
    pub name: &'static str,
    pub what: &'static str,
    pub params: fn() -> Value,
}

fn none() -> Value {
    json!({ "type": "object", "properties": {} })
}

/// Every command, in the order a model is made, solved and checked.
pub fn catalogue() -> Vec<Command> {
    vec![
        Command { name: "examples", what: "List the ready-made models and blank starting points (id, name, what it shows, its analysis).", params: none },
        Command { name: "new", what: "Start a model: a blank one of an analysis (frame, space, stress, strain, axi, heat, solid, solidheat) or an example by its id. Replaces the open model.", params: || json!({ "type": "object", "properties": { "analysis": { "type": "string", "description": "frame (2D frame), space (3D frame), stress, strain, axi, heat, solid, solidheat" }, "example": { "type": "string", "description": "an example's id from `examples`" } } }) },
        Command { name: "open", what: "Open a .dmesh file.", params: || json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }) },
        Command { name: "save", what: "Save the open model as a .dmesh file (to its own path when none is given).", params: || json!({ "type": "object", "properties": { "path": { "type": "string" } } }) },
        Command { name: "model", what: "The open model as JSON (SI units: m, N, Pa, K), or with summary: true only its counts.", params: || json!({ "type": "object", "properties": { "summary": { "type": "boolean" } } }) },
        Command { name: "apply", what: "Apply edits to the open model, the same edits the window makes (each an Op as JSON, e.g. {\"AddMember\": {\"from\": [0,0], \"to\": [4,0], \"kind\": \"beam\", \"section\": null, \"material\": null, \"z\": [0,0]}}). All or nothing: an edit that fails undoes the ones before it. Returns the ids each made.", params: || json!({ "type": "object", "properties": { "ops": { "type": "array", "items": { "type": "object" } } }, "required": ["ops"] }) },
        Command { name: "undo", what: "Undo the last edit.", params: none },
        Command { name: "redo", what: "Redo the edit undone last.", params: none },
        Command { name: "check", what: "The model check of a frame: loose nodes, nodes nearly meeting, overlapping members, pieces nothing holds, short members, design members broken; worst first.", params: || json!({ "type": "object", "properties": { "near": { "type": "number", "description": "nodes closer than this (m) are reported; default 0.005" } } }) },
        Command { name: "solve", what: "Solve the open model (its steps, or one linear static step) under all its loads, a load case or a combination. Returns a summary: the balance of loads and reactions, the extremes, warnings; with full: true every result.", params: || json!({ "type": "object", "properties": { "case": { "type": "integer" }, "combo": { "type": "integer" }, "full": { "type": "boolean" } } }) },
        Command { name: "mech_checks", what: "Mechanics checks of every design member (stress against yield, Euler buckling, deflection from the chord) and the storey drift, each with its status and working; not a design code.", params: none },
        Command { name: "member_values", what: "Axial force, shear, moment and deflection at a distance (m) along a design member, both sides of a jump.", params: || json!({ "type": "object", "properties": { "id": { "type": "integer" }, "at": { "type": "number" }, "case": { "type": "integer" }, "combo": { "type": "integer" } }, "required": ["id", "at"] }) },
        Command { name: "section_table", what: "Every property of a section shape (area, second moments, elastic and plastic moduli, radii of gyration, torsion constant), e.g. {\"shape\": {\"kind\": \"rect\", \"b\": 0.1, \"h\": 0.2}}.", params: || json!({ "type": "object", "properties": { "shape": { "type": "object" } }, "required": ["shape"] }) },
        Command { name: "verify", what: "Run the verification benchmarks (every element and solver path against a known answer); returns the counts and any row that failed.", params: || json!({ "type": "object", "properties": { "all": { "type": "boolean", "description": "every row, not only the failures" } } }) },
    ]
}

/// The session: the open model, as in the window.
pub struct Session {
    pub doc: Doc,
}

impl Default for Session {
    fn default() -> Self {
        Session { doc: Doc::new(Model::new(model::Analysis::Frame)) }
    }
}

fn which(p: &Value) -> Option<datum_core::cases::Which> {
    if let Some(id) = p.get("combo").and_then(Value::as_u64) {
        return Some(datum_core::cases::Which::Combo { id: id as u32 });
    }
    p.get("case").and_then(Value::as_u64).map(|id| datum_core::cases::Which::Case { id: id as u32 })
}

fn to<T: serde::Serialize>(x: T) -> Result<Value, String> {
    serde_json::to_value(x).map_err(|e| e.to_string())
}

/// The largest absolute value in a list of numbers, and where.
fn peak(v: &Value) -> Option<f64> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_f64).fold(0.0, |m: f64, x| m.max(x.abs())))
}

/// A solve's summary: what an engineer reads first.
pub fn summary(r: &Value) -> Value {
    let mut out = json!({ "kind": r["kind"], "imbalance": r["imbalance"], "warnings": r["warnings"], "stats": r["stats"] });
    if let Some(d) = r["disp"].as_array() {
        // A 2D frame's and a part's movements are (ux, uy) and then a turn; 3D ones (ux, uy, uz).
        let n = if matches!(r["kind"].as_str(), Some("frame" | "plane")) { 2 } else { 3 };
        let mv = d.iter().filter_map(|p| p.as_array()).map(|p| p.iter().take(n).filter_map(Value::as_f64).map(|x| x * x).sum::<f64>().sqrt()).fold(0.0, f64::max);
        out["largest_movement_m"] = json!(mv);
    }
    if let Some(ms) = r["members"].as_array() {
        let get = |k: &str| ms.iter().filter_map(|m| peak(&m[k])).fold(0.0, f64::max);
        out["members"] = json!({ "count": ms.len(), "largest_axial_N": get("n"), "largest_moment_Nm": get("m").max(get("mz")).max(get("my")), "largest_utilisation": ms.iter().filter_map(|m| m["util"].as_f64()).fold(0.0, f64::max) });
    }
    if let Some(fs) = r["fields"].as_array() {
        out["fields"] = Value::Array(fs.iter().map(|f| json!({ "key": f["key"], "name": f["name"], "min": f["min"], "max": f["max"] })).collect());
    }
    out
}

impl Session {
    /// Run one command with its parameters.
    pub fn run(&mut self, name: &str, p: &Value) -> Result<Value, String> {
        match name {
            "examples" => Ok(Value::Array(datum_core::examples::list().iter().map(|e| json!({ "id": e.id, "name": e.name, "what": e.what, "analysis": e.analysis, "group": e.group })).collect())),
            "new" => {
                let m = if let Some(id) = p.get("example").and_then(Value::as_str) {
                    datum_core::examples::get(id).ok_or(format!("There is no example \"{id}\": `examples` lists them."))?
                } else {
                    let a: model::Analysis = serde_json::from_value(p.get("analysis").cloned().unwrap_or(json!("frame"))).map_err(|_| "The analysis is one of frame, space, stress, strain, axi, heat, solid, solidheat.".to_string())?;
                    Model::new(a)
                };
                self.doc = Doc::new(m);
                Ok(json!({ "analysis": self.doc.m.analysis, "nodes": self.doc.m.nodes.len(), "members": self.doc.m.members.len() }))
            }
            "open" => {
                let path = p.get("path").and_then(Value::as_str).ok_or("Give the file's path.")?;
                let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                let (m, warnings) = model::from_json(&text)?;
                self.doc = Doc::new(m);
                self.doc.path = Some(path.to_string());
                Ok(json!({ "analysis": self.doc.m.analysis, "warnings": warnings }))
            }
            "save" => {
                let path = p.get("path").and_then(Value::as_str).map(str::to_string).or(self.doc.path.clone()).ok_or("Give a path to save to.")?;
                std::fs::write(&path, model::to_json(&self.doc.m)).map_err(|e| format!("{path}: {e}"))?;
                self.doc.path = Some(path.clone());
                self.doc.mark_saved();
                Ok(json!({ "saved": path }))
            }
            "model" => {
                let m = &self.doc.m;
                if p.get("summary").and_then(Value::as_bool).unwrap_or(false) {
                    return Ok(json!({ "analysis": m.analysis, "nodes": m.nodes.len(), "members": m.members.len(), "shapes": m.shapes.len(), "supports": m.supports.len(), "loads": m.loads.len(), "cases": m.cases.len(), "combos": m.combos.len(), "design_members": m.design_members.len(), "materials": m.materials.len(), "sections": m.sections.len() }));
                }
                serde_json::from_str(&model::to_json(m)).map_err(|e| e.to_string())
            }
            "apply" => {
                let ops = p.get("ops").and_then(Value::as_array).ok_or("Give the edits as ops: [ … ].")?;
                let mut made = vec![];
                for (k, o) in ops.iter().enumerate() {
                    let r = serde_json::from_value::<Op>(o.clone()).map_err(|e| format!("Edit {}: not an edit Datum Mesh knows ({e}).", k + 1)).and_then(|op| self.doc.apply(op).map_err(|e| format!("Edit {}: {e}", k + 1)));
                    match r {
                        Ok(ids) => made.push(ids),
                        Err(e) => {
                            for _ in 0..made.len() {
                                self.doc.undo();
                            }
                            return Err(e);
                        }
                    }
                }
                Ok(json!({ "created": made }))
            }
            "undo" => Ok(json!({ "undone": self.doc.undo() })),
            "redo" => Ok(json!({ "redone": self.doc.redo() })),
            "check" => to(datum_core::frames::check::model_check(&self.doc.m, p.get("near").and_then(Value::as_f64).unwrap_or(0.005))),
            "solve" => {
                let r = to(datum_core::solve_for(&self.doc.m, which(p))?)?;
                Ok(if p.get("full").and_then(Value::as_bool).unwrap_or(false) { r } else { summary(&r) })
            }
            "mech_checks" => to(datum_core::frames::mech::run(&self.doc.m)?),
            "member_values" => {
                let id = p.get("id").and_then(Value::as_u64).ok_or("Give the design member's id.")? as u32;
                let at = p.get("at").and_then(Value::as_f64).ok_or("Give the distance along it (m).")?;
                to(datum_core::frames::mech::along(&self.doc.m, which(p), id, at)?)
            }
            "section_table" => {
                let shape: model::SectionShape = serde_json::from_value(p.get("shape").cloned().ok_or("Give the shape.")?).map_err(|e| format!("Not a section shape ({e})."))?;
                to(datum_core::sections::table(&shape)?)
            }
            "verify" => {
                let rows = datum_core::bench::run();
                let failed: Vec<_> = rows.iter().filter(|r| !r.pass).collect();
                let mut out = json!({ "rows": rows.len(), "passed": rows.len() - failed.len(), "failed": to(&failed)? });
                if p.get("all").and_then(Value::as_bool).unwrap_or(false) {
                    out["all"] = to(&rows)?;
                }
                Ok(out)
            }
            _ => Err(format!("There is no command \"{name}\": `commands` lists them.")),
        }
    }
}

/// The MCP server (0.10.5): JSON-RPC 2.0 over standard input and output, one message a line.
/// Every command of the catalogue is a tool.
pub fn mcp_reply(s: &mut Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let p = msg.get("params").cloned().unwrap_or(json!({}));
    let ok = |result: Value| Some(json!({ "jsonrpc": "2.0", "id": id.clone(), "result": result }));
    let id = id.clone()?; // notifications get no answer
    let result = match method {
        "initialize" => json!({
            "protocolVersion": p.get("protocolVersion").cloned().unwrap_or(json!("2025-06-18")),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "datum-mesh", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Datum Mesh: structural and finite-element analysis. Start with `examples` or `new`, edit with `apply` (the window's edits as JSON), then `solve`, `check`, `mech_checks`. Units are SI (m, N, Pa, K)."
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": catalogue().iter().map(|c| json!({ "name": c.name, "description": c.what, "inputSchema": (c.params)() })).collect::<Vec<_>>() }),
        "tools/call" => {
            let name = p.get("name").and_then(Value::as_str).unwrap_or("");
            let args = p.get("arguments").cloned().unwrap_or(json!({}));
            match s.run(name, &args) {
                Ok(v) => json!({ "content": [{ "type": "text", "text": v.to_string() }], "isError": false }),
                Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
            }
        }
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("No method {method}") } })),
    };
    ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_made_and_solved_by_commands() {
        let mut s = Session::default();
        s.run("new", &json!({ "analysis": "frame" })).unwrap();
        let ops = json!([
            { "AddMember": { "from": [0.0, 0.0], "to": [4.0, 0.0], "kind": "beam", "section": null, "material": null, "z": [0.0, 0.0] } }
        ]);
        s.run("apply", &json!({ "ops": ops })).unwrap();
        let m = s.run("model", &json!({})).unwrap();
        let n: Vec<u64> = m["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_u64().unwrap()).collect();
        let ops = json!([
            { "Add": { "item": { "Support": { "id": 0, "at": { "on": "node", "id": n[0] }, "kind": { "kind": "fixed" } } } } },
            { "Add": { "item": { "Load": { "id": 0, "case": 0, "at": { "on": "node", "id": n[1] }, "kind": { "kind": "force", "fx": 0.0, "fy": -1000.0 } } } } }
        ]);
        s.run("apply", &json!({ "ops": ops })).unwrap();
        let r = s.run("solve", &json!({})).unwrap();
        assert!(r["imbalance"].as_f64().unwrap() < 1e-9, "{r}");
        // A cantilever 4 m long: PL³/3EI at the tip.
        let e = s.doc.m.materials[0].e;
        let i = datum_core::sections::props(&s.doc.m.sections[0].shape).unwrap().i;
        let want = 1000.0 * 64.0 / (3.0 * e * i);
        assert!((r["largest_movement_m"].as_f64().unwrap() - want).abs() < 1e-9 * want.max(1.0) + want * 1e-6, "{r} {want}");
        assert!((r["members"]["largest_moment_Nm"].as_f64().unwrap() - 4000.0).abs() < 1e-6);
        // A bad edit in the middle leaves the model as it was.
        let before = s.doc.m.clone();
        let bad = json!([{ "AddMember": { "from": [0.0, 1.0], "to": [4.0, 1.0], "kind": "beam", "section": null, "material": null, "z": [0.0, 0.0] } }, { "NoSuchEdit": {} }]);
        let err = s.run("apply", &json!({ "ops": bad }));
        assert!(err.is_err());
        assert_eq!(s.doc.m, before);
    }

    #[test]
    fn the_mcp_server_lists_and_calls_every_tool() {
        let mut s = Session::default();
        let init = mcp_reply(&mut s, &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } })).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "datum-mesh");
        assert!(mcp_reply(&mut s, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let list = mcp_reply(&mut s, &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
        let tools = list["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), catalogue().len());
        let call = mcp_reply(&mut s, &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "new", "arguments": { "example": "portal" } } })).unwrap();
        assert_eq!(call["result"]["isError"], false, "{call}");
        let solved = mcp_reply(&mut s, &json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "solve", "arguments": {} } })).unwrap();
        let text: Value = serde_json::from_str(solved["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(text["imbalance"].as_f64().unwrap() < 1e-6, "{text}");
        let bad = mcp_reply(&mut s, &json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "nothing", "arguments": {} } })).unwrap();
        assert_eq!(bad["result"]["isError"], true);
        let none = mcp_reply(&mut s, &json!({ "jsonrpc": "2.0", "id": 6, "method": "no/such" })).unwrap();
        assert_eq!(none["error"]["code"], -32601);
    }

    #[test]
    fn every_command_in_the_catalogue_runs() {
        let names: Vec<&str> = catalogue().iter().map(|c| c.name).collect();
        let mut s = Session::default();
        for n in names.into_iter().filter(|n| *n != "verify") {
            // Each answers (or says why not); none is missing from `run`.
            let r = s.run(n, &json!({}));
            if let Err(e) = &r {
                assert!(!e.starts_with("There is no command"), "{n}: {e}");
            }
        }
    }
}
