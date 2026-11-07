// Analysis steps (0.4.0): the dialog that lists them (Solve › Steps), and the load–deflection
// curve of a large-deflection solution (Results › Load–Deflection). The engine runs the steps and
// follows the path (crates/core/src/steps.rs, engine/path.rs, frames/corot.rs).
import { Step, StepKind, Which, DynInfo, num, parseNum } from "./api";
import { act, model, frameRes, spaceRes } from "./store";

export const KINDS: [StepKind, string][] = [["linear", "Linear (small deflections)"], ["newton", "Large deflection (P-delta)"], ["arc", "Large deflection, past limit loads"], ["spectrum", "Response spectrum (dynamics)"], ["history", "Time history (dynamics)"]];
export const isDyn = (k: StepKind) => k === "spectrum" || k === "history";
/** A table typed as lines of two numbers ("0.5, 2.5"), and back. */
export const toTable = (t: string): [number, number][] | null => { const rows = t.split(/\n+/).map(l => l.trim()).filter(Boolean).map(l => l.split(/[\s,;]+/).map(x => parseNum(x))); return rows.every(r => r.length === 2 && r[0] !== null && r[1] !== null) && rows.length ? rows.map(r => [r[0]!, r[1]!]) : null; };
export const fromTable = (t: [number, number][]) => t.map(p => `${num(p[0], 6)}, ${num(p[1], 6)}`).join("\n");
/** The dynamic results shown, if any. */
export const dynOf = (): DynInfo | undefined => frameRes()?.dynamic ?? spaceRes()?.dynamic;
export const key = (w: Which | null) => w ? `${w.on}:${w.id}` : "";
export const unkey = (k: string): Which | null => { const [on, id] = k.split(":"); return on === "case" || on === "combo" ? { on, id: +id } as Which : null; };

/** Does the model have large-deflection steps? */
export const isNonlinear = () => (model()?.steps ?? []).some(s => s.kind !== "linear");


/** A new step of a kind, with the usual settings. */
export const stepOf = (kind: StepKind): Step => ({ id: 0, name: "", kind, load: null, factor: 1, increments: 10, pieces: isDyn(kind) ? 4 : 10, monitor: null, along: [1, 0, 0], spectrum: [[0, 2.5], [0.5, 2.5], [4, 0.3125]], damping: 0.05, modes: 12, cqc: true, time: kind === "history" ? [[0, 0], [0.1, 1], [0.2, -1], [0.3, 0]] : kind === "explicit" ? [] : [[0, 1], [10, 1]], ground: kind === "history", dt: kind === "explicit" ? 0 : 0.005, duration: kind === "explicit" ? 0.001 : 2 });

/** The analyses each kind of model offers (the Analysis stage, 0.6.0): a step kind, its name, what it is for. */
export const analysesFor = (a: string): [StepKind, string, string][] => {
  const lin: [StepKind, string, string] = ["linear", "Static, linear", "Small movements, elastic materials: most design checks"];
  if (a === "frame") return [lin, ["newton", "Large deflection", "Second-order (P-delta) effects and large rotations, in increments"], ["arc", "Past limit loads", "Follows snap-through past the largest load, by arc length"], ["spectrum", "Earthquake (spectrum)", "Peak response to a design spectrum, modes combined by CQC"], ["history", "Time history", "Response to a record of ground shaking or a load in time"]];
  if (a === "space") return [lin, ["newton", "Large deflection", "Large rotations and second-order effects, in increments (P-delta with plates)"], ["arc", "Past limit loads", "Follows snap-through past the largest load, by arc length (no plates)"], ["spectrum", "Earthquake (spectrum)", "Peak response to a design spectrum, modes combined by CQC"], ["history", "Time history", "Response to a record of ground shaking or a load in time"]];
  if (a === "solid") return [lin, ["newton", "Elastic-plastic", "Yielding (von Mises) up to collapse, in increments"], ["largeplastic", "Crushing and collapse", "Yielding with large movements: a tube crushed, a part bent until it folds (meshed solids)"], ["large", "Large deformation", "Rubber, and parts that move or stretch far"], ["explicit", "Impact (explicit dynamics)", "Fast events in time: a sudden load, or a solid hitting at speed, stepped in time (meshed solids)"]];
  if (["stress", "strain", "axi"].includes(a)) return [lin, ["newton", "Elastic-plastic", "Yielding (von Mises) up to collapse, in increments"]];
  return [];
};
/** The analysis the model is set to (the last step's kind; linear without steps). */
export const curAnalysis = (): StepKind => { const st = model()?.steps ?? []; return st.length ? st[st.length - 1].kind : "linear"; };
/** Set the analysis: one step of that kind (none for static, linear), keeping a step's load and factor. */
export const setAnalysis = (kind: StepKind) => {
  const m = model(); if (!m || curAnalysis() === kind && (m.steps ?? []).length <= 1) return;
  const st = m.steps ?? [], keep = st[st.length - 1];
  act({ Batch: [...(st.length ? [{ Delete: { ids: st.map(s => s.id) } }] : []), ...(kind === "linear" ? [] : [{ Add: { item: { Step: { ...stepOf(kind), load: keep?.load ?? null, factor: keep?.factor ?? 1 } } } }])] });
};

// The dialogs live in stepdialogs.tsx, loaded when first opened (0.9.0: the window's startup
// code stays within its budget).
export const dynDialog = () => { import("./stepdialogs").then(k => k.dynDialog()); };
export const stepsDialog = () => { import("./stepdialogs").then(k => k.stepsDialog()); };
export const partPathDialog = () => { import("./stepdialogs").then(k => k.partPathDialog()); };
export const pathDialog = () => { import("./stepdialogs").then(k => k.pathDialog()); };
