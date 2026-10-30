// The model tree (left) and the Inspector (right): every item of the model, and the values of the
// selected one with their units. Edits go to the engine as Set operations.
import { For, Show, JSX, createMemo } from "solid-js";
import { Model, Node, Member, Shape, Support, Load, SupportKind, LoadKind, Target, Geom, num, parseNum, isFrame } from "./api";
import { model, sel, setSel, edgeSel, setEdgeSel, act, setTool, results, stale, setView, meshed, partRes, solidRes } from "./store";
import { Q, toU, fromU, unit, fmt } from "./units";
import { Help } from "./help";
import { edgeCount, edgePts, outline, area } from "./geom";
import { C } from "./cmds";
import * as D from "./dialogs";
import { sharpCorners, corners } from "./sketch";

export const ANALYSES: [string, string][] = [["frame", "Frame or truss"], ["space", "3D frame or truss"], ["stress", "Plane stress"], ["strain", "Plane strain"], ["axi", "Axisymmetric (turned about y)"], ["heat", "Heat conduction"], ["solid", "3D solid (extruded)"]];

/** A new support of a kind, with typical values. */
export const defaultSupport = (k: string): SupportKind => {
  if (k === "spring") return { kind: "spring", kx: 1e7, ky: 1e7, kr: 0 };
  if (k === "temperature") return { kind: "temperature", t: 293.15 };
  return { kind: k } as SupportKind;
};
/** A new load of a kind, with typical values (frames in kN, parts smaller). */
export const defaultLoad = (k: string, frame: boolean): LoadKind => {
  switch (k) {
    case "moment": return { kind: "moment", m: 10e3 };
    case "dist": return { kind: "dist", w1: -5e3, w2: -5e3, dir: "y" };
    case "pressure": return { kind: "pressure", p: 1e6 };
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

export const SUPPORT_NAMES: Record<string, string> = { fixed: "Fixed", pinned: "Pinned", rollerx: "Roller (moves along x)", rollery: "Roller (moves along y)", roller: "Roller (slides along the edge)", symmetry: "Symmetry", spring: "Spring", temperature: "Temperature" };
export const LOAD_NAMES: Record<string, string> = { force: "Force", moment: "Moment", dist: "Distributed load", pressure: "Pressure", traction: "Edge force", bearing: "Pin load", gravity: "Gravity (self-weight)", flux: "Heat flux", convection: "Convection", heat: "Heat generation", warming: "Temperature change" };

const targetName = (m: Model, t: Target) => {
  if (t.on === "node") return `Node ${m.nodes.findIndex(n => n.id === t.id) + 1}`;
  if (t.on === "member") return `Member ${m.members.findIndex(n => n.id === t.id) + 1}`;
  if (t.on === "edge") return `${m.shapes.find(s => s.id === t.shape)?.name ?? "Shape"}, edge ${t.edge + 1}`;
  if (t.on === "point") return `Point ${fmt("coord", t.x)}, ${fmt("coord", t.y)}`;
  if (t.on === "end") return t.front ? "Front face" : "Back face";
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
const Group = (p: { t: string; n: number; children: JSX.Element; help?: string; add?: () => void }) => (
  <details class="tgrp" open><summary><span>{p.t}</span><span class="pill">{p.n}</span>{p.help && <Help k={p.help} />}{p.add && <button class="tadd" title={`Add: ${p.t}`} onClick={e => { e.preventDefault(); p.add!(); }}>+</button>}</summary><div class="tlist">{p.children}</div></details>
);
const Leaf = (p: { id: number; label: string; sub?: string }) => (
  <button class="ti" classList={{ on: sel().includes(p.id) }} onClick={e => setSel(e.ctrlKey || e.shiftKey ? [...new Set([...sel(), p.id])] : [p.id])}><span>{p.label}</span>{p.sub && <small>{p.sub}</small>}</button>
);

export const Tree = () => {
  const m = () => model();
  const frame = () => isFrame(m()?.analysis);
  const sp = () => m()?.analysis === "space";
  return (
    <aside class="tree">
      <header>Model<Help k="tree" /></header>
      <div class="tbody">
        <Show when={m()}>{mm => <>
          <Show when={frame()} fallback={
            <Group t="Shapes" n={mm().shapes.length} help="shapes" add={() => setTool("rect")}><For each={mm().shapes}>{s => <Leaf id={s.id} label={s.name} sub={s.hole ? "hole" : s.geom.kind === "circle" ? "circle" : `${s.geom.pts.length} corners`} />}</For></Group>}>
            <Group t="Members" n={mm().members.length} help="members" add={() => setTool("beam")}><For each={mm().members}>{(b, i) => <Leaf id={b.id} label={`Member ${i() + 1}`} sub={`${b.kind} · ${mm().sections.find(s => s.id === b.section)?.name ?? ""}`} />}</For></Group>
            <Group t="Nodes" n={mm().nodes.length} help="nodes" add={() => setTool("node")}><For each={mm().nodes}>{(n, i) => <Leaf id={n.id} label={`Node ${i() + 1}`} sub={`${num(toU("coord", n.x))}, ${num(toU("coord", n.y))}${sp() ? `, ${num(toU("coord", n.z ?? 0))}` : ""}`} />}</For></Group>
          </Show>
          <Group t="Materials" n={mm().materials.length} help="materials" add={() => D.materials()}><For each={mm().materials}>{x => <button class="ti" onClick={() => D.materials(x.id)}><span>{x.name}</span></button>}</For></Group>
          <Show when={frame()}><Group t="Sections" n={mm().sections.length} help="sections" add={() => D.sections()}><For each={mm().sections}>{x => <button class="ti" onClick={() => D.sections(x.id)}><span>{x.name}</span></button>}</For></Group></Show>
          <Group t="Supports" n={mm().supports.length} help="supports"><For each={mm().supports}>{s => <Leaf id={s.id} label={SUPPORT_NAMES[s.kind.kind]} sub={targetName(mm(), s.at)} />}</For></Group>
          <Group t="Loads" n={mm().loads.length} help="loads"><For each={mm().loads}>{l => <Leaf id={l.id} label={LOAD_NAMES[l.kind.kind]} sub={targetName(mm(), l.at)} />}</For></Group>
          <Show when={!frame()}><Group t="Mesh" n={mm().mesh.refine.length} help="mesh">
            <button class="ti" onClick={() => C.mesh()}><span>{solidRes() ? `${solidRes()!.elements.toLocaleString()} wedges, ${solidRes()!.layers} ${solidRes()!.layers === 1 ? "layer" : "layers"}` : (meshed() ?? partRes()?.mesh) ? `${(meshed() ?? partRes()!.mesh).tris.length.toLocaleString()} ${mm().analysis === "solid" ? "triangles in the section" : "elements"}` : "Not meshed yet"}</span><small>{mm().mesh.size > 0 ? fmt("len", mm().mesh.size) : "automatic size"} · {mm().mesh.order === 2 ? "6-node" : "3-node"}</small></button>
            <For each={mm().mesh.refine}>{(r, i) => <div class="ti refrow"><span>Finer near {targetName(mm(), r.at)}</span>
              <input class="mono" title={`Element size there (${unit("len")})`} value={num(toU("len", r.size), 4)} onChange={numIn(v => v > 0 && C.meshSettings({ refine: mm().mesh.refine.map((x, k) => k === i() ? { ...x, size: fromU("len", v) } : x) }), () => num(toU("len", r.size), 4))} />
              <button class="x" title="Remove" onClick={() => C.meshSettings({ refine: mm().mesh.refine.filter((_, k) => k !== i()) })}>×</button></div>}</For>
          </Group></Show>
          <Group t="Results" n={results() ? 1 : 0} help="results">
            <Show when={results()} fallback={<p class="muted tnote">Not solved yet: press F5.</p>}><button class="ti" onClick={() => setView("results")}><span>{stale() ? "Out of date" : "Solved"}</span><small>{results()!.stats.equations.toLocaleString()} equations</small></button></Show></Group>
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

const NodeInsp = (p: { n: Node; m: Model }) => (
  <>
    <NumField label="x" q="coord" v={p.n.x} set={x => set("Node", { ...p.n, x })} />
    <NumField label="y" q="coord" v={p.n.y} set={y => set("Node", { ...p.n, y })} />
    <Show when={p.m.analysis === "space"}><NumField label="z" q="coord" v={p.n.z ?? 0} set={z => set("Node", { ...p.n, z })} /></Show>
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

const ShapeInsp = (p: { s: Shape; m: Model }) => {
  const g = () => p.s.geom;
  const put = (geom: Geom) => set("Shape", { ...p.s, geom });
  return (
    <>
      <Row label="Name"><input value={p.s.name} onChange={e => set("Shape", { ...p.s, name: e.currentTarget.value })} /></Row>
      <label class="chk"><input type="checkbox" checked={p.s.hole} onChange={e => set("Shape", { ...p.s, hole: e.currentTarget.checked })} />A hole (cut out of the part)</label>
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
  const kinds = (): string[] => p.s.at.on === "end" ? ["fixed", "roller", "symmetry"] : p.s.at.on === "edge" ? (p.m.analysis === "heat" ? ["temperature"] : ["fixed", "pinned", "roller", "symmetry", "temperature"]) : p.m.analysis === "heat" ? ["temperature"] : isFrame(p.m.analysis) ? ["fixed", "pinned", "rollerx", "rollery", "spring"] : ["fixed", "pinned", "rollerx", "rollery"];
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
    <p class="muted small">{({ fixed: "Holds the point still; beams cannot turn there either.", pinned: "Holds the point still but lets it turn.", rollerx: "Free to slide along x; held in y.", rollery: "Free to slide along y; held in x.", roller: "Free to slide along the edge; held across it.", symmetry: "A plane of symmetry: model half the part and hold the cut edge across.", spring: p.m.analysis === "space" ? "Springs to the ground along x, y and z and turning about each." : "Springs to the ground in x, y and rotation.", temperature: "Holds the edge at this temperature." } as Record<string, string>)[k().kind]}</p>
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
    <Show when={k().kind === "force"}><NumField label="Fx" q="force" v={f("force").fx} set={fx => put({ ...f("force"), fx })} /><NumField label="Fy" q="force" v={f("force").fy} set={fy => put({ ...f("force"), fy })} hint={sp() ? undefined : "Up is positive."} /><Show when={sp()}><NumField label="Fz" q="force" v={f("force").fz ?? 0} set={fz => put({ ...f("force"), fz })} hint="Up (y) is positive; z points towards the front." /></Show></Show>
    <Show when={k().kind === "moment"}><Show when={sp()} fallback={<NumField label="M" q="moment" v={f("moment").m} set={m => put({ kind: "moment", m })} hint="Counter-clockwise is positive." />}>
      <NumField label="Mx" q="moment" v={f("moment").mx ?? 0} set={mx => put({ ...f("moment"), mx })} /><NumField label="My" q="moment" v={f("moment").my ?? 0} set={my => put({ ...f("moment"), my })} /><NumField label="Mz" q="moment" v={f("moment").m} set={m => put({ ...f("moment"), m })} hint="About the global axes, by the right-hand rule." /></Show></Show>
    <Show when={k().kind === "dist"}>
      <Row label="Direction"><Choice v={f("dist").dir} set={v => put({ ...f("dist"), dir: v as any })} opts={sp() ? [["y", "Global y (gravity)"], ["x", "Global x"], ["z", "Global z"], ["local", "Along the member's local y"]] : [["y", "Global y (gravity)"], ["x", "Global x (wind)"], ["local", "Across the member"]]} /></Row>
      <NumField label="At the start" q="dist" v={f("dist").w1} set={w1 => put({ ...f("dist"), w1 })} />
      <NumField label="At the end" q="dist" v={f("dist").w2} set={w2 => put({ ...f("dist"), w2 })} hint="Equal values make a uniform load. Up (or +x) is positive." />
    </Show>
    <Show when={k().kind === "pressure"}><NumField label="Pressure" q="stress" v={f("pressure").p} set={pp => put({ kind: "pressure", p: pp })} hint="Positive pushes into the part." /></Show>
    <Show when={k().kind === "traction"}>
      <label class="chk"><input type="checkbox" checked={f("traction").total} onChange={e => put({ ...f("traction"), total: e.currentTarget.checked })} />The whole force (not per length)</label>
      <NumField label="Fx" q={f("traction").total ? "force" : "dist"} v={f("traction").fx} set={fx => put({ ...f("traction"), fx })} />
      <NumField label="Fy" q={f("traction").total ? "force" : "dist"} v={f("traction").fy} set={fy => put({ ...f("traction"), fy })} />
      <Show when={p.m.analysis === "solid"}><NumField label="Fz" q={f("traction").total ? "force" : "stress"} v={f("traction").fz ?? 0} set={fz => put({ ...f("traction"), fz })} hint={f("traction").total ? "The whole force on the face." : "Per area of the face."} /></Show>
    </Show>
    <Show when={k().kind === "bearing"}>
      <NumField label="Fx" q="force" v={f("bearing").fx} set={fx => put({ ...f("bearing"), fx })} />
      <NumField label="Fy" q="force" v={f("bearing").fy} set={fy => put({ ...f("bearing"), fy })} hint="The pin's whole force on the part. It presses on the side of the hole it pushes into, most in line with the force." />
    </Show>
    <Show when={k().kind === "gravity"}><NumField label="gx" q="accel" v={f("gravity").gx} set={gx => put({ ...f("gravity"), gx })} /><NumField label="gy" q="accel" v={f("gravity").gy} set={gy => put({ ...f("gravity"), gy })} hint="Self-weight from each material's density." /><Show when={sp()}><NumField label="gz" q="accel" v={f("gravity").gz ?? 0} set={gz => put({ ...f("gravity"), gz })} /></Show></Show>
    <Show when={k().kind === "flux"}><NumField label="Heat flux" q="flux" v={f("flux").q} set={q => put({ kind: "flux", q })} hint="Positive flows into the part." /></Show>
    <Show when={k().kind === "convection"}><NumField label="Film coefficient h" q="film" v={f("convection").h} min={0} set={h => put({ ...f("convection"), h })} /><NumField label="Fluid temperature" q="temp" v={f("convection").t_inf} set={t_inf => put({ ...f("convection"), t_inf })} /></Show>
    <Show when={k().kind === "warming"}><NumField label="Change" q="dtemp" v={f("warming").dt} set={dt => put({ kind: "warming", dt })} hint={isFrame(p.m.analysis) ? "Every member grows by αΔT per length; where it cannot, it pushes on its supports." : "A uniform change from the stress-free temperature (Inspector, with nothing selected); added to any temperatures set on edges."} /></Show>
    <Show when={k().kind === "heat"}><NumField label="Heat generated" q="heatgen" v={f("heat").q} set={q => put({ kind: "heat", q })} /></Show>
  </>;
};

const ModelInsp = (p: { m: Model }) => {
  const settings = (s: object) => act({ SetSettings: { s: { analysis: p.m.analysis, units: p.m.units, grid: p.m.grid, thickness: p.m.thickness, mesh: p.m.mesh, ...s } } });
  return <>
    <Row label="Name"><input value={p.m.name} onChange={e => act({ SetMeta: { name: e.currentTarget.value, notes: p.m.notes } })} /></Row>
    <Row label="Analysis"><Choice v={p.m.analysis} set={v => settings({ analysis: v })} opts={ANALYSES} /></Row>
    <Row label="Units"><Choice v={p.m.units} set={v => settings({ units: v })} opts={[["SI", "SI (mm, kN, MPa)"], ["US", "US (in, kip, ksi)"]]} /></Row>
    <NumField label="Grid step" q="coord" v={p.m.grid} min={0} set={grid => grid > 0 && settings({ grid })} />
    <Show when={p.m.analysis === "stress" || p.m.analysis === "heat" || p.m.analysis === "solid"}><NumField label={p.m.analysis === "solid" ? "Thickness (extruded along z)" : "Thickness"} q="len" v={p.m.thickness} min={0} set={thickness => thickness > 0 && settings({ thickness })} /></Show>
    <Show when={p.m.analysis !== "heat"}><NumField label="Stress-free temperature" q="temp" v={p.m.t_ref ?? 293.15} set={t_ref => settings({ t_ref })} hint="Thermal stress comes from the change from this temperature." /></Show>
    <Show when={!isFrame(p.m.analysis)}>
      <h4>Mesh<Help k="mesh" /></h4>
      <NumField label="Element size" q="len" v={p.m.mesh.size} min={0} set={size => settings({ mesh: { ...p.m.mesh, size } })} hint="0 = automatic (a twenty-fifth of the part, finer for thin parts)." />
      <Row label="Elements"><Choice v={String(p.m.mesh.order)} set={v => settings({ mesh: { ...p.m.mesh, order: +v } })} opts={[["2", "Quadratic, 6-node (accurate)"], ["1", "Linear, 3-node"]]} /></Row>
      <Show when={p.m.analysis === "solid"}><div class="iprow"><label>Layers through the thickness</label><input class="mono" value={p.m.mesh.layers || "auto"} onChange={numIn(v => v >= 0 && v <= 40 && Number.isInteger(v) && settings({ mesh: { ...p.m.mesh, layers: v } }), () => p.m.mesh.layers || "auto")} /><span class="hint">0 = automatic (about one element size each). The 2D mesh is swept through the thickness in layers of wedges.</span></div></Show>
      <div class="iprow"><label>Smallest angle<span class="unit">°</span></label><input class="mono" value={p.m.mesh.min_angle} onChange={numIn(v => v >= 5 && v <= 33 && settings({ mesh: { ...p.m.mesh, min_angle: v } }), () => p.m.mesh.min_angle)} /><span class="hint">5° to 33°; 25° is a good balance.</span></div>
    </Show>
    <Row label="Notes"><textarea rows="4" value={p.m.notes} onChange={e => act({ SetMeta: { name: p.m.name, notes: e.currentTarget.value } })} /></Row>
    <table class="vmini"><tbody>
      <Show when={isFrame(p.m.analysis)} fallback={<tr><td>Shapes</td><td class="r">{p.m.shapes.filter(s => !s.hole).length} + {p.m.shapes.filter(s => s.hole).length} holes</td></tr>}>
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
    return { node: f(mm.nodes), member: f(mm.members), shape: f(mm.shapes), support: f(mm.supports), load: f(mm.loads) };
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
          <Show when={!sel().length && edgeSel()}>{e => <EdgeInsp m={mm()} e={e()} />}</Show>
          <Show when={!sel().length && !edgeSel()}><ModelInsp m={mm()} /></Show>
          <Show when={sel().length === 1}><p class="ibtns"><button class="btn sm" onClick={C.del}>Delete</button></p></Show>
        </>}</Show>
      </div>
      <Show when={edgeSel() && sel().length}><button class="btn sm" onClick={() => setEdgeSel(null)}>Clear edge</button></Show>
    </aside>
  );
};
