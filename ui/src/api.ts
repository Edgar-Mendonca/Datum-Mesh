// Calls into the Rust shell and the shapes it returns (they mirror crates/core/src/model.rs).
declare global {
  interface Window { __TAURI__?: { core: { invoke: (cmd: string, args?: object) => Promise<any> }; event: { listen: (e: string, f: (x: { payload: any }) => void) => Promise<() => void> } } }
}

export const invoke = <T = any>(cmd: string, args?: object): Promise<T> =>
  window.__TAURI__ ? window.__TAURI__.core.invoke(cmd, args) : Promise.reject("Datum Mesh must run inside its desktop window.");
export const listen = (e: string, f: (p: any) => void) => window.__TAURI__?.event.listen(e, x => f(x.payload));

export type Pt = [number, number];
export type Analysis = "frame" | "stress" | "strain" | "heat";
export interface Node { id: number; x: number; y: number }
export interface Member { id: number; a: number; b: number; kind: "beam" | "truss"; section: number; material: number }
export type SectionShape =
  | { kind: "rect"; b: number; h: number } | { kind: "circle"; d: number } | { kind: "tube"; d: number; t: number } | { kind: "box"; b: number; h: number; t: number }
  | { kind: "i"; h: number; b: number; tw: number; tf: number } | { kind: "channel"; h: number; b: number; tw: number; tf: number }
  | { kind: "angle"; h: number; b: number; t: number } | { kind: "custom"; a: number; i: number; c: number };
export interface Section { id: number; name: string; shape: SectionShape }
export interface Material { id: number; name: string; e: number; nu: number; rho: number; fy: number; k: number; alpha: number; source: string }
export type Geom = { kind: "poly"; pts: Pt[]; bulge: number[] } | { kind: "circle"; c: Pt; r: number };
export interface Shape { id: number; name: string; hole: boolean; material: number; geom: Geom }
export type Target = { on: "node"; id: number } | { on: "member"; id: number } | { on: "edge"; shape: number; edge: number } | { on: "point"; x: number; y: number } | { on: "body" };
export type SupportKind =
  | { kind: "fixed" } | { kind: "pinned" } | { kind: "rollerx" } | { kind: "rollery" } | { kind: "roller" } | { kind: "symmetry" }
  | { kind: "spring"; kx: number; ky: number; kr: number } | { kind: "temperature"; t: number };
export interface Support { id: number; at: Target; kind: SupportKind }
export type LoadKind =
  | { kind: "force"; fx: number; fy: number } | { kind: "moment"; m: number } | { kind: "dist"; w1: number; w2: number; dir: "y" | "x" | "local" }
  | { kind: "pressure"; p: number } | { kind: "traction"; fx: number; fy: number; total: boolean } | { kind: "gravity"; gx: number; gy: number }
  | { kind: "flux"; q: number } | { kind: "convection"; h: number; t_inf: number } | { kind: "heat"; q: number };
export interface Load { id: number; at: Target; kind: LoadKind }
export interface MeshSettings { size: number; order: number; min_angle: number; refine: { at: Target; size: number }[] }
export interface Model {
  format: number; name: string; notes: string; analysis: Analysis; units: "SI" | "US"; grid: number; thickness: number;
  nodes: Node[]; members: Member[]; shapes: Shape[]; sections: Section[]; materials: Material[]; supports: Support[]; loads: Load[];
  mesh: MeshSettings; next: number;
}
export interface Out { m: Model; path: string | null; dirty: boolean; undo: boolean; redo: boolean; created: number[]; warnings: string[] }
export interface Example { id: string; name: string; what: string; analysis: Analysis }
export interface Props { a: number; i: number; c: number }
export interface Options {
  autosave: number; beta: boolean; theme: string; openLast: boolean; units: "SI" | "US"; snap: boolean; grid: boolean; rulers: boolean;
  contours: "rainbow" | "viridis"; paper: string; landscape: boolean; company: string;
}

/** Rounding noise to zero: values smaller than a billionth of the scale they belong to. */
export const clean = (v: number, scale: number) => (Math.abs(v) <= 1e-9 * Math.abs(scale) ? 0 : v);

/** A number typed by a person: "−" and "," are understood ("1,5" is 1.5, "1,200" is 1200); empty or
 * not a number gives null. */
export const parseNum = (s: string): number | null => {
  let t = s.trim().replace(/[−–]/g, "-").replace(/\s/g, "");
  if (!t) return null;
  if (t.includes(",")) t = t.includes(".") || /^-?\d{1,3}(,\d{3})+$/.test(t) ? t.replace(/,/g, "") : t.replace(",", ".");
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
};

/** Numbers for people: up to 4 significant digits, no trailing zeros. */
export const num = (v: number, sig = 4) => {
  if (!isFinite(v)) return v > 0 ? "∞" : v < 0 ? "−∞" : "—";
  if (v === 0) return "0";
  const a = Math.abs(v);
  if (a >= 1e6 || a < 1e-3) return v.toExponential(Math.max(0, sig - 1)).replace(/\.?0+e/, "e");
  return String(+v.toPrecision(sig)).replace("-", "−");
};

// ---------- results ----------
export interface MemberResult { id: number; length: number; x: number[]; n: number[]; v: number[]; m: number[]; d: Pt[]; stress: number; util: number; ends: number[] }
export interface Reaction { node: number; fx: number; fy: number; m: number }
export interface SolveStats { equations: number; nonzeros: number; residual: number; ms: number }
export interface FrameResults { disp: [number, number, number][]; reactions: Reaction[]; members: MemberResult[]; applied: [number, number, number]; reacted: [number, number, number]; scale: [number, number]; imbalance: number; stats: SolveStats; warnings: string[] }
export interface Check { bench: number; name: string; quantity: string; q: string; reference: number; result: number; error: number; tolerance: number; source: string; pass: boolean }
