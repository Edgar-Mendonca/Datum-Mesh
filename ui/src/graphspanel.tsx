// The bottom panel's Graphs (0.10.2, the mock-ups): a time history's movement against time beside the
// record that drove it (the ground's shaking, or the load's factor in time), as the engine solved
// them. Loaded only when the tab opens; nothing here calculates.
import { For, Show, createSignal, onCleanup } from "solid-js";
import { model } from "./store";
import { num } from "./api";
import { toU, unit } from "./units";
import { dynOf } from "./steps";

type Line = { title: string; xs: number[]; ys: number[]; peak?: number; colour: string };
const L = 52, B = 26, T = 12;
/** Drawn at the panel's own size (no stretching of the text). */
const Graph = (p: { g: Line }) => {
  const [size, setSize] = createSignal<[number, number]>([640, 190]);
  const W0 = () => size()[0], H0 = () => size()[1];
  const watch = (el: HTMLElement) => { const ro = new ResizeObserver(() => setSize([Math.max(200, el.clientWidth), Math.max(80, el.clientHeight)])); ro.observe(el); onCleanup(() => ro.disconnect()); };
  const x1 = () => Math.max(...p.g.xs, 1e-9), lo = () => Math.min(0, ...p.g.ys), hi = () => Math.max(0, ...p.g.ys);
  const sx = (x: number) => L + x / x1() * (W0() - L - 12), sy = (y: number) => T + (hi() - y) / ((hi() - lo()) || 1) * (H0() - T - B);
  const path = () => p.g.ys.map((y, i) => `${i ? "L" : "M"}${sx(p.g.xs[i]).toFixed(1)} ${sy(y).toFixed(1)}`).join("");
  const ticks = () => [hi(), (hi() + lo()) / 2, lo()];
  return <figure class="gpanel"><figcaption>{p.g.title}</figcaption>
    <div class="gbox" ref={watch}><svg viewBox={`0 0 ${W0()} ${H0()}`} width={W0()} height={H0()} role="img" aria-label={p.g.title}>
      <For each={ticks()}>{t => <><line class="grid" x1={L} x2={W0() - 12} y1={sy(t)} y2={sy(t)} /><text x={L - 6} y={sy(t) + 3} text-anchor="end">{num(t, 3)}</text></>}</For>
      <line class="ax" x1={L} x2={W0() - 12} y1={sy(0)} y2={sy(0)} />
      <Show when={p.g.peak !== undefined}><line class="peak" x1={sx(p.g.peak!)} x2={sx(p.g.peak!)} y1={T} y2={H0() - B} /></Show>
      <path d={path()} fill="none" stroke={p.g.colour} stroke-width="1.4" vector-effect="non-scaling-stroke" />
      <text x={W0() - 12} y={H0() - 6} text-anchor="end">t (s) · {num(x1(), 3)}</text>
    </svg></div></figure>;
};

export const GraphsPanel = () => {
  const lines = (): Line[] => {
    const d = dynOf(); if (!d || d.kind !== "history") return [];
    // The component that moves most at the node followed.
    const comps = (["ux", "uy", "uz"] as const).filter(k => d[k]?.length);
    const k = comps.reduce((a, c) => (Math.max(...d[c].map(Math.abs)) > Math.max(...d[a].map(Math.abs)) ? c : a), comps[0]);
    const n = (model()?.nodes.findIndex(x => x.id === d.node) ?? -1) + 1;
    const out: Line[] = [{ title: `Movement in ${k.slice(1)} of node ${n} (${unit("len")})`, xs: d.t, ys: d[k].map(v => toU("len", v)), peak: d.peak_t, colour: "#0F6E8C" }];
    const st = (model()?.steps ?? []).find(s => s.kind === "history");
    if (st?.time.length) out.push({ title: st.ground ? "Ground acceleration, as given" : "Factor on the load, as given", xs: st.time.map(r => r[0]), ys: st.time.map(r => r[1]), colour: "#C27A12" });
    return out;
  };
  return <Show when={lines().length} fallback={<p class="muted cempty">Graphs show a time history's movement against time: add a time history step (Analysis › Steps) and solve.</p>}>
    <div class="gpanels"><For each={lines()}>{g => <Graph g={g} />}</For></div>
  </Show>;
};
