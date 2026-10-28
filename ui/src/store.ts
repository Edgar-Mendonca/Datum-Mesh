// Window state: the open model, the selection, the drawing tool, menus, dialogs and messages.
import { createSignal, batch, JSX } from "solid-js";
import { invoke, Out, Options, Model, Target, FrameResults, Results, PartResults, Mesh } from "./api";

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
export const allIds = (m: Model) => [...m.nodes, ...m.members, ...m.shapes, ...m.supports, ...m.loads].map(x => x.id);

/** auto, light or dark; auto follows Windows. */
export const [theme, setThemeRaw] = createSignal("auto");
const darkMq = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
export const applyTheme = () => { document.documentElement.dataset.theme = theme() === "auto" ? (darkMq?.matches ? "dark" : "light") : theme(); setThemeTick(t => t + 1); };
export const [themeTick, setThemeTick] = createSignal(0);
darkMq?.addEventListener("change", () => theme() === "auto" && applyTheme());
export const setTheme = (t: string) => { setThemeRaw(t); applyTheme(); const o = { ...opts(), theme: t }; setOpts(o); invoke("set_options", { opts: o }).catch(() => {}); };
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
export const [results, setResults] = createSignal<Results | null>(null);
export const frameRes = (): FrameResults | null => { const r = results(); return r?.kind === "frame" ? r : null; };
export const partRes = (): PartResults | null => { const r = results(); return r && r.kind !== "frame" ? r : null; };
export const [stale, setStale] = createSignal(false);
/**
 * What the results show. Frames: deflected shape, a diagram, reactions, utilisation. Parts: the
 * contoured field, smoothed (averaged at nodes) or per element, the deformed shape, mesh lines.
 */
export type Show = { deformed: boolean; diagram: "none" | "n" | "v" | "m"; reactions: boolean; util: boolean; scale: number; field: string; elem: boolean; mesh: boolean; quality: boolean };
export const [show, setShow] = createSignal<Show>({ deformed: true, diagram: "m", reactions: true, util: false, scale: 1, field: "vm", elem: false, mesh: false, quality: true });
export const [solving, setSolving] = createSignal(false);
/** The part's mesh without results (Mesh view), and whether the model changed since. */
export const [meshed, setMeshed] = createSignal<Mesh | null>(null);
export const [meshStale, setMeshStale] = createSignal(false);

// ---------- editing ----------
export const act = async (op: object, quiet = false): Promise<Out | null> => {
  try { const o = await invoke<Out>("apply", { op }); setOut(o); if (results()) setStale(true); if (meshed()) setMeshStale(true); return o; }
  catch (e) { if (!quiet) toast(String(e)); return null; }
};
export const call = async <T = any>(cmd: string, args?: object): Promise<T | undefined> => {
  try { return await invoke<T>(cmd, args); } catch (e) { toast(String(e)); return undefined; }
};
export const loadOut = async (cmd: string, args?: object) => {
  const o = await call<Out | null>(cmd, args);
  if (o) batch(() => { setSel([]); setEdgeSel(null); setTool("select"); setResults(null); setStale(false); setMeshed(null); setMeshStale(false); setOut(o); setBackstage(null); setView("model"); });
  return o;
};

// ---------- messages, menus, dialogs ----------
export const [msg, setMsg] = createSignal<{ text: string; ok: boolean } | null>(null);
let timer = 0;
export const toast = (text: string, ok = false) => { setMsg({ text, ok }); clearTimeout(timer); timer = window.setTimeout(() => setMsg(null), ok ? 3000 : 7000); };
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
