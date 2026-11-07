// Window state: the open model, the selection, the drawing tool, menus, dialogs and messages.
import { createSignal, createMemo, createRoot, batch, JSX } from "solid-js";
import { invoke, Out, Options, Model, Target, FrameResults, Results, PartResults, SpaceResults, SolidResults, SolidHeatResults, Mesh, ModeResults, Load, Which } from "./api";

export const [out, setOutRaw] = createSignal<Out | null>(null);
export const model = (): Model | undefined => out()?.m;
export const setOut = (o: Out | null | undefined) => {
  if (!o) return;
  batch(() => {
    setOutRaw(o);
    // Drop selected ids that no longer exist (deleted or undone away).
    const ids = new Set(allIds(o.m));
    if (sel().some(i => !ids.has(i))) setSel(sel().filter(i => ids.has(i)));
    if (edgeSel() && !o.m.shapes.some(s => s.id === edgeSel()!.shape)) setEdgeSel(null);
  });
  if (o.warnings?.length) toast(`Opened with repairs: ${o.warnings.join(" ")}`);
};
export const allIds = (m: Model) => [...m.nodes, ...m.members, ...m.shapes, ...m.supports, ...m.loads, ...(m.parts ?? []), ...m.materials, ...m.sections, ...(m.cases ?? []), ...(m.combos ?? []), ...(m.steps ?? [])].map(x => x.id);

/** auto, light or dark; auto follows Windows. */
export const [theme, setThemeRaw] = createSignal("auto");
const darkMq = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
export const applyTheme = () => { document.documentElement.dataset.theme = theme() === "auto" ? (darkMq?.matches ? "dark" : "light") : theme(); setThemeTick(t => t + 1); };
export const [themeTick, setThemeTick] = createSignal(0);
darkMq?.addEventListener("change", () => theme() === "auto" && applyTheme());
export const setTheme = (t: string) => { setThemeRaw(t); applyTheme(); const o = { ...opts(), theme: t }; setOpts(o); invoke("set_options", { opts: o }).catch(() => {}); };
/** 3D frames (0.7.5): draw each member as its section swept along it (else as a line). */
export const [sectionsOn, setSectionsOn] = createSignal(true);
export const [opts, setOpts] = createSignal<Options>({ autosave: 5, beta: false, theme: "auto", openLast: false, units: "SI", snap: true, grid: true, rulers: true, contours: "rainbow", paper: "A4", landscape: true, company: "" });

export type View = "model" | "mesh" | "results" | "report";
export const [view, setView] = createSignal<View>("model");
export const [backstage, setBackstage] = createSignal<string | null>("home");
export const [tab, setTab] = createSignal("home");

// ---------- selection and tools ----------
/** Selected items by id (nodes, members, shapes, supports, loads). */
export const [sel, setSel] = createSignal<number[]>([]);
/** A selected edge of a shape (for edge supports and loads). */
export const [edgeSel, setEdgeSel] = createSignal<{ shape: number; edge: number } | null>(null);
/**
 * The canvas tool: "select", drawing tools ("node", "beam", "truss", "rect", "circle", "poly", "hrect", "hcircle"),
 * or placing a support or load ("sup:<kind>", "load:<kind>").
 */
export const [tool, setTool] = createSignal("select");
/** What the pointer is over, for the status bar. */
export const [cursor, setCursor] = createSignal<[number, number] | null>(null);
export const selTargets = (): Target[] => {
  const m = model(); if (!m) return [];
  const s = new Set(sel());
  const t: Target[] = [...m.nodes.filter(n => s.has(n.id)).map(n => ({ on: "node", id: n.id }) as Target), ...m.members.filter(n => s.has(n.id)).map(n => ({ on: "member", id: n.id }) as Target)];
  if (edgeSel()) t.push({ on: "edge", ...edgeSel()! });
  return t;
};

// ---------- results ----------
/** The last solve, and whether the model has changed since. */
export const [rawResults, setResults] = createSignal<Results | null>(null);
/** Playing results (0.6.4): the player's view of the results at its position (player.tsx), or
 * null to show them as solved. Everything drawn reads `results`, so it plays along. */
export const [playView, setPlayView] = createSignal<((r: Results) => Results) | null>(null);
/** The result as solved, for one being played (its scale for drawing is the whole result's). */
export const solved = <T extends Results>(r: T): T => { const x = rawResults(); return (x && x.kind === r.kind ? x : r) as T; };
export const results = createRoot(() => createMemo(() => { const r = rawResults(), f = playView(); return r && f ? f(r) : r; }));
export const frameRes = (): FrameResults | null => { const r = results(); return r?.kind === "frame" ? r : null; };
export const partRes = (): PartResults | null => { const r = results(); return r && (r.kind === "plane" || r.kind === "heat") ? r : null; };
export const spaceRes = (): SpaceResults | null => { const r = results(); return r?.kind === "space" ? r : null; };
export const solidRes = (): SolidResults | SolidHeatResults | null => { const r = results(); return r?.kind === "solid" || r?.kind === "solidheat" ? r : null; };
export const [stale, setStale] = createSignal(false);
/**
 * What the results show. Frames: deflected shape, a diagram, reactions, utilisation. Parts: the
 * contoured field, smoothed (averaged at nodes) or per element, the deformed shape, mesh lines.
 */
export type Show = { deformed: boolean; diagram: "none" | "n" | "v" | "m"; reactions: boolean; util: boolean; scale: number; field: string; elem: boolean; mesh: boolean; quality: boolean };
export const [show, setShow] = createSignal<Show>({ deformed: true, diagram: "m", reactions: true, util: false, scale: 1, field: "vm", elem: false, mesh: false, quality: true });
export const [solving, setSolving] = createSignal(false);
/** Natural frequencies (0.2.2): the modes found, the one shown, whether they are on the drawing
 * (instead of the static results), whether they move, and the phase of the movement (−1 to 1). */
export const [modes, setModes] = createSignal<ModeResults | null>(null);
export const [modesStale, setModesStale] = createSignal(false);
export const [modeSel, setModeSel] = createSignal(0);
export const [modeOn, setModeOn] = createSignal(false);
export const [animate, setAnimate] = createSignal(true);
export const [phase, setPhase] = createSignal(1);
/** The part's mesh without results (Mesh view), and whether the model changed since. */
export const [meshed, setMeshed] = createSignal<Mesh | null>(null);
export const [meshStale, setMeshStale] = createSignal(false);

// ---------- load cases (0.3.3) ----------
export const [activeRaw, setActiveRaw] = createSignal<Which | null>(null);
/** The load case or combination shown and solved: the one picked if it still exists, else the
 * first case; null when the model has no cases (every load together). */
export const active = (): Which | null => {
  const m = model(); if (!m || !m.cases?.length) return null;
  const a = activeRaw();
  if (a && (a.on === "case" ? m.cases.some(c => c.id === a.id) : (m.combos ?? []).some(c => c.id === a.id))) return a;
  return { on: "case", id: m.cases[0].id };
};
/** Pick the case or combination to show; results for another one become out of date. */
export const setActive = (w: Which | null) => { setActiveRaw(w); if (results()) setStale(true); };
/** The case a load is in (its own if it exists, else the first). */
export const caseOf = (m: Model, l: Load) => m.cases?.some(c => c.id === l.case) ? l.case! : (m.cases?.[0]?.id ?? 0);
/** Is a load drawn under the case or combination picked? */
export const shownLoad = (m: Model, l: Load) => {
  const a = active(); if (!a) return true;
  const c = caseOf(m, l);
  return a.on === "case" ? c === a.id : (m.combos ?? []).find(x => x.id === a.id)?.parts.some(p => p.case === c) ?? true;
};
/** The case a new load goes in: the one picked, or a combination's first. */
const newCase = () => {
  const a = active(), m = model(); if (!a || !m) return 0;
  return a.on === "case" ? a.id : (m.combos ?? []).find(x => x.id === a.id)?.parts[0]?.case ?? m.cases![0].id;
};
/** What a case or combination is called. */
export const whichName = (m: Model, w: Which | null) => {
  if (!w) return "All loads";
  const cn = (id: number) => m.cases?.find(c => c.id === id)?.name ?? "?";
  if (w.on === "case") return cn(w.id);
  const c = m.combos?.find(x => x.id === w.id); if (!c) return "?";
  return `${c.name}: ${c.parts.map(p => `${+p.f.toFixed(3)} × ${cn(p.case)}`).join(" + ") || "no cases yet"}`;
};
/** The case or combination the shown results are for. */
export const [solvedFor, setSolvedFor] = createSignal<string>("");
/** Sketching (0.6.2): what is being sketched and on which plane. A part's sketch lies in its own
 * x-y plane, placed with the part; a 3D frame's on a working plane (xy Front, zy Side, xz Top) at a
 * depth. `facing`: the view looks square at the plane and the sketch canvas lies over it. `edits`
 * counts the edits made while sketching, so Cancel can undo them. */
export type SketchOn = { kind: "part" | "frame"; part: number; isNew: boolean; plane: "xy" | "zy" | "xz"; depth: number; facing: boolean; edits: number;
  /** Sketching on a part's face (0.6.3): that part, whether it is its front or back face (a cut can go through it), and where the new part's plane starts. */
  face?: { of: number; end: boolean }; base?: [number, number, number] };
/** Basic modelling (0.6.3): the hole size and the round or bevel size of the 3D tools, and the part whose extrusion is being set (0: none). */
export const [holeD, setHoleD] = createSignal(0.01);
export const [edgeSize, setEdgeSize] = createSignal(0.002);
export const [sweepOn, setSweepOn] = createSignal(0);
export const [sketch, setSketch] = createSignal<SketchOn | null>(null);
/** New Sketch was asked for: a plane is to be picked in the 3D view. */
export const [sketchPick, setSketchPick] = createSignal(false);
/** Assemblies (0.6.0): the part whose sketch the Section view shows and edits. */
export const [activePart, setActivePart] = createSignal(0);
/** The active part, if the model has parts (else 0, the model's own solid). */
export const curPart = () => { const ps = model()?.parts ?? []; return ps.length ? (ps.find(p => p.id === activePart()) ?? ps[0]).id : 0; };
/** New shapes go in the part being sketched. */
const withPart = (op: any): any => {
  if (op?.Batch) return { Batch: op.Batch.map(withPart) };
  const p = curPart(); if (!p) return op;
  const s = op?.Add?.item?.Shape;
  if (s && !s.plate && !s.part) return { Add: { item: { Shape: { ...s, part: p } } } };
  if (op?.Make && !op.Make.part) return { Make: { ...op.Make, part: p } };
  return op;
};
const withCase = (op: any): any => {
  if (op?.Batch) return { Batch: op.Batch.map(withCase) };
  const l = op?.Add?.item?.Load;
  return l && l.case === undefined ? { Add: { item: { Load: { ...l, case: newCase() } } } } : op;
};

// ---------- editing ----------
export const act = async (op: object, quiet = false): Promise<Out | null> => {
  op = withPart(withCase(op));
  try { const o = await invoke<Out>("apply", { op }); setOut(o); const sk = sketch(); if (sk) setSketch({ ...sk, edits: sk.edits + 1 }); if (results()) setStale(true); if (meshed()) setMeshStale(true); if (modes()) setModesStale(true); return o; }
  catch (e) { if (!quiet) toast(String(e)); return null; }
};
export const call = async <T = any>(cmd: string, args?: object): Promise<T | undefined> => {
  try { return await invoke<T>(cmd, args); } catch (e) { toast(String(e)); return undefined; }
};
export const loadOut = async (cmd: string, args?: object) => {
  const o = await call<Out | null>(cmd, args);
  if (o) batch(() => { setSel([]); setEdgeSel(null); setTool("select"); setActiveRaw(null); setResults(null); setStale(false); setMeshed(null); setMeshStale(false); setModes(null); setModeOn(false); setModesStale(false); setOut(o); setBackstage(null); setView("model"); });
  // A model opened or started afresh brings its own geometry (0.7.1): cad.tsx reloads it.
  if (o) window.dispatchEvent(new Event("cad-reload"));
  return o;
};

// ---------- messages, menus, dialogs ----------
export const [msg, setMsg] = createSignal<{ text: string; ok: boolean } | null>(null);
let timer = 0;
/** Messages (0.9.1): everything said to the person is kept, with its time, in the bottom panel. */
export type Note = { at: number; level: "info" | "warn" | "error"; text: string };
export const [notes, setNotes] = createSignal<Note[]>([]);
export const note = (text: string, level: Note["level"] = "info") => setNotes(v => [...v.slice(-499), { at: Date.now(), level, text }]);
export const toast = (text: string, ok = false) => { note(text, ok ? "info" : "warn"); setMsg({ text, ok }); clearTimeout(timer); timer = window.setTimeout(() => setMsg(null), ok ? 3000 : 7000); };
export type Item = { label: string; run?: () => void; keys?: string; off?: boolean; on?: boolean; sub?: Item[]; icon?: string } | "-";
export const [menu, setMenu] = createSignal<{ x: number; y: number; items: Item[] } | null>(null);
export const openMenu = (e: MouseEvent | { x: number; y: number }, items: Item[]) => {
  if ("preventDefault" in e) e.preventDefault();
  const r = (e as MouseEvent).currentTarget instanceof HTMLElement && (e as MouseEvent).type === "click" ? ((e as MouseEvent).currentTarget as HTMLElement).getBoundingClientRect() : null;
  setMenu({ x: r ? r.left : (e as MouseEvent).clientX ?? (e as any).x, y: r ? r.bottom + 2 : (e as MouseEvent).clientY ?? (e as any).y, items });
};
export const [dialogs, setDialogs] = createSignal<(() => JSX.Element)[]>([]);
export const openDialog = (d: () => JSX.Element) => setDialogs(v => [...v, d]);
export const closeDialog = () => setDialogs(v => v.slice(0, -1));
