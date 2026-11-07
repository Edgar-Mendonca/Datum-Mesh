// "?" help: a small button beside a tool; a click opens a short card saying what the tool is for
// and how to use it. One place for every explanation, so the whole app teaches itself.
import { For, Show, createSignal } from "solid-js";
import { openDialog } from "./store";
import { Dialog } from "./ui";

import type { Topic } from "./helptext";

/** The words, loaded just after the window opens (0.10.2: they were a tenth of the startup code). */
const [HELP, setHelp] = createSignal<Record<string, Topic>>({});
const load = () => import("./helptext").then(k => { setHelp(k.HELP); return k.HELP; });
setTimeout(load, 0);
/** Every topic, for Help › What Is This Tool?. */
export const helpTopics = () => load().then(h => Object.entries(h));

/** The small "?" beside a tool. */
export const Help = (p: { k: string }) => <button class="help" title={`What is ${HELP()[p.k]?.t ?? "this"}?`} onClick={e => { e.stopPropagation(); e.preventDefault(); helpCard(p.k); }}>?</button>;

export const helpCard = (k: string) => load().then(all => openDialog(() => {
  const h = all[k];
  return (
    <Dialog title={h?.t ?? "Help"}>
      <div class="helpcard"><p>{h?.what}</p>
        <Show when={h?.how?.length}><h4>How to use it</h4><ul><For each={h!.how}>{x => <li>{x}</li>}</For></ul></Show>
        <Show when={h?.tip}><p class="muted">{h!.tip}</p></Show></div>
    </Dialog>
  );
}));
