//! DXF import and export of a part's section (0.3.5). Reads the ASCII DXF that CAD programs save:
//! lines, arcs, circles and polylines (with their arc bulges) in the ENTITIES section. Loose lines
//! and arcs that meet end to end are joined into closed outlines; outlines inside others become
//! holes (by how deeply they nest). Arcs stay arcs. Anything else (splines, ellipses, text,
//! blocks) is left out and counted. Writes a section back as closed polylines and circles.

use crate::model::{arc, Geom, Model};

/// A section read from a DXF: its shapes (geometry and whether each is a hole) in metres, and
/// what was left out or assumed.
#[derive(Debug, Default)]
pub struct Imported {
    pub shapes: Vec<(Geom, bool)>,
    pub warnings: Vec<String>,
}

type P = [f64; 2];
/// A piece of outline: from p to q, straight or an arc (bulge = tan of a quarter of its angle).
#[derive(Clone, Copy, Debug)]
struct Seg {
    p: P,
    q: P,
    b: f64,
}

/// Metres per drawing unit for a $INSUNITS code.
fn units(code: i64) -> Option<(f64, &'static str)> {
    Some(match code {
        1 => (0.0254, "inches"),
        2 => (0.3048, "feet"),
        4 => (0.001, "millimetres"),
        5 => (0.01, "centimetres"),
        6 => (1.0, "metres"),
        _ => return None,
    })
}

/// Read a DXF's section. `scale` (metres per drawing unit) overrides the units the file gives.
pub fn read(text: &str, scale: Option<f64>) -> Result<Imported, String> {
    // Group code and value pairs.
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    if lines.len() < 4 || !lines.contains(&"ENTITIES") {
        return Err("This is not an ASCII DXF drawing with entities (binary DXF is not read; save it from CAD as ASCII DXF).".into());
    }
    let pairs: Vec<(i64, &str)> = lines.chunks(2).filter(|c| c.len() == 2).filter_map(|c| c[0].parse::<i64>().ok().map(|g| (g, c[1]))).collect();
    let mut out = Imported::default();
    // Units from the header.
    let mut file_units = None;
    for w in pairs.windows(2) {
        if w[0].0 == 9 && w[0].1 == "$INSUNITS" && w[1].0 == 70 {
            file_units = w[1].1.parse::<i64>().ok().and_then(units);
        }
    }
    let s = match (scale, file_units) {
        (Some(s), _) => s,
        (None, Some((s, _))) => s,
        (None, None) => {
            out.warnings.push("The drawing does not say its units: it was read in millimetres.".into());
            0.001
        }
    };
    // The entities: each starts with a 0 group naming it.
    let start = pairs.iter().position(|p| p.0 == 2 && p.1 == "ENTITIES").ok_or("The drawing has no ENTITIES section.")?;
    let mut ents: Vec<(&str, Vec<(i64, &str)>)> = vec![];
    for &(g, v) in &pairs[start + 1..] {
        if g == 0 {
            if v == "ENDSEC" {
                break;
            }
            ents.push((v, vec![]));
        } else if let Some(e) = ents.last_mut() {
            e.1.push((g, v));
        }
    }
    let num = |e: &[(i64, &str)], g: i64| e.iter().find(|p| p.0 == g).and_then(|p| p.1.parse::<f64>().ok()).filter(|v| v.is_finite());
    let mut segs: Vec<Seg> = vec![];
    let mut circles: Vec<Geom> = vec![];
    let mut closed: Vec<Vec<Seg>> = vec![];
    let mut skipped: std::collections::BTreeMap<String, usize> = Default::default();
    // POLYLINE … VERTEX … SEQEND: the vertices follow as entities of their own.
    let mut poly: Option<(bool, Vec<(P, f64)>)> = None;
    let finish = |verts: Vec<(P, f64)>, closed_flag: bool, segs: &mut Vec<Seg>, closed: &mut Vec<Vec<Seg>>| {
        let n = verts.len();
        if n < 2 {
            return;
        }
        let k = if closed_flag { n } else { n - 1 };
        let ring: Vec<Seg> = (0..k).map(|i| Seg { p: verts[i].0, q: verts[(i + 1) % n].0, b: verts[i].1 }).filter(|g| g.p != g.q).collect();
        if closed_flag {
            closed.push(ring);
        } else {
            segs.extend(ring);
        }
    };
    for (kind, e) in &ents {
        let pt = |gx: i64, gy: i64| -> Option<P> { Some([num(e, gx)? * s, num(e, gy)? * s]) };
        match *kind {
            "LINE" => {
                if let (Some(p), Some(q)) = (pt(10, 20), pt(11, 21)) {
                    if p != q {
                        segs.push(Seg { p, q, b: 0.0 });
                    }
                }
            }
            "ARC" => {
                if let (Some(c), Some(r), Some(a0), Some(a1)) = (pt(10, 20), num(e, 40), num(e, 50), num(e, 51)) {
                    let r = r * s;
                    let (t0, mut t1) = (a0.to_radians(), a1.to_radians());
                    while t1 <= t0 {
                        t1 += std::f64::consts::TAU;
                    }
                    let at = |t: f64| [c[0] + r * t.cos(), c[1] + r * t.sin()];
                    let sweep = t1 - t0;
                    // A turn of more than half a circle is two arcs.
                    if sweep > std::f64::consts::PI {
                        let tm = t0 + sweep / 2.0;
                        segs.push(Seg { p: at(t0), q: at(tm), b: (sweep / 8.0).tan() });
                        segs.push(Seg { p: at(tm), q: at(t1), b: (sweep / 8.0).tan() });
                    } else {
                        segs.push(Seg { p: at(t0), q: at(t1), b: (sweep / 4.0).tan() });
                    }
                }
            }
            "CIRCLE" => {
                if let (Some(c), Some(r)) = (pt(10, 20), num(e, 40)) {
                    circles.push(Geom::Circle { c, r: r * s });
                }
            }
            "LWPOLYLINE" => {
                let flag = num(e, 70).unwrap_or(0.0) as i64;
                let mut verts: Vec<(P, f64)> = vec![];
                let mut x = None;
                for &(g, v) in e {
                    match g {
                        10 => x = v.parse::<f64>().ok().filter(|v| v.is_finite()),
                        20 => {
                            if let (Some(xv), Some(yv)) = (x, v.parse::<f64>().ok().filter(|v| v.is_finite())) {
                                verts.push(([xv * s, yv * s], 0.0));
                            }
                        }
                        42 => {
                            if let (Some(last), Some(b)) = (verts.last_mut(), v.parse::<f64>().ok().filter(|v| v.is_finite())) {
                                last.1 = b;
                            }
                        }
                        _ => {}
                    }
                }
                finish(verts, flag & 1 == 1, &mut segs, &mut closed);
            }
            "POLYLINE" => poly = Some((num(e, 70).unwrap_or(0.0) as i64 & 1 == 1, vec![])),
            "VERTEX" => {
                if let (Some((_, verts)), Some(p)) = (poly.as_mut(), pt(10, 20)) {
                    verts.push((p, num(e, 42).unwrap_or(0.0)));
                }
            }
            "SEQEND" => {
                if let Some((flag, verts)) = poly.take() {
                    finish(verts, flag, &mut segs, &mut closed);
                }
            }
            other => *skipped.entry(other.to_string()).or_default() += 1,
        }
    }
    // Pieces that meet end to end become closed outlines.
    let extent = segs.iter().flat_map(|g| [g.p, g.q]).chain(closed.iter().flatten().map(|g| g.p)).fold(0.0f64, |a, p| a.max(p[0].abs()).max(p[1].abs())).max(1e-9);
    let tol = 1e-6 * extent;
    let near = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) < tol;
    let mut used = vec![false; segs.len()];
    let mut open = 0;
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut ring = vec![segs[i]];
        loop {
            let end = ring.last().unwrap().q;
            if near(end, ring[0].p) {
                closed.push(ring);
                break;
            }
            let next = (0..segs.len()).find(|&j| !used[j] && (near(segs[j].p, end) || near(segs[j].q, end)));
            match next {
                Some(j) => {
                    used[j] = true;
                    let g = segs[j];
                    // Turned round, an arc bends the other way.
                    ring.push(if near(g.p, end) { Seg { p: end, ..g } } else { Seg { p: end, q: g.p, b: -g.b } });
                }
                None => {
                    open += ring.len();
                    break;
                }
            }
        }
    }
    if open > 0 {
        out.warnings.push(if open == 1 { "1 line or arc that does not close into an outline was left out.".into() } else { format!("{open} lines or arcs that do not close into an outline were left out.") });
    }
    for (k, n) in skipped {
        if !matches!(k.as_str(), "TEXT" | "MTEXT" | "DIMENSION" | "POINT" | "HATCH" | "VIEWPORT") {
            out.warnings.push(format!("{n} {} {} left out (only lines, arcs, circles and polylines are read).", k.to_lowercase(), if n == 1 { "was" } else { "were" }));
        }
    }
    // Each closed ring as a polygon (at least three corners; arcs at most half a circle).
    let mut geoms: Vec<Geom> = circles;
    for ring in closed {
        let mut pts: Vec<P> = vec![];
        let mut bulge: Vec<f64> = vec![];
        for g in &ring {
            let b = g.b;
            if b.abs() > 1.0 + 1e-9 {
                let (c, r, a0, th) = arc(g.p, g.q, b);
                let t = a0 + th / 2.0;
                pts.extend([g.p, [c[0] + r * t.cos(), c[1] + r * t.sin()]]);
                bulge.extend([(th / 8.0).tan(); 2]);
            } else {
                pts.push(g.p);
                bulge.push(b.clamp(-1.0, 1.0));
            }
        }
        while pts.len() < 3 && !pts.is_empty() {
            // Two pieces (two arcs, say): split the first at its middle.
            let (p, q, b) = (pts[0], pts[1 % pts.len()], bulge[0]);
            let (mid, half) = if b == 0.0 {
                ([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], 0.0)
            } else {
                let (c, r, a0, th) = arc(p, q, b);
                let t = a0 + th / 2.0;
                ([c[0] + r * t.cos(), c[1] + r * t.sin()], (th / 8.0).tan())
            };
            pts.insert(1, mid);
            bulge[0] = half;
            bulge.insert(1, half);
        }
        if pts.len() >= 3 {
            let g = Geom::Poly { pts, bulge };
            if g.area().abs() > 1e-18 {
                geoms.push(g);
            }
        }
    }
    if geoms.is_empty() {
        return Err(out.warnings.first().cloned().map_or("The drawing has no closed outlines (lines and arcs that close, polylines, circles) to make a part from.".into(), |w| format!("The drawing has no closed outlines to make a part from. {w}")));
    }
    // Holes: outlines inside an odd number of others. Each is drawn counter-clockwise.
    let flat: Vec<Vec<P>> = geoms.iter().map(|g| g.outline(extent / 200.0, 64).0).collect();
    let inside = |pt: P, poly: &[P]| {
        let mut c = false;
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + n - 1) % n]);
            if (a[1] > pt[1]) != (b[1] > pt[1]) && pt[0] < (b[0] - a[0]) * (pt[1] - a[1]) / (b[1] - a[1]) + a[0] {
                c = !c;
            }
        }
        c
    };
    for (i, g) in geoms.iter().enumerate() {
        let probe = flat[i][0];
        let depth = (0..geoms.len()).filter(|&j| j != i && geoms[j].area().abs() > g.area().abs() && inside(probe, &flat[j])).count();
        let g = match g {
            Geom::Poly { pts, bulge } if g.area() < 0.0 => {
                // Reverse: the corners backwards, each arc turning the other way.
                let n = pts.len();
                Geom::Poly { pts: (0..n).map(|k| pts[(n - k) % n]).collect(), bulge: (0..n).map(|k| -bulge[(2 * n - 1 - k) % n]).collect() }
            }
            g => g.clone(),
        };
        out.shapes.push((g, depth % 2 == 1));
    }
    // Solids first, then holes, each largest first.
    out.shapes.sort_by(|a, b| a.1.cmp(&b.1).then(b.0.area().abs().total_cmp(&a.0.area().abs())));
    Ok(out)
}

/// The model's section as an ASCII DXF in millimetres: each shape a closed polyline (with its arcs)
/// or a circle, holes on a layer of their own.
pub fn write(m: &Model) -> String {
    let mut s = String::from("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
    let mm = |v: f64| format!("{:.6}", v * 1000.0);
    for sh in &m.shapes {
        let layer = if sh.hole { "HOLES" } else { "PART" };
        match &sh.geom {
            Geom::Circle { c, r } => s += &format!("0\nCIRCLE\n8\n{layer}\n10\n{}\n20\n{}\n40\n{}\n", mm(c[0]), mm(c[1]), mm(*r)),
            Geom::Poly { pts, bulge } => {
                s += &format!("0\nLWPOLYLINE\n8\n{layer}\n90\n{}\n70\n1\n", pts.len());
                for (p, b) in pts.iter().zip(bulge) {
                    s += &format!("10\n{}\n20\n{}\n", mm(p[0]), mm(p[1]));
                    if *b != 0.0 {
                        s += &format!("42\n{b:.12}\n");
                    }
                }
            }
        }
    }
    s + "0\nENDSEC\n0\nEOF\n"
}

/// A small drawing for the tests and the verification (0.3.5): a 100 × 40 plate drawn as four
/// loose lines every which way, a 20 mm slot of two lines and two arcs and a 16 mm hole inside it,
/// a stray line and a spline; in inches.
/// The sample drawing, for the window's tests.
pub const SAMPLE_PUB: &str = SAMPLE;
pub(crate) const SAMPLE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n1\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLINE\n10\n0\n20\n0\n11\n100\n21\n0\n0\nLINE\n10\n100\n20\n40\n11\n100\n21\n0\n0\nLINE\n10\n100\n20\n40\n11\n0\n21\n40\n0\nLINE\n10\n0\n20\n0\n11\n0\n21\n40\n\
0\nLINE\n10\n20\n20\n15\n11\n40\n21\n15\n0\nLINE\n10\n20\n20\n25\n11\n40\n21\n25\n0\nARC\n10\n40\n20\n20\n40\n5\n50\n270\n51\n90\n0\nARC\n10\n20\n20\n20\n40\n5\n50\n90\n51\n270\n\
0\nCIRCLE\n10\n70\n20\n20\n40\n8\n0\nLINE\n10\n200\n20\n0\n11\n300\n21\n0\n0\nSPLINE\n0\nENDSEC\n0\nEOF\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_section_comes_back_from_a_dxf() {
        for e in crate::examples::list() {
            let m = crate::examples::get(e.id).unwrap();
            if m.shapes.is_empty() {
                continue;
            }
            let back = read(&write(&m), None).unwrap();
            assert_eq!(back.shapes.len(), m.shapes.len(), "{}", e.id);
            assert!(back.warnings.is_empty(), "{}: {:?}", e.id, back.warnings);
            let key = |h: bool, a: f64| (h, (a.abs() * 1e12).round() as i64);
            let mut want: Vec<(bool, i64)> = m.shapes.iter().map(|sh| key(sh.hole, sh.geom.area())).collect();
            let mut got: Vec<(bool, i64)> = back.shapes.iter().map(|(g, h)| key(*h, g.area())).collect();
            want.sort();
            got.sort();
            for (w, g) in want.iter().zip(&got) {
                assert!(w.0 == g.0 && (w.1 - g.1).abs() as f64 <= 1e-9 * w.1 as f64 + 2.0, "{}: {want:?} {got:?}", e.id);
            }
        }
    }

    #[test]
    fn loose_lines_and_arcs_join_into_outlines_and_holes() {
        // A 100 × 40 plate as four lines drawn every which way, with a slot of two lines and two
        // arcs inside it, a circle, and a stray line; in inches.
        let dxf = SAMPLE;
        let r = read(dxf, None).unwrap();
        let inch = 0.0254f64;
        let solids: Vec<&Geom> = r.shapes.iter().filter(|s| !s.1).map(|s| &s.0).collect();
        let holes: Vec<&Geom> = r.shapes.iter().filter(|s| s.1).map(|s| &s.0).collect();
        assert_eq!((solids.len(), holes.len()), (1, 2), "{:?}", r.shapes);
        assert!((solids[0].area() - 4000.0 * inch * inch).abs() < 1e-12);
        let slot = 20.0 * 10.0 + std::f64::consts::PI * 25.0;
        let ha: f64 = holes.iter().map(|g| g.area()).sum();
        assert!((ha - (slot + std::f64::consts::PI * 64.0) * inch * inch).abs() < 1e-9, "{ha}");
        assert!(r.warnings.iter().any(|w| w.contains("does not close")) && r.warnings.iter().any(|w| w.contains("spline")), "{:?}", r.warnings);
        assert!(read("hello", None).is_err());
    }
}
