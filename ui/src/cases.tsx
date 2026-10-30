// Load cases and combinations (0.3.3): the picker on the Loads tab, the dialog that names cases
// and gives combinations their factors, and the envelope over every combination. Also the steel
// section tables. The engine solves and checks everything (crates/core/src/cases.rs, steel.rs).
import { For, Show, createSignal, createResource } from "solid-js";
import { Envelope, LibSection, Which, num, isFrame } from "./api";
import { openDialog, act, call, model, active, setActive, caseOf } from "./store";
import { numIn } from "./panels";
import { Dialog } from "./ui";
import { fmt, unit, toU, Q } from "./units";

/** A value in display units, without its unit (the table heads say it). */
const bare = (q: Q, v: number, sig: number) => num(toU(q, v), sig);

/** The picker's value for a case or combination ("case:12"), and back. */
const key = (w: Which | null) => w ? `${w.on}:${w.id}` : "";
const unkey = (k: string): Which | null => { const [on, id] = k.split(":"); return on === "case" || on === "combo" ? { on, id: +id } as Which : null; };

/** Loads › Load Cases: which case or combination is drawn and solved. */
export const CasePicker = () => {
  const m = () => model();
  return <Show when={m()?.cases?.length} fallback={<span class="muted small casepick">All loads together</span>}>
    <select class="casepick" title="The load case or combination drawn and solved" value={key(active())} onChange={e => setActive(unkey(e.currentTarget.value))}>
      <optgroup label="Load cases"><For each={m()!.cases ?? []}>{c => <option value={key({ on: "case", id: c.id })} selected={key(active()) === key({ on: "case", id: c.id })}>{c.name}</option>}</For></optgroup>
      <Show when={m()!.combos?.length}><optgroup label="Combinations"><For each={m()!.combos ?? []}>{c => <option value={key({ on: "combo", id: c.id })} selected={key(active()) === key({ on: "combo", id: c.id })}>{c.name}</option>}</For></optgroup></Show>
    </select>
  </Show>;
};

/** Loads › Cases and Combinations: name the cases, give each combination its factors. */
export const casesDialog = () => openDialog(() => {
  const m = () => model()!;
  const cs = () => m().cases ?? [], cbs = () => m().combos ?? [];
  const count = (id: number) => m().loads.filter(l => caseOf(m(), l) === id).length;
  const addCase = async () => {
    const o = await act({ Add: { item: { Case: { id: 0, name: cs().length ? "" : "Dead" } } } });
    if (o?.created.length) setActive({ on: "case", id: o.created[0] });
  };
  const addCombo = async () => {
    const o = await act({ Add: { item: { Combo: { id: 0, name: "", parts: cs().map(c => ({ case: c.id, f: 1 })) } } } });
    if (o?.created.length) setActive({ on: "combo", id: o.created[0] });
  };
  const factor = (cb: any, cid: number) => cb.parts.find((p: any) => p.case === cid)?.f;
  const setFactor = (cb: any, cid: number, f: number | null) => {
    const parts = cb.parts.filter((p: any) => p.case !== cid);
    if (f !== null && f !== 0) parts.push({ case: cid, f });
    parts.sort((a: any, b: any) => cs().findIndex(c => c.id === a.case) - cs().findIndex(c => c.id === b.case));
    act({ Set: { item: { Combo: { ...cb, parts } } } });
  };
  return <Dialog title="Load cases and combinations" wide help="cases">
    <h4>Load cases</h4>
    <Show when={cs().length} fallback={<p class="muted small">Every load is in one case now. Add cases to keep dead, imposed, wind and other loads apart; the loads so far go into the first.</p>}>
      <table class="tbl cases"><thead><tr><th>Name</th><th class="r">Loads</th><th /></tr></thead><tbody>
        <For each={cs()}>{c => <tr classList={{ on: key(active()) === key({ on: "case", id: c.id }) }}>
          <td><input value={c.name} onChange={e => e.currentTarget.value.trim() && act({ Set: { item: { Case: { ...c, name: e.currentTarget.value.trim() } } } })} /></td>
          <td class="r">{count(c.id)}</td>
          <td><button class="btn sm" onClick={() => setActive({ on: "case", id: c.id })}>Show</button> <button class="x" title="Delete this case and its loads" onClick={() => act({ Delete: { ids: [c.id] } })}>×</button></td>
        </tr>}</For></tbody></table>
    </Show>
    <p class="ibtns"><button class="btn sm" onClick={addCase}>Add a load case</button></p>
    <h4>Combinations</h4>
    <Show when={cs().length} fallback={<p class="muted small">Add load cases first; a combination adds them up, each times a factor.</p>}>
      <Show when={cbs().length} fallback={<p class="muted small">No combinations yet. Each one adds the load cases up, each times its factor (an empty factor leaves the case out).</p>}>
        <table class="tbl cases"><thead><tr><th>Name</th><For each={cs()}>{c => <th class="r">{c.name}</th>}</For><th /></tr></thead><tbody>
          <For each={cbs()}>{cb => <tr classList={{ on: key(active()) === key({ on: "combo", id: cb.id }) }}>
            <td><input value={cb.name} onChange={e => e.currentTarget.value.trim() && act({ Set: { item: { Combo: { ...cb, name: e.currentTarget.value.trim() } } } })} /></td>
            <For each={cs()}>{c => <td class="r"><input class="mono fac" value={factor(cb, c.id) ?? ""} placeholder="–" onChange={e => { const t = e.currentTarget.value.trim(); if (!t) return setFactor(cb, c.id, null); numIn(v => setFactor(cb, c.id, v), () => factor(cb, c.id) ?? "")(e); }} /></td>}</For>
            <td><button class="btn sm" onClick={() => setActive({ on: "combo", id: cb.id })}>Show</button> <button class="x" title="Delete this combination" onClick={() => act({ Delete: { ids: [cb.id] } })}>×</button></td>
          </tr>}</For></tbody></table>
      </Show>
      <p class="ibtns"><button class="btn sm" onClick={addCombo}>Add a combination</button></p>
    </Show>
    <p class="muted small">The case or combination picked on the Loads tab is the one drawn and solved; new loads go into the picked case. Factors are yours to choose: Datum Mesh does not apply a design code.</p>
  </Dialog>;
});

/** Results › Envelope: the largest and smallest member forces and reactions over every
 * combination (or every case when there are none). */
export const envelope = () => openDialog(() => {
  const m = model()!;
  const [env] = createResource(() => call<Envelope>("envelope"));
  const sp = m.analysis === "space";
  const name = (id: number) => `Member ${m.members.findIndex(b => b.id === id) + 1}`;
  const nodeName = (id: number) => `Node ${m.nodes.findIndex(n => n.id === id) + 1}`;
  return <Dialog title="Envelope" wide help="cases">
    <Show when={env()} fallback={<p class="muted">{env.loading ? "Solving every combination…" : "No envelope: add load cases (and combinations) on the Loads tab."}</p>}>{e => <>
      <p class="muted small">Over {e().over.length} {(m.combos?.length ?? 0) ? "combinations" : "load cases"}: {e().over.join(", ")}.</p>
      <table class="tbl"><thead><tr><th>Member</th><th class="r">N max / min ({unit("force")})</th><th class="r">|V| ({unit("force")})</th><th class="r">|M| ({unit("moment")})</th><Show when={sp}><th class="r">|T| ({unit("moment")})</th></Show><th class="r">Deflection ({unit("len")})</th><th class="r">Utilisation</th><th>Governed by</th></tr></thead><tbody>
        <For each={e().members}>{r => <tr classList={{ crit: r.util > 1 }}><td>{name(r.id)}</td><td class="r">{bare("force", r.n_max, 4)} / {bare("force", r.n_min, 4)}</td><td class="r">{bare("force", r.v, 4)}</td><td class="r">{bare("moment", r.m, 4)}</td><Show when={sp}><td class="r">{bare("moment", r.t, 4)}</td></Show><td class="r">{bare("len", r.deflection, 4)}</td><td class="r">{num(r.util * 100, 3)} %</td><td>{r.governs}</td></tr>}</For>
      </tbody></table>
      <h4>Reactions, largest and smallest</h4>
      <table class="tbl"><thead><tr><th>Support</th><th class="r">Fx ({unit("force")})</th><th class="r">Fy</th><Show when={sp}><th class="r">Fz</th></Show><th class="r">{sp ? "Mx · My · Mz" : "M"} ({unit("moment")})</th></tr></thead><tbody>
        <For each={e().reactions}>{r => { const mm = (k: number, q: "force" | "moment") => `${bare(q, r.max[k], 3)} / ${bare(q, r.min[k], 3)}`; return <tr><td>{nodeName(r.node)}</td><td class="r">{mm(0, "force")}</td><td class="r">{mm(1, "force")}</td><Show when={sp}><td class="r">{mm(2, "force")}</td></Show><td class="r">{sp ? `${mm(3, "moment")} · ${mm(4, "moment")} · ${mm(5, "moment")}` : mm(5, "moment")}</td></tr>; }}</For>
      </tbody></table>
      <Show when={e().warnings.length}><p class="muted small">{e().warnings.join(" ")}</p></Show>
      <p class="muted small">Utilisation is the largest |N|/A + |M|/Z over the yield strength; buckling of members is not in it (Solve › Buckling, for a combination picked on the Loads tab).</p>
    </>}</Show>
  </Dialog>;
});

/** Is the envelope worth offering (a frame with load cases)? */
export const hasEnvelope = () => { const m = model(); return !!m && isFrame(m.analysis) && !!m.cases?.length; };

/** Sections › From the tables: rolled steel I and H sections (EN 10365), added to the model. */
export const sectionTables = (added?: (id: number) => void) => openDialog(() => {
  const [lib] = createResource(() => call<LibSection[]>("sections_library"));
  const [q, setQ] = createSignal(""), [cur, setCur] = createSignal("IPE 300");
  const shown = () => (lib() ?? []).filter(s => s.name.toLowerCase().replace(/\s/g, "").includes(q().toLowerCase().replace(/\s/g, "")));
  const fams = () => [...new Set(shown().map(s => s.family))];
  const s = () => (lib() ?? []).find(x => x.name === cur());
  const [props] = createResource(() => s()?.shape, sh => call<any>("section_props", { shape: sh }));
  const use = async () => {
    const x = s(), mm = model(); if (!x || !mm) return;
    const have = mm.sections.find(y => y.name === x.name);
    const id = have?.id ?? (await act({ Add: { item: { Section: { id: 0, name: x.name, shape: x.shape } } } }))?.created[0];
    if (id !== undefined) added?.(id);
    return true;
  };
  return <Dialog title="Steel section tables" wide help="sections" ok={use} okText="Add to the model">
    <div class="mgr lib">
      <div class="mlist"><input placeholder="Search: IPE 300, HEB…" value={q()} onInput={e => setQ(e.currentTarget.value)} />
        <Show when={lib()} fallback={<p class="muted">Loading…</p>}>
          <For each={fams()}>{f => <><small class="fam">{f}</small><For each={shown().filter(x => x.family === f)}>{x => <button classList={{ on: x.name === cur() }} onClick={() => setCur(x.name)} onDblClick={() => use()}>{x.name}</button>}</For></>}</For>
        </Show></div>
      <Show when={s()}>{x => { const sh = x().shape as any; return <div class="form">
        <h3>{x().name}</h3><p class="muted small">Rolled I or H section, EN 10365 nominal dimensions</p>
        <table class="vmini"><tbody>
          <tr><td>Depth h · width b</td><td class="r">{fmt("len", sh.h)} · {fmt("len", sh.b)}</td></tr>
          <tr><td>Web tw · flange tf</td><td class="r">{fmt("len", sh.tw)} · {fmt("len", sh.tf)}</td></tr>
          <tr><td>Root radius r</td><td class="r">{fmt("len", sh.r)}</td></tr>
          <Show when={props()}>{p => <>
            <tr><td>Area A</td><td class="r">{fmt("area", p().a)} (table {fmt("area", x().table_area)})</td></tr>
            <tr><td>Second moment I (strong axis)</td><td class="r">{fmt("inertia", p().i)}</td></tr>
            <tr><td>Second moment Iy (weak axis)</td><td class="r">{fmt("inertia", p().iy)}</td></tr>
            <tr><td>Torsion constant J</td><td class="r">{fmt("inertia", p().j)}</td></tr></>}</Show>
        </tbody></table>
        <p class="muted small">The properties are worked out from the dimensions, root fillets included (Help › Verification checks them against the tables). J leaves the fillets out, so it is a little below the tables' value.</p>
      </div>; }}</Show>
    </div>
  </Dialog>;
});
