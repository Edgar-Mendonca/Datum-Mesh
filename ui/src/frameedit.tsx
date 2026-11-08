// Frames (0.10.3): split, reverse and merge members, merge nodes, and the model check. The engine
// does the edits (doc.rs) and finds the problems (frames/check.rs); this only asks and shows.

import { For, Show, createSignal, createResource } from "solid-js";
import { act, call, model, sel, setSel, toast, openDialog } from "./store";
import { numIn } from "./panels";
import { Dialog } from "./ui";
import { fromU, toU, unit } from "./units";

const chosenMembers = () => { const m = model(), s = new Set(sel()); return m ? m.members.filter(b => s.has(b.id)).map(b => b.id) : []; };

/** Geometry › Split: into equal pieces, or at a distance from the start. */
export const splitDialog = () => {
  const ids = chosenMembers();
  if (!ids.length) return toast("Select the members to split first.");
  openDialog(() => {
    const [how, setHow] = createSignal<"equal" | "at">("equal"), [n, setN] = createSignal(2), [d, setD] = createSignal(1);
    const len = () => { const m = model()!, b = m.members.find(x => x.id === ids[0])!, a = m.nodes.find(x => x.id === b.a)!, c = m.nodes.find(x => x.id === b.b)!; return Math.hypot(c.x - a.x, c.y - a.y, (c.z ?? 0) - (a.z ?? 0)); };
    return <Dialog title="Split members" okText="Split" ok={async () => {
      const at = how() === "equal" ? Array.from({ length: n() - 1 }, (_, k) => (k + 1) / n()) : [fromU("len", d()) / len()];
      if (!at.length || at.some(t => !(t > 0 && t < 1))) { toast("The place must be between the member's ends."); return false; }
      const o = await act({ SplitMember: { ids, at } });
      if (!o) return false;
      toast(`Split ${ids.length} ${ids.length === 1 ? "member" : "members"}; the loads, hinges and offsets went with the pieces.`, true);
    }}>
      <p class="muted small">{ids.length} selected {ids.length === 1 ? "member" : "members"}. A node is put at each cut; loads along the member are shared out exactly.</p>
      <div class="form">
        <label>Split</label><select value={how()} onChange={e => setHow(e.currentTarget.value as "equal" | "at")}><option value="equal">into equal pieces</option><option value="at">at a distance from the start</option></select>
        <Show when={how() === "equal"} fallback={<><label>From the start ({unit("len")})</label><input class="mono" value={toU("len", d())} onChange={numIn(v => setD(v), d)} /></>}>
          <label>Pieces</label><input class="mono" value={n()} onChange={numIn(v => v >= 2 && v <= 50 && Number.isInteger(v) ? setN(v) : false, n)} />
        </Show>
      </div>
    </Dialog>;
  });
};

export const reverseMembers = async () => {
  const ids = chosenMembers();
  if (!ids.length) return toast("Select the members to turn end for end first.");
  if (await act({ ReverseMember: { ids } })) toast(`Turned ${ids.length} ${ids.length === 1 ? "member" : "members"} end for end; hinges, offsets and loads follow, so the answer is the same.`, true);
};

export const mergeMembers = async () => {
  const ids = chosenMembers();
  if (ids.length !== 2) return toast("Select two members in line that meet at a node.");
  const o = await act({ MergeMembers: { ids } });
  if (o) { setSel([ids[0]]); toast("Merged into one member; its loads are kept where they were.", true); }
};

/** Geometry › Merge Nodes: nodes closer than a tolerance become one. */
export const mergeNodesDialog = () => openDialog(() => {
  const [tol, setTol] = createSignal(0.005);
  const picked = () => { const m = model()!, s = new Set(sel()); return m.nodes.filter(n => s.has(n.id)).map(n => n.id); };
  return <Dialog title="Merge nodes" okText="Merge" ok={async () => {
    const before = model()!.nodes.length;
    const o = await act({ MergeNodes: { ids: picked(), tol: tol() } });
    if (!o) return false;
    const gone = before - model()!.nodes.length;
    toast(gone ? `Merged ${gone} ${gone === 1 ? "node" : "nodes"} into their neighbours.` : "No nodes were that close.", true);
  }}>
    <p class="muted small">{picked().length ? `${picked().length} selected nodes` : "Every node"}: any two closer than the tolerance become one (a supported node is kept); members, supports and loads follow it.</p>
    <div class="form"><label>Tolerance ({unit("len")})</label><input class="mono" value={toU("len", tol())} onChange={numIn(v => v >= 0 ? setTol(fromU("len", v)) : false, () => toU("len", tol()))} /></div>
  </Dialog>;
});

type Finding = { level: "error" | "warning" | "note"; what: string; ids: number[]; why: string };

/** Analysis › Model Check (0.10.3), in the panel under the view: it checks again whenever the
 *  model changes; a row selects what it is about, which stays in sight above. */
export const CheckPanel = () => {
  const [near, setNear] = createSignal(0.005);
  const [found] = createResource(() => [near(), model()] as const, ([d]) => call<Finding[]>("model_check", { near: d }));
  const LABEL = { error: "Error", warning: "Warning", note: "Note" };
  const count = (f: Finding[], l: string) => f.filter(x => x.level === l).length;
  return <div class="mcheck">
    <div class="mchead">
      <Show when={found()} fallback={<span class="muted">Checking…</span>}>{f => <span class="mcsum">{f().length === 0 ? "Nothing found: the model is ready to solve." : <>{count(f(), "error")} errors · {count(f(), "warning")} warnings · {count(f(), "note")} notes <span class="muted">· it checks again after every change; click a row to select what it is about</span></>}</span>}</Show>
      <span class="grow" />
      <label class="mcnear">Nodes meet within <input class="mono" value={toU("len", near())} onChange={numIn(v => v > 0 ? setNear(fromU("len", v)) : false, () => toU("len", near()))} /> {unit("len")}</label>
    </div>
    <table class="mctable"><thead><tr><th>Status</th><th>What</th><th>Why it matters</th></tr></thead>
      <tbody><For each={found() ?? []}>{x => <tr class={x.level} onClick={() => x.ids.length && setSel(x.ids)} title={x.ids.length ? "Select it in the view" : ""}><td><span class={`badge ${x.level}`}>{LABEL[x.level]}</span></td><td>{x.what}</td><td class="muted">{x.why}</td></tr>}</For></tbody></table>
  </div>;
};
