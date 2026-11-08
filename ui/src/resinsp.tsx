// The tree's Results tab (0.9.1; a tree from 0.10.2, the approved mock-ups): what was solved at its
// head, then each result with its largest value (click one to show it in the view), the checks
// (the balance, the smallest safety factor), and the ways into the tables, graphs and report.
// Nothing here calculates: the values are the engine's, only picked out for the rows.
import { For, Show, JSX } from "solid-js";
import { Model, num, isFrame } from "./api";
import { results, stale, show, setShow, setView, solidRes, partRes, solvedFor, frameRes, spaceRes } from "./store";
import { fieldsFor, curField } from "./parts";
import { fmt, Q } from "./units";
import { dur } from "./console";
import { Q3S, q3, setQ3, vals, Q3 } from "./space3";

const Ic = (p: { n: string }) => <svg class="tic"><use href={`#i-${p.n}`} /></svg>;
/** A branch of the results tree, open. */
const Br = (p: { t: string; icon: string; n?: number; children: JSX.Element }) =>
  <details class="tgrp" open><summary><Ic n={p.icon} /><span class="tt">{p.t}</span><Show when={p.n !== undefined}><span class="pill">{p.n}</span></Show></summary><div class="tlist">{p.children}</div></details>;
/** A result row: its name, its largest value; on when it is the one shown. */
const Row = (p: { icon: string; label: string; value?: string; on?: boolean; title?: string; run?: () => void; cls?: string }) =>
  <button class={`ti ${p.cls ?? ""}`} classList={{ on: !!p.on }} title={p.title} onClick={() => p.run?.()}><Ic n={p.icon} /><span>{p.label}</span><Show when={p.value}><small>{p.value}</small></Show></button>;

const big = (v: number[]) => v.reduce((a, x) => (Math.abs(x) > Math.abs(a) ? x : a), 0);

export const ResultsInsp = (p: { m: Model }) => {
  const r = () => results()!;
  const fields = () => {
    const pr = partRes(), sr = solidRes(), a = pr?.kind === "heat" ? "heat" : sr ? sr.kind : p.m.analysis;
    const list = (sr?.fields ?? pr?.fields ?? []) as { key: string; min: number; max: number }[];
    return fieldsFor(a).map(d => ({ d, f: list.find(x => x.key === d.key) })).filter(x => x.f && x.d.key !== "sf");
  };
  const imb = () => (r() as { imbalance: number }).imbalance || 0;
  const bad = () => imb() > 1e-6;
  const heat = () => r().kind === "heat" || r().kind === "solidheat";
  const sf = () => { const f = ((solidRes()?.fields ?? partRes()?.fields ?? []) as { key: string; v: number[] }[]).find(x => x.key === "sf"); const pos = f?.v.filter(v => v > 0) ?? []; return pos.length ? Math.min(...pos) : null; };
  // A 2D frame: its diagrams, deflection, reactions and utilisation, each with its largest value.
  const frameRows = () => {
    const fr = frameRes(); if (!fr) return [];
    const all = (k: "m" | "v" | "n") => fr.members.flatMap(x => x[k]);
    const d = Math.max(0, ...fr.members.flatMap(x => x.d.map(q => Math.hypot(q[0], q[1]))));
    return [
      { icon: "dmoment", label: "Bending moment", q: "moment" as Q, v: big(all("m")), on: () => show().diagram === "m", run: () => setShow({ ...show(), diagram: "m" }) },
      { icon: "dshear", label: "Shear force", q: "force" as Q, v: big(all("v")), on: () => show().diagram === "v", run: () => setShow({ ...show(), diagram: "v" }) },
      { icon: "daxial", label: "Axial force", q: "force" as Q, v: big(all("n")), on: () => show().diagram === "n", run: () => setShow({ ...show(), diagram: "n" }) },
      { icon: "deform", label: "Deflection", q: "len" as Q, v: d, on: () => show().deformed, run: () => setShow({ ...show(), deformed: !show().deformed }) },
      { icon: "reactions", label: "Reactions", q: "force" as Q, v: big(fr.reactions.flatMap(x => [x.fx, x.fy])), on: () => show().reactions, run: () => setShow({ ...show(), reactions: !show().reactions }) },
      { icon: "util", label: "Utilisation", q: "ratio" as Q, v: Math.max(0, ...fr.members.map(x => x.util)), on: () => show().util, run: () => setShow({ ...show(), util: !show().util }) },
    ];
  };
  // A 3D frame: what the members can be coloured by, each with its largest value.
  const spaceRows = () => {
    const sr = spaceRes(); if (!sr) return [];
    return Q3S.filter(([k]) => k !== "none").map(([k, l, q]) => ({ k, l, q, v: big(sr.members.flatMap(x => vals(x, k as Q3))) }));
  };
  return <div class="rinsp rtree">
    <div class="troot rhead" title="What was solved"><Ic n="solve" /><b title={`${r().stats.equations.toLocaleString()} equations · ${dur(r().stats.ms)}`}>{p.m.name} · {stale() ? "out of date" : "solved"}</b></div>
    <Show when={stale()}><p class="stalebar small">The model changed since this solve: Solve (F5) again for results that match it.</p></Show>
    <Show when={solvedFor()}><p class="small muted rcase">{solvedFor()}</p></Show>
    <Show when={frameRows().length}>
      <Br t="Members" icon="frame" n={p.m.members.length}><For each={frameRows()}>{x =>
        <Row icon={x.icon} label={x.label} value={x.q === "ratio" ? `${num(x.v * 100, 3)} %` : fmt(x.q, x.v, 3)} on={x.on()} run={x.run} title={`Show the ${x.label.toLowerCase()}`} />}</For></Br>
    </Show>
    <Show when={spaceRows().length}>
      <Br t="Members" icon="frame" n={p.m.members.length}><For each={spaceRows()}>{x =>
        <Row icon={x.k === "util" ? "util" : x.k === "d" ? "deform" : x.k === "n" ? "daxial" : x.k.startsWith("v") ? "dshear" : "dmoment"} label={x.l.split(" (")[0]} value={x.k === "util" ? `${num(x.v * 100, 3)} %` : fmt(x.q, x.v, 3)} on={q3() === x.k} run={() => setQ3(x.k)} title={`Colour the members by ${x.l.toLowerCase()}`} />}</For></Br>
    </Show>
    <Show when={fields().length}>
      <Br t={heat() ? "Temperatures and flux" : isFrame(p.m.analysis) ? "Results" : "Contours"} icon="contour" n={fields().length}><For each={fields()}>{x =>
        <Row icon="contour" label={x.d.name} value={fmt(x.d.q, Math.abs(x.f!.max) >= Math.abs(x.f!.min) ? x.f!.max : x.f!.min, 3)} on={curField().key === x.d.key} run={() => setShow({ ...show(), field: x.d.key })} title={`Show ${x.d.name}: from ${fmt(x.d.q, x.f!.min, 3)} to ${fmt(x.d.q, x.f!.max, 3)}`} />}</For></Br>
    </Show>
    <Br t="Checks" icon="check">
      <Row icon={bad() ? "info" : "check"} cls={bad() ? "badline" : "okline"} label={heat() ? (bad() ? "Heat does not balance" : "Heat balance") : bad() ? "Equilibrium · out of balance" : "Equilibrium · balanced"} value={imb() < 5e-4 ? "0.0 %" : `${num(imb() * 100, 2)} %`} title="The reactions against the loads (heat in against heat out): see Tables and equilibrium" run={() => setView("results")} />
      <Show when={sf() !== null}><Row icon="verify" cls={sf()! < 1 ? "badline" : ""} label={`Safety factor against yield${sf()! < 1 ? " · it yields" : ""}`} value={num(sf()!, 3)} /></Show>
    </Br>
    <Br t="Graphs and tables" icon="curve">
      <Row icon="table" label="Tables and equilibrium" run={() => setView("results")} />
      <Show when={solidRes()}><Row icon="curve" label="Graph along a line" run={() => import("./graphs").then(k => k.lineDialog())} /></Show>
      <Show when={(solidRes() as { history?: unknown } | null)?.history}><Row icon="curve" label="Movement and energy in time" run={() => import("./graphs").then(k => k.timeDialog())} /></Show>
      <Row icon="doc" label="Report" run={() => setView("report")} />
    </Br>
  </div>;
};
