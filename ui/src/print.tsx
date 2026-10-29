// File › Print: settings on the left, a live page preview on the right (FAMILY §8). Pages are laid
// out here at real size (the model as vector SVG with a title block, then a model report) and
// printed with the WebView's print dialog, where "Save as PDF" makes a PDF. Built on Datum Plan's
// print page (paper sizes, header and footer slots and codes, @page), by way of Datum Sim.
import { For, Show, JSX, createSignal, createMemo, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { createStore } from "solid-js/store";
import { Model, Pt, num } from "./api";
import { out, opts, call, toast, model, frameRes as results, partRes, stale, setBackstage } from "./store";
import { FIELDS, fieldsFor, curField, contourImage, palette, rangeOf, ColourBar, EqTable, deformOf } from "./parts";
import { Help } from "./help";
import { Pick, Check } from "./ui";
import { svgPath, outline } from "./geom";
import { fmt, unit } from "./units";
import { itemName, SUPPORT_NAMES, LOAD_NAMES, ANALYSES } from "./panels";

export const PAPERS: [string, number, number][] = [["A4", 210, 297], ["A3", 297, 420], ["A2", 420, 594], ["Letter", 215.9, 279.4], ["Legal", 215.9, 355.6], ["Tabloid", 279.4, 431.8]];
const MM = 96 / 25.4;
type Slots = [string, string, string];
type Setup = { paper: string; landscape: boolean; margins: [number, number, number, number]; header: Slots; footer: Slots; drawing: boolean; report: boolean };
export const CODES: [string, string][] = [["&[Page]", "Page number"], ["&[Pages]", "Total pages"], ["&[Date]", "Today's date"], ["&[Model]", "Model name"], ["&[Company]", "Company"], ["&[File]", "File name"]];
const [ps, setPs] = createStore<Setup>({
  paper: "A4", landscape: true, margins: [12, 10, 12, 10], header: ["&[Company]", "&[Model]", "&[Date]"], footer: ["Datum Mesh", "Page &[Page] of &[Pages]", "&[File]"],
  drawing: true, report: true,
});
const codes = (t: string, page: number, pages: number) => t.replace(/&\[Page\]/g, String(page)).replace(/&\[Pages\]/g, String(pages)).replace(/&\[Date\]/g, new Date().toLocaleDateString())
  .replace(/&\[Model\]/g, out()?.m.name ?? "").replace(/&\[Company\]/g, opts().company).replace(/&\[File\]/g, out()?.path?.split(/[\\/]/).pop() ?? "not saved");

type Page = "drawing" | "report";

/** The model drawn to fit a box, with supports and loads marked. */
export const ModelDrawing = (p: { m: Model; w: number; h: number }) => {
  const pts: Pt[] = [...p.m.nodes.map(n => [n.x, n.y] as Pt), ...p.m.shapes.flatMap(s => s.geom.kind === "circle" ? [[s.geom.c[0] - s.geom.r, s.geom.c[1] - s.geom.r], [s.geom.c[0] + s.geom.r, s.geom.c[1] + s.geom.r]] as Pt[] : outline(s.geom, Infinity))];
  if (!pts.length) return <p>The model is empty.</p>;
  const x0 = Math.min(...pts.map(q => q[0])), x1 = Math.max(...pts.map(q => q[0])), y0 = Math.min(...pts.map(q => q[1])), y1 = Math.max(...pts.map(q => q[1]));
  const pad = 40, s = Math.min((p.w - 2 * pad) / Math.max(x1 - x0, 1e-9), (p.h - 2 * pad) / Math.max(y1 - y0, 1e-9));
  const ox = (p.w - (x1 - x0) * s) / 2, oy = (p.h - (y1 - y0) * s) / 2;
  const S = (q: Pt): Pt => [ox + (q[0] - x0) * s, p.h - oy - (q[1] - y1 + (y1 - y0)) * s];
  const np = new Map(p.m.nodes.map(n => [n.id, S([n.x, n.y])]));
  const at = (t: any): Pt | null => t.on === "node" ? np.get(t.id) ?? null : t.on === "point" ? S([t.x, t.y]) : t.on === "member" ? (() => { const mb = p.m.members.find(x => x.id === t.id); const a = mb && np.get(mb.a), b = mb && np.get(mb.b); return a && b ? [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2] as Pt : null; })()
    : t.on === "edge" ? (() => { const sh = p.m.shapes.find(x => x.id === t.shape); if (!sh) return null; const o = sh.geom.kind === "circle" ? [sh.geom.c[0] + sh.geom.r, sh.geom.c[1]] as Pt : sh.geom.pts[t.edge]; const q = sh.geom.kind === "circle" ? o : sh.geom.pts[(t.edge + 1) % sh.geom.pts.length]; return S([(o[0] + q[0]) / 2, (o[1] + q[1]) / 2]); })() : null;
  return (
    <svg width={p.w} height={p.h} viewBox={`0 0 ${p.w} ${p.h}`} class="printdiag">
      <For each={[...p.m.shapes.filter(x => !x.hole), ...p.m.shapes.filter(x => x.hole)]}>{sh => <path d={svgPath(sh.geom, S, s)} fill={sh.hole ? "#fff" : "#FBEBEF"} stroke="#1B2733" stroke-width="1.2" stroke-dasharray={sh.hole ? "4 3" : undefined} />}</For>
      <For each={p.m.members}>{mb => { const a = np.get(mb.a)!, b = np.get(mb.b)!; return <line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke="#1B2733" stroke-width={mb.kind === "truss" ? 1.4 : 2.4} />; }}</For>
      <For each={p.m.nodes}>{n => { const q = np.get(n.id)!; return <rect x={q[0] - 2.5} y={q[1] - 2.5} width={5} height={5} fill="#1B2733" />; }}</For>
      <For each={p.m.supports}>{sp => { const q = at(sp.at); return q && <><path d={`M${q[0]} ${q[1]}l-7 11h14z`} fill="none" stroke="#1F7A4D" stroke-width="1.4" /><text x={q[0] + 9} y={q[1] + 12} font-size="8.5" fill="#1F7A4D">{SUPPORT_NAMES[sp.kind.kind].split(" ")[0]}</text></>; }}</For>
      <For each={p.m.loads.filter(l => l.at.on !== "body")}>{l => { const q = at(l.at); return q && <><path d={`M${q[0]} ${q[1] - 26}V${q[1] - 3}m-4 -6l4 6l4 -6`} fill="none" stroke="#B83250" stroke-width="1.4" /><text x={q[0] + 6} y={q[1] - 20} font-size="8.5" fill="#B83250">{LOAD_NAMES[l.kind.kind]}</text></>; }}</For>
    </svg>
  );
};

/** One printed page: header, body, footer. */
const Sheet = (p: { pg: Page; n: number; pages: number; w: number; h: number }) => {
  const inner = () => [p.w - (ps.margins[1] + ps.margins[3]) * MM, p.h - (ps.margins[0] + ps.margins[2]) * MM] as const;
  return (
    <div class="ppage" style={{ width: `${p.w}px`, height: `${p.h}px` }}>
      <div class="pin" style={{ left: `${ps.margins[3] * MM}px`, top: `${ps.margins[0] * MM}px`, width: `${inner()[0]}px`, height: `${inner()[1]}px` }}>
        <div class="phead"><For each={ps.header}>{(t, i) => <span class={`ps${i()}`}>{codes(t, p.n, p.pages)}</span>}</For></div>
        <div class="pbody">{body(p.pg, inner()[0], inner()[1] - 90)}</div>
        <div class="pfoot"><For each={ps.footer}>{(t, i) => <span class={`ps${i()}`}>{codes(t, p.n, p.pages)}</span>}</For></div>
      </div>
    </div>
  );
};

const TitleBlock = (p: { m: Model }) => (
  <table class="tblock"><tbody><tr>
    <td><small>Model</small><b>{p.m.name}</b></td><td><small>Analysis</small>{ANALYSES.find(a => a[0] === p.m.analysis)?.[1]}</td>
    <td><small>Units</small>{p.m.analysis === "frame" ? `${unit("coord")} (drawing), ${unit("len")}, ${unit("force")}, ${unit("moment")}, ${unit("stress")}` : p.m.analysis === "heat" ? `${unit("coord")}, ${unit("temp")}, ${unit("flux")}` : `${unit("coord")}, ${unit("force")}, ${unit("stress")}`}</td><td><small>Company</small>{opts().company || "—"}</td><td><small>Date</small>{new Date().toLocaleDateString()}</td><td><small>Datum Mesh</small>{version()}</td>
  </tr></tbody></table>
);
const [version, setVersion] = createSignal("");

const body = (pg: Page, w: number, h: number): JSX.Element => {
  const m = model(); if (!m) return null;
  if (pg === "drawing") return <div class="pdiag"><ModelDrawing m={m} w={w} h={h - 60} /><TitleBlock m={m} /></div>;
  return <Report m={m} />;
};

const Report = (p: { m: Model }) => {
  const m = p.m;
  return <div class="preport">
    <h3>{m.name}</h3>
    <Show when={m.notes}><p>{m.notes}</p></Show>
    <h4>Model</h4>
    <table class="tbl"><tbody>
      <tr><td>Analysis</td><td>{ANALYSES.find(a => a[0] === m.analysis)?.[1]}{m.analysis === "stress" ? ` · thickness ${fmt("len", m.thickness)}` : ""}</td></tr>
      <Show when={m.analysis === "frame"} fallback={<tr><td>Geometry</td><td>{m.shapes.filter(s => !s.hole).length} shapes, {m.shapes.filter(s => s.hole).length} holes</td></tr>}><tr><td>Geometry</td><td>{m.nodes.length} nodes, {m.members.length} members</td></tr></Show>
    </tbody></table>
    <h4>Materials</h4>
    <table class="tbl"><thead><tr><th>Name</th><th>E ({unit("modulus")})</th><th>ν</th><th>Density ({unit("density")})</th><th>Yield ({unit("stress")})</th><th>Source</th></tr></thead><tbody>
      <For each={m.materials}>{x => <tr><td>{x.name}</td><td class="r">{fmt("modulus", x.e).split(" ")[0]}</td><td class="r">{num(x.nu)}</td><td class="r">{fmt("density", x.rho).split(" ")[0]}</td><td class="r">{fmt("stress", x.fy).split(" ")[0]}</td><td>{x.source}</td></tr>}</For></tbody></table>
    <Show when={m.analysis === "frame"}><h4>Sections</h4><table class="tbl"><tbody><For each={m.sections}>{x => <tr><td>{x.name}</td><td>{x.shape.kind}</td><td>{m.members.filter(b => b.section === x.id).length} members</td></tr>}</For></tbody></table></Show>
    <h4>Supports</h4>
    <table class="tbl"><tbody><For each={m.supports} fallback={<tr><td>None yet.</td></tr>}>{x => <tr><td>{itemName(m, x.id)}</td></tr>}</For></tbody></table>
    <h4>Loads</h4>
    <table class="tbl"><tbody><For each={m.loads} fallback={<tr><td>None yet.</td></tr>}>{x => <tr><td>{itemName(m, x.id)}</td><td>{loadValue(x.kind)}</td></tr>}</For></tbody></table>
    <Show when={m.analysis !== "frame"}><PartReport m={m} /></Show>
    <Show when={results() && !stale() && m.analysis === "frame"} fallback={<Show when={m.analysis === "frame"}><p class="muted">Solve the model (F5) to add its results, reactions and the equilibrium check to this page.</p></Show>}>{_ => { const r = results()!; return <>
      <h4>Equilibrium check</h4>
      <EqTable applied={r.applied} reacted={r.reacted} scale={r.scale} />
      <p class={r.imbalance > 1e-6 ? "badline" : "okline"}>{r.imbalance > 1e-6 ? `The reactions miss the loads by ${num(r.imbalance * 100, 2)} %.` : `The reactions balance the loads (difference ${num(r.imbalance * 100, 2)} %).`}</p>
      <h4>Reactions</h4>
      <table class="tbl"><thead><tr><th>Support</th><th class="r">Fx</th><th class="r">Fy</th><th class="r">M</th></tr></thead><tbody>
        <For each={r.reactions}>{x => <tr><td>Node {m.nodes.findIndex(n => n.id === x.node) + 1}</td><td class="r">{fmt("force", x.fx)}</td><td class="r">{fmt("force", x.fy)}</td><td class="r">{fmt("moment", x.m)}</td></tr>}</For></tbody></table>
      <h4>Members</h4>
      <table class="tbl"><thead><tr><th>Member</th><th class="r">N max / min</th><th class="r">|M| max</th><th class="r">Deflection</th><th class="r">Stress</th><th class="r">Utilisation</th></tr></thead><tbody>
        <For each={r.members}>{(x, i) => <tr><td>{i() + 1}</td><td class="r">{fmt("force", Math.max(...x.n))} / {fmt("force", Math.min(...x.n))}</td><td class="r">{fmt("moment", Math.max(...x.m.map(Math.abs)))}</td><td class="r">{fmt("len", Math.max(...x.d.map(d => Math.hypot(d[0], d[1]))))}</td><td class="r">{fmt("stress", x.stress)}</td><td class="r">{x.util ? `${num(x.util * 100, 3)} %` : "—"}</td></tr>}</For></tbody></table>
      <p class="muted">Solved by Datum Mesh {version()}: {r.stats.equations} equations, residual {r.stats.residual.toExponential(1)}. 2D Euler–Bernoulli beam and truss elements.</p>
    </>; }}</Show>
  </div>;
};

/** The results of a part: mesh, pictures with legends, extremes, reactions or heat flows, the check. */
const PartReport = (p: { m: Model }) => {
  const r = () => (stale() ? null : partRes());
  const pics = createMemo(() => {
    const rr = r(); if (!rr) return [];
    const keys = rr.kind === "heat" ? ["t", "qmag"] : [curField().key === "umag" ? "vm" : curField().key, "umag"];
    return [...new Set(keys)].map(k => { const d = FIELDS.find(f => f.key === k)!, cs = palette(opts().contours, d.reverse); return { d, cs, src: contourImage(rr, d, 640, 300, cs), range: rangeOf(d, rr.fields.find(f => f.key === k)!.v) }; });
  });
  return <Show when={r()} fallback={<p class="muted">Solve the model (F5) to add the mesh, result pictures, the largest values, reactions and the equilibrium check to this page.</p>}>{rr => <>
    <h4>Mesh</h4>
    <table class="tbl"><tbody><tr><td>Elements</td><td>{rr().mesh.tris.length.toLocaleString()} {rr().mesh.order === 2 ? "6-node (quadratic)" : "3-node (linear)"} triangles, {rr().mesh.nodes.length.toLocaleString()} nodes; size {fmt("len", rr().mesh.size)}{p.m.mesh.refine.length ? `, finer at ${p.m.mesh.refine.length} places` : ""}</td></tr>
      <tr><td>Quality</td><td>smallest angle {num(rr().mesh.quality.min_angle, 3)}°, worst aspect ratio {num(rr().mesh.quality.max_aspect, 3)}, {num(rr().mesh.quality.good * 100, 3)} % of triangles with every angle over 30°</td></tr></tbody></table>
    <For each={pics()}>{x => <div class="rpic"><h4>{x.d.name}{rr().kind === "plane" && deformOf(rr()) > 0 ? ` (deformed shape × ${num(deformOf(rr()), 3)})` : ""}</h4><img src={x.src} width={640} height={300} alt={x.d.name} /><div class="rlegend"><ColourBar d={x.d} range={x.range} colours={x.cs} /></div></div>}</For>
    <h4>Largest and smallest values</h4>
    <table class="tbl"><thead><tr><th>Result</th><th class="r">Smallest</th><th>at</th><th class="r">Largest</th><th>at</th></tr></thead><tbody>
      <For each={fieldsFor(rr().kind === "heat" ? "heat" : p.m.analysis).filter(d => d.key !== "sf")}>{d => { const f = rr().fields.find(x => x.key === d.key)!; const at = (q: Pt) => `${fmt("coord", q[0]).split(" ")[0]}, ${fmt("coord", q[1])}`; return <tr><td>{d.name}</td><td class="r">{fmt(d.q, f.min)}</td><td>{at(f.at_min)}</td><td class="r">{fmt(d.q, f.max)}</td><td>{at(f.at_max)}</td></tr>; }}</For>
      <Show when={rr().kind === "plane" && rr().fields.find(f => f.key === "sf")!.max > 0}><tr><td>Smallest safety factor against yield</td><td class="r">{num(Math.min(...rr().fields.find(f => f.key === "sf")!.v.filter(v => v > 0)), 3)}</td><td colSpan={3} /></tr></Show></tbody></table>
    <Show when={rr().kind === "plane"}>{_ => { const pr = rr() as any; return <>
      <h4>Equilibrium check</h4><EqTable applied={pr.applied} reacted={pr.reacted} scale={pr.scale} />
      <p class={pr.imbalance > 1e-6 ? "badline" : "okline"}>{pr.imbalance > 1e-6 ? `The reactions miss the loads by ${num(pr.imbalance * 100, 2)} %.` : `The reactions balance the loads (difference ${num(pr.imbalance * 100, 2)} %).`}</p>
      <h4>Reactions</h4>
      <table class="tbl"><thead><tr><th>Support</th><th class="r">Fx</th><th class="r">Fy</th></tr></thead><tbody><For each={pr.reactions}>{(x: any) => <tr><td>{itemName(p.m, x.support)}</td><td class="r">{fmt("force", x.fx)}</td><td class="r">{fmt("force", x.fy)}</td></tr>}</For></tbody></table>
    </>; }}</Show>
    <Show when={rr().kind === "heat"}>{_ => { const hr = rr() as any; return <>
      <h4>Heat balance</h4>
      <table class="tbl"><thead><tr><th>Where</th><th class="r">Heat into the part</th></tr></thead><tbody><For each={[...hr.supports, ...hr.loads]}>{(x: any) => <tr><td>{itemName(p.m, x.id)}</td><td class="r">{fmt("power", x.q)}</td></tr>}</For></tbody></table>
      <p class={hr.imbalance > 1e-6 ? "badline" : "okline"}>{hr.imbalance > 1e-6 ? `Heat in and out differ by ${num(hr.imbalance * 100, 2)} %.` : `Heat flowing in equals heat flowing out (difference ${num(hr.imbalance * 100, 2)} %).`}</p>
    </>; }}</Show>
    <p class="muted">Solved by Datum Mesh {version()}: {rr().stats.equations.toLocaleString()} equations, residual {rr().stats.residual.toExponential(1)}. {rr().kind === "heat" ? "Steady heat conduction" : p.m.analysis === "strain" ? "Plane strain" : "Plane stress"} with {rr().mesh.order === 2 ? "6-node" : "3-node"} triangles; stresses averaged at nodes. Help › Verification lists the benchmarks this version passes.</p>
  </>}</Show>;
};

/** The Report view: the printed report on screen, with a button to print it. */
export const ReportPage = () => {
  onMount(async () => { if (!version()) setVersion(await call<string>("version") ?? ""); });
  return <div class="page">
    <h1>Report <Help k="report" /></h1>
    <p class="muted">What prints with the model drawing: the model, materials, supports and loads{model()?.analysis === "frame" ? ", and once solved the reactions, member forces and the equilibrium check" : ", and once solved the mesh, result pictures with their scales, the largest values, reactions and the equilibrium check"}. <button class="btn pri sm" onClick={() => setBackstage("print")}>Print or save as PDF</button></p>
    <div class="rsheet"><Show when={model()}>{m => <Report m={m()} />}</Show></div>
  </div>;
};

const loadValue = (k: any): string => ({
  force: () => `Fx ${fmt("force", k.fx)}, Fy ${fmt("force", k.fy)}`, moment: () => fmt("moment", k.m), dist: () => `${fmt("dist", k.w1)} to ${fmt("dist", k.w2)} (${k.dir === "local" ? "across" : `global ${k.dir}`})`,
  pressure: () => fmt("stress", k.p), bearing: () => `pin, Fx ${fmt("force", k.fx)}, Fy ${fmt("force", k.fy)}`, traction: () => `Fx ${fmt(k.total ? "force" : "dist", k.fx)}, Fy ${fmt(k.total ? "force" : "dist", k.fy)}${k.total ? " in total" : ""}`,
  gravity: () => `gx ${fmt("accel", k.gx)}, gy ${fmt("accel", k.gy)}`, flux: () => fmt("flux", k.q), warming: () => `ΔT ${fmt("dtemp", k.dt)}`, convection: () => `h ${fmt("film", k.h)}, fluid ${fmt("temp", k.t_inf)}`, heat: () => fmt("heatgen", k.q),
} as Record<string, () => string>)[k.kind]?.() ?? "";

const [printing, setPrinting] = createSignal(false);

export const PrintPage = () => {
  setPs({ paper: opts().paper || "A4", landscape: opts().landscape !== false });
  const [zoomP, setZoomP] = createSignal(0.5);
  const pages = createMemo<Page[]>(() => [...(ps.drawing ? ["drawing" as Page] : []), ...(ps.report ? ["report" as Page] : [])]);
  const size = createMemo(() => { const p = PAPERS.find(x => x[0] === ps.paper) ?? PAPERS[0]; const [w, h] = ps.landscape ? [p[2], p[1]] : [p[1], p[2]]; return { pw: w, ph: h, w: w * MM, h: h * MM }; });
  onMount(async () => setVersion(await call<string>("version") ?? ""));
  const doPrint = () => {
    setPrinting(true);
    const css = document.createElement("style");
    css.textContent = `@page { size: ${size().pw}mm ${size().ph}mm; margin: 0 }`;
    document.head.append(css);
    document.documentElement.classList.add("printing");
    setTimeout(() => { window.print(); document.documentElement.classList.remove("printing"); css.remove(); setPrinting(false); }, 200);
  };
  return (
    <div class="printpage">
      <div class="pset">
        <h1>Print</h1>
        <div class="pbtns"><button class="btn pri big" onClick={doPrint}><svg class="ic"><use href="#i-print" /></svg> Print / Save as PDF</button></div>
        <p class="muted">In the print window choose your printer, or <b>Save as PDF</b>. {pages().length} {pages().length === 1 ? "page" : "pages"} · {ps.paper} {ps.landscape ? "landscape" : "portrait"}.</p>
        <h4>What to print</h4>
        <Check label="The model, drawn to fit the page" value={ps.drawing} set={v => setPs("drawing", v)} />
        <Check label="The report: materials, supports, loads and results" value={ps.report} set={v => setPs("report", v)} />
        <div class="prow"><label>Paper<Pick value={ps.paper} set={v => setPs("paper", v)} options={PAPERS.map(p => p[0])} /></label><label>Orientation<Pick value={ps.landscape ? "1" : "0"} set={v => setPs("landscape", v === "1")} options={[["1", "Landscape"], ["0", "Portrait"]]} /></label></div>
        <h4>Header and footer</h4>
        <For each={["header", "footer"] as const}>{k => <div class="pslots"><b>{k === "header" ? "Header" : "Footer"}</b>
          <For each={["Left", "Center", "Right"]}>{(l, i) => <label><span>{l}</span><textarea rows="1" value={ps[k][i()]} onInput={e => setPs(k, i(), e.currentTarget.value)} /></label>}</For></div>}</For>
        <p class="muted pcodes">Codes: <For each={CODES}>{([c, l]) => <code title={l} onClick={() => navigator.clipboard?.writeText(c).then(() => toast(`${c} copied.`, true))}>{c}</code>}</For></p>
        <div class="prow">{(["Top", "Right", "Bottom", "Left"] as const).map((l, i) => <label>{l} (mm)<input type="number" min="0" max="60" value={ps.margins[i]} onChange={e => setPs("margins", i, +e.currentTarget.value)} /></label>)}</div>
        <p><button class="btn sm" onClick={() => call("save_file", { name: `${out()?.m.name ?? "model"}.svg`, data: document.querySelector(".pprev .printdiag")?.outerHTML ?? "" })}>Save the drawing as SVG</button></p>
      </div>
      <div class="pprev">
        <div class="pzoom"><span class="muted">Preview</span><input type="range" min="0.25" max="1.2" step="0.05" value={zoomP()} onInput={e => setZoomP(+e.currentTarget.value)} /><span class="muted">{Math.round(zoomP() * 100)}%</span></div>
        <For each={pages()} fallback={<p class="pempty">Nothing chosen to print.</p>}>{(pg, i) =>
          <div class="pwrap" style={{ width: `${size().w * zoomP()}px`, height: `${size().h * zoomP()}px` }}><div style={{ transform: `scale(${zoomP()})`, "transform-origin": "0 0" }}><Sheet pg={pg} n={i() + 1} pages={pages().length} w={size().w} h={size().h} /></div><small>Page {i() + 1} of {pages().length}</small></div>}</For>
      </div>
      <Show when={printing()}><Portal mount={document.body}><div id="printroot"><For each={pages()}>{(pg, i) => <Sheet pg={pg} n={i() + 1} pages={pages().length} w={size().w} h={size().h} />}</For></div></Portal></Show>
    </div>
  );
};
