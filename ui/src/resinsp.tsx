// The right panel after a solve (0.9.1): with nothing selected it shows the results, not the
// model's settings (those fold away below): what was solved, each result's smallest and largest
// value (click one to show it), the balance check, and the ways into the tables, report and graphs.
import { For, Show } from "solid-js";
import { Model, num } from "./api";
import { results, stale, show, setShow, setView, solidRes, partRes, solvedFor } from "./store";
import { fieldsFor, curField } from "./parts";
import { fmt } from "./units";
import { dur } from "./console";

export const ResultsInsp = (p: { m: Model }) => {
  const r = () => results()!;
  const fields = () => {
    const pr = partRes(), sr = solidRes(), a = pr?.kind === "heat" ? "heat" : sr ? sr.kind : p.m.analysis;
    const list = (sr?.fields ?? pr?.fields ?? []) as { key: string; min: number; max: number }[];
    return fieldsFor(a).map(d => ({ d, f: list.find(x => x.key === d.key) })).filter(x => x.f && x.d.key !== "sf");
  };
  const bad = () => (r() as any).imbalance > 1e-6;
  const heat = () => r().kind === "heat" || r().kind === "solidheat";
  const sf = () => { const f = ((solidRes()?.fields ?? partRes()?.fields ?? []) as { key: string; v: number[] }[]).find(x => x.key === "sf"); const pos = f?.v.filter(v => v > 0) ?? []; return pos.length ? Math.min(...pos) : null; };
  return <div class="rinsp">
    <Show when={stale()}><p class="stalebar small">The model changed since this solve: Solve (F5) again for results that match it.</p></Show>
    <p class="small muted">{solvedFor() ? `${solvedFor()} · ` : ""}{r().stats.equations.toLocaleString()} equations · {dur(r().stats.ms)}</p>
    <p class="small" classList={{ okline: !bad(), badline: bad() }}>{heat() ? (bad() ? "Heat in and out do not balance." : "Heat in balances heat out.") : bad() ? "The reactions do not balance the loads." : "The reactions balance the loads."}</p>
    <Show when={fields().length}>
      <h4>Values <span class="muted small">smallest · largest</span></h4>
      <table class="rvals"><tbody><For each={fields()}>{x => <tr classList={{ on: curField().key === x.d.key }} class="link" title={`Show ${x.d.name}`} onClick={() => setShow({ ...show(), field: x.d.key })}>
        <td>{x.d.name}</td><td class="r mono">{fmt(x.d.q, x.f!.min, 3)}</td><td class="r mono">{fmt(x.d.q, x.f!.max, 3)}</td></tr>}</For></tbody></table>
      <Show when={sf() !== null}><p class="small">Smallest safety factor against yield: <b>{num(sf()!, 3)}</b>{sf()! < 1 ? " (it yields)" : ""}</p></Show>
    </Show>
    <div class="ibtns col">
      <button class="btn sm" onClick={() => setView("results")}>Tables and equilibrium</button>
      <Show when={solidRes()}><button class="btn sm" onClick={() => import("./graphs").then(k => k.lineDialog())}>Graph along a line</button></Show>
      <Show when={(solidRes() as any)?.history}><button class="btn sm" onClick={() => import("./graphs").then(k => k.timeDialog())}>Movement and energy in time</button></Show>
      <button class="btn sm" onClick={() => setView("report")}>Report</button>
    </div>
  </div>;
};
