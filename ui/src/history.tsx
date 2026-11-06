// The design's history (0.7.2): its features in order, in the tree and in the timeline under the
// 3D view, each with a right-click menu (Edit Feature, Edit Sketch, Rename, Suppress, Roll Back
// Here, Delete), and the solids' menu (Rename, Delete, Sketch on a face). Every action is one
// command of the geometry engine (crates/cad); the tree and the timeline redraw from its scene.
import { For, Show, createSignal } from "solid-js";
import { openMenu, openDialog, closeDialog, setTab, toast, Item } from "./store";
import { cad, cadRun, cadQuery, pick3, pickTool, ways } from "./cad";
import { Dialog } from "./ui";

export type FeatureItem = { id: number; name: string; kind: string; suppressed: boolean };
/** The feature picked in the tree or the timeline. */
export const [selFeature, setSelFeature] = createSignal<number | null>(null);

/** Each kind of feature's icon. */
export const KIND_ICON: Record<string, string> = {
  Sketch: "poly", Extrude: "solid", Revolve: "axi", Hole: "hcircle", Fillet: "fillet", Chamfer: "chamfer", Shell: "solid", PressPull: "up",
  Mirror: "symmetry", Move: "copy", BodyTransfer: "copy", Sweep: "curve", Loft: "curve", Combine: "union",
};
export const kindIcon = (k: string) => KIND_ICON[k] ?? (k.startsWith("Pattern") ? "pattern" : k.startsWith("Construct") || k.includes("Plane") ? "grid" : "solid");
/** Whether a feature is past the timeline's marker (rolled back: not in the model now). */
export const rolledBack = (i: number) => { const m = cad()?.marker; return m != null && i >= m; };

/** Edit Feature: a sketch opens for drawing; any other feature shows its inputs (the sizes it was
 * made with), each changed in place. */
export const editFeature = async (f: FeatureItem) => {
  if (f.kind === "Sketch") return editSketch(f.id);
  const r = await cadQuery<any>("timeline.inspect", { feature: f.id }); if (!r) return;
  const inputs: { label: string; param: string; expression: string; unit?: string }[] = r.inputs ?? [];
  const def = r.definition ?? {};
  // Plain values of the definition that are not already among the inputs: numbers, words, yes/no.
  const plain = Object.entries(def).filter(([k, v]) => ["number", "string", "boolean"].includes(typeof v) && !inputs.some(i => i.param === k) && k !== "sketch" && k !== "kind" && k !== "type");
  openDialog(() => {
    const [vals, setVals] = createSignal<Record<string, string>>(Object.fromEntries([...inputs.map(i => [i.param, i.expression]), ...plain.map(([k, v]) => [k, String(v)])]));
    const apply = async () => {
      for (const i of inputs) if (vals()[i.param] !== i.expression && !(await cadRun("parameters.change", { name: i.param, expression: vals()[i.param] }))) return;
      const set: Record<string, unknown> = {};
      for (const [k, v] of plain) { const t = vals()[k]; if (t !== String(v)) set[k] = typeof v === "number" ? (isNaN(+t) ? t : +t) : typeof v === "boolean" ? t === "true" : t; }
      if (Object.keys(set).length && !(await cadRun("timeline.edit", { feature: f.id, set }))) return;
      closeDialog();
    };
    return <Dialog title={`Edit ${f.name}`} okText="OK" ok={apply} help="modelling">
      <Show when={r.error}><p class="err">{r.error}</p></Show><Show when={r.warning}><p class="warn">{r.warning}</p></Show>
      <Show when={inputs.length || plain.length} fallback={<p class="muted">{f.name} has no sizes to change here. Edit its sketch, or delete it and make it again.</p>}>
        <table class="kv"><tbody>
          <For each={inputs}>{i => <tr><td>{i.label}</td><td><input class="mono" value={vals()[i.param]} onInput={e => setVals({ ...vals(), [i.param]: e.currentTarget.value })} /></td><td class="muted">{i.unit ?? ""}</td></tr>}</For>
          <For each={plain}>{([k, v]) => <tr><td>{k.replace(/_/g, " ")}</td><td>{typeof v === "boolean"
            ? <input type="checkbox" checked={vals()[k] === "true"} onChange={e => setVals({ ...vals(), [k]: String(e.currentTarget.checked) })} />
            : <input class="mono" value={vals()[k]} onInput={e => setVals({ ...vals(), [k]: e.currentTarget.value })} />}</td><td /></tr>}</For>
        </tbody></table></Show>
    </Dialog>;
  });
};
/** Edit Sketch: the sketch itself, or the one a feature was made from. */
export const editSketch = async (id: number) => {
  const f = cad()?.features.find(x => x.id === id);
  let sk = f?.kind === "Sketch" ? id : null;
  if (sk === null) { const r = await cadQuery<any>("timeline.inspect", { feature: id }); const s = r?.definition?.sketch ?? r?.definition?.profile?.sketch; sk = typeof s === "number" ? s : cad()?.features.find(x => x.name === s && x.kind === "Sketch")?.id ?? null; }
  if (sk === null) return toast(`${f?.name ?? "This feature"} is not made from a sketch.`);
  pick3(null);
  if (await cadRun("sketch.edit", { sketch: sk })) { setTab("geometry"); pickTool(ways().Line); }
};
/** A small dialog asking for a new name. */
const renameDialog = (title: string, old: string, run: (name: string) => Promise<boolean>) => openDialog(() => {
  const [v, setV] = createSignal(old);
  return <Dialog title={title} okText="Rename" ok={async () => { const n = v().trim(); if (n && n !== old && !(await run(n))) return; closeDialog(); }}>
    <input class="mono wide" value={v()} ref={el => setTimeout(() => { el.focus(); el.select(); })} onInput={e => setV(e.currentTarget.value)} />
  </Dialog>;
});

/** A feature's right-click menu (tree, timeline). */
export const featureMenu = (e: MouseEvent, f: FeatureItem, i: number) => {
  e.preventDefault(); e.stopPropagation(); setSelFeature(f.id);
  const m = cad()?.marker;
  const items: Item[] = [
    { label: "Edit Feature", icon: "opts", run: () => editFeature(f) },
    { label: "Edit Sketch", icon: "poly", run: () => editSketch(f.id) },
    { label: "Rename", keys: "F2", icon: "rename", run: () => renameDialog(`Rename ${f.name}`, f.name, n => cadRun("timeline.rename", { feature: f.id, name: n })) },
    { label: f.suppressed ? "Unsuppress" : "Suppress", icon: "pause", run: () => cadRun("timeline.suppress", { feature: f.id }) },
    { label: "Roll Back Here", icon: "undo", run: () => cadRun("timeline.rollback", { position: i + 1 }) },
    ...(m != null ? [{ label: "Roll to the End", icon: "redo", run: () => cadRun("timeline.rollback", { position: "end" }) } as Item] : []),
    "-",
    { label: "Delete", keys: "Del", icon: "del", run: () => cadRun("timeline.delete", { feature: f.id }).then(ok => ok && setSelFeature(null)) },
  ];
  openMenu(e, items);
};
/** A solid's right-click menu (tree, 3D view). */
export const bodyMenu = (e: MouseEvent | { x: number; y: number }, name: string) => {
  if ("preventDefault" in e) { e.preventDefault(); e.stopPropagation(); }
  openMenu(e, [
    { label: "Sketch on a face of it…", icon: "new", run: () => pick3("sketchface") },
    { label: "Rename", icon: "rename", run: () => renameDialog(`Rename ${name}`, name, n => cadRun("body.rename", { body: name, name: n })) },
    "-",
    { label: "Delete", icon: "del", run: () => cadRun("solid.remove", { bodies: [name] }) },
  ]);
};
/** Delete or F2 on the feature picked. */
export const historyKey = (e: KeyboardEvent) => {
  const id = selFeature(), fs = cad()?.features ?? [], i = fs.findIndex(f => f.id === id);
  if (i < 0 || (e.target as HTMLElement)?.tagName === "INPUT") return false;
  if (e.key === "Delete") { cadRun("timeline.delete", { feature: id }).then(ok => ok && setSelFeature(null)); return true; }
  if (e.key === "F2") { const f = fs[i]; renameDialog(`Rename ${f.name}`, f.name, n => cadRun("timeline.rename", { feature: f.id, name: n })); return true; }
  return false;
};

/** The timeline under the 3D view: every feature in order, the marker after the last one active,
 * and buttons to step it back and forth. Click picks, double-click edits, right-click opens the
 * menu. */
export const Timeline = () => {
  const fs = () => cad()?.features ?? [];
  const pos = () => cad()?.marker ?? fs().length;
  const go = (p: number) => cadRun("timeline.rollback", { position: p >= fs().length ? "end" : Math.max(0, p) });
  return (
    <Show when={fs().length}>
      <div class="timeline" onPointerDown={e => e.stopPropagation()} onPointerUp={e => e.stopPropagation()} onContextMenu={e => e.preventDefault()}>
        <b>History</b>
        <button title="To the start" onClick={() => go(0)}>⏮</button><button title="One back" onClick={() => go(pos() - 1)}>◀</button>
        <button title="One on" onClick={() => go(pos() + 1)}>▶</button><button title="To the end" onClick={() => go(fs().length)}>⏭</button>
        <div class="tlchips">
          <For each={fs()}>{(f, i) => <>
            <button class="tlchip" classList={{ on: selFeature() === f.id, off: f.suppressed, back: rolledBack(i()) }} title={`${f.name} (${f.kind})${f.suppressed ? ", suppressed" : ""}`}
              onClick={() => setSelFeature(f.id)} onDblClick={() => editFeature(f)} onContextMenu={e => featureMenu(e, f, i())}>
              <svg class="tic"><use href={`#i-${kindIcon(f.kind)}`} /></svg></button>
            <Show when={i() === pos() - 1 && cad()?.marker != null}><i class="tlmark" title="The marker: features after it are rolled back" /></Show>
          </>}</For>
        </div>
      </div>
    </Show>
  );
};
