//! Datum Link (0.5.4): the small JSON packets the Datum apps send each other (docs/LINKS.md).
//! One envelope for all eight apps; SI units always. Datum Mesh writes tables (to Datum Plot) and
//! values (to Datum Calc), and reads materials (from Datum Atlas). Everything read is checked:
//! a packet from another program is data, and a bad one is refused with a reason.

use crate::model::Material;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The apps of the family, by their link names, with their window names.
pub const APPS: [(&str, &str); 8] = [("plan", "Datum Plan"), ("plot", "Datum Plot"), ("atlas", "Datum Atlas"), ("sim", "Datum Sim"), ("calc", "Datum Calc"), ("mesh", "Datum Mesh"), ("pipe", "Datum Pipe"), ("schema", "Datum Schema")];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub unit: String,
}

/// A table for Datum Plot: named columns with SI units, rows of numbers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<f64>>,
}

/// A value for Datum Calc: a name, a number in SI, its unit and a note.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quantity {
    pub name: String,
    pub value: f64,
    pub unit: String,
    #[serde(default)]
    pub note: String,
}

/// What a packet carries, as Datum Mesh sends it.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "lowercase")]
pub enum Outgoing {
    Table(Table),
    Value(Vec<Quantity>),
}

/// The packet: the envelope and its data, ready to write to a `.dlink` file.
pub fn envelope(to: &str, title: &str, file: &str, what: &Outgoing, made: &str) -> Result<String, String> {
    if !APPS.iter().any(|a| a.0 == to) || to == "mesh" {
        return Err(format!("There is no Datum app called “{to}” to send to."));
    }
    let (kind, data) = match what {
        Outgoing::Table(t) => {
            if t.columns.is_empty() || t.rows.is_empty() {
                return Err("The table is empty: there is nothing to send.".into());
            }
            if t.rows.iter().any(|r| r.len() != t.columns.len() || r.iter().any(|v| !v.is_finite())) {
                return Err("The table has a row of the wrong length or a value that is not a number.".into());
            }
            ("table", serde_json::to_value(t).map_err(|e| e.to_string())?)
        }
        Outgoing::Value(q) => {
            if q.is_empty() || q.iter().any(|x| !x.value.is_finite() || x.name.trim().is_empty()) {
                return Err("A value to send has no name or is not a number.".into());
            }
            ("value", serde_json::to_value(q).map_err(|e| e.to_string())?)
        }
    };
    let p = json!({ "datum_link": 1, "from": { "app": "mesh", "version": env!("CARGO_PKG_VERSION"), "file": file }, "to": to, "kind": kind, "title": title, "made": made, "reply": Value::Null, "data": data });
    serde_json::to_string_pretty(&p).map_err(|e| e.to_string())
}

/// What a packet sent to Datum Mesh brings in.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// A material (from Datum Atlas), its values checked and in SI.
    Material(Material),
}

/// Read a packet: the envelope, then the kinds Datum Mesh understands.
pub fn read(text: &str) -> Result<(String, Incoming), String> {
    let v: Value = serde_json::from_str(text).map_err(|_| "This is not a Datum link file (it is not readable JSON).".to_string())?;
    if v.get("datum_link").and_then(Value::as_u64) != Some(1) {
        return Err("This is not a Datum link file, or it comes from a newer version.".into());
    }
    let from = v.pointer("/from/app").and_then(Value::as_str).unwrap_or("");
    let who = APPS.iter().find(|a| a.0 == from).map_or("another program", |a| a.1).to_string();
    let data = v.get("data").ok_or("The link file has no data.")?;
    match v.get("kind").and_then(Value::as_str) {
        Some("material") => {
            let f = |k: &str| data.get(k).and_then(Value::as_f64);
            let name: String = data.get("name").and_then(Value::as_str).unwrap_or("Material from Datum Atlas").chars().filter(|c| !c.is_control()).take(80).collect();
            let (e, nu) = (f("e").ok_or("The material has no Young's modulus (e).")?, f("nu").unwrap_or(0.3));
            if !(e > 0.0 && e < 2e12) {
                return Err(format!("The material's Young's modulus ({e} Pa) is not a plausible value."));
            }
            if !(nu > -1.0 && nu < 0.5) {
                return Err(format!("The material's Poisson's ratio ({nu}) must be between −1 and 0.5."));
            }
            let pos = |k: &str| f(k).filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0);
            let source = data.get("source").and_then(Value::as_str).unwrap_or("").chars().filter(|c| !c.is_control()).take(300).collect::<String>();
            let m = Material { id: 0, name, e, nu, rho: pos("rho"), fy: pos("fy"), k: pos("k"), alpha: f("alpha").filter(|v| v.is_finite() && v.abs() < 1e-3).unwrap_or(0.0), source: if source.is_empty() { format!("From {who}") } else { source }, hardening: 0.0 };
            Ok((who, Incoming::Material(m)))
        }
        Some(k) => Err(format!("Datum Mesh cannot use a “{k}” from {who} yet; it reads materials (from Datum Atlas).")),
        None => Err("The link file does not say what it carries.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_goes_out_in_the_family_envelope() {
        let t = Outgoing::Table(Table { columns: vec![Column { name: "Load factor".into(), unit: "".into() }, Column { name: "Largest movement".into(), unit: "m".into() }], rows: vec![vec![0.0, 0.0], vec![0.5, 0.001]] });
        let s = envelope("plot", "Load against movement", "Tube.dmesh", &t, "2026-11-01T12:00:00Z").unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["datum_link"], 1);
        assert_eq!(v["kind"], "table");
        assert_eq!(v["from"]["app"], "mesh");
        assert_eq!(v["data"]["rows"][1][1], 0.001);
        // Bad tables and unknown apps are refused.
        assert!(envelope("plot", "", "", &Outgoing::Table(Table { columns: vec![], rows: vec![] }), "").is_err());
        assert!(envelope("plot", "", "", &Outgoing::Table(Table { columns: vec![Column { name: "x".into(), unit: "".into() }], rows: vec![vec![f64::NAN]] }), "").is_err());
        assert!(envelope("excel", "", "", &t, "").is_err());
    }

    #[test]
    fn values_go_to_calc() {
        let q = Outgoing::Value(vec![Quantity { name: "sigma_max".into(), value: 2.1e8, unit: "Pa".into(), note: "Largest von Mises".into() }]);
        let v: Value = serde_json::from_str(&envelope("calc", "Bracket", "b.dmesh", &q, "").unwrap()).unwrap();
        assert_eq!(v["kind"], "value");
        assert_eq!(v["data"][0]["unit"], "Pa");
    }

    #[test]
    fn a_material_from_atlas_is_read_and_checked() {
        let p = r#"{"datum_link":1,"from":{"app":"atlas"},"to":"mesh","kind":"material","data":{"name":"Aluminium 6061-T6","e":68.9e9,"nu":0.33,"rho":2700,"fy":276e6,"k":167,"alpha":23.6e-6,"source":"ASM Handbook"}}"#;
        let (who, Incoming::Material(m)) = read(p).unwrap();
        assert_eq!(who, "Datum Atlas");
        assert_eq!((m.e, m.fy, m.rho), (68.9e9, 276e6, 2700.0));
        assert_eq!(m.source, "ASM Handbook");
        // Nonsense is refused with a reason, never a panic.
        for bad in ["", "[]", "{\"datum_link\":2}", r#"{"datum_link":1,"kind":"material","data":{"e":-5}}"#, r#"{"datum_link":1,"kind":"material","data":{"e":2e11,"nu":0.7}}"#, r#"{"datum_link":1,"kind":"task","data":{}}"#, r#"{"datum_link":1,"kind":"material"}"#] {
            assert!(read(bad).is_err(), "{bad}");
        }
    }
}
