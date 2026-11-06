//! Datum Mesh desktop shell: owns the open model, files, options and recovery, and talks to the
//! window. All calculation is in `datum-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod update;

use datum_core::doc::{Doc, Op};
use datum_core::model::{self, Analysis, Model, SectionShape};
use datum_core::{bench, examples, materials, mesh, sections};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Options {
    /// Minutes between recovery saves; 0 turns them off.
    autosave: u32,
    beta: bool,
    /// auto, light or dark.
    theme: String,
    open_last: bool,
    /// Display units for new models: SI or US.
    units: String,
    snap: bool,
    grid: bool,
    rulers: bool,
    /// Contour colours: "rainbow" (blue to red) or "viridis" (colour-blind safe).
    contours: String,
    paper: String,
    landscape: bool,
    company: String,
}

impl Default for Options {
    fn default() -> Self {
        Self { autosave: 5, beta: false, theme: "auto".into(), open_last: false, units: "SI".into(), snap: true, grid: true, rulers: true, contours: "rainbow".into(), paper: "A4".into(), landscape: true, company: String::new() }
    }
}

struct App {
    doc: Doc,
    opts: Options,
}

struct St(Mutex<App>);

impl St {
    /// A command that panicked must not lock every later one out, so a poisoned lock is reused.
    fn lock(&self) -> std::sync::MutexGuard<'_, App> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
type R<T> = Result<T, String>;

/// What the window needs after every change: the whole model (small: the mesh and results are
/// sent separately) and the document's state.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Out {
    /// The model, an imported mesh sent as its preview (0.5.5).
    m: serde_json::Value,
    path: Option<String>,
    dirty: bool,
    undo: bool,
    redo: bool,
    created: Vec<u32>,
    /// What a damaged file lost or changed when it was opened.
    warnings: Vec<String>,
}

fn out(a: &App) -> Out {
    Out { m: datum_core::solids::tet::for_window(&a.doc.m), path: a.doc.path.clone(), dirty: a.doc.dirty, undo: a.doc.can_undo(), redo: a.doc.can_redo(), created: vec![], warnings: vec![] }
}

fn dir(app: &AppHandle, data: bool) -> Option<PathBuf> {
    let d = if data { app.path().app_data_dir() } else { app.path().app_config_dir() }.ok()?;
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

fn fresh(a: &App, mut m: Model) -> Model {
    if m.nodes.is_empty() && m.shapes.is_empty() && (a.opts.units == "SI" || a.opts.units == "US") {
        m.units = a.opts.units.clone();
    }
    m
}

// ---------------------------------------------------------------- the geometry engine (0.7.1)

/// The design modelled with the geometry engine (sketches and solids), beside the analysis model.
struct CadSt(Mutex<datum_cad::Cad>);

impl CadSt {
    fn lock(&self) -> std::sync::MutexGuard<'_, datum_cad::Cad> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Runs one geometry-engine command (a sketch tool, a feature, undo…) and returns the scene.
#[tauri::command]
fn cad_run(st: State<CadSt>, app: State<St>, id: String, params: serde_json::Value) -> R<datum_cad::Scene> {
    let mut c = st.lock();
    c.run(&id, &params)?;
    // The geometry is part of the file: a change to it is a change to save.
    app.lock().doc.mark_unsaved();
    c.scene()
}

/// The geometry engine's scene as it is.
#[tauri::command]
fn cad_scene(st: State<CadSt>) -> R<datum_cad::Scene> {
    st.lock().scene()
}

/// One geometry-engine command's own reply (a sketch's report, the parameters list…).
#[tauri::command]
fn cad_query(st: State<CadSt>, id: String, params: serde_json::Value) -> R<serde_json::Value> {
    st.lock().run(&id, &params)
}

/// A new, empty design for the geometry engine.
#[tauri::command]
fn cad_new(st: State<CadSt>) -> R<datum_cad::Scene> {
    let mut c = st.lock();
    *c = datum_cad::Cad::new();
    c.scene()
}

// ---------------------------------------------------------------- the model

#[tauri::command]
fn state(st: State<St>) -> Out {
    out(&st.lock())
}

#[tauri::command]
fn apply(st: State<St>, op: Op) -> R<Out> {
    let mut a = st.lock();
    let created = a.doc.apply(op)?;
    let mut o = out(&a);
    o.created = created;
    Ok(o)
}

/// Import a part's section from a DXF drawing (0.3.5): the outlines join the model as shapes and
/// holes, in one step to undo. `scale` (metres per drawing unit) overrides the units the file
/// gives. None if the person cancelled the file dialog.
#[tauri::command]
async fn import_dxf(app: AppHandle, st: State<'_, St>, scale: Option<f64>) -> R<Option<Out>> {
    let Some(path) = pick(&app, "DXF drawing", &["dxf"]) else { return Ok(None) };
    let bytes = std::fs::read(&path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let imp = datum_core::dxf::read(&String::from_utf8_lossy(&bytes), scale)?;
    let mut a = st.lock();
    if !a.doc.m.analysis.is_plane() {
        return Err("A DXF brings in a part's section: switch to a part (plane stress, axisymmetric, a 3D solid, …) on Home first.".into());
    }
    let material = a.doc.m.materials.first().map(|x| x.id).ok_or("Add a material first.")?;
    let (n, holes) = (imp.shapes.iter().filter(|s| !s.1).count(), imp.shapes.iter().filter(|s| s.1).count());
    let ops = imp.shapes.into_iter().map(|(geom, hole)| Op::Add { item: datum_core::doc::Item::Shape(datum_core::model::Shape { id: 0, name: String::new(), hole, material, geom, plate: None, part: 0 }) }).collect();
    let created = a.doc.apply(Op::Batch(ops))?;
    let mut o = out(&a);
    o.created = created;
    o.warnings = std::iter::once(format!("Imported {n} {} and {holes} {} from {}.", if n == 1 { "shape" } else { "shapes" }, if holes == 1 { "hole" } else { "holes" }, path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default())).chain(imp.warnings).collect();
    Ok(Some(o))
}

/// The part's section as a DXF drawing (0.3.5), for the window to save.
#[tauri::command]
fn export_dxf(st: State<St>) -> String {
    datum_core::dxf::write(&st.lock().doc.m)
}

#[tauri::command]
fn history(st: State<St>, redo: bool) -> Out {
    let mut a = st.lock();
    if redo {
        a.doc.redo();
    } else {
        a.doc.undo();
    }
    out(&a)
}

#[tauri::command]
fn examples_list() -> Vec<examples::Example> {
    examples::list()
}

/// A new model: a worked example by id, or a blank one of a kind (frame, stress, strain, heat).
#[tauri::command]
fn new_model(st: State<St>, cad: State<CadSt>, example: Option<String>, analysis: Option<Analysis>) -> R<Out> {
    *cad.lock() = datum_cad::Cad::new();
    let mut a = st.lock();
    let m = match example.as_deref() {
        Some(id) => examples::get(id).ok_or("That example does not exist.")?,
        None => Model::new(analysis.unwrap_or_default()),
    };
    let m = fresh(&a, m);
    a.doc = Doc::new(m);
    Ok(out(&a))
}

/// File › Import Mesh (0.5.5): a tetrahedral mesh from a mesher (Gmsh `.msh`, or `.inp` in the
/// CalculiX input format) as a new 3D solid; its coordinates are in `unit` metres (0.001 for mm).
#[tauri::command]
async fn import_mesh(app: AppHandle, st: State<'_, St>, path: Option<String>, unit: f64) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Tetrahedral mesh", &["msh", "inp"])) else { return Ok(None) };
    if !(unit.is_finite() && unit > 0.0) {
        return Err("The mesh's units must be a positive length.".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut ms = if ext == "inp" { datum_core::solids::tet::read_inp(&text, unit)? } else { datum_core::solids::tet::read_msh(&text, unit)? };
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported mesh".into());
    ms.source = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut a = st.lock();
    let mut m = fresh(&a, Model::new(Analysis::Solid));
    m.name = name;
    ms.material = m.materials[0].id;
    m.volume = Some(Box::new(ms));
    m.check()?;
    a.doc = Doc::new(m);
    Ok(Some(out(&a)))
}

/// Solve the open model: frames and trusses, plane stress or strain, or heat conduction. Parts are
/// meshed first. Runs off the window's thread.
#[tauri::command]
async fn solve(st: State<'_, St>, which: Option<datum_core::cases::Which>) -> R<serde_json::Value> {
    let m = st.lock().doc.m.clone();
    tauri::async_runtime::spawn_blocking(move || datum_core::solve_for(&m, which).map(|r| small(&r))).await.map_err(|e| e.to_string())?
}

/// The largest and smallest member forces and reactions over every combination (0.3.3).
#[tauri::command]
async fn envelope(st: State<'_, St>) -> R<datum_core::cases::Envelope> {
    let m = st.lock().doc.m.clone();
    tauri::async_runtime::spawn_blocking(move || datum_core::cases::envelope(&m)).await.map_err(|e| e.to_string())?
}

/// The steel section tables (0.3.3).
#[tauri::command]
fn sections_library() -> Vec<datum_core::steel::LibSection> {
    datum_core::steel::library()
}

/// Results with their long arrays packed small for the window (`datum_core::wire`).
fn small<T: Serialize>(r: &T) -> serde_json::Value {
    datum_core::wire::encode(serde_json::to_value(r).unwrap_or_default())
}

/// The lowest natural frequencies and mode shapes of the open model (0.2.2).
#[tauri::command]
async fn modes(st: State<'_, St>, count: usize) -> R<serde_json::Value> {
    let m = st.lock().doc.m.clone();
    tauri::async_runtime::spawn_blocking(move || datum_core::modes(&m, count).map(|r| small(&r))).await.map_err(|e| e.to_string())?
}

/// The lowest buckling load factors and shapes of the open frame (0.2.3; 3D frames 0.2.6), sent as modes.
#[tauri::command]
async fn buckling(st: State<'_, St>, count: usize, which: Option<datum_core::cases::Which>) -> R<serde_json::Value> {
    let m = st.lock().doc.m.clone();
    tauri::async_runtime::spawn_blocking(move || datum_core::buckling_for(&m, count, which).map(|r| small(&r))).await.map_err(|e| e.to_string())?
}

/// Mesh the open part without solving it (the Mesh view and its statistics).
#[tauri::command]
async fn mesh(st: State<'_, St>) -> R<serde_json::Value> {
    let m = st.lock().doc.m.clone();
    if !m.analysis.is_plane() {
        return Err("Frames and trusses are not meshed: each member is one element.".into());
    }
    tauri::async_runtime::spawn_blocking(move || mesh::mesh(&m).map(|r| small(&r))).await.map_err(|e| e.to_string())?
}

/// The materials library (from Datum Atlas, with sources).
#[tauri::command]
fn materials_library() -> Vec<materials::LibMaterial> {
    materials::library()
}

/// The verification benchmarks, solved now by this version's engine.
#[tauri::command]
async fn verification() -> R<Vec<bench::Check>> {
    tauri::async_runtime::spawn_blocking(bench::run).await.map_err(|e| e.to_string())
}

/// Area, second moment of area and extreme-fibre distance of a section, for the window.
#[tauri::command]
fn section_props(shape: SectionShape) -> R<sections::Props> {
    sections::props(&shape)
}

// ---------------------------------------------------------------- files

/// The model as written to a .dmesh file, with the geometry engine's design (0.7.1) under
/// `geometry` when it has anything in it.
fn file_text(a: &App, cad: &datum_cad::Cad) -> String {
    let text = model::to_json(&a.doc.m);
    if cad.is_empty() {
        return text;
    }
    match (serde_json::from_str::<serde_json::Value>(&text), cad.to_json()) {
        (Ok(serde_json::Value::Object(mut o)), Ok(g)) => {
            o.insert("geometry".into(), g);
            serde_json::to_string_pretty(&serde_json::Value::Object(o)).unwrap_or(text)
        }
        _ => text,
    }
}

/// A .dmesh file's text split into the model's and the geometry engine's design (none in files
/// before 0.7.1, or one that cannot be read, said in the warnings).
fn split_file(text: &str) -> (String, Option<datum_cad::Cad>, Vec<String>) {
    let Ok(serde_json::Value::Object(mut o)) = serde_json::from_str::<serde_json::Value>(text) else { return (text.to_string(), None, vec![]) };
    let Some(g) = o.remove("geometry") else { return (text.to_string(), None, vec![]) };
    let rest = serde_json::to_string(&serde_json::Value::Object(o)).unwrap_or_default();
    match datum_cad::Cad::from_json(g) {
        Ok(c) => (rest, Some(c), vec![]),
        Err(e) => (rest, None, vec![e]),
    }
}


fn pick(app: &AppHandle, name: &str, exts: &[&str]) -> Option<PathBuf> {
    app.dialog().file().add_filter(name, exts).blocking_pick_file().and_then(|f| f.into_path().ok())
}

#[tauri::command]
async fn open(app: AppHandle, st: State<'_, St>, cad: State<'_, CadSt>, path: Option<String>) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Datum Mesh model", &["dmesh"])) else { return Ok(None) };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let (text, geometry, lost) = split_file(&text);
    let (m, mut warnings) = model::from_json(&text)?;
    warnings.extend(lost);
    remember(&app, &path);
    *cad.lock() = geometry.unwrap_or_default();
    let mut a = st.lock();
    a.doc = Doc::new(m);
    a.doc.path = Some(path.display().to_string());
    let mut o = out(&a);
    o.warnings = warnings;
    Ok(Some(o))
}

#[tauri::command]
async fn save(app: AppHandle, st: State<'_, St>, cad: State<'_, CadSt>, as_new: bool) -> R<Option<Out>> {
    let (known, name) = {
        let a = st.lock();
        (a.doc.path.clone().filter(|_| !as_new), a.doc.m.name.clone())
    };
    let path = match known {
        Some(p) => PathBuf::from(p),
        None => match app.dialog().file().add_filter("Datum Mesh model", &["dmesh"]).set_file_name(format!("{name}.dmesh")).blocking_save_file() {
            Some(f) => f.into_path().map_err(|e| e.to_string())?,
            None => return Ok(None),
        },
    };
    let mut a = st.lock();
    write_safely(&path, file_text(&a, &cad.lock()).as_bytes()).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    a.doc.path = Some(path.display().to_string());
    a.doc.mark_saved();
    remember(&app, &path);
    clear_recovery(&app);
    Ok(Some(out(&a)))
}

/// Saves a picture or a file the window made (SVG text, PNG data URL, CSV text).
#[tauri::command]
async fn save_file(app: AppHandle, name: String, data: String) -> R<Option<String>> {
    let ext = name.rsplit('.').next().unwrap_or("txt").to_string();
    let Some(path) = app.dialog().file().add_filter(ext.to_uppercase(), &[&ext]).set_file_name(&name).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let bytes = match data.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
        Some((_, b64)) => base64_decode(b64)?,
        None => data.into_bytes(),
    };
    write_safely(&path, &bytes).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

/// Write a file so a crash or a full disk never leaves it half written: write a temporary file
/// beside it, flush it to disk, then rename it over the old one (0.1.1 review).
fn write_safely(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("{}.saving", path.extension().and_then(|e| e.to_str()).unwrap_or("tmp")));
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

fn base64_decode(s: &str) -> R<Vec<u8>> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = acc << 6 | val(c).ok_or("The picture data is damaged.")? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- options, recovery, recent

fn options_file(app: &AppHandle) -> Option<PathBuf> {
    dir(app, false).map(|d| d.join("options.json"))
}

#[tauri::command]
fn options(st: State<St>) -> Options {
    st.lock().opts.clone()
}

#[tauri::command]
fn set_options(app: AppHandle, st: State<St>, opts: Options) -> Out {
    if let Some(f) = options_file(&app) {
        let _ = write_safely(&f, serde_json::to_string(&opts).unwrap_or_default().as_bytes());
    }
    let mut a = st.lock();
    a.opts = opts;
    out(&a)
}

#[derive(Serialize, Deserialize)]
struct Recovery {
    name: String,
    path: Option<String>,
    saved: String,
}

fn recovery_files(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    dir(app, true).map(|d| (d.join("recovery.dmesh"), d.join("recovery.json")))
}

fn clear_recovery(app: &AppHandle) {
    if let Some((a, b)) = recovery_files(app) {
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }
}

#[tauri::command]
fn autosave(app: AppHandle, st: State<St>, cad: State<CadSt>) {
    let a = st.lock();
    if let (true, Some((file, meta))) = (a.doc.dirty, recovery_files(&app)) {
        let info = Recovery { name: a.doc.m.name.clone(), path: a.doc.path.clone(), saved: chrono::Local::now().format("%d-%b-%y %H:%M").to_string() };
        let _ = write_safely(&file, file_text(&a, &cad.lock()).as_bytes());
        let _ = write_safely(&meta, serde_json::to_string(&info).unwrap_or_default().as_bytes());
    }
}

#[tauri::command]
fn recovery(app: AppHandle) -> Option<Recovery> {
    let (file, meta) = recovery_files(&app)?;
    file.exists().then_some(())?;
    serde_json::from_str(&std::fs::read_to_string(meta).ok()?).ok()
}

#[tauri::command]
fn recover(app: AppHandle, st: State<St>, cad: State<CadSt>, keep: bool) -> R<Option<Out>> {
    let info = recovery(app.clone());
    let files = recovery_files(&app);
    if !keep {
        clear_recovery(&app);
        return Ok(None);
    }
    let (file, _) = files.ok_or("No recovered model was found.")?;
    let (text, geometry, lost) = split_file(&std::fs::read_to_string(file).map_err(|e| e.to_string())?);
    let (m, mut warnings) = model::from_json(&text)?;
    warnings.extend(lost);
    *cad.lock() = geometry.unwrap_or_default();
    let mut a = st.lock();
    a.doc = Doc::new(m);
    a.doc.path = info.and_then(|i| i.path);
    a.doc.mark_unsaved();
    let mut o = out(&a);
    o.warnings = warnings;
    Ok(Some(o))
}

fn recent_paths(app: &AppHandle) -> Vec<String> {
    options_file(app).map(|f| f.with_file_name("recent.json")).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember(app: &AppHandle, path: &Path) {
    let p = path.display().to_string();
    let mut v = recent_paths(app);
    v.retain(|x| *x != p);
    v.insert(0, p);
    v.truncate(12);
    if let Some(f) = options_file(app).map(|f| f.with_file_name("recent.json")) {
        let _ = write_safely(&f, serde_json::to_string(&v).unwrap_or_default().as_bytes());
    }
}

#[derive(Serialize)]
struct Recent {
    path: String,
    name: String,
    modified: String,
}

#[tauri::command]
fn recent(app: AppHandle) -> Vec<Recent> {
    recent_paths(&app)
        .into_iter()
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?.modified().ok()?;
            let modified = chrono::DateTime::<chrono::Local>::from(m).format("%d-%b-%y %H:%M").to_string();
            let name = Path::new(&p).file_stem()?.to_string_lossy().into_owned();
            Some(Recent { path: p, name, modified })
        })
        .collect()
}

#[tauri::command]
fn startup_file(app: AppHandle, st: State<St>) -> Option<String> {
    let last = || if st.lock().opts.open_last { recent_paths(&app).into_iter().find(|p| Path::new(p).exists()) } else { None };
    std::env::args().skip(1).find(|a| { let l = a.to_lowercase(); l.ends_with(".dmesh") || l.ends_with(".dlink") }).or_else(last)
}

/// Datum Link (0.5.4, docs/LINKS.md): write a packet for another Datum app to
/// `%LOCALAPPDATA%\Datum\links` and start that app with it when it is installed. Returns what
/// happened, for the window to say.
#[tauri::command]
fn send_link(app: AppHandle, st: State<St>, to: String, title: String, what: datum_core::link::Outgoing) -> R<String> {
    let name = datum_core::link::APPS.iter().find(|a| a.0 == to).map(|a| a.1).ok_or("Unknown Datum app.")?;
    let file = st.lock().doc.path.as_deref().and_then(|p| Path::new(p).file_name().map(|f| f.to_string_lossy().into_owned())).unwrap_or_default();
    let now = chrono::Utc::now();
    let text = datum_core::link::envelope(&to, &title, &file, &what, &now.to_rfc3339())?;
    let base = app.path().local_data_dir().map_err(|e| e.to_string())?;
    let dir = base.join("Datum").join("links");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not make the links folder: {e}"))?;
    let path = dir.join(format!("{}-{}.dlink", now.format("%Y%m%d-%H%M%S"), to));
    std::fs::write(&path, &text).map_err(|e| format!("Could not write the link file: {e}"))?;
    // Each Datum app installs for the user in %LOCALAPPDATA%\<its name>.
    let exe = std::fs::read_dir(base.join(name)).ok().and_then(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) && !p.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase().contains("unins"))));
    match exe {
        Some(x) => {
            std::process::Command::new(x).arg(&path).spawn().map_err(|e| format!("Could not start {name}: {e}"))?;
            Ok(format!("Sent to {name}."))
        }
        None => Ok(format!("{name} is not installed on this computer. The data is saved in {} and copied, ready to paste; {name} is at edgar-mendonca.github.io/Datum.", path.display())),
    }
}

/// Datum Link: read a packet another Datum app made for Datum Mesh (a material from Datum
/// Atlas), from the given file or one picked. Returns who sent it and the material.
#[tauri::command]
async fn read_link(app: AppHandle, path: Option<String>) -> R<Option<(String, datum_core::model::Material)>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Datum link", &["dlink"])) else { return Ok(None) };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let (who, datum_core::link::Incoming::Material(m)) = datum_core::link::read(&text)?;
    Ok(Some((who, m)))
}

/// Opens one of the project's GitHub pages in the browser (only those).
#[tauri::command]
fn open_url(url: String) -> R<()> {
    if !url.starts_with("https://github.com/Edgar-Mendonca/Datum-Mesh") && !url.starts_with("https://edgar-mendonca.github.io/Datum") {
        return Err("Only the Datum GitHub pages can be opened.".into());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&url).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn quit(app: AppHandle) {
    clear_recovery(&app);
    app.exit(0);
}

#[tauri::command]
async fn check_update(st: State<'_, St>) -> R<update::Info> {
    let beta = st.lock().opts.beta;
    tauri::async_runtime::spawn_blocking(move || update::check(beta)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_update(app: AppHandle, url: String, sums: Option<String>) -> R<()> {
    tauri::async_runtime::spawn_blocking(move || update::install(&app, &url, sums.as_deref())).await.map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let opts: Options = options_file(app.handle()).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            let a = App { doc: Doc::new(Model::new(Analysis::Frame)), opts };
            let m = fresh(&a, Model::new(Analysis::Frame));
            app.manage(St(Mutex::new(App { doc: Doc::new(m), ..a })));
            app.manage(CadSt(Mutex::new(datum_cad::Cad::new())));
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                if w.state::<St>().lock().doc.dirty {
                    api.prevent_close();
                    let _ = w.emit("close-requested", ());
                } else {
                    clear_recovery(w.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            cad_run, cad_scene, cad_new, cad_query, state, apply, history, examples_list, new_model, section_props, solve, envelope, sections_library, import_dxf, export_dxf, modes, buckling, mesh, materials_library, verification, open, save, save_file,
            options, set_options, autosave, recovery, recover, recent, startup_file, open_url, send_link, read_link, import_mesh, version, quit, check_update, install_update
        ])
        .run(tauri::generate_context!())
        .expect("Datum Mesh failed to start");
}

#[cfg(test)]
mod tests {
    #[test]
    fn saving_replaces_the_file_whole_and_leaves_nothing_behind() {
        let dir = std::env::temp_dir().join(format!("datum-mesh-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("model.dmesh");
        super::write_safely(&f, b"first, and longer than the second").unwrap();
        super::write_safely(&f, b"second").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"second");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary file left");
        // A folder that does not exist: an error, and nothing written.
        assert!(super::write_safely(&dir.join("no/such/model.dmesh"), b"x").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
