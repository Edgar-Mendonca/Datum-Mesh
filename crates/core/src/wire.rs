//! Results on their way to the window, made small. Long arrays of numbers (node coordinates,
//! displacements, nodal and element fields, triangle corners) are sent as base64 instead of JSON
//! text: 32-bit floats for values (7 significant digits, more than any display shows) and 32-bit
//! unsigned integers for indices. A 25 000-element part went from 22 MB of JSON to about 6 MB.
//! Every other value (sums, reactions, the equilibrium check) stays as it is, in full precision.
//!
//! An encoded array is `{"$f32": "<base64>", "w": width}` or `{"$u32": …}`; `w` > 1 means rows of
//! that width (such as [x, y] pairs). The window turns them back into arrays (`ui/src/api.ts`).

use serde_json::{json, Map, Value};

/// Arrays shorter than this stay as JSON (frame members' 41 stations, small meshes' lists).
const MIN: usize = 64;

/// Encode the long numeric arrays anywhere in a JSON value.
pub fn encode(v: Value) -> Value {
    match v {
        Value::Array(a) => match flat(&a) {
            Some((nums, w)) if nums.len() >= MIN => pack(&nums, w),
            _ => Value::Array(a.into_iter().map(encode).collect()),
        },
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, encode(v))).collect::<Map<_, _>>()),
        v => v,
    }
}

/// The numbers of an array of numbers, or of an array of equal-length arrays of numbers (rows).
fn flat(a: &[Value]) -> Option<(Vec<f64>, usize)> {
    if a.is_empty() {
        return None;
    }
    if a.iter().all(Value::is_number) {
        return Some((a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect(), 1));
    }
    let w = a[0].as_array()?.len();
    if w == 0 || w > 16 {
        return None;
    }
    let mut out = Vec::with_capacity(a.len() * w);
    for r in a {
        let r = r.as_array()?;
        if r.len() != w || !r.iter().all(Value::is_number) {
            return None;
        }
        out.extend(r.iter().map(|x| x.as_f64().unwrap_or(0.0)));
    }
    Some((out, w))
}

fn pack(nums: &[f64], w: usize) -> Value {
    let ints = nums.iter().all(|x| x.fract() == 0.0 && *x >= 0.0 && *x <= u32::MAX as f64);
    let mut bytes = Vec::with_capacity(nums.len() * 4);
    if ints {
        for x in nums {
            bytes.extend_from_slice(&(*x as u32).to_le_bytes());
        }
        json!({ "$u32": b64(&bytes), "w": w })
    } else {
        for x in nums {
            bytes.extend_from_slice(&(*x as f32).to_le_bytes());
        }
        json!({ "$f32": b64(&bytes), "w": w })
    }
}

const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            s.push(if k <= c.len() { ABC[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unb64(s: &str) -> Vec<u8> {
        let val = |c: u8| ABC.iter().position(|x| *x == c).unwrap() as u32;
        let mut out = vec![];
        for c in s.as_bytes().chunks(4) {
            let n = c.iter().take_while(|x| **x != b'=').enumerate().fold(0, |n, (k, x)| n | val(*x) << (18 - 6 * k));
            let keep = c.iter().filter(|x| **x != b'=').count() - 1;
            out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8][..keep]);
        }
        out
    }
    /// The window's decoder, in Rust, for the round trip.
    fn decode(v: Value) -> Value {
        match v {
            Value::Object(o) if o.contains_key("$f32") || o.contains_key("$u32") => {
                let w = o["w"].as_u64().unwrap() as usize;
                let (k, f) = if let Some(s) = o.get("$f32") { (s, true) } else { (&o["$u32"], false) };
                let b = unb64(k.as_str().unwrap());
                let nums: Vec<Value> = b.chunks(4).map(|c| { let a = [c[0], c[1], c[2], c[3]]; if f { json!(f32::from_le_bytes(a) as f64) } else { json!(u32::from_le_bytes(a)) } }).collect();
                if w == 1 { Value::Array(nums) } else { Value::Array(nums.chunks(w).map(|r| Value::Array(r.to_vec())).collect()) }
            }
            Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, decode(v))).collect()),
            Value::Array(a) => Value::Array(a.into_iter().map(decode).collect()),
            v => v,
        }
    }

    #[test]
    fn base64_is_standard() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(unb64("Zm9vYmE="), b"fooba");
    }

    #[test]
    fn a_solved_part_shrinks_and_comes_back_to_float_precision() {
        let m = crate::examples::get("bracket").unwrap();
        let r = crate::solve(&m).unwrap();
        let plain = serde_json::to_value(&r).unwrap();
        let small = encode(plain.clone());
        let (a, b) = (plain.to_string().len(), small.to_string().len());
        assert!(b * 2 < a, "{a} → {b}");
        let back = decode(small);
        // Same shape; every number within float precision; small arrays and scalars exact.
        fn same(x: &Value, y: &Value, path: &str) {
            match (x, y) {
                (Value::Array(p), Value::Array(q)) => { assert_eq!(p.len(), q.len(), "{path}"); for (i, (u, v)) in p.iter().zip(q).enumerate() { same(u, v, &format!("{path}[{i}]")); } }
                (Value::Object(p), Value::Object(q)) => { assert_eq!(p.len(), q.len(), "{path}"); for (k, u) in p { same(u, &q[k], &format!("{path}.{k}")); } }
                (Value::Number(p), Value::Number(q)) => { let (u, v) = (p.as_f64().unwrap(), q.as_f64().unwrap()); assert!((u - v).abs() <= 1e-7 * u.abs().max(1e-30), "{path}: {u} vs {v}"); }
                _ => assert_eq!(x, y, "{path}"),
            }
        }
        same(&plain, &back, "");
        // The equilibrium sums are not touched.
        assert_eq!(plain["applied"], back["applied"]);
        assert_eq!(plain["imbalance"], back["imbalance"]);
    }

    #[test]
    fn triangle_corners_go_as_integers() {
        let tris: Vec<Vec<u32>> = (0..100).map(|i| vec![i, i + 1, i + 2]).collect();
        let e = encode(json!({ "tris": tris, "few": [1.5, 2.5] }));
        assert!(e["tris"]["$u32"].is_string() && e["tris"]["w"] == 3);
        assert_eq!(e["few"], json!([1.5, 2.5]));
        assert_eq!(decode(e)["tris"][99], json!([99, 100, 101]));
    }
}
