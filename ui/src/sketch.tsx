// Sketch tools (0.3.1): ready-made shapes (slot, rounded rectangle, regular polygon), rounding and
// bevelling corners, combining two shapes, offsetting an outline and patterns of copies. The engine
// does the geometry (crates/core/src/sketch.rs); these dialogs only collect the numbers.
import { For, Show, createSignal } from "solid-js";
import { Geom, Shape, Out, num } from "./api";
import { openDialog, act, call, model, sel, setSel, setOut, toast } from "./store";
import { numIn } from "./panels";
import { Dialog } from "./ui";
import { F } from "./dialogs";

/** The selected shapes, in the order they were picked. */
export const pickedShapes = (): Shape[] => { const m = model(); if (!m) return []; return sel().map(id => m.shapes.find(s => s.id === id)).filter((s): s is Shape => !!s); };

/** A sensible size and place for a new shape: the grid, near the middle of what is drawn. */
const near = () => {
  const m = model()!, g = m.grid > 0 ? m.grid : 0.01;
  const pts = m.shapes.flatMap(s => s.geom.kind === "circle" ? [s.geom.c] : s.geom.pts);
  const c = pts.length ? [pts.reduce((a, p) => a + p[0], 0) / pts.length, pts.reduce((a, p) => a + p[1], 0) / pts.length] : [0, 0];
  return { g, c: [Math.round(c[0] / g) * g, Math.round(c[1] / g) * g] as [number, number] };
};

const KINDS = { slot: "Slot", rounded: "Rounded rectangle", polygon: "Regular polygon" } as const;
export type MakeKind = keyof typeof KINDS;

/** Geometry › Shapes: a slot, a rounded rectangle or a regular polygon from typed sizes. */
export const makeShape = (kind: MakeKind, asHole = false) => openDialog(() => {
  const { g, c } = near();
  const [a, setA] = createSignal<[number, number]>(kind === "polygon" ? c : [c[0] - 4 * g, c[1] - (kind === "slot" ? 0 : 2 * g)]);
  const [b, setB] = createSignal<[number, number]>([c[0] + 4 * g, c[1] + (kind === "slot" ? 0 : 2 * g)]);
  const [w, setW] = createSignal(kind === "slot" ? 2 * g : kind === "rounded" ? g : 3 * g);
  const [n, setN] = createSignal(6), [turn, setTurn] = createSignal(90), [hole, setHole] = createSignal(asHole);
  const xy = (l: string, v: () => [number, number], set: (p: [number, number]) => void) => <>
    <F l={`${l} x`} q="coord" v={v()[0]} set={x => set([x, v()[1]])} /><F l={`${l} y`} q="coord" v={v()[1]} set={y => set([v()[0], y])} /></>;
  const make = () => kind === "slot" ? { kind, a: a(), b: b(), w: w() } : kind === "rounded" ? { kind, a: a(), b: b(), r: w() } : { kind, c: a(), r: w(), n: n(), turn: turn() * Math.PI / 180 };
  return <Dialog title={KINDS[kind]} okText="Add" help="sketch" ok={async () => {
    const o = await act({ Make: { make: make(), hole: hole(), material: null } });
    if (!o) return false;
    setSel(o.created);
  }}>
    <div class="form">
      <Show when={kind === "slot"}>{xy("First end centre", a, setA)}{xy("Second end centre", b, setB)}<F l="Width" q="coord" v={w()} set={setW} /></Show>
      <Show when={kind === "rounded"}>{xy("Corner", a, setA)}{xy("Opposite corner", b, setB)}<F l="Corner radius" q="coord" v={w()} set={setW} /></Show>
      <Show when={kind === "polygon"}>{xy("Centre", a, setA)}<F l="Centre to each corner" q="coord" v={w()} set={setW} />
        <label>Sides</label><input class="mono" value={n()} onChange={numIn(v => v >= 3 && v <= 1000 && Number.isInteger(v) ? setN(v) : false, n)} />
        <label>First corner at (°)</label><input class="mono" value={turn()} onChange={numIn(setTurn, turn)} /></Show>
      <label /><label class="chk"><input type="checkbox" checked={hole()} onChange={e => setHole(e.currentTarget.checked)} />A hole (cut out of the part)</label>
    </div>
  </Dialog>;
});

/** Corners of a polygon that lie between two straight edges (the ones that can be rounded). */
export const sharpCorners = (g: Geom) => g.kind === "poly" ? g.pts.map((_, i) => i).filter(i => g.bulge[i] === 0 && g.bulge[(i + g.pts.length - 1) % g.pts.length] === 0) : [];

/** Round (fillet) or bevel (chamfer) corners of the selected polygon. */
export const corners = (round: boolean, only?: number) => {
  const s = pickedShapes()[0];
  if (!s || s.geom.kind !== "poly") return toast("Select a polygon or rectangle first: its corners are rounded or bevelled.");
  const can = sharpCorners(s.geom);
  if (!can.length) return toast("This shape has no corner between two straight edges.");
  openDialog(() => {
    const [size, setSize] = createSignal(Math.max(model()!.grid, 1e-4));
    const [on, setOn] = createSignal(new Set(only !== undefined && can.includes(only) ? [only] : can));
    const flip = (i: number) => { const v = new Set(on()); if (v.has(i)) v.delete(i); else v.add(i); setOn(v); };
    return <Dialog title={round ? "Round corners" : "Bevel corners"} okText={round ? "Round" : "Bevel"} help="sketch" ok={async () => {
      const cs = [...on()];
      if (!cs.length) { toast("Tick the corners to change."); return false; }
      const o = await act(round ? { Fillet: { shape: s.id, corners: cs, r: size() } } : { Chamfer: { shape: s.id, corners: cs, d: size() } });
      if (!o) return false;
    }}>
      <div class="form"><F l={round ? "Radius" : "Distance along each edge"} q="coord" v={size()} set={setSize} /></div>
      <p class="muted small">Corners of {s.name} between two straight edges (numbered as in the Inspector):</p>
      <div class="cks"><For each={can}>{i => <label class="chk"><input type="checkbox" checked={on().has(i)} onChange={() => flip(i)} />{i + 1}</label>}</For></div>
      <p class="muted small">Supports and loads stay on their edges; the new {round ? "arcs" : "bevels"} start bare.</p>
    </Dialog>;
  });
};

const HOW = { union: ["Union", "joined into one"], subtract: ["Subtract", "cut out of"], intersect: ["Intersect", "kept where they overlap"] } as const;

/** Union, subtract or intersect two selected shapes (the first picked keeps its name). */
export const combine = async (how: keyof typeof HOW) => {
  const ss = pickedShapes();
  if (ss.length !== 2) return toast(how === "subtract" ? "Select two shapes: the one to cut, then (Shift+click) the one to cut out of it." : "Select two shapes (click one, Shift+click the other).");
  const o = await act({ Combine: { a: ss[0].id, b: ss[1].id, how } });
  if (!o) return;
  setSel([ss[0].id, ...o.created]);
  toast(how === "subtract" ? `${ss[1].name} cut out of ${ss[0].name}.` : `${ss[0].name} and ${ss[1].name} ${HOW[how][1]}.`, true);
};

/** A copy of the selected shape's outline, moved out or in. */
export const offset = () => {
  const s = pickedShapes()[0];
  if (!s) return toast("Select a shape first.");
  openDialog(() => {
    const [d, setD] = createSignal(model()!.grid || 0.001), [inward, setIn] = createSignal(true), [hole, setHole] = createSignal(!s.hole);
    return <Dialog title="Offset outline" okText="Offset" help="sketch" ok={async () => {
      const o = await act({ Offset: { shape: s.id, d: inward() ? -d() : d() } });
      if (!o) return false;
      const m = model()!, made = m.shapes.find(x => x.id === o.created[0]);
      if (made && hole() !== made.hole) await act({ Set: { item: { Shape: { ...made, hole: hole() } } } });
      setSel(o.created);
    }}>
      <p class="muted small">A new outline parallel to {s.name}'s, each edge moved by the distance: corners stay sharp, arcs keep their centres. Inwards and made a hole, it leaves a wall of that thickness.</p>
      <div class="form"><F l="Distance" q="coord" v={d()} set={v => v > 0 && setD(v)} />
        <label>Direction</label><select value={inward() ? "in" : "out"} onChange={e => setIn(e.currentTarget.value === "in")}><option value="in">Inwards</option><option value="out">Outwards</option></select>
        <label /><label class="chk"><input type="checkbox" checked={hole()} onChange={e => setHole(e.currentTarget.checked)} />Make the new outline a hole</label></div>
    </Dialog>;
  });
};

/** Copies of the selected shapes in a row (`circular` false) or around a centre. */
export const pattern = (circular: boolean) => {
  const ss = pickedShapes();
  if (!ss.length) return toast("Select the shapes to copy (holes, usually).");
  openDialog(() => {
    const g = model()!.grid || 0.01;
    const [n, setN] = createSignal(circular ? 5 : 3), [step, setStep] = createSignal<[number, number]>([4 * g, 0]);
    const [ang, setAng] = createSignal<number | null>(null), [c, setC] = createSignal<[number, number]>([0, 0]);
    const each = () => ang() ?? 360 / (n() + 1);
    return <Dialog title={circular ? "Circular pattern" : "Linear pattern"} okText="Copy" help="sketch" ok={async () => {
      const o = await act({ Pattern: { ids: ss.map(s => s.id), times: n(), step: circular ? [0, 0] : step(), turn: circular ? each() * Math.PI / 180 : 0, about: c() } });
      if (!o) return false;
      toast(`Made ${n()} ${n() === 1 ? "copy" : "copies"} of ${ss.length === 1 ? ss[0].name : `${ss.length} shapes`}.`, true);
    }}>
      <p class="muted small">{ss.length === 1 ? ss[0].name : `${ss.length} shapes`}, copied with the supports and loads on their edges.</p>
      <div class="form">
        <label>Copies (besides the original)</label><input class="mono" value={n()} onChange={numIn(v => v >= 1 && v <= 1000 && Number.isInteger(v) ? setN(v) : false, n)} />
        <Show when={circular} fallback={<><F l="Step along x" q="coord" v={step()[0]} set={x => setStep([x, step()[1]])} /><F l="Step along y" q="coord" v={step()[1]} set={y => setStep([step()[0], y])} /></>}>
          <label>Angle between copies (°)</label><input class="mono" value={num(each(), 6)} onChange={numIn(setAng, () => num(each(), 6))} />
          <F l="Centre x" q="coord" v={c()[0]} set={x => setC([x, c()[1]])} /><F l="Centre y" q="coord" v={c()[1]} set={y => setC([c()[0], y])} />
        </Show>
      </div>
      <Show when={circular}><p class="muted small">Counter-clockwise; with the angle left as it is, the copies and the original go evenly round a full circle.</p></Show>
    </Dialog>;
  });
};

/** Geometry › DXF (0.3.5): bring in a section drawn in CAD. The engine reads the file and adds its
 * outlines as shapes and holes, in one step to undo. */
export const importDxf = () => openDialog(() => {
  const [u, setU] = createSignal("file");
  const SCALE: Record<string, number | null> = { file: null, mm: 0.001, cm: 0.01, m: 1, in: 0.0254 };
  return <Dialog title="Import a DXF drawing" okText="Choose the file…" help="import" ok={async () => {
    const o = await call<Out | null>("import_dxf", { scale: SCALE[u()] });
    if (!o) return false;
    const notes = o.warnings;
    setOut({ ...o, warnings: [] });
    setSel(o.created);
    toast(notes.join(" "), notes.length === 1);
    import("./canvas").then(c => setTimeout(c.fit, 60));
  }}>
    <p class="muted small">Lines, arcs, circles and polylines (with their arcs) that close into outlines become shapes; outlines inside others become holes. Arcs stay arcs. Save the drawing from CAD as an ASCII DXF of the section alone.</p>
    <div class="form"><label>Drawing units</label><select value={u()} onChange={e => setU(e.currentTarget.value)}>
      <option value="file">As the drawing says (millimetres if it does not)</option><option value="mm">Millimetres</option><option value="cm">Centimetres</option><option value="m">Metres</option><option value="in">Inches</option></select></div>
  </Dialog>;
});

/** File › Export › Section as DXF (0.3.5). */
export const exportDxf = async () => {
  const m = model(); if (!m?.shapes.length) return toast("There is no section to export: draw the part first.");
  const text = await call<string>("export_dxf"); if (!text) return;
  const p = await call<string | null>("save_file", { name: `${m.name || "section"}.dxf`, data: text });
  if (p) toast(`Saved the section as ${p}.`, true);
};
