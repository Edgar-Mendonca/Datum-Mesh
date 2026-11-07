// Graphs (0.7.7): a result along a straight line through a solid, sampled inside its elements.
import { Show, createSignal } from "solid-js";
import { openDialog, solidRes } from "./store";
import { curField } from "./parts";
import { Dialog } from "./ui";
import { num } from "./api";
import { toU, unit } from "./units";

type P3 = [number, number, number];
/** Shown with Z up: display (X, Y, Z) is the model's (x, −z, y). */
const toD = (p: P3): P3 => [p[0], -p[2], p[1]], fromD = (p: P3): P3 => [p[0], p[2], -p[1]];

/** The value of field `v` at point `p` (model axes, m), from the corners of the element holding
 *  it (linear between them), or null outside the solid. Exported for the checks. */
export const sampleAt = (nodes: P3[], tets: number[][], v: number[], p: P3): number | null => {
  for (const t of tets) {
    const [a, b, c, d] = t.map(i => nodes[i]);
    const e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]], e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]], e3 = [d[0] - a[0], d[1] - a[1], d[2] - a[2]], q = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    const det = (x: number[], y: number[], z: number[]) => x[0] * (y[1] * z[2] - y[2] * z[1]) - x[1] * (y[0] * z[2] - y[2] * z[0]) + x[2] * (y[0] * z[1] - y[1] * z[0]);
    const D = det(e1, e2, e3); if (Math.abs(D) < 1e-30) continue;
    const l1 = det(q, e2, e3) / D, l2 = det(e1, q, e3) / D, l3 = det(e1, e2, q) / D, l0 = 1 - l1 - l2 - l3, tol = -1e-9;
    if (l0 >= tol && l1 >= tol && l2 >= tol && l3 >= tol) return l0 * v[t[0]] + l1 * v[t[1]] + l2 * v[t[2]] + l3 * v[t[3]];
  }
  return null;
};

export const lineDialog = () => openDialog(() => {
  const r = solidRes(), vol = r?.volume;
  if (!r || !vol) return <Dialog title="Graph along a line" help="results"><p class="muted">Solve a 3D solid first: the graph is drawn through its elements.</p></Dialog>;
  // From one side of the solid to the other through its middle, along its longest side.
  const lo = [0, 1, 2].map(k => Math.min(...vol.nodes.map(p => p[k]))), hi = [0, 1, 2].map(k => Math.max(...vol.nodes.map(p => p[k])));
  const mid = [0, 1, 2].map(k => (lo[k] + hi[k]) / 2) as P3, long = [0, 1, 2].reduce((b, k) => hi[k] - lo[k] > hi[b] - lo[b] ? k : b, 0);
  const a0 = [...mid] as P3, b0 = [...mid] as P3; a0[long] = lo[long]; b0[long] = hi[long];
  const [from, setFrom] = createSignal(toD(a0).map(x => x * 1e3) as P3), [to, setTo] = createSignal(toD(b0).map(x => x * 1e3) as P3);
  const fd = curField(), fi = Math.max(0, r.fields.findIndex(f => f.key === fd.key)), vs = vol.v[fi] ?? [];
  const W = 520, H = 280, P = 52, N = 120;
  const pts = () => {
    const a = fromD(from().map(x => x / 1e3) as P3), b = fromD(to().map(x => x / 1e3) as P3), L = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
    const out: [number, number][] = [];
    for (let i = 0; i <= N; i++) { const s = i / N, p: P3 = [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s, a[2] + (b[2] - a[2]) * s], val = sampleAt(vol.nodes, vol.tets, vs, p); if (val !== null) out.push([s * L, val]); }
    return out;
  };
  const plot = () => {
    const ps = pts(); if (ps.length < 2) return null;
    const xs = ps.map(p => toU("len", p[0])), ys = ps.map(p => toU(fd.q, p[1]));
    const [x1, y0, y1] = [Math.max(...xs, 1e-12), Math.min(0, ...ys), Math.max(0, ...ys)];
    const sx = (v: number) => P + v / x1 * (W - 2 * P), sy = (v: number) => H - P - (v - y0) / ((y1 - y0) || 1) * (H - 2 * P);
    return { d: xs.map((x, i) => `${i ? "L" : "M"}${sx(x).toFixed(1)} ${sy(ys[i]).toFixed(1)}`).join(""), sx, sy, x1, y0, y1, n: ps.length, ys };
  };
  const xyz = (v: () => P3, set: (p: P3) => void) => <span class="xrow">{[0, 1, 2].map(k => <input class="mono" style={{ width: "72px" }} type="number" value={num(v()[k], 4)} onChange={e => { const p = [...v()] as P3; p[k] = +e.currentTarget.value; set(p); }} />)}</span>;
  return <Dialog title={`${fd.name} along a line`} wide help="results" cancel="Close">
    <table class="kv"><tbody>
      <tr><td>From</td><td>{xyz(from, setFrom)}</td><td class="muted">X, Y, Z mm</td></tr>
      <tr><td>To</td><td>{xyz(to, setTo)}</td><td class="muted">X, Y, Z mm</td></tr>
    </tbody></table>
    <Show when={plot()} fallback={<p class="muted">The line does not pass through the solid: move its ends.</p>}>{q => <svg class="curve" viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label={`${fd.name} along the line`}>
      <line class="ax" x1={P} y1={q().sy(0)} x2={W - P} y2={q().sy(0)} /><line class="ax" x1={P} y1={P} x2={P} y2={H - P} />
      <path class="line" d={q().d} fill="none" />
      <text x={W - P} y={H - P + 28} text-anchor="end" class="lbl">Distance along the line ({unit("len")})</text><text x={W - P} y={H - P + 14} text-anchor="end" class="lbl">{num(q().x1, 4)}</text>
      <text x={P - 6} y={P - 12} class="lbl">{fd.short} ({unit(fd.q)})</text>
      <text x={P - 6} y={q().sy(q().y1) + 4} text-anchor="end" class="lbl">{num(q().y1, 3)}</text>
      <Show when={q().y0 < 0}><text x={P - 6} y={q().sy(q().y0) + 4} text-anchor="end" class="lbl">{num(q().y0, 3)}</text></Show>
    </svg>}</Show>
    <p class="muted small">{fd.name} inside the solid at {N + 1} points along the line, from the corners of the element each lies in (straight between them); where the line is outside the solid it is left out. Pick another result in Results to graph it.</p>
  </Dialog>;
});
