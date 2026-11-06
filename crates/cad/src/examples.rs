//! Worked examples modelled with the geometry engine (0.7.4): each a short recipe of modelling
//! commands (as the window sends them), then the analysis: the material, the steps, and supports
//! and loads put on the mesh's faces found by where they are, once Gmsh has meshed the solids.

use crate::Cad;
use datum_core::model::{Analysis, Load, LoadKind, Model, Step, StepKind, Support, SupportKind, Target};
use datum_core::solids::tet::{TetMesh, FACES};
use serde_json::{json, Value};

/// One modelled example, listed on File › Examples under `group`.
pub struct Modelled {
    pub id: &'static str,
    pub name: &'static str,
    pub what: &'static str,
    pub group: &'static str,
}

/// The faces a support or load goes on, by where they are (model axes, m, Y up).
#[derive(Clone, Copy)]
pub enum Pick {
    /// Every face whose nodes all have this coordinate (axis 0, 1, 2 = x, y, z).
    Plane(usize, f64),
    /// Every face whose nodes all lie at this distance from the Y axis.
    RadiusY(f64),
}

/// What goes on the faces picked.
#[derive(Clone)]
pub enum Put {
    Support(SupportKind),
    Load(LoadKind),
}

/// An example's analysis: the model (material, steps) and what goes where.
pub struct Setup {
    pub model: Model,
    pub places: Vec<(Pick, Put)>,
    /// The element size (mm) to mesh with.
    pub size: f64,
}

pub fn list() -> Vec<Modelled> {
    vec![
        Modelled { id: "m-bracket", name: "Bracket with a hole, modelled", what: "A plate bracket 80 × 50 × 10 mm with a bolt hole, modelled (sketch, extrude), held at one end and pushed 2 kN down at the other: the stress gathers at the root and round the hole.", group: "3D solids and assemblies" },
        Modelled { id: "m-shaft", name: "Stepped shaft, modelled", what: "A shaft with a shoulder, revolved from its half-section, held at one end and bent at the other: the stress gathers at the shoulder.", group: "3D solids and assemblies" },
        Modelled { id: "m-tubecrush", name: "Tube crushed until it collapses", what: "An aluminium tube (Ø40 × 2 mm, 60 mm tall) pushed down 6 mm by a platen: it yields, its wall folds, and the force peaks at the squash load σy A and then falls. Solving takes a few minutes.", group: "Nonlinear: large deflection, yielding, rubber" },
        Modelled { id: "m-tuning", name: "Plate ringing: natural frequencies", what: "A steel strip held at one end, modelled and meshed: its first natural frequencies and mode shapes, against beam theory for the first.", group: "Dynamics" },
    ]
}

fn run(c: &mut Cad, id: &str, p: Value) -> Result<(), String> {
    c.run(id, &p).map(|_| ()).map_err(|e| format!("{id}: {e}"))
}

fn material(m: &mut Model, id: &str, hardening: f64) -> Result<(), String> {
    let mut mt = datum_core::materials::get(id).ok_or("material")?.to_material(1);
    mt.hardening = hardening;
    m.materials = vec![mt];
    Ok(())
}

/// The design and the analysis of an example.
pub fn build(id: &str) -> Result<(Cad, Setup), String> {
    let mut c = Cad::new();
    let mut m = Model::new(Analysis::Solid);
    m.name = list().into_iter().find(|e| e.id == id).ok_or("That example does not exist.")?.name.to_string();
    let mut places = vec![];
    let size = match id {
        "m-bracket" => {
            // 80 × 50 plate, 10 thick, a 16 mm hole near the free end.
            run(&mut c, "sketch.create", json!({"plane": "XY"}))?;
            run(&mut c, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [80, 50]}))?;
            run(&mut c, "sketch.circle.center", json!({"center": [60, 25], "radius": 8}))?;
            run(&mut c, "sketch.finish", json!({}))?;
            run(&mut c, "solid.extrude", json!({"distance": 10.0, "direction": "positive"}))?;
            material(&mut m, "steel-1020", 0.0)?;
            // Held at x = 0; 2 kN down (the model's −y) spread over the end face at x = 80 mm.
            places.push((Pick::Plane(0, 0.0), Put::Support(SupportKind::Fixed)));
            places.push((Pick::Plane(0, 0.08), Put::Load(LoadKind::Traction { fx: 0.0, fy: -2000.0, fz: 0.0, total: true })));
            4.0
        }
        "m-shaft" => {
            // Half-section in the XZ plane, revolved about Z: Ø40 for 60 mm, then Ø25 for 60 mm.
            run(&mut c, "sketch.create", json!({"plane": "XZ"}))?;
            run(&mut c, "sketch.line", json!({"points": [[0, 0], [20, 0], [20, -60], [12.5, -60], [12.5, -120], [0, -120], [0, 0]]}))?;
            run(&mut c, "sketch.finish", json!({}))?;
            run(&mut c, "solid.revolve", json!({"axis": "y", "angle": 360}))?;
            material(&mut m, "steel-1020", 0.0)?;
            // The design's Z is the model's y: held at the wide end (y = 0), bent at the narrow end.
            places.push((Pick::Plane(1, 0.0), Put::Support(SupportKind::Fixed)));
            places.push((Pick::Plane(1, 0.12), Put::Load(LoadKind::Traction { fx: 1000.0, fy: 0.0, fz: 0.0, total: true })));
            4.0
        }
        "m-tubecrush" => {
            // Radius 20 mm, wall 2 mm, 60 mm tall.
            run(&mut c, "sketch.create", json!({"plane": "XY"}))?;
            run(&mut c, "sketch.circle.center", json!({"center": [0, 0], "radius": 20}))?;
            run(&mut c, "sketch.circle.center", json!({"center": [0, 0], "radius": 18}))?;
            run(&mut c, "sketch.finish", json!({}))?;
            run(&mut c, "solid.extrude", json!({"distance": 60.0, "direction": "positive"}))?;
            material(&mut m, "al-6061-t6", 0.5e9)?;
            places.push((Pick::Plane(1, 0.0), Put::Support(SupportKind::Fixed)));
            places.push((Pick::Plane(1, 0.06), Put::Support(SupportKind::Hold { x: true, y: false, z: true })));
            // Crushed 6 mm (a tenth of its length) in 24 increments: the peak at the squash load
            // σy A, then the force falls as the wall folds. Further on, the folds press on each
            // other, which needs contact (0.8).
            places.push((Pick::Plane(1, 0.06), Put::Load(LoadKind::Settlement { dx: 0.0, dy: -0.006, dz: 0.0 })));
            m.steps.push(Step { id: 90, name: "Crush 6 mm".into(), kind: StepKind::LargePlastic, increments: 24, ..Default::default() });
            4.0
        }
        "m-tuning" => {
            // A steel strip 150 × 20 × 5 mm, held at x = 0.
            run(&mut c, "sketch.create", json!({"plane": "XY"}))?;
            run(&mut c, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [150, 20]}))?;
            run(&mut c, "sketch.finish", json!({}))?;
            run(&mut c, "solid.extrude", json!({"distance": 5.0, "direction": "positive"}))?;
            material(&mut m, "steel-1020", 0.0)?;
            places.push((Pick::Plane(0, 0.0), Put::Support(SupportKind::Fixed)));
            3.0
        }
        _ => return Err("That example does not exist.".into()),
    };
    Ok((c, Setup { model: m, places, size }))
}

/// The mesh faces a pick finds.
pub fn faces(ms: &TetMesh, pick: Pick) -> Vec<u32> {
    let tol = 1e-7;
    let all = |f: &datum_core::solids::tet::FaceSet, ok: &dyn Fn([f64; 3]) -> bool| f.facets.iter().all(|&(e, k)| FACES[k as usize].iter().all(|&i| ok(ms.nodes[ms.tets[e as usize][i] as usize])));
    ms.faces
        .iter()
        .enumerate()
        .filter(|(_, f)| match pick {
            Pick::Plane(a, v) => all(f, &|p| (p[a] - v).abs() < tol),
            Pick::RadiusY(r) => all(f, &|p| ((p[0] * p[0] + p[2] * p[2]).sqrt() - r).abs() < 1e-5),
        })
        .map(|(i, _)| i as u32)
        .collect()
}

/// The example's model on its mesh: the mesh as the solid and each support and load on its faces.
pub fn place(setup: &Setup, ms: TetMesh) -> Result<Model, String> {
    let mut m = setup.model.clone();
    let mut v = ms;
    v.material = m.materials[0].id;
    let mut next = m.next.max(100);
    for (pick, put) in &setup.places {
        let on = faces(&v, *pick);
        if on.is_empty() {
            return Err("An example's support or load found no face to go on.".into());
        }
        for face in on {
            match put {
                Put::Support(k) => m.supports.push(Support { id: next, at: Target::Face { face }, kind: k.clone() }),
                Put::Load(k) => m.loads.push(Load { case: 0, id: next, at: Target::Face { face }, kind: k.clone() }),
            }
            next += 1;
        }
    }
    m.volume = Some(Box::new(v));
    m.next = next;
    m.check()?;
    Ok(m)
}
