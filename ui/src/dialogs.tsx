// Dialogs: materials, sections, a node at typed coordinates, and the help pages (shortcuts, getting
// started, glossary), recovery and closing.
import { For, Show, createSignal, createResource } from "solid-js";
import { Out, Material, Section, SectionShape, Props, Check, LibMaterial, num } from "./api";
import { openDialog, closeDialog, act, call, out, setOut, model, sel, setSel, toast } from "./store";
import { C } from "./cmds";
import { numIn } from "./panels";
import { Dialog, Icon } from "./ui";
import { Q, toU, fromU, unit, fmt } from "./units";

/** A number field inside a dialog: display units in, SI out. */
export const F = (p: { l: string; q: Q; v: number; set: (v: number) => void }) => (
  <><label>{p.l}<span class="unit"> {unit(p.q)}</span></label>
    <input class="mono" value={num(toU(p.q, p.v), 6).replace("−", "-")} onChange={numIn(v => p.set(fromU(p.q, v)), () => num(toU(p.q, p.v), 6).replace("−", "-"))} /></>
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
          <p class="ibtns"><button class="btn sm pri" onClick={() => library(id => setCur(id))}>From the library…</button></p>
          <p class="ibtns"><button class="btn sm" onClick={add}>Add a copy</button><button class="btn sm" onClick={del} disabled={list().length < 2}>Delete</button></p>
          <p class="muted small">The library: metals and plastics from Datum Atlas, each with its source.</p></div>
        <Show when={m()}>{mm => <div class="form">
          <label>Name</label><input value={mm().name} onChange={e => put({ name: e.currentTarget.value })} />
          <F l="Young's modulus E" q="modulus" v={mm().e} set={e => put({ e })} />
          <label>Poisson's ratio ν</label><input class="mono" value={mm().nu} onChange={numIn(v => put({ nu: v }), () => mm().nu)} />
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

/** Home › Materials › Library: metals and plastics from Datum Atlas, with sources. Adds the chosen
 * one to the model (and calls `added` with its id). */
export const library = (added?: (id: number) => void) => openDialog(() => {
  const [lib] = createResource(() => call<LibMaterial[]>("materials_library"));
  const [q, setQ] = createSignal(""), [cur, setCur] = createSignal<string>("al-6061-t6");
  const shown = () => (lib() ?? []).filter(x => `${x.name} ${x.family}`.toLowerCase().includes(q().toLowerCase()));
  const families = () => [...new Set(shown().map(x => x.family))];
  const m = () => (lib() ?? []).find(x => x.id === cur());
  const use = async (forShapes: boolean) => {
    const x = m(), mm = model(); if (!x || !mm) return;
    const have = mm.materials.find(y => y.name === x.name);
    let id = have?.id;
    if (!have) {
      const o = await act({ Add: { item: { Material: { id: 0, name: x.name, e: x.e, nu: x.nu, rho: x.rho, fy: x.fy, k: x.k, alpha: x.alpha, source: x.source } } } });
      id = o?.created[0];
    }
    if (id === undefined) return;
    if (forShapes) {
      const m2 = model()!, ops = m2.shapes.filter(s => !s.hole).map(s => ({ Set: { item: { Shape: { ...s, material: id } } } })).concat(m2.members.map(b => ({ Set: { item: { Member: { ...b, material: id } } } })) as any);
      if (ops.length) await act({ Batch: ops });
    }
    closeDialog();
    added?.(id);
  };
  return (
    <Dialog title="Materials library" wide help="materials">
      <div class="mgr lib">
        <div class="mlist"><input placeholder="Search: steel, 6061, nylon…" value={q()} onInput={e => setQ(e.currentTarget.value)} />
          <Show when={lib()} fallback={<p class="muted">Loading…</p>}>
            <For each={families()}>{f => <><small class="fam">{f}</small><For each={shown().filter(x => x.family === f)}>{x => <button classList={{ on: x.id === cur() }} onClick={() => setCur(x.id)} onDblClick={() => use(false)}>{x.name}</button>}</For></>}</For>
          </Show></div>
        <Show when={m()}>{x => <div class="form">
          <h3>{x().name}</h3><p class="muted small">{x().family}</p>
          <table class="vmini"><tbody>
            <tr><td>Young's modulus E</td><td class="r">{fmt("modulus", x().e)}</td></tr><tr><td>Poisson's ratio ν</td><td class="r">{num(x().nu)}</td></tr>
            <tr><td>Density</td><td class="r">{fmt("density", x().rho)}</td></tr><tr><td>Yield strength</td><td class="r">{x().fy ? fmt("stress", x().fy) : "— (brittle or not given)"}</td></tr>
            <tr><td>Conductivity</td><td class="r">{fmt("cond", x().k)}</td></tr><tr><td>Thermal expansion</td><td class="r">{fmt("expansion", x().alpha)}</td></tr></tbody></table>
          <p class="small"><b>Source:</b> {x().source}</p>
          <p class="muted small">Typical values for the grade: E, density, conductivity and expansion are the middle of the usual range; the yield strength is its low end. Check them against your supplier's data before relying on a result.</p>
          <p class="ibtns"><button class="btn pri" onClick={() => use(false)}>Add to the model</button><button class="btn" onClick={() => use(true)}>Add and use everywhere</button></p>
        </div>}</Show>
      </div>
    </Dialog>
  );
});

const SHAPES: [SectionShape["kind"], string, [string, string][]][] = [
  ["rect", "Rectangle", [["b", "Width b"], ["h", "Depth h"]]], ["circle", "Round bar", [["d", "Diameter d"]]], ["tube", "Tube", [["d", "Outside diameter d"], ["t", "Wall t"]]],
  ["box", "Box (RHS)", [["b", "Width b"], ["h", "Depth h"], ["t", "Wall t"]]], ["i", "I-section", [["h", "Depth h"], ["b", "Flange width b"], ["tw", "Web tw"], ["tf", "Flange tf"]]],
  ["channel", "Channel", [["h", "Depth h"], ["b", "Flange width b"], ["tw", "Web tw"], ["tf", "Flange tf"]]], ["angle", "Angle", [["h", "Leg in the plane h"], ["b", "Other leg b"], ["t", "Thickness t"]]],
  ["custom", "Your own values", [["a", "Area A"], ["i", "Second moment I"], ["c", "Extreme fibre c"], ["iy", "3D: other axis Iy"], ["j", "3D: torsion J"]]],
];
const sq = (k: string, kind: string): Q => (kind === "custom" ? (k === "a" ? "area" : k === "i" || k === "iy" || k === "j" ? "inertia" : "len") : "len");

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
          <For each={SHAPES.find(x => x[0] === ss().shape.kind)![2]}>{([k, l]) => <F l={l} q={sq(k, ss().shape.kind)} v={(ss().shape as any)[k] ?? 0} set={v => put({ shape: { ...ss().shape, [k]: v } as SectionShape })} />}</For>
          <label>Properties</label><div><Show when={props()} fallback={<span class="muted">—</span>}>{p => <span class="mono">A = {fmt("area", p().a)} · I = {fmt("inertia", p().i)} · c = {fmt("len", p().c)}<Show when={model()?.analysis === "space"}><br />Iy = {fmt("inertia", p().iy)} · J = {fmt("inertia", p().j)}</Show></span>}</Show></div>
          <span /><p class="muted small">Bending is in the plane of the frame, about the section's axis across h. Angles bend about the axis parallel to b.{model()?.analysis === "space" ? " In 3D, Iy bends across the width b and J resists twisting (open sections, such as I and channel, twist easily). Your own values: Iy and J of 0 are taken as for a round bar." : ""}</p>
        </div>}</Show>
      </div>
    </Dialog>
  );
});

/** Geometry › Node at…: a node at typed coordinates. */
export const nodeAt = () => openDialog(() => {
  const [x, setX] = createSignal(0), [y, setY] = createSignal(0), [z, setZ] = createSignal(0);
  const sp = () => model()?.analysis === "space";
  return <Dialog title="Add a node" okText="Add" ok={async () => { const o = await act({ Add: { item: { Node: { id: 0, x: fromU("coord", x()), y: fromU("coord", y()), z: sp() ? fromU("coord", z()) : 0 } } } }); if (!o) return false; setSel(o.created); }}>
    <div class="form"><label>x ({unit("coord")})</label><input class="mono" value={x()} onChange={numIn(setX, x)} /><label>y ({unit("coord")})</label><input class="mono" value={y()} onChange={numIn(setY, y)} />
      <Show when={sp()}><label>z ({unit("coord")})</label><input class="mono" value={z()} onChange={numIn(setZ, z)} /></Show></div>
  </Dialog>;
});

/** Geometry › Repeat (3D frames): copy members along a step and join the copies to the originals. */
export const repeat = () => openDialog(() => {
  const m = model()!, chosen = () => { const s = new Set(sel()); const ids = m.members.filter(b => s.has(b.id)).map(b => b.id); return ids.length ? ids : m.members.map(b => b.id); };
  const step0 = Math.max(m.grid * 8, 1);
  const [d, setD] = createSignal<[number, number, number]>([0, 0, step0]), [n, setN] = createSignal(1), [link, setLink] = createSignal<string>("beam");
  const axis = (k: number, l: string) => <><label>{l} ({unit("coord")})</label><input class="mono" value={num(toU("coord", d()[k]), 6).replace("−", "-")} onChange={numIn(v => setD(d().map((q, i) => i === k ? fromU("coord", v) : q) as [number, number, number]), () => num(toU("coord", d()[k]), 6).replace("−", "-"))} /></>;
  return <Dialog title="Repeat" okText="Repeat" help="space" ok={async () => {
    const o = await act({ Repeat: { ids: chosen(), step: d(), times: n(), link: link() === "none" ? null : link() } });
    if (!o) return false;
    toast(`Made ${o.created.length} items: ${n()} ${n() === 1 ? "copy" : "copies"} of ${chosen().length} members${link() === "none" ? "" : ", joined"}.`, true);
  }}>
    <p class="muted small">{sel().length ? `${chosen().length} selected members` : `The whole frame (${chosen().length} members); select members first to repeat only those`}, copied with their supports and loads, each step further on.</p>
    <div class="form">{axis(0, "Step along x")}{axis(1, "Step along y")}{axis(2, "Step along z")}
      <label>Copies</label><input class="mono" value={n()} onChange={numIn(v => v >= 1 && v <= 100 && Number.isInteger(v) ? setN(v) : false, n)} />
      <label>Join each copy</label><select value={link()} onChange={e => setLink(e.currentTarget.value)}><option value="beam" selected={link() === "beam"}>with beams</option><option value="truss" selected={link() === "truss"}>with truss bars</option><option value="none" selected={link() === "none"}>not at all</option></select></div>
  </Dialog>;
});

/** Before the open model is replaced: save it, drop the changes, or stay. */
export const keepChanges = (then: () => unknown) => {
  if (!out()?.dirty) { then(); return; }
  openDialog(() => (
    <Dialog title="Save changes?" okText="Save" cancel="Cancel" ok={async () => { await C.save(); if (out()?.dirty) return false; setTimeout(then); }}
      extra={<button class="btn" onClick={() => { closeDialog(); setTimeout(then); }}>Don't save</button>}>
      <p>“{out()?.m.name}” has changes that are not saved.</p>
    </Dialog>
  ));
};

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
      ["Solve", "F5"], ["Move the selection one grid step", "Arrows"], ["Fit to window", "F"], ["Zoom", "Wheel"], ["Pan", "Middle button, or Space + drag; Shift + wheel"], ["This list", "F1"],
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
        <li><b>Solve</b> (F5) shows the deflected shape, the moment, shear and axial diagrams and the reactions; hover a member to read the values there.</li>
      </ol>
      <h4>A part</h4>
      <ol>
        <li>File › Home › <b>Blank part</b>. Draw the outline with Rectangle, Circle or Polygon; cut holes with the hole tools.</li>
        <li>Click an edge, then put a support or a load on it from the Loads tab.</li>
        <li><b>Solve</b> (F5) meshes the part and colours it by von Mises stress; pick another result under the canvas, and hover to read values. The Mesh view shows the triangles and their quality.</li>
        <li>For heat, choose Home › <b>Heat conduction</b>: fix a temperature on an edge, add convection or a heat flux, and solve for temperatures.</li>
      </ol>
      <h4>Good to know</h4>
      <ul>
        <li>Values are shown in mm, kN and MPa (or in, kip and ksi: Home › Units). The file keeps SI units, so nothing is lost when you switch.</li>
        <li>Every tool has a small ? beside it that says what it is for. File › Home has worked examples.</li>
        <li>Every result Datum Mesh gives is checked against known answers; Help › Verification solves them in front of you.</li>
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
    ["Von Mises stress", "One number for the stress at a point that compares with the yield strength from a tensile test: the part starts to yield where it reaches the yield strength."],
    ["Principal stresses", "The largest and smallest normal stresses at a point, on the planes where the shear is zero (σ1 and σ2)."],
    ["Safety factor", "The yield strength over the von Mises stress: below 1 the material yields there."],
    ["Plane stress", "A thin plate loaded in its own plane: no stress across its thickness."], ["Plane strain", "A long body with the same section all along, held from stretching along its length: no strain along it."],
    ["Convection", "Heat carried away by a fluid: h (T − T∞) per area, with h the film coefficient."], ["Heat flux", "Heat flowing through an area, W/m²."],
    ["Smoothed and per element", "Stresses jump between elements; smoothed results average them at the nodes. Where the two differ a lot, the mesh is too coarse there."],
    ["Patch test", "A check that elements reproduce a constant stress exactly; elements that fail it do not converge."],
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


/** Help › Verification: every benchmark solved now by this version, against its reference. */
export const verification = () => openDialog(() => {
  const [rows] = createResource(() => call<Check[]>("verification"));
  const [ver] = createResource(() => call<string>("version"));
  const val = (q: string, v: number) => (q === "ratio" ? num(v, 6) : q === "rad" ? `${num(v, 6)} rad` : fmt(q as Q, v, 6));
  return (
    <Dialog title="Verification" wide help="verification">
      <div class="verif">
        <p>Every check below was solved just now by Datum Mesh {ver()} and compared with a known answer. A check fails if its error is over the tolerance; the same checks run as tests before every release.</p>
        <Show when={rows()} fallback={<p class="muted">Solving the benchmarks…</p>}>{r => <>
          <p classList={{ okline: r().every(x => x.pass), badline: !r().every(x => x.pass) }}><Icon n={r().every(x => x.pass) ? "check" : "info"} /> {r().filter(x => x.pass).length} of {r().length} checks pass.</p>
          <table class="tbl"><thead><tr><th>#</th><th>Benchmark</th><th>Quantity</th><th class="r">Datum Mesh</th><th class="r">Reference</th><th class="r">Error</th><th>Source</th><th /></tr></thead><tbody>
            <For each={r()}>{x => <tr><td>{x.bench}</td><td>{x.name}</td><td>{x.quantity}</td><td class="r">{val(x.q, x.result)}</td><td class="r">{val(x.q, x.reference)}</td><td class="r">{x.error < 1e-12 ? "< 1e-12" : x.error.toExponential(1)}</td><td>{x.source}</td><td>{x.pass ? "✓ pass" : "✗ fail"}</td></tr>}</For></tbody></table>
          <h3>Mesh convergence</h3>
          <p class="muted small">The same model on finer meshes: the error falls as the elements get smaller.</p>
          <div class="conv"><For each={r().filter(x => x.steps.length)}>{x => <table class="tbl"><thead><tr><th colSpan={2}>{x.name}: {x.quantity}</th></tr><tr><th class="r">Elements</th><th class="r">Error</th></tr></thead><tbody>
            <For each={x.steps}>{st => <tr><td class="r">{st[0].toLocaleString()}</td><td class="r">{st[1] < 1e-12 ? "< 1e-12" : st[1].toExponential(1)}</td></tr>}</For></tbody></table>}</For></div>
        </>}</Show>
      </div>
    </Dialog>
  );
});
