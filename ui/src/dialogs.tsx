// Dialogs: materials, sections, a node at typed coordinates, and the help pages (shortcuts, getting
// started, glossary), recovery and closing.
import { For, Show, createSignal, createResource } from "solid-js";
import { Out, Material, Section, SectionShape, Props, num } from "./api";
import { openDialog, closeDialog, act, call, out, setOut, model, setSel } from "./store";
import { C } from "./cmds";
import { Dialog } from "./ui";
import { Q, toU, fromU, unit, fmt } from "./units";

/** A number field inside a dialog: display units in, SI out. */
const F = (p: { l: string; q: Q; v: number; set: (v: number) => void }) => (
  <><label>{p.l}<span class="unit"> {unit(p.q)}</span></label>
    <input class="mono" value={num(toU(p.q, p.v), 6).replace("−", "-")} onChange={e => { const v = +e.currentTarget.value.replace(",", "."); if (isFinite(v)) p.set(fromU(p.q, v)); }} /></>
);

/** Geometry › Materials: the model's materials and their values. */
export const materials = (focus?: number) => openDialog(() => {
  const list = () => model()?.materials ?? [];
  const [cur, setCur] = createSignal<number>(focus ?? list()[0]?.id ?? 0);
  const m = () => list().find(x => x.id === cur());
  const put = (x: Partial<Material>) => { const mm = m(); if (mm) act({ Set: { item: { Material: { ...mm, ...x } } } }); };
  const add = async () => { const base = m() ?? list()[0]; const o = await act({ Add: { item: { Material: { ...base, id: 0, name: `${base.name} (copy)`, source: "Your own values" } } } }); if (o?.created.length) setCur(o.created[0]); };
  const del = async () => { const o = await act({ Delete: { ids: [cur()] } }); if (o) setCur(list()[0]?.id ?? 0); };
  return (
    <Dialog title="Materials" wide help="materials">
      <div class="mgr">
        <div class="mlist"><For each={list()}>{x => <button classList={{ on: x.id === cur() }} onClick={() => setCur(x.id)}>{x.name}</button>}</For>
          <p class="ibtns"><button class="btn sm" onClick={add}>Add a copy</button><button class="btn sm" onClick={del} disabled={list().length < 2}>Delete</button></p>
          <p class="muted small">The materials library from Datum Atlas, with sources, arrives in 0.1.3.</p></div>
        <Show when={m()}>{mm => <div class="form">
          <label>Name</label><input value={mm().name} onChange={e => put({ name: e.currentTarget.value })} />
          <F l="Young's modulus E" q="modulus" v={mm().e} set={e => put({ e })} />
          <label>Poisson's ratio ν</label><input class="mono" value={mm().nu} onChange={e => { const v = +e.currentTarget.value; if (isFinite(v)) put({ nu: v }); }} />
          <F l="Density" q="density" v={mm().rho} set={rho => put({ rho })} />
          <F l="Yield strength" q="stress" v={mm().fy} set={fy => put({ fy })} />
          <F l="Conductivity" q="cond" v={mm().k} set={k => put({ k })} />
          <F l="Thermal expansion" q="expansion" v={mm().alpha} set={alpha => put({ alpha })} />
          <label>Source</label><input value={mm().source} onChange={e => put({ source: e.currentTarget.value })} />
        </div>}</Show>
      </div>
    </Dialog>
  );
});

const SHAPES: [SectionShape["kind"], string, [string, string][]][] = [
  ["rect", "Rectangle", [["b", "Width b"], ["h", "Depth h"]]], ["circle", "Round bar", [["d", "Diameter d"]]], ["tube", "Tube", [["d", "Outside diameter d"], ["t", "Wall t"]]],
  ["box", "Box (RHS)", [["b", "Width b"], ["h", "Depth h"], ["t", "Wall t"]]], ["i", "I-section", [["h", "Depth h"], ["b", "Flange width b"], ["tw", "Web tw"], ["tf", "Flange tf"]]],
  ["channel", "Channel", [["h", "Depth h"], ["b", "Flange width b"], ["tw", "Web tw"], ["tf", "Flange tf"]]], ["angle", "Angle", [["h", "Leg in the plane h"], ["b", "Other leg b"], ["t", "Thickness t"]]],
  ["custom", "Your own values", [["a", "Area A"], ["i", "Second moment I"], ["c", "Extreme fibre c"]]],
];
const sq = (k: string, kind: string): Q => (kind === "custom" ? (k === "a" ? "area" : k === "i" ? "inertia" : "len") : "len");

/** Geometry › Sections: the frame sections and their properties. */
export const sections = (focus?: number) => openDialog(() => {
  const list = () => model()?.sections ?? [];
  const [cur, setCur] = createSignal<number>(focus ?? list()[0]?.id ?? 0);
  const s = () => list().find(x => x.id === cur());
  const [props] = createResource(() => s()?.shape, sh => call<Props>("section_props", { shape: sh }));
  const put = (x: Partial<Section>) => { const ss = s(); if (ss) act({ Set: { item: { Section: { ...ss, ...x } } } }); };
  const kind = (k: SectionShape["kind"]) => {
    const d: Record<string, SectionShape> = { rect: { kind: "rect", b: 0.1, h: 0.2 }, circle: { kind: "circle", d: 0.05 }, tube: { kind: "tube", d: 0.1143, t: 0.006 }, box: { kind: "box", b: 0.1, h: 0.2, t: 0.008 }, i: { kind: "i", h: 0.3, b: 0.15, tw: 0.0071, tf: 0.0107 }, channel: { kind: "channel", h: 0.2, b: 0.075, tw: 0.0085, tf: 0.0115 }, angle: { kind: "angle", h: 0.1, b: 0.1, t: 0.01 }, custom: { kind: "custom", a: 0.005, i: 5e-5, c: 0.15 } };
    put({ shape: d[k], name: SHAPES.find(x => x[0] === k)![1] });
  };
  const add = async () => { const base = s() ?? list()[0]; const o = await act({ Add: { item: { Section: base ? { ...base, id: 0, name: `${base.name} (copy)` } : { id: 0, name: "Rectangle 100 × 200", shape: { kind: "rect", b: 0.1, h: 0.2 } } } } }); if (o?.created.length) setCur(o.created[0]); };
  const del = async () => { const o = await act({ Delete: { ids: [cur()] } }); if (o) setCur(list()[0]?.id ?? 0); };
  return (
    <Dialog title="Sections" wide help="sections">
      <div class="mgr">
        <div class="mlist"><For each={list()}>{x => <button classList={{ on: x.id === cur() }} onClick={() => setCur(x.id)}>{x.name}</button>}</For>
          <p class="ibtns"><button class="btn sm" onClick={add}>Add a copy</button><button class="btn sm" onClick={del} disabled={!list().length}>Delete</button></p></div>
        <Show when={s()}>{ss => <div class="form">
          <label>Name</label><input value={ss().name} onChange={e => put({ name: e.currentTarget.value })} />
          <label>Shape</label><select value={ss().shape.kind} onChange={e => kind(e.currentTarget.value as any)}><For each={SHAPES}>{([k, l]) => <option value={k} selected={k === ss().shape.kind}>{l}</option>}</For></select>
          <For each={SHAPES.find(x => x[0] === ss().shape.kind)![2]}>{([k, l]) => <F l={l} q={sq(k, ss().shape.kind)} v={(ss().shape as any)[k]} set={v => put({ shape: { ...ss().shape, [k]: v } as SectionShape })} />}</For>
          <label>Properties</label><div><Show when={props()} fallback={<span class="muted">—</span>}>{p => <span class="mono">A = {fmt("area", p().a)} · I = {fmt("inertia", p().i)} · c = {fmt("len", p().c)}</span>}</Show></div>
          <span /><p class="muted small">Bending is in the plane of the frame, about the section's axis across h. Angles bend about the axis parallel to b.</p>
        </div>}</Show>
      </div>
    </Dialog>
  );
});

/** Geometry › Node at…: a node at typed coordinates. */
export const nodeAt = () => openDialog(() => {
  const [x, setX] = createSignal(0), [y, setY] = createSignal(0);
  return <Dialog title="Add a node" okText="Add" ok={async () => { const o = await act({ Add: { item: { Node: { id: 0, x: fromU("coord", x()), y: fromU("coord", y()) } } } }); if (!o) return false; setSel(o.created); }}>
    <div class="form"><label>x ({unit("coord")})</label><input class="mono" value={x()} onInput={e => setX(+e.currentTarget.value)} /><label>y ({unit("coord")})</label><input class="mono" value={y()} onInput={e => setY(+e.currentTarget.value)} /></div>
  </Dialog>;
});

export const confirmClose = (quit: () => void) => openDialog(() => (
  <Dialog title="Save changes?" okText="Save" cancel="Cancel" ok={async () => { await C.save(); if (!out()?.dirty) quit(); }}
    extra={<button class="btn" onClick={() => { closeDialog(); quit(); }}>Don't save</button>}>
    <p>“{out()?.m.name}” has changes that are not saved.</p>
  </Dialog>
));

export const shortcuts = () => openDialog(() => (
  <Dialog title="Keyboard shortcuts">
    <table class="tbl keys"><tbody><For each={[
      ["Save / Save As", "Ctrl+S / Ctrl+Shift+S"], ["Open", "Ctrl+O"], ["New", "Ctrl+N"], ["Print", "Ctrl+P"], ["Tell me what you want to do", "Ctrl+K"],
      ["Undo / Redo", "Ctrl+Z / Ctrl+Y"], ["Delete", "Del"], ["Select all", "Ctrl+A"], ["Select tool, stop drawing", "Esc"], ["Close a polygon", "Enter or double-click"],
      ["Move the selection one grid step", "Arrows"], ["Fit to window", "F"], ["Zoom", "Wheel"], ["Pan", "Middle button, or Space + drag; Shift + wheel"], ["This list", "F1"],
    ] as [string, string][]}>{([a, b]) => <tr><td>{a}</td><td><kbd>{b}</kbd></td></tr>}</For></tbody></table>
  </Dialog>
));

export const gettingStarted = () => openDialog(() => (
  <Dialog title="Getting started" wide>
    <div class="helpcard">
      <p>Datum Mesh finds the stresses, deflections and temperatures in parts and structures by the finite-element method: it splits the model into small pieces, solves the equations of each, and puts them back together.</p>
      <h4>A frame in a minute</h4>
      <ol>
        <li>File › Home › <b>Blank frame</b>. Geometry › <b>Beam</b>, then click two points on the grid for each member (it continues from the last point; Esc stops).</li>
        <li>Loads › <b>Fixed</b> or <b>Pinned</b>, then click the nodes at the bottom.</li>
        <li>Loads › <b>Force</b> or <b>Distributed</b>, then click a node or a member; set the value in the Inspector on the right.</li>
        <li>Solve (0.1.1) shows the deflected shape, the moment and shear diagrams and the reactions.</li>
      </ol>
      <h4>A part</h4>
      <ol>
        <li>File › Home › <b>Blank part</b>. Draw the outline with Rectangle, Circle or Polygon; cut holes with the hole tools.</li>
        <li>Click an edge, then put a support or a load on it from the Loads tab.</li>
        <li>Meshing and solving parts arrive in 0.1.2: stress contours, the deformed shape and a probe.</li>
      </ol>
      <h4>Good to know</h4>
      <ul>
        <li>Values are shown in mm, kN and MPa (or in, kip and ksi: Home › Units). The file keeps SI units, so nothing is lost when you switch.</li>
        <li>Every tool has a small ? beside it that says what it is for. File › Home has worked examples.</li>
        <li>Every result Datum Mesh gives is checked against known answers; Help › Verification (0.1.3) lists them.</li>
      </ul>
    </div>
  </Dialog>
));

export const glossary = () => openDialog(() => {
  const [q, setQ] = createSignal("");
  const G: [string, string][] = [
    ["Finite element", "A small piece of the model (a beam segment or a triangle) whose behaviour is known exactly; the model is many of them joined at nodes."],
    ["Node", "A point where elements join. Results are found at the nodes."], ["Member", "A beam or truss bar between two nodes."],
    ["Beam", "A member that carries axial force, shear and bending (Euler–Bernoulli theory)."], ["Truss bar", "A member pinned at both ends that carries only axial force."],
    ["Mesh", "The triangles a part is split into. Smaller triangles give more accurate results and take longer."],
    ["Plane stress", "A thin plate loaded in its own plane: the stress through the thickness is zero."],
    ["Plane strain", "A long body (a dam, a pipe, a tunnel) with the same section along its length: the strain along the length is zero."],
    ["Support", "Where the model is held: fixed, pinned, on rollers, on springs, or (for heat) at a fixed temperature."],
    ["Reaction", "The force a support pushes back with. Reactions balance the loads: the equilibrium check."],
    ["Von Mises stress", "One number for the stress at a point, compared with the yield strength to see if a ductile metal will yield."],
    ["Principal stresses", "The largest and smallest normal stresses at a point, in the directions where there is no shear."],
    ["Safety factor", "Yield strength divided by the von Mises stress. Below 1 the material yields."],
    ["Stress concentration", "A local rise in stress at a hole, notch or sharp corner. A fillet lowers it."],
    ["Convection", "Heat carried away by a moving fluid: q = h (T − T∞), with h the film coefficient."],
    ["Bulge", "How an edge between two corners curves into an arc; the Inspector shows it as the arc's angle."],
  ];
  return <Dialog title="Glossary"><input placeholder="Search" value={q()} onInput={e => setQ(e.currentTarget.value)} style={{ width: "100%", "margin-bottom": "8px" }} />
    <dl class="gloss"><For each={G.filter(g => (g[0] + g[1]).toLowerCase().includes(q().toLowerCase()))}>{([t, d]) => <><dt>{t}</dt><dd>{d}</dd></>}</For></dl></Dialog>;
});

/** Recover unsaved work after a crash. */
export const recoverDialog = (name: string, saved: string, done: () => void) => openDialog(() => (
  <Dialog title="Recover unsaved work?" okText="Recover" cancel="Not now" ok={async () => { const r = await call<Out>("recover", { keep: true }); if (r) { setOut(r); done(); } }}
    extra={<button class="btn" onClick={() => { call("recover", { keep: false }); closeDialog(); }}>Discard</button>}>
    <p>Datum Mesh closed before “{name}” was saved. A copy from {saved} was kept.</p>
  </Dialog>
));

