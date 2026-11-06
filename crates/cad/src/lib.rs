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
    /// line, circle, arc, ellipse, spline or conic.
    pub kind: &'static str,
    /// Its points' ids: a line's and an arc's ends ("start", "end"), a circle's or arc's centre.
    pub start: Option<String>,
    pub end: Option<String>,
    pub center: Option<String>,
    /// Fully fixed by its dimensions and constraints.
    pub fixed: bool,
}

/// A point of a sketch (an end or a centre): what a click on the sheet snaps to and picks.
#[derive(Serialize)]
pub struct SketchPoint {
    pub id: String,
    pub at: [f64; 2],
    pub fixed: bool,
}

/// A dimension or constraint, placed by what it ties: a label for the sheet.
#[derive(Serialize)]
pub struct SketchMark {
    pub id: String,
    /// The engine's name for it (Distance, Radius, Parallel, Tangent…).
    pub name: String,
    /// A dimension's value: mm, or degrees for an angle; none for a constraint.
    pub value: Option<f64>,
    pub at: [f64; 2],
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
    pub points: Vec<SketchPoint>,
    pub marks: Vec<SketchMark>,
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

    /// The design as it is kept in a .dmesh file (its `geometry`).
    pub fn to_json(&self) -> Result<Value, String> {
        serde_json::to_value(&*self.session.doc).map_err(|e| e.to_string())
    }

    /// A design read back from a .dmesh file's `geometry`.
    pub fn from_json(v: Value) -> Result<Self, String> {
        let doc: solidcraft_doc::Document = serde_json::from_value(v).map_err(|e| format!("The geometry in this file could not be read: {e}"))?;
        Ok(Cad { session: Session::new(doc) })
    }

    /// Whether the design has anything in it (a file without geometry stays as it was).
    pub fn is_empty(&self) -> bool {
        self.scene().map(|s| s.sketches.is_empty() && s.bodies.is_empty()).unwrap_or(true)
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
                    curves: (0..s.sketch.curves.len()).map(|i| curve(&s.sketch, &s.report, i)).collect(),
                    points: s.sketch.points.iter().enumerate().map(|(i, p)| SketchPoint { id: p.id.clone(), at: [p.pos.x, p.pos.y], fixed: s.report.point_determined.get(i).copied().unwrap_or(false) }).collect(),
                    marks: s.sketch.constraints.iter().filter_map(|c| mark(&s.sketch, c)).collect(),
                    profiles: s.profiles.len(),
                    status: if s.report.dof == 0 { format!("{:?}", s.report.status) } else { format!("{:?}, {} free", s.report.status, s.report.dof) },
                }
            })
            .collect();
        Ok(Scene { bodies, sketches, active: self.session.active_sketch, undo: !self.session.undo.is_empty(), redo: !self.session.redo.is_empty() })
    }
}

/// A curve as the sheet draws and picks it.
fn curve(sk: &solidcraft_sketch::Sketch, rep: &solidcraft_sketch::SolveReport, i: usize) -> SketchCurve {
    use solidcraft_sketch::CurveKind as K;
    let c = &sk.curves[i];
    let id = |k: usize| sk.points.get(k).map(|p| p.id.clone());
    let (kind, start, end, center) = match &c.kind {
        K::Line { a, b } => ("line", id(*a), id(*b), None),
        K::Circle { c, .. } => ("circle", None, None, id(*c)),
        K::Arc { c, a, b } => ("arc", id(*a), id(*b), id(*c)),
        K::Ellipse { c, .. } => ("ellipse", None, None, id(*c)),
        K::Spline { .. } => ("spline", None, None, None),
        K::Conic { a, b, .. } => ("conic", id(*a), id(*b), None),
    };
    SketchCurve {
        id: c.id.clone(),
        pts: sk.polyline(i).iter().map(|p| [p.x, p.y]).collect(),
        construction: c.construction,
        kind,
        start,
        end,
        center,
        fixed: rep.curve_determined.get(i).copied().unwrap_or(false),
    }
}

/// Where a dimension or constraint is labelled: amid the points (`p`, `q`) and curves (`l`, `c`,
/// `a`, `b`) it ties, as the engine's own record of it names them; its value for a dimension.
fn mark(sk: &solidcraft_sketch::Sketch, c: &solidcraft_sketch::Constraint) -> Option<SketchMark> {
    let v = serde_json::to_value(&c.kind).ok()?;
    let o = v.as_object()?;
    let name = c.kind.name().to_string();
    if name.starts_with("Pattern") {
        return None;
    }
    let mut at: Vec<[f64; 2]> = Vec::new();
    for (k, x) in o {
        let Some(i) = x.as_u64().map(|i| i as usize) else { continue };
        match k.as_str() {
            "p" | "q" => {
                if let Some(p) = sk.points.get(i) {
                    at.push([p.pos.x, p.pos.y]);
                }
            }
            "l" | "c" | "a" | "b" => {
                let pl = sk.polyline(i);
                if let Some(m) = pl.get(pl.len() / 2) {
                    at.push([m.x, m.y]);
                }
            }
            _ => {}
        }
    }
    if at.is_empty() {
        return None;
    }
    let n = at.len() as f64;
    let mid = [at.iter().map(|p| p[0]).sum::<f64>() / n, at.iter().map(|p| p[1]).sum::<f64>() / n];
    let value = o.get("value").and_then(Value::as_f64).map(|x| if name.contains("Angle") || name.contains("Sweep") { x.to_degrees() } else { x });
    Some(SketchMark { id: c.id.clone(), name, value, at: mid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(c: &mut Cad, id: &str, p: Value) -> Value {
        c.run(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    /// A design written to JSON and read back draws the same solid.
    #[test]
    fn kept_in_the_file() {
        let mut c = Cad::new();
        run(&mut c, "sketch.create", json!({"plane": "XY"}));
        run(&mut c, "sketch.circle.center", json!({"center": [0, 0], "radius": 10}));
        run(&mut c, "sketch.finish", json!({}));
        run(&mut c, "solid.extrude", json!({"distance": 5.0}));
        let v = c.to_json().unwrap();
        let d = Cad::from_json(serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap()).unwrap();
        let (a, b) = (c.scene().unwrap(), d.scene().unwrap());
        assert_eq!(a.bodies.len(), 1);
        assert_eq!(a.bodies[0].tris.len(), b.bodies[0].tris.len());
        assert_eq!(a.sketches.len(), b.sketches.len());
        assert!(!d.is_empty() && Cad::new().is_empty());
    }

    /// Every 3D tool, with the inputs the window sends (points picked on faces and edges, in mm).
    #[test]
    fn each_3d_tool() {
        let block = |c: &mut Cad| {
            run(c, "sketch.create", json!({"plane": "XY"}));
            run(c, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [40, 20]}));
            run(c, "sketch.finish", json!({}));
            run(c, "solid.extrude", json!({"distance": 10.0, "direction": "positive"}));
        };
        let faces = |c: &Cad| c.scene().unwrap().bodies.iter().map(|b| b.face.iter().max().map_or(0, |m| m + 1)).sum::<u32>();
        let mut c = Cad::new();
        block(&mut c);
        assert_eq!(faces(&c), 6);
        run(&mut c, "solid.hole", json!({"position": [20.0, 10.0, 10.0], "diameter": 6}));
        assert_eq!(faces(&c), 7, "a hole through adds its wall");
        run(&mut c, "solid.fillet", json!({"edges": [[20.0, 0.0, 10.0]], "radius": 2}));
        run(&mut c, "solid.chamfer", json!({"edges": [[20.0, 20.0, 10.0]], "distance": 1.5}));
        assert_eq!(faces(&c), 9, "a fillet and a chamfer each add a face");
        run(&mut c, "solid.press_pull", json!({"face": [0.0, 10.0, 5.0], "distance": 5}));
        let xmin = c.scene().unwrap().bodies[0].pos.chunks(3).map(|p| p[0]).fold(f32::MAX, f32::min);
        assert!((xmin + 5.0).abs() < 1e-3, "press pull moved the end face out by 5: {xmin}");
        // Sketch on the top face, a circle, extrude a boss from it.
        run(&mut c, "sketch.create", json!({"plane": {"face": [5.0, 15.0, 10.0]}}));
        let sk = c.scene().unwrap().sketches.last().unwrap().origin;
        assert!((sk[2] - 10.0).abs() < 1e-6, "the sketch lies on the top face: {sk:?}");
        run(&mut c, "sketch.finish", json!({}));
        let mut s = Cad::new();
        block(&mut s);
        run(&mut s, "solid.shell", json!({"faces": [[20.0, 10.0, 10.0]], "thickness": 2, "body": "Body1"}));
        assert!(faces(&s) > 6, "a shell opens the top and adds inner walls");
        let mut a = Cad::new();
        block(&mut a);
        run(&mut a, "solid.move", json!({"bodies": ["Body1"], "translate": [0, 30, 0], "copy": true}));
        assert_eq!(a.scene().unwrap().bodies.len(), 2, "a copy");
        run(&mut a, "solid.mirror", json!({"bodies": ["Body1"], "plane": "YZ", "combine": false}));
        assert_eq!(a.scene().unwrap().bodies.len(), 3, "a mirror copy");
        run(&mut a, "solid.pattern.rectangular", json!({"bodies": ["Body1"], "dir1": [1, 0, 0], "count1": 3, "spacing1": 60}));
        assert_eq!(a.scene().unwrap().bodies.len(), 5, "two more in a row");
        let mut r = Cad::new();
        block(&mut r);
        run(&mut r, "solid.pattern.circular", json!({"bodies": ["Body1"], "axis": "Z", "count": 4, "angle": 360}));
        assert_eq!(r.scene().unwrap().bodies.len(), 4, "four round the Z axis");
        let reply = run(&mut r, "construct.plane.offset", json!({"base": "XY", "offset": 30}));
        let name = reply.get("name").or(reply.get("plane")).or(reply.get("id")).cloned().unwrap_or(Value::Null);
        assert!(!name.is_null(), "the plane's name comes back: {reply}");
        run(&mut r, "sketch.create", json!({"plane": name}));
        let o = r.scene().unwrap().sketches.last().unwrap().origin;
        assert!((o[2] - 30.0).abs() < 1e-6, "the sketch lies on the offset plane: {o:?}");
    }

    /// Every editing tool, with the inputs the window sends, is accepted and changes the sketch.
    #[test]
    fn each_edit_tool() {
        let mut c = Cad::new();
        run(&mut c, "sketch.create", json!({"plane": "XY"}));
        run(&mut c, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [60, 40]}));
        let sk = |c: &Cad| c.scene().unwrap().sketches.into_iter().next().unwrap();
        let s = sk(&c);
        let lines: Vec<String> = s.curves.iter().filter(|k| k.kind == "line").map(|k| k.id.clone()).collect();
        assert_eq!(lines.len(), 4, "a rectangle is four lines");
        let (l0, l1, l2) = (lines[0].clone(), lines[1].clone(), lines[2].clone());
        // Dimension: a line's length, typed.
        run(&mut c, "sketch.dimension", json!({"entities": [l0], "value": 80}));
        let s = sk(&c);
        assert!(s.marks.iter().any(|m| m.value.is_some_and(|v| (v - 80.0).abs() < 1e-6)), "the dimension is listed with its value");
        // Fillet between two sides, then a chamfer on the opposite corner.
        run(&mut c, "sketch.fillet", json!({"a": l0, "b": l1, "radius": 5}));
        assert!(sk(&c).curves.iter().any(|k| k.kind == "arc"), "a fillet adds an arc");
        run(&mut c, "sketch.chamfer.equal_distance", json!({"a": lines[2], "b": lines[3], "distance": 4}));
        // Constraints between curves.
        let circ = |c: &mut Cad, x: f64| { run(c, "sketch.circle.center", json!({"center": [x, 20], "radius": 5})); sk(c).curves.last().unwrap().id.clone() };
        let c1 = circ(&mut c, 20.0);
        let c2 = circ(&mut c, 45.0);
        run(&mut c, "sketch.constraint.equal", json!({"a": c1, "b": c2}));
        run(&mut c, "sketch.constraint.fix", json!({"entity": c1}));
        let before = sk(&c).curves.len();
        // Offset a side outwards; mirror a circle in a line; copy it round; in rows; construction; delete.
        run(&mut c, "sketch.offset", json!({"curves": [l2], "distance": 5, "side": [30, 60]}));
        assert!(sk(&c).curves.len() > before, "an offset adds curves");
        let n = sk(&c).curves.len();
        run(&mut c, "sketch.line", json!({"points": [[0, -20], [80, -20]]}));
        let ml = sk(&c).curves.last().unwrap().id.clone();
        run(&mut c, "sketch.mirror", json!({"entities": [c2.clone()], "line": ml}));
        run(&mut c, "sketch.pattern.circular", json!({"entities": [c2.clone()], "center": [40, 20], "count": 4, "angle": 360}));
        run(&mut c, "sketch.pattern.rectangular", json!({"entities": [c1.clone()], "count": 3, "spacing": 12}));
        assert!(sk(&c).curves.len() >= n + 1 + 1 + 3 + 2, "mirror and patterns add copies");
        run(&mut c, "sketch.construction", json!({"curves": [ml.clone()]}));
        assert!(sk(&c).curves.iter().any(|k| k.id == ml && k.construction));
        run(&mut c, "sketch.line", json!({"points": [[-30, 0], [-10, 0]]}));
        let tl = sk(&c).curves.last().unwrap().id.clone();
        run(&mut c, "sketch.extend", json!({"curve": tl, "at": [-11, 0]}));
        run(&mut c, "sketch.line", json!({"points": [[-20, -10], [-20, 10]]}));
        let cross = sk(&c).curves.last().unwrap().id.clone();
        run(&mut c, "sketch.trim", json!({"curve": cross, "at": [-20, 8]}));
        let m = sk(&c).curves.len();
        run(&mut c, "sketch.delete", json!({"entities": [tl]}));
        assert!(sk(&c).curves.len() < m);
        run(&mut c, "sketch.constraint.horizontal_vertical", json!({"line": cross}));
        assert!(!sk(&c).status.is_empty());
        run(&mut c, "sketch.finish", json!({}));
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
        assert!(b.closed && b.tris.len() >= 36 && b.tris.len() == b.face.len() * 3 && b.pos.len().is_multiple_of(3));
        assert!(b.face.iter().max() == Some(&5), "six faces on a block");
        assert!(sc.undo && sc.active.is_none());
        let zmax = b.pos.chunks(3).map(|p| p[2]).fold(f32::MIN, f32::max);
        assert!((zmax - 10.0).abs() < 1e-4, "extruded 10 mm, top at {zmax}");
    }
}
