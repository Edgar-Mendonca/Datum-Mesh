//! The Simulation workspace (Datum Mesh 0.7.0): the analysis of the design's solids, in steps.
//!
//! 1 Material, 2 Supports & Loads, 3 Mesh, 4 Solve, 5 Results. The stage bar replaces the ribbon
//! (as in the Render workspace); its second row holds the step's tools. The study (material,
//! supports and loads on faces by their persistent names, element size) is saved in the design's
//! metadata (`datum_analysis::METADATA_KEY`), so it is undone, saved and opened with the design.
//! Meshing (Gmsh, a separate program) and solving (datum-core) run off the window's thread.

use crate::{SolveApp, ui_kit};
use datum_analysis as da;
use std::sync::mpsc;

pub const TAB: &str = "SIMULATION";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Material,
    Holds,
    Mesh,
    Solve,
    Results,
}
impl Stage {
    pub const ALL: [Stage; 5] = [Stage::Material, Stage::Holds, Stage::Mesh, Stage::Solve, Stage::Results];
    fn label(self) -> &'static str {
        match self {
            Stage::Material => "1  Material",
            Stage::Holds => "2  Supports & Loads",
            Stage::Mesh => "3  Mesh",
            Stage::Solve => "4  Solve",
            Stage::Results => "5  Results",
        }
    }
}

/// What a finished background job hands back.
enum Done {
    Meshed(da::Meshed),
    Solved(da::Meshed, Box<datum_core::solid::SolidResults>),
}

struct Job {
    rx: mpsc::Receiver<Result<Done, String>>,
    what: &'static str,
}

/// The workspace's state in the window (the study itself lives in the design).
pub struct SimState {
    pub stage: Stage,
    /// The force (kN, along X, Y, Z) and the pressure (MPa) the next load takes.
    pub force: [f64; 3],
    pub pressure: f64,
    /// The field shown: 0 von Mises stress, 1 movement.
    pub field: usize,
    pub deformed: bool,
    job: Option<Job>,
    /// The last mesh, and the design and size it was made for.
    meshed: Option<(usize, f64, da::Meshed)>,
    pub results: Option<std::sync::Arc<datum_core::solid::SolidResults>>,
    /// Bumped whenever what the view shows changes (results, field, deformation).
    pub revision: u64,
    pub note: String,
}

impl Default for SimState {
    fn default() -> Self {
        SimState { stage: Stage::Material, force: [0.0, 0.0, -1.0], pressure: 1.0, field: 0, deformed: true, job: None, meshed: None, results: None, revision: 0, note: String::new() }
    }
}

pub fn active(app: &SolveApp) -> bool {
    app.ui.tab == TAB
}

/// Results replace the solids in the view.
pub fn showing_results(app: &SolveApp) -> bool {
    active(app) && app.sim.stage == Stage::Results && app.sim.results.is_some()
}

/// Enter (or leave, back to Design) the Simulation workspace.
pub fn enter(app: &mut SolveApp, on: bool) -> Result<(), String> {
    if on {
        if app.session.active_sketch.is_some() || app.tool.is_some() || app.dialog.is_some() {
            return Err("Finish the sketch or the feature first, then open Simulation.".into());
        }
        if crate::render_workspace::active(app) {
            crate::render_workspace::enter(app, false)?;
        }
        // Faces are what supports and loads go on. (A command can move the ribbon to its own
        // tab, so the workspace's tab is set after it.)
        let _ = app.run("select.filter", serde_json::json!({"mode": "face"}));
        app.ui.tab = TAB.into();
    } else {
        app.ui.tab = "MODEL".into();
    }
    app.sim.revision += 1;
    Ok(())
}

fn study(app: &SolveApp) -> da::Study {
    da::Study::from_json(app.session.doc.metadata.get(da::METADATA_KEY))
}

fn save(app: &mut SolveApp, s: &da::Study) {
    if let Err(e) = app.run("document.metadata.set", serde_json::json!({"key": da::METADATA_KEY, "value": s.to_json()})) {
        app.set_status(format!("The study could not be saved in the design: {e}"), true);
    }
}

/// The faces picked in the view, by their bodies' and their own persistent names.
fn picked_faces(app: &SolveApp) -> Vec<da::FaceRef> {
    let Ok(state) = app.session.world_state() else { return vec![] };
    app.session
        .selection
        .iter()
        .filter_map(|s| match s {
            solidcraft_engine::Sel::Face { body, index, .. } => {
                let b = state.bodies.iter().find(|b| &b.name == body)?;
                let names = solidcraft_engine::doc::naming::face_names(b);
                Some(da::FaceRef { body: body.clone(), face: names.get(*index)?.clone() })
            }
            _ => None,
        })
        .collect()
}

/// Gmsh: beside the program (the installer puts it in `gmsh\`), named by DATUM_GMSH, or on the PATH.
fn gmsh() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("DATUM_GMSH") {
        return p.into();
    }
    let exe = if cfg!(windows) { "gmsh.exe" } else { "gmsh" };
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())) {
        for p in [dir.join("gmsh").join(exe), dir.join(exe)] {
            if p.exists() {
                return p;
            }
        }
    }
    exe.into()
}

/// Start meshing (and solving, with `solve`) in the background.
fn start(app: &mut SolveApp, solve: bool) {
    if app.sim.job.is_some() {
        return;
    }
    let study = study(app);
    if solve && !study.missing().is_empty() {
        app.sim.note = format!("Still needed: {}.", study.missing().join(", "));
        return;
    }
    let Ok(state) = app.session.world_state() else {
        app.sim.note = "The design could not be evaluated.".into();
        return;
    };
    let solids: Vec<(String, solidcraft_engine::kernel::Body, solidcraft_engine::geom::Mesh, Vec<String>)> = state
        .bodies
        .iter()
        .filter(|b| b.is_closed() && !b.body.is_mesh() && app.body_visible(&b.name))
        .map(|b| (b.name.clone(), b.body.clone(), (*b.mesh()).clone(), (*solidcraft_engine::doc::naming::face_names(b)).clone()))
        .collect();
    if solids.is_empty() {
        app.sim.note = "There is no solid to analyse: model a solid in Design first.".into();
        return;
    }
    let doc = std::sync::Arc::as_ptr(&app.session.doc) as usize;
    let cached = app.sim.meshed.take().filter(|(d, sz, _)| *d == doc && *sz == study.size).map(|x| x.2);
    let (tx, rx) = mpsc::channel();
    let work = std::env::temp_dir().join("datum-mesh-study");
    std::thread::spawn(move || {
        let run = || -> Result<Done, String> {
            let meshed = match cached {
                Some(m) => m,
                None => {
                    let list: Vec<da::Solid> = solids.iter().map(|(n, b, m, f)| da::Solid { name: n, body: b, surface: m, face_names: f }).collect();
                    da::mesh(&list, study.size, &gmsh(), &work)?
                }
            };
            if !solve {
                return Ok(Done::Meshed(meshed));
            }
            let r = da::solve(&meshed, &study)?;
            Ok(Done::Solved(meshed, Box::new(r)))
        };
        let _ = tx.send(run());
    });
    app.sim.job = Some(Job { rx, what: if solve { "Meshing and solving…" } else { "Meshing…" } });
    app.sim.note.clear();
    let _ = doc;
}

/// Collect a finished job (every frame, whichever workspace shows).
pub fn tick(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(job) = &app.sim.job else { return };
    match job.rx.try_recv() {
        Ok(done) => {
            app.sim.job = None;
            let doc = std::sync::Arc::as_ptr(&app.session.doc) as usize;
            let size = study(app).size;
            match done {
                Ok(Done::Meshed(m)) => {
                    app.sim.note = format!("Meshed: {} quadratic tetrahedra, {} nodes.", m.mesh.tets.len(), m.mesh.nodes.len());
                    app.sim.meshed = Some((doc, size, m));
                }
                Ok(Done::Solved(m, r)) => {
                    app.sim.note = format!("Solved: {} tetrahedra; equilibrium within {:.1e} of the loads.", m.mesh.tets.len(), r.imbalance);
                    app.sim.meshed = Some((doc, size, m));
                    app.sim.results = Some(std::sync::Arc::new(*r));
                    app.sim.stage = Stage::Results;
                    app.sim.revision += 1;
                    // Picked faces would light up over the results.
                    let _ = app.run("select.clear", serde_json::json!({}));
                }
                Err(e) => app.sim.note = e,
            }
        }
        Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
        Err(mpsc::TryRecvError::Disconnected) => {
            app.sim.job = None;
            app.sim.note = "The analysis stopped unexpectedly.".into();
        }
    }
}

/// The stage bar: the steps, Solve, and the step's tools.
pub fn stage_bar(app: &mut SolveApp, ui: &mut egui::Ui, rect: egui::Rect) {
    tick(app, ui.ctx());
    let labels: Vec<&str> = Stage::ALL.iter().map(|s| s.label()).collect();
    let selected = Stage::ALL.iter().position(|s| *s == app.sim.stage).unwrap_or(0);
    let busy = app.sim.job.is_some();
    let actions = [
        ui_kit::StageAction { icon: Some("play"), label: "Solve", enabled: !busy, primary: true, reason: busy.then_some("Working…") },
        ui_kit::StageAction { icon: None, label: "Back to Design", enabled: !busy, primary: false, reason: None },
    ];
    let choice = ui_kit::StageBar::row(ui, rect, false, |ui| ui_kit::StageBar::header(ui, selected, &labels, &actions));
    if let Some(choice) = choice {
        if let Some(stage) = choice.stage.and_then(|i| Stage::ALL.get(i)) {
            app.sim.stage = *stage;
            app.sim.revision += 1;
        }
        match choice.action {
            Some(0) => start(app, true),
            Some(1) => {
                let _ = enter(app, false);
            }
            _ => {}
        }
    }
    ui_kit::StageBar::row(ui, rect, true, |ui| tools(app, ui));
}

fn tools(app: &mut SolveApp, ui: &mut egui::Ui) {
    let mut s = study(app);
    match app.sim.stage {
        Stage::Material => {
            ui_kit::StageBar::caption(ui, "Material of every solid");
            let mats = da::materials();
            let current = mats.iter().find(|m| m.0 == s.material).map_or("Choose…".to_string(), |m| m.1.clone());
            let mut pick = None;
            egui::ComboBox::from_id_salt("sim_material").selected_text(current).width(300.0).show_ui(ui, |ui| {
                for (id, name, family) in &mats {
                    if ui.selectable_label(*id == s.material, format!("{name}  ·  {family}")).clicked() {
                        pick = Some(id.clone());
                    }
                }
            });
            if let Some(id) = pick {
                s.material = id;
                save(app, &s);
            }
            if let Some(m) = datum_core::materials::get(&s.material) {
                ui_kit::StageBar::note(ui, &format!("E {:.0} GPa · ν {:.2} · yield {:.0} MPa · {:.0} kg/m³", m.e / 1e9, m.nu, m.fy / 1e6, m.rho));
            }
        }
        Stage::Holds => {
            ui_kit::StageBar::caption(ui, "Click faces (Ctrl adds), then:");
            let faces = picked_faces(app);
            let none = faces.is_empty();
            for (hold, label) in [(da::Hold::Fixed, "Fixed"), (da::Hold::Roller, "Roller"), (da::Hold::Symmetry, "Symmetry")] {
                if ui_kit::compact_icon_label_button(ui, label, "pin", !none).clicked() {
                    s.supports.push(da::SupportSet { id: s.next, hold, faces: faces.clone() });
                    s.next += 1;
                    save(app, &s);
                }
            }
            ui_kit::StageBar::separator(ui);
            ui.label("Force (kN)");
            for k in 0..3 {
                ui.add(egui::DragValue::new(&mut app.sim.force[k]).speed(0.1).prefix(["X ", "Y ", "Z "][k]));
            }
            if ui_kit::compact_icon_label_button(ui, "Force", "arrow", !none).clicked() {
                let f = app.sim.force;
                s.loads.push(da::LoadSet { id: s.next, push: da::Push::Force { x: f[0] * 1e3, y: f[1] * 1e3, z: f[2] * 1e3 }, faces: faces.clone() });
                s.next += 1;
                save(app, &s);
            }
            ui.add(egui::DragValue::new(&mut app.sim.pressure).speed(0.1).suffix(" MPa"));
            if ui_kit::compact_icon_label_button(ui, "Pressure", "arrow", !none).clicked() {
                s.loads.push(da::LoadSet { id: s.next, push: da::Push::Pressure { p: app.sim.pressure * 1e6 }, faces: faces.clone() });
                s.next += 1;
                save(app, &s);
            }
            ui_kit::StageBar::separator(ui);
            // What is set: each as a chip, × to take it away.
            let mut drop = None;
            for x in &s.supports {
                let text = egui::RichText::new(format!("{:?} · {} face{}  ×", x.hold, x.faces.len(), if x.faces.len() == 1 { "" } else { "s" })).color(egui::Color32::from_rgb(40, 110, 220));
                if ui.small_button(text).on_hover_text("Take this support away").clicked() {
                    drop = Some(x.id);
                }
            }
            for x in &s.loads {
                let text = match x.push {
                    da::Push::Force { x: fx, y, z } => format!("Force {:.3} kN · {} face{}  ×", (fx * fx + y * y + z * z).sqrt() / 1e3, x.faces.len(), if x.faces.len() == 1 { "" } else { "s" }),
                    da::Push::Pressure { p } => format!("Pressure {:.3} MPa · {} face{}  ×", p / 1e6, x.faces.len(), if x.faces.len() == 1 { "" } else { "s" }),
                };
                let text = egui::RichText::new(text).color(egui::Color32::from_rgb(205, 120, 10));
                if ui.small_button(text).on_hover_text("Take this load away").clicked() {
                    drop = Some(x.id);
                }
            }
            if let Some(id) = drop {
                s.supports.retain(|x| x.id != id);
                s.loads.retain(|x| x.id != id);
                save(app, &s);
            }
        }
        Stage::Mesh => {
            ui_kit::StageBar::caption(ui, "Element size (0: automatic)");
            let mut size = s.size;
            if ui.add(egui::DragValue::new(&mut size).range(0.0..=1e5).speed(0.2).suffix(" mm")).changed() {
                s.size = size;
                save(app, &s);
            }
            if ui_kit::compact_icon_label_button(ui, "Mesh", "mesh", app.sim.job.is_none()).clicked() {
                start(app, false);
            }
        }
        Stage::Solve => {
            let missing = s.missing();
            if missing.is_empty() {
                ui_kit::StageBar::caption(ui, "Ready: linear static stress (small movements, elastic material)");
                if ui_kit::compact_icon_label_button(ui, "Solve", "play", app.sim.job.is_none()).clicked() {
                    start(app, true);
                }
            } else {
                ui_kit::StageBar::caption(ui, &format!("Still needed: {}", missing.join(", ")));
            }
        }
        Stage::Results => {
            if let Some(r) = app.sim.results.clone() {
                let mut field = app.sim.field;
                ui_kit::segmented(ui, &mut field, &["Von Mises stress", "Movement"], ui_kit::Segments { dense: true, intrinsic: true, ..Default::default() });
                if field != app.sim.field {
                    app.sim.field = field;
                    app.sim.revision += 1;
                }
                if ui.checkbox(&mut app.sim.deformed, "Deformed shape").changed() {
                    app.sim.revision += 1;
                }
                let (lo, hi, unit, k) = range(&r, app.sim.field);
                legend(ui, lo / k, hi / k, unit);
            } else {
                ui_kit::StageBar::caption(ui, "Solve to see the results.");
            }
        }
    }
    if let Some(job) = &app.sim.job {
        ui.spinner();
        ui.label(job.what);
    } else if !app.sim.note.is_empty() {
        ui_kit::StageBar::note(ui, &app.sim.note.clone());
    }
}

/// The shown field's range and its unit: (min, max, unit, divisor from SI).
fn range(r: &datum_core::solid::SolidResults, field: usize) -> (f64, f64, &'static str, f64) {
    let key = if field == 0 { "vm" } else { "umag" };
    let f = r.fields.iter().find(|f| f.key == key);
    let (lo, hi) = f.map_or((0.0, 0.0), |f| (f.min, f.max));
    if field == 0 { (lo, hi, "MPa", 1e6) } else { (lo, hi, "mm", 1e-3) }
}

/// Ten bands from blue to red.
const BANDS: [[u8; 3]; 10] = [[38, 70, 180], [40, 110, 220], [50, 160, 230], [60, 200, 200], [90, 200, 120], [170, 210, 70], [235, 205, 60], [245, 150, 50], [230, 90, 40], [200, 35, 35]];


/// A value with about four significant figures, for the legend.
fn nice(v: f64) -> String {
    let a = v.abs();
    let d = if a == 0.0 || a >= 1000.0 { 0 } else if a >= 100.0 { 1 } else if a >= 10.0 { 2 } else { 3 };
    format!("{v:.d$}")
}

fn legend(ui: &mut egui::Ui, lo: f64, hi: f64, unit: &str) {
    ui.label(format!("{} {unit}", nice(lo)));
    let (r, p) = ui.allocate_painter(egui::vec2(200.0, 14.0), egui::Sense::hover());
    let w = r.rect.width() / BANDS.len() as f32;
    for (i, c) in BANDS.iter().enumerate() {
        p.rect_filled(egui::Rect::from_min_size(r.rect.min + egui::vec2(i as f32 * w, 0.0), egui::vec2(w + 0.5, r.rect.height())), 0.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
    }
    ui.label(format!("{} {unit}", nice(hi)));
}

/// The results in the view: the surface coloured by the field, moved by the movements (scaled
/// so the largest is a twentieth of the size), in place of the solids. Millimetres, as the design.
pub fn scene(app: &SolveApp, sc: &mut crate::gpu::GpuScene) {
    if !active(app) {
        return;
    }
    if !showing_results(app) {
        marks(app, sc);
        return;
    }
    let Some(r) = app.sim.results.as_ref() else { return };
    let key = if app.sim.field == 0 { "vm" } else { "umag" };
    let Some(f) = r.fields.iter().find(|f| f.key == key) else { return };
    let span = (f.max - f.min).max(1e-300);
    let pts = &r.surface.nodes;
    let size = {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for p in pts {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (0..3).map(|k| hi[k] - lo[k]).fold(0.0_f64, f64::max)
    };
    let dmax = r.disp.iter().map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()).fold(0.0_f64, f64::max);
    let scale = if app.sim.deformed && dmax > 0.0 { 0.05 * size / dmax } else { 0.0 };
    let at = |i: usize| -> [f32; 3] {
        let (p, d) = (pts[i], r.disp.get(i).copied().unwrap_or([0.0; 3]));
        [((p[0] + d[0] * scale) * 1e3) as f32, ((p[1] + d[1] * scale) * 1e3) as f32, ((p[2] + d[2] * scale) * 1e3) as f32]
    };
    let colours: Vec<_> = BANDS.iter().map(|c| sc.color([c[0], c[1], c[2], 255])).collect();
    for t in &r.surface.tris {
        let [a, b, c] = t.map(|i| at(i as usize));
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
        let n = [n[0] / l, n[1] / l, n[2] / l];
        for (p, i) in [(a, t[0]), (b, t[1]), (c, t[2])] {
            let v = f.v.get(i as usize).copied().unwrap_or(f.min);
            let k = ((((v - f.min) / span).clamp(0.0, 1.0) * 10.0) as usize).min(9);
            sc.tri(p, n, colours[k]);
        }
    }
}

/// Faces held (blue) and loaded (amber), drawn a hair outside the body so they show over it.
fn marks(app: &SolveApp, sc: &mut crate::gpu::GpuScene) {
    let st = study(app);
    let Ok(state) = app.session.world_state() else { return };
    let sets = st.supports.iter().map(|s| (&s.faces, [40u8, 110, 220])).chain(st.loads.iter().map(|l| (&l.faces, [235u8, 150, 30])));
    for (faces, rgb) in sets {
        let colour = sc.color([rgb[0], rgb[1], rgb[2], 255]);
        for f in faces {
            let Some(b) = state.bodies.iter().find(|b| b.name == f.body && app.body_visible(&b.name)) else { continue };
            let names = solidcraft_engine::doc::naming::face_names(b);
            let Some(index) = names.iter().position(|n| *n == f.face) else { continue };
            let m = b.mesh();
            let lift = 0.002 * m.positions.iter().fold(0.0_f64, |a, p| a.max(p.x.abs()).max(p.y.abs()).max(p.z.abs())).max(1.0);
            for (t, &face) in m.triangles.iter().zip(&m.tri_face) {
                if face as usize != index {
                    continue;
                }
                let [a, b2, c] = t.map(|i| m.positions.get(i as usize).map_or([0.0; 3], |p| [p.x, p.y, p.z]));
                let (u, v) = ([b2[0] - a[0], b2[1] - a[1], b2[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
                let n = n.map(|x| x / l);
                for p in [a, b2, c] {
                    let q = [p[0] + n[0] * lift, p[1] + n[1] * lift, p[2] + n[2] * lift];
                    sc.tri(q.map(|x| x as f32), n.map(|x| x as f32), colour);
                }
            }
        }
    }
}

/// The control channel's `ui.sim` (tests and automation): `stage` (material, holds, mesh, solve,
/// results), `start` ("mesh" or "solve"); replies with the study, the note, whether a job runs and
/// the results' extremes.
pub fn control(app: &mut SolveApp, p: &serde_json::Value) -> Result<serde_json::Value, String> {
    if let Some(st) = p.get("stage").and_then(serde_json::Value::as_str) {
        app.sim.stage = match st {
            "material" => Stage::Material,
            "holds" => Stage::Holds,
            "mesh" => Stage::Mesh,
            "solve" => Stage::Solve,
            "results" => Stage::Results,
            _ => return Err("stage must be material, holds, mesh, solve or results".into()),
        };
        app.sim.revision += 1;
    }
    match p.get("start").and_then(serde_json::Value::as_str) {
        Some("mesh") => start(app, false),
        Some("solve") => start(app, true),
        Some(_) => return Err("start must be mesh or solve".into()),
        None => {}
    }
    if let Some(f) = p.get("field").and_then(serde_json::Value::as_u64) {
        app.sim.field = (f as usize).min(1);
        app.sim.revision += 1;
    }
    let results = app.sim.results.as_ref().map(|r| {
        let f = |k: &str| r.fields.iter().find(|f| f.key == k).map(|f| serde_json::json!([f.min, f.max]));
        serde_json::json!({"vm": f("vm"), "umag": f("umag"), "uz": f("uz"), "imbalance": r.imbalance, "elements": r.elements})
    });
    Ok(serde_json::json!({
        "active": active(app), "study": study(app).to_json(), "note": app.sim.note, "busy": app.sim.job.is_some(), "results": results,
        "gmsh": gmsh().to_string_lossy(),
    }))
}
