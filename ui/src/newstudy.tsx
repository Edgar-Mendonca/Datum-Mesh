// File › New (0.6.0): a new study, by answering what you are analysing and what you want to know.
import { For, Show, createSignal } from "solid-js";
import { Analysis, StepKind } from "./api";
import { act, loadOut, setTool, opts, setBackstage, setTab } from "./store";
import { keepChanges } from "./dialogs";
import { analysesFor, setAnalysis } from "./steps";
import { fit } from "./canvas";

type Kind = { id: string; name: string; what: string; art: string; group: string; analysis?: Analysis; sub?: [string, string, string, Analysis][] };
const KINDS: Kind[] = [
  { id: "truss", group: "Structures: members", name: "2D truss", what: "Pin-jointed bars: roof trusses, bridges, bracing.", art: "M8 50H92L76 22H24zM24 22V50M42 22V50M58 22V50M76 22V50M24 22L42 50M76 22L58 50M42 22H58", analysis: "frame" },
  { id: "frame", group: "Structures: members", name: "2D frame", what: "Beams and columns that bend: portals, multi-storey frames.", art: "M18 54V22L50 8L82 22V54M10 54h16M74 54h16", analysis: "frame" },
  { id: "space", group: "Structures: members", name: "3D frame", what: "Buildings and platforms in 3D; slabs and walls as plates.", art: "M14 24L50 10L86 24L50 38zM14 24V48M50 38V58M86 24V48M50 10V28", analysis: "space" },
  { id: "part", group: "Parts: meshed", name: "2D part", what: "A part you draw as an outline: plates, sections, turned parts.", art: "M10 12H90V48H10zM43 30a7 7 0 1 0 14 0a7 7 0 1 0 -14 0",
    sub: [["stress", "Plane stress", "A plate thin across its face, loaded in its plane: brackets, lugs, gussets.", "stress"], ["strain", "Plane strain", "A long part with the same section all along: dams, pipes, retaining walls.", "strain"], ["axi", "Turned (axisymmetric)", "A part turned about an axis: shafts, vessels, discs. Draw half the section.", "axi"]] },
  { id: "solid", group: "Parts: meshed", name: "3D solid", what: "An outline extruded to a thickness, or turned about an axis.", art: "M10 22L22 12H90L78 22zM10 22H78V50H10zM78 50L90 40V12",
    sub: [["extrude", "Extruded", "The outline pushed out along z by a thickness.", "solid"], ["revolve", "Revolved", "The outline turned about the y axis, a full turn or a sector.", "solid"]] },
  { id: "heat", group: "Heat", name: "Heat in a 2D part", what: "Temperatures and heat flow: fins, walls, plates.", art: "M8 52H92V44H84V12H78V44H64V12H58V44H44V12H38V44H24V12H18V44H8z", analysis: "heat" },
  { id: "heat3", group: "Heat", name: "Heat in a 3D solid", what: "Temperatures and heat flow through a 3D part.", art: "M10 22L22 12H90L78 22zM10 22H78V50H10zM78 50L90 40V12M28 22V50M46 22V50M64 22V50", analysis: "solidheat" },
];
const Art = (p: { d: string }) => <svg viewBox="0 0 100 60" class="exart"><path d={p.d} fill="none" stroke="var(--blue)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>;

export const NewStudy = () => {
  const [kind, setKind] = createSignal<Kind | null>(null);
  const [sub, setSub] = createSignal(0);
  const [want, setWant] = createSignal<StepKind>("linear");
  const [units, setUnits] = createSignal<"SI" | "US">(opts().units ?? "SI");
  const analysis = (): Analysis => { const k = kind()!; return k.analysis ?? k.sub![sub()][3]; };
  const pick = (k: Kind) => { setKind(k); setSub(0); setWant("linear"); };
  const create = () => keepChanges(async () => {
    const k = kind()!, a = analysis(), o = await loadOut("new_model", { example: null, analysis: a });
    if (!o) return;
    const m = o.m, rev = k.id === "solid" && k.sub![sub()][0] === "revolve";
    await act({ SetSettings: { s: { analysis: a, units: units(), grid: m.grid, thickness: m.thickness, mesh: m.mesh, study: k.id === "truss" ? "truss" : "", ...(rev ? { revolve: 2 * Math.PI } : {}) } } });
    if (want() !== "linear") setAnalysis(want());
    setTab("geometry");
    setTool(k.id === "truss" ? "truss" : k.id === "frame" ? "beam" : k.id === "space" ? "select" : "rect");
    setTimeout(fit, 80);
  });
  const groups = [...new Set(KINDS.map(k => k.group))];
  return <div class="newstudy">
    <h1>New study</h1>
    <p class="muted">Answer two questions and the model is set up for it: its analysis, and the right drawing tool ready. Everything can be changed later.</p>
    <h3><span class="qn">1</span>What are you analysing?</h3>
    <div class="kinds"><For each={groups}>{g => <div class="kgroup"><b>{g}</b><For each={KINDS.filter(k => k.group === g)}>{k =>
      <button class="kcard" classList={{ on: kind()?.id === k.id }} data-kind={k.id} onClick={() => pick(k)}><Art d={k.art} /><span><b>{k.name}</b><small>{k.what}</small></span></button>}</For></div>}</For></div>
    <Show when={kind()}>{k => <>
      <Show when={k().sub}><h3><span class="qn">1b</span>Which kind of {k().name.toLowerCase()}?</h3>
        <div class="subs"><For each={k().sub!}>{(s, i) => <button class="kcard sub" classList={{ on: sub() === i() }} data-sub={s[0]} onClick={() => setSub(i())}><span><b>{s[1]}</b><small>{s[2]}</small></span></button>}</For></div></Show>
      <Show when={analysesFor(analysis()).length}><h3><span class="qn">2</span>What do you want to know?</h3>
        <div class="subs"><For each={analysesFor(analysis())}>{([w, l, t]) => <button class="kcard sub" classList={{ on: want() === w }} data-want={w} onClick={() => setWant(w)}><span><b>{l}</b><small>{t}</small></span></button>}</For></div>
        <p class="muted small">Natural frequencies{k().id === "frame" || k().id === "truss" ? " and buckling" : ""} can be found from any of these, on the Solve tab.</p></Show>
      <h3><span class="qn">3</span>Units</h3>
      <div class="subs"><For each={[["SI", "SI: mm, kN, MPa, °C"], ["US", "US: in, kip, ksi, °F"]] as const}>{([u, l]) => <button class="kcard sub" classList={{ on: units() === u }} onClick={() => setUnits(u)}><span><b>{l}</b></span></button>}</For></div>
      <p class="ncreate"><button class="btn pri" onClick={create}>Create the {k().name.toLowerCase()} study</button><button class="btn" onClick={() => setBackstage("home")}>Start from an example instead</button></p>
    </>}</Show>
  </div>;
};
