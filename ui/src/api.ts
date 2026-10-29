// Calls into the Rust shell and the shapes it returns (they mirror crates/core/src/model.rs).
declare global {
  interface Window { __TAURI__?: { core: { invoke: (cmd: string, args?: object) => Promise<any> }; event: { listen: (e: string, f: (x: { payload: any }) => void) => Promise<() => void> } } }
}

export const invoke = <T = any>(cmd: string, args?: object): Promise<T> =>
  window.__TAURI__ ? window.__TAURI__.core.invoke(cmd, args) : Promise.reject("Datum Mesh must run inside its desktop window.");
export const listen = (e: string, f: (p: any) => void) => window.__TAURI__?.event.listen(e, x => f(x.payload));

export type Pt = [number, number];
export type Analysis = "frame" | "stress" | "strain" | "heat" | "axi" | "space" | "solid";
/** Frames and trusses: in the plane, or in space (0.2.4). */
export const isFrame = (a?: Analysis) => a === undefined || a === "frame" || a === "space";
export interface Node { id: number; x: number; y: number; z?: number }
export interface Member { id: number; a: number; b: number; kind: "beam" | "truss"; section: number; material: number; roll?: number }
export type SectionShape =
  | { kind: "rect"; b: number; h: number } | { kind: "circle"; d: number } | { kind: "tube"; d: number; t: number } | { kind: "box"; b: number; h: number; t: number }
  | { kind: "i"; h: number; b: number; tw: number; tf: number } | { kind: "channel"; h: number; b: number; tw: number; tf: number }
  | { kind: "angle"; h: number; b: number; t: number } | { kind: "custom"; a: number; i: number; c: number; iy?: number; j?: number };
export interface Section { id: number; name: string; shape: SectionShape }
export interface Material { id: number; name: string; e: number; nu: number; rho: number; fy: number; k: number; alpha: number; source: string }
export type Geom = { kind: "poly"; pts: Pt[]; bulge: number[] } | { kind: "circle"; c: Pt; r: number };
export interface Shape { id: number; name: string; hole: boolean; material: number; geom: Geom }
export type Target = { on: "node"; id: number } | { on: "member"; id: number } | { on: "edge"; shape: number; edge: number } | { on: "point"; x: number; y: number } | { on: "body" } | { on: "end"; front: boolean };
export type SupportKind =
  | { kind: "fixed" } | { kind: "pinned" } | { kind: "rollerx" } | { kind: "rollery" } | { kind: "roller" } | { kind: "symmetry" }
  | { kind: "spring"; kx: number; ky: number; kr: number; kz?: number; krx?: number; kry?: number } | { kind: "temperature"; t: number };
export interface Support { id: number; at: Target; kind: SupportKind }
export type LoadKind =
  | { kind: "force"; fx: number; fy: number; fz?: number } | { kind: "moment"; m: number; mx?: number; my?: number } | { kind: "dist"; w1: number; w2: number; dir: "y" | "x" | "z" | "local" }
  | { kind: "pressure"; p: number } | { kind: "traction"; fx: number; fy: number; total: boolean; fz?: number } | { kind: "bearing"; fx: number; fy: number } | { kind: "gravity"; gx: number; gy: number; gz?: number }
  | { kind: "warming"; dt: number } | { kind: "flux"; q: number } | { kind: "convection"; h: number; t_inf: number } | { kind: "heat"; q: number };
export interface Load { id: number; at: Target; kind: LoadKind }
export interface MeshSettings { size: number; order: number; min_angle: number; refine: { at: Target; size: number }[]; layers?: number }
export interface Model {
  format: number; name: string; notes: string; analysis: Analysis; units: "SI" | "US"; grid: number; thickness: number; t_ref: number;
  nodes: Node[]; members: Member[]; shapes: Shape[]; sections: Section[]; materials: Material[]; supports: Support[]; loads: Load[];
  mesh: MeshSettings; next: number;
}
export interface Out { m: Model; path: string | null; dirty: boolean; undo: boolean; redo: boolean; created: number[]; warnings: string[] }
export interface Example { id: string; name: string; what: string; analysis: Analysis }
export interface Props { a: number; i: number; c: number; iy: number; cy: number; j: number }
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
export interface Check { bench: number; name: string; quantity: string; q: string; reference: number; result: number; error: number; tolerance: number; source: string; pass: boolean; steps: [number, number][] }

// Parts: the mesh, nodal and element fields, and the results of plane stress, strain and heat.
export interface BEdge { shape: number; edge: number; n: [number, number, number] }
export interface Quality { min_angle: number; max_aspect: number; good: number }
export interface Mesh { nodes: Pt[]; tris: number[][]; order: number; mat: number[]; edges: BEdge[]; points: [Pt, number][]; size: number; quality: Quality }
export interface Field { key: string; v: number[]; min: number; max: number; at_min: Pt; at_max: Pt }
export interface PlaneReaction { support: number; fx: number; fy: number }
export interface PlaneResults { kind: "plane"; mesh: Mesh; draw: [number, number, number][]; disp: Pt[]; fields: Field[]; elem: Field[]; reactions: PlaneReaction[]; applied: [number, number, number]; reacted: [number, number, number]; scale: [number, number]; imbalance: number; stats: SolveStats; warnings: string[] }
export interface Flow { id: number; q: number }
export interface HeatResults { kind: "heat"; mesh: Mesh; draw: [number, number, number][]; fields: Field[]; elem: Field[]; supports: Flow[]; loads: Flow[]; imbalance: number; stats: SolveStats; warnings: string[] }
/** 3D frames (0.2.4): six movements per node, forces along and moments about both section axes. */
export type P3 = [number, number, number];
export interface SpaceMember { id: number; length: number; x: number[]; n: number[]; vy: number[]; vz: number[]; t: number[]; my: number[]; mz: number[]; d: P3[]; stress: number; util: number; axes: [P3, P3, P3] }
export interface SpaceResults { kind: "space"; disp: number[][]; reactions: { node: number; f: number[] }[]; members: SpaceMember[]; applied: number[]; reacted: number[]; scale: [number, number]; imbalance: number; stats: SolveStats; warnings: string[] }
/** 3D solids (0.3.0): the outer surface with its displacements and fields; min and max over every node. */
export interface Field3 { key: string; v: number[]; min: number; max: number; at_min: P3; at_max: P3 }
export interface SolidResults {
  kind: "solid"; surface: { nodes: P3[]; tris: [number, number, number][]; lines: [number, number][] }; disp: P3[]; fields: Field3[];
  reactions: { support: number; f: P3 }[]; applied: number[]; reacted: number[]; scale: [number, number]; imbalance: number; stats: SolveStats; warnings: string[];
  elements: number; nodes: number; layers: number; order: number; size: number; quality: Quality;
}
export type Results = (FrameResults & { kind: "frame" }) | PlaneResults | HeatResults | SpaceResults | SolidResults;
export type PartResults = PlaneResults | HeatResults;
export interface LibMaterial { id: string; name: string; family: string; e: number; nu: number; rho: number; fy: number; k: number; alpha: number; source: string }

// ---------- results on the wire ----------
/** Long arrays arrive packed as base64 (`crates/core/src/wire.rs`): `{$f32 | $u32, w}`; this
 * turns them back into arrays (rows of `w` when w > 1), everything else as it came. */
export const unwire = (v: any): any => {
  if (Array.isArray(v)) return v.map(unwire);
  if (v && typeof v === "object") {
    const s = v.$f32 ?? v.$u32;
    if (typeof s === "string") {
      const bin = atob(s), bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const nums = Array.from(v.$f32 !== undefined ? new Float32Array(bytes.buffer) : new Uint32Array(bytes.buffer));
      if (v.w === 1) return nums;
      const out: number[][] = [];
      for (let i = 0; i < nums.length; i += v.w) out.push(nums.slice(i, i + v.w));
      return out;
    }
    const o: any = {};
    for (const k in v) o[k] = unwire(v[k]);
    return o;
  }
  return v;
};
/** The drawing triangles of a mesh: 6-node triangles split in four, so curved fields read smoothly. */
export const drawTris = (ms: Mesh): [number, number, number][] => ms.tris.flatMap(t => (t.length === 6 ? [[t[0], t[3], t[5]], [t[3], t[1], t[4]], [t[5], t[4], t[2]], [t[3], t[4], t[5]]] : [[t[0], t[1], t[2]]]) as [number, number, number][]);
// ---------- natural frequencies (0.2.2) ----------
export interface FrameMode { f: number; disp: [number, number, number][]; members: { id: number; d: Pt[] }[] }
export interface PartMode { f: number; disp: Pt[] }
/** A buckling mode (0.2.3): the load factor λ (every load times λ buckles the frame) and its shape. */
export interface FrameBuckle { lambda: number; disp: [number, number, number][]; members: { id: number; d: Pt[] }[] }
/** A truss bar in compression checked on its own as a pinned strut. */
export interface BarBuckle { id: number; n: number; ncr: number; lambda: number }
export type ModeResults = { kind: "frame"; modes: FrameMode[]; equations: number; iterations: number; ms: number }
  | { kind: "plane"; mesh: Mesh; draw: [number, number, number][]; modes: PartMode[]; equations: number; iterations: number; ms: number }
  | { kind: "buckle"; modes: FrameBuckle[]; bars: BarBuckle[]; warnings: string[]; equations: number; iterations: number; ms: number }
  | { kind: "space"; modes: SpaceMode[]; equations: number; iterations: number; ms: number; warnings: string[] }
  | { kind: "buckle3"; modes: SpaceBuckle[]; bars: BarBuckle[]; warnings: string[]; equations: number; iterations: number; ms: number };
/** 3D frames (0.2.6): a mode or buckled shape with six movements per node and 3D member shapes. */
export interface SpaceMode { f: number; disp: number[][]; members: { id: number; d: P3[] }[] }
export interface SpaceBuckle { lambda: number; disp: number[][]; members: { id: number; d: P3[] }[] }
/** Buckling results, plane or 3D. */
export const isBuckle = (r: ModeResults): r is Extract<ModeResults, { kind: "buckle" | "buckle3" }> => r.kind === "buckle" || r.kind === "buckle3";
/** Modes as the shell sends them, made whole. */
export const modesFromWire = (raw: any): ModeResults => { const r = unwire(raw); if (r.kind === "plane") r.draw = drawTris(r.mesh); return r; };

/** Results as the shell sends them, made whole: arrays unpacked, drawing triangles added. */
export const fromWire = (raw: any): Results => {
  const r = unwire(raw);
  if (r.kind === "plane" || r.kind === "heat") r.draw = drawTris(r.mesh);
  return r;
};
