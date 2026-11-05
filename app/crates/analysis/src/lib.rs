//! Datum Mesh analysis of a design's solids (0.7.0).
//!
//! A [`Study`] is what the Simulation workspace sets up: one material, supports and loads on
//! faces named as the modeller names them (so they follow the faces through edits), and an
//! element size. It is saved in the design under [`METADATA_KEY`].
//!
//! Meshing: the solids go to Gmsh (a separate program, GPL, run as its own process) as STEP; it
//! fuses touching solids so they share nodes, marks every surface, and writes quadratic
//! tetrahedra. Each mesh surface is then traced back to the model face it lies on, by the
//! model's own surface triangles, so supports and loads find their facets.
//!
//! Solving: the mesh, the material and the supports and loads become a `datum_core` model of an
//! imported mesh, solved by its verified tetrahedra (benchmark 28 and on).

use datum_core::model::{Analysis, Load, LoadKind, Model, Support, SupportKind, Target};
use datum_core::solids::tet::{FACES, TetMesh};
use serde::{Deserialize, Serialize};
use solidcraft_geom::Mesh;
use std::path::Path;

/// Where a study is kept in the design's metadata.
pub const METADATA_KEY: &str = "com.edgarmendonca.datummesh.study";

/// How a set of faces is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hold {
    /// Held still.
    Fixed,
    /// Free to slide on the face, held across it.
    Roller,
    /// A plane of symmetry: held across it.
    Symmetry,
}

/// What pushes on a set of faces.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Push {
    /// A force (N) shared over the faces by area, along X, Y, Z of the design.
    Force { x: f64, y: f64, z: f64 },
    /// A pressure (Pa) on the faces, pushing into the solid.
    Pressure { p: f64 },
}

/// A face of a solid, by the solid's name and the face's persistent name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FaceRef {
    pub body: String,
    pub face: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupportSet {
    pub id: u32,
    pub hold: Hold,
    pub faces: Vec<FaceRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadSet {
    pub id: u32,
    pub push: Push,
    pub faces: Vec<FaceRef>,
}

/// The analysis set up on a design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Study {
    /// The material's id in the library (`datum_core::materials`).
    pub material: String,
    pub supports: Vec<SupportSet>,
    pub loads: Vec<LoadSet>,
    /// Element size (mm); 0: chosen from the solids' size.
    pub size: f64,
    pub next: u32,
}

impl Default for Study {
    fn default() -> Self {
        Study { material: "steel-1020".into(), supports: vec![], loads: vec![], size: 0.0, next: 1 }
    }
}

impl Study {
    /// The study saved in a design's metadata (or a new one).
    pub fn from_json(v: Option<&serde_json::Value>) -> Study {
        v.and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
    }
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
    /// What is still missing before it can be solved, in words (empty: ready).
    pub fn missing(&self) -> Vec<&'static str> {
        let mut v = vec![];
        if datum_core::materials::get(&self.material).is_none() {
            v.push("a material");
        }
        if self.supports.is_empty() {
            v.push("a support (hold a face)");
        }
        if self.loads.is_empty() {
            v.push("a load (push on a face)");
        }
        v
    }
}

/// The materials offered: (id, name, family).
pub fn materials() -> Vec<(String, String, String)> {
    datum_core::materials::library().into_iter().map(|m| (m.id, m.name, m.family)).collect()
}

/// One solid to mesh: its name, the modelling core's body, its surface triangles with the face of
/// each, and its faces' names.
pub struct Solid<'a> {
    pub name: &'a str,
    pub body: &'a solidcraft_kernel::Body,
    pub surface: &'a Mesh,
    pub face_names: &'a [String],
}

/// A study's mesh: the tetrahedra (m), and for each of its faces the model face it lies on.
pub struct Meshed {
    pub mesh: TetMesh,
    pub owner: Vec<Option<FaceRef>>,
}

/// The element size (mm) for solids this big when none is given: about a twentieth of the
/// largest extent.
pub fn auto_size(solids: &[Solid]) -> f64 {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for s in solids {
        for p in &s.surface.positions {
            for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[k] = lo[k].min(v);
                hi[k] = hi[k].max(v);
            }
        }
    }
    let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0_f64, f64::max);
    if span.is_finite() && span > 0.0 { span / 20.0 } else { 5.0 }
}

/// The Gmsh script: read the solids, fuse touching ones, name every surface `S<tag>`, mesh in
/// quadratic tetrahedra of about `size` (mm) and save as text.
pub fn gmsh_script(step: &str, msh: &str, size: f64) -> String {
    format!(
        r#"SetFactory("OpenCASCADE");
Merge "{step}";
v() = Volume{{:}};
If (#v() > 1)
  BooleanFragments{{ Volume{{:}}; Delete; }}{{}}
EndIf
s() = Surface{{:}};
For i In {{0:#s()-1}}
  Physical Surface(Sprintf("S%g", s(i))) = {{s(i)}};
EndFor
Physical Volume("solid") = Volume{{:}};
Mesh.MeshSizeMax = {size};
Mesh.MeshSizeMin = {min};
Mesh.MeshSizeFromCurvature = 12;
Mesh.ElementOrder = 2;
Mesh.HighOrderOptimize = 1;
Mesh.MshFileVersion = 4.1;
Mesh.Binary = 0;
Mesh 3;
Save "{msh}";
"#,
        min = size / 10.0
    )
}

/// Mesh the solids: STEP out, Gmsh (`gmsh`, a program path), the mesh read back and each of its
/// faces traced to a model face. `work` is a folder for the files passed between them.
pub fn mesh(solids: &[Solid], size: f64, gmsh: &Path, work: &Path) -> Result<Meshed, String> {
    if solids.is_empty() {
        return Err("There is no solid to mesh: model a solid first.".into());
    }
    let size = if size > 0.0 { size } else { auto_size(solids) };
    std::fs::create_dir_all(work).map_err(|e| format!("The mesh folder could not be made: {e}"))?;
    let bodies: Vec<&solidcraft_kernel::Body> = solids.iter().map(|s| s.body).collect();
    let step = solidcraft_kernel::step_export(&bodies, "Datum Mesh").map_err(|e| format!("The solids could not be written for meshing: {e}"))?;
    let (step_path, geo_path, msh_path) = (work.join("model.step"), work.join("mesh.geo"), work.join("mesh.msh"));
    std::fs::write(&step_path, step).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&msh_path);
    let slash = |p: &Path| p.to_string_lossy().replace('\\', "/");
    std::fs::write(&geo_path, gmsh_script(&slash(&step_path), &slash(&msh_path), size)).map_err(|e| e.to_string())?;
    let out = std::process::Command::new(gmsh)
        .arg(&geo_path)
        .arg("-parse_and_exit")
        .output()
        .map_err(|e| format!("The mesher (Gmsh) could not be started from {}: {e}", gmsh.display()))?;
    let text = std::fs::read_to_string(&msh_path).map_err(|_| {
        let log = String::from_utf8_lossy(&out.stderr);
        let last = log.lines().rev().find(|l| l.contains("Error")).unwrap_or("no message");
        format!("The mesher could not mesh these solids ({last}). Try a larger element size, or simplify small details.")
    })?;
    // The design is in millimetres; the solver works in metres.
    let mesh = datum_core::solids::tet::read_msh(&text, 1e-3)?;
    let owner = trace_faces(&mesh, solids);
    Ok(Meshed { mesh, owner })
}

/// The model face each mesh face lies on: by its facets' centres, each matched to the nearest
/// model surface triangle, the face most of them land on.
pub fn trace_faces(mesh: &TetMesh, solids: &[Solid]) -> Vec<Option<FaceRef>> {
    // Model triangles' centres (mm) and faces, in a grid for nearest searches.
    let mut pts: Vec<([f64; 3], usize, u32)> = vec![];
    for (b, s) in solids.iter().enumerate() {
        for (t, tri) in s.surface.triangles.iter().enumerate() {
            let c = tri.iter().fold([0.0; 3], |a, &i| {
                let p = &s.surface.positions[i as usize];
                [a[0] + p.x / 3.0, a[1] + p.y / 3.0, a[2] + p.z / 3.0]
            });
            pts.push((c, b, s.surface.tri_face.get(t).copied().unwrap_or(0)));
        }
    }
    let nearest = |q: [f64; 3]| -> Option<(usize, u32)> {
        pts.iter().min_by(|a, b| d2(a.0, q).total_cmp(&d2(b.0, q))).map(|p| (p.1, p.2))
    };
    mesh.faces
        .iter()
        .map(|f| {
            let mut votes: std::collections::HashMap<(usize, u32), usize> = Default::default();
            // A sample of each face's facets is enough to tell which model face it is.
            let step = (f.facets.len() / 24).max(1);
            for &(e, k) in f.facets.iter().step_by(step) {
                let el = &mesh.tets[e as usize];
                let c = FACES[k as usize].iter().fold([0.0; 3], |a, &i| {
                    let p = mesh.nodes[el[i] as usize];
                    [a[0] + p[0] * 1e3 / 3.0, a[1] + p[1] * 1e3 / 3.0, a[2] + p[2] * 1e3 / 3.0]
                });
                if let Some(hit) = nearest(c) {
                    *votes.entry(hit).or_default() += 1;
                }
            }
            let ((b, face), _) = votes.into_iter().max_by_key(|x| x.1)?;
            let s = solids.get(b)?;
            Some(FaceRef { body: s.name.to_string(), face: s.face_names.get(face as usize)?.clone() })
        })
        .collect()
}

fn d2(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// The engine's model of the study on its mesh.
pub fn model(meshed: &Meshed, study: &Study) -> Result<Model, String> {
    let lib = datum_core::materials::get(&study.material).ok_or("Choose a material (step 1).")?;
    let mut m = Model::new(Analysis::Solid);
    m.name = "Study".into();
    let mat = lib.to_material(1);
    m.materials = vec![mat];
    let mut mesh = meshed.mesh.clone();
    mesh.material = 1;
    m.volume = Some(Box::new(mesh));
    let mut next = 100;
    let faces_of = |faces: &[FaceRef]| -> Vec<u32> {
        meshed.owner.iter().enumerate().filter(|(_, o)| o.as_ref().is_some_and(|o| faces.contains(o))).map(|(i, _)| i as u32).collect()
    };
    for s in &study.supports {
        let kind = match s.hold {
            Hold::Fixed => SupportKind::Fixed,
            Hold::Roller => SupportKind::Roller,
            Hold::Symmetry => SupportKind::Symmetry,
        };
        let on = faces_of(&s.faces);
        if on.is_empty() {
            return Err("A support's faces are no longer on the model: pick them again.".into());
        }
        for face in on {
            m.supports.push(Support { id: next, at: Target::Face { face }, kind: kind.clone() });
            next += 1;
        }
    }
    for l in &study.loads {
        let on = faces_of(&l.faces);
        if on.is_empty() {
            return Err("A load's faces are no longer on the model: pick them again.".into());
        }
        let area = |f: u32| face_area(&meshed.mesh, f as usize);
        let total: f64 = on.iter().map(|&f| area(f)).sum::<f64>().max(1e-300);
        for face in on {
            let kind = match l.push {
                Push::Force { x, y, z } => {
                    let share = area(face) / total;
                    LoadKind::Traction { fx: x * share, fy: y * share, fz: z * share, total: true }
                }
                Push::Pressure { p } => LoadKind::Pressure { p },
            };
            m.loads.push(Load { case: 0, id: next, at: Target::Face { face }, kind });
            next += 1;
        }
    }
    m.next = next;
    Ok(m)
}

/// A mesh face's area (m²), by its facets' corners.
pub fn face_area(mesh: &TetMesh, f: usize) -> f64 {
    let Some(face) = mesh.faces.get(f) else { return 0.0 };
    face.facets
        .iter()
        .map(|&(e, k)| {
            let el = &mesh.tets[e as usize];
            let [a, b, c] = FACES[k as usize].map(|i| mesh.nodes[el[i] as usize]);
            let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            0.5 * ((u[1] * v[2] - u[2] * v[1]).powi(2) + (u[2] * v[0] - u[0] * v[2]).powi(2) + (u[0] * v[1] - u[1] * v[0]).powi(2)).sqrt()
        })
        .sum()
}

/// Solve the study on its mesh: the engine's results for a 3D solid (surface, movements, fields).
pub fn solve(meshed: &Meshed, study: &Study) -> Result<datum_core::solid::SolidResults, String> {
    let m = model(meshed, study)?;
    match datum_core::solve_for(&m, None)? {
        datum_core::Results::Solid(r) => Ok(r),
        _ => Err("The solver returned something other than a solid's results.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_study_round_trips_through_the_design_metadata_and_says_what_is_missing() {
        let mut s = Study::default();
        assert_eq!(s.missing(), vec!["a support (hold a face)", "a load (push on a face)"]);
        s.supports.push(SupportSet { id: 1, hold: Hold::Fixed, faces: vec![FaceRef { body: "Body1".into(), face: "F1:back".into() }] });
        s.loads.push(LoadSet { id: 2, push: Push::Force { x: 0.0, y: 0.0, z: -1000.0 }, faces: vec![FaceRef { body: "Body1".into(), face: "F2:front".into() }] });
        assert!(s.missing().is_empty());
        assert_eq!(Study::from_json(Some(&s.to_json())), s);
        assert_eq!(Study::from_json(None), Study::default());
        assert!(materials().iter().any(|m| m.0 == "steel-1020"));
    }

    #[test]
    fn the_gmsh_script_names_every_surface_and_asks_for_quadratic_tetrahedra() {
        let g = gmsh_script("a/model.step", "a/mesh.msh", 4.0);
        assert!(g.contains("Merge \"a/model.step\";") && g.contains("Physical Surface(Sprintf(\"S%g\", s(i)))") && g.contains("Mesh.ElementOrder = 2;") && g.contains("Mesh.MeshSizeMax = 4;") && g.contains("Save \"a/mesh.msh\";"));
    }
}
