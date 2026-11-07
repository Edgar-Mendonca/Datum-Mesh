// Planned features in the window (0.10.0, docs/PLAN-1.0.md): the register (features.ts) loads after
// the window starts, and every feature not built yet shows on its ribbon tab, in its step of the
// tree and in Help › Roadmap, dimmed, with the release that brings it. Clicked, it opens the dialog
// it will have, every field shown and disabled. View › Show planned features hides them all.
import { For, Show, createSignal, createMemo } from "solid-js";
import { model, openDialog, openMenu, Item } from "./store";
import { Dialog, Icon } from "./ui";
import type { Feat, PField } from "./features";

const KEY = "dm.planned";
const read = () => { try { return localStorage.getItem(KEY) !== "0"; } catch { return true; } };
const [showPlanned, setShowRaw] = createSignal(read());
export { showPlanned };
export const setShowPlanned = (on: boolean) => { setShowRaw(on); try { localStorage.setItem(KEY, on ? "1" : "0"); } catch { /* private window */ } };

const [feats, setFeats] = createSignal<Feat[]>([]);
let mod: typeof import("./features") | null = null;
// Loaded after the window's first paint: the register is words, not needed to start.
setTimeout(() => import("./features").then(m => { mod = m; setFeats(m.FEATS); }), 0);

/** Opens a planned feature's greyed dialog by its id (the bottom panel's planned tabs). */
export const openPlanned = (id: string) => { const f = feats().find(x => x.id === id); if (f) plannedDialog(f); };

/** The planned features shown for the open model (none when the switch is off). */
export const plannedFor = (f: { tab?: string; step?: number }) => createMemo(() => {
  feats(); if (!showPlanned() || !mod) return [] as Feat[];
  return mod.planned(model()?.analysis, f);
});

const Fld = (p: { f: PField }) => {
  const f = p.f;
  if (f.k === "num") return <><label>{f.l}</label><div class="pf"><input class="mono" disabled value={f.v ?? ""} />{f.u && <span class="unit">{f.u}</span>}</div></>;
  if (f.k === "choice") return <><label>{f.l}</label><div><select disabled><For each={f.o}>{o => <option>{o}</option>}</For></select></div></>;
  if (f.k === "check") return <><span /><div><label class="chk"><input type="checkbox" disabled checked={!!f.on} />{f.l}</label></div></>;
  if (f.k === "text") return <><label>{f.l}</label><div><input disabled value={f.v ?? ""} /></div></>;
  if (f.k === "pick") return <><label>{f.l}</label><div><button class="btn sm" disabled>Pick {f.what} in the view…</button></div></>;
  if (f.k === "table") return <><label class="top">{f.l}</label><div class="ptable"><table class="tbl"><thead><tr><For each={f.cols}>{c => <th>{c}</th>}</For></tr></thead>
    <tbody><For each={f.rows ?? [f.cols.map(() => ""), f.cols.map(() => "")]}>{r => <tr><For each={r}>{c => <td><input disabled value={c} /></td>}</For></tr>}</For></tbody></table></div></>;
  // A curve: its table beside the empty graph it will draw.
  return <><label class="top">{f.l}</label><div class="pcurve">
    <table class="tbl"><thead><tr><th>{f.x}</th><th>{f.y}</th></tr></thead><tbody><For each={[0, 1, 2, 3]}>{() => <tr><td><input disabled /></td><td><input disabled /></td></tr>}</For></tbody></table>
    <svg viewBox="0 0 200 120" width="200" height="120" role="img" aria-label={`${f.y} against ${f.x}`}><path d="M28 8V100H192" fill="none" class="ax" /><text x="192" y="114" text-anchor="end">{f.x}</text><text x="30" y="8">{f.y}</text></svg>
  </div></>;
};

/** The dialog a planned feature will have, every field disabled. */
export const plannedDialog = (f: Feat) => openDialog(() => <Dialog title={f.label} wide cancel="Close">
  <div class="pband"><Icon n="roadmap" /><div><b>Planned for {f.v}</b><span>{f.what}</span></div></div>
  <fieldset class="pform form" disabled><For each={f.fields}>{x => <Fld f={x} />}</For></fieldset>
  <p class="muted small">This is the dialog it will have. Nothing here works yet: it shows what you will set. Hide planned features in View › Show planned features.</p>
</Dialog>);

/** A ribbon tab's planned features: one Coming ▾ button after the working tools, its menu the
 *  features by group, each with its release (a tab can have fifteen: inline they crowd it out). */
export const PlannedGroups = (p: { tab: string }) => {
  const list = plannedFor({ tab: p.tab });
  const menu = (e: MouseEvent) => {
    const g = new Map<string, Feat[]>(); for (const f of list()) g.set(f.group, [...(g.get(f.group) ?? []), f]);
    const items: Item[] = [];
    for (const [name, fs] of g) { if (items.length) items.push("-"); items.push({ label: name, off: true }); for (const f of fs) items.push({ label: `${f.label}  ·  ${f.v}`, icon: f.icon, run: () => plannedDialog(f) }); }
    openMenu(e, items);
  };
  return <Show when={list().length}><div class="grp planned"><div class="items"><div class="rbw big"><button class="rb big planned" title={`${list().length} features still to come on this tab, dimmed: click one to see the dialog it will have`} onClick={menu}><Icon n="roadmap" /><span>Coming ▾</span><i class="pv">{list().length}</i></button></div></div><div class="cap">Planned</div></div></Show>;
};

/** A step's planned features in the tree, folded under "Coming". */
export const PlannedRows = (p: { step: number }) => {
  const list = plannedFor({ step: p.step });
  return <Show when={list().length}><details class="tgrp tplan"><summary><Icon n="roadmap" /><span class="tt">Coming</span><span class="pill">{list().length}</span></summary>
    <div class="tlist"><For each={list()}>{f => <button class="ti planned" title={`Planned for ${f.v}`} onClick={() => plannedDialog(f)}><svg class="tic"><use href={`#i-${f.icon}`} /></svg><span>{f.label}</span><small>{f.v}</small></button>}</For></div>
  </details></Show>;
};

/** Help › Roadmap's list, from the register: every planned feature by release. */
export const PlannedByRelease = () => {
  const by = createMemo(() => { feats(); const g = new Map<string, Feat[]>(); for (const f of mod?.FEATS ?? []) g.set(f.v, [...(g.get(f.v) ?? []), f]); return [...g.entries()].sort((a, b) => a[0].localeCompare(b[0], undefined, { numeric: true })); });
  return <div class="rmfeats"><For each={by()}>{([v, fs]) => <section><h3>{v}</h3><ul><For each={fs}>{f => <li><button class="lnk" onClick={() => plannedDialog(f)}>{f.label}</button> <span class="muted">{f.what}</span></li>}</For></ul></section>}</For></div>;
};
