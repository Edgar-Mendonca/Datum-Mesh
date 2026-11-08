// The bottom panel's Tables (0.10.2, the user's mock-ups): a frame's nodes and members side by side,
// typed into as a sheet is, and picked with the view (a picked row is picked in the view, and the
// other way). Paste from a sheet adds the nodes or members copied from a spreadsheet. Loaded only
// when the tab opens. Every change goes to the engine as an ordinary edit (undoable); nothing here
// calculates.
import { For, Show, createMemo, createSignal } from "solid-js";
import { model, sel, setSel, act, note, toast } from "./store";
import { Node, Member, Load, LoadKind, num } from "./api";
import { fromU, toU, unit, Q } from "./units";
import { loadName } from "./panels";
import { toD, fromD, zUp, V3 } from "./axes";

const SUP: Record<string, string> = { fixed: "Fixed", pinned: "Pinned", rollerx: "Roller x", rollery: "Roller y", roller: "Roller", spring: "Spring", symmetry: "Symmetry" };
const shown = (n: Node): V3 => toD([n.x, n.y, n.z ?? 0]);
const len = (m: Member, nodes: Map<number, Node>) => {
  const a = nodes.get(m.a), b = nodes.get(m.b);
  return a && b ? Math.hypot(b.x - a.x, b.y - a.y, (b.z ?? 0) - (a.z ?? 0)) : 0;
};
/** A typed number in the shown units, or null when it is not one. */
const typed = (s: string) => { const v = Number(s.trim().replace(",", ".")); return s.trim() !== "" && isFinite(v) ? v : null; };
const pick = (e: MouseEvent, id: number) => setSel(e.ctrlKey || e.shiftKey ? [...new Set([...sel(), id])] : [id]);

/** A cell typed into: the change goes when Enter is pressed or the cell is left. */
const Cell = (p: { v: string; set: (s: string) => void; title?: string }) =>
  <input class="tcell mono" value={p.v} title={p.title} onClick={e => e.stopPropagation()}
    onKeyDown={e => { if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur(); if (e.key === "Escape") { e.currentTarget.value = p.v; e.currentTarget.blur(); } }}
    onChange={e => { const s = e.currentTarget.value; if (s !== p.v) p.set(s); }} />;

const setNode = (n: Node, k: number, s: string) => {
  const v = typed(s); if (v === null) { toast("Type a number.", false); return; }
  const d = shown(n); d[k] = v;
  const [x, y, z] = fromD(d.map(c => fromU("coord", c)) as V3);
  act({ Set: { item: { Node: { ...n, x, y, z } } } });
};
const nodeRef = (s: string, nodes: Map<number, Node>) => { const id = Number(s.trim().replace(/^n/i, "")); return nodes.has(id) ? id : null; };

/** Paste from a sheet: rows of two or three numbers are nodes; rows of four or six numbers are
 *  members from point to point (made with their nodes); rows of two node numbers (N1, N2) are
 *  members between nodes already there. A heading row and a text first column are skipped. */
export const pasteSheet = async () => {
  const m = model(); if (!m) return;
  let text = "";
  try { text = await navigator.clipboard.readText(); } catch { toast("The clipboard could not be read: copy the cells again and try once more.", false); return; }
  const nodes = new Map(m.nodes.map(n => [n.id, n]));
  const three = zUp(), sec = m.sections[0]?.id ?? 0, mat = m.materials[0]?.id ?? 0;
  const ops: object[] = []; let nn = 0, mm = 0, skipped = 0;
  for (const line of text.split(/\r?\n/)) {
    let cells = line.split(/\t|;|,(?=\s*-?\d)/).map(c => c.trim()).filter(c => c !== "");
    if (!cells.length) continue;
    // Two node numbers: a member between nodes already there.
    if (cells.length >= 2 && /^n?\d+$/i.test(cells[cells.length - 2]) && /^n\d+$/i.test(cells[cells.length - 1])) {
      const a = nodeRef(cells[cells.length - 2], nodes), b = nodeRef(cells[cells.length - 1], nodes);
      if (a !== null && b !== null && a !== b) { ops.push({ Add: { item: { Member: { id: 0, a, b, kind: "beam", section: sec, material: mat } } } }); mm++; } else skipped++;
      continue;
    }
    if (typed(cells[0]) === null) cells = cells.slice(1);
    const v = cells.map(typed);
    if (!v.length || v.some(x => x === null)) { skipped++; continue; }
    const c = (v as number[]).map(x => fromU("coord", x));
    const pt = (p: number[]): V3 => fromD([p[0], p[1], three ? (p[2] ?? 0) : 0]);
    if (v.length === 2 || (three && v.length === 3)) { const [x, y, z] = pt(c); ops.push({ Add: { item: { Node: { id: 0, x, y, z } } } }); nn++; }
    else if (v.length === 4 || (three && v.length === 6)) {
      const k = v.length / 2, a = pt(c.slice(0, k)), b = pt(c.slice(k));
      ops.push({ AddMember: { from: [a[0], a[1]], to: [b[0], b[1]], kind: "beam", section: m.sections[0]?.id ?? null, material: m.materials[0]?.id ?? null, z: [a[2], b[2]] } }); mm++;
    } else skipped++;
  }
  if (!ops.length) { toast(`Nothing to paste: copy rows of x and y (a node each), of x, y to x, y (a member each), or of two node numbers such as N1 and N2. In ${unit("coord")}.`, false); return; }
  const o = await act({ Batch: ops });
  if (o) note(`Pasted from a sheet: ${nn ? `${nn} node${nn === 1 ? "" : "s"}` : ""}${nn && mm ? " and " : ""}${mm ? `${mm} member${mm === 1 ? "" : "s"}` : ""}${skipped ? `; ${skipped} row${skipped === 1 ? "" : "s"} not understood, left out` : ""}`, skipped ? "warn" : "info", "action");
};

const NodesMembers = () => {
  const m = () => model()!;
  const nodes = createMemo(() => new Map(m().nodes.map(n => [n.id, n])));
  const three = () => zUp();
  const sup = (id: number) => { const s = m().supports.find(x => x.at.on === "node" && x.at.id === id); return s ? SUP[s.kind.kind] ?? s.kind.kind : ""; };
  const setMember = (x: Member, f: Partial<Member>) => act({ Set: { item: { Member: { ...x, ...f } } } });
  const ends = (x: Member, which: "a" | "b", s: string) => { const id = nodeRef(s, nodes()); if (id === null || id === (which === "a" ? x.b : x.a)) { toast("Type the number of another node that is there, such as 3 or N3.", false); return; } setMember(x, { [which]: id }); };
  const on = (id: number) => sel().includes(id);
  const ax = () => (three() ? ["x", "y", "z"] : ["x", "y"]);
  return <div class="ctables inner">
    <div class="ctbar"><button class="btn sm" onClick={pasteSheet} title="Add the nodes or members copied from a spreadsheet: x, y (z) for nodes; x, y to x, y for members; or two node numbers">Paste from a sheet</button>
      <span class="muted small">Type into a cell and press Enter; click a row to pick it in the view. Lengths in {unit("coord")}.</span></div>
    <div class="ctwrap">
      <div class="ctpane"><table class="tbl ttab" data-table="nodes"><thead><tr><th>Node</th><For each={ax()}>{a => <th class="r">{a} ({unit("coord")})</th>}</For><th>Support</th></tr></thead>
        <tbody><For each={m().nodes}>{n => <tr classList={{ on: on(n.id) }} onClick={e => pick(e, n.id)}><td>N{n.id}</td>
          <For each={ax()}>{(_, k) => <td class="r"><Cell v={num(toU("coord", shown(n)[k()]), 6)} set={s => setNode(n, k(), s)} /></td>}</For><td>{sup(n.id)}</td></tr>}</For></tbody></table></div>
      <div class="ctpane"><table class="tbl ttab" data-table="members"><thead><tr><th>Member</th><th>From</th><th>To</th><th>Kind</th><th>Section</th><th>Material</th><th class="r">Length ({unit("coord")})</th></tr></thead>
        <tbody><For each={m().members}>{x => <tr classList={{ on: on(x.id) }} onClick={e => pick(e, x.id)}><td>M{x.id}</td>
          <td><Cell v={`N${x.a}`} set={s => ends(x, "a", s)} /></td><td><Cell v={`N${x.b}`} set={s => ends(x, "b", s)} /></td>
          <td><select value={x.kind} onClick={e => e.stopPropagation()} onChange={e => setMember(x, { kind: e.currentTarget.value as Member["kind"] })}><option value="beam">Beam</option><option value="truss">Truss bar</option></select></td>
          <td><select value={x.section} onClick={e => e.stopPropagation()} onChange={e => setMember(x, { section: +e.currentTarget.value })}><For each={m().sections}>{s => <option value={s.id}>{s.name}</option>}</For></select></td>
          <td><select value={x.material} onClick={e => e.stopPropagation()} onChange={e => setMember(x, { material: +e.currentTarget.value })}><For each={m().materials}>{s => <option value={s.id}>{s.name}</option>}</For></select></td>
          <td class="r mono">{num(toU("coord", len(x, nodes())), 4)}</td></tr>}</For></tbody></table></div>
    </div>
    <Show when={!m().nodes.length}><p class="muted cempty">No nodes yet. Draw members on the view, or paste rows from a sheet.</p></Show>
  </div>;
};

// 0.10.3: every load in one table, and every member's assignments, typed into as the others are.
/** The numbers a load of each kind carries, in order: [field, label, quantity]. */
const LOAD_FIELDS: Record<string, [string, string, Q][]> = {
  force: [["fx", "Fx", "force"], ["fy", "Fy", "force"], ["fz", "Fz", "force"]],
  moment: [["m", "M", "moment"], ["mx", "Mx", "moment"], ["my", "My", "moment"]],
  dist: [["w1", "w start", "dist"], ["w2", "w end", "dist"]],
  patch: [["w1", "w from", "dist"], ["w2", "w to", "dist"], ["from", "From", "len"], ["to", "To", "len"]],
  concentrated: [["f", "F", "force"], ["at", "At", "len"]],
  settlement: [["dx", "dx", "len"], ["dy", "dy", "len"], ["dz", "dz", "len"]],
  mass: [["m", "Mass", "mass"]],
  warming: [["dt", "ΔT", "dtemp"]],
  gravity: [["gx", "gx", "accel"], ["gy", "gy", "accel"], ["gz", "gz", "accel"]],
};
const DIRS: Record<string, string> = { y: "Global y (down −)", x: "Global x", z: "Global z", local: "Across the member" };

const LoadsTable = () => {
  const m = () => model()!;
  const cases = () => m().cases ?? [];
  const on = (id: number) => sel().includes(id);
  const place = (list: { id: number }[], id: number) => list.findIndex(x => x.id === id) + 1;
  const where = (l: Load) => l.at.on === "node" ? `Node ${place(m().nodes, l.at.id)}` : l.at.on === "member" ? `Member ${place(m().members, l.at.id)}` : l.at.on === "body" ? "Whole model" : l.at.on;
  const put = (l: Load, kind: LoadKind) => act({ Set: { item: { Load: { ...l, kind } } } });
  const sp = () => m().analysis === "space";
  const fields = (k: LoadKind) => (LOAD_FIELDS[k.kind] ?? []).filter(([f]) => sp() || !["fz", "mx", "my", "dz", "gz"].includes(f));
  return <div class="ctpane wide"><table class="tbl ttab" data-table="loads"><thead><tr><th>Load</th><th>Case</th><th>On</th><th>Kind</th><th>Direction</th><th>Values</th></tr></thead>
    <tbody><For each={m().loads}>{(l, i) => <tr classList={{ on: on(l.id) }} onClick={e => pick(e, l.id)}><td>Load {i() + 1}</td>
      <td><Show when={cases().length} fallback={<span class="muted">—</span>}><select value={String(l.case ?? cases()[0]?.id ?? 0)} onClick={e => e.stopPropagation()} onChange={e => act({ Set: { item: { Load: { ...l, case: +e.currentTarget.value } } } })}><For each={cases()}>{c => <option value={c.id}>{c.name}</option>}</For></select></Show></td>
      <td>{where(l)}</td><td>{loadName(l)}</td>
      <td><Show when={"dir" in l.kind} fallback={<span class="muted">—</span>}><select value={(l.kind as { dir: string }).dir} onClick={e => e.stopPropagation()} onChange={e => put(l, { ...l.kind, dir: e.currentTarget.value } as LoadKind)}><For each={Object.entries(DIRS).filter(([d]) => sp() || d !== "z")}>{([d, t]) => <option value={d}>{t}</option>}</For></select></Show></td>
      <td class="lvals"><For each={fields(l.kind)}>{([f, lab, q]) => <label>{lab}<Cell v={num(toU(q, (l.kind as unknown as Record<string, number>)[f] ?? 0), 5)} set={str => { const v = typed(str); if (v === null) return toast("Type a number.", false); put(l, { ...l.kind, [f]: fromU(q, v) } as LoadKind); }} /><span class="unit">{unit(q)}</span></label>}</For>
        <Show when={!fields(l.kind).length}><span class="muted">set in its card</span></Show></td></tr>}</For></tbody></table>
    <Show when={!m().loads.length}><p class="muted cempty">No loads yet. Add them from the Loads tab.</p></Show></div>;
};

const AssignTable = () => {
  const m = () => model()!;
  const on = (id: number) => sel().includes(id);
  const sp = () => m().analysis === "space";
  const set = (x: Member, f: Partial<Member>) => act({ Set: { item: { Member: { ...x, ...f } } } });
  const tick = (x: Member, k: 0 | 1) => <input type="checkbox" checked={!!x.hinge?.[k]} disabled={x.kind === "truss"} onClick={e => e.stopPropagation()} onChange={e => { const h: [boolean, boolean] = [!!x.hinge?.[0], !!x.hinge?.[1]]; h[k] = e.currentTarget.checked; set(x, { hinge: h }); }} />;
  const off = (x: Member, k: 0 | 1) => <Cell v={num(toU("len", x.offset?.[k] ?? 0), 4)} set={str => { const v = typed(str); if (v === null || v < 0) return toast("Type a length of zero or more.", false); const o: [number, number] = [x.offset?.[0] ?? 0, x.offset?.[1] ?? 0]; o[k] = fromU("len", v); set(x, { offset: o }); }} />;
  return <div class="ctpane wide"><table class="tbl ttab" data-table="assignments"><thead><tr><th>Member</th><th>Kind</th><th>Section</th><th>Material</th><th>Carries</th><th>Hinge start</th><th>Hinge end</th><th class="r">Rigid start ({unit("len")})</th><th class="r">Rigid end ({unit("len")})</th><Show when={sp()}><th class="r">Roll (°)</th></Show></tr></thead>
    <tbody><For each={m().members}>{(x, i) => <tr classList={{ on: on(x.id) }} onClick={e => pick(e, x.id)}><td>Member {i() + 1}</td>
      <td><select value={x.kind} onClick={e => e.stopPropagation()} onChange={e => set(x, { kind: e.currentTarget.value as Member["kind"] })}><option value="beam">Beam</option><option value="truss">Truss bar</option></select></td>
      <td><select value={x.section} onClick={e => e.stopPropagation()} onChange={e => set(x, { section: +e.currentTarget.value })}><For each={m().sections}>{q => <option value={q.id}>{q.name}</option>}</For></select></td>
      <td><select value={x.material} onClick={e => e.stopPropagation()} onChange={e => set(x, { material: +e.currentTarget.value })}><For each={m().materials}>{q => <option value={q.id}>{q.name}</option>}</For></select></td>
      <td><select value={x.only ?? "both"} onClick={e => e.stopPropagation()} onChange={e => set(x, { only: (e.currentTarget.value === "both" ? undefined : e.currentTarget.value) as Member["only"] })}><option value="both">Both</option><option value="tension">Tension only</option><option value="compression">Compression only</option></select></td>
      <td class="c">{tick(x, 0)}</td><td class="c">{tick(x, 1)}</td>
      <td class="r">{off(x, 0)}</td><td class="r">{off(x, 1)}</td>
      <Show when={sp()}><td class="r"><Cell v={num((x.roll ?? 0) * 180 / Math.PI, 4)} set={str => { const v = typed(str); if (v === null) return toast("Type an angle in degrees.", false); set(x, { roll: v * Math.PI / 180 }); }} /></td></Show></tr>}</For></tbody></table>
    <Show when={!m().members.length}><p class="muted cempty">No members yet.</p></Show></div>;
};

/** The bottom panel's Tables: nodes and members, loads, and each member's assignments. */
export const FrameTables = () => {
  const [which, setWhich] = createSignal<"geo" | "loads" | "assign">("geo");
  const m = () => model()!;
  return <div class="ctables">
    <div class="cttabs">
      <button classList={{ on: which() === "geo" }} data-ttab="geo" onClick={() => setWhich("geo")}>Nodes & members <i>{m().nodes.length + m().members.length}</i></button>
      <button classList={{ on: which() === "loads" }} data-ttab="loads" onClick={() => setWhich("loads")}>Loads <i>{m().loads.length}</i></button>
      <button classList={{ on: which() === "assign" }} data-ttab="assign" onClick={() => setWhich("assign")}>Assignments <i>{m().members.length}</i></button>
    </div>
    <Show when={which() === "geo"}><NodesMembers /></Show>
    <Show when={which() === "loads"}><div class="ctwrap one"><LoadsTable /></div></Show>
    <Show when={which() === "assign"}><div class="ctwrap one"><AssignTable /></div></Show>
  </div>;
};
