// Solids made in one go, and solids combined (0.7.6): Box, Cylinder, Cone and Sphere, placed by
// a point and their sizes (mm), and Combine: join, cut or intersect one solid with others. Each is
// one command of the geometry engine (crates/cad) and lands in the history like any feature.
import { For, Show, createSignal } from "solid-js";
import { openDialog, closeDialog, toast } from "./store";
import { cad, cadRun, pick3, turnBack, setTurnBack } from "./cad";
import { Dialog } from "./ui";

type Kind = "box" | "cylinder" | "cone" | "sphere";
const FIELDS: Record<Kind, [string, string, number][]> = {
  box: [["length", "Length (along X)", 40], ["width", "Width (along Y)", 30], ["height", "Height (along Z)", 20]],
  cylinder: [["radius", "Radius", 10], ["height", "Height (along Z)", 30]],
  cone: [["radius", "Base radius", 10], ["top_radius", "Top radius (0 for a point)", 0], ["height", "Height (along Z)", 30]],
  sphere: [["radius", "Radius", 10]],
};
const NAMES: Record<Kind, string> = { box: "Box", cylinder: "Cylinder", cone: "Cone", sphere: "Sphere" };

/** A box, cylinder, cone or sphere at a point, with its sizes. */
export const primitiveDialog = (kind: Kind) => openDialog(() => {
  const [vals, setVals] = createSignal<Record<string, number>>(Object.fromEntries(FIELDS[kind].map(([k, , v]) => [k, v])));
  const [at, setAt] = createSignal<[number, number, number]>([0, 0, 0]);
  const ok = async () => {
    const v = vals(), where = kind === "box" ? { corner: at() } : kind === "sphere" ? { center: at() } : { base: at() };
    if (Object.entries(v).some(([k, x]) => !(x > 0) && k !== "top_radius")) { toast("Every size must be more than 0."); return false; }
    pick3(null);
    if (!(await cadRun(`solid.${kind}`, { ...where, ...v }))) return false;
    setTurnBack(turnBack() + 1); // frame the model with the new solid in it
    return undefined;
  };
  return <Dialog title={NAMES[kind]} okText="Make" ok={ok} help="modelling">
    <table class="kv"><tbody>
      <For each={FIELDS[kind]}>{([k, l]) => <tr><td>{l}</td><td><input class="mono" type="number" value={vals()[k]} onInput={e => setVals({ ...vals(), [k]: +e.currentTarget.value })} /></td><td class="muted">mm</td></tr>}</For>
      <tr><td>{kind === "box" ? "Corner at" : kind === "sphere" ? "Centre at" : "Base centre at"}</td><td><span class="xrow">{[0, 1, 2].map(i => <input class="mono" style={{ width: "60px" }} type="number" value={at()[i]} onInput={e => { const a = [...at()] as [number, number, number]; a[i] = +e.currentTarget.value; setAt(a); }} />)}</span></td><td class="muted">X, Y, Z mm</td></tr>
    </tbody></table>
  </Dialog>;
});

/** Join, cut or intersect: one solid (the target) with others (the tools). */
export const combineDialog = () => {
  const names = (cad()?.bodies ?? []).map(b => b.name);
  if (names.length < 2) return toast("Combine needs two solids or more: make another (Extrude, or Primitives).");
  openDialog(() => {
    const [target, setTarget] = createSignal(names[0]), [tools, setTools] = createSignal<string[]>([names[1]]), [op, setOp] = createSignal<"join" | "cut" | "intersect">("join");
    const ok = async () => {
      const t = tools().filter(n => n !== target());
      if (!t.length) { toast("Pick at least one other solid to combine with."); return false; }
      return (await cadRun("solid.combine", { target: target(), tools: t, operation: op() })) ? undefined : false;
    };
    return <Dialog title="Combine solids" okText="Combine" ok={ok} help="modelling">
      <div class="seg"><For each={[["join", "Join"], ["cut", "Cut"], ["intersect", "Intersect"]] as const}>{([k, l]) => <button classList={{ on: op() === k }} onClick={() => setOp(k)}>{l}</button>}</For></div>
      <p class="muted small">{op() === "join" ? "The solids become one." : op() === "cut" ? "The others are cut out of the target, and go." : "Only what all of them share is kept."}</p>
      <table class="kv"><tbody>
        <tr><td>Target</td><td><select value={target()} onChange={e => setTarget(e.currentTarget.value)}><For each={names}>{n => <option value={n}>{n}</option>}</For></select></td></tr>
        <tr><td>With</td><td><For each={names}>{n => <Show when={n !== target()}><label class="chk"><input type="checkbox" checked={tools().includes(n)} onChange={e => setTools(e.currentTarget.checked ? [...tools(), n] : tools().filter(x => x !== n))} />{n}</label></Show>}</For></td></tr>
      </tbody></table>
    </Dialog>;
  });
  return closeDialog;
};
