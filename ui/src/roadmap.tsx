// File › Roadmap: what Datum Mesh will do next. The list is in docs/site.json (also read by the
// Datum family site); keep it in step with docs/MEMORY.md (the roadmap table).
import { For } from "solid-js";
import { call } from "./store";

import site from "../../docs/site.json";

type Stage = { v: string; title: string; when: string; items: string[] };
export const ROADMAP: Stage[] = site.roadmap;

export const RoadmapPage = () => (
  <div class="roadmap">
    <h1>Roadmap</h1>
    <p class="muted">What Datum Mesh will do, and when. Releases are small and arrive through File › Updates. Ideas and bug reports are welcome on GitHub.</p>
    <p><button class="btn" onClick={() => call("open_url", { url: "https://github.com/Edgar-Mendonca/Datum-Mesh/issues/new" })}>Suggest a feature or report an issue</button>
      <button class="btn" onClick={() => call("open_url", { url: "https://github.com/Edgar-Mendonca/Datum-Mesh/releases" })}>Every release so far</button></p>
    <div class="rmlist">
      <For each={ROADMAP}>{s => (
        <section class="rmstage" classList={{ next: s.when === "Next" }}>
          <header><span class="rmv">{s.v}</span><b>{s.title}</b><span class="pill">{s.when}</span></header>
          <ul><For each={s.items}>{i => <li>{i}</li>}</For></ul>
        </section>
      )}</For>
    </div>
  </div>
);
