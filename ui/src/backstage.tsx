// File (backstage): home with examples and recent models, export, print, options, roadmap, updates.
import { For, Show, createSignal, createResource, onMount, JSX } from "solid-js";
import { PAPERS, PrintPage } from "./print";
import { RoadmapPage } from "./roadmap";
import { Options, Out, Example, listen, invoke } from "./api";
import { backstage, setBackstage, call, out, setOut, opts, setOpts, toast, setThemeRaw, applyTheme } from "./store";
import { C } from "./cmds";
import * as D from "./dialogs";
import { Icon, Text, Pick, Check } from "./ui";

type Info = { current: string; latest: string | null; beta: boolean; available: boolean; notes: string; published: string; url: string | null; sums: string | null; size: number; page: string };
export const [update, setUpdate] = createSignal<Info | null>(null);
export const [checking, setChecking] = createSignal(false), [checkedAt, setCheckedAt] = createSignal("");
export const checkUpdate = async (quiet: boolean) => {
  setChecking(true);
  try { setUpdate(await invoke<Info>("check_update")); setCheckedAt(new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })); if (!quiet) toast(update()?.available ? `Version ${update()!.latest} is available.` : "You have the latest version.", true); }
  catch (e) { if (!quiet) toast(`Could not check for updates: ${e}`); }
  finally { setChecking(false); }
};
const GH = "https://github.com/Edgar-Mendonca/Datum-Mesh";
export const GitHubLinks = () => (
  <div class="ghlinks">
    <For each={[["", "GitHub", "The project and its code"], ["/releases", "Releases", "Every version and its change log"], ["/issues/new", "Report an issue", "Tell us about a bug or an idea"]]}>{([path, l, t]) =>
      <button class="btn" title={t} onClick={() => call("open_url", { url: GH + path })}><svg class="ic"><use href={path === "/issues/new" ? "#i-bug" : path ? "#i-tag" : "#i-github"} /></svg>{l}</button>}</For>
  </div>
);
const md = (s: string) => s.split("\n").map(l => {
  const e = l.replace(/[&<>]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>").replace(/`(.+?)`/g, "<code>$1</code>");
  return l.startsWith("## ") ? `<h3>${e.slice(3)}</h3>` : l.startsWith("- ") ? `<li>${e.slice(2)}</li>` : e.trim() ? `<p>${e}</p>` : "";
}).join("");

/** A small drawing for each example tile. */
const ExampleArt = (p: { id: string }) => {
  const s = { fill: "none", stroke: "var(--blue)", "stroke-width": 2, "stroke-linecap": "round" as const, "stroke-linejoin": "round" as const };
  const pics: Record<string, string> = {
    "blank-frame": "M20 50V22H80V50M20 22L50 10L80 22",
    "blank-stress": "M18 48V14H82V48z",
    portal: "M18 52V24L50 12L82 24V52M12 52h12M76 52h12",
    truss: "M10 46H90L74 30H26zM26 30V46M42 30V46M58 30V46M74 30V46M26 30L42 46M74 30L58 46M42 30H58",
    lug: "M30 54V30A20 20 0 0 1 70 30V54zM42 30a8 8 0 1 0 16 0a8 8 0 1 0 -16 0",
    bracket: "M24 8H38V40Q38 46 44 46H84V56H24z",
    flange: "M28 30a22 22 0 1 0 44 0a22 22 0 1 0 -44 0M42 30a8 8 0 1 0 16 0a8 8 0 1 0 -16 0M47 14a3 3 0 1 0 6 0a3 3 0 1 0 -6 0M47 46a3 3 0 1 0 6 0a3 3 0 1 0 -6 0",
    fin: "M14 52H86V44H80V10H76V44H62V10H58V44H44V10H40V44H26V10H22V44H14z",
  };
  return <svg viewBox="0 0 100 60" class="exart"><path d={pics[p.id] ?? pics["blank-frame"]} {...s} /></svg>;
};

const Home = () => {
  const [ex] = createResource(() => call<Example[]>("examples_list"));
  const [recent] = createResource(() => call<{ path: string; name: string; modified: string }[]>("recent"));
  return (
    <div class="home">
      <div>
        <h1>Good {new Date().getHours() < 12 ? "morning" : new Date().getHours() < 18 ? "afternoon" : "evening"}</h1>
        <div class="tiles">
          <For each={ex() ?? []}>{t => <button class="tile" onClick={() => C.newModel(t.id)}><div class="pic"><ExampleArt id={t.id} /></div><b>{t.name}</b><span>{t.what}</span></button>}</For>
          <button class="tile" onClick={() => setBackstage("roadmap")}><div class="pic"><svg class="ic"><use href="#i-list" /></svg></div><b>Roadmap</b><span>Everything Datum Mesh will do, and when.</span></button>
        </div>
        <h2>Recent</h2>
        <table class="recent"><tbody>
          <For each={recent() ?? []} fallback={<tr><td class="muted">Models you open or save appear here.</td></tr>}>{r => <tr class="f" onClick={() => C.openPath(r.path)}><td><b>{r.name}</b><div class="p">{r.path}</div></td><td class="p">{r.modified}</td></tr>}</For>
        </tbody></table>
      </div>
      <UpdateCard />
    </div>
  );
};

const UpdateCard = () => {
  const [pct, setPct] = createSignal<number | null>(null);
  const [ver] = createResource(() => call<string>("version"));
  onMount(() => { listen("update-progress", (p: number) => setPct(p)); if (!update()) checkUpdate(true); });
  const install = async () => { const u = update(); if (!u?.url) return; setPct(0); await call("install_update", { url: u.url, sums: u.sums }); setPct(null); };
  return (
    <div class="card">
      <div class="brand"><img src="logo.svg" alt="" /><div><b>Datum Mesh</b><span class="muted">Version {ver()}{opts().beta ? " · beta channel" : ""}</span></div></div>
      <Show when={update()} fallback={<><span class="pill">{checking() ? "Checking for updates…" : "Not checked yet"}</span><button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check for updates"}</button><GitHubLinks /></>}>{u => <>
        <Show when={u().available} fallback={<span class="pill ok">You have the latest version</span>}><span class="pill warn">Version {u().latest}{u().beta ? " (beta)" : ""} is ready</span></Show>
        <Show when={u().available && u().url}><button class="btn pri wide" disabled={pct() !== null} onClick={install}>{pct() === null ? `Update now (${Math.round(u().size / 1048576)} MB)` : `Downloading… ${pct()}%`}</button></Show>
        <Show when={pct() !== null}><div class="prog"><i style={{ width: `${pct()}%` }} /></div></Show>
        <button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check again"}</button>
        <Show when={checkedAt() && !checking()}><span class="muted small">Checked at {checkedAt()}</span></Show>
        <GitHubLinks />
        <div class="notes"><h4>What's new{u().latest ? ` in ${u().latest}` : ""}</h4><div innerHTML={md(u().notes || "Release notes will appear here.")} /></div>
      </>}</Show>
    </div>
  );
};

const Export = () => (
  <div><h1>Export</h1>
    <div class="tiles">
      <button class="tile" onClick={() => setBackstage("print")}><div class="pic"><svg class="ic"><use href="#i-print" /></svg></div><b>Print or save as PDF</b><span>The model drawn to scale, with its materials, sections, supports and loads.</span></button>
      <button class="tile" disabled><div class="pic"><svg class="ic"><use href="#i-csv" /></svg><small>0.2</small></div><b>Results as CSV</b><span>Probe lines and convergence data for Datum Plot. Planned for 0.2.</span></button>
      <button class="tile" disabled><div class="pic"><svg class="ic"><use href="#i-calc" /></svg><small>0.2</small></div><b>Send to Datum Calc</b><span>A result as a variable with units, for a hand check beside it. Planned for 0.2.</span></button>
    </div>
  </div>
);

const Opt = (p: { l: string; hint?: string; children: JSX.Element }) => <div class="opt"><div><b>{p.l}</b>{p.hint && <small>{p.hint}</small>}</div><div class="ctl">{p.children}</div></div>;
const Sec = (p: { t: string; about: string; children: JSX.Element }) => <section class="osec"><h3>{p.t}</h3><p class="muted">{p.about}</p>{p.children}</section>;

const OptionsPage = () => {
  const [o, setO] = createSignal<Options>({ ...opts() });
  const save = async () => { const r = await call<Out>("set_options", { opts: o() }); if (r) { setOpts(o()); setOut(r); toast("Options saved.", true); } };
  const put = <K extends keyof Options>(k: K, v: Options[K]) => setO({ ...o(), [k]: v });
  return (
    <div class="options"><h1>Options</h1>
      <p class="muted">Settings for Datum Mesh on this computer. Defaults for new models apply when a model is created; the settings of an open model are in the Inspector when nothing is selected.</p>
      <Sec t="General" about="How the window looks.">
        <Opt l="Theme" hint="Auto follows the Windows light or dark setting."><Pick value={o().theme || "auto"} set={v => { put("theme", v); setThemeRaw(v); applyTheme(); }} options={[["auto", "Auto (follow Windows)"], ["light", "Light"], ["dark", "Dark"]]} /></Opt>
        <Opt l="Canvas" hint="The grid, snapping to it, and the rulers along the edges."><span class="inline"><Check label="Show the grid" value={o().grid} set={v => put("grid", v)} /><Check label="Snap to the grid" value={o().snap} set={v => put("snap", v)} /><Check label="Rulers" value={o().rulers} set={v => put("rulers", v)} /></span></Opt>
        <Opt l="Contour colours" hint="Blue (low) to red (high), or viridis, which people with colour-blindness read as well as anyone."><Pick value={o().contours} set={v => put("contours", v as any)} options={[["rainbow", "Blue to red"], ["viridis", "Viridis (colour-blind safe)"]]} /></Opt>
      </Sec>
      <Sec t="Start-up and saving" about="What happens when Datum Mesh starts, and how your work is protected.">
        <Opt l="When Datum Mesh starts" hint="Show the File page, or reopen the model you used last."><Pick value={o().openLast ? "last" : "home"} set={v => put("openLast", v === "last")} options={[["home", "Show the File page"], ["last", "Reopen the last model"]]} /></Opt>
        <Opt l="Recovery copy every" hint="A copy is kept while you work, so a crash or power cut loses little. 0 turns it off."><span class="inline"><input type="number" min="0" style={{ width: "70px" }} value={o().autosave} onChange={e => put("autosave", +e.currentTarget.value)} /> minutes</span></Opt>
      </Sec>
      <Sec t="New models" about="Defaults given to every new model. Each model keeps its own settings afterwards.">
        <Opt l="Units" hint="How values are shown and typed. Files always keep SI units."><Pick value={o().units} set={v => put("units", v as any)} options={[["SI", "SI: mm, kN, MPa, °C"], ["US", "US: in, kip, ksi, °F"]]} /></Opt>
      </Sec>
      <Sec t="Printing" about="Defaults for File › Print.">
        <Opt l="Paper" hint="Default paper for printouts."><span class="inline"><Pick value={o().paper || "A4"} set={v => put("paper", v)} options={PAPERS.map(p => p[0])} /><Pick value={o().landscape === false ? "0" : "1"} set={v => put("landscape", v === "1")} options={[["1", "Landscape"], ["0", "Portrait"]]} /></span></Opt>
        <Opt l="Company" hint="Shown in the header of printouts."><Text value={o().company} set={v => put("company", v)} /></Opt>
      </Sec>
      <Sec t="Updates" about="Datum Mesh checks GitHub for new versions when it starts. Updates install from File › Updates.">
        <Opt l="Beta versions" hint="Get new features early. They may have bugs."><Check label="Get beta versions" value={o().beta} set={v => put("beta", v)} /></Opt>
      </Sec>
      <p class="osave"><button class="btn pri" onClick={save}>Save options</button><span class="muted">Datum Mesh keeps everything on this computer. Its only connection is the update check to GitHub.</span></p>
    </div>
  );
};

export const Backstage = () => (
  <div class="bs">
    <aside>
      <button class="back" onClick={() => out() && setBackstage(null)} title="Back">←</button>
      <For each={[["home", "home", "Home"], ["new", "new", "New"], ["open", "open", "Open"], ["save", "save", "Save"], ["saveas", "save", "Save As"], ["export", "export", "Export"], ["print", "print", "Print"], ["roadmap", "list", "Roadmap"]]}>{([k, i, l]) =>
        <button classList={{ on: backstage() === k }} onClick={() => {
          if (k === "open") C.open(); else if (k === "save") C.save(); else if (k === "saveas") C.save(true);
          else setBackstage(k === "new" ? "home" : k);
        }}><Icon n={i} size={22} />{l}</button>}</For>
      <span class="grow" />
      <button classList={{ on: backstage() === "options" }} onClick={() => setBackstage("options")}><Icon n="opts" size={22} />Options</button>
      <button classList={{ on: backstage() === "about" }} onClick={() => setBackstage("about")}><Icon n="upd" size={22} />Updates</button>
    </aside>
    <section class="bsmain">
      <Show when={backstage() === "home"}><Home /></Show>
      <Show when={backstage() === "export"}><Export /></Show>
      <Show when={backstage() === "options"}><OptionsPage /></Show>
      <Show when={backstage() === "print"}><PrintPage /></Show>
      <Show when={backstage() === "roadmap"}><RoadmapPage /></Show>
      <Show when={backstage() === "about"}><div class="about"><h1>Updates</h1><UpdateCard /><p><button class="btn" onClick={() => D.shortcuts()}>Keyboard shortcuts</button></p></div></Show>
    </section>
  </div>
);
