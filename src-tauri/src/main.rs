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
    m: Model,
    path: Option<String>,
    dirty: bool,
    undo: bool,
    redo: bool,
    created: Vec<u32>,
    /// What a damaged file lost or changed when it was opened.
    warnings: Vec<String>,
}

fn out(a: &App) -> Out {
    Out { m: a.doc.m.clone(), path: a.doc.path.clone(), dirty: a.doc.dirty, undo: a.doc.can_undo(), redo: a.doc.can_redo(), created: vec![], warnings: vec![] }
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
fn new_model(st: State<St>, example: Option<String>, analysis: Option<Analysis>) -> R<Out> {
    let mut a = st.lock();
    let m = match example.as_deref() {
        Some(id) => examples::get(id).ok_or("That example does not exist.")?,
        None => Model::new(analysis.unwrap_or_default()),
    };
    let m = fresh(&a, m);
    a.doc = Doc::new(m);
    Ok(out(&a))
}

/// Solve the open model: frames and trusses, plane stress or strain, or heat conduction. Parts are
/// meshed first. Runs off the window's thread.
#[tauri::command]
async fn solve(st: State<'_, St>) -> R<serde_json::Value> {
    let m = st.lock().doc.m.clone();
    tauri::async_runtime::spawn_blocking(move || datum_core::solve(&m).map(|r| small(&r))).await.map_err(|e| e.to_string())?
}

/// Results with their long arrays packed small for the window (`datum_core::wire`).
fn small<T: Serialize>(r: &T) -> serde_json::Value {
    datum_core::wire::encode(serde_json::to_value(r).unwrap_or_default())
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

fn pick(app: &AppHandle, name: &str, exts: &[&str]) -> Option<PathBuf> {
    app.dialog().file().add_filter(name, exts).blocking_pick_file().and_then(|f| f.into_path().ok())
}

#[tauri::command]
async fn open(app: AppHandle, st: State<'_, St>, path: Option<String>) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Datum Mesh model", &["dmesh"])) else { return Ok(None) };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let (m, warnings) = model::from_json(&text)?;
    remember(&app, &path);
    let mut a = st.lock();
    a.doc = Doc::new(m);
    a.doc.path = Some(path.display().to_string());
    let mut o = out(&a);
    o.warnings = warnings;
    Ok(Some(o))
}

#[tauri::command]
async fn save(app: AppHandle, st: State<'_, St>, as_new: bool) -> R<Option<Out>> {
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
    write_safely(&path, model::to_json(&a.doc.m).as_bytes()).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
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
fn autosave(app: AppHandle, st: State<St>) {
    let a = st.lock();
    if let (true, Some((file, meta))) = (a.doc.dirty, recovery_files(&app)) {
        let info = Recovery { name: a.doc.m.name.clone(), path: a.doc.path.clone(), saved: chrono::Local::now().format("%d-%b-%y %H:%M").to_string() };
        let _ = write_safely(&file, model::to_json(&a.doc.m).as_bytes());
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
fn recover(app: AppHandle, st: State<St>, keep: bool) -> R<Option<Out>> {
    let info = recovery(app.clone());
    let files = recovery_files(&app);
    if !keep {
        clear_recovery(&app);
        return Ok(None);
    }
    let (file, _) = files.ok_or("No recovered model was found.")?;
    let (m, warnings) = model::from_json(&std::fs::read_to_string(file).map_err(|e| e.to_string())?)?;
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
    std::env::args().skip(1).find(|a| a.to_lowercase().ends_with(".dmesh")).or_else(last)
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
            state, apply, history, examples_list, new_model, section_props, solve, mesh, materials_library, verification, open, save, save_file,
            options, set_options, autosave, recovery, recover, recent, startup_file, open_url, version, quit, check_update, install_update
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
