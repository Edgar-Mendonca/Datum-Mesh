// The bottom panel (0.9.1): Messages (everything the window said, kept), the Solver monitor (the
// solve as it runs: load increments, Newton iterations and how far from balance each is,
// increments cut back, time reached; Stop) and the Log (the same as plain lines, to copy).
// The engine reports its progress (`datum_core::progress`); the shell sends it as `solver` events.
import { For, Show, Switch, Match, createSignal, createMemo, JSX } from "solid-js";
import { notes, setNotes, note } from "./store";
import { invoke, listen, num } from "./api";

type Ev = { e: "stage"; text: string } | { e: "increment"; lambda: number } | { e: "iteration"; it: number; residual: number }
  | { e: "converged"; lambda: number; iterations: number } | { e: "cut"; lambda: number; why: string }
  | { e: "time"; t: number; of: number; steps: number } | { e: "note"; text: string };
type Inc = { n: number; lambda: number; iterations: number; cuts: number };
export type Run = {
  what: string; t0: number; t1: number; status: "running" | "done" | "stopped" | "failed"; stage: string;
  target: number; lambda: number; it: number; residual: number; resid: number[]; incs: Inc[]; cuts: number; pending: number;
  time: { t: number; of: number; steps: number } | null; stopAsked: boolean;
};

const [run, setRun] = createSignal<Run | null>(null);
export { run };
const [lines, setLines] = createSignal<string[]>([]);
const [tab, setTab] = createSignal<"messages" | "solver" | "log">("messages");
const [open, setOpen] = createSignal(false);
const store = (k: string, v?: string) => { try { if (v === undefined) return localStorage.getItem(k); localStorage.setItem(k, v); } catch { /* private window */ } return null; };
const [height, setHeight] = createSignal(Math.min(480, Math.max(120, +(store("dm.console.h") ?? 200) || 200)));
const [seen, setSeen] = createSignal(0);
const [now, setNow] = createSignal(Date.now());
let cur: Run | null = null, tick = 0;

const pct = (x: number) => `${num(x * 100, 3)} %`;
export const dur = (ms: number) => ms < 1000 ? `${Math.round(ms)} ms` : ms < 60000 ? `${(ms / 1000).toFixed(1)} s` : `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`;
const stamp = () => { const d = new Date(); return [d.getHours(), d.getMinutes(), d.getSeconds()].map(x => String(x).padStart(2, "0")).join(":"); };
/** A line in the Log. */
export const logLine = (text: string) => setLines(v => [...v.slice(-3999), `${stamp()}  ${text}`]);
const push = () => { if (cur) setRun({ ...cur, resid: [...cur.resid], incs: [...cur.incs] }); };

/** A solve (or meshing) starts: the monitor follows it; it opens by itself if it takes a while. */
export const beginRun = (what: string) => {
  cur = { what, t0: Date.now(), t1: 0, status: "running", stage: "", target: 0, lambda: 0, it: 0, residual: NaN, resid: [], incs: [], cuts: 0, pending: 0, time: null, stopAsked: false };
  push(); logLine(`${what}: started`);
  clearInterval(tick); tick = window.setInterval(() => setNow(Date.now()), 250);
  setTimeout(() => { if (cur?.status === "running" && Date.now() - cur.t0 >= 900 && !open()) { setTab("solver"); setOpen(true); } }, 1000);
};
/** The solve ended: done, or failed with `why`. */
export const endRun = (ok: boolean, why = "") => {
  if (!cur) return;
  cur.t1 = Date.now(); cur.status = !ok ? "failed" : cur.stopAsked ? "stopped" : "done";
  clearInterval(tick); setNow(cur.t1);
  logLine(`${cur.what}: ${cur.status === "failed" ? `failed. ${why}` : cur.status === "stopped" ? "stopped at your request" : "finished"} after ${dur(cur.t1 - cur.t0)}`);
  push();
};
export const stopRun = () => {
  if (!cur || cur.status !== "running") return;
  cur.stopAsked = true; push();
  logLine("Stop asked: the solve ends at its next load increment or time step, keeping what it reached.");
  invoke("stop_solve").catch(() => {});
};

const take = (evs: Ev[]) => {
  if (!cur) return;
  for (const e of evs) {
    if (e.e === "stage") { cur.stage = e.text; logLine(e.text); }
    else if (e.e === "increment") { cur.target = e.lambda; cur.it = 0; logLine(`Increment ${cur.incs.length + 1}: towards ${pct(e.lambda)} of the load`); }
    else if (e.e === "iteration") { cur.it = e.it; cur.residual = e.residual; cur.resid.push(e.residual); if (cur.resid.length > 300) cur.resid.splice(0, cur.resid.length - 300); logLine(`  iteration ${e.it}: out of balance ${e.residual.toExponential(2)}`); }
    else if (e.e === "converged") { cur.lambda = e.lambda; cur.incs.push({ n: cur.incs.length + 1, lambda: e.lambda, iterations: e.iterations, cuts: cur.pending }); cur.pending = 0; if (cur.incs.length > 2000) cur.incs.shift(); logLine(`  balanced at ${pct(e.lambda)} in ${e.iterations} ${e.iterations === 1 ? "iteration" : "iterations"}`); }
    else if (e.e === "cut") { cur.cuts++; cur.pending++; logLine(`  cut back: ${e.why}; next try ${pct(e.lambda)}`); }
    else if (e.e === "time") { cur.time = { t: e.t, of: e.of, steps: e.steps }; logLine(`Time ${num(e.t * 1e3, 4)} of ${num(e.of * 1e3, 4)} ms (${e.steps.toLocaleString()} steps)`); }
    else if (e.e === "note") { logLine(e.text); note(e.text); }
  }
  push();
};
listen("solver", (p: Ev[]) => take(p));

/** How far the solve is (0 to 1), or null before it says. */
export const progress = (r: Run) => r.time ? r.time.t / (r.time.of || 1) : r.incs.length || r.target ? r.lambda : null;
const left = (r: Run) => { const p = progress(r); if (r.status !== "running" || p === null || p < 0.02) return ""; const el = now() - r.t0; return dur(el * (1 - p) / p); };
const elapsed = (r: Run) => dur((r.status === "running" ? now() : r.t1) - r.t0);
/** One line for the status bar while solving. */
export const runLine = () => {
  const r = run(); if (!r || r.status !== "running") return "";
  const p = progress(r);
  return [r.stopAsked ? "Stopping" : r.what, r.time ? `${r.time.steps.toLocaleString()} steps` : r.incs.length || r.target ? `increment ${r.incs.length + 1}, iteration ${r.it}` : r.stage, p !== null ? pct(p) : "", elapsed(r)].filter(Boolean).join(" · ");
};

const STATUS: Record<Run["status"], string> = { running: "Solving", done: "Finished", stopped: "Stopped", failed: "Failed" };

const Resid = (p: { r: Run }) => {
  const W = 330, H = 128, L = 40, B = 18, lo = -10, hi = 1;
  const y = (v: number) => { const l = Math.min(hi, Math.max(lo, Math.log10(Math.max(v, 1e-300)))); return 6 + (hi - l) / (hi - lo) * (H - B - 6); };
  const pts = () => { const v = p.r.resid.slice(-150); return v.map((r, i) => `${(L + (v.length > 1 ? i / (v.length - 1) : 0) * (W - L - 6)).toFixed(1)},${y(r).toFixed(1)}`).join(" "); };
  return <svg class="cres" viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label="Out-of-balance force against iteration, log scale">
    <For each={[0, -2, -4, -6, -8, -10]}>{e => <><line x1={L} x2={W - 6} y1={y(10 ** e)} y2={y(10 ** e)} class="grid" /><text x={L - 4} y={y(10 ** e) + 3} text-anchor="end">{e === 0 ? "1" : `1e${e}`}</text></>}</For>
    <line x1={L} x2={W - 6} y1={y(1e-8)} y2={y(1e-8)} class="tol" />
    <polyline points={pts()} class="res" />
    <text x={W - 6} y={H - 4} text-anchor="end">last {Math.min(150, p.r.resid.length)} iterations · dashed: balanced</text>
  </svg>;
};

const Monitor = () => <Show when={run()} fallback={<p class="muted cempty">Nothing solved yet in this session. Solve (F5) and it is followed here as it runs: each load increment, the Newton iterations and how far from balance each one is, increments cut back and why, the time reached in an impact, and how long is left. Stop ends it at the next increment, keeping what it reached.</p>}>{r => {
  const p = () => progress(r());
  return <div class="cmon">
    <table class="kv cmkv"><tbody>
      <tr><td>Status</td><td><b classList={{ ok: r().status === "done", bad: r().status === "failed" }}>{STATUS[r().status]}{r().stopAsked && r().status === "running" ? " (stopping)" : ""}</b> · {r().what}</td></tr>
      <Show when={r().stage}><tr><td>Doing</td><td>{r().stage}</td></tr></Show>
      <Show when={r().time} fallback={<Show when={r().incs.length || r().target}>
        <tr><td>Increment</td><td>{r().status === "running" ? r().incs.length + 1 : r().incs.length}{r().status === "running" ? `, iteration ${r().it}` : ""}</td></tr>
        <tr><td>Load reached</td><td>{pct(r().lambda)}{r().status === "running" ? ` (trying ${pct(r().target)})` : ""}</td></tr>
        <tr><td>Out of balance</td><td class="mono">{isNaN(r().residual) ? "—" : r().residual.toExponential(2)}</td></tr>
        <tr><td>Cut back</td><td>{r().cuts ? `${r().cuts} ${r().cuts === 1 ? "time" : "times"}` : "never"}</td></tr></Show>}>{t =>
        <tr><td>Time</td><td>{num(t().t * 1e3, 4)} of {num(t().of * 1e3, 4)} ms · {t().steps.toLocaleString()} steps</td></tr>}</Show>
      <tr><td>Elapsed</td><td>{elapsed(r())}{left(r()) ? ` · about ${left(r())} left` : ""}</td></tr>
    </tbody></table>
    <Show when={p() !== null}><div class="cbar" title={pct(p()!)}><i style={{ width: `${Math.min(100, p()! * 100)}%` }} /></div></Show>
    <Show when={r().resid.length}><Resid r={r()} /></Show>
    <Show when={r().incs.length}><div class="cinc"><table class="tbl"><thead><tr><th>#</th><th class="r">Load</th><th class="r">Iterations</th><th class="r">Cut back</th></tr></thead><tbody>
      <For each={r().incs.slice(-60).reverse()}>{x => <tr><td>{x.n}</td><td class="r">{pct(x.lambda)}</td><td class="r">{x.iterations}</td><td class="r">{x.cuts || ""}</td></tr>}</For>
    </tbody></table></div></Show>
  </div>;
}}</Show>;

const LEVEL: Record<string, string> = { info: "Info", warn: "Warning", error: "Error" };
const Messages = () => <Show when={notes().length} fallback={<p class="muted cempty">No messages yet. What the window tells you (a solve finished, a warning, an error) is kept here with its time.</p>}>
  <div class="cmsgs"><For each={[...notes()].reverse()}>{n => <div class={`cmsg ${n.level}`}><time>{new Date(n.at).toLocaleTimeString()}</time><b>{LEVEL[n.level]}</b><span>{n.text}</span></div>}</For></div>
</Show>;

const Log = () => <div class="clog"><pre>{lines().join("\n") || "The solver's log shows here when you solve."}</pre></div>;

export const Console = () => {
  const unread = createMemo(() => notes().slice(seen()).filter(n => n.level !== "info").length);
  const show = (t: "messages" | "solver" | "log") => { if (open() && tab() === t) setOpen(false); else { setTab(t); setOpen(true); } if (t === "messages") setSeen(notes().length); };
  const grip = (e: PointerEvent) => {
    const y0 = e.clientY, h0 = height(), el = e.currentTarget as HTMLElement; el.setPointerCapture(e.pointerId);
    const mv = (m: PointerEvent) => setHeight(Math.min(window.innerHeight * 0.6, Math.max(90, h0 + y0 - m.clientY)));
    const up = () => { el.removeEventListener("pointermove", mv); el.removeEventListener("pointerup", up); store("dm.console.h", String(Math.round(height()))); window.dispatchEvent(new Event("resize")); };
    el.addEventListener("pointermove", mv); el.addEventListener("pointerup", up);
  };
  const T = (p: { t: "messages" | "solver" | "log"; children: JSX.Element }) => <button classList={{ on: open() && tab() === p.t }} onClick={() => show(p.t)}>{p.children}</button>;
  return <section class="console" classList={{ open: open() }} style={open() ? { height: `${height()}px` } : {}}>
    <Show when={open()}><div class="cgrip" onPointerDown={grip} title="Drag to make the panel taller or shorter" /></Show>
    <header>
      <T t="messages">Messages<Show when={unread()}><i class="cbadge">{unread()}</i></Show></T>
      <T t="solver">Solver monitor</T>
      <T t="log">Log</T>
      <span class="grow" />
      <Show when={run()}>{r => <span class="cnow"><span classList={{ cdot: true, live: r().status === "running", bad: r().status === "failed" }} />{STATUS[r().status]} · {elapsed(r())}<Show when={progress(r()) !== null}><span class="cbar sm"><i style={{ width: `${Math.min(100, progress(r())! * 100)}%` }} /></span></Show></span>}</Show>
      <Show when={run()?.status === "running"}><button class="btn sm" disabled={run()?.stopAsked} onClick={stopRun} title="End the solve at its next load increment or time step, keeping what it reached">Stop</button></Show>
      <Show when={tab() === "log" && open()}><button class="btn sm" onClick={() => navigator.clipboard?.writeText(lines().join("\n")).then(() => note("The log is on the clipboard."), () => {})}>Copy</button></Show>
      <Show when={tab() === "messages" && open() && notes().length}><button class="btn sm" onClick={() => { setNotes([]); setSeen(0); }}>Clear</button></Show>
      <button class="cchev" onClick={() => { setOpen(!open()); window.dispatchEvent(new Event("resize")); }} title={open() ? "Hide the panel" : "Show the panel"} aria-label={open() ? "Hide the panel" : "Show the panel"}>{open() ? "▾" : "▴"}</button>
    </header>
    <Show when={open()}><div class="conbody"><Switch>
      <Match when={tab() === "messages"}><Messages /></Match>
      <Match when={tab() === "solver"}><Monitor /></Match>
      <Match when={tab() === "log"}><Log /></Match>
    </Switch></div></Show>
  </section>;
};
