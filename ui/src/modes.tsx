// Natural frequencies (0.2.2) and buckling (0.2.3): the list of modes under the canvas, the chosen mode drawn on the
// model (members bent along their mode shape; parts deformed and coloured by how far they move),
// moving back and forth if wanted, and the table of frequencies. The engine finds the modes.
import { For, Show, createEffect, onCleanup } from "solid-js";
import { num, ModeResults } from "./api";
import { fmt } from "./units";
import { modes, modeSel, setModeSel, modeOn, setModeOn, animate, setAnimate, phase, setPhase, model, modesStale, show, setShow, view } from "./store";
import { toS } from "./canvas";
import { Icon } from "./ui";
import { itemName } from "./panels";

/** How far the drawn shape swings: a tenth of the model's size at full swing, times the slider. */
export const modeAmp = () => {
  const m = model(); if (!m) return 0;
  const xs = [...m.nodes.map(n => n.x), ...m.shapes.flatMap(s => s.geom.kind === "circle" ? [s.geom.c[0] - s.geom.r, s.geom.c[0] + s.geom.r] : s.geom.pts.map(p => p[0]))];
  const ys = [...m.nodes.map(n => n.y), ...m.shapes.flatMap(s => s.geom.kind === "circle" ? [s.geom.c[1] - s.geom.r, s.geom.c[1] + s.geom.r] : s.geom.pts.map(p => p[1]))];
  const size = Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys), 1e-9);
  return 0.1 * size * show().scale;
};

/** Swing the mode back and forth (about 1.2 s a cycle) while it is shown and moving. */
export const ModeAnimator = () => {
  createEffect(() => {
    if (!modeOn() || !animate()) { setPhase(1); return; }
    let raf = 0;
    const t0 = performance.now();
    const tick = (t: number) => { setPhase(Math.sin((t - t0) / 1200 * 2 * Math.PI)); raf = requestAnimationFrame(tick); };
    raf = requestAnimationFrame(tick);
    onCleanup(() => cancelAnimationFrame(raf));
  });
  return null;
};

/** A frame's mode: every member drawn along its mode shape. */
export const FrameModeLayer = () => {
  const md = () => { const r = modes(); return (r?.kind === "frame" || r?.kind === "buckle") && modeOn() && view() !== "mesh" ? r.modes[modeSel()] : null; };
  const path = (id: number) => {
    const m = model(), mo = md(); if (!m || !mo) return "";
    const mb = m.members.find(x => x.id === id), sh = mo.members.find(x => x.id === id); if (!mb || !sh) return "";
    const a = m.nodes.find(n => n.id === mb.a)!, b = m.nodes.find(n => n.id === mb.b)!, k = modeAmp() * phase(), n = sh.d.length - 1;
    return sh.d.map((d, i) => { const s = i / n, p = toS([a.x + (b.x - a.x) * s + d[0] * k, a.y + (b.y - a.y) * s + d[1] * k]); return `${i ? "L" : "M"}${p[0]} ${p[1]}`; }).join("");
  };
  return <Show when={md()}><g class="modes"><For each={model()?.members ?? []}>{mb => <path d={path(mb.id)} class="modeline" />}</For></g></Show>;
};

/** A mode's label in the list, and its longer title. */
const label = (r: ModeResults, i: number) => r.kind === "buckle" ? `${i + 1}: λ ${num(r.modes[i].lambda, 4)}` : `${i + 1}: ${num(r.modes[i].f, 4)} Hz`;
const title = (r: ModeResults, i: number) => r.kind === "buckle" ? `Mode ${i + 1}: the loads times ${num(r.modes[i].lambda, 5)} buckle the frame` : `Mode ${i + 1}: ${num(r.modes[i].f, 5)} Hz, a period of ${num(1000 / r.modes[i].f, 4)} ms`;

/** Under the canvas while modes are shown: pick a mode, let it move, change the swing. */
export const ModesBar = () => {
  const r = () => modes()!;
  const buckle = () => r().kind === "buckle";
  const again = () => import("./cmds").then(c => buckle() ? c.C.buckling() : c.C.modes());
  return (
    <div class="legend">
      <Show when={modesStale()}><div class="stalebar"><Icon n="info" /> The model changed since {buckle() ? "its buckling was found" : "its frequencies were found"}. <button class="btn sm pri" onClick={again}>Find {buckle() ? "it" : "them"} again</button></div></Show>
      <div class="lrow"><b>{buckle() ? "Buckling load factors" : "Natural frequencies"}</b>
        <span class="modelist"><For each={r().modes}>{(_, i) => <button classList={{ on: modeSel() === i() }} onClick={() => setModeSel(i())} title={title(r(), i())}>{label(r(), i())}</button>}</For></span>
        <Show when={buckle() && r().modes.length === 0}><span class="muted">The frame as a whole does not buckle; see the truss bars on the Results page.</span></Show></div>
      <div class="lrow small">
        <label class="chk"><input type="checkbox" checked={animate()} onChange={e => setAnimate(e.currentTarget.checked)} />Moving</label>
        <span class="muted">swing</span><input type="range" min="0.2" max="3" step="0.1" value={show().scale} onInput={e => setShow({ ...show(), scale: +e.currentTarget.value })} />
        <span class="muted">{buckle() ? "Buckled shapes" : "Mode shapes"} have no size of their own: they are drawn to a tenth of the model. {r().equations.toLocaleString()} equations, {r().iterations} iterations, {num(r().ms, 3)} ms.</span>
        <span class="grow" /><button class="btn sm" onClick={() => setModeOn(false)}>Back to the static results</button>
      </div>
    </div>
  );
};

const showMode = (i: number) => { setModeSel(i); setModeOn(true); import("./store").then(s => s.setView("model")); };

/** The table of frequencies or load factors, for the Results page and the report. */
export const ModesTable = () => (
  <Show when={modes()}>{r => { const x = r(); return x.kind === "buckle" ? <>
    <h2>Buckling</h2>
    <Show when={x.modes.length}><table class="tbl"><thead><tr><th>Mode</th><th class="r">Load factor λ</th></tr></thead><tbody>
      <For each={x.modes}>{(m, i) => <tr class="link" onClick={() => showMode(i())}><td>{i() + 1}</td><td class="r">{num(m.lambda, 5)}</td></tr>}</For></tbody></table></Show>
    <Show when={x.bars.length}><h3>Truss bars in compression, each on its own</h3><table class="tbl"><thead><tr><th>Member</th><th class="r">Axial force</th><th class="r">Buckling load π²EI/L²</th><th class="r">Load factor</th></tr></thead><tbody>
      <For each={[...x.bars].sort((a, b) => a.lambda - b.lambda)}>{b => <tr classList={{ bad: b.lambda < 1 }}><td>{itemName(model()!, b.id)}</td><td class="r">{fmt("force", b.n)}</td><td class="r">{fmt("force", b.ncr)}</td><td class="r">{num(b.lambda, 4)}</td></tr>}</For></tbody></table></Show>
    <For each={x.warnings}>{w => <p class="warn small">{w}</p>}</For>
    <p class="muted small">Every load on the model, self-weight and warming included, times λ buckles it. Linear buckling of a perfect frame in its own plane: imperfections, yielding and buckling out of the plane lower the real load, so keep λ well above 1. Click a row to see the shape.</p>
  </> : <>
    <h2>Natural frequencies</h2>
    <table class="tbl"><thead><tr><th>Mode</th><th class="r">Frequency (Hz)</th><th class="r">Period (ms)</th><th class="r">ω (rad/s)</th></tr></thead><tbody>
      <For each={x.modes}>{(m, i) => <tr class="link" onClick={() => showMode(i())}><td>{i() + 1}</td><td class="r">{num(m.f, 5)}</td><td class="r">{num(1000 / m.f, 4)}</td><td class="r">{num(2 * Math.PI * m.f, 5)}</td></tr>}</For></tbody></table>
    <p class="muted small">Consistent masses from each material's density; supports held. {x.kind === "frame" ? "Beams bend and stretch; truss bars stretch and carry their mass along." : "Every element of the mesh vibrates in its plane."} Click a row to see the mode.</p>
  </>; }}</Show>
);
