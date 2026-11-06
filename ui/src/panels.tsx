// The model tree (left) and the Inspector (right): every item of the model, and the values of the
// selected one with their units. Edits go to the engine as Set operations.
import { For, Show, JSX, createMemo } from "solid-js";
import { Model, Node, Member, Shape, Support, Load, SupportKind, LoadKind, Target, Geom, Part, num, parseNum, isFrame, isSolidA } from "./api";
import { tab, setTab, model, sel, setSel, edgeSel, setEdgeSel, act, setTool, results, stale, setView, meshed, partRes, solidRes, caseOf, openMenu, setActivePart } from "./store";
import { stepsDialog } from "./steps";
import { Q, toU, fromU, unit, fmt } from "./units";
import { Help } from "./help";
import { zUp, toD, fromD, AX, engineAxis, V3 } from "./axes";
import { edgeCount, edgePts, outline, area } from "./geom";
import { C } from "./cmds";
import * as D from "./dialogs";
import { sharpCorners, corners } from "./sketch";

export const ANALYSES: [string, string][] = [["frame", "Frame or truss"], ["space", "3D frame or truss"], ["stress", "Plane stress"], ["strain", "Plane strain"], ["axi", "Axisymmetric (turned about y)"], ["heat", "Heat conduction"], ["solid", "3D solid (extruded or revolved)"], ["solidheat", "3D heat conduction (extruded or revolved)"]];

/** A new support of a kind, with typical values. */
export const defaultSupport = (k: string): SupportKind => {
  if (k === "spring") return { kind: "spring", kx: 1e7, ky: 1e7, kr: 0 };
  if (k === "temperature") return { kind: "temperature", t: 293.15 };
  if (k === "hold") return { kind: "hold", x: true, y: true, z: false };
  return { kind: k } as SupportKind;
};
/** A new load of a kind, with typical values (frames in kN, parts smaller). */
export const defaultLoad = (k: string, frame: boolean): LoadKind => {
  switch (k) {
    case "moment": return { kind: "moment", m: 10e3 };
    case "dist": return { kind: "dist", w1: -5e3, w2: -5e3, dir: "y" };
    case "pressure": return { kind: "pressure", p: 1e6 };
    case "platepressure": return { kind: "pressure", p: 5e3 };
    case "mass": return { kind: "mass", m: 1000 };
    case "settlement": return { kind: "settlement", dx: 0, dy: -0.01, dz: 0 };
    case "traction": return { kind: "traction", fx: 0, fy: -1e3, total: true };
    case "bearing": return { kind: "bearing", fx: 0, fy: 1e3 };
    case "gravity": return { kind: "gravity", gx: 0, gy: -9.81 };
    case "flux": return { kind: "flux", q: 1000 };
    case "convection": return { kind: "convection", h: 25, t_inf: 293.15 };
    case "heat": return { kind: "heat", q: 1e5 };
    case "warming": return { kind: "warming", dt: 50 };
    default: return { kind: "force", fx: 0, fy: frame ? -10e3 : -1e3 };
  }
};

export const SUPPORT_NAMES: Record<string, string> = { fixed: "Fixed", pinned: "Pinned", rollerx: "Roller (moves along x)", rollery: "Roller (moves along y)", roller: "Roller (slides along the edge)", symmetry: "Symmetry", spring: "Spring", temperature: "Temperature", hold: "Held directions" };
export const LOAD_NAMES: Record<string, string> = { force: "Force", moment: "Moment", dist: "Distributed load", pressure: "Pressure", traction: "Edge force", bearing: "Pin load", gravity: "Gravity (self-weight)", flux: "Heat flux", convection: "Convection", heat: "Heat generation", warming: "Temperature change", mass: "Mass", settlement: "Settlement" };

const targetName = (m: Model, t: Target) => {
  if (t.on === "node") return `Node ${m.nodes.findIndex(n => n.id === t.id) + 1}`;
  if (t.on === "member") return `Member ${m.members.findIndex(n => n.id === t.id) + 1}`;
  if (t.on === "edge" && m.assembly) { const f = m.assembly.targets.findIndex(x => x?.on === "edge" && x.shape === t.shape && x.edge === t.edge); if (f >= 0) return m.assembly.faces[f].name; }
  if (t.on === "edge") return `${m.shapes.find(s => s.id === t.shape)?.name ?? "Shape"}, edge ${t.edge + 1}`;
  if (t.on === "point") return `Point ${fmt("coord", t.x)}, ${fmt("coord", t.y)}`;
  if (t.on === "end") return t.front ? "Front face" : "Back face";
  if (t.on === "partend") { const a = m.assembly, f = a ? a.targets.findIndex(x => x?.on === "partend" && x.part === t.part && x.front === t.front) : -1; return f >= 0 ? a!.faces[f].name : `${m.parts?.find(p => p.id === t.part)?.name ?? "Part"}, ${t.front ? "end" : "start"} face`; }
  if (t.on === "face") return m.volume?.faces[t.face]?.name ?? `Face ${t.face + 1}`;
  if (t.on === "plate") return `${m.shapes.find(s => s.id === t.shape)?.name ?? "Plate"}, whole face`;
  if (t.on === "line") return `${m.shapes.find(s => s.id === t.shape)?.name ?? "Shape"}, edge ${t.edge + 1}, line at ${num(t.at * 100, 3)} %`;
  return "Whole model";
};
export const itemName = (m: Model, id: number): string => {
  const i = (xs: { id: number }[]) => xs.findIndex(x => x.id === id);
  if (i(m.nodes) >= 0) return `Node ${i(m.nodes) + 1}`;
  if (i(m.members) >= 0) return `Member ${i(m.members) + 1}`;
  const s = m.shapes.find(x => x.id === id); if (s) return s.name;
  const sp = m.supports.find(x => x.id === id); if (sp) return `${SUPPORT_NAMES[sp.kind.kind]} · ${targetName(m, sp.at)}`;
  const l = m.loads.find(x => x.id === id); if (l) return `${LOAD_NAMES[l.kind.kind]} · ${targetName(m, l.at)}`;
  return "";
};

// ---------- the tree ----------
// 0.6.0: one branch per part of the model, in the order a model is built; each with an icon, a
// count, a mark that says whether it is done (✓) or needs something (!, with the reason), an
// "add" row while it is empty, and a menu on the right button.
type Mark = { ok: boolean; why?: string } | null;
const Ic = (p: { n: string }) => <svg class="tic"><use href={`#i-${p.n}`} /></svg>;
const Branch = (p: { t: string; icon: string; n?: number; note?: string; mark?: Mark; help?: string; add?: () => void; addText?: string; open?: boolean; children?: JSX.Element }) => (
  <details class="tgrp" open={p.open ?? (p.n === undefined || p.n <= 12)}>
    <summary title={p.mark && !p.mark.ok ? p.mark.why : undefined}><Ic n={p.icon} /><span class="tt">{p.t}</span><Show when={p.n !== undefined}><span class="pill">{p.n}</span></Show>
      <Show when={p.note}><small class="tnote2">{p.note}</small></Show><span class="grow" />
      <Show when={p.mark}>{mk => <i class={`tmark ${mk().ok ? "ok" : "need"}`} title={mk().ok ? "Done" : mk().why}>{mk().ok ? "✓" : "!"}</i>}</Show>
      {p.help && <Help k={p.help} />}{p.add && <button class="tadd" title={`Add: ${p.t}`} onClick={e => { e.preventDefault(); p.add!(); }}>+</button>}</summary>
    <div class="tlist">{p.children}<Show when={p.add && p.addText && !p.n}><button class="ti ghost" onClick={() => p.add!()}><Ic n="new" /><span>{p.addText}</span></button></Show></div>
  </details>
);
const Leaf = (p: { id: number; label: string; sub?: string; icon: string; menu?: [string, () => void][] }) => (
  <button class="ti" classList={{ on: sel().includes(p.id) }} onClick={e => setSel(e.ctrlKey || e.shiftKey ? [...new Set([...sel(), p.id])] : [p.id])}
    onContextMenu={e => { setSel([p.id]); openMenu(e, [...(p.menu ?? []).map(([label, run]) => ({ label, run })), { label: "Delete", run: () => act({ Delete: { ids: [p.id] } }) }]); }}>
    <Ic n={p.icon} /><span>{p.label}</span>{p.sub && <small>{p.sub}</small>}</button>
);
const Row2 = (p: { icon: string; label: string; sub?: string; run?: () => void; on?: boolean }) => (
  <button class="ti" classList={{ on: !!p.on }} onClick={() => p.run?.()}><Ic n={p.icon} /><span>{p.label}</span>{p.sub && <small>{p.sub}</small>}</button>
);
const SUP_ICON: Record<string, string> = { fixed: "fixed", pinned: "pinned", rollerx: "roller", rollery: "roller", roller: "roller", symmetry: "symmetry", spring: "spring", temperature: "temp", hold: "roller" };
const LOAD_ICON: Record<string, string> = { force: "force", moment: "moment", dist: "dist", pressure: "pressure", traction: "edgeforce", bearing: "pin", gravity: "gravity", flux: "flux", convection: "convection", heat: "heatgen", warming: "temp", mass: "mass", settlement: "settle" };
const STEP_NAME: Record<string, string> = { linear: "Linear (small deflections)", newton: "Large deflection", arc: "Past limit loads", spectrum: "Response spectrum", history: "Time history", large: "Large deformation" };
const plural = (n: number, one: string, many = one + "s") => `${n} ${n === 1 ? one : many}`;

// 0.7.1: the tree is laid out in numbered steps, in the order a model is made; each step's
// header turns its ribbon tab on, shows ✓ when it is done or says what it still needs.
export const STEP_TABS = ["geometry", "properties", "loads", "mesh", "solve", "results"];
const Step = (p: { n: number; t: string; done: boolean; why?: string; children?: JSX.Element }) => (
  <section class="step" classList={{ here: tab() === STEP_TABS[p.n - 1], done: p.done }}>
    <button class="sthead" onClick={() => setTab(STEP_TABS[p.n - 1])} title={p.done ? `Step ${p.n}: done` : p.why}>
      <b class="stn">{p.done ? "✓" : p.n}</b><span>{p.t}</span><Show when={!p.done && p.why}><small>{p.why}</small></Show></button>
    <div class="stbody">{p.children}</div>
  </section>
);

export const Tree = () => {
  const m = () => model();
  const frame = () => isFrame(m()?.analysis);
  const sp = () => m()?.analysis === "space";
  const heat = () => m()?.analysis === "heat" || m()?.analysis === "solidheat";
  const part = () => !frame() && !heat();
  const plastic = () => part() && (m()?.steps ?? []).some(s => s.kind === "newton");
  const parts = () => (m()?.shapes ?? []).filter(s => !s.plate);
  const plates = () => (m()?.shapes ?? []).filter(s => s.plate);
  const drawn = () => frame() ? (m()?.members.length ?? 0) > 0 : parts().some(s => !s.hole);
  const sups = () => (m()?.supports ?? []).filter(s => heat() ? s.kind.kind === "temperature" : s.kind.kind !== "temperature");
  const mesh = () => solidRes() && !solidRes()!.layers ? `${solidRes()!.elements.toLocaleString()} tetrahedra` : m()?.assembly ? `${m()!.assembly!.elements.toLocaleString()} tetrahedra in ${m()!.parts!.length} parts` : solidRes() ? `${solidRes()!.elements.toLocaleString()} wedges, ${solidRes()!.layers} ${solidRes()!.layers === 1 ? "layer" : "layers"}` : (meshed() ?? partRes()?.mesh) ? `${(meshed() ?? partRes()!.mesh).tris.length.toLocaleString()} ${isSolidA(m()!.analysis) ? "triangles in the section" : "elements"}` : "Not meshed yet: made when you solve";
  const noYield = () => plastic() ? (m()?.materials ?? []).filter(x => !(x.fy > 0) && (frame() ? m()!.members.some(b => b.material === x.id) : parts().some(s => s.material === x.id))) : [];
  return (
    <aside class="tree">
      <header>Steps<Help k="tree" /></header>
      <div class="tbody">
        <Show when={m()}>{mm => <>
          <div class="troot" title="Model › Name in the Inspector (nothing selected)" onClick={() => setSel([])}><img src="logo.svg" alt="" /><b>{mm().name}</b><small>{ANALYSES.find(a => a[0] === mm().analysis)?.[1]}</small></div>
          <Step n={1} t="Geometry" done={drawn() || !!mm().volume || (mm().parts?.length ?? 0) > 0} why={frame() ? "Draw the members" : "Draw or import the part"}>
          <Show when={!mm().volume} fallback={<Branch t="Imported mesh" icon="solid" n={mm().volume!.faces.length} help="meshimport" mark={{ ok: true }}>
            <For each={mm().volume!.faces}>{(f, i) => <Row2 icon="rect" label={f.name} sub={fmt("area", f.area)} on={mm().supports.concat(mm().loads as any).some((x: any) => sel().includes(x.id) && x.at.on === "face" && x.at.face === i())} />}</For></Branch>}>
          <Show when={frame() || !(isSolidA(mm().analysis) && (mm().parts?.length || !parts().length))} fallback={
            <Branch t="Parts" icon="solid" n={mm().parts?.length ?? 0} help="parts" add={newPart} addText="Add a part: sketch it, then extrude or revolve it" mark={{ ok: (mm().parts?.length ?? 0) > 0 && !mm().assembly_error, why: mm().assembly_error ?? "Add a part: Geometry › New Part" }}>
              <For each={mm().parts ?? []}>{pt => <Leaf id={pt.id} icon={pt.angle > 0 ? "axi" : "solid"} label={pt.name} sub={`${pt.angle > 0 ? `revolved ${num(pt.angle * 180 / Math.PI, 4)}°` : `extruded ${fmt("len", pt.depth)}`} · ${mm().materials.find(x => x.id === pt.material)?.name ?? ""}`} />}</For></Branch>}>
          <Show when={frame()} fallback={
            <Branch t={heat() ? "Part" : isSolidA(mm().analysis) ? "Section" : "Shapes"} icon={isSolidA(mm().analysis) ? "solid" : mm().analysis === "axi" ? "axi" : "part"} n={parts().length} help="shapes" add={() => setTool("rect")} addText="Draw the outline (Geometry)" mark={{ ok: drawn(), why: "Draw the part's outline: Geometry › Rectangle, Circle or Polygon" }}>
              <For each={parts()}>{s => <Leaf id={s.id} icon={s.hole ? (s.geom.kind === "circle" ? "hcircle" : "hrect") : s.geom.kind === "circle" ? "circle" : "poly"} label={s.name} sub={s.hole ? "hole" : s.geom.kind === "circle" ? "circle" : `${s.geom.pts.length} corners`} />}</For></Branch>}>
            <Branch t="Members" icon="frame" n={mm().members.length} open={mm().members.length <= 12} help="members" add={() => setTool("beam")} addText="Draw members (Geometry › Beam)" mark={{ ok: drawn(), why: "Draw members: Geometry › Beam or Truss Bar" }}>
              <For each={mm().members}>{(b, i) => <Leaf id={b.id} icon={b.kind === "truss" ? "truss" : "beam"} label={`Member ${i() + 1}`} sub={`${b.kind} · ${mm().sections.find(s => s.id === b.section)?.name ?? ""}`} />}</For></Branch>
            <Show when={sp() && plates().length}><Branch t="Plates" icon="rect" n={plates().length} help="plates"><For each={plates()}>{s => <Leaf id={s.id} icon={s.hole ? "hrect" : "rect"} label={s.name} sub={s.hole ? "opening" : `${fmt("len", s.plate?.t ?? 0)} thick`} />}</For></Branch></Show>
            <Branch t="Nodes" icon="node" n={mm().nodes.length} help="nodes" add={() => setTool("node")} open={mm().nodes.length <= 12}><For each={mm().nodes}>{(n, i) => <Leaf id={n.id} icon="node" label={`Node ${i() + 1}`} sub={sp() ? toD([n.x, n.y, n.z ?? 0]).map(c => num(toU("coord", c))).join(", ") : `${num(toU("coord", n.x))}, ${num(toU("coord", n.y))}`} />}</For></Branch>
          </Show>
          </Show>
          </Show>
          </Step>
          <Step n={2} t="Properties" done={mm().materials.length > 0 && !noYield().length && (!frame() || mm().sections.length > 0)} why="Pick a material">
          <Branch t="Materials" icon="material" n={mm().materials.length} help="materials" add={() => D.materials()} mark={noYield().length ? { ok: false, why: `${noYield()[0].name} needs a yield strength for the elastic-plastic step` } : null}>
            <For each={mm().materials}>{x => <Row2 icon="material" label={x.name} sub={noYield().includes(x) ? "no yield strength" : undefined} run={() => D.materials(x.id)} />}</For></Branch>
          <Show when={frame()}><Branch t="Sections" icon="section" n={mm().sections.length} help="sections" add={() => D.sections()}><For each={mm().sections}>{x => <Row2 icon="section" label={x.name} run={() => D.sections(x.id)} />}</For></Branch></Show>
          </Step>
          <Step n={3} t={heat() ? "Temperatures & Heat" : "Supports & Loads"} done={sups().length > 0 && (heat() || mm().loads.length > 0)} why={sups().length ? "Add a load" : "Add a support"}>
          <Branch t={heat() ? "Temperatures" : "Supports"} icon={heat() ? "temp" : "fixed"} n={sups().length} help="supports" add={() => setTool(heat() ? "sup:temperature" : "sup:fixed")} addText={heat() ? "Fix a temperature (Loads tab)" : "Add a support (Loads › Fixed, Pinned…)"}
            mark={drawn() ? { ok: sups().length > 0 || (heat() && mm().loads.some(l => l.kind.kind === "convection")), why: heat() ? "Fix a temperature on an edge, or add convection: heat needs somewhere to go" : "Add supports: nothing holds the model yet" } : null}>
            <For each={sups()}>{s => <Leaf id={s.id} icon={SUP_ICON[s.kind.kind] ?? "fixed"} label={SUPPORT_NAMES[s.kind.kind]} sub={targetName(mm(), s.at)} />}</For></Branch>
          <Branch t={heat() ? "Heat loads" : "Loads"} icon={heat() ? "flux" : "force"} n={mm().loads.length} help="loads" add={() => setTool(heat() ? "load:flux" : "load:force")} addText={heat() ? "Add heat flux or convection (Loads tab)" : "Add a load (Loads › Force, Pressure…)"}
            mark={drawn() && !heat() ? { ok: mm().loads.length > 0, why: "Add a load (natural frequencies need none)" } : null}>
            <Show when={(mm().cases ?? []).length > 1} fallback={<For each={mm().loads}>{l => <Leaf id={l.id} icon={LOAD_ICON[l.kind.kind] ?? "force"} label={LOAD_NAMES[l.kind.kind]} sub={targetName(mm(), l.at)} />}</For>}>
              <For each={mm().cases!}>{c => <details class="tsub" open><summary><Ic n="list" /><span>{c.name}</span><span class="pill">{mm().loads.filter(l => caseOf(mm(), l) === c.id).length}</span></summary>
                <For each={mm().loads.filter(l => caseOf(mm(), l) === c.id)}>{l => <Leaf id={l.id} icon={LOAD_ICON[l.kind.kind] ?? "force"} label={LOAD_NAMES[l.kind.kind]} sub={targetName(mm(), l.at)} />}</For></details>}</For></Show></Branch>
          <Show when={(mm().combos ?? []).length}><Branch t="Combinations" icon="table" n={mm().combos!.length} help="cases" open={false}><For each={mm().combos!}>{c => <Row2 icon="table" label={c.name} sub={c.parts.map(x => `${num(x.f, 3)} ${mm().cases?.find(k => k.id === x.case)?.name ?? ""}`).join(" + ")} />}</For></Branch></Show>
          </Step>
          <Step n={4} t="Mesh" done={frame() || !!mm().volume || !!mm().assembly || !!(meshed() ?? partRes()?.mesh) || !!solidRes()} why="Made when you solve, or Mesh › Mesh now">
          <Show when={frame()}><Row2 icon="frame" label="Each member is one element" sub="no mesh needed" /></Show>
          <Show when={!frame()}><Branch t="Mesh" icon="mesh" n={mm().mesh.refine.length || undefined} help="mesh">
            <Show when={mm().volume}>{v => <Row2 icon="solid" label={`${v().elements.toLocaleString()} ${v().order === 2 ? "10-node" : "4-node"} tetrahedra`} sub={`${v().nodes.toLocaleString()} nodes, imported`} />}</Show>
            <Show when={!mm().volume}><Row2 icon={mm().mesh.order === 2 ? "tri6" : "tri3"} label={mesh()} sub={mm().mesh.size > 0 ? fmt("len", mm().mesh.size) : "automatic size"} run={() => C.mesh()} />
            <For each={mm().mesh.refine}>{(r, i) => <div class="ti refrow"><Ic n="refine" /><span>Finer near {targetName(mm(), r.at)}</span>
              <input class="mono" title={`Element size there (${unit("len")})`} value={num(toU("len", r.size), 4)} onChange={numIn(v => v > 0 && C.meshSettings({ refine: mm().mesh.refine.map((x, k) => k === i() ? { ...x, size: fromU("len", v) } : x) }), () => num(toU("len", r.size), 4))} />
              <button class="x" title="Remove" onClick={() => C.meshSettings({ refine: mm().mesh.refine.filter((_, k) => k !== i()) })}>×</button></div>}</For></Show>
          </Branch></Show>
          </Step>
          <Step n={5} t="Analysis" done={true}>
          <Show when={heat()}><Row2 icon="solve" label="Steady heat conduction" sub="one solve" /></Show>
          <Show when={!heat()}><Branch t="Steps" icon="list" n={(mm().steps ?? []).length || undefined} help="steps">
            <Show when={(mm().steps ?? []).length} fallback={<Row2 icon="solve" label="Static, linear" sub="one solve" run={stepsDialog} />}>
              <For each={mm().steps!}>{(s, i) => <Row2 icon={s.kind === "spectrum" || s.kind === "history" ? "wave" : s.kind === "linear" ? "solve" : "curve"} label={s.name || `Step ${i() + 1}`} sub={`${STEP_NAME[s.kind] ?? s.kind}${s.kind === "newton" || s.kind === "arc" || s.kind === "large" ? ` · ${plural(s.increments, "increment")}` : ""}`} run={stepsDialog} />}</For></Show></Branch></Show>
          </Step>
          <Step n={6} t="Results" done={!!results() && !stale()} why={results() ? "Out of date: solve again (F5)" : "Solve (F5) to see the results"}>
          <Branch t="Results" icon="contour" help="results" mark={results() ? { ok: !stale(), why: "The model changed since it was solved: solve again (F5)" } : null}>
            <Show when={results()} fallback={<button class="ti ghost" onClick={() => C.solve()}><Ic n="solve" /><span>Not solved yet: Solve (F5)</span></button>}>
              <Row2 icon="solve" label={stale() ? "Out of date" : "Solved"} sub={`${results()!.stats.equations.toLocaleString()} equations`} />
              <Row2 icon="deform" label={frame() ? "Deflected shape and diagrams" : heat() ? "Temperatures" : "Contours on the drawing"} run={() => setView("model")} />
              <Row2 icon="table" label="Tables and equilibrium" run={() => setView("results")} />
              <Row2 icon="report" label="Report" run={() => setView("report")} /></Show></Branch>
          </Step>
        </>}</Show>
      </div>
    </aside>
  );
};

// ---------- the inspector ----------
/** A change handler for a number input: understands "−" and "," and puts the old text back when
 * the value cannot be read or is refused (0.1.1 bug: negative corners could not be edited). */
export const numIn = (f: (v: number) => unknown, shown: () => string | number) => (e: Event & { currentTarget: HTMLInputElement }) => {
  const v = parseNum(e.currentTarget.value);
  if (v === null || f(v) === false) e.currentTarget.value = String(shown());
};

/** A number shown in display units and stored in SI. */
export const NumField = (p: { label: string; q: Q; v: number; set: (v: number) => void; hint?: string; min?: number }) => (
  <div class="iprow"><label>{p.label}<span class="unit">{unit(p.q)}</span></label>
    <input class="mono" value={num(toU(p.q, p.v), 6).replace("−", "-")} onChange={e => {
      const v = parseNum(e.currentTarget.value);
      if (v === null || (p.min !== undefined && v < p.min)) { e.currentTarget.value = num(toU(p.q, p.v), 6).replace("−", "-"); return; }
      p.set(fromU(p.q, v));
    }} />{p.hint && <span class="hint">{p.hint}</span>}</div>
);
const Row = (p: { label: string; children: JSX.Element }) => <div class="iprow"><label>{p.label}</label>{p.children}</div>;
const Choice = (p: { v: string; set: (v: string) => void; opts: [string, string][] }) => (
  <select value={p.v} onChange={e => p.set(e.currentTarget.value)}><For each={p.opts}>{([k, l]) => <option value={k} selected={k === p.v}>{l}</option>}</For></select>
);
const set = (kind: string, item: object) => act({ Set: { item: { [kind]: item } } });

/** A vector in the Inspector (0.6.1): shown in the shown axes (Z up in 3D), kept in the
 * engine's; `pre` is put before each axis name (Fx, Mz, gx…). */
const Vec3 = (p: { pre: string; q: Q; v: V3; set: (v: V3) => void; hint?: string; post?: string }) => {
  const d = () => toD(p.v);
  return <>{[0, 1, 2].map(k => <NumField label={`${p.pre}${AX[k]}${p.post ?? ""}`} q={p.q} v={d()[k]} set={x => { const n = [...d()] as V3; n[k] = x; p.set(fromD(n)); }} hint={k === 2 ? p.hint : undefined} />)}</>;
};

const NodeInsp = (p: { n: Node; m: Model }) => (
  <>
    <Show when={p.m.analysis === "space"} fallback={<>
      <NumField label="x" q="coord" v={p.n.x} set={x => set("Node", { ...p.n, x })} />
      <NumField label="y" q="coord" v={p.n.y} set={y => set("Node", { ...p.n, y })} /></>}>
      <Vec3 pre="" q="coord" v={[p.n.x, p.n.y, p.n.z ?? 0]} set={v => set("Node", { ...p.n, x: v[0], y: v[1], z: v[2] })} hint="Z is up." />
    </Show>
    <Attached m={p.m} at={{ on: "node", id: p.n.id }} />
  </>
);

const MemberInsp = (p: { b: Member; m: Model }) => {
  const a = () => p.m.nodes.find(n => n.id === p.b.a)!, b = () => p.m.nodes.find(n => n.id === p.b.b)!;
  const sp = () => p.m.analysis === "space";
  const len = () => Math.hypot(b().x - a().x, b().y - a().y, (b().z ?? 0) - (a().z ?? 0));
  return (
    <>
      <Row label="Kind"><Choice v={p.b.kind} set={v => set("Member", { ...p.b, kind: v })} opts={[["beam", "Beam (bending and axial)"], ["truss", "Truss bar (axial only)"]]} /></Row>
      <Row label="Section"><Choice v={String(p.b.section)} set={v => set("Member", { ...p.b, section: +v })} opts={p.m.sections.map(s => [String(s.id), s.name])} /></Row>
      <Row label="Material"><Choice v={String(p.b.material)} set={v => set("Member", { ...p.b, material: +v })} opts={p.m.materials.map(s => [String(s.id), s.name])} /></Row>
      <Row label="Carries"><Choice v={p.b.only ?? "both"} set={v => set("Member", { ...p.b, only: v as any })} opts={[["both", "Tension and compression"], ["tension", "Tension only (a rod, a cable)"], ["compression", "Compression only (a strut on a bearing)"]]} /></Row>
      <Show when={p.b.kind === "beam"}><div class="cks">
        <label class="chk" title="The member carries no bending moment at its start (a pinned connection)"><input type="checkbox" checked={p.b.hinge?.[0] ?? false} onChange={e => set("Member", { ...p.b, hinge: [e.currentTarget.checked, p.b.hinge?.[1] ?? false] })} />Hinge at the start</label>
        <label class="chk" title="The member carries no bending moment at its end (a pinned connection)"><input type="checkbox" checked={p.b.hinge?.[1] ?? false} onChange={e => set("Member", { ...p.b, hinge: [p.b.hinge?.[0] ?? false, e.currentTarget.checked] })} />Hinge at the end</label></div></Show>
      <Show when={p.b.kind === "beam"}>
        <NumField label="Rigid end at the start" q="len" v={p.b.offset?.[0] ?? 0} min={0} set={v => v >= 0 && set("Member", { ...p.b, offset: [v, p.b.offset?.[1] ?? 0] })} hint="The length inside a deep column or beam at its start, where it does not bend (an end offset); 0 for none." />
        <NumField label="Rigid end at the end" q="len" v={p.b.offset?.[1] ?? 0} min={0} set={v => v >= 0 && set("Member", { ...p.b, offset: [p.b.offset?.[0] ?? 0, v] })} />
      </Show>
      <Show when={sp() && p.b.kind === "beam"}><div class="iprow"><label>Roll<span class="unit">°</span></label><input class="mono" value={num((p.b.roll ?? 0) * 180 / Math.PI)} onChange={numIn(v => set("Member", { ...p.b, roll: v * Math.PI / 180 }), () => num((p.b.roll ?? 0) * 180 / Math.PI))} /><span class="hint">Turns the section about the member. At 0 the depth h stands upright, or lies along z for a column.</span></div></Show>
      <table class="vmini"><tbody>
        <tr><td>Length</td><td class="r">{fmt("len", len())}</td></tr>
        <Show when={!sp()} fallback={<tr><td>Slope</td><td class="r">{num(Math.asin(Math.max(-1, Math.min(1, (b().y - a().y) / len()))) * 180 / Math.PI)}° up</td></tr>}><tr><td>Angle</td><td class="r">{num(Math.atan2(b().y - a().y, b().x - a().x) * 180 / Math.PI)}°</td></tr></Show>
        <tr><td>From · to</td><td class="r">{itemName(p.m, p.b.a)} · {itemName(p.m, p.b.b)}</td></tr>
      </tbody></table>
      <Attached m={p.m} at={{ on: "member", id: p.b.id }} />
    </>
  );
};

/** A new part (0.6.0): added, made the one being sketched, and its sketch opened to draw in. */
export const newPart = () => import("./sketchmode").then(k => k.newSketch());

/** An angle shown and typed in degrees, kept in radians. */
const DegField = (p: { label: string; v: number; set: (v: number) => void; hint?: string }) => (
  <div class="iprow"><label>{p.label}<span class="unit">°</span></label>
    <input class="mono" value={num(p.v * 180 / Math.PI, 6).replace("−", "-")} onChange={e => { const v = parseNum(e.currentTarget.value); if (v === null) { e.currentTarget.value = num(p.v * 180 / Math.PI, 6); return; } p.set(v * Math.PI / 180); }} />{p.hint && <span class="hint">{p.hint}</span>}</div>
);

/** Which way a part's sweep goes from its sketch (0.6.3): out of it, into it, or both ways. */
const dirOf = (p: Part) => { const l = p.angle > 0 ? p.angle : p.depth, f = p.from ?? 0; return Math.abs(f) < 1e-12 ? "one" : Math.abs(f + l) < 1e-9 * Math.max(1, l) ? "other" : "both"; };

/** A part of an assembly (0.6.0): how its sketch is swept, its material and size, where it is. */
const PartInsp = (p: { p: Part; m: Model }) => {
  setActivePart(p.p.id);
  const put = (ch: Partial<Part>) => set("Part", { ...p.p, ...ch });
  const rev = () => p.p.angle > 0;
  const shapes = () => p.m.shapes.filter(s => s.part === p.p.id);
  return <>
    <Row label="Name"><input value={p.p.name} onChange={e => put({ name: e.currentTarget.value })} /></Row>
    <p class="muted small">{shapes().length ? `Its sketch: ${shapes().filter(s => !s.hole).length} outline${shapes().filter(s => !s.hole).length === 1 ? "" : "s"}${shapes().some(s => s.hole) ? `, ${shapes().filter(s => s.hole).length} hole${shapes().filter(s => s.hole).length === 1 ? "" : "s"}` : ""}.` : "Its sketch is empty: draw its outline."}</p>
    <p class="ibtns"><button class="btn sm pri" title="Draw or change this part's outline and holes, on its own sketch plane (x across, y up)" onClick={() => import("./sketchmode").then(k => k.editSketch(p.p.id))}>Edit Sketch</button></p>
    <Row label="Made by"><Choice v={rev() ? "revolve" : "extrude"} set={v => put(v === "revolve" ? { angle: Math.PI * 2, depth: p.p.depth } : { angle: 0, depth: p.p.depth > 0 ? p.p.depth : 0.1 })} opts={[["extrude", "Extruding the sketch along its z"], ["revolve", "Revolving the sketch about its y axis"]]} /></Row>
    <Show when={rev()} fallback={<NumField label="Depth" q="len" v={p.p.depth} min={0} set={depth => depth > 0 && put({ depth })} hint="How far the sketch is extruded, along the sketch's z" />}>
      <DegField label="Turned through" v={p.p.angle} set={angle => angle > 0 && angle <= 360.0001 * Math.PI / 180 && put({ angle: Math.min(angle, Math.PI * 2) })} hint="360° for a whole ring or shaft; draw the sketch right of its y axis" />
    </Show>
    <Row label="Which way"><Choice v={dirOf(p.p)} set={v => { const l = rev() ? p.p.angle : p.p.depth; put({ from: v === "one" ? 0 : v === "other" ? -l : -l / 2 }); }} opts={[["one", rev() ? "One way" : "Out of the sketch"], ["other", rev() ? "The other way" : "Into the sketch"], ["both", "Both ways, evenly"]]} /></Row>
    <p class="ibtns"><button class="btn sm" title="Set the extrusion or revolve with a preview in 3D" onClick={() => import("./modelling").then(k => k.editSweep(p.p.id))}>Extrude…</button></p>
    <Row label="Material"><Choice v={String(p.p.material)} set={v => put({ material: +v })} opts={p.m.materials.map(s => [String(s.id), s.name])} /></Row>
    <Show when={p.m.part_volume?.[String(p.p.id)] !== undefined}>{(() => { const v = () => p.m.part_volume?.[String(p.p.id)] ?? 0, rho = () => p.m.materials.find(x => x.id === p.p.material)?.rho ?? 0; return <table class="vmini"><tbody><tr><td>Volume</td><td class="r">{fmt("vol", v())}</td></tr><tr><td>Mass</td><td class="r">{fmt("mass", v() * rho())}</td></tr></tbody></table>; })()}</Show>
    <NumField label="Element size" q="len" v={p.p.size} min={0} set={size => put({ size })} hint="0: the model's size (Mesh); thin walls want two or three elements across" />
    <p class="isub">Where it is</p>
    <Vec3 pre="Moved by " q="coord" v={p.p.at} set={at => put({ at })} />
    <Show when={zUp()} fallback={<>
      <DegField label="Turned about x" v={p.p.turn[0]} set={a => put({ turn: [a, p.p.turn[1], p.p.turn[2]] })} hint="The part is turned about x, then y, then z, then moved" />
      <DegField label="Turned about y" v={p.p.turn[1]} set={a => put({ turn: [p.p.turn[0], a, p.p.turn[2]] })} />
      <DegField label="Turned about z" v={p.p.turn[2]} set={a => put({ turn: [p.p.turn[0], p.p.turn[1], a] })} /></>}>
      <DegField label="Turned about X" v={p.p.turn[0]} set={a => put({ turn: [a, p.p.turn[1], p.p.turn[2]] })} hint="Its sketch starts flat in the X-Z plane (upright, facing you). It is turned about X, then Z, then Y, then moved." />
      <DegField label="Turned about Z" v={p.p.turn[1]} set={a => put({ turn: [p.p.turn[0], a, p.p.turn[2]] })} />
      <DegField label="Turned about Y" v={-p.p.turn[2]} set={a => put({ turn: [p.p.turn[0], p.p.turn[1], -a] })} />
    </Show>
    <p class="muted small">Parts that touch are tied together where they touch. Put supports and loads on a part's faces: pick one on the Loads tab, then click the face in 3D.</p>
  </>;
};

const ShapeInsp = (p: { s: Shape; m: Model }) => {
  const g = () => p.s.geom;
  const put = (geom: Geom) => set("Shape", { ...p.s, geom });
  return (
    <>
      <Row label="Name"><input value={p.s.name} onChange={e => set("Shape", { ...p.s, name: e.currentTarget.value })} /></Row>
      <label class="chk"><input type="checkbox" checked={p.s.hole} onChange={e => set("Shape", { ...p.s, hole: e.currentTarget.checked })} />{p.s.plate ? "An opening (cut out of the plates on its plane)" : "A hole (cut out of the part)"}</label>
      <Show when={p.s.plate}>{pl => <>
        <NumField label="Thickness" q="len" v={pl().t} min={0} set={t => t > 0 && set("Shape", { ...p.s, plate: { ...pl(), t } })} hint={`A plate on the ${({ xy: "Front", zy: "Side", xz: "Plan" } as const)[pl().plane]} plane at ${({ xy: "z", zy: "x", xz: "y" } as const)[pl().plane]} = ${fmt("coord", pl().depth)}. Thin-plate (Kirchhoff) bending with membrane action; it joins beams and other plates where their nodes meet.`} />
      </>}</Show>
      <Show when={!p.s.hole}><Row label="Material"><Choice v={String(p.s.material)} set={v => set("Shape", { ...p.s, material: +v })} opts={p.m.materials.map(s => [String(s.id), s.name])} /></Row></Show>
      <Show when={g().kind === "circle"}>{_ => { const c = g() as Extract<Geom, { kind: "circle" }>; return <>
        <NumField label="Centre x" q="coord" v={c.c[0]} set={x => put({ ...c, c: [x, c.c[1]] })} />
        <NumField label="Centre y" q="coord" v={c.c[1]} set={y => put({ ...c, c: [c.c[0], y] })} />
        <NumField label="Radius" q="coord" v={c.r} min={0} set={r => put({ ...c, r })} />
      </>; }}</Show>
      <Show when={g().kind === "poly"}>{_ => { const pg = () => g() as Extract<Geom, { kind: "poly" }>; return <>
        <h4>Corners<Help k="corners" /></h4>
        <table class="ptbl"><thead><tr><th>#</th><th>x ({unit("coord")})</th><th>y</th><th title="The angle the edge after this corner turns through as an arc; 0 = straight">Arc °</th><th /></tr></thead><tbody>
          <For each={pg().pts}>{(pt, i) => <tr>
            <td>{i() + 1}</td>
            <td><input class="mono" value={num(toU("coord", pt[0]), 6).replace("−", "-")} onChange={numIn(v => put({ ...pg(), pts: pg().pts.map((q, k) => k === i() ? [fromU("coord", v), q[1]] : q) }), () => num(toU("coord", pt[0]), 6).replace("−", "-"))} /></td>
            <td><input class="mono" value={num(toU("coord", pt[1]), 6).replace("−", "-")} onChange={numIn(v => put({ ...pg(), pts: pg().pts.map((q, k) => k === i() ? [q[0], fromU("coord", v)] : q) }), () => num(toU("coord", pt[1]), 6).replace("−", "-"))} /></td>
            <td><input class="mono" value={num(4 * Math.atan(pg().bulge[i()] ?? 0) * 180 / Math.PI).replace("−", "-")} onChange={numIn(v => Math.abs(v) <= 180 && put({ ...pg(), bulge: pg().bulge.map((b, k) => k === i() ? Math.tan(v * Math.PI / 720) : b) }), () => num(4 * Math.atan(pg().bulge[i()] ?? 0) * 180 / Math.PI).replace("−", "-"))} /></td>
            <td><button class="x" title="Remove this corner" disabled={pg().pts.length <= 3} onClick={() => act({ RemoveCorner: { shape: p.s.id, corner: i() } })}>×</button><button class="x" title="Add a corner in the middle of the next edge" onClick={() => act({ InsertCorner: { shape: p.s.id, edge: i() } })}>+</button><Show when={sharpCorners(pg()).includes(i())}><button class="x" title="Round or bevel this corner" onClick={() => { setSel([p.s.id]); corners(true, i()); }}>⌒</button></Show></td>
          </tr>}</For></tbody></table>
        <p class="muted small">+ adds a corner halfway along the next edge (on the arc, for an arc); ⌒ rounds the corner; supports and loads stay on their edges.</p>
      </>; }}</Show>
      <table class="vmini"><tbody><tr><td>Area</td><td class="r">{fmt("area", Math.abs(g().kind === "circle" ? Math.PI * (g() as any).r ** 2 : area(outline(g(), 1e-4))))}</td></tr><tr><td>Edges</td><td class="r">{edgeCount(g())}</td></tr></tbody></table>
    </>
  );
};

/** Supports and loads on a node, member or edge, with quick buttons to add more. */
const Attached = (p: { m: Model; at: Target }) => {
  const same = (t: Target) => JSON.stringify(t) === JSON.stringify(p.at);
  const sups = () => p.m.supports.filter(s => same(s.at)), loads = () => p.m.loads.filter(s => same(s.at));
  return <>
    <h4>On it</h4>
    <For each={[...sups(), ...loads()]} fallback={<p class="muted small">No supports or loads.</p>}>{x => <button class="ti" onClick={() => setSel([x.id])}><span>{itemName(p.m, x.id).split(" · ")[0]}</span></button>}</For>
  </>;
};

const EdgeInsp = (p: { m: Model; e: { shape: number; edge: number } }) => {
  const s = () => p.m.shapes.find(x => x.id === p.e.shape);
  const len = () => { const pts = edgePts(s()!.geom, p.e.edge, 1e-4); return pts.slice(1).reduce((a, q, i) => a + Math.hypot(q[0] - pts[i][0], q[1] - pts[i][1]), 0); };
  return <Show when={s()}>
    <table class="vmini"><tbody><tr><td>Shape</td><td class="r">{s()!.name}</td></tr><tr><td>Length</td><td class="r">{fmt("len", len())}</td></tr></tbody></table>
    <Attached m={p.m} at={{ on: "edge", ...p.e }} />
    <p class="muted small">Pick a support or a load on the Loads tab, then click this edge.</p>
  </Show>;
};

const SupportInsp = (p: { s: Support; m: Model }) => {
  const k = () => p.s.kind;
  const sol = () => isSolidA(p.m.analysis);
  const kinds = (): string[] => p.m.analysis === "solidheat" ? ["temperature"] : p.s.at.on === "line" ? ["fixed", "hold"] : p.s.at.on === "end" ? ["fixed", "roller", "symmetry", "hold"] : p.s.at.on === "edge" ? (p.m.analysis === "heat" ? ["temperature"] : sol() ? ["fixed", "pinned", "roller", "symmetry", "hold"] : ["fixed", "pinned", "roller", "symmetry", "temperature"]) : p.m.analysis === "heat" ? ["temperature"] : isFrame(p.m.analysis) ? ["fixed", "pinned", "rollerx", "rollery", "spring"] : ["fixed", "pinned", "rollerx", "rollery"];
  const put = (kind: SupportKind) => set("Support", { ...p.s, kind });
  return <>
    <Row label="On"><span>{targetName(p.m, p.s.at)}</span></Row>
    <Row label="Kind"><Choice v={k().kind} set={v => put(defaultSupport(v))} opts={kinds().map(x => [x, SUPPORT_NAMES[x]])} /></Row>
    <Show when={k().kind === "spring"}>{_ => { const sp = k() as Extract<SupportKind, { kind: "spring" }>; return <>
      <NumField label="kx" q="spring" v={sp.kx} min={0} set={kx => put({ ...sp, kx })} />
      <NumField label="ky" q="spring" v={sp.ky} min={0} set={ky => put({ ...sp, ky })} />
      <Show when={p.m.analysis === "space"}><NumField label="kz" q="spring" v={sp.kz ?? 0} min={0} set={kz => put({ ...sp, kz })} /><NumField label="Turning about x" q="rspring" v={sp.krx ?? 0} min={0} set={krx => put({ ...sp, krx })} /><NumField label="Turning about y" q="rspring" v={sp.kry ?? 0} min={0} set={kry => put({ ...sp, kry })} /></Show>
      <NumField label={p.m.analysis === "space" ? "Turning about z" : "Rotation"} q="rspring" v={sp.kr} min={0} set={kr => put({ ...sp, kr })} />
    </>; }}</Show>
    <Show when={k().kind === "temperature"}><NumField label="Temperature" q="temp" v={(k() as any).t} set={t => put({ kind: "temperature", t })} /></Show>
    <Show when={k().kind === "hold"}>{_ => { const h = k() as Extract<SupportKind, { kind: "hold" }>; return <div class="cks">
      {[0, 1, 2].map(k => { const a = (["x", "y", "z"] as const)[engineAxis(k)[0]]; return <label class="chk"><input type="checkbox" checked={h[a]} onChange={e => { const v = { ...h, [a]: e.currentTarget.checked }; if (v.x || v.y || v.z) put(v); else e.currentTarget.checked = true; }} />Hold {AX[k]}</label>; })}</div>; }}</Show>
    <Show when={sol() && (p.s.at.on === "edge" || p.s.at.on === "line")}>{_ => { const at = p.s.at as any; return <>
      <Row label="Where"><Choice v={at.on} set={v => set("Support", { ...p.s, at: v === "line" ? { on: "line", shape: at.shape, edge: at.edge, at: 0.5 } : { on: "edge", shape: at.shape, edge: at.edge }, kind: v === "line" && !["fixed", "hold"].includes(k().kind) ? defaultSupport("hold") : k() })} opts={[["edge", "The whole side face"], ["line", "A line along it"]]} /></Row>
      <Show when={at.on === "line"}><div class="iprow"><label>Line at<span class="unit">%</span></label><input class="mono" value={num(at.at * 100, 4)} onChange={numIn(v => v >= 0 && v <= 100 && set("Support", { ...p.s, at: { ...at, at: v / 100 } }), () => num(at.at * 100, 4))} /><span class="hint">of the way {p.m.revolve ? "round the turn" : "through the thickness"} (50 % is the middle; it goes to the nearest layer of nodes).</span></div></Show>
    </>; }}</Show>
    <p class="muted small">{({ fixed: "Holds the point still; beams cannot turn there either.", pinned: "Holds the point still but lets it turn.", rollerx: "Free to slide along x; held in y.", rollery: "Free to slide along y; held in x.", roller: "Free to slide along the edge; held across it.", symmetry: "A plane of symmetry: model half the part and hold the cut edge across.", hold: "Holds only the ticked directions (global x, y, z); the others are free.", spring: p.m.analysis === "space" ? "Springs to the ground along x, y and z and turning about each." : "Springs to the ground in x, y and rotation.", temperature: "Holds the edge at this temperature." } as Record<string, string>)[k().kind]}</p>
  </>;
};

const LoadInsp = (p: { l: Load; m: Model }) => {
  const k = () => p.l.kind;
  const put = (kind: LoadKind) => set("Load", { ...p.l, kind });
  const f = <K extends LoadKind["kind"]>(_: K) => k() as Extract<LoadKind, { kind: K }>;
  const sp = () => p.m.analysis === "space";
  return <>
    <Row label="On"><span>{targetName(p.m, p.l.at)}</span></Row>
    <Row label="Kind"><span>{LOAD_NAMES[k().kind]}</span></Row>
    <Show when={p.m.cases?.length}><Row label="Load case"><Choice v={String(caseOf(p.m, p.l))} set={v => set("Load", { ...p.l, case: +v })} opts={(p.m.cases ?? []).map(c => [String(c.id), c.name])} /></Row></Show>
    <Show when={k().kind === "force" && sp()}><Vec3 pre="F" q="force" v={[f("force").fx, f("force").fy, f("force").fz ?? 0]} set={v => put({ ...f("force"), fx: v[0], fy: v[1], fz: v[2] })} hint="Up (Z) is positive." /></Show>
    <Show when={k().kind === "force" && !sp()}><NumField label="Fx" q="force" v={f("force").fx} set={fx => put({ ...f("force"), fx })} /><NumField label="Fy" q="force" v={f("force").fy} set={fy => put({ ...f("force"), fy })} hint={sp() ? undefined : "Up is positive."} /></Show>
    <Show when={k().kind === "moment"}><Show when={sp()} fallback={<NumField label="M" q="moment" v={f("moment").m} set={m => put({ kind: "moment", m })} hint="Counter-clockwise is positive." />}>
      <Vec3 pre="M" q="moment" v={[f("moment").mx ?? 0, f("moment").my ?? 0, f("moment").m]} set={v => put({ ...f("moment"), mx: v[0], my: v[1], m: v[2] })} hint="About the global axes, by the right-hand rule." /></Show></Show>
    <Show when={k().kind === "dist"}>
      <Row label="Direction"><Choice v={f("dist").dir} set={v => put({ ...f("dist"), dir: v as any })} opts={sp() ? [["y", "Vertical, Z (gravity)"], ["x", "Global X"], ["z", "Global Y, towards −Y"], ["local", "Along the member's local y"]] : [["y", "Global y (gravity)"], ["x", "Global x (wind)"], ["local", "Across the member"]]} /></Row>
      <NumField label="At the start" q="dist" v={f("dist").w1} set={w1 => put({ ...f("dist"), w1 })} />
      <NumField label="At the end" q="dist" v={f("dist").w2} set={w2 => put({ ...f("dist"), w2 })} hint="Equal values make a uniform load. Up (or +x) is positive." />
    </Show>
    <Show when={k().kind === "pressure"}><NumField label="Pressure" q="stress" v={f("pressure").p} set={pp => put({ kind: "pressure", p: pp })} hint="Positive pushes into the part." /></Show>
    <Show when={k().kind === "traction"}>
      <label class="chk"><input type="checkbox" checked={f("traction").total} onChange={e => put({ ...f("traction"), total: e.currentTarget.checked })} />The whole force (not per length)</label>
      <Show when={p.m.analysis === "solid"} fallback={<>
        <NumField label="Fx" q={f("traction").total ? "force" : "dist"} v={f("traction").fx} set={fx => put({ ...f("traction"), fx })} />
        <NumField label="Fy" q={f("traction").total ? "force" : "dist"} v={f("traction").fy} set={fy => put({ ...f("traction"), fy })} /></>}>
        <Vec3 pre="F" q={f("traction").total ? "force" : "stress"} v={[f("traction").fx, f("traction").fy, f("traction").fz ?? 0]} set={v => put({ ...f("traction"), fx: v[0], fy: v[1], fz: v[2] })} hint={f("traction").total ? "The whole force on the face; Z is up." : "Per area of the face; Z is up."} />
      </Show>
    </Show>
    <Show when={k().kind === "bearing"}>
      <NumField label="Fx" q="force" v={f("bearing").fx} set={fx => put({ ...f("bearing"), fx })} />
      <NumField label="Fy" q="force" v={f("bearing").fy} set={fy => put({ ...f("bearing"), fy })} hint="The pin's whole force on the part. It presses on the side of the hole it pushes into, most in line with the force." />
    </Show>
    <Show when={k().kind === "gravity" && zUp()}><Vec3 pre="g" q="accel" v={[f("gravity").gx, f("gravity").gy, f("gravity").gz ?? 0]} set={v => put({ ...f("gravity"), gx: v[0], gy: v[1], gz: v[2] })} hint="Self-weight from each material's density; down is −Z." /></Show>
    <Show when={k().kind === "gravity" && !zUp()}><NumField label="gx" q="accel" v={f("gravity").gx} set={gx => put({ ...f("gravity"), gx })} /><NumField label="gy" q="accel" v={f("gravity").gy} set={gy => put({ ...f("gravity"), gy })} hint="Self-weight from each material's density." /><Show when={sp()}><NumField label="gz" q="accel" v={f("gravity").gz ?? 0} set={gz => put({ ...f("gravity"), gz })} /></Show></Show>
    <Show when={k().kind === "flux"}><NumField label="Heat flux" q="flux" v={f("flux").q} set={q => put({ kind: "flux", q })} hint="Positive flows into the part." /></Show>
    <Show when={k().kind === "convection"}><NumField label="Film coefficient h" q="film" v={f("convection").h} min={0} set={h => put({ ...f("convection"), h })} /><NumField label="Fluid temperature" q="temp" v={f("convection").t_inf} set={t_inf => put({ ...f("convection"), t_inf })} /></Show>
    <Show when={k().kind === "warming"}><NumField label="Change" q="dtemp" v={f("warming").dt} set={dt => put({ kind: "warming", dt })} hint={isFrame(p.m.analysis) ? "Every member grows by αΔT per length; where it cannot, it pushes on its supports." : "A uniform change from the stress-free temperature (Inspector, with nothing selected); added to any temperatures set on edges."} /></Show>
    <Show when={k().kind === "heat"}><NumField label="Heat generated" q="heatgen" v={f("heat").q} set={q => put({ kind: "heat", q })} /></Show>
    <Show when={k().kind === "settlement"}>
      <Show when={p.m.analysis === "space"} fallback={<>
        <NumField label="Moves x" q="len" v={f("settlement").dx} set={dx => put({ ...f("settlement"), dx })} />
        <NumField label="Moves y" q="len" v={f("settlement").dy} set={dy => put({ ...f("settlement"), dy })} hint="Down is negative." /></>}>
        <Vec3 pre="Moves " q="len" v={[f("settlement").dx, f("settlement").dy, f("settlement").dz ?? 0]} set={v => put({ ...f("settlement"), dx: v[0], dy: v[1], dz: v[2] })} hint="Down is −Z." />
      </Show>
      <p class="hint">The support moves by this much in the directions it holds; the frame follows, and the forces it takes to make it are in the results.</p></Show>
    <Show when={k().kind === "mass"}><NumField label="Mass" q="mass" v={f("mass").m} min={0} set={m => m >= 0 && put({ kind: "mass", m })} hint="Moves with the node in every direction, for natural frequencies and dynamics. It is in every load case and adds no weight to a static solve (use loads for that)." /></Show>
  </>;
};

const ModelInsp = (p: { m: Model }) => {
  const settings = (s: object) => act({ SetSettings: { s: { analysis: p.m.analysis, units: p.m.units, grid: p.m.grid, thickness: p.m.thickness, mesh: p.m.mesh, ...s } } });
  return <>
    <Row label="Name"><input value={p.m.name} onChange={e => act({ SetMeta: { name: e.currentTarget.value, notes: p.m.notes } })} /></Row>
    <Row label="Analysis"><Choice v={p.m.analysis} set={v => settings({ analysis: v })} opts={ANALYSES} /></Row>
    <Row label="Units"><Choice v={p.m.units} set={v => settings({ units: v })} opts={[["SI", "SI (mm, kN, MPa)"], ["US", "US (in, kip, ksi)"]]} /></Row>
    <NumField label="Grid step" q="coord" v={p.m.grid} min={0} set={grid => grid > 0 && settings({ grid })} />
    <Show when={p.m.analysis === "space"}><label class="chk" title="At each height above the lowest, the nodes move together in the floor's plane: x and z together, turning about y together (a concrete floor that does not stretch in its plane)"><input type="checkbox" checked={!!p.m.rigid_floors} onChange={e => settings({ rigid_floors: e.currentTarget.checked })} />Rigid floors</label></Show>
    <Show when={p.m.volume}>{v => <Row label="Material of the mesh"><Choice v={String(v().material)} set={x => settings({ material: +x })} opts={p.m.materials.map(x => [String(x.id), x.name])} /></Row>}</Show>
    <Show when={isSolidA(p.m.analysis) && !p.m.volume}><Row label="Solid"><Choice v={p.m.revolve ? "revolve" : "extrude"} set={v => settings({ revolve: v === "revolve" ? Math.PI * 2 : 0 })} opts={[["extrude", "Extruded along z"], ["revolve", "Revolved about the y axis"]]} /></Row>
      <Show when={p.m.revolve}><div class="iprow"><label>Turned through<span class="unit">°</span></label><input class="mono" value={num((p.m.revolve ?? 0) * 180 / Math.PI, 6)} onChange={numIn(v => v > 0 && v <= 360 && settings({ revolve: v * Math.PI / 180 }), () => num((p.m.revolve ?? 0) * 180 / Math.PI, 6))} /><span class="hint">360° for the whole part; less for a sector, with symmetry on its two cut faces (the back and front faces). The section is drawn right of the y axis (x is the radius).</span></div></Show></Show>
    <Show when={!p.m.volume && (p.m.analysis === "stress" || p.m.analysis === "heat" || (isSolidA(p.m.analysis) && !p.m.revolve))}><NumField label={isSolidA(p.m.analysis) ? "Thickness (extruded along z)" : "Thickness"} q="len" v={p.m.thickness} min={0} set={thickness => thickness > 0 && settings({ thickness })} /></Show>
    <Show when={p.m.analysis !== "heat"}><NumField label="Stress-free temperature" q="temp" v={p.m.t_ref ?? 293.15} set={t_ref => settings({ t_ref })} hint="Thermal stress comes from the change from this temperature." /></Show>
    <Show when={!isFrame(p.m.analysis) && !p.m.volume}>
      <h4>Mesh<Help k="mesh" /></h4>
      <NumField label="Element size" q="len" v={p.m.mesh.size} min={0} set={size => settings({ mesh: { ...p.m.mesh, size } })} hint="0 = automatic (a twenty-fifth of the part, finer for thin parts)." />
      <Row label="Elements"><Choice v={String(p.m.mesh.order)} set={v => settings({ mesh: { ...p.m.mesh, order: +v } })} opts={[["2", "Quadratic, 6-node (accurate)"], ["1", "Linear, 3-node"]]} /></Row>
      <Show when={isSolidA(p.m.analysis)}><div class="iprow"><label>Layers {p.m.revolve ? "round the turn" : "through the thickness"}</label><input class="mono" value={p.m.mesh.layers || "auto"} onChange={numIn(v => v >= 0 && v <= 128 && Number.isInteger(v) && settings({ mesh: { ...p.m.mesh, layers: v } }), () => p.m.mesh.layers || "auto")} /><span class="hint">0 = automatic (about one element size each{p.m.revolve ? ", at most 22.5° each" : ""}). The 2D mesh is swept {p.m.revolve ? "round the turn" : "through the thickness"} in layers of wedges.</span></div></Show>
      <div class="iprow"><label>Smallest angle<span class="unit">°</span></label><input class="mono" value={p.m.mesh.min_angle} onChange={numIn(v => v >= 5 && v <= 33 && settings({ mesh: { ...p.m.mesh, min_angle: v } }), () => p.m.mesh.min_angle)} /><span class="hint">5° to 33°; 25° is a good balance.</span></div>
    </Show>
    <Row label="Notes"><textarea rows="4" value={p.m.notes} onChange={e => act({ SetMeta: { name: p.m.name, notes: e.currentTarget.value } })} /></Row>
    <table class="vmini"><tbody>
      <Show when={isFrame(p.m.analysis)} fallback={p.m.volume ? <tr><td>Imported mesh</td><td class="r">{p.m.volume.elements.toLocaleString()} tetrahedra, {p.m.volume.faces.length} faces</td></tr> : <tr><td>Shapes</td><td class="r">{p.m.shapes.filter(s => !s.hole).length} + {p.m.shapes.filter(s => s.hole).length} holes</td></tr>}>
        <tr><td>Nodes</td><td class="r">{p.m.nodes.length}</td></tr><tr><td>Members</td><td class="r">{p.m.members.length}</td></tr></Show>
      <tr><td>Supports</td><td class="r">{p.m.supports.length}</td></tr><tr><td>Loads</td><td class="r">{p.m.loads.length}</td></tr>
    </tbody></table>
  </>;
};

export const Inspector = () => {
  const m = () => model();
  const one = createMemo(() => {
    const mm = m(), s = sel(); if (!mm || s.length !== 1) return null;
    const id = s[0];
    const f = <T extends { id: number }>(xs: T[]) => xs.find(x => x.id === id);
    return { node: f(mm.nodes), member: f(mm.members), shape: f(mm.shapes), support: f(mm.supports), load: f(mm.loads), part: f(mm.parts ?? []) };
  });
  const title = () => {
    const mm = m(); if (!mm) return "";
    if (sel().length > 1) return `${sel().length} items`;
    if (sel().length === 1) return itemName(mm, sel()[0]).split(" · ")[0];
    if (edgeSel()) return `Edge ${edgeSel()!.edge + 1}`;
    return "Model";
  };
  return (
    <aside class="insp">
      <header><b>{title()}</b><span class="grow" /><Help k="inspector" /></header>
      <div class="ibody">
        <Show when={m()}>{mm => <>
          <Show when={sel().length > 1}><p class="muted">Drag any of them to move them together, or press Delete.</p><p class="ibtns"><button class="btn sm" onClick={C.del}>Delete</button><button class="btn sm" onClick={() => setSel([])}>Clear selection</button></p></Show>
          <Show when={one()?.node}>{n => <NodeInsp n={n()} m={mm()} />}</Show>
          <Show when={one()?.member}>{b => <MemberInsp b={b()} m={mm()} />}</Show>
          <Show when={one()?.shape}>{s => <ShapeInsp s={s()} m={mm()} />}</Show>
          <Show when={one()?.support}>{s => <SupportInsp s={s()} m={mm()} />}</Show>
          <Show when={one()?.load}>{l => <LoadInsp l={l()} m={mm()} />}</Show>
          <Show when={one()?.part}>{p => <PartInsp p={p()} m={mm()} />}</Show>
          <Show when={!sel().length && edgeSel()}>{e => <EdgeInsp m={mm()} e={e()} />}</Show>
          <Show when={!sel().length && !edgeSel()}><ModelInsp m={mm()} /></Show>
          <Show when={sel().length === 1}><p class="ibtns"><button class="btn sm" onClick={C.del}>Delete</button></p></Show>
        </>}</Show>
      </div>
      <Show when={edgeSel() && sel().length}><button class="btn sm" onClick={() => setEdgeSel(null)}>Clear edge</button></Show>
    </aside>
  );
};
