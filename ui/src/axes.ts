// Z up in 3D (0.6.1). The engine and the .dmesh file keep y up (plane frames and sections are
// drawn in x-y), but in 3D models (3D frames, 3D solids, 3D heat) the window shows and takes
// coordinates with Z up, as CAD programs do: display (X, Y, Z) = (x, −z, y), a turn about x that
// keeps the axes right-handed. The drawing itself does not move (three.js keeps y up on screen);
// only what is written and typed is mapped, here and nowhere else.
import { model } from "./store";

export type V3 = [number, number, number];

/** Is the model a 3D one, shown Z up? */
export const zUp = () => ["space", "solid", "solidheat"].includes(model()?.analysis ?? "");
const z0 = (v: number) => v || 0;
/** An engine point or vector as shown. */
export const toD = (p: V3): V3 => (zUp() ? [z0(p[0]), z0(-p[2]), z0(p[1])] : p);
/** A shown point or vector back in engine terms. */
export const fromD = (q: V3): V3 => (zUp() ? [z0(q[0]), z0(q[2]), z0(-q[1])] : q);
/** The shown name of each shown axis. */
export const AX = ["x", "y", "z"] as const;
/** For shown axis k: the engine axis it is and the sign (X = x, Y = −z, Z = y). */
export const engineAxis = (k: number): [number, number] => (zUp() ? ([[0, 1], [2, -1], [1, 1]] as [number, number][])[k] : [k, 1]);

/** A stress or flux field's shown key and how it is made from the engine's fields (key, sign),
 * for 3D models: σY is the engine's σz, τXY = −τzx, and so on. */
export const FIELD_MAP: Record<string, [string, number]> = {
  ux: ["ux", 1], uy: ["uz", -1], uz: ["uy", 1],
  sx: ["sx", 1], sy: ["sz", 1], sz: ["sy", 1], txy: ["tzx", -1], tyz: ["tyz", -1], tzx: ["txy", 1],
  qx: ["qx", 1], qy: ["qz", -1], qz: ["qy", 1],
};

/** A 3D solid's results as shown: component fields remapped (the deformed shape, drawn from the
 * displacements, is not touched). Marked so it is done once. */
export const mapFields = <R extends { fields: { key: string; v: number[]; min: number; max: number }[] }>(r: R): R => {
  if ((r as any).__zup) return r;
  const by = new Map(r.fields.map(f => [f.key, f]));
  const fields = r.fields.map(f => {
    const m = FIELD_MAP[f.key]; if (!m) return f;
    const src = by.get(m[0]); if (!src) return f;
    const v = m[1] === 1 ? src.v : src.v.map(x => -x);
    return { ...f, v, min: m[1] === 1 ? src.min : -src.max, max: m[1] === 1 ? src.max : -src.min };
  });
  return { ...r, fields, __zup: true } as R;
};
