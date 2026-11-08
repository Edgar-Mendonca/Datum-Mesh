// Status badges (0.10.3, the specification's rule): what is true of the model, kept apart and in
// words, never one green "safe". Model: ready, or the model check's errors. Solve: not solved,
// solved, stopped short or out of date. Balance: the equilibrium check, or not checked. Design
// checks are not run by Datum Mesh yet, so no badge claims them.
import { Show, createResource, createSignal } from "solid-js";
import { isFrame, num } from "./api";
import { model, results, stale, call } from "./store";

type Finding = { level: string };
/** Bumped when the Model Check runs, so the badge counts the same findings. */
export const [checkTick, setCheckTick] = createSignal(0);

export const StatusBadges = () => {
  const [found] = createResource(() => { const m = model(); checkTick(); return m && isFrame(m.analysis) && m.members.length ? [m, checkTick()] : null; }, () => call<Finding[]>("model_check", { near: 0.005 }).catch(() => [] as Finding[]));
  const errors = () => (found() ?? []).filter(f => f.level === "error").length;
  const res = () => results() as { imbalance?: number; path?: { reached?: boolean; load?: number[] } } | null;
  const solve = (): [string, string, string] => {
    const r = res();
    if (!r) return ["none", "Not solved", "Press Solve (F5)"];
    if (stale()) return ["warn", "Out of date", "The model changed after this solve: solve again"];
    if (r.path && r.path.reached === false) return ["warn", "Stopped short", "The solve did not reach the whole load: see the load–deflection curve"];
    return ["ok", "Solved", "The solver converged"];
  };
  const balance = (): [string, string, string] => {
    const r = res();
    if (!r || stale() || r.imbalance === undefined) return ["none", "Not checked", "Equilibrium is checked after a solve"];
    return r.imbalance < 1e-3 ? ["ok", "Balanced", `Reactions balance the loads to ${num(r.imbalance * 100, 2)} %`] : ["bad", "Out of balance", `Reactions miss the loads by ${num(r.imbalance * 100, 3)} %: the answer is not reliable`];
  };
  return <Show when={model()}><span class="sbadges">
    <span class="sb" data-badge="model" classList={{ ok: !errors(), bad: errors() > 0 }} title={errors() ? "The model check found errors: Analysis › Model Check" : "Nothing the model check counts as an error"}><b>Model</b>{errors() ? `${errors()} error${errors() === 1 ? "" : "s"}` : "Ready"}</span>
    <span class={`sb ${solve()[0]}`} data-badge="solve" title={solve()[2]}><b>Solve</b>{solve()[1]}</span>
    <span class={`sb ${balance()[0]}`} data-badge="balance" title={balance()[2]}><b>Balance</b>{balance()[1]}</span>
  </span></Show>;
};
