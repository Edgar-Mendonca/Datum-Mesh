// Natural frequencies (0.2.2): the list of modes under the canvas, the chosen mode drawn on the
// model (members bent along their mode shape; parts deformed and coloured by how far they move),
// moving back and forth if wanted, and the table of frequencies. The engine finds the modes.
import { For, Show, createEffect, onCleanup } from "solid-js";
import { num } from "./api";
import { modes, modeSel, setModeSel, modeOn, setModeOn, animate, setAnimate, phase, setPhase, model, modesStale, show, setShow, view } from "./store";
import { toS } from "./canvas";
import { Icon } from "./ui";

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
  const md = () => { const r = modes(); return r?.kind === "frame" && modeOn() && view() !== "mesh" ? r.modes[modeSel()] : null; };
  const path = (id: number) => {
    const m = model(), mo = md(); if (!m || !mo) return "";
    const mb = m.members.find(x => x.id === id), sh = mo.members.find(x => x.id === id); if (!mb || !sh) return "";
    const a = m.nodes.find(n => n.id === mb.a)!, b = m.nodes.find(n => n.id === mb.b)!, k = modeAmp() * phase(), n = sh.d.length - 1;
    return sh.d.map((d, i) => { const s = i / n, p = toS([a.x + (b.x - a.x) * s + d[0] * k, a.y + (b.y - a.y) * s + d[1] * k]); return `${i ? "L" : "M"}${p[0]} ${p[1]}`; }).join("");
  };
  return <Show when={md()}><g class="modes"><For each={model()?.members ?? []}>{mb => <path d={path(mb.id)} class="modeline" />}</For></g></Show>;
};

/** Under the canvas while modes are shown: pick a mode, let it move, change the swing. */
export const ModesBar = () => {
  const r = () => modes()!;
  return (
    <div class="legend">
      <Show when={modesStale()}><div class="stalebar"><Icon n="info" /> The model changed since its frequencies were found. <button class="btn sm pri" onClick={() => import("./cmds").then(c => c.C.modes())}>Find them again</button></div></Show>
      <div class="lrow"><b>Natural frequencies</b>
        <span class="modelist"><For each={r().modes}>{(m, i) => <button classList={{ on: modeSel() === i() }} onClick={() => setModeSel(i())} title={`Mode ${i() + 1}: ${num(m.f, 5)} Hz, a period of ${num(1000 / m.f, 4)} ms`}>{i() + 1}: {num(m.f, 4)} Hz</button>}</For></span></div>
      <div class="lrow small">
        <label class="chk"><input type="checkbox" checked={animate()} onChange={e => setAnimate(e.currentTarget.checked)} />Moving</label>
        <span class="muted">swing</span><input type="range" min="0.2" max="3" step="0.1" value={show().scale} onInput={e => setShow({ ...show(), scale: +e.currentTarget.value })} />
        <span class="muted">Mode shapes have no size of their own: they are drawn to a tenth of the model. {r().equations.toLocaleString()} equations, {r().iterations} iterations, {num(r().ms, 3)} ms.</span>
        <span class="grow" /><button class="btn sm" onClick={() => setModeOn(false)}>Back to the static results</button>
      </div>
    </div>
  );
};

/** The table of frequencies, for the Results page and the report. */
export const ModesTable = () => (
  <Show when={modes()}>{r => <>
    <h2>Natural frequencies</h2>
    <table class="tbl"><thead><tr><th>Mode</th><th class="r">Frequency (Hz)</th><th class="r">Period (ms)</th><th class="r">ω (rad/s)</th></tr></thead><tbody>
      <For each={r().modes}>{(m, i) => <tr class="link" onClick={() => { setModeSel(i()); setModeOn(true); import("./store").then(s => s.setView("model")); }}><td>{i() + 1}</td><td class="r">{num(m.f, 5)}</td><td class="r">{num(1000 / m.f, 4)}</td><td class="r">{num(2 * Math.PI * m.f, 5)}</td></tr>}</For></tbody></table>
    <p class="muted small">Consistent masses from each material's density; supports held. {r().kind === "frame" ? "Beams bend and stretch; truss bars stretch and carry their mass along." : "Every element of the mesh vibrates in its plane."} Click a row to see the mode.</p>
  </>}</Show>
);
