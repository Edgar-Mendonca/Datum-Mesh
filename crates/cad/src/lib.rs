//! The geometry engine behind Datum Mesh's window (0.7.1): a headless session of SolidCraft's
//! modelling engine (MIT or Apache-2.0, `app/`), driven by its JSON commands
//! (`sketch.circle.three_point`, `solid.extrude`, …), and the scene the window draws from it.
//! The window never sees the engine's types: only [`Scene`], in millimetres, as the design is.

use serde::Serialize;
use serde_json::Value;
use solidcraft_engine::Session;

/// One design being modelled.
pub struct Cad {
    session: Session,
}

/// A solid as the 3D view draws it: triangles with the face each one belongs to, and the edges.
#[derive(Serialize)]
pub struct Body {
    pub name: String,
    /// x, y, z of each vertex (mm).
    pub pos: Vec<f32>,
    /// The surface's normal at each vertex.
    pub nrm: Vec<f32>,
    /// Three vertices per triangle.
    pub tris: Vec<u32>,
    /// The face of each triangle (its index in the solid).
    pub face: Vec<u32>,
    /// Each edge as a polyline: x, y, z after one another (mm).
    pub edges: Vec<Vec<f32>>,
    pub closed: bool,
}

/// A curve of a sketch: its points in the sketch's plane (mm).
#[derive(Serialize)]
pub struct SketchCurve {
    pub id: String,
    pub pts: Vec<[f64; 2]>,
    pub construction: bool,
}

/// A sketch: its plane (origin and in-plane axes, world mm) and its curves.
#[derive(Serialize)]
pub struct Sketch {
    pub id: u64,
    pub name: String,
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub curves: Vec<SketchCurve>,
    /// Closed regions the sketch makes (what Extrude and Revolve can use).
    pub profiles: usize,
    /// Whether the sketch is fully defined, says what is still free, or that it failed.
    pub status: String,
}

/// What the window draws and lists.
#[derive(Serialize)]
pub struct Scene {
    pub bodies: Vec<Body>,
    pub sketches: Vec<Sketch>,
    /// The sketch being edited, if any.
    pub active: Option<u64>,
    pub undo: bool,
    pub redo: bool,
}

impl Default for Cad {
    fn default() -> Self {
        Self::new()
    }
}

impl Cad {
    /// An empty design.
    pub fn new() -> Self {
        Cad { session: Session::new(solidcraft_doc::Document::default()) }
    }

    /// Runs one engine command; its reply, or the engine's message for a user.
    pub fn run(&mut self, id: &str, params: &Value) -> Result<Value, String> {
        self.session.execute(id, params).map_err(|e| e.to_string())
    }

    /// The design as the window draws it.
    pub fn scene(&self) -> Result<Scene, String> {
        let st = self.session.world_state().map_err(|e| e.to_string())?;
        let bodies = st
            .bodies
            .iter()
            .map(|b| {
                let m = b.mesh();
                Body {
                    name: b.name.clone(),
                    pos: m.positions.iter().flat_map(|p| [p.x as f32, p.y as f32, p.z as f32]).collect(),
                    nrm: m.normals.iter().flat_map(|p| [p.x as f32, p.y as f32, p.z as f32]).collect(),
                    tris: m.triangles.iter().flatten().copied().collect(),
                    face: m.tri_face.clone(),
                    edges: m.edges.iter().map(|e| e.iter().flat_map(|p| [p.x as f32, p.y as f32, p.z as f32]).collect()).collect(),
                    closed: b.is_closed(),
                }
            })
            .collect();
        let sketches = st
            .sketches
            .iter()
            .map(|s| {
                let pl = &s.plane;
                Sketch {
                    id: s.feature,
                    name: s.name.clone(),
                    origin: [pl.origin.x, pl.origin.y, pl.origin.z],
                    x: [pl.x.x, pl.x.y, pl.x.z],
                    y: [pl.y.x, pl.y.y, pl.y.z],
                    curves: (0..s.sketch.curves.len())
                        .map(|i| SketchCurve {
                            id: s.sketch.curves[i].id.clone(),
                            pts: s.sketch.polyline(i).iter().map(|p| [p.x, p.y]).collect(),
                            construction: s.sketch.curves[i].construction,
                        })
                        .collect(),
                    profiles: s.profiles.len(),
                    status: if s.report.dof == 0 { format!("{:?}", s.report.status) } else { format!("{:?}, {} free", s.report.status, s.report.dof) },
                }
            })
            .collect();
        Ok(Scene { bodies, sketches, active: self.session.active_sketch, undo: !self.session.undo.is_empty(), redo: !self.session.redo.is_empty() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(c: &mut Cad, id: &str, p: Value) -> Value {
        c.run(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    /// Every way of drawing the ribbon offers makes curves in the sketch; a closed profile
    /// extrudes into one closed solid, drawn with its faces and edges.
    #[test]
    fn each_draw_tool_and_extrude() {
        let mut c = Cad::new();
        run(&mut c, "sketch.create", json!({"plane": "XY"}));
        let tools = [
            ("sketch.line", json!({"points": [[0, 0], [10, 0], [10, 5]]})),
            ("sketch.arc.three_point", json!({"start": [20, 0], "end": [30, 0], "through": [25, 4]})),
            ("sketch.arc.center_point", json!({"center": [40, 0], "start": [45, 0], "end": [40, 5]})),
            ("sketch.circle.center", json!({"center": [0, 30], "radius": 5})),
            ("sketch.circle.two_point", json!({"p0": [15, 30], "p1": [25, 30]})),
            ("sketch.circle.three_point", json!({"p0": [30, 30], "p1": [40, 30], "p2": [35, 35]})),
            ("sketch.rectangle.center", json!({"center": [0, 60], "corner": [5, 64]})),
            ("sketch.rectangle.three_point", json!({"p0": [20, 60], "p1": [30, 60], "p2": [30, 66]})),
            ("sketch.polygon.inscribed", json!({"center": [50, 60], "radius": 6, "sides": 6})),
            ("sketch.polygon.circumscribed", json!({"center": [70, 60], "radius": 6, "sides": 8})),
            ("sketch.polygon.edge", json!({"p0": [90, 60], "p1": [96, 60], "sides": 5})),
            ("sketch.slot.center_to_center", json!({"p0": [0, 90], "p1": [20, 90], "width": 6})),
            ("sketch.slot.overall", json!({"p0": [30, 90], "p1": [50, 90], "width": 6})),
        ];
        let mut before = 0;
        for (id, p) in tools {
            run(&mut c, id, p);
            let sc = c.scene().unwrap();
            let n = sc.sketches.iter().map(|s| s.curves.len()).sum::<usize>();
            assert!(n > before, "{id} drew nothing");
            before = n;
        }
        run(&mut c, "sketch.finish", json!({}));

        let mut c = Cad::new();
        run(&mut c, "sketch.create", json!({"plane": "XY"}));
        run(&mut c, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [40, 20]}));
        run(&mut c, "sketch.finish", json!({}));
        run(&mut c, "solid.extrude", json!({"distance": 10.0}));
        let sc = c.scene().unwrap();
        assert_eq!(sc.bodies.len(), 1);
        let b = &sc.bodies[0];
        assert!(b.closed && b.tris.len() >= 36 && b.tris.len() == b.face.len() * 3 && b.pos.len() % 3 == 0);
        assert!(b.face.iter().max() == Some(&5), "six faces on a block");
        assert!(sc.undo && sc.active.is_none());
        let zmax = b.pos.chunks(3).map(|p| p[2]).fold(f32::MIN, f32::max);
        assert!((zmax - 10.0).abs() < 1e-4, "extruded 10 mm, top at {zmax}");
    }
}
