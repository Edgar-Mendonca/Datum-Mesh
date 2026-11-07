// Analysis steps (0.4.0): the dialog that lists them (Solve › Steps), and the load–deflection
// curve of a large-deflection solution (Results › Load–Deflection). The engine runs the steps and
// follows the path (crates/core/src/steps.rs, engine/path.rs, frames/corot.rs).
import { For, Show, createSignal } from "solid-js";
import { Step, StepKind, Which, Curve, DynInfo, num, parseNum } from "./api";
import { openDialog, act, model, frameRes, spaceRes, results } from "./store";
import { numIn } from "./panels";
import { Dialog } from "./ui";
import { fmt, unit, toU } from "./units";

const KINDS: [StepKind, string][] = [["linear", "Linear (small deflections)"], ["newton", "Large deflection (P-delta)"], ["arc", "Large deflection, past limit loads"], ["spectrum", "Response spectrum (dynamics)"], ["history", "Time history (dynamics)"]];
const isDyn = (k: StepKind) => k === "spectrum" || k === "history";
/** A table typed as lines of two numbers ("0.5, 2.5"), and back. */
const toTable = (t: string): [number, number][] | null => { const rows = t.split(/\n+/).map(l => l.trim()).filter(Boolean).map(l => l.split(/[\s,;]+/).map(x => parseNum(x))); return rows.every(r => r.length === 2 && r[0] !== null && r[1] !== null) && rows.length ? rows.map(r => [r[0]!, r[1]!]) : null; };
const fromTable = (t: [number, number][]) => t.map(p => `${num(p[0], 6)}, ${num(p[1], 6)}`).join("\n");
/** The dynamic results shown, if any. */
export const dynOf = (): DynInfo | undefined => frameRes()?.dynamic ?? spaceRes()?.dynamic;
const key = (w: Which | null) => w ? `${w.on}:${w.id}` : "";
const unkey = (k: string): Which | null => { const [on, id] = k.split(":"); return on === "case" || on === "combo" ? { on, id: +id } as Which : null; };

/** Does the model have large-deflection steps? */
export const isNonlinear = () => (model()?.steps ?? []).some(s => s.kind !== "linear");

/** Results › Dynamics: a spectrum's modes, or a time history's record of a node. */
export const dynDialog = () => openDialog(() => {
  const [comp, setComp] = createSignal<"ux" | "uy" | "uz">("ux");
  const d = () => dynOf();
  const nodeName = (id: number) => `Node ${(model()?.nodes.findIndex(n => n.id === id) ?? -1) + 1}`;
  const W = 560, H = 260, P = 48;
  const plot = () => {
    const x = d(); if (!x || x.kind !== "history") return null;
    const ys = x[comp()].map(v => toU("len", v)), ts = x.t;
    const [y0, y1] = [Math.min(0, ...ys), Math.max(0, ...ys)], t1 = Math.max(...ts, 1e-9);
    const sx = (t: number) => P + t / t1 * (W - 2 * P), sy = (v: number) => H - P - (v - y0) / ((y1 - y0) || 1) * (H - 2 * P);
    return { d: ys.map((y, i) => `${i ? "L" : "M"}${sx(ts[i]).toFixed(1)} ${sy(y).toFixed(1)}`).join(""), sx, sy, y0, y1, t1 };
  };
  const top = () => { const x = d(), m = model(); if (!x || !m) return []; return x.peak.map((p, i) => [i, p] as [number, number]).sort((a, b) => b[1] - a[1]).slice(0, 8).map(([i, p]) => [m.nodes[i]?.id ?? 0, p] as [number, number]); };
  return <Dialog title="Dynamics" wide help="dynamics">
    <Show when={d()} fallback={<p class="muted">No dynamic results: add a response spectrum or a time history step (Solve › Steps) and solve.</p>}>{x => <>
      <Show when={x().kind === "spectrum"}>
        <p class="small">{x().modes.length} modes, combined by <b>{x().combined}</b>. Together they move <b>{num(x().share * 100, 3)} %</b> of the mass along the shaking ({fmt("mass", x().total_mass)}); aim for 90 % or more.</p>
        <table class="tbl"><thead><tr><th>Mode</th><th class="r">Frequency (Hz)</th><th class="r">Period (s)</th><th class="r">Sa (m/s²)</th><th class="r">Participation Γ</th><th class="r">Effective mass ({unit("mass")})</th><th class="r">Share</th></tr></thead><tbody>
          <For each={x().modes}>{(r, i) => { const tiny = r.share < 1e-6; return <tr><td>{i() + 1}</td><td class="r">{num(r.f, 4)}</td><td class="r">{num(r.period, 4)}</td><td class="r">{num(r.sa, 4)}</td><td class="r">{tiny ? "0" : num(r.gamma, 4)}</td><td class="r">{tiny ? "0" : num(toU("mass", r.mass), 4)}</td><td class="r">{tiny ? "0" : num(r.share * 100, 3)} %</td></tr>; }}</For>
        </tbody></table>
      </Show>
      <Show when={x().kind === "history"}>
        <p class="small">{x().steps.toLocaleString()} steps (Newmark, average acceleration). Following {nodeName(x().node)}; the results shown on the model are at <b>t = {num(x().peak_t, 4)} s</b>, when it moves most.</p>
        <p class="ibtns"><For each={[["ux", "x"], ["uy", "y"], ...(model()?.analysis === "space" ? [["uz", "z"]] : [])] as [("ux" | "uy" | "uz"), string][]}>{([k, l]) => <button class="btn sm" classList={{ on: comp() === k }} onClick={() => setComp(k)}>Movement in {l}</button>}</For></p>
        <Show when={plot()}>{p => <svg class="curve" viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label="Movement against time">
          <line class="ax" x1={P} y1={p().sy(0)} x2={W - P} y2={p().sy(0)} /><line class="ax" x1={P} y1={P} x2={P} y2={H - P} />
          <line class="ax" x1={p().sx(x().peak_t)} y1={P} x2={p().sx(x().peak_t)} y2={H - P} stroke-dasharray="3 3" />
          <path class="line" d={p().d} fill="none" />
          <text x={W - P} y={H - P + 28} text-anchor="end" class="lbl">Time (s)</text><text x={W - P} y={H - P + 14} text-anchor="end" class="lbl">{num(p().t1, 3)}</text>
          <text x={P - 6} y={P - 10} class="lbl">Movement ({unit("len")})</text>
          <text x={P - 6} y={p().sy(p().y1) + 4} text-anchor="end" class="lbl">{num(p().y1, 3)}</text>
          <Show when={p().y0 < 0}><text x={P - 6} y={p().sy(p().y0) + 4} text-anchor="end" class="lbl">{num(p().y0, 3)}</text></Show>
        </svg>}</Show>
        <h4>Largest movements over the whole history</h4>
        <table class="tbl"><thead><tr><th>Node</th><th class="r">Largest movement</th></tr></thead><tbody><For each={top()}>{([id, v]) => <tr><td>{nodeName(id)}</td><td class="r">{fmt("len", v)}</td></tr>}</For></tbody></table>
      </Show>
      <For each={x().notes}>{n => <p class="muted small">{n}</p>}</For>
    </>}</Show>
  </Dialog>;
});

/** A new step of a kind, with the usual settings. */
export const stepOf = (kind: StepKind): Step => ({ id: 0, name: "", kind, load: null, factor: 1, increments: 10, pieces: isDyn(kind) ? 4 : 10, monitor: null, along: [1, 0, 0], spectrum: [[0, 2.5], [0.5, 2.5], [4, 0.3125]], damping: 0.05, modes: 12, cqc: true, time: kind === "history" ? [[0, 0], [0.1, 1], [0.2, -1], [0.3, 0]] : kind === "explicit" ? [] : [[0, 1], [10, 1]], ground: kind === "history", dt: kind === "explicit" ? 0 : 0.005, duration: kind === "explicit" ? 0.001 : 2 });

/** The analyses each kind of model offers (the Analysis stage, 0.6.0): a step kind, its name, what it is for. */
export const analysesFor = (a: string): [StepKind, string, string][] => {
  const lin: [StepKind, string, string] = ["linear", "Static, linear", "Small movements, elastic materials: most design checks"];
  if (a === "frame") return [lin, ["newton", "Large deflection", "Second-order (P-delta) effects and large rotations, in increments"], ["arc", "Past limit loads", "Follows snap-through past the largest load, by arc length"], ["spectrum", "Earthquake (spectrum)", "Peak response to a design spectrum, modes combined by CQC"], ["history", "Time history", "Response to a record of ground shaking or a load in time"]];
  if (a === "space") return [lin, ["newton", "Large deflection", "Large rotations and second-order effects, in increments (P-delta with plates)"], ["arc", "Past limit loads", "Follows snap-through past the largest load, by arc length (no plates)"], ["spectrum", "Earthquake (spectrum)", "Peak response to a design spectrum, modes combined by CQC"], ["history", "Time history", "Response to a record of ground shaking or a load in time"]];
  if (a === "solid") return [lin, ["newton", "Elastic-plastic", "Yielding (von Mises) up to collapse, in increments"], ["largeplastic", "Crushing and collapse", "Yielding with large movements: a tube crushed, a part bent until it folds (meshed solids)"], ["large", "Large deformation", "Rubber, and parts that move or stretch far"], ["explicit", "Impact (explicit dynamics)", "Fast events in time: a sudden load, or a solid hitting at speed, stepped in time (meshed solids)"]];
  if (["stress", "strain", "axi"].includes(a)) return [lin, ["newton", "Elastic-plastic", "Yielding (von Mises) up to collapse, in increments"]];
  return [];
};
/** The analysis the model is set to (the last step's kind; linear without steps). */
export const curAnalysis = (): StepKind => { const st = model()?.steps ?? []; return st.length ? st[st.length - 1].kind : "linear"; };
/** Set the analysis: one step of that kind (none for static, linear), keeping a step's load and factor. */
export const setAnalysis = (kind: StepKind) => {
  const m = model(); if (!m || curAnalysis() === kind && (m.steps ?? []).length <= 1) return;
  const st = m.steps ?? [], keep = st[st.length - 1];
  act({ Batch: [...(st.length ? [{ Delete: { ids: st.map(s => s.id) } }] : []), ...(kind === "linear" ? [] : [{ Add: { item: { Step: { ...stepOf(kind), load: keep?.load ?? null, factor: keep?.factor ?? 1 } } } }])] });
};

/** Solve › Steps: the analysis steps, run in order. */
export const stepsDialog = () => openDialog(() => {
  const m = () => model()!;
  const sts = () => m().steps ?? [];
  const set = (s: Step, ch: Partial<Step>) => act({ Set: { item: { Step: { ...s, ...ch } } } });
  const add = (kind: StepKind) => act({ Add: { item: { Step: stepOf(kind) } } });
  const plane = () => m().analysis === "frame";
  /** Parts (0.5.0) and 3D solids (0.5.1): a nonlinear step is elastic-plastic; no arc length or dynamics yet. */
  const part = () => ["stress", "strain", "axi", "solid"].includes(m().analysis);
  const solid = () => m().analysis === "solid";
  const kinds = (): [StepKind, string][] => part() ? [["linear", "Linear (elastic)"], ["newton", "Elastic-plastic (yielding, in increments)"], ...(solid() ? [["largeplastic", "Crushing and collapse (yielding with large movements)"] as [StepKind, string], ["large", "Large deformation (rubber, large movements)"] as [StepKind, string], ["explicit", "Impact (explicit dynamics)"] as [StepKind, string]] : [])] : KINDS;
  return <Dialog title="Analysis steps" wide help="steps">
    <Show when={sts().length} fallback={<p class="muted small">No steps: the model is solved as one linear step, small deflections, for the load case or combination picked on the Loads tab. Add a large-deflection step to take second-order (P-delta) effects, large rotations and snap-through into account.</p>}>
      <table class="tbl cases steps"><thead><tr><th>Name</th><th>How</th><th>Load</th><th class="r">Factor</th><th class="r">Increments</th><th class="r">Pieces</th><th>Curve follows</th><th /></tr></thead><tbody>
        <For each={sts()}>{s => <tr>
          <td><input value={s.name} onChange={e => e.currentTarget.value.trim() && set(s, { name: e.currentTarget.value.trim() })} /></td>
          <td><select value={s.kind} onChange={e => { const k = e.currentTarget.value as StepKind, d = stepOf(k); set(s, (k === "explicit") !== (s.kind === "explicit") ? { kind: k, duration: d.duration, dt: d.dt, time: d.time } : { kind: k }); }}><For each={kinds()}>{([k, l]) => <option value={k} selected={s.kind === k}>{l}</option>}</For></select></td>
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
    <For each={sts().filter(s => s.kind === "explicit")}>{s => <div class="dynset">
      <h4>{s.name}: explicit dynamics</h4>
      <div class="dyngrid">
        <label>Length<input class="mono fac" value={num(s.duration, 6)} onChange={numIn(v => v > 0 && set(s, { duration: v }), () => num(s.duration, 6))} /><span class="unit">s</span></label>
        <label>Time step<input class="mono fac" value={num(s.dt, 6)} title="0: the largest stable step, found from the mesh (a step larger than that is cut to it)" onChange={numIn(v => v >= 0 && set(s, { dt: v }), () => num(s.dt, 6))} /><span class="unit">s (0: the largest stable)</span></label>
      </div>
      <label class="tbl2">Factor on the step's load against time (s): one “time, factor” a line, straight between; empty: the load on at once and held<textarea class="mono" rows={4} value={fromTable(s.time)} onChange={e => { const v = e.currentTarget.value.trim(); if (!v) { set(s, { time: [] }); return; } const t = toTable(v); if (t) set(s, { time: t }); else e.currentTarget.value = fromTable(s.time); }} /></label>
      <p class="muted small">Stepped in time by central differences with lumped masses: no equations to solve each step, but the step must be shorter than the time a wave takes to cross the smallest element (a few microseconds in steel), so keep events short (milliseconds). Give solids a starting velocity with Supports & Loads › Velocity. The results shown are at the moment the solid moves most; Results › In Time graphs the movement and the energies.</p>
    </div>}</For>
    <For each={sts().filter(s => isDyn(s.kind))}>{s => <div class="dynset">
      <h4>{s.name}: {s.kind === "spectrum" ? "response spectrum" : "time history"}</h4>
      <div class="dyngrid">
        <label>Shaking along<select value={s.along.findIndex(v => v !== 0)} onChange={e => set(s, { along: [0, 1, 2].map(k => k === +e.currentTarget.value ? 1 : 0) as [number, number, number] })}><For each={[["0", "x"], ["1", "y"], ...(m().analysis === "space" ? [["2", "z"]] : [])]}>{([k, l]) => <option value={k} selected={String(s.along.findIndex(v => v !== 0)) === k}>{l}</option>}</For></select></label>
        <label>Damping<input class="mono fac" value={num(s.damping * 100, 4)} onChange={numIn(v => v >= 0 && v < 100 && set(s, { damping: v / 100 }), () => num(s.damping * 100, 4))} /><span class="unit">%</span></label>
        <Show when={s.kind === "spectrum"}>
          <label>Modes<input class="mono fac" value={s.modes} onChange={numIn(v => set(s, { modes: Math.round(v) }), () => String(s.modes))} /></label>
          <label>Combined by<select value={s.cqc ? "cqc" : "srss"} onChange={e => set(s, { cqc: e.currentTarget.value === "cqc" })}><option value="cqc" selected={s.cqc}>CQC</option><option value="srss" selected={!s.cqc}>SRSS</option></select></label>
        </Show>
        <Show when={s.kind === "history"}>
          <label class="chk"><input type="checkbox" checked={s.ground} onChange={e => set(s, { ground: e.currentTarget.checked })} />The ground shakes (the record is its acceleration)</label>
          <label>Time step<input class="mono fac" value={num(s.dt, 6)} onChange={numIn(v => v > 0 && set(s, { dt: v }), () => num(s.dt, 6))} /><span class="unit">s</span></label>
          <label>Length<input class="mono fac" value={num(s.duration, 6)} onChange={numIn(v => v > 0 && set(s, { duration: v }), () => num(s.duration, 6))} /><span class="unit">s</span></label>
        </Show>
      </div>
      <Show when={s.kind === "spectrum"} fallback={<label class="tbl2">{s.ground ? "Ground acceleration (m/s²) against time (s)" : "Factor on the step's load against time (s)"}: one “time, value” a line, straight between<textarea class="mono" rows={5} value={fromTable(s.time)} onChange={e => { const t = toTable(e.currentTarget.value); if (t) set(s, { time: t }); else e.currentTarget.value = fromTable(s.time); }} /></label>}>
        <label class="tbl2">Spectral acceleration (m/s²) against period (s): one “period, acceleration” a line, straight between, flat beyond the ends<textarea class="mono" rows={5} value={fromTable(s.spectrum)} onChange={e => { const t = toTable(e.currentTarget.value); if (t) set(s, { spectrum: t }); else e.currentTarget.value = fromTable(s.spectrum); }} /></label>
      </Show>
    </div>}</For>
    <Show when={part()}><p class="ibtns"><button class="btn sm" onClick={() => add("newton")}>Add an elastic-plastic step</button> <Show when={solid()}><button class="btn sm" onClick={() => add("largeplastic")}>Add a crushing step</button> <button class="btn sm" onClick={() => add("large")}>Add a large-deformation step</button> <button class="btn sm" onClick={() => add("explicit")}>Add an impact step</button> </Show><button class="btn sm" onClick={() => add("linear")}>Add a linear step</button></p>
      <Show when={solid()}><p class="muted small">A large-deformation step follows the solid through large movements and strains: every material is a neo-Hookean solid with its own E and ν (rubber: a few MPa, ν 0.49 or so; steel stays elastic, however far it bends). The loads keep their size and direction as it moves. The shape is drawn true to size.</p></Show>
      <p class="muted small">An elastic-plastic step puts the load on in increments; where the von Mises stress reaches a material's yield strength it yields, hardening as its Hardening says (0 for perfectly plastic). If the part cannot carry the whole load it collapses, and the solve says at what share of it. Small strains.</p></Show>
    <Show when={!part()}><p class="ibtns"><button class="btn sm" onClick={() => add("newton")}>Add a large-deflection step</button> <button class="btn sm" onClick={() => add("arc")}>Add a step past limit loads</button> <button class="btn sm" onClick={() => add("linear")}>Add a linear step</button> <button class="btn sm" onClick={() => add("spectrum")}>Add a response spectrum</button> <button class="btn sm" onClick={() => add("history")}>Add a time history</button></p></Show>
    <Show when={sts().some(s => isDyn(s.kind)) && sts().length > 1}><p class="warn small">A dynamic step stands on its own: make it the only step to solve it.</p></Show>
    <Show when={m().analysis === "space" && sts().some(s => s.kind === "newton" || s.kind === "arc")}><p class="muted small">{(m().shapes ?? []).some(s => s.plate) ? "With plates, a large-deflection step in a 3D frame is second order (P-delta): the axial forces' geometric stiffness, found again until it settles. Arc length and large rotations are for 3D frames without plates." : "In a 3D frame a large-deflection step follows large rotations in space: every beam is cut into pieces that turn as rigid bodies and bend a little on their own (corotational). Twist, bending both ways and their coupling are all taken in."}</p></Show>
    <Show when={!plane() && !part() && m().analysis !== "space" && sts().some(s => s.kind === "newton" || s.kind === "arc")}><p class="warn small">Large-deflection steps are for frames; parts come later.</p></Show>
    <p class="muted small">Each large-deflection step starts where the one before ended and takes the loads to its own: its case or combination times its factor. <b>Large deflection</b> applies the load in increments, each corrected until the frame is in balance in its deflected shape (second-order and P-delta effects, large rotations). <b>Past limit loads</b> follows the path by arc length, so it goes on past the largest load a frame can carry (snap-through) and shows the load falling. Loads keep their directions as the frame deflects.</p>
  </Dialog>;
});

/** Results › Load–Deflection for a part (0.5.0): the load factor against the largest movement,
 * and the largest plastic strain. */
export const partPathDialog = () => openDialog(() => {
  const p = () => { const r = results(); return r?.kind === "plane" || r?.kind === "solid" ? r.path : undefined; };
  const W = 520, H = 280, P = 48;
  const plot = () => {
    const x = p(); if (!x) return null;
    // Faces moved by set amounts (0.7.4): the force they take against their movement.
    const xs = x.umax.map(v => toU("len", v)), ys = x.force?.length ? x.force.map(v => toU("force", v)) : x.load;
    const x1 = Math.max(...xs, 1e-12), y1 = Math.max(...ys, 1e-12);
    const sx = (v: number) => P + v / x1 * (W - 2 * P), sy = (v: number) => H - P - v / y1 * (H - 2 * P);
    return { d: xs.map((v, i) => `${i ? "L" : "M"}${sx(v).toFixed(1)} ${sy(ys[i]).toFixed(1)}`).join(""), sx, sy, x1, y1, xs, ys };
  };
  return <Dialog title="Load–deflection curve" wide help={(results() as any)?.path?.force?.length ? "crushing" : "plasticity"}>
    <Show when={p()} fallback={<p class="muted">No curve: add an elastic-plastic step (Solve › Steps) and solve.</p>}>{x => <>
      <Show when={x().force?.length} fallback={<p class="small">{x().load.length - 1} increments, {x().iterations} Newton iterations. {(results() as any)?.large ? "Large deformation (neo-Hookean)" : x().first_yield !== null ? <>It first yields at <b>{num(x().first_yield! * 100, 3)} %</b> of the load</> : "Nothing yields"}{x().reached ? "; it carries the whole load." : <>; it <b>collapses</b> at about {num(x().load[x().load.length - 1] * 100, 3)} % of it.</>}</p>}>{(() => {
        // Crushing (0.7.3): the force on the moved face, its peak and where it ends.
        const f = x().force!, i = f.reduce((b, v, k) => v > f[b] ? k : b, 0), last = f[f.length - 1];
        return <p class="small">{x().load.length - 1} increments, {x().iterations} Newton iterations. Yielding with large movements: the force peaks at <b>{num(toU("force", f[i]), 4)} {unit("force")}</b> at {num(toU("len", x().umax[i]), 3)} {unit("len")}{last < 0.95 * f[i] ? <> and falls to {num(toU("force", last), 4)} {unit("force")} at {num(toU("len", x().umax[x().umax.length - 1]), 3)} {unit("len")}: it has <b>collapsed</b></> : ""}{x().reached ? "." : <>; the solve stopped at {num(x().load[x().load.length - 1] * 100, 3)} % of the movement.</>}</p>;
      })()}</Show>
      <Show when={plot()}>{q => <svg class="curve" viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label="Load against the largest movement">
        <line class="ax" x1={P} y1={H - P} x2={W - P} y2={H - P} /><line class="ax" x1={P} y1={P} x2={P} y2={H - P} />
        <path class="line" d={q().d} fill="none" />
        <For each={q().xs}>{(v, i) => <circle class="pt" classList={{ unstable: x().peeq[i()] > 0 }} cx={q().sx(v)} cy={q().sy(q().ys[i()])} r="3"><title>{`${x().force?.length ? `${num(q().ys[i()], 4)} ${unit("force")}` : `Load × ${num(q().ys[i()], 4)}`}, ${num(v, 4)} ${unit("len")}, plastic strain ${num(x().peeq[i()], 3)}`}</title></circle>}</For>
        <text x={W - P} y={H - P + 28} text-anchor="end" class="lbl">{x().force?.length ? "Movement of the moved face" : "Largest movement"} ({unit("len")})</text><text x={W - P} y={H - P + 14} text-anchor="end" class="lbl">{num(q().x1, 3)}</text>
        <text x={P - 6} y={P - 10} class="lbl">{x().force?.length ? `Force it takes (${unit("force")})` : "Load factor"}</text><text x={P - 6} y={q().sy(q().y1) + 4} text-anchor="end" class="lbl">{num(q().y1, 3)}</text>
      </svg>}</Show>
      <Show when={x().force?.length}><p class="muted small">The peak is the collapse load: before it the part yields and stiffens less and less; after it, it folds and carries less while the platen keeps moving. Hollow points: the part has yielded somewhere.</p></Show>
      <Show when={!x().force?.length && (results() as any)?.large} fallback={!x().force?.length && <p class="muted small">Hollow points: the part has yielded somewhere. As it yields it softens, and the curve bends over; at collapse it would run flat.</p>}><p class="muted small">A stiffening curve (bending up) is the solid carrying load by stretching, as rubber and slender parts do once they move far; a softening one (bending over) is heading for buckling or snap-through.</p></Show>
    </>}</Show>
  </Dialog>;
});

/** Results › Load–Deflection: the curve of the node a step follows. */
export const pathDialog = () => openDialog(() => {
  const [comp, setComp] = createSignal<"uy" | "ux" | "uz" | "rz">("uy");
  const c = (): Curve | undefined => frameRes()?.path ?? spaceRes()?.path;
  const sp = () => !!spaceRes()?.path;
  const nodeName = (id: number) => `Node ${(model()?.nodes.findIndex(n => n.id === id) ?? -1) + 1}`;
  const W = 520, H = 300, P = 44;
  const plot = () => {
    const cv = c(); if (!cv) return null;
    const q = comp() === "rz" ? "rad" : "len";
    const xs = (cv[comp()] ?? []).map(v => toU(q, v)), ys = cv.load;
    const [x0, x1] = [Math.min(0, ...xs), Math.max(0, ...xs)], [y0, y1] = [Math.min(0, ...ys), Math.max(0, ...ys)];
    const sx = (v: number) => P + (v - x0) / ((x1 - x0) || 1) * (W - 2 * P), sy = (v: number) => H - P - (v - y0) / ((y1 - y0) || 1) * (H - 2 * P);
    const d = xs.map((x, i) => `${i ? "L" : "M"}${sx(x).toFixed(1)} ${sy(ys[i]).toFixed(1)}`).join("");
    return { d, sx, sy, xs, ys, x0, x1, y0, y1, q };
  };
  return <Dialog title="Load–deflection curve" wide help="steps">
    <Show when={c()} fallback={<p class="muted">No curve: add a large-deflection step (Solve › Steps) and solve.</p>}>{cv => <>
      <p class="small">Following {nodeName(cv().node)}: {cv().load.length - 1} increments, {cv().iterations} iterations. The first limit (largest load before it falls): <b>{num(cv().peak, 5)}</b> times the load{(model()?.steps?.length ?? 0) > 1 ? `, in step ${cv().peak_step}` : ""}.</p>
      <p class="ibtns"><For each={(sp() ? [["uy", "Movement in y"], ["ux", "Movement in x"], ["uz", "Movement in z"], ["rz", "Turn"]] : [["uy", "Movement in y"], ["ux", "Movement in x"], ["rz", "Rotation"]]) as ["uy" | "ux" | "uz" | "rz", string][]}>{([k, l]) => <button class="btn sm" classList={{ on: comp() === k }} onClick={() => setComp(k)}>{l}</button>}</For></p>
      <Show when={plot()}>{p => <svg class="curve" viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label="Load against movement">
        <line class="ax" x1={P} y1={p().sy(0)} x2={W - P} y2={p().sy(0)} /><line class="ax" x1={p().sx(0)} y1={P} x2={p().sx(0)} y2={H - P} />
        <path class="line" d={p().d} fill="none" />
        <For each={p().xs}>{(x, i) => <circle class="pt" classList={{ unstable: !cv().stable[i()] }} cx={p().sx(x)} cy={p().sy(p().ys[i()])} r="3"><title>{`Load × ${num(p().ys[i()], 5)}, ${num(x, 4)} ${unit(p().q as any)}`}</title></circle>}</For>
        <text x={W - P} y={H - P + 28} text-anchor="end" class="lbl">{comp() === "rz" ? (sp() ? "Turn (angle of the rotation)" : "Rotation") : `Movement in ${comp().slice(1)}`} ({unit(p().q as any)})</text>
        <text x={P - 6} y={P - 10} class="lbl">Load factor</text>
        <text x={P - 6} y={p().sy(p().y1) + 4} text-anchor="end" class="lbl">{num(p().y1, 3)}</text>
        <Show when={p().y0 < 0}><text x={P - 6} y={p().sy(p().y0) + 4} text-anchor="end" class="lbl">{num(p().y0, 3)}</text></Show>
        <text x={p().sx(p().x1)} y={H - P + 18} text-anchor="end" class="lbl">{num(p().x1, 3)}</text>
        <Show when={p().x0 < 0}><text x={p().sx(p().x0)} y={H - P + 18} class="lbl">{num(p().x0, 3)}</text></Show>
      </svg>}</Show>
      <p class="muted small">Hollow points are where the frame is not stable (its stiffness is not positive): past a limit load, on the way down. The load factor is how many times the step's case or combination is on the frame. Last point: {fmt("len", Math.hypot(cv().ux[cv().ux.length - 1], cv().uy[cv().uy.length - 1], cv().uz?.[cv().uz!.length - 1] ?? 0))} from where the node started.</p>
    </>}</Show>
  </Dialog>;
});
