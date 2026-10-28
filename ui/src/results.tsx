// Frame results: drawn over the canvas (deflected shape, axial, shear and moment diagrams,
// reactions, utilisation colours, a probe that follows the pointer, a legend) and the Results page
// (equilibrium check, reactions, members, the solver report). The engine computes every number.
import { For, Show, JSX, createMemo } from "solid-js";
import { Pt, MemberResult, Model, FrameResults, num, clean } from "./api";
import { results, stale, show, model, cursor, setView, sel, setSel } from "./store";
import { toS, scale } from "./canvas";
import { fmt, unit } from "./units";
import { C, imbalanceOf } from "./cmds";
import { Help } from "./help";
import { Icon } from "./ui";

const DIAG: Record<string, [string, "force" | "moment"]> = { n: ["Axial force", "force"], v: ["Shear force", "force"], m: ["Bending moment", "moment"] };

/** Model size, for scaling the deflected shape. */
const span = (m: Model) => {
  const xs = m.nodes.map(n => n.x), ys = m.nodes.map(n => n.y);
  return Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys), 1e-9);
};
/** How many times the deflections are drawn larger than they are. */
export const deformFactor = (m: Model, r: FrameResults) => {
  const dmax = Math.max(0, ...r.members.flatMap(mr => mr.d.map(d => Math.hypot(d[0], d[1]))));
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

const utilColour = (u: number) => (u >= 1 ? "var(--crit)" : u >= 0.5 ? "var(--amber)" : "var(--green)");

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
    <Show when={r() && m() && m()!.analysis === "frame"}>
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
    <Show when={r() && m()?.analysis === "frame"}>
      <div class="legend">
        <Show when={stale()}><div class="stalebar"><Icon n="info" /> The model changed since it was solved. <button class="btn sm pri" onClick={C.solve}>Solve again (F5)</button></div></Show>
        <Show when={diagInfo()}>{d => <div><b>{d().name}</b><span>max {fmt(d().q, clean(d().max, d().big))} · min {fmt(d().q, clean(d().min, d().big))}</span><small>{show().diagram === "m" ? "Drawn on the tension side." : "Positive drawn on the member's left side (looking from start to end); tension positive."}</small></div>}</Show>
        <Show when={show().deformed}><div><b>Deflected shape</b><span>{deformFactor(m()!, r()!) > 0 ? <>largest {fmt("len", dmax())} · drawn × {num(deformFactor(m()!, r()!), 3)}</> : "nothing moves: there are no loads"}</span>
          <input type="range" min="0.1" max="5" step="0.1" value={show().scale} onInput={e => C.show({ scale: +e.currentTarget.value })} title="Exaggeration" /></div></Show>
        <Show when={show().util}><div><b>Utilisation</b><span class="ukey"><i style={{ background: "var(--green)" }} />below 50 %<i style={{ background: "var(--amber)" }} />50–100 %<i style={{ background: "var(--crit)" }} />over 100 % (yields)</span></div></Show>
        <div class="eqline" classList={{ bad: imbalanceOf(r()!) > 1e-6 }}><Icon n={imbalanceOf(r()!) > 1e-6 ? "info" : "check"} /><span>{imbalanceOf(r()!) > 1e-6 ? "Reactions do not balance the loads" : "Equilibrium: reactions balance the loads"}</span><button class="lnk" onClick={() => setView("results")}>Details</button></div>
      </div>
    </Show>
  );
};

/** The largest size of a result over the whole model, for telling rounding noise from values. */
const big = (k: "n" | "v" | "m" | "d") => {
  const r = results(); if (!r) return 0;
  return Math.max(0, ...r.members.flatMap(mr => k === "d" ? mr.d.map(d => Math.hypot(d[0], d[1])) : mr[k].map(Math.abs)));
};

/** The probe readout over the canvas. */
export const ResultsProbe = () => (
  <Show when={probe() && model()}>{_ => { const p = probe()!, mr = p.mr, i = p.i, d = mr.d[i]; return (
    <div class="probe"><b>Member {model()!.members.findIndex(x => x.id === mr.id) + 1} at {fmt("len", mr.x[i])}</b>
      <span>N {fmt("force", clean(mr.n[i], big("n")))}</span><span>V {fmt("force", clean(mr.v[i], big("v")))}</span><span>M {fmt("moment", clean(mr.m[i], big("m")))}</span><span>δ {fmt("len", clean(Math.hypot(d[0], d[1]), big("d")))}</span></div>); }}</Show>
);

const Eq = (p: { r: FrameResults }) => {
  const rows: [string, number, number, "force" | "moment", number][] = [["Σ Fx", p.r.applied[0], p.r.reacted[0], "force", p.r.scale[0]], ["Σ Fy", p.r.applied[1], p.r.reacted[1], "force", p.r.scale[0]], ["Σ M about the origin", p.r.applied[2], p.r.reacted[2], "moment", p.r.scale[1]]];
  return <table class="tbl"><thead><tr><th /><th class="r">Loads</th><th class="r">Reactions</th><th class="r">Sum</th></tr></thead><tbody>
    <For each={rows}>{([l, a, b, q, sc]) => <tr><td>{l}</td><td class="r">{fmt(q, clean(a, sc))}</td><td class="r">{fmt(q, clean(b, sc))}</td><td class="r">{fmt(q, clean(a + b, sc))}</td></tr>}</For></tbody></table>;
};

/** View › Results: the tables behind the pictures. */
export const ResultsPage = () => {
  const r = () => results(), m = () => model()!;
  const nodeName = (id: number) => `Node ${m().nodes.findIndex(n => n.id === id) + 1}`;
  return (
    <div class="page">
      <h1>Results <Help k="results" /></h1>
      <Show when={r()} fallback={<div class="emptypage"><Icon n="solve" size={34} /><b>{m().analysis === "frame" ? "Solve the model to see its results" : "Solving parts arrives in 0.1.3"}</b><p class="muted">Deflections, reactions, the equilibrium check and the forces in every member.</p><Show when={m().analysis === "frame"}><p><button class="btn pri" onClick={C.solve}>Solve (F5)</button></p></Show></div>}>{rr => <>
        <Show when={stale()}><p class="stalebar"><Icon n="info" /> The model changed since it was solved. <button class="btn sm pri" onClick={C.solve}>Solve again</button></p></Show>
        <Show when={rr().warnings.length}><For each={rr().warnings}>{w => <p class="issue warn">{w}</p>}</For></Show>
        <h2>Equilibrium check <Help k="equilibrium" /></h2>
        <p classList={{ okline: imbalanceOf(rr()) <= 1e-6, badline: imbalanceOf(rr()) > 1e-6 }}><Icon n={imbalanceOf(rr()) > 1e-6 ? "info" : "check"} /> {imbalanceOf(rr()) > 1e-6 ? `The reactions miss the loads by ${num(imbalanceOf(rr()) * 100, 2)} %.` : `The reactions balance the loads (difference ${num(imbalanceOf(rr()) * 100, 2)} % of the load).`}</p>
        <Eq r={rr()} />
        <h2>Reactions</h2>
        <table class="tbl"><thead><tr><th>Support</th><th class="r">Fx ({unit("force")})</th><th class="r">Fy ({unit("force")})</th><th class="r">M ({unit("moment")})</th></tr></thead><tbody>
          <For each={rr().reactions}>{x => <tr><td>{nodeName(x.node)}</td><td class="r">{fmt("force", clean(x.fx, rr().scale[0])).split(" ")[0]}</td><td class="r">{fmt("force", clean(x.fy, rr().scale[0])).split(" ")[0]}</td><td class="r">{fmt("moment", clean(x.m, rr().scale[1])).split(" ")[0]}</td></tr>}</For></tbody></table>
        <h2>Members</h2>
        <table class="tbl"><thead><tr><th>Member</th><th class="r">Length ({unit("coord")})</th><th class="r">N max / min ({unit("force")})</th><th class="r">|V| max ({unit("force")})</th><th class="r">|M| max ({unit("moment")})</th><th class="r">Deflection max ({unit("len")})</th><th class="r">Stress ({unit("stress")})</th><th class="r">Utilisation</th></tr></thead><tbody>
          <For each={rr().members}>{(mr, i) => <tr class="link" classList={{ on: sel().includes(mr.id) }} onClick={() => { setSel([mr.id]); setView("model"); }}>
            <td>Member {i() + 1} · {m().members.find(x => x.id === mr.id)?.kind}</td><td class="r">{fmt("coord", mr.length).split(" ")[0]}</td>
            <td class="r">{fmt("force", clean(Math.max(...mr.n), big("n"))).split(" ")[0]} / {fmt("force", clean(Math.min(...mr.n), big("n"))).split(" ")[0]}</td>
            <td class="r">{fmt("force", clean(Math.max(...mr.v.map(Math.abs)), big("v"))).split(" ")[0]}</td><td class="r">{fmt("moment", clean(Math.max(...mr.m.map(Math.abs)), big("m"))).split(" ")[0]}</td>
            <td class="r">{fmt("len", Math.max(...mr.d.map(d => Math.hypot(d[0], d[1])))).split(" ")[0]}</td><td class="r">{fmt("stress", mr.stress).split(" ")[0]}</td>
            <td class="r"><span class="upill" style={{ background: utilColour(mr.util) }}>{mr.util ? `${num(mr.util * 100, 3)} %` : "—"}</span></td></tr>}</For></tbody></table>
        <p class="muted small">Stress is |N|/A + |M|/Z, the largest along the member; utilisation is that over the yield strength. Buckling of members in compression is not checked in 0.1.</p>
        <h2>Solver</h2>
        <table class="tbl"><tbody>
          <tr><td>Equations</td><td class="r">{rr().stats.equations.toLocaleString()}</td></tr><tr><td>Stored stiffness entries</td><td class="r">{rr().stats.nonzeros.toLocaleString()}</td></tr>
          <tr><td>Residual ‖Ku − f‖ / ‖f‖</td><td class="r">{rr().stats.residual.toExponential(1)}</td></tr><tr><td>Time</td><td class="r">{num(rr().stats.ms, 3)} ms</td></tr>
          <tr><td>Method</td><td>2D Euler–Bernoulli beam and truss elements; sparse Cholesky with a fill-reducing ordering</td></tr></tbody></table>
      </>}</Show>
    </div>
  );
};
