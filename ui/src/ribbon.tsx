// Title bar, quick access toolbar, Tell Me and the ribbon (Home · Geometry · Mesh · Loads · Solve ·
// Results · View · Help, with Format as a contextual tab on a selection). Copied from Datum Plan's
// ribbon by way of Datum Sim; planned tools are shown dimmed with the release they arrive in.
import { For, Show, JSX, createSignal, createMemo, onMount } from "solid-js";
import { modes, modeOn, setModeOn, toast, call, out, model, tab, setTab, setBackstage, openMenu, openDialog, closeDialog, theme, setTheme, sel, opts, setOpts, tool, act, results, show, solving, partRes, setSel, setEdgeSel, view } from "./store";
import { fieldsFor, curField } from "./parts";
import { C, ALL_CMDS } from "./cmds";
import * as D from "./dialogs";
import { Icon } from "./ui";
import { Help, helpCard, HELP } from "./help";
import { invoke, Example, isFrame, isBuckle } from "./api";
import { q3, setQ3, Q3 } from "./space3";
import { setPlane } from "./plane3";

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
/** A planned tool: shown dimmed with the release it arrives in. */
const Planned = (p: { i: string; l: string; v: string; big?: boolean }) => <B i={p.i} l={p.l} big={p.big} off title={`${p.l}: planned for ${p.v}`} />;
/** A contour field button: shows that result on the part. */
const F = (p: { k: string; l: string; big?: boolean }) => <B i="contour" l={p.l} big={p.big} on={!!partRes() && curField().key === p.k} off={!partRes()} run={() => C.show({ field: p.k })} />;
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
      <For each={[["home", "Home"], ["geometry", "Geometry"], ["mesh", "Mesh"], ["loads", "Loads"], ["solve", "Solve"], ["results", "Results"], ["view", "View"], ["help", "Help"]]}>{([k, l]) => <button classList={{ on: tab() === k }} onClick={() => setTab(k)}>{l}</button>}</For>
      <div class="ctxgrp" style={{ visibility: fmtKind() ? "visible" : "hidden" }}><small>{fmtKind() === "shape" ? "Shape Tools" : "Member Tools"}</small><button classList={{ on: tab() === "format" }} onClick={() => setTab("format")}>Format</button></div>
    </nav>
  </header>
);

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
  openMenu({ x: r.left, y: r.bottom + 2 }, list.map(x => ({ label: x.name, run: () => C.newModel(x.id) })));
};

/** Join the selected members where they cross (crossing members are otherwise left apart). */
const joinCrossings = () => { const m = model()!, s = new Set(sel()), ids = m.members.filter(b => s.has(b.id)).map(b => b.id); if (ids.length < 2) return toast("Select two or more members that cross."); act({ JoinCrossings: { ids } }).then(o => o && toast(`Joined at ${o.created.length} ${o.created.length === 1 ? "crossing" : "crossings"}.`, true)); };
/** Format: the selected shapes or members, all at once. */
const formatShapes = (f: (s: any) => object) => { const m = model()!, s = new Set(sel()); act({ Batch: m.shapes.filter(x => s.has(x.id)).map(x => ({ Set: { item: { Shape: { ...x, ...f(x) } } } })) }); };
const formatMembers = (f: (s: any) => object) => { const m = model()!, s = new Set(sel()); act({ Batch: m.members.filter(x => s.has(x.id)).map(x => ({ Set: { item: { Member: { ...x, ...f(x) } } } })) }); };

export const Ribbon = () => {
  const m = () => model();
  const frame = () => isFrame(m()?.analysis), heat = () => m()?.analysis === "heat", sp = () => m()?.analysis === "space";
  return (
    <section class="ribbon" ref={fitRibbon}>
      <Show when={tab() === "home"}>
        <G l="Model" h="analysis"><S><B i="frame" l="Frame or truss" on={m()?.analysis === "frame"} run={() => C.analysis("frame")} /><B i="part" l="Plane stress" on={m()?.analysis === "stress"} run={() => C.analysis("stress")} /></S>
          <S><B i="part" l="Plane strain" on={m()?.analysis === "strain"} run={() => C.analysis("strain")} /><B i="temp" l="Heat conduction" on={m()?.analysis === "heat"} run={() => C.analysis("heat")} /></S>
          <S><B i="axi" l="Axisymmetric" on={m()?.analysis === "axi"} run={() => C.analysis("axi")} title="Axisymmetric: draw the half-section right of the y axis; it is turned a full circle about it (vessels, shafts, discs)" /><B i="space" l="3D Frame" on={sp()} run={() => C.analysis("space")} title="3D frames and trusses: members bend both ways and twist; drawn on planes, seen in 3D" /></S></G>
        <G l="Units" h="units"><S><B i="units" l="SI: mm, kN, MPa" on={m()?.units === "SI"} run={() => C.units("SI")} /><B i="units" l="US: in, kip, ksi" on={m()?.units === "US"} run={() => C.units("US")} /></S></G>
        <G l="Select" h="select"><T big i="select" l="Select" t="select" /><S><B i="all" l="Select All" run={C.selectAll} /><B i="del" l="Delete" run={C.del} off={!sel().length} /></S></G>
        <G l="Examples" h="examples"><B big i="open" l="Examples" dd={exampleMenu} /></G>
        <G l="Solve" h="solve"><B big i="solve" l="Solve" run={C.solve} off={solving()} title="Solve the model (F5)" /></G>
      </Show>
      <Show when={tab() === "geometry"}>
        <Show when={frame()} fallback={<>
          <G l="Part" h="shapes"><T big i="rect" l="Rectangle" t="rect" /><S><T i="circle" l="Circle" t="circle" /><T i="poly" l="Polygon" t="poly" /></S></G>
          <G l="Holes" h="holes"><S><T i="hrect" l="Rectangular Hole" t="hrect" /><T i="hcircle" l="Round Hole" t="hcircle" /></S></G>
          <G l="Material" h="materials"><B big i="material" l="Materials" run={() => D.materials()} /></G>
          <G l="Import" h="import"><Planned big i="import" l="DXF" v="0.2" /></G>
        </>}>
          <G l="Draw" h="members"><T big i="beam" l="Beam" t="beam" title="Draw beams: click two points; it continues from the last" /><T big i="truss" l="Truss Bar" t="truss" /><S><T i="node" l="Node" t="node" /><B i="node" l="Node at…" run={() => D.nodeAt()} /></S></G>
          <Show when={sp()}><G l="3D" h="space"><B big i="repeat" l="Repeat" run={() => D.repeat()} title="Copy the selected members along the depth and join the copies (bays of a building)" /></G></Show>
          <G l="Properties" h="sections"><B big i="section" l="Sections" run={() => D.sections()} /><B big i="material" l="Materials" run={() => D.materials()} /></G>
          <G l="Import" h="import"><Planned big i="import" l="DXF" v="0.2" /></G>
        </Show>
        <G l="Snap" h="snap"><S><label class="chk"><input type="checkbox" checked={opts().snap} onChange={e => setOpts({ ...opts(), snap: e.currentTarget.checked })} />Snap to grid</label><label class="chk"><input type="checkbox" checked={opts().grid} onChange={e => setOpts({ ...opts(), grid: e.currentTarget.checked })} />Show grid</label></S></G>
      </Show>
      <Show when={tab() === "mesh"}>
        <Show when={!frame()} fallback={<G l="Mesh" h="mesh"><div class="rnote">Frames and trusses are not meshed: each member is one exact element. Meshing is for parts.</div></G>}>
          <G l="Mesh" h="mesh"><B big i="mesh" l="Mesh" run={C.mesh} off={solving()} title="Mesh the part and show it" />
            <S><B i="mesh" l="Element Size" dd={sizeMenu} /><T i="refine" l="Refine Near" t="refine" title="Click an edge or a point: elements a quarter of the size there" /></S></G>
          <G l="Elements" h="elements"><S><B i="tri6" l="Quadratic (6-node)" on={m()?.mesh.order === 2} run={() => C.meshSettings({ order: 2 })} /><B i="tri3" l="Linear (3-node)" on={m()?.mesh.order === 1} run={() => C.meshSettings({ order: 1 })} /></S></G>
          <G l="Quality" h="mesh"><S><B i="check" l="Colour by Quality" on={show().quality && view() === "mesh"} run={() => { C.show({ quality: view() !== "mesh" || !show().quality }); if (view() !== "mesh") C.mesh(); }} /><B i="table" l="Statistics" run={() => C.mesh()} /></S></G>
        </Show>
      </Show>
      <Show when={tab() === "loads"}>
        <Show when={!heat()}>
          <G l="Supports" h="supports"><T big i="fixed" l="Fixed" t="sup:fixed" /><T big i="pinned" l="Pinned" t="sup:pinned" />
            <S><Show when={frame()} fallback={<><T i="roller" l="Roller" t="sup:roller" /><T i="symmetry" l="Symmetry" t="sup:symmetry" /></>}><T i="roller" l="Roller (along x)" t="sup:rollerx" /><T i="roller" l="Roller (along y)" t="sup:rollery" /><T i="spring" l="Spring" t="sup:spring" /></Show></S></G>
          <G l="Loads" h="loads"><T big i="force" l="Force" t="load:force" />
            <Show when={frame()} fallback={<S><T i="pressure" l="Pressure" t="load:pressure" /><T i="edgeforce" l="Edge Force" t="load:traction" /><Show when={m()?.analysis !== "axi"}><T i="pin" l="Pin Load" t="load:bearing" title="Pin Load: click a hole's edge; the pin presses on the side it pushes into" /></Show><B i="gravity" l="Gravity" run={() => C.body("gravity")} /></S>}>
              <T big i="dist" l="Distributed" t="load:dist" /><S><T i="moment" l="Moment" t="load:moment" /><B i="gravity" l="Self-weight" run={() => C.body("gravity")} /></S></Show></G>
        </Show>
        <Show when={heat()}>
          <G l="Heat" h="heat"><T big i="temp" l="Temperature" t="sup:temperature" /><S><T i="flux" l="Heat Flux" t="load:flux" /><T i="convection" l="Convection" t="load:convection" /><B i="heatgen" l="Heat Generation" run={() => C.body("heat")} /></S></G>
        </Show>
        <Show when={!heat()}><G l="Thermal" h="thermal"><B big i="temp" l="Temperature Change" run={() => C.body("warming")} title="A uniform change of temperature: thermal stress" />
          <Show when={!frame()}><S><T i="temp" l="Edge Temperature" t="sup:temperature" title="Hold an edge at a temperature; the temperatures inside are solved first, then the thermal stress" /><T i="convection" l="Convection" t="load:convection" /></S></Show></G></Show>
        <G l="Load Cases" h="loads"><S><Planned i="table" l="Load Cases" v="0.2" /><Planned i="table" l="Combinations" v="0.2" /></S></G>
      </Show>
      <Show when={tab() === "solve"}>
        <G l="Solve" h="solve"><B big i="solve" l="Solve" run={C.solve} off={solving()} title="Solve the model (F5)" />
          <S><B i="table" l="Results Tables" run={() => C.view("results")} off={!results()} /><B i="verify" l="Verification" run={() => D.verification()} /></S></G>
        <G l="Studies" h="solve"><S><B i="modes" l="Natural Frequencies" run={() => C.modes()} off={solving() || m()?.analysis === "heat"} title="The lowest natural frequencies and their mode shapes" /><B i="buckle" l="Buckling" run={() => C.buckling()} off={solving() || !frame()} title="The lowest buckling load factors of a frame and their shapes" /><Planned i="mesh" l="Mesh Convergence" v="0.3" /></S></G>
      </Show>
      <Show when={tab() === "results"}>
        <Show when={frame()} fallback={<>
          <G l="Contours" h="contours"><Show when={!heat()} fallback={<><F big k="t" l="Temperature" /><S><F k="qmag" l="Heat Flux" /><F k="qx" l="Flux in x" /><F k="qy" l="Flux in y" /></S></>}>
            <F big k="vm" l="Von Mises" /><S><F k="umag" l="Displacement" /><F k="s1" l="Principal σ1" /><F k="sf" l="Safety Factor" /></S></Show>
            <S><B i="contour" l="More Results" dd={e => openMenu(e, fieldsFor(m()!.analysis).map(f => ({ label: f.name, on: curField().key === f.key, run: () => C.show({ field: f.key }) })))} off={!partRes()} /></S></G>
          <G l="Show" h="contours"><S><Show when={!heat()}><B i="deform" l="Deformed Shape" on={show().deformed} run={() => C.show({ deformed: !show().deformed })} off={!partRes()} /></Show><B i="mesh" l="Mesh Lines" on={show().mesh} run={() => C.show({ mesh: !show().mesh })} off={!partRes()} /><B i="tri3" l="Per Element" on={show().elem} run={() => C.show({ elem: !show().elem })} off={!partRes()} /></S></G>
          <G l="Inspect" h="results"><S><B i="table" l="Tables and Equilibrium" run={() => C.view("results")} off={!partRes()} /><B i="reactions" l="Report" run={() => C.view("report")} /></S></G>
        </>}>
          <G l="Shape" h="results"><B big i="deform" l="Deflected Shape" on={show().deformed} run={() => C.show({ deformed: !show().deformed })} off={!results()} /></G>
          <Show when={sp()}>{(() => { const Q = (p: { k: Q3; l: string; big?: boolean }) => <B big={p.big} i={p.k === "util" ? "check" : p.k === "d" ? "deform" : "diagram"} l={p.l} on={q3() === p.k} run={() => { setQ3(q3() === p.k ? "none" : p.k); setPlane("3d"); }} off={!results()} />; return <>
            <G l="Colour By" h="space"><Q big k="util" l="Utilisation" /><S><Q k="n" l="Axial Force" /><Q k="t" l="Torque" /><Q k="d" l="Deflection" /></S><S><Q k="mz" l="Moment Mz" /><Q k="my" l="Moment My" /><Q k="vy" l="Shear Vy" /></S></G>
            <G l="Inspect" h="results"><S><B i="table" l="Tables and Equilibrium" run={() => C.view("results")} off={!results()} /></S></G></>; })()}</Show>
          <Show when={!sp()}><G l="Diagrams" h="diagrams"><B big i="diagram" l="Bending Moment" on={show().diagram === "m"} run={() => C.show({ diagram: show().diagram === "m" ? "none" : "m" })} off={!results()} />
            <S><B i="diagram" l="Shear Force" on={show().diagram === "v"} run={() => C.show({ diagram: show().diagram === "v" ? "none" : "v" })} off={!results()} /><B i="diagram" l="Axial Force" on={show().diagram === "n"} run={() => C.show({ diagram: show().diagram === "n" ? "none" : "n" })} off={!results()} /></S></G>
          <G l="Inspect" h="results"><S><B i="reactions" l="Reactions" on={show().reactions} run={() => C.show({ reactions: !show().reactions })} off={!results()} /><B i="check" l="Utilisation" on={show().util} run={() => C.show({ util: !show().util })} off={!results()} /><B i="table" l="Tables and Equilibrium" run={() => C.view("results")} off={!results()} /></S></G></Show>
        </Show>
        <Show when={modes()}>{r => <Show when={isBuckle(r())} fallback={<G l="Vibration" h="modes"><B big i="modes" l="Mode Shapes" on={modeOn()} run={() => setModeOn(!modeOn())} title="Show the natural modes on the drawing instead of the static results" /></G>}><G l="Buckling" h="buckle"><B big i="buckle" l="Buckled Shapes" on={modeOn()} run={() => setModeOn(!modeOn())} title="Show the buckled shapes on the drawing instead of the static results" /></G></Show>}</Show>
        <G l="Report" h="report"><B big i="print" l="Print" run={C.print} /><S><B i="report" l="Results Report" run={() => C.view("report")} /><Planned i="csv" l="Send to Datum Plot" v="0.2" /><Planned i="calc" l="Send to Datum Calc" v="0.2" /></S></G>
      </Show>
      <Show when={tab() === "view"}>
        <G l="Zoom" h="zoom"><B big i="fit" l="Fit" run={C.fit} /><S><B i="zin" l="Zoom In" run={C.zoomIn} /><B i="zout" l="Zoom Out" run={C.zoomOut} /></S></G>
        <G l="Show" h="show"><S>
          <label class="chk"><input type="checkbox" checked={opts().grid} onChange={e => setOpts({ ...opts(), grid: e.currentTarget.checked })} />Grid</label>
          <label class="chk"><input type="checkbox" checked={opts().snap} onChange={e => setOpts({ ...opts(), snap: e.currentTarget.checked })} />Snap to grid</label>
          <label class="chk"><input type="checkbox" checked={opts().rulers} onChange={e => setOpts({ ...opts(), rulers: e.currentTarget.checked })} />Rulers</label>
        </S></G>
      </Show>
      <Show when={tab() === "format"}>
        <Show when={fmtKind() === "shape"}><G l="Shape" h="shapes"><B big i="hrect" l="Make a Hole" run={() => formatShapes(() => ({ hole: true }))} /><B big i="rect" l="Make Solid" run={() => formatShapes(() => ({ hole: false }))} />
          <S><B i="material" l="Material ▾" dd={e => openMenu(e, (m()?.materials ?? []).map(x => ({ label: x.name, run: () => formatShapes(() => ({ material: x.id })) })))} /></S></G></Show>
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
