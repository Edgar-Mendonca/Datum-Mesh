// Standard parts (0.10.3, the user): a small library of the parts an analysis most often needs (bolts,
// nuts, washers and pins in metric sizes), dropped into a 3D model by a click or by dragging one onto
// the view. Each is made by the geometry engine from simple solids, as an analysis model wants it:
// a bolt as its head and shank (no thread), a nut and a washer as rings. Where they go: at the
// origin, to be placed with Move or Copy. Loaded when the panel opens.
import { For, Show, createSignal } from "solid-js";
import { model, toast, note } from "./store";
import { cad, cadRun, turnBack, setTurnBack } from "./cad";
import { isSolidA } from "./api";
import { setPartsOpen } from "./panels";
import { showPlanned } from "./planned";

/** Metric coarse sizes: d, width across flats s, head height k, nut height m, washer bore, outside
 *  diameter and thickness (mm); the usual dimensions of hexagon bolts, nuts and plain washers. */
const SIZES: [string, number, number, number, number, number, number, number][] = [
  // name, d, s, k, m, washer d1, d2, h
  ["M6", 6, 10, 4, 5.2, 6.4, 12, 1.6], ["M8", 8, 13, 5.3, 6.8, 8.4, 16, 1.6], ["M10", 10, 16, 6.4, 8.4, 10.5, 20, 2],
  ["M12", 12, 18, 7.5, 10.8, 13, 24, 2.5], ["M16", 16, 24, 10, 14.8, 17, 30, 3], ["M20", 20, 30, 12.5, 18, 21, 37, 3], ["M24", 24, 36, 15, 21.5, 25, 44, 4],
];
const LENGTHS = [16, 20, 25, 30, 35, 40, 45, 50, 60, 70, 80, 90, 100, 120, 140, 160];
const PIN_D = [3, 4, 5, 6, 8, 10, 12, 16, 20];

type Kind = "bolt" | "nut" | "washer" | "pin";
type Pick = { kind: Kind; size: string; length: number };
const KINDS: [Kind, string, string, string][] = [
  ["bolt", "Hexagon bolts", "bolt", "Head and plain shank, metric coarse"],
  ["nut", "Hexagon nuts", "nut", "A ring of the nut's height"],
  ["washer", "Plain washers", "washer", "A flat ring"],
  ["pin", "Dowel pins", "pinpart", "A plain cylinder"],
];
/** Still to come in the library, with the release that brings them. */
const PLANNED: [string, string, string][] = [["Socket head screws", "bolt", "0.13"], ["Ball bearings", "bearing", "0.13"], ["Shaft keys", "rect", "0.13"], ["Springs and rubber mounts", "spring", "0.13"]];

const [kind, setKind] = createSignal<Kind>("bolt");
const [size, setSize] = createSignal("M12");
const [length, setLength] = createSignal(40);
const [q, setQ] = createSignal("");

const label = (p: Pick) => p.kind === "bolt" ? `Bolt ${p.size} × ${p.length}` : p.kind === "nut" ? `Nut ${p.size}` : p.kind === "washer" ? `Washer ${p.size}` : `Pin Ø${p.size} × ${p.length}`;

/** Makes one solid with the engine and returns its name (the body it added), or null. */
const make = async (id: string, params: object) => {
  const before = new Set((cad()?.bodies ?? []).map(b => b.name));
  if (!(await cadRun(id, params))) return null;
  return (cad()?.bodies ?? []).map(b => b.name).find(n => !before.has(n)) ?? null;
};

/** Puts a standard part into the model, at the origin, standing on the floor (along Z). */
export const insertPart = async (p: Pick) => {
  if (!isSolidA(model()?.analysis)) { toast("Standard parts go into a 3D solid model: start one from File › New (a blank 3D solid)."); return; }
  const z = SIZES.find(s => s[0] === p.size);
  const O: [number, number, number] = [0, 0, 0];
  if (p.kind === "pin") {
    const d = parseFloat(p.size); if (!(d > 0)) return;
    await make("solid.cylinder", { base: O, radius: d / 2, height: p.length });
  } else if (z) {
    const [, d, s, k, m, d1, d2, h] = z;
    if (p.kind === "bolt") {
      const shank = await make("solid.cylinder", { base: O, radius: d / 2, height: p.length });
      const head = await make("solid.cylinder", { base: [0, 0, p.length], radius: s / 2, height: k });
      if (shank && head) await cadRun("solid.combine", { target: head, tools: [shank], operation: "join" });
    } else {
      const [ro, ri, t] = p.kind === "nut" ? [s / 2, d / 2, m] : [d2 / 2, d1 / 2, h];
      const ring = await make("solid.cylinder", { base: O, radius: ro, height: t });
      const bore = await make("solid.cylinder", { base: [0, 0, -0.1], radius: ri, height: t + 0.2 });
      if (ring && bore) await cadRun("solid.combine", { target: ring, tools: [bore], operation: "cut" });
    }
  }
  setTurnBack(turnBack() + 1);
  note(`Standard part: ${label(p)} placed at the origin (move it with Move or Copy)`, "info", "action");
};

/** The view takes a part dropped on it (0.10.3, the user: drag and drop). */
export const dropPart = (e: DragEvent) => {
  const t = e.dataTransfer?.getData("application/x-datum-part"); if (!t) return;
  e.preventDefault();
  try { insertPart(JSON.parse(t) as Pick); } catch { /* not ours */ }
};

export const StdParts = () => {
  const rows = () => kind() === "pin" ? PIN_D.map(d => String(d)) : SIZES.map(s => s[0]);
  const shown = () => KINDS.filter(k => !q().trim() || `${k[1]} ${k[3]}`.toLowerCase().includes(q().trim().toLowerCase()));
  const solid = () => isSolidA(model()?.analysis);
  return <aside class="stdparts" aria-label="Standard parts">
    <header><b>Standard parts</b><button class="x" title="Close" onClick={() => setPartsOpen(false)}>×</button></header>
    <label class="msearch"><svg class="ic"><use href="#i-search" /></svg><input type="search" placeholder="Search the library" value={q()} onInput={e => setQ(e.currentTarget.value)} /></label>
    <Show when={!solid()}><p class="muted small spnote">Standard parts go into 3D solid models. This model is not one; open or start a 3D solid to use them.</p></Show>
    <div class="spcats"><For each={shown()}>{([k, l, ic, what]) =>
      <button class="spcat" classList={{ on: kind() === k }} data-kind={k} onClick={() => setKind(k)} title={what}><svg class="ic"><use href={`#i-${ic}`} /></svg><span>{l}</span></button>}</For>
      <Show when={showPlanned()}><For each={PLANNED}>{([l, ic, v]) => <button class="spcat planned" title={`${l}: planned for ${v}`} onClick={() => toast(`${l}: planned for ${v}.`, true)}><svg class="ic"><use href={`#i-${ic}`} /></svg><span>{l}</span><i>{v}</i></button>}</For></Show>
    </div>
    <div class="spsizes">
      <div class="sphead">{kind() === "pin" ? "Diameter" : "Size"}<Show when={kind() === "bolt" || kind() === "pin"}><label>Length <select value={length()} onChange={e => setLength(+e.currentTarget.value)}><For each={LENGTHS}>{l => <option value={l}>{l} mm</option>}</For></select></label></Show></div>
      <For each={rows()}>{r => {
        const p = (): Pick => ({ kind: kind(), size: r, length: length() });
        const z = SIZES.find(s => s[0] === r);
        return <div class="sprow" classList={{ on: size() === r && kind() !== "pin" }} draggable={solid()} title={`${label(p())}: drag it onto the view, or click Insert`}
          onDragStart={e => { e.dataTransfer?.setData("application/x-datum-part", JSON.stringify(p())); e.dataTransfer!.effectAllowed = "copy"; }}
          onClick={() => setSize(r)}>
          <span class="spname">{kind() === "pin" ? `Ø${r}` : r}</span>
          <small>{kind() === "bolt" && z ? `head ${z[2]} × ${z[3]}` : kind() === "nut" && z ? `${z[2]} across flats, ${z[4]} high` : kind() === "washer" && z ? `Ø${z[5]}/${z[6]} × ${z[7]}` : `× ${length()} mm`}</small>
          <button class="btn sm" disabled={!solid()} onClick={e => { e.stopPropagation(); insertPart(p()); }}>Insert</button></div>;
      }}</For>
    </div>
    <p class="muted small spnote">Drag a size onto the view, or click Insert: it goes in at the origin, standing on the floor. Simplified for analysis (no thread).</p>
  </aside>;
};
