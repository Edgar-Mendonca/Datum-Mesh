// Mesh statistics (0.10.3): the mesh's elements, sizes and shapes in a table under the view, for
// 2D parts and 3D solids. The engine measures them (solids/mesh.rs, tet.rs); this only shows them.
import { For, Show } from "solid-js";
import { Quality, num, isSolidA } from "./api";
import { model, meshed, results } from "./store";
import { fmt } from "./units";

/** The mesh's quality: a 2D part's mesh, or a 3D solid's (made by meshing, or after a solve). */
export const meshQuality = (): { q: Quality; nodes: number; kind: string } | null => {
  const m = model(); if (!m) return null;
  if (isSolidA(m.analysis)) {
    const v = m.volume ?? m.assembly; if (v?.quality?.elements) return { q: v.quality, nodes: v.nodes, kind: "tetrahedra" };
    const r = results() as { quality?: Quality; nodes?: number; elements?: number } | null;
    return r?.quality?.elements ? { q: r.quality, nodes: r.nodes ?? 0, kind: "elements" } : null;
  }
  const ms = meshed(); if (ms?.quality?.elements) return { q: ms.quality, nodes: ms.nodes.length, kind: "triangles" };
  const r = results() as { mesh?: { quality?: Quality; nodes?: unknown[] } } | null;
  return r?.mesh?.quality?.elements ? { q: r.mesh.quality, nodes: (r.mesh.nodes ?? []).length, kind: "triangles" } : null;
};

const ANGLE = ["over 30°", "20–30°", "10–20°", "under 10°"], ASPECT = ["up to 2", "2–5", "5–10", "over 10"], VERDICT = ["Good", "Fair", "Poor", "Bad"];

export const MeshStats = () => {
  const s = () => meshQuality();
  const pct = (n: number, all: number) => `${num(100 * n / Math.max(all, 1), 3)} %`;
  return <div class="mstats">
    <Show when={s()} fallback={<p class="muted cempty">No mesh yet. Mesh the model (Mesh › Mesh Now, or Solve) and its statistics show here.</p>}>{st => {
      const q = () => st().q, n = () => q().elements ?? 0;
      return <div class="msgrid">
        <section><h4>The mesh</h4><table class="tbl ttab"><tbody>
          <tr><td>Elements</td><td class="r mono">{n()} {st().kind}</td></tr>
          <tr><td>Nodes</td><td class="r mono">{st().nodes}</td></tr>
          <tr><td>Edge length: smallest · mean · largest</td><td class="r mono">{(q().edges ?? [0, 0, 0]).map(v => fmt("len", v, 3)).join(" · ")}</td></tr>
          <tr><td>Smallest angle anywhere</td><td class="r mono">{num(q().min_angle, 3)}°</td></tr>
          <tr><td>Worst aspect ratio</td><td class="r mono">{num(q().max_aspect, 3)}</td></tr>
        </tbody></table></section>
        <section><h4>Shape of the elements</h4><table class="tbl ttab" data-table="bands"><thead><tr><th /><th>Smallest angle</th><th class="r">Elements</th><th>Aspect ratio</th><th class="r">Elements</th></tr></thead><tbody>
          <For each={[0, 1, 2, 3]}>{k => <tr><td><span class={`badge q${k}`}>{VERDICT[k]}</span></td><td>{ANGLE[k]}</td><td class="r mono">{q().angle_bands?.[k] ?? 0} <span class="muted">({pct(q().angle_bands?.[k] ?? 0, n())})</span></td><td>{ASPECT[k]}</td><td class="r mono">{q().aspect_bands?.[k] ?? 0} <span class="muted">({pct(q().aspect_bands?.[k] ?? 0, n())})</span></td></tr>}</For>
        </tbody></table><p class="muted small">Stresses in poor and bad elements are less trustworthy; refine the mesh near them (Mesh › Finer Near) or round the corner they sit in.</p></section>
        <section><h4>The worst elements</h4><table class="tbl ttab" data-table="worst"><thead><tr><th>Element</th><th>Where (x, y{isSolidA(model()?.analysis) ? ", z" : ""})</th><th class="r">Smallest angle</th><th class="r">Aspect</th></tr></thead><tbody>
          <For each={q().worst ?? []}>{w => <tr><td>#{w.element + 1}</td><td class="mono">{(isSolidA(model()?.analysis) ? w.at : w.at.slice(0, 2)).map(v => fmt("coord", v, 4)).join(", ")}</td><td class="r mono">{num(w.angle, 3)}°</td><td class="r mono">{num(w.aspect, 3)}</td></tr>}</For>
        </tbody></table></section>
      </div>;
    }}</Show>
  </div>;
};
