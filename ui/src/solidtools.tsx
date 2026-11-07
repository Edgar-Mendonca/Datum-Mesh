// More modelling (0.7.6): Sweep and Loft from finished sketches, Split a solid by a plane,
// Measure what is clicked (one face or edge, or the distance and angle between two), the mass of
// the solids, and STEP in and out (STL out). Each is one command of the geometry engine.
import { For, Show, createSignal } from "solid-js";
import { openDialog, toast, model, setOut } from "./store";
import { cad, setCad, cadRun, cadQuery, pick3, turnBack, setTurnBack, CadHit, CadScene, setMeshOld } from "./cad";
import { invoke, Out } from "./api";
import { Dialog } from "./ui";

const sketches = () => (cad()?.sketches ?? []).filter(s => s.curves.some(c => !c.construction));
const frameIt = () => setTurnBack(turnBack() + 1);

/** Sweep: a closed profile carried along a path (the lines and arcs of another sketch, in order). */
export const sweepDialog = () => {
  const sk = sketches();
  if (sk.length < 2) return toast("Sweep needs two finished sketches: a closed profile, and the path to carry it along (on another plane).");
  openDialog(() => {
    const profiles = sk.filter(s => s.profiles > 0);
    const [prof, setProf] = createSignal(profiles[0]?.id ?? sk[0].id), [path, setPath] = createSignal(sk.find(s => s.id !== prof())?.id ?? sk[0].id);
    const [twist, setTwist] = createSignal(0), [scale, setScale] = createSignal(1);
    const ok = async () => {
      const ps = sk.find(s => s.id === path());
      if (!ps || path() === prof()) { toast("Pick a path sketch other than the profile's."); return false; }
      const ids = ps.curves.filter(c => !c.construction).map(c => c.id);
      if (!(await cadRun("solid.sweep", { sketch: prof(), path_sketch: path(), path: ids, ...(twist() ? { twist: twist() } : {}), ...(scale() !== 1 ? { scale: scale() } : {}) }))) return false;
      frameIt();
    };
    return <Dialog title="Sweep" okText="Sweep" ok={ok} help="modelling">
      <p class="muted small">The profile (a closed shape) is carried along the path, square to it as it turns. Draw the profile on a plane across the path's start.</p>
      <table class="kv"><tbody>
        <tr><td>Profile</td><td><select value={prof()} onChange={e => setProf(+e.currentTarget.value)}><For each={sk}>{s => <option value={s.id}>{s.name}{s.profiles ? "" : " (not closed)"}</option>}</For></select></td></tr>
        <tr><td>Path</td><td><select value={path()} onChange={e => setPath(+e.currentTarget.value)}><For each={sk}>{s => <option value={s.id}>{s.name}</option>}</For></select></td></tr>
        <tr><td>Twist</td><td><input class="mono" type="number" value={twist()} onInput={e => setTwist(+e.currentTarget.value)} /></td><td class="muted">° over its length</td></tr>
        <tr><td>Scale at the end</td><td><input class="mono" type="number" step="0.1" value={scale()} onInput={e => setScale(+e.currentTarget.value)} /></td><td class="muted">1: the same size</td></tr>
      </tbody></table>
    </Dialog>;
  });
};

/** Loft: a solid through closed sections on different planes, in order. */
export const loftDialog = () => {
  const sk = sketches().filter(s => s.profiles > 0);
  if (sk.length < 2) return toast("Loft needs two closed sketches or more, on different planes (an Offset Plane for each).");
  openDialog(() => {
    const [on, setOn] = createSignal(sk.map(s => s.id));
    const ok = async () => {
      const ids = sk.map(s => s.id).filter(i => on().includes(i));
      if (ids.length < 2) { toast("Pick two sections or more."); return false; }
      if (!(await cadRun("solid.loft", { sections: ids.map(i => ({ sketch: i })) }))) return false;
      frameIt();
    };
    return <Dialog title="Loft" okText="Loft" ok={ok} help="modelling">
      <p class="muted small">A solid that passes through each section in turn (in the order they were drawn), changing smoothly from one to the next.</p>
      <For each={sk}>{s => <label class="chk"><input type="checkbox" checked={on().includes(s.id)} onChange={e => setOn(e.currentTarget.checked ? [...on(), s.id] : on().filter(x => x !== s.id))} />{s.name}</label>}</For>
    </Dialog>;
  });
};

/** Split: a solid cut in two by a plane square to X, Y or Z (each piece a solid of its own). */
export const splitDialog = () => {
  const names = (cad()?.bodies ?? []).map(b => b.name);
  if (!names.length) return toast("There is no solid to split yet.");
  openDialog(() => {
    const [body, setBody] = createSignal(names[0]), [ax, setAx] = createSignal(0), [at, setAt] = createSignal(0);
    const ok = async () => {
      const n = [0, 0, 0], o = [0, 0, 0]; n[ax()] = 1; o[ax()] = at();
      return (await cadRun("solid.split_body", { body: body(), plane: { origin: o, normal: n } })) ? undefined : false;
    };
    return <Dialog title="Split a solid" okText="Split" ok={ok} help="modelling">
      <p class="muted small">Each piece becomes a solid of its own. Pieces that touch are meshed joined, sharing their nodes, so the part still carries load across the cut.</p>
      <table class="kv"><tbody>
        <tr><td>Solid</td><td><select value={body()} onChange={e => setBody(e.currentTarget.value)}><For each={names}>{n => <option value={n}>{n}</option>}</For></select></td></tr>
        <tr><td>Plane square to</td><td><div class="seg"><For each={["X", "Y", "Z"]}>{(l, i) => <button classList={{ on: ax() === i() }} onClick={() => setAx(i())}>{l}</button>}</For></div></td></tr>
        <tr><td>At</td><td><input class="mono" type="number" value={at()} onInput={e => setAt(+e.currentTarget.value)} /></td><td class="muted">mm along it</td></tr>
      </tbody></table>
    </Dialog>;
  });
};

// Measure: the first click measures what it hits; the second, the distance and angle from the first.
type Item = { type: "face" | "edge"; body: string; point: [number, number, number] };
export const [measured, setMeasured] = createSignal<Item[]>([]);
const n = (x: unknown, d = 2) => typeof x === "number" ? x.toFixed(d) : "";
const one = (r: Record<string, unknown>) => [
  r.length_mm != null && `length ${n(r.length_mm)} mm`, r.area_mm2 != null && `area ${n(r.area_mm2)} mm²`, r.diameter_mm != null && `diameter ${n(r.diameter_mm)} mm`,
  r.radius_mm != null && `radius ${n(r.radius_mm)} mm`, r.angle_deg != null && `angle ${n(r.angle_deg, 1)}°`,
].filter(Boolean).join(", ");
export const measureClick = async (h: CadHit) => {
  const it: Item | null = h.edge ? { type: "edge", body: h.edge.body, point: h.edge.at } : h.face ? { type: "face", body: h.face.body, point: h.face.at } : null;
  if (!it) return;
  const items = measured().length === 1 ? [measured()[0], it] : [it];
  setMeasured(items);
  const r = await cadQuery<Record<string, unknown> & { items?: Record<string, unknown>[] }>("inspect.measure", { items });
  if (!r) return;
  const what = (r.items ?? []).map((x, i) => `${i ? "Second" : "First"} (${x.type}): ${one(x) || "measured"}`);
  if (items.length === 2) what.push(`Distance between them ${n(r.distance_mm)} mm${r.angle_deg != null ? `, at ${n(r.angle_deg, 1)}° to each other` : ""}. Click again to start over.`);
  else what.push("Click a second face or edge for the distance and angle between them.");
  toast(what.join(" · "), true);
  if (items.length === 2) setMeasured([]);
};

/** Each solid's volume, surface area, centre of mass and mass (from the model's material). */
export const massDialog = async () => {
  type B = { name: string; closed: boolean; volume_mm3?: number; area_mm2: number; center_of_mass?: number[] };
  const r = await cadQuery<{ bodies: B[] }>("inspect.measure", {});
  if (!r) return;
  if (!r.bodies.length) return toast("There is no solid to measure yet.");
  const rho = model()?.materials[0]?.rho ?? 0, mat = model()?.materials[0]?.name ?? "";
  openDialog(() => <Dialog title="Mass and size" help="modelling" cancel="Close">
    <table class="kv" style={{ "border-collapse": "separate", "border-spacing": "14px 4px" }}><thead><tr><td>Solid</td><td>Volume</td><td>Area</td><td>Mass</td><td>Centre of mass</td></tr></thead><tbody>
      <For each={r.bodies}>{b => <tr><td>{b.name}</td><td class="mono">{b.volume_mm3 != null ? `${(b.volume_mm3 / 1e3).toFixed(2)} cm³` : "open"}</td><td class="mono">{(b.area_mm2 / 100).toFixed(2)} cm²</td>
        <td class="mono">{b.volume_mm3 != null ? `${(b.volume_mm3 * 1e-9 * rho).toFixed(3)} kg` : ""}</td><td class="mono">{b.center_of_mass?.map(x => x.toFixed(1)).join(", ") ?? ""}{b.center_of_mass ? " mm" : ""}</td></tr>}</For>
    </tbody></table>
    <Show when={mat}><p class="muted small">Mass from {mat} ({rho} kg/m³).</p></Show>
  </Dialog>);
};

/** A STEP file's solids into the design. */
export const importStep = async () => {
  pick3(null);
  try {
    const sc = await invoke<CadScene | null>("cad_import_step");
    if (!sc) return;
    setCad(sc); setMeshOld(true); frameIt();
    toast(`${sc.bodies.length} ${sc.bodies.length === 1 ? "solid" : "solids"} in the design now: give them supports and loads, then solve.`, true);
  } catch (e) { toast(String(e)); }
};

/** The solids out to a STEP or STL file. */
export const exportCad = async (format: "step" | "stl") => {
  if (!cad()?.bodies.length) return toast("There is no solid to export yet.");
  try {
    const p = await invoke<string | null>("cad_export", { name: model()?.name || "model", format });
    if (p) toast(`Saved ${p}`, true);
  } catch (e) { toast(String(e)); }
};

/** 3D frames (0.7.6): levels (planes at each floor, to sketch the floor's beams on) and a grid of
 *  construction lines on the ground (where the columns go), drawn once from counts and spacings. */
export const levelsDialog = () => openDialog(() => {
  const [n, setN] = createSignal(3), [h, setH] = createSignal(3.5), [nx, setNx] = createSignal(3), [sx, setSx] = createSignal(6), [ny, setNy] = createSignal(2), [sy, setSy] = createSignal(5);
  const ok = async () => {
    if (!(n() >= 0 && h() > 0 && nx() >= 1 && ny() >= 1 && sx() > 0 && sy() > 0)) { toast("Counts must be whole numbers from 1, sizes more than 0."); return false; }
    const have = new Set((cad()?.sketches ?? []).map(s => s.name));
    for (let i = 1; i <= Math.round(n()); i++) if (!(await cadRun("construct.plane.offset", { base: "XY", offset: i * h() * 1000, name: `Level ${i}` }))) return false;
    // The grid: construction lines (left out of the members), each bay's lines across the whole plan.
    const X = Math.round(nx()) * sx() * 1000, Y = Math.round(ny()) * sy() * 1000;
    if (!(await cadRun("sketch.create", { plane: "XY" }))) return false;
    for (let i = 0; i <= Math.round(nx()); i++) await cadRun("sketch.line", { points: [[i * sx() * 1000, 0], [i * sx() * 1000, Y]], construction: true });
    for (let j = 0; j <= Math.round(ny()); j++) await cadRun("sketch.line", { points: [[0, j * sy() * 1000], [X, j * sy() * 1000]], construction: true });
    await cadRun("sketch.finish");
    const g = (cad()?.sketches ?? []).find(s => !have.has(s.name));
    if (g) await cadRun("timeline.rename", { feature: g.id, name: "Grid" });
    frameIt();
    toast(`${Math.round(n())} levels and a ${Math.round(nx())} × ${Math.round(ny())} bay grid: New Sketch ▾ › On Level 1 (and so on) to draw each floor's beams; snap to the grid's crossings for the columns.`, true);
  };
  return <Dialog title="Levels and grid" okText="Make" ok={ok} help="sketchplane">
    <p class="muted small">Planes at each floor to sketch on, and dashed grid lines on the ground where the columns stand. The grid is construction: it guides the drawing and does not become members.</p>
    <table class="kv"><tbody>
      <tr><td>Levels above the ground</td><td><input class="mono" type="number" value={n()} onInput={e => setN(+e.currentTarget.value)} /></td><td class="muted">each</td><td><input class="mono" type="number" step="0.1" value={h()} onInput={e => setH(+e.currentTarget.value)} /></td><td class="muted">m apart</td></tr>
      <tr><td>Bays along X</td><td><input class="mono" type="number" value={nx()} onInput={e => setNx(+e.currentTarget.value)} /></td><td class="muted">each</td><td><input class="mono" type="number" step="0.1" value={sx()} onInput={e => setSx(+e.currentTarget.value)} /></td><td class="muted">m</td></tr>
      <tr><td>Bays along Y</td><td><input class="mono" type="number" value={ny()} onInput={e => setNy(+e.currentTarget.value)} /></td><td class="muted">each</td><td><input class="mono" type="number" step="0.1" value={sy()} onInput={e => setSy(+e.currentTarget.value)} /></td><td class="muted">m</td></tr>
    </tbody></table>
  </Dialog>;
});

/** Contact between two solids (0.8.0): they are meshed apart and every pair of faces where they
 *  touch gets a contact, with the friction coefficient given. */
export const contactDialog = () => {
  const names = (cad()?.bodies ?? []).map(b => b.name);
  if (names.length < 2) return toast("Contact is between two solids: make another (Extrude, Primitives) that touches this one.");
  openDialog(() => {
    const [a, setA] = createSignal(names[0]), [b, setB] = createSignal(names[1]), [mu, setMu] = createSignal(0.2);
    const ok = async () => {
      if (a() === b()) { toast("Pick two different solids."); return false; }
      try {
        const o = await invoke<Out>("cad_contacts", { a: a(), b: b(), friction: Math.max(0, mu()), size: (model()?.mesh.size || 0) * 1e3 });
        setOut(o); setMeshOld(false); import("./study").then(k => k.setMeshApart(true));
        toast(`${o.created.length} ${o.created.length === 1 ? "contact" : "contacts"} where ${a()} touches ${b()}: they now press and slide instead of being joined.`, true);
      } catch (e) { toast(String(e)); return false; }
    };
    return <Dialog title="Contact between solids" okText="Make" ok={ok} help="contact">
      <p class="muted small">Solids that touch are joined, as if glued. With contact they press on each other where they meet, part where they pull, and slide held back by friction. The solids are meshed apart and the faces where they touch are found.</p>
      <table class="kv"><tbody>
        <tr><td>Solid</td><td><select value={a()} onChange={e => setA(e.currentTarget.value)}><For each={names}>{n => <option value={n}>{n}</option>}</For></select></td></tr>
        <tr><td>Touching</td><td><select value={b()} onChange={e => setB(e.currentTarget.value)}><For each={names}>{n => <option value={n}>{n}</option>}</For></select></td></tr>
        <tr><td>Friction coefficient μ</td><td><input class="mono" type="number" step="0.05" min="0" value={mu()} onInput={e => setMu(+e.currentTarget.value)} /></td><td class="muted">0: none</td></tr>
      </tbody></table>
    </Dialog>;
  });
};
