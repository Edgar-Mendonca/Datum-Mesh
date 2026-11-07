// Analysis of the solids modelled in step 1 (0.7.3). The solids are meshed by Gmsh into 10-node
// tetrahedra (shell `cad_mesh`), and the mesh becomes the model's solid, each of its faces named
// after the solid's face it lies on ("Body1/…"). From then on supports, loads, the solve and the
// results are those of any meshed solid. Step 1 shows the solids themselves; the later steps show
// the mesh, made again by itself whenever the solids have changed since.
import { createEffect, createSignal, on } from "solid-js";
import { model, setOut, tab, toast, setStale, results } from "./store";
import { invoke as _i, Out } from "./api";
import { cad, activeSketch, meshOld, setMeshOld } from "./cad";
import { isSolidA } from "./api";
import { beginRun, endRun } from "./console";

/** The model's solid is a mesh of the modelled solids. */
export const geoMesh = () => model()?.volume?.source === "geometry";
/** There are modelled solids to analyse. */
export const hasSolids = () => isSolidA(model()?.analysis) && !!cad()?.bodies.length && !model()?.parts?.length && !(model()?.shapes.length);
/** The 3D view shows the mesh (not the solids being modelled): after step 1, once meshed and current. */
export const showMesh = () => geoMesh() && !meshOld() && tab() !== "geometry" && tab() !== "home" && !activeSketch();

let busy: Promise<boolean> | null = null;
/** Mesh the solids now (Mesh › Mesh, or by itself when needed). */
export const meshSolids = (quiet = false): Promise<boolean> => {
  if (busy) return busy;
  const m = model(); if (!m) return Promise.resolve(false);
  busy = (async () => {
    try {
      const t0 = performance.now();
      beginRun("Meshing the solids (Gmsh)");
      const o = await _i<Out>("cad_mesh", { size: (m.mesh.size || 0) * 1e3 }).catch(e => { endRun(false, String(e)); throw e; });
      endRun(true);
      setOut(o); setMeshOld(false); if (results()) setStale(true);
      const v = o.m.volume;
      if (o.warnings.length) toast(o.warnings.join(" "));
      else if (!quiet && v) toast(`Meshed: ${v.elements.toLocaleString()} ${v.order === 2 ? "10-node" : "4-node"} tetrahedra, ${v.nodes.toLocaleString()} nodes, in ${((performance.now() - t0) / 1000).toFixed(1)} s.`, true);
      return true;
    } catch (e) { toast(String(e)); return false; }
    finally { busy = null; }
  })();
  return busy;
};
/** The solids were meshed apart for contact (0.8.0); with the contacts gone they are meshed
 *  joined again before the next solve. */
export const [meshApart, setMeshApart] = createSignal(false);
/** Mesh first if the solids have none, or have changed since. */
export const ensureMesh = async () => {
  if (meshApart() && !(model()?.contacts ?? []).length) { setMeshApart(false); return meshSolids(true); }
  return !hasSolids() || (geoMesh() && !meshOld()) || meshSolids(true);
};

// Supports, loads and the mesh are put on the mesh's faces: entering those steps meshes first.
createEffect(on(tab, t => { if ((t === "loads" || t === "mesh" || t === "properties") && hasSolids() && (!geoMesh() || meshOld())) meshSolids(true); }, { defer: true }));

/** A mesh face's name for people (0.7.3): "Body1/F2:side:l3" reads "Body1: Extrude1 side 3", by the
 * feature that made the face (its id after F) and the face's own words. */
export const faceLabel = (name: string) => {
  const k = name.indexOf("/"); if (k < 0) return name;
  const body = name.slice(0, k), rest = name.slice(k + 1).split(":");
  const fid = /^F(\d+)$/.exec(rest[0]), feat = fid ? cad()?.features.find(f => f.id === +fid[1])?.name ?? `Feature ${fid[1]}` : rest[0];
  const words = (fid ? rest.slice(1) : rest.slice(1)).map(w => w.replace(/^l(\d+)$/, "$1").replace(/^e(\d+)$/, "edge $1").replace(/_/g, " ")).join(" ");
  return `${body}: ${feat}${words ? ` ${words}` : ""}`;
};
