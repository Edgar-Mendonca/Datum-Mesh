// Design members (0.10.4): form one from a chain of members, its card (chain, restraints,
// segments) and dissolving it. The engine finds the chain and the segments (frames/design.rs).
import { For, Show, createResource, createSignal } from "solid-js";
import { DesignMember, Model, num } from "./api";
import { act, active, call, model, openDialog, sel, setSel, toast, whichName } from "./store";
import { Dialog } from "./ui";
import { numIn } from "./panels";
import { fmt, fromU, toU, unit } from "./units";

type Info = { chain: { length: number; members: { member: number; start: number; length: number; forward: boolean }[] } | null; segments: { from: number; to: number; length: number; why: string }[]; broken: string | null };

/** Geometry › Form Design Member: from the selected members, end to end. */
export const formDesignMember = async () => {
  const m = model(); if (!m) return;
  const s = new Set(sel()), ids = m.members.filter(b => s.has(b.id)).map(b => b.id);
  if (!ids.length) return toast("Select the members of the beam or column first, end to end.", false);
  const o = await act({ FormDesignMember: { ids, name: "" } });
  if (o?.created.length) { setSel(o.created); toast(`Design member made from ${ids.length} ${ids.length === 1 ? "member" : "members"}: add its restraints in its card.`, true); }
};

const HOLDS: [string, string][] = [["both", "Sideways and twisting"], ["sideways", "Sideways"], ["twist", "Twisting"]];

/** A design member's card: its chain, its restraints (typed in) and the segments they make. */
export const DMInsp = (p: { d: DesignMember; m: Model }) => {
  const [info] = createResource(() => [p.d, p.m] as const, ([d]) => call<Info>("design_member", { id: d.id }));
  const set = (d: Partial<DesignMember>) => act({ Set: { item: { DesignMember: { ...p.d, ...d } } } });
  const place = (id: number) => p.m.members.findIndex(x => x.id === id) + 1;
  const len = () => info()?.chain?.length ?? 0;
  const addR = () => { const at = Math.round(len() / 2 * 1000) / 1000; set({ restraints: [...p.d.restraints, { at, holds: "both" }] }); };
  return <div class="dmcard">
    <div class="iprow"><label>Name</label><input value={p.d.name} onChange={e => e.currentTarget.value.trim() && set({ name: e.currentTarget.value.trim() })} /></div>
    <Show when={info()} fallback={<p class="muted small">Finding the chain…</p>}>{i => <Show when={!i().broken} fallback={<p class="warn small">Broken: {i().broken} Dissolve it, or put the members back.</p>}>
      <div class="iprow"><label>Length</label><span class="mono">{fmt("len", len(), 4)}</span></div>
      <div class="iprow"><label>Made of</label><span class="small">{i().chain!.members.map(l => `Member ${place(l.member)}`).join(" → ")}</span></div>
      <h5 class="ihead">Restraints <span class="muted small">(braces, measured from the start)</span></h5>
      <table class="tbl dmres"><thead><tr><th class="r">At ({unit("len")})</th><th>Holds</th><th /></tr></thead><tbody>
        <For each={p.d.restraints}>{(r, k) => <tr>
          <td class="r"><input class="mono fac" value={num(toU("len", r.at), 5)} onChange={numIn(v => { const at = fromU("len", v); if (!(at >= 0 && at <= len())) return false; set({ restraints: p.d.restraints.map((x, j) => j === k() ? { ...x, at } : x) }); }, () => num(toU("len", r.at), 5))} /></td>
          <td><select value={r.holds} onChange={e => set({ restraints: p.d.restraints.map((x, j) => j === k() ? { ...x, holds: e.currentTarget.value as typeof r.holds } : x) })}><For each={HOLDS}>{([v, l]) => <option value={v}>{l}</option>}</For></select></td>
          <td><button class="x" title="Remove this restraint" onClick={() => set({ restraints: p.d.restraints.filter((_, j) => j !== k()) })}>×</button></td></tr>}</For>
      </tbody></table>
      <p class="ibtns"><button class="btn sm" onClick={addR}>Add a restraint</button></p>
      <h5 class="ihead">Segments <span class="muted small">(between restraints; extra mesh nodes never change them)</span></h5>
      <table class="tbl dmseg" data-table="segments"><thead><tr><th class="r">From</th><th class="r">To</th><th class="r">Length</th><th>Ends at</th></tr></thead><tbody>
        <For each={i().segments}>{sg => <tr><td class="r mono">{fmt("len", sg.from, 4)}</td><td class="r mono">{fmt("len", sg.to, 4)}</td><td class="r mono">{fmt("len", sg.length, 4)}</td><td class="small">{sg.why}</td></tr>}</For>
      </tbody></table>
      <p class="muted small">A brace you mark here is taken as effective: check that it really holds the member. Results › Mechanics Checks takes the longest length between sideways restraints as the buckling length the weak way.</p>
    </Show>}</Show>
    <p class="ibtns"><button class="btn sm" onClick={() => act({ Delete: { ids: [p.d.id] } }).then(o => o && setSel([]))} title="Undo the design member; its members stay as they are">Dissolve</button></p>
  </div>;
};

// ---------- Results › Design Members (0.10.4): checks from mechanics formulas ----------

type Check = { kind: string; status: string; ratio: number | null; governing: string; trace: string[] };
type MemberReport = { id: number; name: string; input: string; solve: string; checks: Check[]; status: string; ratio: number | null; governing: string };
type Storey = { level: number; height: number; dx: number; dz: number; ratio: number; status: string; governing: string };
export type Report = { settings: Settings; combos: { name: string; service: boolean; solve: string }[]; members: MemberReport[]; storeys: Storey[]; notes: string[]; not_checked: string[] };
type Settings = { k: number; span_ratio: number; drift: number };

/** A status as a coloured word: pass, fail, not checked, missing data. */
const St = (p: { s: string }) => <span class={`cst ${p.s.replace(" ", "-")}`} data-status={p.s}>{p.s}</span>;
const ratio = (r: number | null) => r === null ? "–" : num(r, 3);

/** Results › Mechanics Checks: the settings, then each design member's checks, statuses and
 * trace; the storey drift; what is never checked. */
export const checksDialog = (start: "members" | "drift" = "members") => openDialog(() => {
  const m = () => model()!;
  const s = () => m().check_settings ?? { k: 1, span_ratio: 300, drift: 0.005 };
  const [tab, setTab] = createSignal(start);
  const [open, setOpen] = createSignal<number | null>(null);
  const [rep] = createResource(() => m(), () => call<Report>("mech_checks").catch((e: unknown) => String(e)));
  const r = () => { const x = rep(); return x && typeof x !== "string" ? x : null; };
  const set = (f: Partial<Settings>) => act({ CheckSettings: { s: { ...s(), ...f } } });
  const cell = (c: Check | undefined) => c ? <td class="r"><St s={c.status} /> <span class="mono">{ratio(c.ratio)}</span></td> : <td />;
  return <Dialog title="Mechanics checks" wide help="design" okText="Close">
    <div class="mech">
      <p class="muted small">From mechanics formulas only, not a design code: stress against yield, the Euler load with your effective length, deflection against span over your ratio, storey drift. Every factor is yours.</p>
      <div class="form mset">
        <label>Effective length factor K</label><div class="pf"><input class="mono" value={s().k} onChange={numIn(v => v > 0 ? set({ k: v }) : false, () => s().k)} /><span class="hint small muted">buckling length = K × length between restraints</span></div>
        <label>Deflection limit: span /</label><div class="pf"><input class="mono" value={s().span_ratio} onChange={numIn(v => v > 0 ? set({ span_ratio: v }) : false, () => s().span_ratio)} /><span class="hint small muted">measured from the line between the member's moved ends</span></div>
        <label>Storey drift limit</label><div class="pf"><input class="mono" value={num(s().drift * 100, 4)} onChange={numIn(v => v > 0 ? set({ drift: v / 100 }) : false, () => num(s().drift * 100, 4))} /><span class="hint small muted">% of the storey height</span></div>
      </div>
      <div class="segtabs"><button classList={{ on: tab() === "members" }} onClick={() => setTab("members")}>Design members</button><button classList={{ on: tab() === "drift" }} onClick={() => setTab("drift")}>Storey drift</button><button classList={{ on: tab() === "missing" as never }} onClick={() => setTab("missing" as never)}>Not checked</button></div>
      <Show when={rep.loading}><p class="muted small">Solving every combination…</p></Show>
      <Show when={typeof rep() === "string"}><p class="warn small">{String(rep())}</p></Show>
      <Show when={r()}>{x => <>
        <Show when={tab() === "members"}>
          <Show when={x().members.length} fallback={<p class="muted small">No design members yet: select the members of a beam or column and use Geometry › Form Design Member.</p>}>
            <table class="tbl mtab" data-table="checks"><thead><tr><th>Member</th><th>Input</th><th>Solve</th><th class="r">Stress</th><th class="r">Buckling</th><th class="r">Deflection</th><th>Check</th><th class="r">Ratio</th><th>Governing</th></tr></thead><tbody>
              <For each={x().members}>{mr => <>
                <tr class="mrow" classList={{ on: open() === mr.id }} onClick={() => setOpen(open() === mr.id ? null : mr.id)} title="Show the formulas and values"><td>{open() === mr.id ? "▾" : "▸"} {mr.name}</td><td class="small">{mr.input}</td><td class="small">{mr.solve}</td>
                  {cell(mr.checks.find(c => c.kind === "stress"))}{cell(mr.checks.find(c => c.kind === "buckling"))}{cell(mr.checks.find(c => c.kind === "deflection"))}
                  <td><St s={mr.status} /></td><td class="r mono">{ratio(mr.ratio)}</td><td class="small">{mr.governing}</td></tr>
                <Show when={open() === mr.id}><tr class="trace"><td colspan="9"><For each={mr.checks}>{c => <div class="tr1"><h5>{c.kind} <St s={c.status} /></h5><ol><For each={c.trace}>{t => <li class="mono small">{t}</li>}</For></ol></div>}</For></td></tr></Show>
              </>}</For></tbody></table>
          </Show>
        </Show>
        <Show when={tab() === "drift"}>
          <Show when={x().storeys.length} fallback={<p class="muted small">No storeys: drift needs members at two levels or more (levels along y, up).</p>}>
            <table class="tbl mtab" data-table="drift"><thead><tr><th class="r">Level</th><th class="r">Height</th><th class="r">Drift x</th><Show when={m().analysis === "space"}><th class="r">Drift z</th></Show><th class="r">Limit</th><th>Check</th><th class="r">Ratio</th><th>Governing</th></tr></thead><tbody>
              <For each={x().storeys}>{d => <tr><td class="r mono">{fmt("len", d.level, 4)}</td><td class="r mono">{fmt("len", d.height, 4)}</td><td class="r mono">{num(d.dx * 100, 3)} %</td><Show when={m().analysis === "space"}><td class="r mono">{num(d.dz * 100, 3)} %</td></Show><td class="r mono">{num(x().settings.drift * 100, 3)} %</td><td><St s={d.status} /></td><td class="r mono">{num(d.ratio, 3)}</td><td class="small">{d.governing}</td></tr>}</For></tbody></table>
            <p class="muted small">Drift = (mean movement of a level's nodes − that of the level below) / storey height, {m().analysis === "space" ? "along x and z" : "along x"}.</p>
          </Show>
        </Show>
        <Show when={(tab() as string) === "missing"}>
          <p class="small">These checks never look at:</p><ul class="small notchk"><For each={x().not_checked}>{t => <li>{t}</li>}</For></ul>
        </Show>
        <div class="mfoot small muted"><For each={x().notes}>{t => <p>{t}</p>}</For>
          <p>Solved: <For each={x().combos}>{(c, i) => <>{i() ? " · " : ""}{c.name} <span class="pill">{!(m().combos ?? []).length ? "every check" : c.service ? "service" : "strength"}</span>{c.solve === "solved" ? "" : ` (${c.solve})`}</>}</For></p></div>
      </>}</Show>
    </div>
  </Dialog>;
});

type Probe = { at: number; length: number; left: Vals; right: Vals; jump: boolean; case: string };
type Vals = { n: number; v: number; m: number; m_minor: number; d: number[]; deflection: number };

/** Results › Values Along: N, V, M and deflection at any distance along a design member, both
 * sides of a jump, under the case or combination picked on the Loads tab. */
export const valuesDialog = () => openDialog(() => {
  const m = () => model()!;
  const dms = () => m().design_members ?? [];
  const pre = sel().find(id => dms().some(d => d.id === id));
  const [id, setId] = createSignal(pre ?? dms()[0]?.id ?? 0);
  const [at, setAt] = createSignal(0);
  const [p] = createResource(() => [id(), at(), m()] as const, ([i, a]) => i ? call<Probe>("member_values", { id: i, at: a, which: active() }).catch((e: unknown) => String(e)) : Promise.resolve("No design members yet: form one under Geometry › Design Members."));
  const pr = () => { const x = p(); return x && typeof x !== "string" ? x : null; };
  const row = (l: string, v: Vals) => <tr><td>{l}</td><td class="r mono">{fmt("force", v.n, 4)}</td><td class="r mono">{fmt("force", v.v, 4)}</td><td class="r mono">{fmt("moment", v.m, 4)}</td><td class="r mono">{fmt("len", v.deflection, 4)}</td></tr>;
  return <Dialog title="Values along a member" help="design" okText="Close">
    <div class="form">
      <label>Design member</label><select value={id()} onChange={e => setId(+e.currentTarget.value)}><For each={dms()}>{d => <option value={d.id}>{d.name}</option>}</For></select>
      <label>At ({unit("len")} from its start)</label><div class="pf"><input class="mono" data-at value={num(toU("len", at()), 5)} onChange={numIn(v => { const a = fromU("len", v); if (!(a >= 0)) return false; setAt(a); }, () => num(toU("len", at()), 5))} /><Show when={pr()}>{x => <span class="hint small muted">of {fmt("len", x().length, 4)}</span>}</Show></div>
      <label>Under</label><span class="small">{whichName(m(), active())} <span class="muted">(pick another on the Loads tab)</span></span>
    </div>
    <Show when={typeof p() === "string"}><p class="warn small">{String(p())}</p></Show>
    <Show when={pr()}>{x => <>
      <table class="tbl vals" data-table="values"><thead><tr><th /><th class="r">N</th><th class="r">V</th><th class="r">M</th><th class="r">Deflection</th></tr></thead><tbody>
        <Show when={x().jump} fallback={row("Here", x().left)}>{row("Just before", x().left)}{row("Just after", x().right)}</Show></tbody></table>
      <p class="muted small">{x().jump ? "A point load or a joint makes a jump here: both sides are given. " : ""}Axial force positive in tension. Deflection is measured from the line between the member's moved ends.</p>
    </>}</Show>
  </Dialog>;
});
