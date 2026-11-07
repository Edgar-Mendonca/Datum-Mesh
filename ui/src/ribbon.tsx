// Title bar, quick access toolbar, Tell Me and the ribbon (Home · Geometry · Mesh · Loads · Solve ·
// Results · View · Help, with Format as a contextual tab on a selection). Copied from Datum Plan's
// ribbon by way of Datum Sim; planned tools are shown dimmed with the release they arrive in.
import { sectionsOn, setSectionsOn } from "./store";
import { memberTruss, setMemberTruss, memberPlates, setMemberPlates } from "./cad";
import { hasSolids, meshSolids } from "./study";
import { For, Show, JSX, createSignal, createMemo, onMount } from "solid-js";
import { holeD, setHoleD, edgeSize, setEdgeSize, sketch, solidRes, modes, modeOn, setModeOn, toast, call, out, model, tab, setTab, setBackstage, openMenu, openDialog, closeDialog, theme, setTheme, sel, opts, setOpts, tool, act, results, show, solving, partRes, setSel, setEdgeSel, view, setView, frameRes, spaceRes } from "./store";
import { fieldsFor, curField } from "./parts";
import { C, ALL_CMDS } from "./cmds";
import * as D from "./dialogs";
import * as K from "./sketch";
import { CasePicker, casesDialog, envelope, hasEnvelope } from "./cases";
import { stepsDialog, pathDialog, partPathDialog, isNonlinear, dynDialog, dynOf } from "./steps";
import { Icon, Dialog } from "./ui";
import { Help, helpCard, HELP } from "./help";
import { invoke, Example, isFrame, isBuckle, isSolidA } from "./api";
import { q3, setQ3, Q3 } from "./space3";
import { setPlane, onPlane, plateT, setPlateT } from "./plane3";
import { parseNum } from "./api";
import { toU, fromU, unit } from "./units";
import { setPlanePick } from "./cad";
import { cad3Tool, pick3, TOOLS3, holeD3, setHoleD3, holeDepth, setHoleDepth, fillet3R, setFillet3R, chamfer3D, setChamfer3D, shellT, setShellT, pressD, setPressD, bodyNames, draftA, setDraftA } from "./cad";
import { cad, cadRun, cadQuery, activeSketch, cadTool, pickTool, ways, setWays, TOOLS, FAMILIES, sides, setSides, slotW, setSlotW, roundR, setRoundR, EDITS, CONSTRAINTS, pickEdit, consMode, setConsMode, toggleConstruction, deleteSelected, rectPattern, sel as cadSel, filletR, setFilletR, chamferD, setChamferD, offsetD, setOffsetD, patN, setPatN, patA, setPatA, rowN, setRowN, rowS, setRowS, colN, setColN, colS, setColS, newSketch, finishSketch, cancelSketch, revolve, extrudeD, setExtrudeD, extrudeDir, setExtrudeDir, revolveA, setRevolveA, revolveAxis, setRevolveAxis } from "./cad";

const GH = "https://github.com/Edgar-Mendonca/Datum-Mesh";

const B = (p: { i: string; l: string; run?: (e: MouseEvent) => void; big?: boolean; on?: boolean; off?: boolean; dd?: (e: MouseEvent) => void; title?: string }) => p.big && p.run && p.dd ? (
  <div class="rbw big splitbig" classList={{ on: p.on }}>
    <button class="rb top" title={p.title ?? p.l} disabled={p.off} onClick={e => p.run!(e)}><Icon n={p.i} /></button>
    <button class="rb bot" title={`${p.l} options`} onClick={e => p.dd!(e)}><span>{p.l}</span><b class="car">▾</b></button>
  </div>
) : (
  <div class="rbw" classList={{ big: p.big }}>
    <button class="rb" classList={{ big: p.big, on: p.on }} disabled={p.off} title={p.title ?? p.l} onClick={e => (p.run ?? p.dd)?.(e)}><Icon n={p.i} />{p.big ? <span>{p.l}{p.dd && !p.run ? " ▾" : ""}</span> : <span>{p.l}</span>}{!p.big && p.dd && !p.run && <b class="car">▾</b>}</button>
    {p.run && p.dd && <button class="rb split" onClick={e => p.dd!(e)}>▾</button>}
  </div>
);
const G = (p: { l: string; h?: string; children: JSX.Element }) => <div class="grp"><div class="items">{p.children}</div><div class="cap">{p.l}{p.h && <Help k={p.h} />}</div></div>;
const S = (p: { children: JSX.Element }) => <div class="stack">{p.children}</div>;
/** A length typed on the ribbon (shown units), kept in metres. */
const Num = (p: { l: string; title: string; v: () => number; set: (v: number) => void }) => (
  <label class="ribnum" title={p.title}>{p.l}<input class="mono" value={toU("len", p.v())} onChange={e => { const v = parseNum(e.currentTarget.value); if (v !== null && v > 0) p.set(fromU("len", v)); else e.currentTarget.value = String(toU("len", p.v())); }} /><span class="unit">{unit("len")}</span></label>
);
const Mo = () => import("./modelling");
/** A planned tool: shown dimmed with the release it arrives in. */
const Planned = (p: { i: string; l: string; v: string; big?: boolean }) => <B i={p.i} l={p.l} big={p.big} off title={`${p.l}: planned for ${p.v}`} />;
/** A contour field button: shows that result on the part. */
/** Contour results of a part, 2D or 3D. */
const pres = () => !!partRes() || !!solidRes();
const F = (p: { k: string; l: string; big?: boolean }) => <B i="contour" l={p.l} big={p.big} on={pres() && curField().key === p.k} off={!pres()} run={() => C.show({ field: p.k })} />;
/** Element size: automatic, or the current size made coarser or finer. */
const sizeMenu = (e: MouseEvent) => {
  const m = model(); if (!m) return;
  const now = m.mesh.size > 0 ? m.mesh.size : partRes()?.mesh.size ?? 0;
  const at = (f: number) => () => { if (now > 0) C.meshSettings({ size: now * f }); else toastAuto(); };
  openMenu(e, [
    { label: "Automatic", on: m.mesh.size === 0, run: () => C.meshSettings({ size: 0 }) },
    { label: "Coarser (× 2)", run: at(2) }, { label: "Finer (÷ 2)", run: at(0.5) }, { label: "Much finer (÷ 4)", run: at(0.25) },
    "-", { label: "Type a size…", run: () => { setSel([]); setEdgeSel(null); C.toastSize(); } },
  ]);
};
const toastAuto = () => C.mesh().then(() => C.toastSize());
/** A canvas tool: highlighted while it is picked. */
const T = (p: { i: string; l: string; t: string; big?: boolean; title?: string }) => <B i={p.i} l={p.l} big={p.big} on={tool() === p.t} title={p.title ?? p.l} run={() => C.tool(p.t)} />;

/** A whole number or a length typed on the ribbon, in the sketch's millimetres. */
const MmNum = (p: { l: string; title: string; v: () => number; set: (v: number) => void; u?: string; int?: boolean }) => (
  <label class="ribnum" title={p.title}>{p.l}<input class="mono" value={p.v()} onChange={e => { const v = parseNum(e.currentTarget.value); if (v !== null && v > 0 && (!p.int || (Number.isInteger(v) && v >= 3))) p.set(v); else e.currentTarget.value = String(p.v()); }} /><span class="unit">{p.u ?? "mm"}</span></label>
);

/** The constraint the Constrain button applies (the last one picked from its ▾). */
const [conWay, setConWay] = createSignal("coincident");
type Field = [string, string, boolean, () => number, (v: number) => void];
/** The sizes the tool in use takes, shown under Sizes (and only those). */
const sizeFields = (): Field[] => {
  const t = cadTool() ?? "";
  if (t.startsWith("poly")) return [["Sides", "", true, sides, setSides]];
  if (t.startsWith("slot")) return [["Slot width", "mm", false, slotW, setSlotW]];
  if (t === "rectr") return [["Corner radius", "mm", false, roundR, setRoundR]];
  if (t === "fillet") return [["Radius", "mm", false, filletR, setFilletR]];
  if (t === "chamfer") return [["Distance", "mm", false, chamferD, setChamferD]];
  if (t === "offset") return [["Distance", "mm", false, offsetD, setOffsetD]];
  if (t === "cpattern") return [["Copies", "", true, patN, setPatN], ["Angle", "°", false, patA, v => setPatA(Math.min(360, v))]];
  if (!t || t === "select") return [["Rows", "", true, rowN, setRowN], ["Row spacing", "mm", false, rowS, setRowS], ["Columns", "", true, colN, (v: number) => setColN(Math.max(1, v))], ["Column spacing", "mm", false, colS, setColS]].slice(0, cadSel().length ? 4 : 0) as Field[];
  return [];
};
/** The sizes of the 3D tool in use (or of Extrude and Revolve when none is). */
const size3 = (): Field[] => {
  const t = cad3Tool();
  if (t === "hole3") return [["Ø", "mm", false, holeD3, setHoleD3], ["Depth (0: through)", "mm", false, holeDepth, v => setHoleDepth(Math.max(0, v))]];
  if (t === "fillet3") return [["Radius", "mm", false, fillet3R, setFillet3R]];
  if (t === "chamfer3") return [["Distance", "mm", false, chamfer3D, setChamfer3D]];
  if (t === "shell3") return [["Wall", "mm", false, shellT, setShellT]];
  if (t === "draft3") return [["Draft angle", "°", false, draftA, setDraftA]];
  if (t === "press3") return [["Distance", "mm", false, pressD, setPressD]];
  if (t) return [];
  return [["Extrude", "mm", false, extrudeD, setExtrudeD], ["Revolve", "°", false, revolveA, v => setRevolveA(Math.min(360, v))]];
};
/** Ticks for the solids an Arrange tool works on (all of them to start with). */
const BodyTicks = (p: { on: () => string[]; set: (v: string[]) => void }) => (
  <div class="row wrap"><For each={bodyNames()}>{n => <label class="chk"><input type="checkbox" checked={p.on().includes(n)} onChange={e => p.set(e.currentTarget.checked ? [...p.on(), n] : p.on().filter(x => x !== n))} />{n}</label>}</For></div>
);
const NumIn = (p: { l: string; v: () => number; set: (v: number) => void; u?: string }) => (
  <label class="fld">{p.l}<input class="mono" value={p.v()} onChange={e => { const v = parseNum(e.currentTarget.value); if (v !== null) p.set(v); else e.currentTarget.value = String(p.v()); }} /><span class="unit">{p.u ?? "mm"}</span></label>
);
const needBodies = () => { if (!bodyNames().length) { toast("Make a solid first: New Sketch, a closed shape, then Extrude."); return false; } return true; };
/** Arrange › Move / Copy. */
const moveDialog = () => {
  if (!needBodies()) return;
  const [bs, setBs] = createSignal(bodyNames()), [x, setX] = createSignal(0), [y, setY] = createSignal(0), [z, setZ] = createSignal(0), [a, setA] = createSignal(0), [copy, setCopy] = createSignal(false);
  openDialog(() => <Dialog title="Move / Copy solids" help="modelling">
    <BodyTicks on={bs} set={setBs} />
    <div class="row"><NumIn l="X" v={x} set={setX} /><NumIn l="Y" v={y} set={setY} /><NumIn l="Z" v={z} set={setZ} /><NumIn l="Turn about Z" u="°" v={a} set={setA} /></div>
    <label class="chk"><input type="checkbox" checked={copy()} onChange={e => setCopy(e.currentTarget.checked)} />Make a copy (keep the originals where they are)</label>
    <div class="row end"><button class="btn pri" onClick={async () => { if (await cadRun("solid.move", { bodies: bs(), translate: [x(), y(), z()], ...(a() ? { axis: [0, 0, 1], angle: a() } : {}), copy: copy() })) closeDialog(); }}>OK</button></div>
  </Dialog>);
};
/** Arrange › Mirror. */
const mirrorDialog = () => {
  if (!needBodies()) return;
  const [bs, setBs] = createSignal(bodyNames()), [pl, setPl] = createSignal<"YZ" | "XZ" | "XY">("YZ"), [join, setJoin] = createSignal(false);
  openDialog(() => <Dialog title="Mirror solids" help="modelling">
    <BodyTicks on={bs} set={setBs} />
    <div class="row"><span>In the plane</span><For each={[["YZ", "Y–Z (across X)"], ["XZ", "X–Z (across Y)"], ["XY", "X–Y (across Z)"]] as const}>{([k, l]) => <label class="chk"><input type="radio" name="mpl" checked={pl() === k} onChange={() => setPl(k)} />{l}</label>}</For></div>
    <label class="chk"><input type="checkbox" checked={join()} onChange={e => setJoin(e.currentTarget.checked)} />Join the copy to the original</label>
    <div class="row end"><button class="btn pri" onClick={async () => { if (await cadRun("solid.mirror", { bodies: bs(), plane: pl(), combine: join() })) closeDialog(); }}>OK</button></div>
  </Dialog>);
};
/** Arrange › Pattern: rows and columns, or copies round an axis. */
const patternDialog = () => {
  if (!needBodies()) return;
  const [bs, setBs] = createSignal(bodyNames()), [kind, setKind] = createSignal<"rect" | "circ">("rect");
  const [n1, setN1] = createSignal(3), [s1, setS1] = createSignal(50), [n2, setN2] = createSignal(1), [s2, setS2] = createSignal(50), [n, setN] = createSignal(6), [ax, setAx] = createSignal<"X" | "Y" | "Z">("Z"), [ang, setAng] = createSignal(360);
  openDialog(() => <Dialog title="Pattern of solids" help="modelling">
    <BodyTicks on={bs} set={setBs} />
    <div class="row"><label class="chk"><input type="radio" name="pk" checked={kind() === "rect"} onChange={() => setKind("rect")} />Rows and columns</label><label class="chk"><input type="radio" name="pk" checked={kind() === "circ"} onChange={() => setKind("circ")} />Round an axis</label></div>
    <Show when={kind() === "rect"} fallback={<div class="row"><NumIn l="Copies (with the original)" u="" v={n} set={setN} /><NumIn l="Over" u="°" v={ang} set={setAng} /><span>about</span><For each={["X", "Y", "Z"] as const}>{k => <label class="chk"><input type="radio" name="pax" checked={ax() === k} onChange={() => setAx(k)} />{k}</label>}</For></div>}>
      <div class="row"><NumIn l="Along X: count" u="" v={n1} set={setN1} /><NumIn l="spacing" v={s1} set={setS1} /></div>
      <div class="row"><NumIn l="Along Y: count" u="" v={n2} set={setN2} /><NumIn l="spacing" v={s2} set={setS2} /></div></Show>
    <div class="row end"><button class="btn pri" onClick={async () => {
      const ok = kind() === "rect" ? await cadRun("solid.pattern.rectangular", { bodies: bs(), dir1: [1, 0, 0], count1: n1(), spacing1: s1(), ...(n2() > 1 ? { dir2: [0, 1, 0], count2: n2(), spacing2: s2() } : {}) })
        : await cadRun("solid.pattern.circular", { bodies: bs(), axis: ax(), count: n(), angle: ang() });
      if (ok) closeDialog(); }}>OK</button></div>
  </Dialog>);
};
/** Construct › Offset Plane, then a sketch on it. */
const offsetPlaneDialog = () => {
  const [base, setBase] = createSignal<"XY" | "XZ" | "YZ">("XY"), [off, setOff] = createSignal(20);
  openDialog(() => <Dialog title="Offset plane" help="sketchplane">
    <div class="row"><span>Parallel to</span><For each={[["XY", "the floor (XY)"], ["XZ", "the front (XZ)"], ["YZ", "the side (YZ)"]] as const}>{([k, l]) => <label class="chk"><input type="radio" name="opb" checked={base() === k} onChange={() => setBase(k)} />{l}</label>}</For></div>
    <div class="row"><NumIn l="Set off by" v={off} set={setOff} /></div>
    <div class="row end"><button class="btn pri" onClick={async () => {
      const r = await cadQuery<any>("construct.plane.offset", { base: base(), offset: off() }); if (!r) return;
      const name = r.name ?? r.plane ?? r.created?.[0] ?? r.id;
      closeDialog(); pick3(null);
      if (await cadRun("sketch.create", { plane: name })) pickTool(ways().Line);
    }}>Sketch on it</button></div>
  </Dialog>);
};

/** Inspect › Sketch Report: what the engine says of the open sketch. */
const sketchReport = async () => {
  const r = await cadQuery<any>("sketch.inspect"); if (!r) return;
  const sk = activeSketch();
  openDialog(() => <Dialog title={`Sketch report: ${sk?.name ?? ""}`} help="sketchplane">
    <table class="kv"><tbody>
      <tr><td>State</td><td>{sk?.status.replace("Solved, ", "").replace("Solved", "fully defined")}</td></tr>
      <tr><td>Curves</td><td>{(r.curves ?? []).length} ({(r.curves ?? []).filter((c: any) => c.fully_constrained).length} fully defined)</td></tr>
      <tr><td>Points</td><td>{(r.points ?? []).length}</td></tr>
      <tr><td>Closed regions</td><td>{sk?.profiles ?? 0}</td></tr>
      <tr><td>Dimensions</td><td>{(sk?.marks ?? []).filter(m => m.value !== null).map(m => `${m.name} ${+m.value!.toPrecision(5)}`).join(", ") || "none"}</td></tr>
      <tr><td>Constraints</td><td>{(sk?.marks ?? []).filter(m => m.value === null).map(m => m.name).join(", ") || "none"}</td></tr>
    </tbody></table>
    <p class="muted">Blue curves are still free to move; black ones are fixed by dimensions and constraints.</p>
  </Dialog>);
};
/** Configure › Parameters: named sizes for dimensions and features. */
const parametersDialog = async () => {
  const [list, setList] = createSignal<any[]>([]);
  const load = async () => { const r = await cadQuery<any>("parameters.list"); setList(Array.isArray(r) ? r : (r?.parameters ?? [])); };
  await load();
  const [name, setName] = createSignal(""), [expr, setExpr] = createSignal("");
  openDialog(() => <Dialog title="Parameters" help="sketchplane">
    <table class="kv"><thead><tr><th>Name</th><th>Expression</th><th>Value</th></tr></thead><tbody>
      <For each={list()}>{x => <tr><td class="mono">{x.name}</td><td><input class="mono" value={x.expression ?? x.expr ?? ""} onChange={async e => { if (await cadRun("parameters.change", { name: x.name, expression: e.currentTarget.value })) load(); }} /></td><td class="mono">{x.value ?? ""}</td></tr>}</For>
      <Show when={!list().length}><tr><td colspan={3} class="muted">No parameters yet: add one below, then type its name in a dimension (W / 2).</td></tr></Show>
    </tbody></table>
    <div class="row"><input class="mono" placeholder="name (W)" value={name()} onInput={e => setName(e.currentTarget.value)} /><input class="mono" placeholder="expression (40 mm)" value={expr()} onInput={e => setExpr(e.currentTarget.value)} />
      <button class="btn pri sm" onClick={async () => { if (name() && expr() && await cadRun("parameters.add", { name: name(), expression: expr() })) { setName(""); setExpr(""); load(); } }}>Add</button></div>
  </Dialog>);
};

/** 0.7.1: the geometry engine's tools on the Geometry ribbon of a 3D solid. In a sketch: the
 * shapes, each with its ways of drawing under ▾ (the way last used stays on the button), their
 * sizes, Finish and Cancel. Outside one: New Sketch on a plane, Extrude, Revolve, Undo, Redo. */
const CadGroups = () => {
  const inSk = () => !!activeSketch();
  const fam = (f: [string, string, string[]]) => {
    const [name, icon, list] = f, cur = () => ways()[name] ?? list[0];
    return <B big i={icon} l={name} on={list.includes(cadTool() ?? "")} title={`${TOOLS[cur()].label}${list.length > 1 ? " (▾ for the other ways)" : ""}`} run={() => pickTool(cur())}
      dd={list.length > 1 ? (e: MouseEvent) => openMenu(e, list.map(k => ({ label: TOOLS[k].label, on: k === cur(), run: () => { setWays({ ...ways(), [name]: k }); pickTool(k); } }))) : undefined} />;
  };
  return <>
    <Show when={inSk()} fallback={<>
      <G l="Sketch" h="sketchplane"><B big i="new" l="New Sketch" run={() => { pick3(null); setPlanePick(true); }} title="Start a sketch: click a plane at the origin or a flat face of a solid (▾: a named plane, or a plane set off from one)"
        dd={e => openMenu(e, [...([["On the floor (XY)", "XY"], ["On the front (XZ)", "XZ"], ["On the side (YZ)", "YZ"]] as const).map(([l, pl]) => ({ label: l, run: () => { pick3(null); newSketch(pl); } })),
          { label: "On a face of a solid…", run: () => pick3("sketchface") }, { label: "On an offset plane…", run: offsetPlaneDialog }])} /></G>
      <G l="Create"><B big i="solid" l="Extrude" run={() => { pick3(null); import("./extrude").then(k => k.openExtrude()); }} title="Extrude the last sketch's closed regions into a solid (distance and direction under Sizes)" />
        <B big i="axi" l="Revolve" run={() => { pick3(null); revolve(); }} title="Turn the last sketch's closed regions about one of its axes into a solid" />
        <B big i={TOOLS3.hole3.icon} l="Hole" on={cad3Tool() === "hole3"} run={() => pick3("hole3")} title={TOOLS3.hole3.hint} />
        <S><B i="curve" l="Sweep" run={() => { pick3(null); import("./solidtools").then(k => k.sweepDialog()); }} title="Carry a closed profile along a path drawn in another sketch (pipes, rails, springs)" />
          <B i="poly" l="Loft" run={() => { pick3(null); import("./solidtools").then(k => k.loftDialog()); }} title="A solid through closed sections drawn on different planes (a square base to a round top)" /></S></G>
      <G l="Primitives" h="modelling"><B big i="solid" l="Box" run={() => import("./primitives").then(k => k.primitiveDialog("box"))} title="A box: its corner and its length, width and height" dd={e => openMenu(e, ([["cylinder", "Cylinder"], ["cone", "Cone"], ["sphere", "Sphere"]] as const).map(([k, l]) => ({ label: `${l}…`, run: () => import("./primitives").then(m => m.primitiveDialog(k)) })))} />
        <B big i="union" l="Combine" run={() => import("./primitives").then(k => k.combineDialog())} title="Join, cut or intersect solids" /></G>
      <G l="Modify"><For each={["fillet3", "chamfer3", "shell3", "press3"]}>{k => <B big i={TOOLS3[k].icon} l={TOOLS3[k].label} on={cad3Tool() === k} run={() => pick3(k)} title={TOOLS3[k].hint} />}</For>
        <S><B i={TOOLS3.draft3.icon} l="Draft" on={cad3Tool() === "draft3"} run={() => pick3("draft3")} title={TOOLS3.draft3.hint} />
          <B i="cut" l="Split" run={() => { pick3(null); import("./solidtools").then(k => k.splitDialog()); }} title="Cut a solid in two by a plane: each piece a solid of its own" /></S></G>
      <G l="Inspect" h="modelling"><B big i="dist" l="Measure" on={cad3Tool() === "measure3"} run={() => pick3("measure3")} title={TOOLS3.measure3.hint} />
        <S><B i="info" l="Mass" run={() => import("./solidtools").then(k => k.massDialog())} title="Each solid's volume, area, centre of mass and mass" /></S></G>
      <G l="Arrange"><B big i="copy" l="Move / Copy" run={moveDialog} title="Move, turn or copy solids" />
        <B big i="symmetry" l="Mirror" run={mirrorDialog} title="Mirror solids in a plane, as a copy (or joined)" />
        <B big i="pattern" l="Pattern" run={patternDialog} title="Copies of solids in rows and columns, or round an axis" /></G>
      <Show when={size3().length || !cad3Tool()}><G l="Sizes"><S><For each={size3()}>{f => <MmNum l={f[0]} u={f[1]} int={f[2]} title={f[0]} v={f[3]} set={f[4]} />}</For>
        <Show when={!cad3Tool()}><B i="up" l={({ positive: "Out of the sketch", negative: "Into it", symmetric: "Both ways evenly" } as const)[extrudeDir()]} dd={e => openMenu(e, ([["Out of the sketch", "positive"], ["Into it", "negative"], ["Both ways evenly", "symmetric"]] as const).map(([l, k]) => ({ label: l, on: extrudeDir() === k, run: () => setExtrudeDir(k) })))} />
          <B i="axi" l={`Revolve about the sketch's ${revolveAxis()} axis`} dd={e => openMenu(e, (["x", "y"] as const).map(k => ({ label: `Revolve about the sketch's ${k} axis`, on: revolveAxis() === k, run: () => setRevolveAxis(k) })))} /></Show></S></G></Show>
      <G l="Construct"><S><B i="grid" l="Offset Plane" run={offsetPlaneDialog} title="A plane parallel to the floor, front or side, set off by a distance: sketch on it" />
        <B i="fx" l="Parameters" run={parametersDialog} title="Named sizes (W = 40) that dimensions and features can use" /></S></G>
      <G l="Edit"><S><B i="undo" l="Undo" off={!cad()?.undo} run={() => cadRun("edit.undo")} /><B i="redo" l="Redo" off={!cad()?.redo} run={() => cadRun("edit.redo")} /><B i="select" l="Select" on={!cad3Tool()} run={() => pick3(null)} /></S></G>
    </>}>
      <G l="Create"><For each={FAMILIES}>{fam}</For>
        <S><B i="dist" l="Construction" on={consMode()} run={() => setConsMode(!consMode())} title="Draw the next shapes as construction (dashed): guides that dimensions and constraints can use, not part of the solid" /></S></G>
      <G l="Modify"><For each={["trim", "extend", "offset", "fillet", "chamfer"]}>{k => <B big i={EDITS[k].icon} l={EDITS[k].label} on={cadTool() === k} run={() => pickEdit(k)} title={EDITS[k].hint.join(", then ")} />}</For></G>
      <G l="Pattern"><B big i="symmetry" l="Mirror" on={cadTool() === "mirror"} run={() => pickEdit("mirror")} title="Mirror the selected curves in a line" />
        <B big i="cpattern" l="Circular" on={cadTool() === "cpattern"} run={() => pickEdit("cpattern")} title="Copy the selected curves around a centre" />
        <B big i="pattern" l="Rectangular" run={() => { pickEdit(null); rectPattern(); }} title="Copy the selected curves in rows (and columns), by the counts and spacings under Sizes" /></G>
      <G l="Constrain"><B big i="dist" l="Dimension" on={cadTool() === "dimension"} run={() => pickEdit("dimension")} title="Size a line, circle or arc (or the distance or angle between two items): type the value" />
        <B big i="pin" l={EDITS[conWay()].label} on={CONSTRAINTS.includes(cadTool() ?? "")} run={() => pickEdit(conWay())} title={`${EDITS[conWay()].label}: ${EDITS[conWay()].hint.join(", then ")} (▾ for the others)`}
          dd={e => openMenu(e, CONSTRAINTS.map(k => ({ label: EDITS[k].label, on: k === conWay(), run: () => { setConWay(k); pickEdit(k); } })))} />
        <S><B i="tag" l="Construction" run={toggleConstruction} title="Turn the selected curves into construction, or back" /><B i="del" l="Delete" run={deleteSelected} title="Delete the selected curves (Delete key)" /></S></G>
      <Show when={sizeFields().length}><G l="Sizes"><S><For each={sizeFields()}>{f => <MmNum l={f[0]} u={f[1]} int={f[2]} title={f[0]} v={f[3]} set={f[4]} />}</For></S></G></Show>
      <G l="Inspect"><S><B i="info" l="Sketch Report" run={sketchReport} title="What is still free in the sketch, its dimensions and constraints, loose ends" /><B i="fx" l="Parameters" run={parametersDialog} title="Named sizes (W = 40) that dimensions and features can use" /></S></G>
      <G l="Exit"><B big i="check" l="Finish Sketch" run={() => finishSketch()} title="Keep the sketch and leave it (then Extrude or Revolve)" /><B big i="clear" l="Cancel" run={() => cancelSketch()} title="Leave the sketch without keeping it" />
        <S><B i="undo" l="Undo" off={!cad()?.undo} run={() => cadRun("edit.undo")} /><B i="select" l="Select" on={!cadTool() || cadTool() === "select"} run={() => pickEdit("select")} /></S></G>
    </Show>
  </>;
};

/** Tell Me / command search (Ctrl+K). */
export const palette = () => openDialog(() => {
  const [q, setQ] = createSignal(""), [k, setK] = createSignal(0);
  const list = createMemo(() => { const s = q().toLowerCase(); return ALL_CMDS.filter(c => c[0].toLowerCase().includes(s)).slice(0, 14); });
  const go = (i: number) => { const c = list()[i]; if (c) { closeDialog(); setTimeout(() => c[1]()); } };
  let input!: HTMLInputElement;
  onMount(() => input.focus());
  return (
    <div class="ov top" onMouseDown={e => e.target === e.currentTarget && closeDialog()}>
      <div class="pal">
        <input ref={input} placeholder="Tell me what you want to do" value={q()} onInput={e => { setQ(e.currentTarget.value); setK(0); }}
          onKeyDown={e => { if (e.key === "ArrowDown") { e.preventDefault(); setK(Math.min(list().length - 1, k() + 1)); } else if (e.key === "ArrowUp") { e.preventDefault(); setK(Math.max(0, k() - 1)); } else if (e.key === "Enter") { e.preventDefault(); go(k()); } else if (e.key === "Escape") closeDialog(); }} />
        <div class="pl"><For each={list()}>{(c, i) => <button classList={{ on: i() === k() }} onMouseEnter={() => setK(i())} onClick={() => go(i())}><span>{c[0]}</span>{c[2] && <kbd>{c[2]}</kbd>}</button>}</For></div>
      </div>
    </div>
  );
});

export const TitleBar = (p: { update: string | null }) => (
  <header class="top">
    <div class="bar">
      <div class="qat">
        <img src="logo.svg" alt="" class="logo" />
        <button title="Save (Ctrl+S)" onClick={() => C.save()}><Icon n="save" /></button>
        <button title="Undo (Ctrl+Z)" disabled={!out()?.undo} onClick={C.undo}><Icon n="undo" /></button>
        <button title="Redo (Ctrl+Y)" disabled={!out()?.redo} onClick={C.redo}><Icon n="redo" /></button>
        <button title="Print (Ctrl+P)" onClick={C.print}><Icon n="print" /></button>
      </div>
      <div class="title">{out() ? `${out()!.m.name}${out()!.dirty ? " •" : ""} — Datum Mesh` : "Datum Mesh"}</div>
      <button class="search" onClick={palette} title="Tell me what you want to do (Ctrl+K)"><Icon n="search" /><span>Tell me what you want to do</span><kbd>Ctrl+K</kbd></button>
      <Show when={p.update}><button class="upd" onClick={() => setBackstage("about")}>Update {p.update} available</button></Show>
      <button class="theme" title="Theme: Auto follows Windows" onClick={() => setTheme(theme() === "auto" ? "light" : theme() === "light" ? "dark" : "auto")}>Theme · {theme()}</button>
    </div>
    <nav class="tabs">
      <button class="file" onClick={() => setBackstage("home")}>File</button>
      <Show when={sketch()}><button class="sktab" classList={{ on: tab() === "sketch" }} onClick={() => setTab("sketch")}>Sketch</button></Show>
      <button classList={{ on: tab() === "home" }} onClick={() => setTab("home")}>Home</button>
      {/* 0.7.1: the steps are chosen in the tree on the left; the ribbon shows the tools of the step chosen. */}
      <Show when={STEP_NAMES[tab()]}><button class="on steptools" title="The tools of the step chosen in the tree on the left">{STEP_NAMES[tab()]}</button></Show>
      <For each={[["view", "View"], ["help", "Help"]]}>{([k, l]) => <button classList={{ on: tab() === k }} onClick={() => setTab(k)}>{l}</button>}</For>
      <div class="ctxgrp" style={{ visibility: fmtKind() ? "visible" : "hidden" }}><small>{fmtKind() === "shape" ? "Shape Tools" : "Member Tools"}</small><button classList={{ on: tab() === "format" }} onClick={() => setTab("format")}>Format</button></div>
    </nav>
  </header>
);

/** The ribbon's label for each step's tools (the steps are in the tree). */
const STEP_NAMES: Record<string, string> = { geometry: "Geometry", properties: "Properties", loads: "Supports & Loads", mesh: "Mesh", solve: "Analysis", results: "Results" };

/** What the Format tab works on: the selected shapes or members. */
const fmtKind = () => {
  const m = model(), s = new Set(sel()); if (!m || !s.size) return null;
  if (m.shapes.some(x => s.has(x.id))) return "shape";
  if (m.members.some(x => s.has(x.id))) return "member";
  return null;
};

/** Shrinks the ribbon in steps instead of scrolling sideways (FAMILY §6). */
const fitRibbon = (el: HTMLElement) => {
  const fit = () => {
    const gs = [...el.querySelectorAll<HTMLElement>(":scope > .grp")];
    el.dataset.fit = "0"; gs.forEach(g => delete g.dataset.fit);
    const over = () => el.scrollWidth > el.clientWidth + 1;
    if (!over()) return;
    el.dataset.fit = "1";
    for (const lvl of ["2", "3"]) for (let i = gs.length - 1; i >= 0 && over(); i--) gs[i].dataset.fit = lvl;
  };
  new ResizeObserver(fit).observe(el);
  new MutationObserver(() => requestAnimationFrame(fit)).observe(el, { childList: true, subtree: false });
};

const exampleMenu = async (e: MouseEvent) => {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const list = await invoke<Example[]>("examples_list").catch(() => [] as Example[]);
  // 0.6.1: the blank models, then each heading of File › Examples as a submenu.
  const groups: string[] = [];
  for (const x of list) if (x.group && x.group !== "Blank" && !groups.includes(x.group)) groups.push(x.group);
  openMenu({ x: r.left, y: r.bottom + 2 }, [
    ...list.filter(x => (x.group ?? "Blank") === "Blank").map(x => ({ label: x.name, run: () => C.newModel(x.id) })),
    "-",
    ...groups.map(g => ({ label: g, sub: list.filter(x => x.group === g).map(x => ({ label: x.name, run: () => C.newModel(x.id) })) })),
    "-",
    { label: "All examples, with what each shows…", run: () => setBackstage("examples") },
  ]);
};

/** Join the selected members where they cross (crossing members are otherwise left apart). */
const joinCrossings = () => { const m = model()!, s = new Set(sel()), ids = m.members.filter(b => s.has(b.id)).map(b => b.id); if (ids.length < 2) return toast("Select two or more members that cross."); act({ JoinCrossings: { ids } }).then(o => o && toast(`Joined at ${o.created.length} ${o.created.length === 1 ? "crossing" : "crossings"}.`, true)); };
/** Format: the selected shapes or members, all at once. */
const formatShapes = (f: (s: any) => object) => { const m = model()!, s = new Set(sel()); act({ Batch: m.shapes.filter(x => s.has(x.id)).map(x => ({ Set: { item: { Shape: { ...x, ...f(x) } } } })) }); };
const formatMembers = (f: (s: any) => object) => { const m = model()!, s = new Set(sel()); act({ Batch: m.members.filter(x => s.has(x.id)).map(x => ({ Set: { item: { Member: { ...x, ...f(x) } } } })) }); };

/** Sketch tools on the selected shapes (Geometry and Format tabs). */
/** The shapes a sketch is drawn with (plane parts, and a part's sketch, 0.6.2). */
const SketchDraw = () => <>
          <G l="Part" h="shapes"><T big i="rect" l="Rectangle" t="rect" /><S><T i="circle" l="Circle" t="circle" /><T i="poly" l="Polygon" t="poly" /></S></G>
          <G l="Holes" h="holes"><S><T i="hrect" l="Rectangular Hole" t="hrect" /><T i="hcircle" l="Round Hole" t="hcircle" /></S></G>
          <G l="Ready-made" h="sketch"><S><B i="slot" l="Slot…" run={() => K.makeShape("slot")} /><B i="rrect" l="Rounded Rectangle…" run={() => K.makeShape("rounded")} /><B i="ngon" l="Regular Polygon…" run={() => K.makeShape("polygon")} /></S></G>
          <SketchTools />
</>;

/** Plates drawn on a plane of a 3D frame (0.4.1). */
const FramePlates = () => <G l="Plates" h="plates"><T big i="rect" l="Plate" t="rect" title="Draw a rectangular plate on this plane: click two corners" /><S><T i="poly" l="Polygon Plate" t="poly" /><T i="circle" l="Round Plate" t="circle" /><T i="hrect" l="Opening" t="hrect" title="Cut a rectangular opening out of the plates on this plane" /></S>
            <S><label class="ribnum" title="Thickness of the plates drawn next (each plate's own is in the Inspector)">Thickness<input class="mono" value={toU("len", plateT())} onChange={e => { const v = parseNum(e.currentTarget.value); if (v !== null && v > 0) setPlateT(fromU("len", v)); else e.currentTarget.value = String(toU("len", plateT())); }} /><span class="unit">{unit("len")}</span></label></S></G>;

const SketchTools = () => <>
  <G l="Combine" h="sketch"><S><B i="union" l="Union" run={() => K.combine("union")} title="Join the two selected shapes into one" /><B i="subtract" l="Subtract" run={() => K.combine("subtract")} title="Cut the second selected shape out of the first" /><B i="intersect" l="Intersect" run={() => K.combine("intersect")} title="Keep only where the two selected shapes overlap" /></S></G>
  <G l="Modify" h="sketch"><S><B i="fillet" l="Round Corners…" run={() => K.corners(true)} title="Fillet: round corners of the selected polygon" /><B i="chamfer" l="Bevel Corners…" run={() => K.corners(false)} title="Chamfer: bevel corners of the selected polygon" /><B i="offset" l="Offset…" run={K.offset} title="A new outline parallel to the selected shape's" /></S>
    <S><B i="pattern" l="Linear Pattern…" run={() => K.pattern(false)} title="Copies of the selected shapes in a row" /><B i="cpattern" l="Circular Pattern…" run={() => K.pattern(true)} title="Copies of the selected shapes around a centre" /></S></G>
</>;

export const Ribbon = () => {
  const m = () => model();
  const frame = () => isFrame(m()?.analysis), heat = () => m()?.analysis === "heat" || m()?.analysis === "solidheat", sp = () => m()?.analysis === "space", solid3 = () => isSolidA(m()?.analysis);
  return (
    <section class="ribbon" ref={fitRibbon}>
      <Show when={tab() === "home"}>
        <G l="Model" h="analysis"><S><B i="frame" l="Frame or truss" on={m()?.analysis === "frame"} run={() => C.analysis("frame")} /><B i="pstress" l="Plane stress" on={m()?.analysis === "stress"} run={() => C.analysis("stress")} /></S>
          <S><B i="pstrain" l="Plane strain" on={m()?.analysis === "strain"} run={() => C.analysis("strain")} /><B i="heat" l="Heat conduction" on={m()?.analysis === "heat"} run={() => C.analysis("heat")} /></S>
          <S><B i="axi" l="Axisymmetric" on={m()?.analysis === "axi"} run={() => C.analysis("axi")} title="Axisymmetric: draw the half-section right of the y axis; it is turned a full circle about it (vessels, shafts, discs)" /><B i="space" l="3D Frame" on={sp()} run={() => C.analysis("space")} title="3D frames and trusses: members bend both ways and twist; drawn on planes, seen in 3D" /></S>
          <S><B i="solid" l="3D Solid" on={m()?.analysis === "solid"} run={() => C.analysis("solid")} title="3D solids: the part's section extruded along z or revolved about y, solved in 3D for stress and vibration" /><B i="heat3" l="3D Heat" on={m()?.analysis === "solidheat"} run={() => C.analysis("solidheat")} title="Heat conduction in a 3D solid, extruded or revolved: temperatures and heat flow in 3D" /></S></G>
        <G l="Units" h="units"><S><B i="units" l="SI: mm, kN, MPa" on={m()?.units === "SI"} run={() => C.units("SI")} /><B i="units" l="US: in, kip, ksi" on={m()?.units === "US"} run={() => C.units("US")} /></S></G>
        <G l="Select" h="select"><T big i="select" l="Select" t="select" /><S><B i="all" l="Select All" run={C.selectAll} /><B i="del" l="Delete" run={C.del} off={!sel().length} /></S></G>
        <G l="Examples" h="examples"><B big i="open" l="Examples" dd={exampleMenu} /></G>
        <G l="Solve" h="solve"><B big i="solve" l="Solve" run={C.solve} off={solving()} title="Solve the model (F5)" /></G>
      </Show>
      <Show when={tab() === "sketch" && sketch()}>{sk => <>
        <Show when={sk().kind === "part"} fallback={<G l="Draw" h="members"><T big i="beam" l="Line (Beam)" t="beam" title="Draw beams: click two points; it goes on from the last" /><T big i="truss" l="Truss Bar" t="truss" /><S><T i="node" l="Node" t="node" /><B i="node" l="Node at…" run={() => D.nodeAt()} /></S></G>}><SketchDraw /></Show>
        <Show when={sk().kind === "frame"}><FramePlates /></Show>
        <G l="Exit" h="sketchplane"><B big i="check" l="Finish Sketch" run={() => import("./sketchmode").then(k => k.finishSketch())} title="Keep the sketch" /><B big i="del" l="Cancel" run={() => import("./sketchmode").then(k => k.cancelSketch())} title="Throw the sketch away" /></G>
      </>}</Show>
      <Show when={tab() === "geometry"}>
        <Show when={frame()} fallback={<>
          <Show when={isSolidA(m()?.analysis) && !m()?.parts?.length}><CadGroups /></Show>
          <Show when={isSolidA(m()?.analysis) && !!m()?.parts?.length}><G l="Parts sketch" h="sketchplane"><B big i="new" l="New Part" run={() => import("./sketchmode").then(k => k.newSketch())} title="Pick a plane and sketch a new part on it; then extrude or revolve it" /></G></Show>
          <Show when={isSolidA(m()?.analysis) && m()?.parts?.length}>
            <G l="Features" h="modelling"><B big i="solid" l="Extrude" run={() => Mo().then(k => k.editSweep())} title="Extrude or revolve the selected part, with a preview: how far, and which way" />
              <S><T i="hcircle" l="Hole" t="hole3" title="Click a part's front or back face: a round hole right through it" /><T i="fillet" l="Fillet" t="fillet3" title="Click a part near an edge along its depth: the edge is rounded" /><T i="chamfer" l="Chamfer" t="chamfer3" title="Click a part near an edge along its depth: the edge is bevelled" /></S>
              <S><Num l="Ø" title="Diameter of the holes made next" v={holeD} set={setHoleD} /><Num l="Size" title="Radius of the next fillet, or how much the next chamfer cuts off each side" v={edgeSize} set={setEdgeSize} /></S>
              <B big i="subtract" l="Cut" run={() => Mo().then(k => k.cutParts())} title="Cut the selected part right through another: its outlines become holes there" /></G>
            <G l="Arrange" h="modelling"><S><B i="repeat" l="Move or Copy…" run={() => Mo().then(k => k.moveParts())} title="Move the selected parts, or copy them in a row" /><B i="cpattern" l="Turn or Pattern…" run={() => Mo().then(k => k.turnParts())} title="Turn the selected parts about an axis, or copy them round it" /><B i="symmetry" l="Mirror…" run={() => Mo().then(k => k.mirrorParts())} title="Mirror the selected parts in a plane" /></S></G>
          </Show>
          <Show when={!(isSolidA(m()?.analysis) && (m()?.parts?.length || !m()?.shapes.length))}><SketchDraw /></Show>

          <Show when={!isSolidA(m()?.analysis) || !!m()?.parts?.length || !!m()?.shapes.length}>
          <G l="Material" h="materials"><B big i="material" l="Materials" run={() => D.materials()} /></G></Show>
          <Show when={!activeSketch() && isSolidA(m()?.analysis)}><G l="Import" h="import"><B big i="import" l="STEP" run={() => import("./solidtools").then(k => k.importStep())} title="Bring in the solids of a STEP file from another CAD program, to mesh and analyse (▾: a mesh file, or a DXF section)"
              dd={e => openMenu(e, [{ label: "STEP file (solids)…", run: () => import("./solidtools").then(k => k.importStep()) }, ...([["coordinates in millimetres", 1e-3], ["coordinates in metres", 1], ["coordinates in inches", 0.0254]] as [string, number][]).map(([l, u]) => ({ label: `Mesh file, ${l} (.msh, .inp)…`, run: () => C.importMesh(u as number) })), { label: "DXF section (extruded)…", run: K.importDxf }])} />
            <B big i="export" l="Export" dd={e => openMenu(e, [{ label: "STEP file (solids)…", run: () => import("./solidtools").then(k => k.exportCad("step")) }, { label: "STL file (surface triangles)…", run: () => import("./solidtools").then(k => k.exportCad("stl")) }])} title="Save the solids as a STEP or STL file for other programs" /></G></Show>
          <Show when={!activeSketch() && !isSolidA(m()?.analysis)}><G l="Import" h="import"><B big i="import" l="DXF" run={K.importDxf} title="Import a section drawn in CAD (ASCII DXF): its closed outlines become shapes and holes" /><B big i="solid" l="Mesh" dd={(e: MouseEvent) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); openMenu({ x: r.left, y: r.bottom + 2 }, [["Coordinates in millimetres", 1e-3], ["Coordinates in metres", 1], ["Coordinates in inches", 0.0254]].map(([l, u]) => ({ label: `${l} (.msh, .inp)…`, run: () => C.importMesh(u as number) }))); }} title="A part of any shape: a tetrahedral mesh from a mesher (Gmsh .msh, CalculiX-format .inp), as a new 3D solid" /></G></Show>
        </>}>
          <Show when={sp() && activeSketch()}><CadGroups /></Show>
          <Show when={sp() && !activeSketch()}><G l="Sketch" h="sketchplane"><B big i="new" l="New Sketch" run={() => { pick3(null); setPlanePick(true); }} title="Draw the frame with the sketch tools: click a plane, draw lines (and arcs); Finish Sketch makes them members joined where they meet" />
            <S><B i="beam" l="Lines become Beams" on={!memberTruss()} run={() => setMemberTruss(false)} /><B i="truss" l="Lines become Truss Bars" on={memberTruss()} run={() => setMemberTruss(true)} /><B i="poly" l="Closed Shapes become Plates" on={memberPlates()} run={() => setMemberPlates(!memberPlates())} title="A closed outline drawn on its own (a rectangle, polygon or circle, its corners touching nothing else) becomes a plate (a slab or wall), not members round it" /></S><S><B i="new" l="Members & Plates" run={() => import("./sketchmode").then(k => k.newSketch())} title="Pick a plane and draw members and plates on it one by one (the plane sketch of 0.6.2)" /></S></G></Show>
          <Show when={!(sp() && activeSketch())}>
          <G l="Draw" h="members"><T big i="beam" l="Beam" t="beam" title="Draw beams: click two points; it continues from the last" /><T big i="truss" l="Truss Bar" t="truss" /><S><T i="node" l="Node" t="node" /><B i="node" l="Node at…" run={() => D.nodeAt()} /></S></G>
          <Show when={sp() && onPlane()}><FramePlates /></Show>
          <Show when={sp()}><G l="3D" h="space"><B big i="repeat" l="Repeat" run={() => D.repeat()} title="Copy the selected members along the depth and join the copies (bays of a building)" /></G></Show>
          <G l="Properties" h="sections"><B big i="section" l="Sections" run={() => D.sections()} /><B big i="material" l="Materials" run={() => D.materials()} /></G>
          </Show>
        </Show>
        <Show when={!activeSketch()}><G l="Snap" h="snap"><S><label class="chk"><input type="checkbox" checked={opts().snap} onChange={e => setOpts({ ...opts(), snap: e.currentTarget.checked })} />Snap to grid</label><label class="chk"><input type="checkbox" checked={opts().grid} onChange={e => setOpts({ ...opts(), grid: e.currentTarget.checked })} />Show grid</label></S></G></Show>
      </Show>
      <Show when={tab() === "mesh"}>
        <Show when={hasSolids()}>
          <G l="Mesh" h="mesh"><B big i="mesh" l="Mesh" run={() => meshSolids()} off={solving()} title="Fill the solids with 10-node tetrahedra (Gmsh); made again by itself when the solids change" />
            <S><B i="mesh" l="Element Size" dd={sizeMenu} /><B i="view" l="Show Elements" on={view() === "mesh"} run={() => setView(view() === "mesh" ? "model" : "mesh")} title="Draw the elements' edges on the solid" /></S></G>
          <G l="Elements" h="elements"><div class="rnote">10-node tetrahedra (quadratic): accurate for stress in any shape.</div></G>
        </Show>
        <Show when={!frame() && !hasSolids()} fallback={<Show when={frame()}><G l="Mesh" h="mesh"><div class="rnote">Frames and trusses are not meshed: each member is one exact element. Meshing is for parts.</div></G></Show>}>
          <G l="Mesh" h="mesh"><B big i="mesh" l="Mesh" run={C.mesh} off={solving()} title="Mesh the part and show it" />
            <S><B i="mesh" l="Element Size" dd={sizeMenu} /><T i="refine" l="Refine Near" t="refine" title="Click an edge or a point: elements a quarter of the size there" /></S></G>
          <G l="Elements" h="elements"><S><B i="tri6" l="Quadratic (6-node)" on={m()?.mesh.order === 2} run={() => C.meshSettings({ order: 2 })} /><B i="tri3" l="Linear (3-node)" on={m()?.mesh.order === 1} run={() => C.meshSettings({ order: 1 })} /></S></G>
          <G l="Quality" h="mesh"><S><B i="quality" l="Colour by Quality" on={show().quality && view() === "mesh"} run={() => { C.show({ quality: view() !== "mesh" || !show().quality }); if (view() !== "mesh") C.mesh(); }} /><B i="table" l="Statistics" run={() => C.mesh()} /></S></G>
        </Show>
      </Show>
      <Show when={tab() === "loads"}>
        <Show when={!heat()}>
          <G l="Supports" h="supports"><T big i="fixed" l="Fixed" t="sup:fixed" /><T big i="pinned" l="Pinned" t="sup:pinned" />
            <S><Show when={frame()} fallback={<><T i="roller" l="Roller" t="sup:roller" /><T i="symmetry" l="Symmetry" t="sup:symmetry" /><Show when={solid3()}><T i="roller" l="Hold Directions" t="sup:hold" title="Hold only chosen directions (x, y, z) of a face; set them in the Inspector, or make it a line along the face" /></Show></>}><T i="rollerx" l="Roller (along x)" t="sup:rollerx" /><T i="rollery" l="Roller (along y)" t="sup:rollery" /><T i="spring" l="Spring" t="sup:spring" /></Show></S></G>
          <G l="Loads" h="loads"><T big i="force" l="Force" t="load:force" />
            <Show when={frame()} fallback={<S><T i="pressure" l="Pressure" t="load:pressure" /><T i="edgeforce" l={solid3() ? "Face Force" : "Edge Force"} t="load:traction" /><Show when={solid3()}><T i="settle" l="Move a Face" t="load:settlement" title="Move a face by a set amount (a crusher's platen, a pull test); with a crushing step the force it takes is drawn against the movement" /></Show><Show when={m()?.analysis !== "axi" && !solid3()}><T i="pin" l="Pin Load" t="load:bearing" title="Pin Load: click a hole's edge; the pin presses on the side it pushes into" /></Show><B i="gravity" l="Gravity" run={() => C.body("gravity")} /></S>}>
              <T big i="dist" l="Distributed" t="load:dist" /><S><T i="moment" l="Moment" t="load:moment" /><B i="gravity" l="Self-weight" run={() => C.body("gravity")} /><T i="settle" l="Settlement" t="load:settlement" title="Move a support: click a supported node; set how far in the Inspector" /></S><S><T i="mass" l="Mass" t="load:mass" title="A mass at a node, for natural frequencies and dynamics (in every load case; no weight in a static solve)" /><Show when={sp()}><T i="pressure" l="Plate Pressure" t="load:platepressure" title="Pressure on a plate: click inside it (positive pushes a floor down)" /></Show></S></Show></G>
        </Show>
        <Show when={heat()}>
          <G l="Heat" h="heat"><T big i="temp" l="Temperature" t="sup:temperature" /><S><T i="flux" l="Heat Flux" t="load:flux" /><T i="convection" l="Convection" t="load:convection" /><B i="heatgen" l="Heat Generation" run={() => C.body("heat")} /></S></G>
        </Show>
        <Show when={!heat()}><G l="Thermal" h="thermal"><B big i="tchange" l="Temperature Change" run={() => C.body("warming")} title="A uniform change of temperature: thermal stress" />
          <Show when={!frame()}><S><T i="tedge" l="Edge Temperature" t="sup:temperature" title="Hold an edge at a temperature; the temperatures inside are solved first, then the thermal stress" /><T i="convection" l="Convection" t="load:convection" /></S></Show></G></Show>
        <G l="Load Cases" h="cases"><S><CasePicker /><B i="table" l="Cases and Combinations…" run={casesDialog} title="Name the load cases and give each combination its factors" /></S></G>
      </Show>
      <Show when={tab() === "properties"}>
        <G l="Materials" h="materials"><B big i="material" l="Materials" run={() => D.materials()} title="The materials of the model: pick from the library or type your own" />
          <S><B i="material" l="Assign ▾" off={!sel().length} dd={e => openMenu(e, (m()?.materials ?? []).map(x => ({ label: x.name, run: () => (frame() ? formatMembers : formatShapes)(() => ({ material: x.id })) })))} title="Give the selected parts or members a material" /></S></G>
        <Show when={frame()}><G l="Sections" h="sections"><B big i="section" l="Sections" run={() => D.sections()} title="Cross-sections of the members: steel tables, or typed sizes" /></G></Show>
      </Show>
      <Show when={tab() === "solve"}>
        <G l="Solve" h="solve"><B big i="solve" l="Solve" run={C.solve} off={solving()} title="Solve the model (F5)" />
          <S><B i="table" l="Results Tables" run={() => C.view("results")} off={!results()} /><B i="verify" l="Verification" run={() => D.verification()} /></S></G>
        <G l="Studies" h="solve"><S><B i="modes" l="Natural Frequencies" run={() => C.modes()} off={solving() || heat()} title="The lowest natural frequencies and their mode shapes" /><B i="buckle" l="Buckling" run={() => C.buckling()} off={solving() || !frame()} title="The lowest buckling load factors of a frame and their shapes" /><Planned i="mesh" l="Mesh Convergence" v="a later release" /></S></G>
        <G l="Steps" h="steps"><B big i="list" l="Steps" on={isNonlinear()} run={stepsDialog} title="Analysis steps: linear, or large deflection (P-delta, large rotations, snap-through)" /></G>
      </Show>
      <Show when={tab() === "results"}>
        <Show when={frame()} fallback={<>
          <G l="Contours" h="contours"><Show when={!heat()} fallback={<><F big k="t" l="Temperature" /><S><F k="qmag" l="Heat Flux" /><F k="qx" l="Flux in x" /><F k="qy" l="Flux in y" /></S></>}>
            <F big k="vm" l="Von Mises" /><S><F k="umag" l="Displacement" /><F k="s1" l="Principal σ1" /><F k="sf" l="Safety Factor" /></S></Show>
            <S><B i="contour" l="More Results" dd={e => openMenu(e, fieldsFor(m()!.analysis).map(f => ({ label: f.name, on: curField().key === f.key, run: () => C.show({ field: f.key }) })))} off={!pres()} /></S></G>
          <G l="Show" h="contours"><S><Show when={!heat()}><B i="deform" l="Deformed Shape" on={show().deformed} run={() => C.show({ deformed: !show().deformed })} off={!pres()} /></Show><B i="meshlines" l="Mesh Lines" on={show().mesh} run={() => C.show({ mesh: !show().mesh })} off={!pres()} /><Show when={!solid3()}><B i="tri3" l="Per Element" on={show().elem} run={() => C.show({ elem: !show().elem })} off={!partRes()} /></Show></S></G>
          <G l="Inspect" h="results"><S><B i="table" l="Tables and Equilibrium" run={() => C.view("results")} off={!pres()} /><B i="doc" l="Report" run={() => C.view("report")} /></S></G>
          <Show when={!heat()}><G l="Path" h="plasticity"><B big i="curve" l="Load–Deflection" run={partPathDialog} off={!((results() as any)?.path?.umax)} title="The load against the largest movement, through an elastic-plastic solve" /></G></Show>
        </>}>
          <G l="Shape" h="results"><B big i="deform" l="Deflected Shape" on={show().deformed} run={() => C.show({ deformed: !show().deformed })} off={!results()} /></G>
          <Show when={sp()}>{(() => { const Q = (p: { k: Q3; l: string; big?: boolean }) => <B big={p.big} i={({ util: "util", d: "deform", n: "daxial", mz: "dmoment", my: "dmoment", vy: "dshear" } as Record<string, string>)[p.k] ?? "diagram"} l={p.l} on={q3() === p.k} run={() => { setQ3(q3() === p.k ? "none" : p.k); setPlane("3d"); }} off={!results()} />; return <>
            <G l="Colour By" h="space"><Q big k="util" l="Utilisation" /><S><Q k="n" l="Axial Force" /><Q k="t" l="Torque" /><Q k="d" l="Deflection" /></S><S><Q k="mz" l="Moment Mz" /><Q k="my" l="Moment My" /><Q k="vy" l="Shear Vy" /></S></G>
            <G l="Inspect" h="results"><S><B i="table" l="Tables and Equilibrium" run={() => C.view("results")} off={!results()} /></S></G></>; })()}</Show>
          <Show when={!sp()}><G l="Diagrams" h="diagrams"><B big i="dmoment" l="Bending Moment" on={show().diagram === "m"} run={() => C.show({ diagram: show().diagram === "m" ? "none" : "m" })} off={!results()} />
            <S><B i="dshear" l="Shear Force" on={show().diagram === "v"} run={() => C.show({ diagram: show().diagram === "v" ? "none" : "v" })} off={!results()} /><B i="daxial" l="Axial Force" on={show().diagram === "n"} run={() => C.show({ diagram: show().diagram === "n" ? "none" : "n" })} off={!results()} /></S></G>
          <G l="Inspect" h="results"><S><B i="reactions" l="Reactions" on={show().reactions} run={() => C.show({ reactions: !show().reactions })} off={!results()} /><B i="util" l="Utilisation" on={show().util} run={() => C.show({ util: !show().util })} off={!results()} /><B i="table" l="Tables and Equilibrium" run={() => C.view("results")} off={!results()} /></S></G></Show>
          <G l="Dynamics" h="dynamics"><B big i="wave" l="Dynamics" run={dynDialog} off={!dynOf()} title="A dynamic solve's modes (response spectrum) or its record over time (time history)" /></G>
          <G l="Path" h="steps"><B big i="curve" l="Load–Deflection" run={pathDialog} off={!(frameRes()?.path ?? spaceRes()?.path)} title="The load against the movement of a node, through a large-deflection solution" /></G>
          <G l="Combinations" h="cases"><B big i="table" l="Envelope" run={envelope} off={!hasEnvelope()} title="The largest and smallest member forces and reactions over every combination" /></G>
        </Show>
        <Show when={modes()}>{r => <Show when={isBuckle(r())} fallback={<G l="Vibration" h="modes"><B big i="modes" l="Mode Shapes" on={modeOn()} run={() => setModeOn(!modeOn())} title="Show the natural modes on the drawing instead of the static results" /></G>}><G l="Buckling" h="buckle"><B big i="buckle" l="Buckled Shapes" on={modeOn()} run={() => setModeOn(!modeOn())} title="Show the buckled shapes on the drawing instead of the static results" /></G></Show>}</Show>
        <G l="Report" h="report"><B big i="print" l="Print" run={C.print} /><S><B i="doc" l="Results Report" run={() => C.view("report")} /><B i="csv" l="Send to Datum Plot" off={!results()} dd={e => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); import("./link").then(k => { const t = k.plotTables(); if (t.length) openMenu({ x: r.left, y: r.bottom + 2 }, t); else toast("Nothing to plot yet: a nonlinear step, a time history or natural frequencies give curves and lists."); }); }} title="A curve or list from this solve, as a table for Datum Plot (graphs)" /><B i="calc" l="Send to Datum Calc" off={!results()} run={() => import("./link").then(k => k.sendCalc())} title="The largest stress, movement, safety factor or frequency, as values with units for a hand check in Datum Calc" /></S></G>
      </Show>
      <Show when={tab() === "view"}>
        <G l="Zoom" h="zoom"><B big i="fit" l="Fit" run={C.fit} /><S><B i="zin" l="Zoom In" run={C.zoomIn} /><B i="zout" l="Zoom Out" run={C.zoomOut} /></S></G>
        <G l="Show" h="show"><S>
          <label class="chk"><input type="checkbox" checked={opts().grid} onChange={e => setOpts({ ...opts(), grid: e.currentTarget.checked })} />Grid</label>
          <label class="chk"><input type="checkbox" checked={opts().snap} onChange={e => setOpts({ ...opts(), snap: e.currentTarget.checked })} />Snap to grid</label>
          <label class="chk"><input type="checkbox" checked={opts().rulers} onChange={e => setOpts({ ...opts(), rulers: e.currentTarget.checked })} />Rulers</label>
          <Show when={m()?.analysis === "space"}><label class="chk" title="Draw each member as its section (I, channel, box, tube…) swept along it, turned as the solver turns it"><input type="checkbox" checked={sectionsOn()} onChange={e => setSectionsOn(e.currentTarget.checked)} />Section shapes</label></Show>
        </S></G>
      </Show>
      <Show when={tab() === "format"}>
        <Show when={fmtKind() === "shape"}><G l="Shape" h="shapes"><B big i="hrect" l="Make a Hole" run={() => formatShapes(() => ({ hole: true }))} /><B big i="rect" l="Make Solid" run={() => formatShapes(() => ({ hole: false }))} />
          <S><B i="material" l="Material ▾" dd={e => openMenu(e, (m()?.materials ?? []).map(x => ({ label: x.name, run: () => formatShapes(() => ({ material: x.id })) })))} /></S></G><SketchTools /></Show>
        <Show when={fmtKind() === "member"}><G l="Member" h="members"><B big i="beam" l="Beam" run={() => formatMembers(() => ({ kind: "beam" }))} /><B big i="truss" l="Truss Bar" run={() => formatMembers(() => ({ kind: "truss" }))} /><B big i="node" l="Join Where They Cross" run={() => joinCrossings()} title="Put a node where the selected members cross, joining them there" />
          <S><B i="section" l="Section ▾" dd={e => openMenu(e, (m()?.sections ?? []).map(x => ({ label: x.name, run: () => formatMembers(() => ({ section: x.id })) })))} /><B i="material" l="Material ▾" dd={e => openMenu(e, (m()?.materials ?? []).map(x => ({ label: x.name, run: () => formatMembers(() => ({ material: x.id })) })))} /></S></G></Show>
      </Show>
      <Show when={tab() === "help"}>
        <G l="Learn"><B big i="report" l="Getting Started" run={() => D.gettingStarted()} /><B big i="info" l="Glossary" run={() => D.glossary()} /><B big i="help" l="What Is This Tool?" dd={e => openMenu(e, Object.entries(HELP).map(([k, h]) => ({ label: h.t, run: () => helpCard(k) })))} /><B big i="keys" l="Shortcuts" run={() => D.shortcuts()} /><B big i="search" l="Tell Me" run={palette} /></G>
        <G l="Trust"><B big i="verify" l="Verification" run={() => D.verification()} /></G>
        <G l="Datum Mesh on GitHub"><B big i="github" l="GitHub" run={() => call("open_url", { url: GH })} /><B big i="tag" l="Releases" run={() => call("open_url", { url: `${GH}/releases` })} /><B big i="bug" l="Report an Issue" run={() => call("open_url", { url: `${GH}/issues/new` })} /></G>
        <G l="Updates"><B big i="upd" l="Check for Updates" run={() => setBackstage("about")} /><B big i="list" l="Roadmap" run={() => setBackstage("roadmap")} /></G>
      </Show>
    </section>
  );
};
