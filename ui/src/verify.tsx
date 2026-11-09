// Help › Verification and Help › Command Line (moved out of dialogs.tsx in 0.10.8 so the window
// starts lighter): every benchmark solved now by this version, and how to run Datum Mesh without
// its window.
import { For, Show, createResource } from "solid-js";
import { Check, num } from "./api";
import { openDialog, call, toast } from "./store";
import { Dialog, Icon } from "./ui";
import { Q, fmt } from "./units";

/** Help › Verification: every benchmark solved now by this version, against its reference. */
/** Help › Command Line and AI Agents (0.10.5): where datum-mesh-cli is and how to connect an agent. */
export const cliHelp = () => openDialog(() => {
  const [path] = createResource(() => call<string | null>("cli_path").catch(() => null));
  const exe = () => path() ?? "datum-mesh-cli";
  const q = (s: string) => (s.includes(" ") ? `"${s}"` : s);
  const lines = () => [
    ["Every command and its parameters", `${q(exe())} commands`],
    ["Solve a model, the summary as JSON", `${q(exe())} solve model.dmesh`],
    ["One command on a model", `${q(exe())} call mech_checks --file model.dmesh`],
    ["A script of commands", `${q(exe())} run script.json`],
    ["Run the verification benchmarks", `${q(exe())} verify`],
    ["The MCP server, for an AI agent (add it as a server with this command)", `${q(exe())} mcp`],
  ];
  return <Dialog title="Command line and AI agents" help="cli" okText="Close">
    <p class="small">Datum Mesh also runs without its window: <b>datum-mesh-cli</b> makes, edits, solves and checks models with the same engine and the same edits as the window, and answers in JSON. AI agents drive it through MCP, the Model Context Protocol. Units are SI (m, N, Pa, K).</p>
    <p class="small muted">{path() ? `Installed at ${path()}` : "Not found beside this copy of Datum Mesh (a development build): build it with cargo build -p datum-mesh-cli."}</p>
    <table class="tbl clitab"><tbody><For each={lines()}>{([w, c]) => <tr><td class="small">{w}</td><td><code class="mono small">{c}</code></td><td><button class="btn sm" title="Copy" onClick={() => navigator.clipboard?.writeText(c).then(() => toast("Copied.", true)).catch(() => {})}>Copy</button></td></tr>}</For></tbody></table>
    <p class="muted small">Not yet from the command line: meshing solids drawn with the modelling tools (they need the window's geometry engine) and pictures of results.</p>
  </Dialog>;
});

export const verification = () => openDialog(() => {
  const [rows] = createResource(() => call<Check[]>("verification"));
  const [gaps] = createResource(() => call<{ what: string; why: string }[]>("not_verified").catch(() => []));
  const [ver] = createResource(() => call<string>("version"));
  const val = (q: string, v: number) => (q === "ratio" ? num(v, 6) : q === "rad" ? `${num(v, 6)} rad` : fmt(q as Q, v, 6));
  return (
    <Dialog title="Verification" wide help="verification">
      <div class="verif">
        <p>Every check below was solved just now by Datum Mesh {ver()} and compared with a known answer. A check fails if its error is over the tolerance; the same checks run as tests before every release.</p>
        <Show when={rows()} fallback={<p class="muted">Solving the benchmarks…</p>}>{r => <>
          <p classList={{ okline: r().every(x => x.pass), badline: !r().every(x => x.pass) }}><Icon n={r().every(x => x.pass) ? "check" : "info"} /> {r().filter(x => x.pass).length} of {r().length} checks pass.</p>
          <table class="tbl"><thead><tr><th>#</th><th>Benchmark</th><th>Quantity</th><th class="r">Datum Mesh</th><th class="r">Reference</th><th class="r">Error</th><th>Source</th><th /></tr></thead><tbody>
            <For each={r()}>{x => <tr><td>{x.bench}</td><td>{x.name}</td><td>{x.quantity}</td><td class="r">{val(x.q, x.result)}</td><td class="r">{val(x.q, x.reference)}</td><td class="r">{x.error < 1e-12 ? "< 1e-12" : x.error.toExponential(1)}</td><td>{x.source}</td><td>{x.pass ? "✓ pass" : "✗ fail"}</td></tr>}</For></tbody></table>
          <h3>Failed</h3>
          <Show when={r().some(x => !x.pass)} fallback={<p class="small okline" data-failed="0">None: every check above passed.</p>}>
            <ul class="small badline" data-failed={r().filter(x => !x.pass).length}><For each={r().filter(x => !x.pass)}>{x => <li>{x.bench} · {x.name}: {x.quantity} ({val(x.q, x.result)} against {val(x.q, x.reference)})</li>}</For></ul>
          </Show>
          <h3>Not yet verified</h3>
          <p class="muted small">What these checks do not yet cover, and what was measured. Nothing here is claimed as verified.</p>
          <ul class="small gaps"><For each={gaps() ?? []}>{g => <li><b>{g.what}.</b> {g.why}</li>}</For></ul>
          <h3>Mesh convergence</h3>
          <p class="muted small">The same model on finer meshes: the error falls as the elements get smaller.</p>
          <div class="conv"><For each={r().filter(x => x.steps.length)}>{x => <table class="tbl"><thead><tr><th colSpan={2}>{x.name}: {x.quantity}</th></tr><tr><th class="r">Elements</th><th class="r">Error</th></tr></thead><tbody>
            <For each={x.steps}>{st => <tr><td class="r">{st[0].toLocaleString()}</td><td class="r">{st[1] < 1e-12 ? "< 1e-12" : st[1].toExponential(1)}</td></tr>}</For></tbody></table>}</For></div>
        </>}</Show>
      </div>
    </Dialog>
  );
});
