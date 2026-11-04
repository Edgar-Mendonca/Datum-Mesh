// Playing results (0.6.4), loaded with the results. A bar over the drawing plays the solution:
// a linear static result grows from no load to the full load; a nonlinear one goes through its
// increments, a time history through its times (both from the engine's frames), and a mode swings.
// Play, pause, a slider to any moment, the speed and looping. The results are not copied into the
// drawing code: the player hands the store a view of them at its position (`setPlayView`), and the
// canvas and the 3D view draw that as they draw any result.
import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import { Results, num } from "./api";
import { rawResults, setPlayView, modeOn, modes, setPhase, animate, setAnimate, stale } from "./store";
import { fmt } from "./units";
import { Icon } from "./ui";

type Frames = { what: "load" | "time"; field: string; at: number[]; disp: [number, number, number][][]; v: number[][] };
type Any = any; // eslint-disable-line @typescript-eslint/no-explicit-any

export const [playing, setPlaying] = createSignal(false);
/** Where the playing is: 0 (the start) to 1 (the end, the result as solved). */
export const [pos, setPos] = createSignal(1);
export const [speed, setSpeed] = createSignal(1);
export const [loop, setLoop] = createSignal(true);

const framesOf = (r: Results | null): Frames | null => { const f = (r as Any)?.frames as Frames | undefined; return f && f.at.length > 1 ? f : null; };
/** Can this result be played? (Heat is not played: it does not move.) */
export const playable = (r: Results | null) => !!r && r.kind !== "heat" && r.kind !== "solidheat";

/** A range fixed for a field's colours while playing, so the colours grow with the load. */
const fixed = (v: number[], range: [number, number]) => { (v as Any).range = range; return v; };
const scaleField = (f: Any, s: number) => (f.key === "sf" ? f : { ...f, v: fixed(f.v.map((x: number) => x * s), [f.min, f.max]) });

/** A linear result at a share s of its load: everything in proportion. */
const scaled = (r: Results, s: number): Results => {
  const R = r as Any;
  switch (r.kind) {
    case "solid": return { ...R, disp: R.disp.map((d: number[]) => d.map(x => x * s)), fields: R.fields.map((f: Any) => scaleField(f, s)) };
    case "plane": return { ...R, disp: R.disp.map((d: number[]) => d.map(x => x * s)), fields: R.fields.map((f: Any) => scaleField(f, s)), elem: R.elem.map((f: Any) => scaleField(f, s)) };
    case "frame": return { ...R, disp: R.disp.map((d: number[]) => d.map(x => x * s)), members: R.members.map((mr: Any) => ({ ...mr, d: mr.d.map((p: number[]) => p.map(x => x * s)), n: mr.n.map((x: number) => x * s), v: mr.v.map((x: number) => x * s), m: mr.m.map((x: number) => x * s) })) };
    case "space": return { ...R, disp: R.disp.map((d: number[]) => d.map(x => x * s)), members: R.members.map((mr: Any) => ({ ...mr, d: mr.d.map((p: number[]) => p.map(x => x * s)), n: mr.n.map((x: number) => x * s), vy: mr.vy.map((x: number) => x * s), vz: mr.vz.map((x: number) => x * s), t: mr.t.map((x: number) => x * s), my: mr.my.map((x: number) => x * s), mz: mr.mz.map((x: number) => x * s) })), plates: R.plates ? { ...R.plates, disp: R.plates.disp.map((d: number[]) => d.map(x => x * s)), fields: R.plates.fields.map((f: Any) => scaleField(f, s)) } : R.plates };
    default: return r;
  }
};

/** The largest value of each frame's colour field, over all the frames (for a fixed range). */
const peakOf = new WeakMap<Frames, number>();
const peak = (F: Frames) => { let p = peakOf.get(F); if (p === undefined) { p = 0; for (const v of F.v) for (const x of v) if (x > p) p = x; peakOf.set(F, p); } return p; };

/** A nonlinear result or a history between two of its frames. */
const between = (r: Results, F: Frames, x: number): Results => {
  const k = Math.min(Math.floor(x), F.at.length - 2), t = x - k, a = F.disp[k], b = F.disp[k + 1];
  const d = a.map((p, i) => [p[0] + (b[i][0] - p[0]) * t, p[1] + (b[i][1] - p[1]) * t, p[2] + (b[i][2] - p[2]) * t]);
  const v = F.v.length ? F.v[k].map((p, i) => p + (F.v[k + 1][i] - p) * t) : null;
  const R = r as Any;
  const moved = (f: Any, c: number) => ({ ...f, v: fixed(d.map(p => (c < 3 ? p[c] : Math.hypot(p[0], p[1], p[2]))), [f.min, f.max]) });
  const keyed = (f: Any) => (f.key === "ux" ? moved(f, 0) : f.key === "uy" ? moved(f, 1) : f.key === "uz" ? moved(f, 2) : f.key === "umag" ? moved(f, 3) : f.key === "vm" && v ? { ...f, v: fixed(v, [0, Math.max(f.max, peak(F))]) } : f);
  switch (r.kind) {
    case "solid": return { ...R, disp: d, fields: R.fields.map(keyed) };
    case "plane": return { ...R, disp: d.map(p => [p[0], p[1]]), fields: R.fields.map(keyed) };
    case "frame": case "space": {
      let i = 0;
      const members = R.members.map((mr: Any) => { const n = mr.d.length, s = d.slice(i, i + n); i += n; return { ...mr, d: r.kind === "frame" ? s.map(p => [p[0], p[1]]) : s }; });
      return { ...R, members };
    }
    default: return r;
  }
};

/** The result at position p (0 to 1). */
export const atPos = (r: Results, p: number): Results => {
  if (p >= 1 - 1e-9 || !playable(r)) return r;
  const F = framesOf(r);
  return F ? between(r, F, p * (F.at.length - 1)) : scaled(r, p);
};

/** The value the slider stands at, in words: a share of the load, or a time. */
const label = () => {
  const r = rawResults(), F = framesOf(r), p = pos();
  if (modeOn()) return "the mode swinging";
  if (!F) return `${Math.round(p * 100)} % of the load`;
  const x = p * (F.at.length - 1), k = Math.min(Math.floor(x), F.at.length - 2), at = F.at[k] + (F.at[k + 1] - F.at[k]) * (x - k);
  return F.what === "time" ? `t = ${fmt("time", at)}` : `load factor ${num(at, 3)}`;
};

/** The bar: play or pause, the slider, the speed, looping. */
export const Player = () => {
  setPlayView(() => (r: Results) => atPos(r, pos()));
  onCleanup(() => { setPlayView(null); setPlaying(false); setPos(1); });
  // A new result starts at its end, still.
  createEffect(() => { rawResults(); setPlaying(false); setPos(1); });
  // The clock: a whole run takes 3 s at speed 1 (a mode swings once in 1.2 s).
  createEffect(() => {
    if (!playing()) return;
    let raf = 0, last = performance.now();
    const tick = (t: number) => {
      const dt = (t - last) / 1000; last = t;
      if (modeOn()) { setPos(p => (p + dt * speed() / 1.2) % 1); setPhase(Math.sin(pos() * 2 * Math.PI)); }
      else {
        let p = pos() + dt * speed() / 3;
        if (p >= 1) { if (loop()) p = 0; else { p = 1; setPlaying(false); } }
        setPos(p);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    onCleanup(() => cancelAnimationFrame(raf));
  });
  // Modes keep their own swinging until the player takes over.
  createEffect(() => { if (modeOn() && playing() && animate()) setAnimate(false); });
  const toggle = () => { if (!playing() && pos() >= 1 && !modeOn()) setPos(0); setPlaying(!playing()); };
  return (
    <Show when={(playable(rawResults()) && !stale()) || (modeOn() && modes())}>
      <div class="player" onPointerDown={e => e.stopPropagation()} onPointerUp={e => e.stopPropagation()}>
        <button class="pbtn" title={playing() ? "Pause" : "Play the result: from no load to the whole load, through the increments or the time history"} onClick={toggle}><Icon n={playing() ? "pause" : "play"} /></button>
        <button class="pbtn" title="Back to the start" onClick={() => { setPlaying(false); setPos(0); if (modeOn()) setPhase(0); }}><Icon n="first" /></button>
        <input type="range" min="0" max="1000" value={Math.round(pos() * 1000)} title="Go to any moment" onInput={e => { setPlaying(false); setPos(+e.currentTarget.value / 1000); if (modeOn()) setPhase(Math.sin(pos() * 2 * Math.PI)); }} />
        <span class="plabel mono">{label()}</span>
        <select title="Speed" onChange={e => setSpeed(+e.currentTarget.value)}>{[0.25, 0.5, 1, 2, 4].map(s => <option value={s} selected={s === speed()}>{s}×</option>)}</select>
        <label class="ploop" title="Start again at the end"><input type="checkbox" checked={loop()} onChange={e => setLoop(e.currentTarget.checked)} />Loop</label>
      </div>
    </Show>
  );
};
