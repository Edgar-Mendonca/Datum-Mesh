// More modelling (0.7.6): Sweep and Loft from finished sketches, Split a solid by a plane,
// Measure what is clicked (one face or edge, or the distance and angle between two), the mass of
// the solids, and STEP in and out (STL out). Each is one command of the geometry engine.
import { For, Show, createSignal } from "solid-js";
import { openDialog, toast, model } from "./store";
import { cad, setCad, cadRun, cadQuery, pick3, turnBack, setTurnBack, CadHit, CadScene, setMeshOld } from "./cad";
import { invoke } from "./api";
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
