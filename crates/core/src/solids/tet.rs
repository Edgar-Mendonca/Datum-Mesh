//! Tetrahedra and imported meshes (0.5.5): a 3D solid of any shape, meshed by a mesher (Gmsh's
//! `.msh` or an `.inp` in the CalculiX input format, which most meshers write) and solved with our
//! own 4-node (linear) and 10-node (quadratic) tetrahedra, isoparametric, in volume coordinates
//! (Zienkiewicz and Taylor, *The Finite Element Method*, vol. 1, §8.8; Bathe §5.3). The outer
//! surface is split into faces where it turns sharply, and the file's named surfaces are kept, so
//! supports and loads go on faces as they do on swept solids.

use super::solid::{principal, stress_set, tri_n, tri_rule, Field3, SolidReaction, SolidResults, Surface, Tally, Volume};
use crate::engine::sparse::System;
use crate::model::{Analysis, LoadKind, Model, SupportKind, Target};
use crate::solids::mesh::Quality;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A face of the outer surface: its name and its facets (element, and the corner it is opposite).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaceSet {
    pub name: String,
    pub facets: Vec<(u32, u8)>,
}

/// A tetrahedral mesh (m): 4 corner nodes per element, then for quadratic elements the six
/// mid-edge nodes on edges (0,1), (1,2), (2,0), (0,3), (1,3), (2,3). Every element is turned so
/// its volume is positive.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TetMesh {
    pub nodes: Vec<[f64; 3]>,
    pub tets: Vec<Vec<u32>>,
    pub order: u8,
    pub faces: Vec<FaceSet>,
    /// The material of the whole solid (its id in the model).
    pub material: u32,
    /// Where it came from (the file's name), for the report.
    pub source: String,
}

/// The six edges, by corner, in mid-node order.
pub const EDGES: [(usize, usize); 6] = [(0, 1), (1, 2), (2, 0), (0, 3), (1, 3), (2, 3)];
/// The face opposite each corner, by its corners.
pub const FACES: [[usize; 3]; 4] = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];
/// The most nodes an imported mesh may have (three unknowns each: 200 000 unknowns, as swept solids).
pub const MAX_NODES: usize = 66_000;

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn edge_of(a: usize, b: usize) -> usize {
    EDGES.iter().position(|&(p, q)| (p, q) == (a, b) || (p, q) == (b, a)).expect("an edge")
}

impl TetMesh {
    /// The nodes of a facet (3 corners, then for quadratic elements the mid-nodes of its edges),
    /// ordered so the normal by the right-hand rule points out of the element.
    pub fn facet(&self, t: usize, k: usize) -> Vec<u32> {
        let el = &self.tets[t];
        let c = FACES[k];
        let mut out: Vec<u32> = c.iter().map(|&i| el[i]).collect();
        if self.order == 2 {
            for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
                out.push(el[4 + edge_of(a, b)]);
            }
        }
        out
    }
    fn vol6(&self, t: usize) -> f64 {
        let el = &self.tets[t];
        let p = |i: usize| self.nodes[el[i] as usize];
        dot(cross(sub(p(1), p(0)), sub(p(2), p(0))), sub(p(3), p(0)))
    }

    /// Check the mesh and put it in order: indices in range, numbers finite, every element turned
    /// to a positive volume, faces found. Named facet sets (by their three corner nodes) become
    /// faces first; the rest of the surface is split into faces where it turns more than 35°.
    pub fn finish(mut self, named: Vec<(String, Vec<[u32; 3]>)>) -> Result<TetMesh, String> {
        let nn = self.nodes.len();
        let per = if self.order == 2 { 10 } else { 4 };
        if self.tets.is_empty() {
            return Err("The mesh has no tetrahedra. Datum Mesh reads 4- and 10-node tetrahedra (Gmsh types 4 and 11; C3D4 and C3D10).".into());
        }
        if nn > MAX_NODES {
            return Err(format!("The mesh has {nn} nodes; Datum Mesh solves up to {MAX_NODES} (200 000 unknowns). Mesh it coarser."));
        }
        if self.nodes.iter().flatten().any(|v| !v.is_finite()) {
            return Err("A node of the mesh has a coordinate that is not a number.".into());
        }
        for el in &self.tets {
            if el.len() != per || el.iter().any(|&i| i as usize >= nn) {
                return Err("An element of the mesh refers to a node that is not there, or has the wrong number of nodes.".into());
            }
        }
        let size = self.size();
        for t in 0..self.tets.len() {
            let v = self.vol6(t);
            if v.abs() <= 1e-12 * size.powi(3) {
                return Err(format!("Element {} of the mesh is flat (no volume).", t + 1));
            }
            if v < 0.0 {
                let el = &mut self.tets[t];
                el.swap(1, 2);
                if per == 10 {
                    let m = [el[4], el[5], el[6], el[7], el[8], el[9]];
                    el[4..10].copy_from_slice(&[m[2], m[1], m[0], m[3], m[5], m[4]]);
                }
            }
        }
        // The outer surface: the facets that only one element has.
        let key = |t: &TetMesh, e: usize, k: usize| {
            let mut c = FACES[k].map(|i| t.tets[e][i]);
            c.sort_unstable();
            c
        };
        let mut seen: HashMap<[u32; 3], (u32, u8, u32)> = HashMap::new();
        for e in 0..self.tets.len() {
            for k in 0..4 {
                let s = seen.entry(key(&self, e, k)).or_insert((e as u32, k as u8, 0));
                s.2 += 1;
            }
        }
        let mut outer: Vec<(u32, u8)> = seen.values().filter(|s| s.2 == 1).map(|s| (s.0, s.1)).collect();
        outer.sort_unstable();
        let by_key: HashMap<[u32; 3], usize> = outer.iter().enumerate().map(|(i, &(e, k))| (key(&self, e as usize, k as usize), i)).collect();
        let mut owner = vec![usize::MAX; outer.len()];
        let mut faces = vec![];
        for (name, tris) in named {
            let mut f: Vec<(u32, u8)> = vec![];
            for mut c in tris {
                c.sort_unstable();
                if let Some(&i) = by_key.get(&c) {
                    if owner[i] == usize::MAX {
                        owner[i] = faces.len();
                        f.push(outer[i]);
                    }
                }
            }
            if !f.is_empty() {
                faces.push(FaceSet { name, facets: f });
            }
        }
        // Split the rest where the surface turns sharply.
        let normal = |m: &TetMesh, (e, k): (u32, u8)| {
            let f = m.facet(e as usize, k as usize);
            let p = |i: usize| m.nodes[f[i] as usize];
            let n = cross(sub(p(1), p(0)), sub(p(2), p(0)));
            let l = dot(n, n).sqrt().max(1e-300);
            n.map(|v| v / l)
        };
        let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, &(e, k)) in outer.iter().enumerate() {
            let c = FACES[k as usize].map(|j| self.tets[e as usize][j]);
            for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
                by_edge.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        let cos_max = 35f64.to_radians().cos();
        let mut auto = 0;
        for start in 0..outer.len() {
            if owner[start] != usize::MAX {
                continue;
            }
            let id = faces.len();
            auto += 1;
            let mut f = vec![];
            let mut stack = vec![start];
            owner[start] = id;
            while let Some(i) = stack.pop() {
                f.push(outer[i]);
                let ni = normal(&self, outer[i]);
                let c = FACES[outer[i].1 as usize].map(|j| self.tets[outer[i].0 as usize][j]);
                for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
                    for &j in &by_edge[&(a.min(b), a.max(b))] {
                        if owner[j] == usize::MAX && dot(ni, normal(&self, outer[j])) >= cos_max {
                            owner[j] = id;
                            stack.push(j);
                        }
                    }
                }
            }
            faces.push(FaceSet { name: format!("Face {auto}"), facets: f });
        }
        self.faces = faces;
        Ok(self)
    }

    /// A typical length: the size of the box round the mesh.
    pub fn size(&self) -> f64 {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in &self.nodes {
            for c in 0..3 {
                lo[c] = lo[c].min(p[c]);
                hi[c] = hi[c].max(p[c]);
            }
        }
        (0..3).map(|c| hi[c] - lo[c]).fold(0.0, f64::max).max(1e-300)
    }

    /// The centre of a face (the mean of its facets' corners), for finding it by where it is.
    pub fn face_centre(&self, f: usize) -> [f64; 3] {
        let mut s = [0.0; 3];
        let mut n = 0.0f64;
        for &(e, k) in &self.faces[f].facets {
            for &i in &self.facet(e as usize, k as usize)[..3] {
                for c in 0..3 {
                    s[c] += self.nodes[i as usize][c];
                }
                n += 1.0;
            }
        }
        s.map(|v| v / n.max(1.0))
    }

    /// The first face whose every corner node satisfies `inside` (for the benchmarks and examples).
    pub fn face_where(&self, inside: impl Fn([f64; 3]) -> bool) -> Option<u32> {
        (0..self.faces.len()).find(|&f| self.faces[f].facets.iter().all(|&(e, k)| self.facet(e as usize, k as usize)[..3].iter().all(|&i| inside(self.nodes[i as usize])))).map(|f| f as u32)
    }
}

/// Check a stored mesh holds together (a model read from a file): a known order, every element's
/// nodes there, numbers finite, faces on real elements.
pub fn valid(m: &TetMesh) -> Result<(), String> {
    let per = match m.order {
        1 => 4,
        2 => 10,
        _ => return Err("The imported mesh has an unknown element order.".into()),
    };
    if m.tets.is_empty() || m.nodes.len() > MAX_NODES || m.nodes.iter().flatten().any(|v| !v.is_finite()) {
        return Err("The imported mesh is empty, too large or has a coordinate that is not a number.".into());
    }
    if m.tets.iter().any(|e| e.len() != per || e.iter().any(|&i| i as usize >= m.nodes.len())) {
        return Err("An element of the imported mesh refers to a node that is not there.".into());
    }
    if m.faces.iter().any(|f| f.facets.iter().any(|&(e, k)| e as usize >= m.tets.len() || k > 3)) {
        return Err("A face of the imported mesh refers to an element that is not there.".into());
    }
    Ok(())
}

/// A structured tetrahedral mesh of a mapped box: `n` cells along each of u, v, w in [0, 1]³, each
/// cut into six tetrahedra about its diagonal, placed in space by `map` (mid-edge nodes are mapped
/// too, so curved shapes are followed). For the benchmarks and examples.
pub fn grid(n: [usize; 3], order: u8, map: impl Fn([f64; 3]) -> [f64; 3]) -> Result<TetMesh, String> {
    let id = |i: usize, j: usize, k: usize| (i + (n[0] + 1) * (j + (n[1] + 1) * k)) as u32;
    let mut nodes = vec![];
    for k in 0..=n[2] {
        for j in 0..=n[1] {
            for i in 0..=n[0] {
                nodes.push(map([i as f64 / n[0] as f64, j as f64 / n[1] as f64, k as f64 / n[2] as f64]));
            }
        }
    }
    let param = |q: u32| {
        let q = q as usize;
        let (i, r) = (q % (n[0] + 1), q / (n[0] + 1));
        [i as f64 / n[0] as f64, (r % (n[1] + 1)) as f64 / n[1] as f64, (r / (n[1] + 1)) as f64 / n[2] as f64]
    };
    let mut tets = vec![];
    let mut mids: HashMap<(u32, u32), u32> = HashMap::new();
    for k in 0..n[2] {
        for j in 0..n[1] {
            for i in 0..n[0] {
                let v = |b: [usize; 3]| id(i + b[0], j + b[1], k + b[2]);
                for perm in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                    let mut b = [0usize; 3];
                    let mut el = vec![v(b)];
                    for &a in &perm {
                        b[a] = 1;
                        el.push(v(b));
                    }
                    if order == 2 {
                        for (p, q) in EDGES {
                            let (a, c) = (el[p].min(el[q]), el[p].max(el[q]));
                            let m = *mids.entry((a, c)).or_insert_with(|| {
                                let (pa, pc) = (param(a), param(c));
                                nodes.push(map([(pa[0] + pc[0]) / 2.0, (pa[1] + pc[1]) / 2.0, (pa[2] + pc[2]) / 2.0]));
                                (nodes.len() - 1) as u32
                            });
                            el.push(m);
                        }
                    }
                    tets.push(el);
                }
            }
        }
    }
    TetMesh { nodes, tets, order, faces: vec![], material: 1, source: "Structured mesh".into() }.finish(vec![])
}

// ---------- reading meshes ----------

/// Read a Gmsh mesh (`.msh`, ASCII, format 2.2 or 4.1), coordinates times `scale` (to metres).
/// Physical surfaces become named faces.
pub fn read_msh(text: &str, scale: f64) -> Result<TetMesh, String> {
    let sec = |name: &str| -> Option<Vec<&str>> {
        let start = text.find(&format!("${name}"))?;
        let body = &text[start..];
        let end = body.find(&format!("$End{name}"))?;
        Some(body[..end].lines().skip(1).map(str::trim).filter(|l| !l.is_empty()).collect())
    };
    let fmt = sec("MeshFormat").ok_or("This is not a Gmsh mesh file (no $MeshFormat).")?;
    let head: Vec<&str> = fmt.first().ok_or("The $MeshFormat section is empty.")?.split_whitespace().collect();
    let version: f64 = head.first().and_then(|v| v.parse().ok()).ok_or("The mesh format version cannot be read.")?;
    if head.get(1) == Some(&"1") {
        return Err("This Gmsh file is binary; save it as ASCII (File › Export, uncheck Binary).".into());
    }
    let num = |s: &str| s.parse::<f64>().map_err(|_| format!("“{s}” in the mesh file is not a number."));
    let int = |s: &str| s.parse::<usize>().map_err(|_| format!("“{s}” in the mesh file is not a whole number."));
    let names: HashMap<usize, String> = sec("PhysicalNames").map(|l| l.iter().skip(1).filter_map(|x| {
        let mut p = x.splitn(3, ' ');
        let (dim, tag, name) = (p.next()?, p.next()?, p.next()?);
        (dim == "2").then(|| (tag.parse().ok(), name.trim().trim_matches('"').to_string())).and_then(|(t, n)| t.map(|t| (t, n)))
    }).collect()).unwrap_or_default();
    let mut index: HashMap<usize, u32> = HashMap::new();
    let mut nodes: Vec<[f64; 3]> = vec![];
    let lines = sec("Nodes").ok_or("The mesh file has no $Nodes.")?;
    let mut add = |tag: usize, x: &[&str]| -> Result<(), String> {
        if x.len() < 3 {
            return Err("A node line has fewer than three coordinates.".into());
        }
        index.insert(tag, nodes.len() as u32);
        nodes.push([num(x[0])? * scale, num(x[1])? * scale, num(x[2])? * scale]);
        Ok(())
    };
    // Tetrahedra (type 4: 4 nodes; 11: 10 nodes) and triangles (2: 3; 9: 6) with their physical
    // surface (2.2: the first tag; 4.1: by the entity's physical tags).
    let mut tets: Vec<Vec<usize>> = vec![];
    let mut tris: Vec<(usize, [usize; 3])> = vec![];
    let mut other = 0;
    if version < 3.0 {
        for l in lines.iter().skip(1) {
            let x: Vec<&str> = l.split_whitespace().collect();
            if x.len() < 4 {
                return Err("A node line has fewer than three coordinates.".into());
            }
            add(int(x[0])?, &x[1..])?;
        }
        for l in sec("Elements").ok_or("The mesh file has no $Elements.")?.iter().skip(1) {
            let x: Vec<usize> = l.split_whitespace().map(int).collect::<Result<_, _>>()?;
            if x.len() < 3 {
                continue;
            }
            let (ty, ntags) = (x[1], x[2]);
            let nodes_at = 3 + ntags;
            let phys = if ntags > 0 { *x.get(3).ok_or("An element line is too short.")? } else { 0 };
            match ty {
                4 | 11 => tets.push(x.get(nodes_at..).ok_or("An element line is too short.")?.to_vec()),
                2 | 9 if x.len() >= nodes_at + 3 => tris.push((phys, [x[nodes_at], x[nodes_at + 1], x[nodes_at + 2]])),
                15 | 1 | 8 | 2 | 9 => {}
                _ => other += 1,
            }
        }
    } else {
        // 4.1: the surfaces' physical tags from $Entities.
        let mut surf_phys: HashMap<usize, usize> = HashMap::new();
        if let Some(ent) = sec("Entities") {
            let c: Vec<usize> = ent.first().ok_or("The $Entities section is empty.")?.split_whitespace().map(int).collect::<Result<_, _>>()?;
            let skip = c.first().copied().unwrap_or(0) + c.get(1).copied().unwrap_or(0);
            for l in ent.iter().skip(1 + skip).take(c.get(2).copied().unwrap_or(0)) {
                let x: Vec<&str> = l.split_whitespace().collect();
                if x.len() > 8 && int(x[7]).unwrap_or(0) > 0 {
                    surf_phys.insert(int(x[0])?, int(x[8])?);
                }
            }
        }
        let mut i = 1;
        while i < lines.len() {
            let h: Vec<usize> = lines[i].split_whitespace().map(int).collect::<Result<_, _>>()?;
            let count = *h.get(3).ok_or("A $Nodes block header is too short.")?;
            let tags: Vec<usize> = lines.get(i + 1..i + 1 + count).ok_or("A $Nodes block is cut short.")?.iter().map(|t| int(t)).collect::<Result<_, _>>()?;
            for (j, t) in tags.iter().enumerate() {
                let x: Vec<&str> = lines.get(i + 1 + count + j).ok_or("A $Nodes block is cut short.")?.split_whitespace().collect();
                add(*t, &x)?;
            }
            i += 1 + 2 * count;
        }
        let el = sec("Elements").ok_or("The mesh file has no $Elements.")?;
        let mut i = 1;
        while i < el.len() {
            let h: Vec<usize> = el[i].split_whitespace().map(int).collect::<Result<_, _>>()?;
            if h.len() < 4 {
                return Err("An $Elements block header is too short.".into());
            }
            let (dim, ent, ty, count) = (h[0], h[1], h[2], h[3]);
            for l in el.get(i + 1..i + 1 + count).ok_or("An $Elements block is cut short.")? {
                let x: Vec<usize> = l.split_whitespace().map(int).collect::<Result<_, _>>()?;
                match ty {
                    4 | 11 => tets.push(x.get(1..).ok_or("An element line is empty.")?.to_vec()),
                    2 | 9 if dim == 2 && x.len() >= 4 => tris.push((surf_phys.get(&ent).copied().unwrap_or(0), [x[1], x[2], x[3]])),
                    15 | 1 | 8 | 2 | 9 => {}
                    _ => other += 1,
                }
            }
            i += 1 + count;
        }
    }
    if tets.is_empty() && other > 0 {
        return Err("The mesh has no tetrahedra: it is made of other solid elements (hexahedra, prisms…). Mesh it with tetrahedra.".into());
    }
    let order = if tets.first().is_some_and(|t| t.len() == 10) { 2 } else { 1 };
    let look = |t: usize| index.get(&t).copied().ok_or_else(|| format!("An element refers to node {t}, which is not in the file."));
    let mut out = vec![];
    for t in &tets {
        if t.len() != if order == 2 { 10 } else { 4 } {
            return Err("The mesh mixes linear and quadratic tetrahedra.".into());
        }
        let mut el: Vec<u32> = t.iter().map(|&q| look(q)).collect::<Result<_, _>>()?;
        if order == 2 {
            // Gmsh numbers the last two mid-edge nodes (2,3) then (1,3); ours (1,3) then (2,3).
            el.swap(8, 9);
        }
        out.push(el);
    }
    let mut named: Vec<(String, Vec<[u32; 3]>)> = vec![];
    for (phys, tri) in tris {
        if phys == 0 {
            continue;
        }
        let name = names.get(&phys).cloned().unwrap_or_else(|| format!("Surface {phys}"));
        let c = [look(tri[0])?, look(tri[1])?, look(tri[2])?];
        match named.iter_mut().find(|x| x.0 == name) {
            Some(x) => x.1.push(c),
            None => named.push((name, vec![c])),
        }
    }
    TetMesh { nodes, tets: out, order, faces: vec![], material: 1, source: String::new() }.finish(named)
}

/// Read an `.inp` mesh in the CalculiX input format (nodes, C3D4 or C3D10 elements, node sets and
/// element-face surfaces), coordinates times `scale` (to metres). Named surfaces and node sets on
/// the outside become named faces.
pub fn read_inp(text: &str, scale: f64) -> Result<TetMesh, String> {
    let mut index: HashMap<usize, u32> = HashMap::new();
    let mut nodes: Vec<[f64; 3]> = vec![];
    let mut tets: Vec<Vec<usize>> = vec![];
    let mut tet_id: HashMap<usize, usize> = HashMap::new();
    let mut nsets: Vec<(String, Vec<usize>)> = vec![];
    let mut elsets: HashMap<String, Vec<usize>> = HashMap::new();
    let mut surfaces: Vec<(String, Vec<(String, usize)>)> = vec![];
    let mut mode = "";
    let mut per = 0;
    let mut cur = String::new();
    let mut generate = false;
    let mut pending: Vec<usize> = vec![];
    let mut other = 0;
    let opt = |l: &str, k: &str| l.split(',').skip(1).find_map(|p| { let mut kv = p.splitn(2, '='); (kv.next()?.trim().eq_ignore_ascii_case(k)).then(|| kv.next().unwrap_or("").trim().to_string()) });
    let int = |s: &str| s.trim().parse::<usize>().map_err(|_| format!("“{}” in the .inp file is not a whole number.", s.trim()));
    for raw in text.lines() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with("**") {
            continue;
        }
        if let Some(kw) = l.strip_prefix('*') {
            let k = kw.split(',').next().unwrap_or("").trim().to_ascii_uppercase();
            generate = l.to_ascii_uppercase().contains("GENERATE");
            mode = match k.as_str() {
                "NODE" => "node",
                "ELEMENT" => {
                    let ty = opt(l, "type").unwrap_or_default().to_ascii_uppercase();
                    per = if ty.starts_with("C3D10") { 10 } else if ty == "C3D4" || ty == "C3D4H" { 4 } else { 0 };
                    if per == 0 {
                        if ty.starts_with("C3D") {
                            other += 1;
                        }
                        ""
                    } else {
                        cur = opt(l, "elset").unwrap_or_default().to_ascii_uppercase();
                        "elem"
                    }
                }
                "NSET" => {
                    cur = opt(l, "nset").unwrap_or_default();
                    nsets.push((cur.clone(), vec![]));
                    "nset"
                }
                "ELSET" => {
                    cur = opt(l, "elset").unwrap_or_default().to_ascii_uppercase();
                    "elset"
                }
                "SURFACE" => {
                    if opt(l, "type").is_some_and(|t| t.eq_ignore_ascii_case("node")) {
                        ""
                    } else {
                        surfaces.push((opt(l, "name").unwrap_or_else(|| "Surface".into()), vec![]));
                        "surface"
                    }
                }
                _ => "",
            };
            continue;
        }
        let parts: Vec<&str> = l.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        match mode {
            "node" => {
                if parts.len() < 4 {
                    return Err("A *NODE line needs a number and three coordinates.".into());
                }
                let p = |i: usize| parts[i].parse::<f64>().map_err(|_| format!("“{}” in the .inp file is not a number.", parts[i]));
                index.insert(int(parts[0])?, nodes.len() as u32);
                nodes.push([p(1)? * scale, p(2)? * scale, p(3)? * scale]);
            }
            "elem" => {
                // An element may run over several lines.
                for p in &parts {
                    pending.push(int(p)?);
                }
                if pending.len() > per {
                    let id = pending[0];
                    tet_id.insert(id, tets.len());
                    if !cur.is_empty() {
                        elsets.entry(cur.clone()).or_default().push(id);
                    }
                    tets.push(pending[1..=per].to_vec());
                    pending.clear();
                }
            }
            "nset" | "elset" => {
                let ids: Vec<usize> = if generate && parts.len() >= 2 {
                    let (a, b, s) = (int(parts[0])?, int(parts[1])?, parts.get(2).map(|x| int(x)).transpose()?.unwrap_or(1).max(1));
                    (a..=b.min(a + 1_000_000)).step_by(s).collect()
                } else {
                    parts.iter().filter_map(|p| p.parse().ok()).collect()
                };
                if mode == "nset" {
                    if let Some(s) = nsets.last_mut() {
                        s.1.extend(ids);
                    }
                } else {
                    elsets.entry(cur.clone()).or_default().extend(ids);
                }
            }
            "surface" if parts.len() >= 2 => {
                let face = match parts[1].to_ascii_uppercase().as_str() {
                    "S1" => 3,
                    "S2" => 2,
                    "S3" => 0,
                    "S4" => 1,
                    _ => continue,
                };
                if let Some(s) = surfaces.last_mut() {
                    s.1.push((parts[0].to_ascii_uppercase(), face));
                }
            }
            _ => {}
        }
    }
    if tets.is_empty() {
        return Err(if other > 0 { "The .inp file has no tetrahedra: its solid elements are of another kind (hexahedra, wedges…). Mesh it with C3D4 or C3D10.".into() } else { "The .inp file has no C3D4 or C3D10 elements.".into() });
    }
    let order = if tets[0].len() == 10 { 2 } else { 1 };
    let look = |t: usize| index.get(&t).copied().ok_or_else(|| format!("An element refers to node {t}, which is not in the file."));
    let out: Vec<Vec<u32>> = tets.iter().map(|t| t.iter().map(|&q| look(q)).collect::<Result<Vec<u32>, String>>()).collect::<Result<_, _>>()?;
    if out.iter().any(|t| t.len() != out[0].len()) {
        return Err("The .inp file mixes C3D4 and C3D10 elements.".into());
    }
    let mut named: Vec<(String, Vec<[u32; 3]>)> = vec![];
    for (name, list) in surfaces {
        let mut tris = vec![];
        for (set, k) in list {
            let els: Vec<usize> = match set.parse::<usize>() {
                Ok(e) => vec![e],
                Err(_) => elsets.get(&set).cloned().unwrap_or_default(),
            };
            for e in els {
                if let Some(&t) = tet_id.get(&e) {
                    tris.push(FACES[k].map(|i| out[t][i]));
                }
            }
        }
        named.push((name, tris));
    }
    // Node sets: the outer facets whose corners are all in the set (found after finishing).
    let mesh = TetMesh { nodes, tets: out, order, faces: vec![], material: 1, source: String::new() };
    let corner_sets: Vec<(String, std::collections::HashSet<u32>)> = nsets.into_iter().filter(|s| !s.0.is_empty()).map(|(n, ids)| (n, ids.into_iter().filter_map(|i| index.get(&i).copied()).collect())).collect();
    let probe = mesh.clone().finish(vec![])?;
    for (name, set) in corner_sets {
        let tris: Vec<[u32; 3]> = probe.faces.iter().flat_map(|f| f.facets.iter()).map(|&(e, k)| FACES[k as usize].map(|i| probe.tets[e as usize][i])).filter(|c| c.iter().all(|i| set.contains(i))).collect();
        if !tris.is_empty() && !named.iter().any(|x| x.0 == name) {
            named.push((name, tris));
        }
    }
    mesh.finish(named)
}

// ---------- the element ----------

/// Shape functions of a tetrahedron and their derivatives in (ξ, η, ζ) (L1 = ξ, L2 = η, L3 = ζ).
fn shape(order: u8, x: [f64; 3]) -> (Vec<f64>, Vec<[f64; 3]>) {
    let l = [1.0 - x[0] - x[1] - x[2], x[0], x[1], x[2]];
    // dL/d(ξ, η, ζ).
    let dl: [[f64; 3]; 4] = [[-1.0, -1.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if order == 1 {
        return (l.to_vec(), dl.to_vec());
    }
    let mut n = vec![];
    let mut d = vec![];
    for i in 0..4 {
        n.push(l[i] * (2.0 * l[i] - 1.0));
        d.push(dl[i].map(|v| v * (4.0 * l[i] - 1.0)));
    }
    for (a, b) in EDGES {
        n.push(4.0 * l[a] * l[b]);
        d.push([0, 1, 2].map(|c| 4.0 * (dl[a][c] * l[b] + l[a] * dl[b][c])));
    }
    (n, d)
}

/// Global gradients of the shape functions at a point, the functions, and det J.
fn grads(xyz: &[[f64; 3]], order: u8, x: [f64; 3]) -> (Vec<[f64; 3]>, Vec<f64>, f64) {
    let (n, d) = shape(order, x);
    let mut j = [[0.0; 3]; 3];
    for (p, dn) in xyz.iter().zip(&d) {
        for r in 0..3 {
            for c in 0..3 {
                j[r][c] += p[r] * dn[c];
            }
        }
    }
    let det = j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1]) - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0]) + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0]);
    let inv = [
        [(j[1][1] * j[2][2] - j[1][2] * j[2][1]) / det, (j[0][2] * j[2][1] - j[0][1] * j[2][2]) / det, (j[0][1] * j[1][2] - j[0][2] * j[1][1]) / det],
        [(j[1][2] * j[2][0] - j[1][0] * j[2][2]) / det, (j[0][0] * j[2][2] - j[0][2] * j[2][0]) / det, (j[0][2] * j[1][0] - j[0][0] * j[1][2]) / det],
        [(j[1][0] * j[2][1] - j[1][1] * j[2][0]) / det, (j[0][1] * j[2][0] - j[0][0] * j[2][1]) / det, (j[0][0] * j[1][1] - j[0][1] * j[1][0]) / det],
    ];
    let g = d.iter().map(|dn| [0, 1, 2].map(|r| inv[0][r] * dn[0] + inv[1][r] * dn[1] + inv[2][r] * dn[2])).collect();
    (g, n, det)
}

/// Gauss points in the tetrahedron (ξ, η, ζ, weight; weights sum to 1/6): one point for linear
/// elements, four (exact to degree 2) for quadratic ones.
fn rule(order: u8) -> Vec<([f64; 3], f64)> {
    if order == 1 {
        return vec![([0.25; 3], 1.0 / 6.0)];
    }
    let (a, b) = (0.585_410_196_624_968_5, 0.138_196_601_125_010_5);
    vec![([b, b, b], 1.0 / 24.0), ([a, b, b], 1.0 / 24.0), ([b, a, b], 1.0 / 24.0), ([b, b, a], 1.0 / 24.0)]
}

/// The natural coordinates of the nodes, in element order.
fn node_ref(order: u8) -> Vec<[f64; 3]> {
    let c = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut v = c.to_vec();
    if order == 2 {
        for (a, b) in EDGES {
            v.push([0, 1, 2].map(|k| (c[a][k] + c[b][k]) / 2.0));
        }
    }
    v
}

/// Integration points on a facet: point, outward normal times the area element, and the facet's
/// nodes with their shape function values.
fn facet_points(m: &TetMesh, e: usize, k: usize) -> Vec<([f64; 3], [f64; 3], Vec<(usize, f64)>)> {
    let f = m.facet(e, k);
    let o = if m.order == 2 { 2 } else { 1 };
    let xyz: Vec<[f64; 3]> = f.iter().map(|&i| m.nodes[i as usize]).collect();
    // The facet's own numbering: corners, then mid-nodes of (0,1), (1,2), (2,0), as `tri_n` expects.
    tri_rule(o)
        .into_iter()
        .map(|(xi, eta, w)| {
            let (n, d) = tri_n(o, xi, eta);
            let mut p = [0.0; 3];
            let (mut t1, mut t2) = ([0.0; 3], [0.0; 3]);
            for (i, x) in xyz.iter().enumerate() {
                for c in 0..3 {
                    p[c] += n[i] * x[c];
                    t1[c] += d[i][0] * x[c];
                    t2[c] += d[i][1] * x[c];
                }
            }
            let nv = cross(t1, t2).map(|v| v * w);
            (p, nv, f.iter().zip(&n).map(|(&q, &s)| (q as usize, s)).collect())
        })
        .collect()
}

// ---------- solving ----------

/// Solve a 3D solid given as an imported tetrahedral mesh.
pub fn solve(m: &Model) -> Result<SolidResults, String> {
    let ms = m.volume.as_deref().ok_or("This 3D solid has no imported mesh.")?;
    if m.analysis != Analysis::Solid {
        return Err("Imported meshes are solved as 3D solids.".into());
    }
    let nn = ms.nodes.len();
    let mt = m.materials.iter().find(|x| x.id == ms.material).or(m.materials.first()).ok_or("The model has no material.")?;
    let (lam, mu) = (mt.e * mt.nu / ((1.0 + mt.nu) * (1.0 - 2.0 * mt.nu)), mt.e / (2.0 * (1.0 + mt.nu)));
    let mut sys = System::lower(3 * nn);
    let mut warnings = vec![];
    let g = m.loads.iter().filter_map(|l| if let (LoadKind::Gravity { gx, gy, gz }, Target::Body) = (&l.kind, &l.at) { Some([*gx, *gy, *gz]) } else { None }).fold([0.0; 3], |a, b| [a[0] + b[0], a[1] + b[1], a[2] + b[2]]);
    let rule = rule(ms.order);
    let mut volume = 0.0;
    for el in &ms.tets {
        let xyz: Vec<[f64; 3]> = el.iter().map(|&q| ms.nodes[q as usize]).collect();
        let nd = el.len();
        let mut ke = vec![0.0; 9 * nd * nd];
        for &(x, w) in &rule {
            let (gd, n, det) = grads(&xyz, ms.order, x);
            if det <= 0.0 {
                return Err("The mesh has an element turned inside out (its mid-side nodes are too far off); remesh it.".into());
            }
            let w = w * det;
            volume += w;
            for i in 0..nd {
                for j in 0..=i {
                    let dij = dot(gd[i], gd[j]);
                    for a in 0..3 {
                        for b in 0..3 {
                            ke[(3 * i + a) * 3 * nd + 3 * j + b] += w * (lam * gd[i][a] * gd[j][b] + mu * (gd[i][b] * gd[j][a] + if a == b { dij } else { 0.0 }));
                        }
                    }
                }
                if mt.rho > 0.0 && g != [0.0; 3] {
                    for c in 0..3 {
                        sys.f[3 * el[i] as usize + c] += n[i] * mt.rho * g[c] * w;
                    }
                }
            }
        }
        for i in 0..nd {
            for j in 0..=i {
                for a in 0..3 {
                    for b in 0..if i == j { a + 1 } else { 3 } {
                        let (r, c) = (3 * el[i] as usize + a, 3 * el[j] as usize + b);
                        let v = ke[(3 * i + a) * 3 * nd + 3 * j + b];
                        if r >= c {
                            sys.add(r, c, v);
                        } else {
                            sys.add(c, r, v);
                        }
                    }
                }
            }
        }
    }
    // Loads on faces.
    let face = |at: &Target| if let Target::Face { face } = at { ms.faces.get(*face as usize) } else { None };
    let points = |fs: &FaceSet| fs.facets.iter().flat_map(|&(e, k)| facet_points(ms, e as usize, k as usize)).collect::<Vec<_>>();
    for l in &m.loads {
        match (&l.kind, face(&l.at)) {
            (LoadKind::Pressure { .. } | LoadKind::Traction { .. }, Some(fs)) => {
                let pts = points(fs);
                let area: f64 = pts.iter().map(|(_, nv, _)| dot(*nv, *nv).sqrt()).sum();
                for (_, nv, ns) in pts {
                    let da = dot(nv, nv).sqrt();
                    let tr = match l.kind {
                        LoadKind::Pressure { p } => nv.map(|v| -p * v),
                        LoadKind::Traction { fx, fy, fz, total } => {
                            let s = if total { da / area.max(1e-300) } else { da };
                            [fx * s, fy * s, fz * s]
                        }
                        _ => [0.0; 3],
                    };
                    for (q, s) in ns {
                        for c in 0..3 {
                            sys.f[3 * q + c] += s * tr[c];
                        }
                    }
                }
            }
            (LoadKind::Gravity { .. }, _) if l.at == Target::Body => {}
            _ => warnings.push("A load that does not act on an imported mesh (only pressure, face forces and gravity do) was left out.".into()),
        }
    }
    let mut applied = Tally::default();
    for q in 0..nn {
        applied.add(ms.nodes[q], [sys.f[3 * q], sys.f[3 * q + 1], sys.f[3 * q + 2]]);
    }
    if applied.scale == 0.0 {
        warnings.push("The model has no loads, so nothing moves. Add a pressure, a face force or gravity.".into());
    }
    // Supports on faces: held movements, or stiff springs along the normal of sloping faces.
    let diag_mean = {
        let mut dg = vec![0.0; 3 * nn];
        for &(i, j, v) in &sys.k {
            if i == j {
                dg[i] += v;
            }
        }
        dg.iter().sum::<f64>() / dg.len().max(1) as f64
    };
    let kpen = 1e7 * diag_mean;
    let mut held = vec![false; 3 * nn];
    let mut sup_nodes: Vec<(u32, Vec<(usize, [bool; 3])>)> = vec![];
    let mut penalties: Vec<(u32, usize, [f64; 3])> = vec![];
    for s in &m.supports {
        let Some(fs) = face(&s.at) else {
            warnings.push("A support that is not on a face of the imported mesh was left out.".into());
            continue;
        };
        let mut nrm: std::collections::BTreeMap<usize, [f64; 3]> = std::collections::BTreeMap::new();
        for (_, nv, ns) in points(fs) {
            for (q, w) in ns {
                let v = nrm.entry(q).or_insert([0.0; 3]);
                for c in 0..3 {
                    v[c] += nv[c] * w.abs();
                }
            }
        }
        let mut nodes = vec![];
        for (q, v) in nrm {
            match s.kind {
                SupportKind::Fixed | SupportKind::Pinned => nodes.push((q, [true; 3])),
                SupportKind::Hold { x, y, z } => nodes.push((q, [x, y, z])),
                SupportKind::Roller | SupportKind::Symmetry => {
                    let l = dot(v, v).sqrt().max(1e-300);
                    let nv = v.map(|c| c / l);
                    match (0..3).find(|&c| (nv[c].abs() - 1.0).abs() < 1e-9) {
                        Some(c) => {
                            let mut h = [false; 3];
                            h[c] = true;
                            nodes.push((q, h));
                        }
                        None => penalties.push((s.id, q, nv)),
                    }
                }
                _ => {}
            }
        }
        for (q, h) in &nodes {
            for c in 0..3 {
                held[3 * q + c] |= h[c];
            }
        }
        sup_nodes.push((s.id, nodes));
    }
    for &(_, q, nv) in &penalties {
        for a in 0..3 {
            for b in 0..=a {
                sys.add(3 * q + a, 3 * q + b, kpen * nv[a] * nv[b]);
            }
        }
    }
    let (u, stats) = sys.solve(&held, &vec![0.0; 3 * nn]).map_err(|e| format!("{e} A 3D solid needs holding so it cannot turn about any axis: a fixed face, or rollers on three faces at angles to each other."))?;
    if stats.residual > 1e-8 {
        warnings.push(format!("The supports barely hold the solid (solved only to {:.0e}): it can probably turn or slide.", stats.residual));
    }
    // Reactions and the balance.
    let ku = sys.mul(&u);
    let mut taken = vec![false; 3 * nn];
    let mut reactions = vec![];
    let mut reacted = Tally::default();
    for (sid, nodes) in &sup_nodes {
        let mut r = [0.0; 3];
        for (q, h) in nodes {
            let mut f = [0.0; 3];
            for c in 0..3 {
                if h[c] && !taken[3 * q + c] {
                    taken[3 * q + c] = true;
                    f[c] = ku[3 * q + c] - sys.f[3 * q + c];
                }
            }
            reacted.add(ms.nodes[*q], f);
            for c in 0..3 {
                r[c] += f[c];
            }
        }
        for &(s2, q, nv) in &penalties {
            if s2 == *sid {
                let un = (0..3).map(|c| u[3 * q + c] * nv[c]).sum::<f64>();
                let f = nv.map(|c| -kpen * un * c);
                reacted.add(ms.nodes[q], f);
                for c in 0..3 {
                    r[c] += f[c];
                }
            }
        }
        reactions.push(SolidReaction { support: *sid, f: r });
    }
    // Stresses at each element's nodes, averaged by element volume.
    let refs = node_ref(ms.order);
    let mut acc = vec![[0.0f64; 6]; nn];
    let mut wsum = vec![0.0f64; nn];
    for el in &ms.tets {
        let xyz: Vec<[f64; 3]> = el.iter().map(|&q| ms.nodes[q as usize]).collect();
        let vol = grads(&xyz, ms.order, [0.25; 3]).2.abs() / 6.0;
        for (q, &x) in refs.iter().enumerate() {
            let (gd, _, _) = grads(&xyz, ms.order, x);
            let mut e = [0.0; 6];
            for (i, gi) in gd.iter().enumerate() {
                let ui = [u[3 * el[i] as usize], u[3 * el[i] as usize + 1], u[3 * el[i] as usize + 2]];
                e[0] += gi[0] * ui[0];
                e[1] += gi[1] * ui[1];
                e[2] += gi[2] * ui[2];
                e[3] += gi[1] * ui[0] + gi[0] * ui[1];
                e[4] += gi[2] * ui[1] + gi[1] * ui[2];
                e[5] += gi[2] * ui[0] + gi[0] * ui[2];
            }
            let tr = e[0] + e[1] + e[2];
            let s = [lam * tr + 2.0 * mu * e[0], lam * tr + 2.0 * mu * e[1], lam * tr + 2.0 * mu * e[2], mu * e[3], mu * e[4], mu * e[5]];
            let n = el[q] as usize;
            for r in 0..6 {
                acc[n][r] += s[r] * vol;
            }
            wsum[n] += vol;
        }
    }
    const KEYS: [&str; 14] = ["ux", "uy", "uz", "umag", "sx", "sy", "sz", "txy", "tyz", "tzx", "vm", "s1", "s3", "sf"];
    let mut all: Vec<Vec<f64>> = vec![Vec::with_capacity(nn); KEYS.len()];
    for q in 0..nn {
        let st = stress_set(acc[q].map(|v| v / wsum[q].max(1e-300)), mt.fy);
        let d = [u[3 * q], u[3 * q + 1], u[3 * q + 2]];
        for (c, v) in [d[0], d[1], d[2], dot(d, d).sqrt(), st[0], st[1], st[2], st[3], st[4], st[5], st[6], st[7], st[8], st[9]].iter().enumerate() {
            all[c].push(*v);
        }
    }
    let _ = principal;
    let (surface, ids) = surface(ms);
    let fields = KEYS
        .iter()
        .zip(&all)
        .map(|(key, v)| {
            let (mut imin, mut imax) = (0, 0);
            for i in 0..v.len() {
                if v[i] < v[imin] {
                    imin = i;
                }
                if v[i] > v[imax] {
                    imax = i;
                }
            }
            Field3 { key, min: v[imin], max: v[imax], at_min: ms.nodes[imin], at_max: ms.nodes[imax], v: ids.iter().map(|&n| v[n]).collect() }
        })
        .collect();
    let size = ms.size();
    let umax = (0..nn).map(|q| (u[3 * q].powi(2) + u[3 * q + 1].powi(2) + u[3 * q + 2].powi(2)).sqrt()).fold(0.0, f64::max);
    if umax > 0.1 * size {
        warnings.push(format!("The largest movement is {:.0} % of the solid's size: far beyond small deflections, so this linear answer is not reliable. Check the supports, loads and units.", umax / size * 100.0));
    }
    let reach = ms.nodes.iter().map(|p| dot(*p, *p).sqrt()).fold(0.0, f64::max) + size;
    let scale = [applied.scale, applied.scale * reach];
    let fb = if scale[0] > 0.0 { (0..3).map(|k| (applied.sum[k] + reacted.sum[k]).abs()).fold(0.0, f64::max) / scale[0] } else { 0.0 };
    let mo = if scale[1] > 0.0 { (3..6).map(|k| (applied.sum[k] + reacted.sum[k]).abs()).fold(0.0, f64::max) / scale[1] } else { 0.0 };
    let quality = quality(ms);
    let _ = volume;
    Ok(SolidResults {
        disp: ids.iter().map(|&n| [u[3 * n], u[3 * n + 1], u[3 * n + 2]]).collect(),
        surface,
        fields,
        reactions,
        applied: applied.sum,
        reacted: reacted.sum,
        scale,
        imbalance: fb.max(mo),
        stats,
        warnings,
        elements: ms.tets.len(),
        nodes: nn,
        layers: 0,
        order: ms.order,
        size: mean_edge(ms),
        quality,
        volume: Volume { nodes: ms.nodes.clone(), disp: (0..nn).map(|q| [u[3 * q], u[3 * q + 1], u[3 * q + 2]]).collect(), tets: corner_tets(ms), v: all },
        path: None,
        large: false,
    })
}

/// The outer surface for drawing: the facets (quadratic ones split in four), and the lines where
/// one face meets another. Returns it with the solid's node behind each surface node.
pub fn surface(ms: &TetMesh) -> (Surface, Vec<usize>) {
    let (s, ids, _) = surface_faces(ms);
    (s, ids)
}

/// The surface as `surface` gives it, with the face each drawn triangle belongs to.
fn surface_faces(ms: &TetMesh) -> (Surface, Vec<usize>, Vec<u32>) {
    let mut ids: Vec<usize> = vec![];
    let mut at: HashMap<usize, u32> = HashMap::new();
    let mut idx = |n: usize, ids: &mut Vec<usize>| *at.entry(n).or_insert_with(|| {
        ids.push(n);
        (ids.len() - 1) as u32
    });
    let mut tris = vec![];
    let mut tface = vec![];
    let mut owner: HashMap<(u32, u32), Vec<(usize, [u32; 3])>> = HashMap::new();
    for (fi, fs) in ms.faces.iter().enumerate() {
        for &(e, k) in &fs.facets {
            let f = ms.facet(e as usize, k as usize);
            let parts: Vec<[u32; 3]> = if f.len() == 6 { vec![[f[0], f[3], f[5]], [f[3], f[1], f[4]], [f[5], f[4], f[2]], [f[3], f[4], f[5]]] } else { vec![[f[0], f[1], f[2]]] };
            for p in parts {
                tris.push(p.map(|q| idx(q as usize, &mut ids)));
                tface.push(fi as u32);
            }
            for (a, b, mid) in [(0, 1, 3), (1, 2, 4), (2, 0, 5)] {
                let key = (f[a].min(f[b]), f[a].max(f[b]));
                owner.entry(key).or_default().push((fi, [f[a], f.get(mid).copied().unwrap_or(u32::MAX), f[b]]));
            }
        }
    }
    let mut lines = vec![];
    let mut keys: Vec<_> = owner.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let o = &owner[&key];
        if o.len() == 2 && o[0].0 != o[1].0 {
            let [a, mid, b] = o[0].1;
            if mid == u32::MAX {
                lines.push([idx(a as usize, &mut ids), idx(b as usize, &mut ids)]);
            } else {
                let m = idx(mid as usize, &mut ids);
                lines.push([idx(a as usize, &mut ids), m]);
                lines.push([m, idx(b as usize, &mut ids)]);
            }
        }
    }
    let nodes = ids.iter().map(|&n| ms.nodes[n]).collect();
    (Surface { nodes, tris, lines }, ids, tface)
}

/// What the window needs of an imported mesh to draw it and pick its faces (the whole mesh stays
/// in the engine): the outer surface, the face of each drawn triangle, the faces' names and sizes.
#[derive(Serialize)]
pub struct Preview {
    pub order: u8,
    pub elements: usize,
    pub nodes: usize,
    pub material: u32,
    pub source: String,
    pub faces: Vec<FacePreview>,
    pub surface: Surface,
    pub tface: Vec<u32>,
}
#[derive(Serialize)]
pub struct FacePreview {
    pub name: String,
    pub facets: usize,
    pub area: f64,
}

pub fn preview(ms: &TetMesh) -> Preview {
    let (surface, _, tface) = surface_faces(ms);
    let faces = ms.faces.iter().map(|f| FacePreview { name: f.name.clone(), facets: f.facets.len(), area: f.facets.iter().flat_map(|&(e, k)| facet_points(ms, e as usize, k as usize)).map(|(_, nv, _)| dot(nv, nv).sqrt()).sum() }).collect();
    Preview { order: ms.order, elements: ms.tets.len(), nodes: ms.nodes.len(), material: ms.material, source: ms.source.clone(), faces, surface, tface }
}

/// The model as the window gets it: an imported mesh replaced by its preview.
pub fn for_window(m: &Model) -> serde_json::Value {
    let mut v = serde_json::to_value(m).unwrap_or_default();
    if let (Some(ms), Some(o)) = (&m.volume, v.as_object_mut()) {
        o.insert("volume".into(), serde_json::to_value(preview(ms)).unwrap_or_default());
    }
    v
}

/// The elements as tetrahedra on corner nodes for the cutting plane: quadratic ones split in eight.
fn corner_tets(ms: &TetMesh) -> Vec<[u32; 4]> {
    let mut out = vec![];
    for el in &ms.tets {
        if el.len() == 4 {
            out.push([el[0], el[1], el[2], el[3]]);
            continue;
        }
        let (c0, c1, c2, c3, m01, m12, m20, m03, m13, m23) = (el[0], el[1], el[2], el[3], el[4], el[5], el[6], el[7], el[8], el[9]);
        out.extend([[c0, m01, m20, m03], [m01, c1, m12, m13], [m20, m12, c2, m23], [m03, m13, m23, c3], [m01, m12, m20, m13], [m01, m20, m03, m13], [m20, m12, m23, m13], [m20, m23, m03, m13]]);
    }
    out
}

fn mean_edge(ms: &TetMesh) -> f64 {
    let mut s = 0.0;
    for el in &ms.tets {
        for (a, b) in EDGES {
            let d = sub(ms.nodes[el[a] as usize], ms.nodes[el[b] as usize]);
            s += dot(d, d).sqrt();
        }
    }
    s / (6 * ms.tets.len()).max(1) as f64
}

/// The mesh's quality in the terms the window shows: the smallest angle of any face of any
/// element, the largest ratio of longest edge to the height a regular element would need, and the
/// share of elements with every face angle over 30°.
fn quality(ms: &TetMesh) -> Quality {
    let (mut min_angle, mut max_aspect, mut good) = (180.0f64, 1.0f64, 0usize);
    for el in &ms.tets {
        let p = |i: usize| ms.nodes[el[i] as usize];
        let mut worst = 180.0f64;
        for f in FACES {
            for r in 0..3 {
                let (a, b, c) = (p(f[r]), p(f[(r + 1) % 3]), p(f[(r + 2) % 3]));
                let (u, v) = (sub(b, a), sub(c, a));
                let cosv = dot(u, v) / (dot(u, u) * dot(v, v)).sqrt().max(1e-300);
                worst = worst.min(cosv.clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        min_angle = min_angle.min(worst);
        good += (worst > 30.0) as usize;
        let lmax = EDGES.iter().map(|&(a, b)| { let d = sub(p(a), p(b)); dot(d, d).sqrt() }).fold(0.0, f64::max);
        let v = dot(cross(sub(p(1), p(0)), sub(p(2), p(0))), sub(p(3), p(0))).abs() / 6.0;
        // A regular tetrahedron of edge l has volume l³/(6√2).
        let ideal = lmax.powi(3) / (6.0 * 2f64.sqrt());
        max_aspect = max_aspect.max((ideal / v.max(1e-300)).cbrt());
    }
    Quality { min_angle, max_aspect, good: good as f64 / ms.tets.len().max(1) as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_cube_has_six_faces_and_its_volume() {
        for order in [1, 2] {
            let m = grid([2, 2, 2], order, |p| p).unwrap();
            assert_eq!(m.faces.len(), 6, "{:?}", m.faces.iter().map(|f| f.facets.len()).collect::<Vec<_>>());
            let v: f64 = (0..m.tets.len()).map(|t| m.vol6(t) / 6.0).sum();
            assert!((v - 1.0).abs() < 1e-12, "{v}");
            // Every facet's normal points out of the cube.
            for f in &m.faces {
                for &(e, k) in &f.facets {
                    let ff = m.facet(e as usize, k as usize);
                    let p = |i: usize| m.nodes[ff[i] as usize];
                    let n = cross(sub(p(1), p(0)), sub(p(2), p(0)));
                    let c = [0, 1, 2].map(|j| (p(0)[j] + p(1)[j] + p(2)[j]) / 3.0 - 0.5);
                    assert!(dot(n, c) > 0.0);
                }
            }
        }
    }

    #[test]
    fn shape_functions_sum_to_one_and_reproduce_a_linear_field() {
        let xyz: Vec<[f64; 3]> = node_ref(2).iter().map(|p| [2.0 * p[0] + 0.3 * p[1], p[1] + 0.1, 1.5 * p[2] + 0.2 * p[0]]).collect();
        let (g, n, det) = grads(&xyz, 2, [0.2, 0.3, 0.1]);
        assert!((n.iter().sum::<f64>() - 1.0).abs() < 1e-14 && det > 0.0);
        // d/dx of x is 1, of y is 0.
        let dx: f64 = g.iter().zip(&xyz).map(|(gi, p)| gi[0] * p[0]).sum();
        let dy: f64 = g.iter().zip(&xyz).map(|(gi, p)| gi[0] * p[1]).sum();
        assert!((dx - 1.0).abs() < 1e-12 && dy.abs() < 1e-12);
    }

    #[test]
    fn gmsh_and_inp_files_read_to_the_same_mesh() {
        // One quadratic tetrahedron, with a named face, in both formats.
        let msh = "$MeshFormat\n2.2 0 8\n$EndMeshFormat\n$PhysicalNames\n1\n2 7 \"Base\"\n$EndPhysicalNames\n$Nodes\n10\n1 0 0 0\n2 10 0 0\n3 0 10 0\n4 0 0 10\n5 5 0 0\n6 5 5 0\n7 0 5 0\n8 0 0 5\n9 0 5 5\n10 5 0 5\n$EndNodes\n$Elements\n2\n1 9 2 7 1 1 3 2 7 6 5\n2 11 2 1 1 1 2 3 4 5 6 7 8 9 10\n$EndElements\n";
        let a = read_msh(msh, 1e-3).unwrap();
        let inp = "*NODE\n1, 0, 0, 0\n2, 10, 0, 0\n3, 0, 10, 0\n4, 0, 0, 10\n5, 5, 0, 0\n6, 5, 5, 0\n7, 0, 5, 0\n8, 0, 0, 5\n9, 5, 0, 5\n10, 0, 5, 5\n*ELEMENT, TYPE=C3D10, ELSET=ALL\n1, 1, 2, 3, 4, 5, 6, 7, 8, 9,\n 10\n*SURFACE, NAME=Base\nALL, S1\n";
        let b = read_inp(inp, 1e-3).unwrap();
        assert_eq!((a.order, b.order, a.tets.len(), b.tets.len()), (2, 2, 1, 1));
        assert_eq!(a.faces[0].name, "Base");
        assert_eq!(b.faces[0].name, "Base");
        // The same nodes in the same places.
        let pos = |m: &TetMesh| { let mut v: Vec<[i64; 3]> = m.tets[0].iter().map(|&i| m.nodes[i as usize].map(|c| (c * 1e6).round() as i64)).collect(); v.sort(); v };
        assert_eq!(pos(&a), pos(&b));
        // Each mid-node sits halfway along its edge.
        for m in [&a, &b] {
            for (i, (p, q)) in EDGES.iter().enumerate() {
                let (x, y, z) = (m.nodes[m.tets[0][*p] as usize], m.nodes[m.tets[0][*q] as usize], m.nodes[m.tets[0][4 + i] as usize]);
                assert!((0..3).all(|c| ((x[c] + y[c]) / 2.0 - z[c]).abs() < 1e-12), "edge {i}");
            }
        }
        // 4.1 format: the same tetrahedron.
        let v4 = "$MeshFormat\n4.1 0 8\n$EndMeshFormat\n$Nodes\n1 4 1 4\n3 1 0 4\n1\n2\n3\n4\n0 0 0\n10 0 0\n0 10 0\n0 0 10\n$EndNodes\n$Elements\n1 1 1 1\n3 1 4 1\n1 1 2 3 4\n$EndElements\n";
        let c = read_msh(v4, 1e-3).unwrap();
        assert_eq!((c.order, c.tets.len(), c.faces.len()), (1, 1, 4));
        // What cannot be read says why.
        assert!(read_msh("$MeshFormat\n4.1 1 8\n$EndMeshFormat\n", 1.0).unwrap_err().contains("binary"));
        assert!(read_inp("*NODE\n1,0,0,0\n*ELEMENT, TYPE=C3D8\n1,1,1,1,1,1,1,1,1\n", 1.0).unwrap_err().contains("hexahedra"));
    }
}
