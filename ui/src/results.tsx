// Frame results: drawn over the canvas (deflected shape, axial, shear and moment diagrams,
// reactions, utilisation colours, a probe that follows the pointer, a legend). The Results page is
// in resultspage.tsx. The engine computes every number.
import { For, Show, JSX, createMemo } from "solid-js";
import { Pt, MemberResult, Model, FrameResults, num, clean } from "./api";
import { solved, frameRes as results, stale, show, model, cursor, setView, modeOn, solvedFor } from "./store";
import { toS, scale } from "./canvas";
import { fmt } from "./units";
import { C, imbalanceOf } from "./cmds";
import { engineAxis } from "./axes";
import { Icon } from "./ui";
import { dynDialog } from "./steps";

const DIAG: Record<string, [string, "force" | "moment"]> = { n: ["Axial force", "force"], v: ["Shear force", "force"], m: ["Bending moment", "moment"] };

/** Model size, for scaling the deflected shape. */
const span = (m: Model) => {
  const xs = m.nodes.map(n => n.x), ys = m.nodes.map(n => n.y);
  return Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys), 1e-9);
};
/** How many times the deflections are drawn larger than they are. */
/** Component k (0–2 forces, 3–5 moments) of an engine 6-vector, as shown (Z up in 3D, 0.6.1). */
export const comp = (v: number[], k: number) => { const [e, sg] = engineAxis(k % 3); return sg * v[e + (k < 3 ? 0 : 3)]; };

export const deformFactor = (m: Model, r: FrameResults) => {
  // Large deflection (0.4.0): the shape as it truly is, not exaggerated.
  if (r.path) return show().scale;
  const dmax = Math.max(0, ...solved({ ...r, kind: "frame" as const }).members.flatMap(mr => mr.d.map(d => Math.hypot(d[0], d[1]))));
  // Nothing moves (no loads): no exaggeration to speak of (0.1.1 showed "× 2e+299").
  if (!(dmax > 1e-12 * span(m))) return 0;
  const f = 0.08 * span(m) / dmax;
  // A round number, so the legend reads well.
  const p = 10 ** Math.floor(Math.log10(f)), k = f / p;
  return (k >= 5 ? 5 : k >= 2 ? 2 : 1) * p * show().scale;
};

type Geo = { a: Pt; b: Pt; c: number; s: number };
const geo = (m: Model, id: number): Geo | null => {
  const mb = m.members.find(x => x.id === id); if (!mb) return null;
  const a = m.nodes.find(n => n.id === mb.a), b = m.nodes.find(n => n.id === mb.b); if (!a || !b) return null;
  const l = Math.hypot(b.x - a.x, b.y - a.y);
  return { a: [a.x, a.y], b: [b.x, b.y], c: (b.x - a.x) / l, s: (b.y - a.y) / l };
};
const along = (g: Geo, x: number): Pt => [g.a[0] + g.c * x, g.a[1] + g.s * x];

export const utilColour = (u: number) => (u >= 1 ? "var(--crit)" : u >= 0.5 ? "var(--amber)" : "var(--green)");

/** The probe: the member station nearest the pointer, within a few pixels. */
const probe = createMemo(() => {
  const r = results(), m = model(), c = cursor(); if (!r || !m || !c) return null;
  let best: { mr: MemberResult; i: number; d: number } | null = null;
  for (const mr of r.members) {
    const g = geo(m, mr.id); if (!g) continue;
    // Project onto the member.
    const t = Math.max(0, Math.min(mr.length, (c[0] - g.a[0]) * g.c + (c[1] - g.a[1]) * g.s));
    const p = along(g, t), d = Math.hypot(p[0] - c[0], p[1] - c[1]) * scale();
    if (d < 10 && (!best || d < best.d)) best = { mr, i: Math.round(t / mr.length * (mr.x.length - 1)), d };
  }
  return best;
});

export const ResultsLayer = () => {
  const r = () => results(), m = () => model();
  const f = createMemo(() => (r() && m() ? deformFactor(m()!, r()!) : 1));
  const diag = createMemo(() => {
    const k = show().diagram, rr = r(); if (k === "none" || !rr) return null;
    const vals = rr.members.flatMap(mr => mr[k]);
    return { k, max: Math.max(...vals), min: Math.min(...vals), big: Math.max(...vals.map(Math.abs), 1e-300) };
  });
  const deformed = (mr: MemberResult) => {
    const g = geo(m()!, mr.id); if (!g) return "";
    return mr.x.map((x, i) => { const p = along(g, x), d = mr.d[i], s = toS([p[0] + d[0] * f(), p[1] + d[1] * f()]); return `${i ? "L" : "M"}${s[0]} ${s[1]}`; }).join("");
  };
  /** The diagram as a closed shape off the member: moments on the tension side, N and V on local +y. */
  const diagramPath = (mr: MemberResult) => {
    const dg = diag(), g = geo(m()!, mr.id); if (!dg || !g) return { d: "", peaks: [] as [Pt, number][] };
    const vals = mr[dg.k], k = 50 / dg.big, sign = dg.k === "m" ? -1 : 1;
    // Local +y on screen is (−s, −c) (the screen flips y).
    const off = (x: number, v: number): Pt => { const s = toS(along(g, x)); return [s[0] - g.s * v * k * sign, s[1] - g.c * v * k * sign]; };
    const pts = mr.x.map((x, i) => off(x, vals[i]));
    const a = toS(g.a), b = toS(g.b);
    const d = `M${a[0]} ${a[1]}` + pts.map(p => `L${p[0]} ${p[1]}`).join("") + `L${b[0]} ${b[1]}z`;
    // Label the end values and the extreme inside the member.
    const iMax = vals.reduce((bi, v, i) => (Math.abs(v) > Math.abs(vals[bi]) ? i : bi), 0);
    const marks = [...new Set([0, vals.length - 1, iMax])].filter(i => Math.abs(vals[i]) > dg.big * 0.02);
    return { d, peaks: marks.map(i => [off(mr.x[i], vals[i] * 1.12), vals[i]] as [Pt, number]) };
  };
  const reactionGlyphs = () => {
    const rr = r(), mm = m(); if (!rr || !mm) return [];
    const big = Math.max(1e-300, ...rr.reactions.flatMap(x => [Math.abs(x.fx), Math.abs(x.fy)]));
    return rr.reactions.flatMap(x => {
      const n = mm.nodes.find(q => q.id === x.node); if (!n) return [];
      const [sx, sy] = toS([n.x, n.y]), out: JSX.Element[] = [];
      for (const [v, dx, dy] of [[x.fx, 1, 0], [x.fy, 0, -1]] as [number, number, number][]) {
        if (Math.abs(v) < big * 1e-6) continue;
        const len = 18 + 34 * Math.abs(v) / big, sg = Math.sign(v);
        // The arrow points the way the support pushes, ending at the node.
        const tail: Pt = [sx - dx * sg * len, sy - dy * sg * len];
        out.push(<path d={`M${tail[0]} ${tail[1]}L${sx} ${sy}M${sx - dx * sg * 7 - dy * 4} ${sy - dy * sg * 7 - dx * 4}L${sx} ${sy}L${sx - dx * sg * 7 + dy * 4} ${sy - dy * sg * 7 + dx * 4}`} class="rarrow" />);
        out.push(<text x={tail[0] + (dx ? -4 * sg : 6)} y={tail[1] + (dy ? 4 * sg : -4) + (dy && sg < 0 ? 8 : 0)} class="rtxt2" text-anchor={dx ? (sg > 0 ? "end" : "start") : "start"}>{fmt("force", v, 3)}</text>);
      }
      if (Math.abs(x.m) > 1e-9) out.push(<text x={sx + 8} y={sy + 26} class="rtxt2">{`M ${fmt("moment", x.m, 3)}`}</text>);
      return out;
    });
  };
  return (
    <Show when={r() && m() && m()!.analysis === "frame" && !modeOn()}>
      <g class="results" classList={{ stale: stale() }}>
        {/* Everything below reads the view (toS) inside JSX or a memo, so it follows pan and zoom (0.1.1 bug: drawn once). */}
        <Show when={show().util}><For each={r()!.members}>{mr => <Show when={geo(m()!, mr.id)}>{g => <line x1={toS(g().a)[0]} y1={toS(g().a)[1]} x2={toS(g().b)[0]} y2={toS(g().b)[1]} class="utilline" style={{ stroke: utilColour(mr.util) }} />}</Show>}</For></Show>
        <Show when={diag()}><For each={r()!.members}>{mr => { const p = createMemo(() => diagramPath(mr)); return <><path d={p().d} class="diag" data-id={mr.id} classList={{ [`d-${show().diagram}`]: true }} /><For each={p().peaks}>{([q, v]) => <text x={q[0]} y={q[1]} class="dtxt" text-anchor="middle">{fmt(DIAG[show().diagram][1], clean(v, diag()!.big), 3)}</text>}</For></>; }}</For></Show>
        <Show when={show().deformed && f() > 0}><For each={r()!.members}>{mr => <path d={deformed(mr)} class="defl" />}</For></Show>
        <Show when={show().reactions}>{reactionGlyphs()}</Show>
        <Show when={probe()}>{p => { const at = () => toS(along(geo(m()!, p().mr.id)!, p().mr.x[p().i])); return <circle cx={at()[0]} cy={at()[1]} r={4.5} class="probept" />; }}</Show>
      </g>
    </Show>
  );
};

/** The legend and the probe readout, over the canvas (HTML, so it reads at any zoom). */
export const ResultsLegend = () => {
  const r = () => results(), m = () => model();
  const diagInfo = createMemo(() => {
    const k = show().diagram, rr = r(); if (k === "none" || !rr) return null;
    const vals = rr.members.flatMap(mr => mr[k]);
    return { name: DIAG[k][0], q: DIAG[k][1], max: Math.max(...vals), min: Math.min(...vals), big: Math.max(...vals.map(Math.abs)) };
  });
  const dmax = () => Math.max(0, ...(r()?.members.flatMap(mr => mr.d.map(d => Math.hypot(d[0], d[1]))) ?? [0]));
  return (
    <Show when={r() && m()?.analysis === "frame" && !modeOn()}>
      <div class="legend">
        <Show when={stale()}><div class="stalebar"><Icon n="info" /> The model changed since it was solved. <button class="btn sm pri" onClick={C.solve}>Solve again (F5)</button></div></Show>
        <Show when={solvedFor()}><div><b>Load case</b><span>{solvedFor()}</span></div></Show>
        <Show when={diagInfo()}>{d => <div><b>{d().name}</b><span>max {fmt(d().q, clean(d().max, d().big))} · min {fmt(d().q, clean(d().min, d().big))}</span><small>{show().diagram === "m" ? "Drawn on the tension side." : "Positive drawn on the member's left side (looking from start to end); tension positive."}</small></div>}</Show>
        <Show when={show().deformed}><div><b>Deflected shape</b><span>{r()!.path ? <>largest {fmt("len", dmax())} · drawn true to size{show().scale !== 1 ? ` × ${num(show().scale, 3)}` : ""} (large deflection)</> : deformFactor(m()!, r()!) > 0 ? <>largest {fmt("len", dmax())} · drawn × {num(deformFactor(m()!, r()!), 3)}</> : "nothing moves: there are no loads"}</span>
          <input type="range" min="0.1" max="5" step="0.1" value={show().scale} onInput={e => C.show({ scale: +e.currentTarget.value })} title="Exaggeration" /></div></Show>
        <Show when={r()!.slack?.length}><div><b>Slack</b><span>{r()!.slack!.length} tension-only or compression-only member{r()!.slack!.length > 1 ? "s" : ""} carr{r()!.slack!.length > 1 ? "y" : "ies"} nothing under these loads (dashed)</span></div></Show>
        <Show when={show().util}><div><b>Utilisation</b><span class="ukey"><i style={{ background: "var(--green)" }} />below 50 %<i style={{ background: "var(--amber)" }} />50–100 %<i style={{ background: "var(--crit)" }} />over 100 % (yields)</span></div></Show>
        <Show when={r()!.dynamic?.kind !== "spectrum"} fallback={<div class="eqline"><Icon n="info" /><span>Peak values of the modes combined by {r()!.dynamic!.combined}: no balance to check</span><button class="lnk" onClick={dynDialog}>Modes</button></div>}>
        <div class="eqline" classList={{ bad: imbalanceOf(r()!) > 1e-6 }}><Icon n={imbalanceOf(r()!) > 1e-6 ? "info" : "check"} /><span>{imbalanceOf(r()!) > 1e-6 ? "Reactions do not balance the loads" : r()!.dynamic ? `Equilibrium at t = ${num(r()!.dynamic!.peak_t, 4)} s, inertia included` : "Equilibrium: reactions balance the loads"}</span><button class="lnk" onClick={() => setView("results")}>Details</button></div></Show>
      </div>
    </Show>
  );
};

/** The largest size of a result over the whole model, for telling rounding noise from values. */
export const big = (k: "n" | "v" | "m" | "d") => {
  const r = results(); if (!r) return 0;
  return Math.max(0, ...r.members.flatMap(mr => k === "d" ? mr.d.map(d => Math.hypot(d[0], d[1])) : mr[k].map(Math.abs)));
};

/** The probe readout over the canvas. */
export const ResultsProbe = () => (
  <Show when={probe() && model()}>{_ => { const p = probe()!, mr = p.mr, i = p.i, d = mr.d[i]; return (
    <div class="probe"><b>Member {model()!.members.findIndex(x => x.id === mr.id) + 1} at {fmt("len", mr.x[i])}</b>
      <span>N {fmt("force", clean(mr.n[i], big("n")))}</span><span>V {fmt("force", clean(mr.v[i], big("v")))}</span><span>M {fmt("moment", clean(mr.m[i], big("m")))}</span><span>δ {fmt("len", clean(Math.hypot(d[0], d[1]), big("d")))}</span></div>); }}</Show>
);
