// Extrude with the solid in view (0.7.2): the solid is made at once from the last sketch, and the
// panel over the 3D view changes it live (its distance through the feature's own input, its
// direction through its definition); OK keeps it, Cancel takes it away again.
import { For, Show, createSignal } from "solid-js";
import { toast } from "./store";
import { cad, cadRun, cadQuery, extrudeD, setExtrudeD, extrudeDir, setExtrudeDir } from "./cad";

type Panel = { feature: number; name: string; param: string | null; extent: Record<string, unknown> | null; dist: string; dir: "positive" | "negative" | "symmetric" };
export const [extrudePanel, setExtrudePanel] = createSignal<Panel | null>(null);

/** Extrude the last sketch and open the panel on the new feature. */
export const openExtrude = async () => {
  const sks = cad()?.sketches ?? [], sk = sks[sks.length - 1];
  if (!sk) return toast("Draw a sketch first: New Sketch, then a closed shape.");
  if (!sk.profiles) return toast(`${sk.name} has no closed region to extrude: close the shape.`);
  if (!(await cadRun("solid.extrude", { distance: extrudeD(), direction: extrudeDir(), sketch: sk.id }))) return;
  const f = (cad()?.features ?? []).slice(-1)[0]; if (!f) return;
  const r = await cadQuery<any>("timeline.inspect", { feature: f.id });
  const input = (r?.inputs ?? []).find((i: any) => /distance/i.test(i.label)) ?? (r?.inputs ?? [])[0];
  setExtrudePanel({ feature: f.id, name: f.name, param: input?.param ?? null, extent: r?.definition?.extent ?? null, dist: input?.expression ?? String(extrudeD()), dir: extrudeDir() });
};

let timer = 0;
const setDist = (v: string) => {
  const p = extrudePanel(); if (!p) return;
  setExtrudePanel({ ...p, dist: v });
  clearTimeout(timer);
  // The solid follows as the size is typed (after a short pause), through the feature's input.
  timer = window.setTimeout(async () => {
    const n = parseFloat(v); if (!(n > 0) || !p.param) return;
    if (await cadRun("parameters.change", { name: p.param, expression: v })) setExtrudeD(n);
  }, 180);
};
const nudge = (k: number) => { const p = extrudePanel(); if (!p) return; const n = parseFloat(p.dist) || 0, step = n >= 100 ? 10 : n >= 10 ? 1 : 0.5; setDist(String(Math.max(step, +(n + k * step).toFixed(3)))); };
const setDir = async (d: Panel["dir"]) => {
  const p = extrudePanel(); if (!p) return;
  if (p.extent && await cadRun("timeline.edit", { feature: p.feature, set: { extent: { ...p.extent, direction: d } } })) { setExtrudePanel({ ...p, dir: d, extent: { ...p.extent, direction: d } }); setExtrudeDir(d); }
};

/** The panel over the 3D view. */
export const ExtrudePanel = () => (
  <Show when={extrudePanel()}>{p => (
    <div class="xpanel" onPointerDown={e => e.stopPropagation()} onPointerUp={e => e.stopPropagation()} onWheel={e => { e.stopPropagation(); e.preventDefault(); nudge(e.deltaY < 0 ? 1 : -1); }}>
      <header><b>Extrude</b><small>{p().name}</small></header>
      <label>Distance<span class="xrow"><button class="btn sm" title="Less" onClick={() => nudge(-1)}>−</button>
        <input class="mono" value={p().dist} onInput={e => setDist(e.currentTarget.value)} onKeyDown={e => { if (e.key === "Enter") setExtrudePanel(null); if (e.key === "ArrowUp") nudge(1); if (e.key === "ArrowDown") nudge(-1); }} ref={el => setTimeout(() => { el.focus(); el.select(); })} />
        <button class="btn sm" title="More" onClick={() => nudge(1)}>+</button><span class="unit">mm</span></span></label>
      <div class="seg"><For each={[["positive", "Out"], ["negative", "In"], ["symmetric", "Both ways"]] as const}>{([k, l]) => <button classList={{ on: p().dir === k }} onClick={() => setDir(k)}>{l}</button>}</For></div>
      <footer><button class="btn pri sm" onClick={() => setExtrudePanel(null)}>OK</button>
        <button class="btn sm" onClick={async () => { if (await cadRun("timeline.delete", { feature: p().feature })) setExtrudePanel(null); }}>Cancel</button></footer>
    </div>)}</Show>
);
