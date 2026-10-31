// Analysis steps (0.4.0): the dialog that lists them (Solve › Steps), and the load–deflection
// curve of a large-deflection solution (Results › Load–Deflection). The engine runs the steps and
// follows the path (crates/core/src/steps.rs, engine/path.rs, frames/corot.rs).
import { For, Show, createSignal } from "solid-js";
import { Step, StepKind, Which, Curve, num } from "./api";
import { openDialog, act, model, frameRes } from "./store";
import { numIn } from "./panels";
import { Dialog } from "./ui";
import { fmt, unit, toU } from "./units";

const KINDS: [StepKind, string][] = [["linear", "Linear (small deflections)"], ["newton", "Large deflection (P-delta)"], ["arc", "Large deflection, past limit loads"]];
const key = (w: Which | null) => w ? `${w.on}:${w.id}` : "";
const unkey = (k: string): Which | null => { const [on, id] = k.split(":"); return on === "case" || on === "combo" ? { on, id: +id } as Which : null; };

/** Does the model have large-deflection steps? */
export const isNonlinear = () => (model()?.steps ?? []).some(s => s.kind !== "linear");

/** Solve › Steps: the analysis steps, run in order. */
export const stepsDialog = () => openDialog(() => {
  const m = () => model()!;
  const sts = () => m().steps ?? [];
  const set = (s: Step, ch: Partial<Step>) => act({ Set: { item: { Step: { ...s, ...ch } } } });
  const add = (kind: StepKind) => act({ Add: { item: { Step: { id: 0, name: "", kind, load: null, factor: 1, increments: 10, pieces: 10, monitor: null } } } });
  const plane = () => m().analysis === "frame";
  return <Dialog title="Analysis steps" wide help="steps">
    <Show when={sts().length} fallback={<p class="muted small">No steps: the model is solved as one linear step, small deflections, for the load case or combination picked on the Loads tab. Add a large-deflection step to take second-order (P-delta) effects, large rotations and snap-through into account.</p>}>
      <table class="tbl cases steps"><thead><tr><th>Name</th><th>How</th><th>Load</th><th class="r">Factor</th><th class="r">Increments</th><th class="r">Pieces</th><th>Curve follows</th><th /></tr></thead><tbody>
        <For each={sts()}>{s => <tr>
          <td><input value={s.name} onChange={e => e.currentTarget.value.trim() && set(s, { name: e.currentTarget.value.trim() })} /></td>
          <td><select value={s.kind} onChange={e => set(s, { kind: e.currentTarget.value as StepKind })}><For each={KINDS}>{([k, l]) => <option value={k} selected={s.kind === k}>{l}</option>}</For></select></td>
          <td><select value={key(s.load)} onChange={e => set(s, { load: unkey(e.currentTarget.value) })}>
            <option value="" selected={!s.load}>As picked on the Loads tab</option>
            <For each={m().cases ?? []}>{c => <option value={key({ on: "case", id: c.id })} selected={key(s.load) === key({ on: "case", id: c.id })}>{c.name}</option>}</For>
            <For each={m().combos ?? []}>{c => <option value={key({ on: "combo", id: c.id })} selected={key(s.load) === key({ on: "combo", id: c.id })}>{c.name}</option>}</For>
          </select></td>
          <td class="r"><input class="mono fac" value={num(s.factor, 6)} onChange={numIn(v => set(s, { factor: v }), () => num(s.factor, 6))} /></td>
          <td class="r"><input class="mono fac" value={s.increments} onChange={numIn(v => set(s, { increments: Math.round(v) }), () => String(s.increments))} /></td>
          <td class="r"><input class="mono fac" value={s.pieces} title="Pieces each beam is cut into, so it can bend and turn along its length (the first step's count is used)" onChange={numIn(v => set(s, { pieces: Math.round(v) }), () => String(s.pieces))} /></td>
          <td><select value={s.monitor ?? ""} onChange={e => set(s, { monitor: e.currentTarget.value ? +e.currentTarget.value : null })}>
            <option value="" selected={s.monitor === null}>The node that moves most</option>
            <For each={m().nodes}>{(n, i) => <option value={n.id} selected={s.monitor === n.id}>Node {i() + 1}</option>}</For>
          </select></td>
          <td><button class="x" title="Delete this step" onClick={() => act({ Delete: { ids: [s.id] } })}>×</button></td>
        </tr>}</For></tbody></table>
    </Show>
    <p class="ibtns"><button class="btn sm" onClick={() => add("newton")}>Add a large-deflection step</button> <button class="btn sm" onClick={() => add("arc")}>Add a step past limit loads</button> <button class="btn sm" onClick={() => add("linear")}>Add a linear step</button></p>
    <Show when={!plane() && sts().some(s => s.kind !== "linear")}><p class="warn small">Large-deflection steps are for plane frames in this release; 3D frames and parts come later.</p></Show>
    <p class="muted small">Each large-deflection step starts where the one before ended and takes the loads to its own: its case or combination times its factor. <b>Large deflection</b> applies the load in increments, each corrected until the frame is in balance in its deflected shape (second-order and P-delta effects, large rotations). <b>Past limit loads</b> follows the path by arc length, so it goes on past the largest load a frame can carry (snap-through) and shows the load falling. Loads keep their directions as the frame deflects.</p>
  </Dialog>;
});

/** Results › Load–Deflection: the curve of the node a step follows. */
export const pathDialog = () => openDialog(() => {
  const [comp, setComp] = createSignal<"uy" | "ux" | "rz">("uy");
  const c = (): Curve | undefined => frameRes()?.path;
  const nodeName = (id: number) => `Node ${(model()?.nodes.findIndex(n => n.id === id) ?? -1) + 1}`;
  const W = 520, H = 300, P = 44;
  const plot = () => {
    const cv = c(); if (!cv) return null;
    const q = comp() === "rz" ? "rad" : "len";
    const xs = cv[comp()].map(v => toU(q, v)), ys = cv.load;
    const [x0, x1] = [Math.min(0, ...xs), Math.max(0, ...xs)], [y0, y1] = [Math.min(0, ...ys), Math.max(0, ...ys)];
    const sx = (v: number) => P + (v - x0) / ((x1 - x0) || 1) * (W - 2 * P), sy = (v: number) => H - P - (v - y0) / ((y1 - y0) || 1) * (H - 2 * P);
    const d = xs.map((x, i) => `${i ? "L" : "M"}${sx(x).toFixed(1)} ${sy(ys[i]).toFixed(1)}`).join("");
    return { d, sx, sy, xs, ys, x0, x1, y0, y1, q };
  };
  return <Dialog title="Load–deflection curve" wide help="steps">
    <Show when={c()} fallback={<p class="muted">No curve: add a large-deflection step (Solve › Steps) and solve.</p>}>{cv => <>
      <p class="small">Following {nodeName(cv().node)}: {cv().load.length - 1} increments, {cv().iterations} iterations. The first limit (largest load before it falls): <b>{num(cv().peak, 5)}</b> times the load{(model()?.steps?.length ?? 0) > 1 ? `, in step ${cv().peak_step}` : ""}.</p>
      <p class="ibtns"><For each={[["uy", "Movement in y"], ["ux", "Movement in x"], ["rz", "Rotation"]] as const}>{([k, l]) => <button class="btn sm" classList={{ on: comp() === k }} onClick={() => setComp(k)}>{l}</button>}</For></p>
      <Show when={plot()}>{p => <svg class="curve" viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label="Load against movement">
        <line class="ax" x1={P} y1={p().sy(0)} x2={W - P} y2={p().sy(0)} /><line class="ax" x1={p().sx(0)} y1={P} x2={p().sx(0)} y2={H - P} />
        <path class="line" d={p().d} fill="none" />
        <For each={p().xs}>{(x, i) => <circle class="pt" classList={{ unstable: !cv().stable[i()] }} cx={p().sx(x)} cy={p().sy(p().ys[i()])} r="2.5"><title>{`Load × ${num(p().ys[i()], 5)}, ${num(x, 4)} ${unit(p().q as any)}`}</title></circle>}</For>
        <text x={W - P} y={H - P + 28} text-anchor="end" class="lbl">{comp() === "rz" ? "Rotation" : comp() === "ux" ? "Movement in x" : "Movement in y"} ({unit(p().q as any)})</text>
        <text x={P - 6} y={P - 10} class="lbl">Load factor</text>
        <text x={P - 6} y={p().sy(p().y1) + 4} text-anchor="end" class="lbl">{num(p().y1, 3)}</text>
        <Show when={p().y0 < 0}><text x={P - 6} y={p().sy(p().y0) + 4} text-anchor="end" class="lbl">{num(p().y0, 3)}</text></Show>
        <text x={p().sx(p().x1)} y={H - P + 14} text-anchor="end" class="lbl">{num(p().x1, 3)}</text>
        <Show when={p().x0 < 0}><text x={p().sx(p().x0)} y={H - P + 14} class="lbl">{num(p().x0, 3)}</text></Show>
      </svg>}</Show>
      <p class="muted small">Hollow points are where the frame is not stable (its stiffness is not positive): past a limit load, on the way down. The load factor is how many times the step's case or combination is on the frame. Last point: {fmt("len", Math.hypot(cv().ux[cv().ux.length - 1], cv().uy[cv().uy.length - 1]))} from where the node started.</p>
    </>}</Show>
  </Dialog>;
});
