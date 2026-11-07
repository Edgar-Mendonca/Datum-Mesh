// The 3D view of a 3D frame (0.2.4), drawn with three.js: members, nodes, supports and loads; once
// solved, the deflected shape coloured by a result, with a probe that reads the values under the
// pointer. Turn with the left button, pan with the right, zoom with the wheel. Clicking picks
// nodes and members (and places supports and loads); drawing happens on a working plane.
import { sectionsOn } from "./store";
import type { SectionShape } from "./api";
import { cad, activeSketch, SketchCanvas, newSketch, cad3Tool, cad3Click, CadHit, sheetView, planePick, setPlanePick, turnBack, pick3, setMeshOld } from "./cad";
import { Timeline, bodyMenu } from "./history";
import { showMesh, geoMesh, meshSolids } from "./study";
import { ExtrudePanel } from "./extrude";
import { For, Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { LineSegments2 } from "three/examples/jsm/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { LineMaterial } from "three/examples/jsm/lines/LineMaterial.js";
import { Model, SpaceMember, Target, clean, isSolidA, solidDisp, invoke } from "./api";
import { solved, sketch, sketchPick, setSketchPick, model, sel, setSel, tool, setTool, act, spaceRes, stale, show, themeTick, opts, toast, setEdgeSel, modes, modeOn, modeSel, phase, solidRes, shownLoad } from "./store";
import { palette, BANDS, curField, rangeOf } from "./parts";
import { outline, edgePts, inside } from "./geom";
import { defaultLoad, defaultSupport } from "./panels";
import { fmt, unit } from "./units";
import { scale, pan } from "./canvas";
import { sketchFrame, placeOn } from "./sketchmode";
import { draft, SweepPanel, toolOnPart, PartHit } from "./modelling";
import { setPlane, fit3d } from "./plane3";
import { toD } from "./axes";
import { C } from "./cmds";
import { P3, at3, span3, dlen, deform3, vals, q3, range3, cut, pq, plateRange } from "./space3";
import { plate3 } from "./plane3";
import { modeAmp } from "./modes";

const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#888";


export const View3D = () => {
  let host!: HTMLDivElement;
  const [probe, setProbe] = createSignal<{ mr: SpaceMember; i: number; x: number; y: number } | null>(null);
  const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
  renderer.setPixelRatio(window.devicePixelRatio || 1);
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(40, 1, 0.01, 1e6);
  camera.up.set(0, 1, 0);
  let controls: OrbitControls | null = null;
  const world = new THREE.Group();
  scene.add(world);
  // A mode or buckled shape (0.2.6), redrawn alone as it swings.
  const modeGroup = new THREE.Group();
  scene.add(modeGroup);
  const mode3 = () => { const r = modes(); return modeOn() && r && (r.kind === "space" || r.kind === "buckle3") ? r.modes[modeSel()] ?? null : null; };
  /** A 3D solid's mode (0.3.4): its surface and the movement of each surface node. */
  const modeSolid = () => { const r = modes(); return modeOn() && r?.kind === "solid" && r.modes[modeSel()] ? { s: r.surface, d: r.modes[modeSel()].disp } : null; };
  scene.add(new THREE.AmbientLight(0xffffff, 2.2));
  const sun = new THREE.DirectionalLight(0xffffff, 1.4);
  sun.position.set(1, 2, 1.5);
  scene.add(sun);
  const lineMats: LineMaterial[] = [];
  const size = () => [host?.clientWidth || 1, host?.clientHeight || 1];
  let frame = 0;
  const draw = () => { if (frame) return; frame = requestAnimationFrame(() => { frame = 0; camera.updateMatrixWorld(); gizmo(); adaptFloor(); renderer.render(scene, cam()); }); };

  // The stage (0.6.1): a floor grid fixed in size, fading into the distance, the axes (Z up, as
  // shown: red X, green Y, blue Z) and, while nothing is drawn or a plane is to be picked, the
  // three planes at the origin. Its size is set when a model opens and only ever grows, so the
  // scene does not jump about while you draw.
  const nice = (v: number) => { const p = 10 ** Math.floor(Math.log10(v)), k = v / p; return (k <= 1 ? 1 : k <= 2 ? 2 : k <= 5 ? 5 : 10) * p; };
  const stage = { key: "", L: 0 };
  const stageL = (m: Model) => {
    const key = `${m.name}|${m.analysis}`, b = boxOf(m);
    const span = b.isEmpty() ? (isSolidA(m.analysis) ? 0.1 : 6) : Math.max(...b.getSize(new THREE.Vector3()).toArray(), b.max.length(), b.min.length());
    const want = nice(span * 2.5);
    if (key !== stage.key || want > stage.L) { stage.key = key; stage.L = Math.max(want, key === stage.key ? stage.L : 0); }
    return stage.L;
  };
  // The floor grid (0.7.2, the user): big squares with small squares inside, sized to the zoom
  // (a closer look gives smaller squares), centred under what the view looks at and fading out
  // around it. Built in its own group, again only when the squares' size or place changes.
  const floorGrp = new THREE.Group();
  scene.add(floorGrp);
  let floorOn = false, floorL = 1, floorKey = "";
  const floor = (L: number) => { floorOn = true; floorL = L; };
  const adaptFloor = () => {
    floorGrp.visible = floorOn && !cadFacing(); // the sheet has its own grid
    if (!floorOn) return;
    const t = controls?.target ?? new THREE.Vector3(), d = Math.max(camera.position.distanceTo(t), 1e-4);
    const minor = nice(d / 30), mant = Math.round(minor / 10 ** Math.floor(Math.log10(minor) + 1e-9)), major = minor * (mant === 5 ? 10 : 5);
    const R = Math.ceil(Math.max(floorL, d * 3) / major) * major, Rm = Math.min(R, d * 1.2);
    const cx = Math.round(t.x / major) * major, cz = Math.round(t.z / major) * major;
    const bgc = css("--bg2"), c1 = css("--line2"), c2 = css("--line3");
    const key = `${minor}|${cx}|${cz}|${R}|${bgc}|${c1}|${c2}`;
    if (key === floorKey) return;
    floorKey = key;
    floorGrp.children.forEach(o => { const x = o as any; x.geometry?.dispose?.(); x.material?.dispose?.(); });
    floorGrp.clear();
    const bg = new THREE.Color(bgc), cMin = new THREE.Color(c2).lerp(bg, 0.35), cMaj = new THREE.Color(c2).lerp(new THREE.Color(css("--text")), 0.18);
    const pos: number[] = [], col: number[] = [];
    // A line in short pieces, each coloured by its distance from the centre (out to `reach`).
    const seg = (a: [number, number], b: [number, number], c: THREE.Color, reach: number) => {
      const k = 16;
      for (let i = 0; i < k; i++) for (const u of [i / k, (i + 1) / k]) {
        const x = a[0] + (b[0] - a[0]) * u, z = a[1] + (b[1] - a[1]) * u, f = Math.min(1, Math.hypot(x - cx, z - cz) / reach) ** 1.5, cc = c.clone().lerp(bg, f);
        pos.push(x, 0, z); col.push(cc.r, cc.g, cc.b);
      }
    };
    const nm = Math.ceil(Rm / minor), nM = Math.round(R / major), per = Math.round(major / minor);
    for (let i = -nm; i <= nm; i++) {
      if (i % per === 0) continue;
      const v = i * minor;
      seg([cx + v, cz - Rm], [cx + v, cz + Rm], cMin, Rm); seg([cx - Rm, cz + v], [cx + Rm, cz + v], cMin, Rm);
    }
    for (let i = -nM; i <= nM; i++) {
      const v = i * major;
      seg([cx + v, cz - R], [cx + v, cz + R], cMaj, R); seg([cx - R, cz + v], [cx + R, cz + v], cMaj, R);
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(pos), 3));
    g.setAttribute("color", new THREE.BufferAttribute(new Float32Array(col), 3));
    floorGrp.add(new THREE.LineSegments(g, new THREE.LineBasicMaterial({ vertexColors: true })));
  };
  const axes = (L: number) => {
    const a = L * 0.12;
    for (const [v, c] of [[[a, 0, 0], "#D64545"], [[0, 0, -a], "#2E9D57"], [[0, a, 0], "#2F6FD0"]] as [number[], string][]) {
      world.add(new THREE.Line(new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array([0, 0, 0, ...v]), 3)), new THREE.LineBasicMaterial({ color: new THREE.Color(c) })));
      world.add(new THREE.Line(new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array([0, 0, 0, ...v.map(x => -x)]), 3)), new THREE.LineBasicMaterial({ color: new THREE.Color(c), transparent: true, opacity: 0.3 })));
    }
  };
  /** The three planes at the origin, each a small square in its own colour: click one to draw on it. */
  const originPlanes = (L: number) => {
    const s = L * 0.1;
    const quads: [string, number[][]][] = [["xy", [[0, 0, 0], [s, 0, 0], [s, s, 0], [0, s, 0]]], ["zy", [[0, 0, 0], [0, 0, -s], [0, s, -s], [0, s, 0]]], ["xz", [[0, 0, 0], [s, 0, 0], [s, 0, -s], [0, 0, -s]]]];
    for (const [k, q] of quads) {
      const g = new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array([...q[0], ...q[1], ...q[2], ...q[0], ...q[2], ...q[3]]), 3));
      const on = false;
      const o = new THREE.Mesh(g, new THREE.MeshBasicMaterial({ color: new THREE.Color("#E8A26A"), transparent: true, opacity: on ? 0.55 : 0.3, side: THREE.DoubleSide, depthWrite: false }));
      o.userData.origin = k;
      world.add(o);
      world.add(new THREE.LineLoop(new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array(q.flat()), 3)), new THREE.LineBasicMaterial({ color: new THREE.Color("#C0703A") })));
    }
  };
  /** The origin plane under the pointer, if any. */
  const pickOrigin = (x: number, y: number): "xy" | "zy" | "xz" | null => {
    const [w, h] = size(), ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(x / w * 2 - 1, 1 - y / h * 2), camera);
    const hit = ray.intersectObjects(world.children.filter(o => o.userData.origin), false)[0];
    if (!hit) return null;
    // A part in front of the square hides it (0.6.3: its face is picked instead).
    const part = ray.intersectObjects(world.children.filter(o => o.userData.pick), false)[0];
    return part && part.distance < hit.distance ? null : hit.object.userData.origin;
  };

  // The orientation cube and the axes key (0.6.1), turned with the view.
  let cubeEl: SVGSVGElement | undefined, keyEl: SVGSVGElement | undefined;
  /** The cube's faces as last drawn: outline in the view box, depth (nearer is larger), shown. */
  let cubePolys: { k: number; pts: [number, number][]; depth: number }[] = [];
  /** The cube's faces: name, outward normal (engine axes), and the view direction to look from. */
  /** Look at the model from along a face's normal (a little off straight up or down, to keep the
   * camera's up direction defined). */
  const lookFrom = (d: P3) => fit(false, [d[0], d[1], d[2] + (d[1] ? 0.001 : 0)]);
  const FACES: [string, P3][] = [["Front", [0, 0, 1]], ["Back", [0, 0, -1]], ["Right", [1, 0, 0]], ["Left", [-1, 0, 0]], ["Top", [0, 1, 0]], ["Bottom", [0, -1, 0]]];
  const gizmo = () => {
    const e = (cam() as THREE.Camera).matrixWorldInverse.elements, R = (i: number, j: number) => e[j * 4 + i], S = [1, -1, 1];
    void S;
    if (cubeEl) {
      // Each face's four corners turned with the view; a face shows when it faces the viewer.
      const rot = (v: number[]) => [0, 1, 2].map(i => R(i, 0) * v[0] + R(i, 1) * v[1] + R(i, 2) * v[2]);
      const order: [number, number][] = [], gs = FACES.map(([name]) => cubeEl!.querySelector(`[data-face="${name}"]`) as SVGGElement);
      cubePolys = [];
      FACES.forEach(([, n], k) => {
        const a = n.findIndex(c => c !== 0), [u, w] = [0, 1, 2].filter(i => i !== a);
        const pts = [[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([p, q]) => { const v = [0, 0, 0]; v[a] = n[a]; v[u] = p; v[w] = q; return rot(v); });
        const g = gs[k], depth = rot(n)[2];
        g.style.display = depth > 0.02 ? "" : "none";
        const xy = pts.map(p => [32 + 17 * p[0], 32 - 17 * p[1]] as [number, number]);
        g.querySelector("polygon")!.setAttribute("points", xy.map(p => `${p[0].toFixed(1)},${p[1].toFixed(1)}`).join(" "));
        if (depth > 0.02) cubePolys.push({ k, pts: xy, depth });
        const t = g.querySelector("text")!; t.setAttribute("x", (32 + 17 * rot(n)[0]).toFixed(1)); t.setAttribute("y", (35 - 17 * rot(n)[1]).toFixed(1));
        t.style.opacity = String(Math.min(1, depth * 1.6));
        order.push([depth, k]);
      });
      // Nearer faces on top.
      order.sort((x, y) => x[0] - y[0]).forEach(([, k]) => cubeEl!.appendChild(gs[k]));
    }
    if (keyEl) {
      // The shown axes X, Y, Z are the engine's x, −z, y.
      [[1, 0, 0], [0, 0, -1], [0, 1, 0]].forEach((v, k) => {
        const sx = R(0, 0) * v[0] + R(0, 1) * v[1] + R(0, 2) * v[2], sy = R(1, 0) * v[0] + R(1, 1) * v[1] + R(1, 2) * v[2];
        const l = keyEl!.children[k] as SVGGElement;
        l.querySelector("line")!.setAttribute("x2", String(26 + 20 * sx)); l.querySelector("line")!.setAttribute("y2", String(26 - 20 * sy));
        l.querySelector("text")!.setAttribute("x", String(26 + 25 * sx)); l.querySelector("text")!.setAttribute("y", String(29 - 25 * sy));
      });
    }
  };

  /** A section's outline (m), (y, z) in its local axes: y the depth, z the width (0.7.5). */
  const sectionRing = (s: SectionShape): [number, number][] => {
    const circ = (r: number, n = 20) => Array.from({ length: n }, (_, i) => [r * Math.cos(2 * Math.PI * i / n), r * Math.sin(2 * Math.PI * i / n)] as [number, number]);
    switch (s.kind) {
      case "rect": return [[-s.h / 2, -s.b / 2], [s.h / 2, -s.b / 2], [s.h / 2, s.b / 2], [-s.h / 2, s.b / 2]];
      case "box": return [[-s.h / 2, -s.b / 2], [s.h / 2, -s.b / 2], [s.h / 2, s.b / 2], [-s.h / 2, s.b / 2]];
      case "circle": return circ(s.d / 2);
      case "tube": return circ(s.d / 2);
      case "i": { const [h, b, tw, tf] = [s.h / 2, s.b / 2, s.tw / 2, s.tf]; return [[-h, -b], [-h, b], [-h + tf, b], [-h + tf, tw], [h - tf, tw], [h - tf, b], [h, b], [h, -b], [h - tf, -b], [h - tf, -tw], [-h + tf, -tw], [-h + tf, -b]]; }
      case "channel": { const [h, b, tw, tf] = [s.h / 2, s.b, s.tw, s.tf]; return [[-h, 0], [-h, b], [-h + tf, b], [-h + tf, tw], [h - tf, tw], [h - tf, b], [h, b], [h, 0]]; }
      case "angle": { const [h, b, t] = [s.h, s.b, s.t]; return [[0, 0], [0, b], [t, b], [t, t], [h, t], [h, 0]]; }
      case "custom": { const d = Math.sqrt(Math.max(s.a, 1e-8)) / 2; return [[-d, -d], [d, -d], [d, d], [-d, d]]; }
      default: return [];
    }
  };
  const clear = () => {
    lineMats.length = 0;
    world.traverse(o => { const x = o as any; x.geometry?.dispose?.(); (Array.isArray(x.material) ? x.material : [x.material]).forEach((mt: any) => mt?.dispose?.()); });
    world.clear();
    floorOn = false;
  };
  const fat = (pts: number[], colours: number[] | null, colour: string, width: number, dashed = false) => {
    const g = new LineSegmentsGeometry();
    g.setPositions(pts);
    if (colours) g.setColors(colours);
    const [w, h] = size();
    const mt = new LineMaterial({ color: colours ? 0xffffff : new THREE.Color(colour).getHex(), linewidth: width, vertexColors: !!colours, dashed, dashSize: 0.3, gapSize: 0.2, worldUnits: false });
    mt.resolution.set(w, h);
    lineMats.push(mt);
    const l = new LineSegments2(g, mt);
    if (dashed) l.computeLineDistances();
    world.add(l);
    return l;
  };

  /** Build the scene from the model, the selection and the results. */
  // 3D solids (0.3.0): the deflection scale drawn, for the probe.
  let solidF = 0;
  /** A 3D solid: the extruded part with its supported and loaded faces outlined; once solved,
   * the surface coloured by the chosen result on the deformed shape. */
  /** 0.7.1: the geometry engine's solids and finished sketches. The engine models in millimetres
   * with Z up; the view keeps the engine's metres with y up: (X, Y, Z) mm → (X, Z, −Y) m. */
  const addCad = (bodies = true) => {
    const c = cad(); if (!c) return;
    const sky = new THREE.Color(css("--solid")), ink = new THREE.Color(css("--text")), faint = new THREE.Color(css("--blue2"));
    const put = (a: Float32Array, i: number, x: number, y: number, z: number) => { a[i] = x * 1e-3; a[i + 1] = z * 1e-3; a[i + 2] = -y * 1e-3; };
    const lines = (pts: number[], col: THREE.Color) => { const a = new Float32Array(pts.length); for (let i = 0; i < pts.length; i += 3) put(a, i, pts[i], pts[i + 1], pts[i + 2]); const g = new THREE.BufferGeometry(); g.setAttribute("position", new THREE.BufferAttribute(a, 3)); world.add(new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color: col }))); };
    for (const b of bodies ? c.bodies : []) {
      const pos = new Float32Array(b.pos.length), nrm = new Float32Array(b.nrm.length);
      for (let i = 0; i < b.pos.length; i += 3) put(pos, i, b.pos[i], b.pos[i + 1], b.pos[i + 2]);
      for (let i = 0; i < b.nrm.length; i += 3) { nrm[i] = b.nrm[i]; nrm[i + 1] = b.nrm[i + 2]; nrm[i + 2] = -b.nrm[i + 1]; }
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
      if (nrm.length === pos.length) g.setAttribute("normal", new THREE.BufferAttribute(nrm, 3)); else g.computeVertexNormals();
      g.setIndex(b.tris);
      const o = new THREE.Mesh(g, new THREE.MeshLambertMaterial({ color: sky, side: THREE.DoubleSide, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }));
      o.userData.cadBody = b.name; world.add(o);
      o.userData.cadFace = b.face;
      const seg: number[] = [];
      for (const e of b.edges) for (let i = 0; i + 5 < e.length; i += 3) seg.push(e[i], e[i + 1], e[i + 2], e[i + 3], e[i + 4], e[i + 5]);
      lines(seg, ink);
      world.children[world.children.length - 1].userData.cadEdges = b.name;
    }
    // Finished sketches; construction lines (a frame's grid, 0.7.6) dashed.
    const dash: number[] = [], lo = [Infinity, Infinity], hi = [-Infinity, -Infinity];
    for (const s of c.sketches) {
      if (s.id === c.active) continue;
      const seg: number[] = [], at = (p: [number, number]) => [0, 1, 2].map(k => s.origin[k] + p[0] * s.x[k] + p[1] * s.y[k]);
      for (const cv of s.curves) for (let i = 0; i + 1 < cv.pts.length; i++) {
        const a = at(cv.pts[i]), b = at(cv.pts[i + 1]);
        (cv.construction ? dash : seg).push(...a, ...b);
        for (const q of [a, b]) for (const k of [0, 1]) { lo[k] = Math.min(lo[k], q[k]); hi[k] = Math.max(hi[k], q[k]); }
      }
      lines(seg, faint);
    }
    if (dash.length) {
      const a = new Float32Array(dash.length); for (let i = 0; i < dash.length; i += 3) put(a, i, dash[i], dash[i + 1], dash[i + 2]);
      const g = new THREE.BufferGeometry(); g.setAttribute("position", new THREE.BufferAttribute(a, 3));
      const o = new THREE.LineSegments(g, new THREE.LineDashedMaterial({ color: faint, dashSize: 0.25, gapSize: 0.15 })); o.computeLineDistances(); world.add(o);
    }
    // Levels (0.7.6): each plane parallel to the ground drawn as an outline over the sketches.
    if (isFinite(lo[0])) {
      const lv: number[] = [];
      for (const p of c.planes ?? []) {
        const pl = p.plane; if (!pl || Math.abs(pl.x_dir[2]) > 1e-9 || Math.abs(pl.y_dir[2]) > 1e-9) continue;
        const z = pl.origin[2];
        lv.push(lo[0], lo[1], z, hi[0], lo[1], z, hi[0], lo[1], z, hi[0], hi[1], z, hi[0], hi[1], z, lo[0], hi[1], z, lo[0], hi[1], z, lo[0], lo[1], z);
      }
      if (lv.length) lines(lv, new THREE.Color(css("--line3")));
    }
  };
  const buildSolid = (m: Model) => {
    const ink = css("--text"), green = css("--green"), load = css("--blue2"), accent = css("--blue");
    scene.background = new THREE.Color(css("--bg2"));
    const t = m.thickness, tol = Math.max(1e-6, (m.grid || 0.005) / 4), rev = m.revolve ?? 0, full = rev >= Math.PI * 2 - 1e-9;
    // An imported mesh, or an assembly of parts (0.6.0) meshed and joined: drawn from their surface.
    const asm = m.assembly, vp = m.volume ?? asm, bounds: number[][] = vp ? vp.surface.nodes : m.shapes.flatMap(s => outline(s.geom, tol));
    const size = Math.max(rev ? 0 : t, ...(bounds.length ? [Math.max(...bounds.map(p => p[0])) - Math.min(...bounds.map(p => p[0])), Math.max(...bounds.map(p => p[1])) - Math.min(...bounds.map(p => p[1]))] : [1e-3]));
    const L = stageL(m);
    if (opts().grid) floor(L);
    axes(L);
    if (picking() || planePick() || (!m.shapes.length && !m.volume && !m.parts?.length && !sketch() && !activeSketch() && !cad()?.bodies.length)) originPlanes(L);
    renderer.localClippingEnabled = false;
    /** A section point a fraction f of the way along the sweep, in 3D. */
    const sweep = (p: number[], f: number): P3 => rev ? [p[0] * Math.cos(rev * f), p[1], p[0] * Math.sin(rev * f)] : [p[0], p[1], t * f];
    const loop = (pts: number[], ps: number[][], f: number) => { for (let i = 0; i + 1 < ps.length; i++) pts.push(...sweep(ps[i], f), ...sweep(ps[i + 1], f)); };
    const r = solidRes();
    if (modeSolid()) {
      // A mode is shown: it is drawn, swinging, by the mode effect below.
    } else if (r && r.surface.nodes.length) {
      const fd = curField(), fld = r.fields.find(f => f.key === fd.key) ?? r.fields[0];
      const cs = palette(opts().contours, fd.reverse).map(c => new THREE.Color(c)), rg = rangeOf(fd, fld.v) ?? [fld.min, fld.max];
      const disp = solidDisp(r), dmax = Math.max(0, ...solidDisp(solved(r)).map(d => Math.hypot(d[0], d[1], d[2])));
      solidF = show().deformed && r.kind === "solid" && r.large ? show().scale : show().deformed && dmax > 1e-12 * size ? (() => { const f = 0.08 * size / dmax, p = 10 ** Math.floor(Math.log10(f)), k = f / p; return (k >= 5 ? 5 : k >= 2 ? 2 : 1) * p * show().scale; })() : 0;
      const pos = new Float32Array(r.surface.nodes.length * 3), col = new Float32Array(r.surface.nodes.length * 3);
      r.surface.nodes.forEach((p, i) => {
        const d = disp[i];
        for (let k = 0; k < 3; k++) pos[3 * i + k] = p[k] + d[k] * solidF;
        const c = cs[Math.max(0, Math.min(BANDS - 1, Math.floor((fld.v[i] - rg[0]) / ((rg[1] - rg[0]) || 1) * BANDS)))];
        col[3 * i] = c.r; col[3 * i + 1] = c.g; col[3 * i + 2] = c.b;
      });
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
      g.setAttribute("color", new THREE.BufferAttribute(col, 3));
      g.setIndex(r.surface.tris.flat());
      g.computeVertexNormals();
      // The cutting plane (0.3.2): the part beyond it hidden, the inside drawn on the plane.
      const ct = cut(), vol = r.volume, ax = { x: 0, y: 1, z: 2, off: -1 }[ct.axis];
      let clip: THREE.Plane[] = [];
      if (vol && ax >= 0) {
        const fi = r.fields.indexOf(fld), q = vol.nodes.map((p, i) => [p[0] + vol.disp[i][0] * solidF, p[1] + vol.disp[i][1] * solidF, p[2] + vol.disp[i][2] * solidF]);
        let lo = Infinity, hi = -Infinity;
        for (const p of q) { lo = Math.min(lo, p[ax]); hi = Math.max(hi, p[ax]); }
        const c = lo + (hi - lo) * Math.min(0.999, Math.max(0.001, ct.at)), nrm = new THREE.Vector3(); nrm.setComponent(ax, -1);
        clip = [new THREE.Plane(nrm, c)];
        renderer.localClippingEnabled = true;
        const vs = vol.v[fi] ?? [], cp: number[] = [], cc: number[] = [];
        const colour = (v: number) => cs[Math.max(0, Math.min(BANDS - 1, Math.floor((v - rg[0]) / ((rg[1] - rg[0]) || 1) * BANDS)))];
        const put = (i: number, j: number) => { const a = q[i][ax] - c, b = q[j][ax] - c, s2 = a / (a - b); for (let k = 0; k < 3; k++) cp.push(q[i][k] + (q[j][k] - q[i][k]) * s2); const col = colour(vs[i] + (vs[j] - vs[i]) * s2); cc.push(col.r, col.g, col.b); };
        for (const tt of vol.tets) {
          const up = tt.filter(i => q[i][ax] > c), dn = tt.filter(i => q[i][ax] <= c);
          if (up.length === 0 || dn.length === 0) continue;
          if (up.length === 2) { put(up[0], dn[0]); put(up[0], dn[1]); put(up[1], dn[1]); put(up[0], dn[0]); put(up[1], dn[1]); put(up[1], dn[0]); }
          else { const [one, rest] = up.length === 1 ? [up[0], dn] : [dn[0], up]; for (const k of rest) put(one, k); }
        }
        if (cp.length) {
          const cg = new THREE.BufferGeometry();
          cg.setAttribute("position", new THREE.BufferAttribute(new Float32Array(cp), 3));
          cg.setAttribute("color", new THREE.BufferAttribute(new Float32Array(cc), 3));
          world.add(new THREE.Mesh(cg, new THREE.MeshBasicMaterial({ vertexColors: true, side: THREE.DoubleSide })));
        }
      }
      const surf = new THREE.Mesh(g, new THREE.MeshLambertMaterial({ vertexColors: true, side: THREE.DoubleSide, clippingPlanes: clip }));
      surf.userData.pick = true;
      world.add(surf);
      if (show().mesh) world.add(new THREE.LineSegments(new THREE.WireframeGeometry(g), new THREE.LineBasicMaterial({ color: new THREE.Color(ink), transparent: true, opacity: 0.18, clippingPlanes: clip })));
      const pts: number[] = [];
      for (const [a, b] of r.surface.lines) pts.push(pos[3 * a], pos[3 * a + 1], pos[3 * a + 2], pos[3 * b], pos[3 * b + 1], pos[3 * b + 2]);
      if (pts.length) { const l = fat(pts, null, ink, 1.5), lm = l.material as LineMaterial; lm.clippingPlanes = clip; lm.clipping = clip.length > 0; }
    } else if (vp) {
      // An imported mesh (0.5.5): its outer surface, the lines where its faces meet, pickable by face.
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(vp.surface.nodes.flat()), 3));
      g.setIndex(vp.surface.tris.flat());
      g.computeVertexNormals();
      // Assemblies: each part in its own colour.
      if (asm) {
        const hues = ["#9CC2F7", "#F7D48A", "#A8DDB5", "#F2B3C0", "#C9B8F0", "#9ED9D3"], ids = (m.parts ?? []).map(p => p.id);
        const col = new Float32Array(vp.surface.nodes.length * 3);
        const picked = new Set(sel()), selC = new THREE.Color(css("--sel2"));
        vp.tface.forEach((f, i) => { const c = picked.has(asm.face_part[f]) ? selC : new THREE.Color(hues[Math.max(0, ids.indexOf(asm.face_part[f])) % hues.length]); for (const n of vp.surface.tris[i]) { col[3 * n] = c.r; col[3 * n + 1] = c.g; col[3 * n + 2] = c.b; } });
        g.setAttribute("color", new THREE.BufferAttribute(col, 3));
      }
      const o = new THREE.Mesh(g, new THREE.MeshLambertMaterial({ color: asm ? 0xffffff : new THREE.Color(css("--sky")), vertexColors: !!asm, side: THREE.DoubleSide, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }));
      o.userData.pick = true;
      world.add(o);
      if (show().mesh) world.add(new THREE.LineSegments(new THREE.WireframeGeometry(g), new THREE.LineBasicMaterial({ color: new THREE.Color(ink), transparent: true, opacity: 0.18 })));
      const pts: number[] = [];
      for (const [a, b] of vp.surface.lines) pts.push(...vp.surface.nodes[a], ...vp.surface.nodes[b]);
      if (pts.length) fat(pts, null, ink, 1.2);
    } else if (!m.parts?.length && m.shapes.some(s => !s.hole) && (t > 0 || rev > 0)) {
      // The part as it will be: each solid shape with the holes inside it, extruded or revolved.
      const holes = m.shapes.filter(s => s.hole).map(s => outline(s.geom, tol));
      const sky = () => new THREE.MeshLambertMaterial({ color: new THREE.Color(css("--sky")), side: THREE.DoubleSide });
      const part = (g: THREE.BufferGeometry) => { const o = new THREE.Mesh(g, sky()); o.userData.pick = true; world.add(o); return o; };
      for (const s of m.shapes.filter(x => !x.hole)) {
        const o = outline(s.geom, tol), sh = new THREE.Shape(o.map(p => new THREE.Vector2(p[0], p[1])));
        const inner = holes.filter(h => h.length && inside(o, h[0]));
        for (const h of inner) sh.holes.push(new THREE.Path(h.map(p => new THREE.Vector2(p[0], p[1]))));
        if (!rev) {
          const g = new THREE.ExtrudeGeometry(sh, { depth: t, bevelEnabled: false, curveSegments: 24 });
          part(g);
          world.add(new THREE.LineSegments(new THREE.EdgesGeometry(g, 25), new THREE.LineBasicMaterial({ color: new THREE.Color(ink) })));
          continue;
        }
        // Revolved: each outline (and hole) turned about y, and the section at both cut faces.
        const segs = Math.max(4, Math.ceil(rev / (Math.PI / 48)));
        for (const ring of [o, ...inner]) part(new THREE.LatheGeometry([...ring, ring[0]].map(p => new THREE.Vector2(Math.max(0, p[0]), p[1])), segs, Math.PI / 2 - rev, rev));
        if (!full) for (const a of [0, rev]) { const cap = part(new THREE.ShapeGeometry(sh, 24)); cap.rotation.y = -a; }
      }
      const pts: number[] = [];
      for (const s of m.shapes) for (const f of full ? [0, 0.5] : [0, 1]) loop(pts, [...outline(s.geom, tol), outline(s.geom, tol)[0]], f);
      if (pts.length) fat(pts, null, ink, 1.2);
    }
    // The sweep being set in the Extrude panel (0.6.3): its outlines at the start and the end (and
    // in between, turned), joined along the sweep, over the model.
    const sweepLines = (dp: NonNullable<Model["parts"]>[number], dr: { depth: number; angle: number; from: number }) => {
      const g: number[] = [], rv = dr.angle > 0, len = rv ? dr.angle : dr.depth;
      const at = (q: number[], f: number): P3 => { const s = dr.from + len * f; return placeOn(dp, rv ? [q[0] * Math.cos(s), q[1], q[0] * Math.sin(s)] : [q[0], q[1], s]); };
      const steps = rv ? Math.max(2, Math.ceil(len / (Math.PI / 24))) : 1;
      for (const s of m.shapes.filter(x => x.part === dp.id)) {
        const ring = outline(s.geom, tol);
        for (let k = 0; k <= steps; k++) for (let i = 0; i < ring.length; i++) g.push(...at(ring[i], k / steps), ...at(ring[(i + 1) % ring.length], k / steps));
        const every = Math.max(1, Math.floor(ring.length / 12));
        for (let i = 0; i < ring.length; i += every) for (let k = 0; k < steps; k++) g.push(...at(ring[i], k / steps), ...at(ring[i], (k + 1) / steps));
      }
      return g;
    };
    // Parts that could not be meshed together (too fine, or a sketch to mend): their outlines,
    // swept, so they stay in sight; the bar above says why.
    if (m.parts?.length && !asm) {
      const g = m.parts.flatMap(p => sweepLines(p, { depth: p.depth, angle: p.angle, from: p.from ?? 0 }));
      if (g.length) fat(g, null, ink, 1.2);
    }
    const dr = draft(), dp = dr && m.parts?.find(p => p.id === dr.part);
    if (dr && dp) {
      const g = sweepLines(dp, dr);
      if (g.length) fat(g, null, accent, 2.2);
    }
    // Supported and loaded faces outlined: green for supports, crimson for loads.
    const selSet = new Set(sel());
    const faceLines = (at: Target, colour: string) => {
      const pts: number[] = [];
      if ((!asm && at.on === "edge") || at.on === "line") {
        const s = m.shapes.find(x => x.id === at.shape); if (!s) return;
        const ps = edgePts(s.geom, at.edge, tol) as [number, number][];
        if (at.on === "line") loop(pts, ps, at.at);
        else {
          for (const f of full ? [0] : [0, 1]) loop(pts, ps, f);
          // The face's ends swept along (straight for an extrusion, round for a turn).
          const n = rev ? Math.max(2, Math.ceil(rev / (Math.PI / 48))) : 1;
          for (const p of [ps[0], ps[ps.length - 1]]) for (let i = 0; i < n; i++) pts.push(...sweep(p, i / n), ...sweep(p, (i + 1) / n));
        }
      } else if (asm && (at.on === "edge" || at.on === "partend")) {
        // A face of a part (0.6.0): its triangles tinted.
        const f = asm.targets.findIndex(x => x && JSON.stringify(x) === JSON.stringify(at)), idx: number[] = [];
        vp!.tface.forEach((k, i) => { if (k === f) idx.push(...vp!.surface.tris[i]); });
        if (idx.length) {
          const g = new THREE.BufferGeometry();
          g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(vp!.surface.nodes.flat()), 3));
          g.setIndex(idx);
          world.add(new THREE.Mesh(g, new THREE.MeshBasicMaterial({ color: new THREE.Color(colour), transparent: true, opacity: solidRes() && !stale() ? 0.25 : 0.55, side: THREE.DoubleSide, depthWrite: false })));
        }
      } else if (at.on === "end") {
        for (const s of m.shapes) { const o = outline(s.geom, tol) as [number, number][]; loop(pts, [...o, o[0]], at.front ? 1 : 0); }
      } else if (at.on === "face" && vp) {
        // A face of an imported mesh: its triangles tinted.
        const idx: number[] = [];
        vp.tface.forEach((f, i) => { if (f === at.face) idx.push(...vp.surface.tris[i]); });
        if (idx.length) {
          const g = new THREE.BufferGeometry();
          g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(vp.surface.nodes.flat()), 3));
          g.setIndex(idx);
          world.add(new THREE.Mesh(g, new THREE.MeshBasicMaterial({ color: new THREE.Color(colour), transparent: true, opacity: solidRes() && !stale() ? 0.25 : 0.55, side: THREE.DoubleSide, depthWrite: false })));
        }
      }
      if (pts.length) fat(pts, null, colour, at.on === "line" ? 5 : 3.5);
    };
    for (const s of m.supports) faceLines(s.at, selSet.has(s.id) ? accent : green);
    for (const l of m.loads.filter(x => shownLoad(m, x))) faceLines(l.at, selSet.has(l.id) ? accent : load);
    // Remote forces (0.7.7): an arrow at the point, and a line from it to the face's centre.
    for (const l of m.loads) {
      if (l.kind.kind !== "remote" || l.at.on !== "face" || !shownLoad(m, l)) continue;
      const k = l.kind, c = faceCentre(l.at.face), col = new THREE.Color(selSet.has(l.id) ? accent : load);
      const span = vp ? Math.max(...[0, 1, 2].map(j => Math.max(...vp.surface.nodes.map(q => q[j])) - Math.min(...vp.surface.nodes.map(q => q[j])))) : 0.1;
      const g = new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(...k.at), new THREE.Vector3(...c)]);
      world.add(new THREE.Line(g, new THREE.LineBasicMaterial({ color: col })));
      const fl = Math.hypot(...k.f);
      if (fl > 0) { const d = new THREE.Vector3(...k.f).normalize(), len = span * 0.3; world.add(new THREE.ArrowHelper(d, new THREE.Vector3(...k.at).addScaledVector(d, -len), len, col, len * 0.25, len * 0.12)); }
      const dot = new THREE.Mesh(new THREE.SphereGeometry(span * 0.012, 12, 8), new THREE.MeshBasicMaterial({ color: col })); dot.position.set(...k.at); world.add(dot);
    }
    draw();
  };

  const build = () => {
    themeTick();
    const m = model(); if (!m) return;
    clear();
    // Modelled solids (0.7.3): the solids in step 1, their mesh after it (with supports, loads, results).
    if (isSolidA(m.analysis)) { const sm = showMesh(); buildSolid(geoMesh() && !sm ? { ...m, volume: undefined, supports: [], loads: [] } : m); addCad(!sm); return; }
    if (m.analysis === "space") addCad(false);
    // An empty frame (0.6.0): the floor covers the 6 × 4 × 4 m space the view first looks at.
    const empty = !m.nodes.length && !m.shapes.length;
    const P = at3(m), span = empty ? 6 : span3(m), selSet = new Set(sel());
    const ink = css("--text"), accent = css("--blue"), faint = css("--line3"), load = css("--blue2");
    scene.background = new THREE.Color(css("--bg2"));
    // The stage: the floor grid at the origin, fixed in size, the axes, and the origin planes
    // while nothing is drawn or a plane is to be picked.
    const SL = stageL(m);
    if (opts().grid) floor(SL);
    axes(SL);
    if (picking() || planePick() || (empty && !sketch() && !activeSketch() && !cad()?.sketches.length)) originPlanes(SL);
    const r = spaceRes(), f = r ? deform3(m, r) : 0, q = q3(), rg = range3(), cols = palette(opts().contours).map(c => new THREE.Color(c));
    const showRes = !!r && r.members.length === m.members.length;
    // Members: as drawn (faint when the deflected shape is on top), selected ones in the accent.
    const plain: number[] = [], chosen: number[] = [], bars: number[] = [];
    for (const b of m.members) {
      const a = P.get(b.a), c = P.get(b.b); if (!a || !c) continue;
      if (r?.slack?.includes(b.id) && !stale()) continue;
      (selSet.has(b.id) ? chosen : b.kind === "truss" ? bars : plain).push(...a, ...c);
    }
    const resOn = (showRes && show().deformed) || !!mode3();
    if (plain.length) fat(plain, null, resOn ? faint : ink, resOn ? 1.5 : 3);
    if (bars.length) fat(bars, null, resOn ? faint : ink, resOn ? 1.2 : 2, true);
    if (chosen.length) fat(chosen, null, accent, 4);
    // 0.7.5: each member drawn as its section swept along it, in the solver's local axes (x along
    // it; y, the section's depth, square to it and to global z, or global y for a member along
    // z; the roll turns y and z about x): the frame looks as it will be built.
    if (sectionsOn() && !resOn) {
      const pos: number[] = [], idx: number[] = [];
      const cr = (a: P3, b: P3): P3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
      const un = (a: P3): P3 => { const l = Math.hypot(a[0], a[1], a[2]) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };
      for (const b of m.members) {
        const a = P.get(b.a), c = P.get(b.b), sec = m.sections.find(x => x.id === b.section); if (!a || !c || !sec) continue;
        const ring = sectionRing(sec.shape); if (ring.length < 3) continue;
        const ex = un([c[0] - a[0], c[1] - a[1], c[2] - a[2]]), zx = cr([0, 0, 1], ex);
        const ey0 = Math.hypot(...zx) < 1e-9 ? [0, 1, 0] as P3 : un(zx), ez0 = cr(ex, ey0), r = b.roll ?? 0;
        const ey: P3 = [0, 1, 2].map(k => Math.cos(r) * ey0[k] + Math.sin(r) * ez0[k]) as P3, ez = cr(ex, ey);
        const base = pos.length / 3;
        for (const end of [a, c]) for (const [y, z] of ring) pos.push(end[0] + ey[0] * y + ez[0] * z, end[1] + ey[1] * y + ez[1] * z, end[2] + ey[2] * y + ez[2] * z);
        const n = ring.length;
        for (let i = 0; i < n; i++) { const j = (i + 1) % n; idx.push(base + i, base + j, base + n + j, base + i, base + n + j, base + n + i); }
      }
      if (idx.length) {
        const g = new THREE.BufferGeometry();
        g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(pos), 3)); g.setIndex(idx); g.computeVertexNormals();
        world.add(new THREE.Mesh(g, new THREE.MeshStandardMaterial({ color: new THREE.Color(css("--solid")), roughness: 0.75, side: THREE.DoubleSide, transparent: true, opacity: 0.9 })));
      }
    }
    // The deflected shape, coloured by the chosen result.
    if (showRes && r && !mode3()) {
      const pts: number[] = [], cs: number[] = [];
      const colourOf = (v: number) => { if (!rg || q === "none") return new THREE.Color(ink); const t = (v - rg[0]) / (rg[1] - rg[0]); return cols[Math.max(0, Math.min(BANDS - 1, Math.floor(t * BANDS)))]; };
      for (const mr of r.members) {
        const b = m.members.find(x => x.id === mr.id); if (!b) continue;
        const a = P.get(b.a)!, v = vals(mr, q), ex = mr.axes[0];
        const pos = (i: number): P3 => { const s = mr.x[i], d = show().deformed ? mr.d[i] : [0, 0, 0]; return [a[0] + ex[0] * s + d[0] * f, a[1] + ex[1] * s + d[1] * f, a[2] + ex[2] * s + d[2] * f]; };
        for (let i = 0; i + 1 < mr.x.length; i++) {
          pts.push(...pos(i), ...pos(i + 1));
          const k1 = colourOf(v[i]), k2 = colourOf(v[i + 1]);
          cs.push(k1.r, k1.g, k1.b, k2.r, k2.g, k2.b);
        }
      }
      if (pts.length) fat(pts, cs, ink, 4);
    }
    // Plates (0.4.1): solved, the mesh moved and coloured by the chosen result; else each plate's
    // face (openings cut out), tinted.
    const pr = showRes && r && !mode3() ? r.plates : undefined;
    if (pr) {
      const fl = pr.fields.find(x => x.key === pq()) ?? pr.fields[0], prg = plateRange() ?? [fl.min, fl.max];
      const pos = new Float32Array(pr.nodes.length * 3), col = new Float32Array(pr.nodes.length * 3);
      pr.nodes.forEach((p, i) => {
        const d = show().deformed ? pr.disp[i] : [0, 0, 0];
        for (let k = 0; k < 3; k++) pos[3 * i + k] = p[k] + d[k] * f;
        const c = cols[Math.max(0, Math.min(BANDS - 1, Math.floor((fl.v[i] - prg[0]) / ((prg[1] - prg[0]) || 1) * BANDS)))];
        col[3 * i] = c.r; col[3 * i + 1] = c.g; col[3 * i + 2] = c.b;
      });
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
      g.setAttribute("color", new THREE.BufferAttribute(col, 3));
      g.setIndex(pr.tris.flat());
      g.computeVertexNormals();
      world.add(new THREE.Mesh(g, new THREE.MeshLambertMaterial({ vertexColors: true, side: THREE.DoubleSide })));
      const edges = new THREE.EdgesGeometry(g, 1);
      world.add(new THREE.LineSegments(edges, new THREE.LineBasicMaterial({ color: new THREE.Color(faint) })));
    } else if (!mode3()) {
      for (const s of m.shapes) {
        const pl = s.plate; if (!pl || s.hole) continue;
        const shp = new THREE.Shape(outline(s.geom, span / 200).map(p => new THREE.Vector2(p[0], p[1])));
        for (const h of m.shapes) if (h.hole && h.plate && h.plate.plane === pl.plane && Math.abs(h.plate.depth - pl.depth) < 1e-7) shp.holes.push(new THREE.Path(outline(h.geom, span / 200).map(p => new THREE.Vector2(p[0], p[1]))));
        const g = new THREE.ShapeGeometry(shp), a = g.getAttribute("position");
        for (let i = 0; i < a.count; i++) { const q = plate3(pl, a.getX(i), a.getY(i)); a.setXYZ(i, q[0], q[1], q[2]); }
        g.computeVertexNormals();
        const on = selSet.has(s.id);
        world.add(new THREE.Mesh(g, new THREE.MeshLambertMaterial({ color: new THREE.Color(on ? accent : css("--accent2")), transparent: true, opacity: on ? 0.55 : 0.3, side: THREE.DoubleSide, depthWrite: false })));
        const rim = outline(s.geom, span / 200).flatMap((p, i, o) => [...plate3(pl, p[0], p[1]), ...plate3(pl, o[(i + 1) % o.length][0], o[(i + 1) % o.length][1])]);
        world.add(new THREE.LineSegments(new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array(rim), 3)), new THREE.LineBasicMaterial({ color: new THREE.Color(on ? accent : ink) })));
      }
    }
    // Nodes.
    const np = new Float32Array(m.nodes.flatMap(n => [n.x, n.y, n.z ?? 0]));
    const ng = new THREE.BufferGeometry(); ng.setAttribute("position", new THREE.BufferAttribute(np, 3));
    world.add(new THREE.Points(ng, new THREE.PointsMaterial({ color: new THREE.Color(ink), size: 6, sizeAttenuation: false })));
    const sn = m.nodes.filter(n => selSet.has(n.id));
    if (sn.length) {
      const sg = new THREE.BufferGeometry(); sg.setAttribute("position", new THREE.BufferAttribute(new Float32Array(sn.flatMap(n => [n.x, n.y, n.z ?? 0])), 3));
      world.add(new THREE.Points(sg, new THREE.PointsMaterial({ color: new THREE.Color(accent), size: 10, sizeAttenuation: false })));
    }
    // Supports: a block for fixed, a cone for pinned, a ball for rollers, a coil-like post for springs.
    const s0 = span * 0.03;
    for (const s of m.supports) {
      if (s.at.on !== "node") continue;
      const p = P.get(s.at.id); if (!p) continue;
      const k = s.kind.kind, on = selSet.has(s.id);
      const mat = new THREE.MeshLambertMaterial({ color: new THREE.Color(on ? accent : css("--muted")) });
      const g = k === "fixed" ? new THREE.BoxGeometry(s0 * 1.6, s0 * 0.5, s0 * 1.6) : k === "pinned" ? new THREE.ConeGeometry(s0 * 0.8, s0 * 1.2, 4) : k === "spring" ? new THREE.CylinderGeometry(s0 * 0.25, s0 * 0.25, s0 * 1.4, 8) : new THREE.SphereGeometry(s0 * 0.5, 12, 8);
      const mesh = new THREE.Mesh(g, mat);
      mesh.position.set(p[0], p[1] - (k === "fixed" ? s0 * 0.25 : k === "pinned" ? s0 * 0.6 : s0 * 0.5), p[2]);
      world.add(mesh);
    }
    // Loads: arrows ending at the node; moments as double arrows along their axis; member loads
    // as rows of small arrows.
    const L = span * 0.14;
    const arrow = (tip: P3, dir: P3, len: number, colour: string, double = false) => {
      const d = new THREE.Vector3(...dir); if (d.length() === 0) return; d.normalize();
      const tail = new THREE.Vector3(...tip).addScaledVector(d, -len);
      world.add(new THREE.ArrowHelper(d, tail, len, new THREE.Color(colour), len * 0.25, len * 0.12));
      if (double) world.add(new THREE.ArrowHelper(d, tail, len * 0.78, new THREE.Color(colour), len * 0.25, len * 0.12));
    };
    for (const l of m.loads.filter(x => shownLoad(m, x))) {
      const k = l.kind, on = selSet.has(l.id), colour = on ? accent : load;
      if (k.kind === "force" && l.at.on === "node") { const p = P.get(l.at.id); if (p) arrow(p, [k.fx, k.fy, k.fz ?? 0], L, colour); }
      if (k.kind === "moment" && l.at.on === "node") { const p = P.get(l.at.id); if (p) arrow(p, [k.mx ?? 0, k.my ?? 0, k.m], L * 0.8, colour, true); }
      if (k.kind === "settlement" && l.at.on === "node") { const p = P.get(l.at.id); if (p) arrow([p[0] + k.dx * 0, p[1] - span * 0.04, p[2]], [k.dx, k.dy, k.dz ?? 0], L * 0.6, colour); }
      if (k.kind === "mass" && l.at.on === "node") { const p = P.get(l.at.id); if (p) { const b = new THREE.Mesh(new THREE.SphereGeometry(span * 0.018, 16, 12), new THREE.MeshLambertMaterial({ color: new THREE.Color(on ? accent : ink) })); b.position.set(...p); world.add(b); } }
      if (k.kind === "dist" && l.at.on === "member") {
        const b = m.members.find(x => x.id === (l.at as any).id), a = b && P.get(b.a), c = b && P.get(b.b); if (!a || !c) continue;
        const res = r?.members.find(x => x.id === b!.id);
        const dir: P3 = k.dir === "x" ? [1, 0, 0] : k.dir === "y" ? [0, 1, 0] : k.dir === "z" ? [0, 0, 1] : (res?.axes[1] ?? [0, 1, 0]);
        const wmax = Math.max(Math.abs(k.w1), Math.abs(k.w2)) || 1;
        for (let i = 0; i <= 6; i++) {
          const t = i / 6, w = k.w1 + (k.w2 - k.w1) * t; if (Math.abs(w) < 1e-12) continue;
          arrow([a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t, a[2] + (c[2] - a[2]) * t], dir.map(x => x * Math.sign(w)) as P3, L * 0.5 * Math.abs(w) / wmax, colour);
        }
      }
    }
    draw();
  };

  /** Frame the model, looking along `from` (towards the model), or as now, or from the corner. */
  /** The box round everything in the model (empty if nothing is drawn). */
  const boxOf = (m: Model) => {
    const box = new THREE.Box3(); m.nodes.forEach(n => box.expandByPoint(new THREE.Vector3(n.x, n.y, n.z ?? 0)));
    // Plates (0.4.1): their outlines in 3D.
    for (const s of m.shapes) if (s.plate) for (const p of outline(s.geom, Infinity)) box.expandByPoint(new THREE.Vector3(...plate3(s.plate, p[0], p[1])));
    // A solid: its outline at both ends of the extrusion.
    if (isSolidA(m.analysis)) for (const s of m.shapes) for (const p of outline(s.geom, m.revolve ? 1e-3 : Infinity)) {
      if (m.revolve) for (let i = 0; i <= 16; i++) { const a = m.revolve * i / 16; box.expandByPoint(new THREE.Vector3(p[0] * Math.cos(a), p[1], p[0] * Math.sin(a))); }
      else for (const z of [0, m.thickness]) box.expandByPoint(new THREE.Vector3(p[0], p[1], z));
    }
    // An imported mesh (0.5.5): its outer surface.
    for (const p of (m.volume ?? m.assembly)?.surface.nodes ?? []) box.expandByPoint(new THREE.Vector3(p[0], p[1], p[2]));
    // The geometry engine's solids (0.7.2): mm with Z up.
    if (isSolidA(m.analysis)) for (const b of cad()?.bodies ?? []) for (let i = 0; i < b.pos.length; i += 3) box.expandByPoint(new THREE.Vector3(b.pos[i] * 1e-3, b.pos[i + 2] * 1e-3, -b.pos[i + 1] * 1e-3));
    // ...and their sketches, so a sketch just finished is in view before its solid is made.
    if (isSolidA(m.analysis) || m.analysis === "space") for (const k of cad()?.sketches ?? []) for (const cv of k.curves) for (const [x, y] of cv.pts) {
      const q = [0, 1, 2].map(j => k.origin[j] + x * k.x[j] + y * k.y[j]);
      box.expandByPoint(new THREE.Vector3(q[0] * 1e-3, q[2] * 1e-3, -q[1] * 1e-3));
    }
    // ...and a frame's levels over its grid (0.7.6).
    if (m.analysis === "space" && !box.isEmpty()) for (const p of cad()?.planes ?? []) if (p.plane && Math.abs(p.plane.x_dir[2]) < 1e-9 && Math.abs(p.plane.y_dir[2]) < 1e-9) box.expandByPoint(new THREE.Vector3(box.min.x, p.plane.origin[2] * 1e-3, box.min.z));
    return box;
  };
  const fit = (keepDir = false, from?: P3) => {
    const m = model(); if (!m) return;
    const box = boxOf(m);
    // Nothing drawn yet (0.5.9): look at a 6 × 4 × 4 m space from the corner, to draw in.
    if (box.isEmpty()) { const L = stageL(m) * 0.16; box.expandByPoint(new THREE.Vector3(-L * 0.4, -L * 0.2, L * 0.4)); box.expandByPoint(new THREE.Vector3(L, L, -L)); }
    const c = box.getCenter(new THREE.Vector3()), rad = Math.max(box.getSize(new THREE.Vector3()).length() / 2, 1e-3);
    const dir = from ? new THREE.Vector3(...from).normalize() : keepDir && controls ? camera.position.clone().sub(controls.target).normalize() : new THREE.Vector3(1, 0.75, 1.3).normalize();
    camera.position.copy(c).addScaledVector(dir, rad / Math.sin(camera.fov * Math.PI / 360) * 1.15);
    camera.near = rad / 100; camera.far = rad * 200; camera.updateProjectionMatrix();
    controls?.target.copy(c); controls?.update();
    draw();
  };

  // Sketching (0.6.2). New Sketch, or a drawing tool of a 3D frame, asks for a plane: the squares
  // at the origin. While sketching, the view looks square at the plane through an orthographic
  // camera lined up with the sketch canvas lying over it (same scale and pan), so the sketch sits
  // on the model; turned away, the usual camera comes back.
  type WP = "xy" | "zy" | "xz";
  const picking = () => sketchPick() || (!sketch() && model()?.analysis === "space" && ["beam", "truss", "node"].includes(tool()));
  const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  // 0.7.2: the engine's sketch is seen through the same flat camera once the turn to it is done.
  const [cadFacing, setCadFacing] = createSignal(false);
  const cam = (): THREE.Camera => (sketch()?.facing || cadFacing() ? ortho : camera);
  /** The engine's sketch plane in the view's terms: origin, in-plane axes and normal (metres, y up). */
  const cadFrame = () => {
    const sk = activeSketch(); if (!sk) return null;
    const v = (p: number[], k = 1) => new THREE.Vector3(p[0] * k, p[2] * k, -p[1] * k);
    const O = v(sk.origin, 1e-3), U = v(sk.x).normalize(), V = v(sk.y).normalize();
    return { O, U, V, N: U.clone().cross(V).normalize() };
  };
  /** Moves the usual camera smoothly to look from `pos` at `target` with `up`, then calls `done`. */
  let tween = 0;
  const glide = (pos: THREE.Vector3, target: THREE.Vector3, up: THREE.Vector3, done?: () => void, ms = 450) => {
    const p0 = camera.position.clone(), t0 = (controls?.target ?? new THREE.Vector3()).clone(), u0 = camera.up.clone(), start = performance.now(), id = ++tween;
    const step = () => {
      if (id !== tween) return;
      const k = Math.min(1, (performance.now() - start) / ms), e = k < 0.5 ? 2 * k * k : 1 - (-2 * k + 2) ** 2 / 2;
      camera.position.lerpVectors(p0, pos, e); camera.up.lerpVectors(u0, up, e).normalize();
      const t = t0.clone().lerp(target, e); camera.lookAt(t); controls?.target.copy(t);
      camera.updateMatrixWorld(); draw();
      if (k < 1) requestAnimationFrame(step); else { controls?.update(); done?.(); }
    };
    requestAnimationFrame(step);
  };
  // A sketch opens: the camera turns square to its plane, then the flat camera takes over.
  createEffect(on(() => activeSketch()?.id, (id, was) => {
    if (id == null || id === was) { if (id == null) setCadFacing(false); return; }
    const f = cadFrame(); if (!f) return;
    setCadFacing(false);
    const d = Math.max(camera.position.distanceTo(controls?.target ?? f.O), 0.05);
    glide(f.O.clone().addScaledVector(f.N, d), f.O.clone(), f.V.clone(), () => setCadFacing(true));
  }));
  // The flat camera follows the sketch sheet's scale and pan.
  createEffect(() => {
    if (!cadFacing()) return;
    const f = cadFrame(), v = sheetView(), m = model(); if (!f || !m) return;
    const [w, h] = size(), s = v.s * 1e3, L = stageL(m) * 20;
    const c = f.O.clone().addScaledVector(f.U, v.cx * 1e-3).addScaledVector(f.V, v.cy * 1e-3);
    ortho.left = -w / 2 / s; ortho.right = w / 2 / s; ortho.top = h / 2 / s; ortho.bottom = -h / 2 / s; ortho.near = 0; ortho.far = 2 * L;
    ortho.position.copy(c).addScaledVector(f.N, L); ortho.up.copy(f.V); ortho.lookAt(c); ortho.updateProjectionMatrix(); ortho.updateMatrixWorld();
    draw();
  });
  // Finish or Cancel: back to the usual view from the corner, framing the model, smoothly.
  createEffect(on(turnBack, n => {
    if (!n) return;
    setCadFacing(false);
    const m = model(); if (!m) return;
    const box = boxOf(m), c = box.isEmpty() ? new THREE.Vector3() : box.getCenter(new THREE.Vector3());
    const rad = box.isEmpty() ? stageL(m) * 0.1 : Math.max(box.getSize(new THREE.Vector3()).length() / 2, 1e-3);
    const dir = new THREE.Vector3(1, 0.75, 1.3).normalize();
    camera.near = Math.min(camera.near, rad / 100); camera.far = Math.max(camera.far, rad * 200); camera.updateProjectionMatrix();
    glide(c.clone().addScaledVector(dir, rad / Math.sin(camera.fov * Math.PI / 360) * 1.15), c, new THREE.Vector3(0, 1, 0), () => { camera.near = rad / 100; camera.far = rad * 200; camera.updateProjectionMatrix(); draw(); }, 600);
  }, { defer: true }));
  // Turning away from the sketch: the usual camera starts from the sketch's side, a little turned,
  // framing the whole model, so the sketch stays in sight.
  let wasFacing = false;
  createEffect(() => {
    const on = !!sketch()?.facing, f = sketchFrame();
    if (wasFacing && !on && sketch() && f) {
      const N = new THREE.Vector3(...f.N).normalize(), U = new THREE.Vector3(...f.U).normalize(), V = new THREE.Vector3(...f.V).normalize();
      const d = N.multiplyScalar(1).addScaledVector(U, 0.45).addScaledVector(V, 0.35);
      queueMicrotask(() => fit(false, [d.x, d.y, d.z]));
    }
    wasFacing = on;
  });
  createEffect(() => {
    const sk = sketch(), s = scale(), pn = pan(), m = model();
    if (!sk?.facing || !m) { draw(); return; }
    const f = sketchFrame(); if (!f) return;
    const [w, h] = size(), U = new THREE.Vector3(...f.U).normalize(), V = new THREE.Vector3(...f.V).normalize(), N = new THREE.Vector3(...f.N).normalize();
    const L = stageL(m) * 20, c = new THREE.Vector3(...f.O).addScaledVector(U, (w / 2 - pn[0]) / s).addScaledVector(V, (pn[1] - h / 2) / s);
    ortho.left = -w / 2 / s; ortho.right = w / 2 / s; ortho.top = h / 2 / s; ortho.bottom = -h / 2 / s; ortho.near = 0; ortho.far = 2 * L;
    ortho.position.copy(c).addScaledVector(N, L); ortho.up.copy(V); ortho.lookAt(c); ortho.updateProjectionMatrix(); ortho.updateMatrixWorld();
    draw();
  });
  /** Sketch on plane k: for a part, a new part placed on it; for a 3D frame, its working plane. */
  const sketchOn = (k: WP) => import("./sketchmode").then(x => x.beginSketch(k));

  /** Screen position of a 3D point. */
  const screen = (p: P3): [number, number] => {
    const v = new THREE.Vector3(...p).project(cam()), [w, h] = size();
    return [(v.x + 1) / 2 * w, (1 - v.y) / 2 * h];
  };
  const segDist = (q: [number, number], a: [number, number], b: [number, number]) => {
    const dx = b[0] - a[0], dy = b[1] - a[1], l2 = dx * dx + dy * dy, t = l2 ? Math.max(0, Math.min(1, ((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l2)) : 0;
    return [Math.hypot(a[0] + dx * t - q[0], a[1] + dy * t - q[1]), t];
  };
  /** The node or member under the pointer (nodes first), within a few pixels. */
  const pick = (x: number, y: number): { kind: "node" | "member"; id: number } | null => {
    const m = model(); if (!m) return null;
    let best: { kind: "node" | "member"; id: number; d: number } | null = null;
    for (const n of m.nodes) { const s = screen([n.x, n.y, n.z ?? 0]), d = Math.hypot(s[0] - x, s[1] - y); if (d < 8 && (!best || d < best.d)) best = { kind: "node", id: n.id, d }; }
    if (best) return best;
    const P = at3(m);
    for (const b of m.members) { const a = P.get(b.a), c = P.get(b.b); if (!a || !c) continue; const [d] = segDist([x, y], screen(a), screen(c)); if (d < 6 && (!best || d < best.d)) best = { kind: "member", id: b.id, d }; }
    return best;
  };
  /** The result station nearest the pointer, along the drawn (deflected) members. */
  const probeAt = (x: number, y: number) => {
    const m = model(), r = spaceRes(); if (!m || !r || stale()) return null;
    const P = at3(m), f = deform3(m, r);
    let best: { mr: SpaceMember; i: number; d: number } | null = null;
    for (const mr of r.members) {
      const b = m.members.find(z => z.id === mr.id), a = b && P.get(b.a); if (!a) continue;
      const ex = mr.axes[0];
      for (let i = 0; i < mr.x.length; i++) {
        const d = show().deformed ? mr.d[i] : [0, 0, 0], s = screen([a[0] + ex[0] * mr.x[i] + d[0] * f, a[1] + ex[1] * mr.x[i] + d[1] * f, a[2] + ex[2] * mr.x[i] + d[2] * f]);
        const dd = Math.hypot(s[0] - x, s[1] - y);
        if (dd < 10 && (!best || dd < best.d)) best = { mr, i, d: dd };
      }
    }
    return best ? { mr: best.mr, i: best.i, x, y } : null;
  };

  let down: [number, number] | null = null;
  const onDown = (e: PointerEvent) => { down = [e.clientX, e.clientY]; };
  const onUp = async (e: PointerEvent) => {
    if (!down || e.button !== 0) { down = null; return; }
    const moved = Math.hypot(e.clientX - down[0], e.clientY - down[1]) > 4; down = null;
    if (moved) return;
    const rc = host.getBoundingClientRect(); let t = tool();
    // Picking the plane to sketch on (0.6.2).
    if (picking()) {
      const o = pickOrigin(e.clientX - rc.left, e.clientY - rc.top);
      if (o) { sketchOn(o); return; }
      // A flat face of a part (0.6.3).
      const ph = model()?.analysis !== "space" ? hitPart(e.clientX - rc.left, e.clientY - rc.top) : null;
      if (ph) { import("./sketchmode").then(k => k.beginSketchOnFace(ph)); return; }
      toast(model()?.parts?.length ? "Pick a plane to sketch on: a square at the origin, or a flat face of a part." : "Pick a plane to sketch on: click one of the three squares at the origin.");
      return;
    }
    // 3D frames drawn with the sketch tools (0.7.5) pick their plane the same way.
    if ((model()?.analysis === "space" && planePick() && !activeSketch()) || (isSolidA(model()?.analysis) && !activeSketch() && !cad3Tool() && !(model()?.parts?.length) && (planePick() || !cad()?.bodies.length))) {
      const x = e.clientX - rc.left, y = e.clientY - rc.top, o = pickOrigin(x, y);
      // The engine's planes: XY the floor (the view's x–z), XZ the front (x–y), YZ the side (z–y).
      if (o) { setPlanePick(false); newSketch(({ xz: "XY", xy: "XZ", zy: "YZ" } as const)[o]); return; }
      const h = cadHit(x, y);
      if (h.face && planePick()) { setPlanePick(false); await cad3Click(h, "sketchface"); return; }
      if (planePick()) { toast("Click one of the three planes at the origin, or a flat face of a solid."); return; }
    }
    if (isSolidA(model()?.analysis) && cad3Tool()) { await cad3Click(cadHit(e.clientX - rc.left, e.clientY - rc.top)); return; }
    if (isSolidA(model()?.analysis)) {
      // The 3D modelling tools (0.6.3) act on the part clicked.
      if (["hole3", "fillet3", "chamfer3"].includes(t)) {
        const ph = hitPart(e.clientX - rc.left, e.clientY - rc.top);
        if (!ph) { toast(t === "hole3" ? "Click the front or back face of a part where the hole goes." : "Click a part near the edge to round or bevel (an edge along its depth)."); return; }
        await toolOnPart(t, ph); return;
      }
      if (t === "select") {
        // Parts are picked in 3D: click one, Shift or Ctrl to add or take away.
        const ph = hitPart(e.clientX - rc.left, e.clientY - rc.top);
        if (!ph) { if (!e.shiftKey && !e.ctrlKey) setSel([]); return; }
        setSel(e.shiftKey || e.ctrlKey ? (sel().includes(ph.part) ? sel().filter(i => i !== ph.part) : [...sel(), ph.part]) : [ph.part]);
        return;
      }
      // On a solid, Force on a face is shared over the face (0.7.3).
      if (t === "load:force") t = "load:traction";
      // Finer near a face (0.7.7): a click adds the face (a quarter of the element size there), a
      // second click takes it off; the solids are meshed again.
      if (t === "refine" && model()?.volume) {
        const f = pickFace(e.clientX - rc.left, e.clientY - rc.top), m = model()!;
        if (!f || f.on !== "face") return toast("Click a face of the solid to mesh finer near it.");
        const had = m.mesh.refine.some(r => r.at.on === "face" && r.at.face === f.face);
        const base = m.mesh.size > 0 ? m.mesh.size : (await invoke<number>("cad_auto_size")) * 1e-3;
        await C.meshSettings({ refine: had ? m.mesh.refine.filter(r => !(r.at.on === "face" && r.at.face === f.face)) : [...m.mesh.refine, { at: f, size: base / 4 }] });
        if (geoMesh()) { setMeshOld(true); await meshSolids(); }
        return;
      }
      const at = pickFace(e.clientX - rc.left, e.clientY - rc.top);
      const ok = model()?.analysis === "solidheat" ? ["sup:temperature", "load:flux", "load:convection"].includes(t) : (t.startsWith("sup:") && !["sup:temperature", "sup:rollerx", "sup:rollery", "sup:spring"].includes(t)) || ["load:pressure", "load:traction", "load:settlement", "load:remote", "load:velocity"].includes(t);
      if (t === "load:remote" && !model()?.volume) { toast("A remote force goes on a face of a meshed solid."); return; }
      if (!at || !ok) { toast(model()?.volume ? (ok ? "Click on a face of the mesh." : `On a ${model()?.volume?.source === "geometry" ? "solid" : "meshed solid"}, supports (fixed, pinned, roller, symmetry, held directions) and forces or pressures go on its faces.`) : ok ? "Click on the part: a side face, or the back or front face." : "Put it on a face: an edge in Section (its side face), or the back or front face."); return; }
      const [kind, val] = t.startsWith("sup:") ? ["Support", defaultSupport(at.on === "end" && t === "sup:pinned" ? "fixed" : t.slice(4))] : ["Load", defaultLoad(t.slice(5), false)];
      // A remote force (0.7.7) starts at the face's centre, 1 kN down: move its point in the Inspector.
      if (t === "load:remote" && at.on === "face") (val as { at: number[] }).at = faceCentre(at.face);
      const o = await act({ Add: { item: { [kind]: { id: 0, at, kind: val } } } });
      if (o?.created.length) setSel(o.created);
      return;
    }
    const h = pick(e.clientX - rc.left, e.clientY - rc.top);
    if (t === "select") {
      setEdgeSel(null);
      if (!h) { if (!e.shiftKey && !e.ctrlKey) setSel([]); return; }
      setSel(e.shiftKey || e.ctrlKey ? (sel().includes(h.id) ? sel().filter(i => i !== h.id) : [...sel(), h.id]) : [h.id]);
      return;
    }
    if (t.startsWith("sup:") || t.startsWith("load:")) {
      if (!h || (t.startsWith("sup:") && h.kind !== "node")) return toast(t.startsWith("sup:") ? "Click a node to put the support on." : "Click a node or a member for the load.");
      const at: Target = { on: h.kind, id: h.id };
      const [kind, val] = t.startsWith("sup:") ? ["Support", defaultSupport(t.slice(4))] : ["Load", defaultLoad(t.slice(5), true)];
      const o = await act({ Add: { item: { [kind]: { id: 0, at, kind: val } } } });
      if (o?.created.length) setSel(o.created);
      return;
    }
    setPlane("xy");
  };
  /** 0.7.1: what of the geometry engine's solids is under the pointer: the face hit (its body,
   * number and the point, in the engine's mm with Z up) and the nearest edge within 8 px. */
  const cadHit = (x: number, y: number): CadHit => {
    const [w, h] = size(), ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(x / w * 2 - 1, 1 - y / h * 2), camera);
    const mm = (v: THREE.Vector3): [number, number, number] => [v.x * 1e3, -v.z * 1e3, v.y * 1e3];
    const hit = ray.intersectObjects(world.children.filter(o => o.userData.cadBody), false)[0];
    const face = hit && hit.faceIndex != null ? { body: hit.object.userData.cadBody as string, index: (hit.object.userData.cadFace as number[])[hit.faceIndex] ?? -1, at: mm(hit.point) } : null;
    let edge: CadHit["edge"] = null, best = 8;
    const scr = (v: THREE.Vector3) => { const q = v.clone().project(camera); return [(q.x + 1) / 2 * w, (1 - q.y) / 2 * h, q.z]; };
    for (const o of world.children.filter(o => o.userData.cadEdges)) {
      const a = ((o as THREE.LineSegments).geometry.getAttribute("position") as THREE.BufferAttribute).array as Float32Array;
      for (let i = 0; i + 5 < a.length; i += 6) {
        const A = new THREE.Vector3(a[i], a[i + 1], a[i + 2]), B = new THREE.Vector3(a[i + 3], a[i + 4], a[i + 5]), sa = scr(A), sb = scr(B);
        const dx = sb[0] - sa[0], dy = sb[1] - sa[1], L = dx * dx + dy * dy || 1, t = Math.max(0, Math.min(1, ((x - sa[0]) * dx + (y - sa[1]) * dy) / L));
        const dd = Math.hypot(sa[0] + t * dx - x, sa[1] + t * dy - y);
        // An edge behind the face hit is hidden: it must be about as near as that face.
        const P = A.clone().lerp(B, t);
        if (dd < best && (!hit || P.distanceTo(camera.position) <= hit.distance * 1.02 + 1e-4)) { best = dd; edge = { body: o.userData.cadEdges, at: mm(P) }; }
      }
    }
    return { face, edge };
  };
  /** The part under the pointer in an assembly (0.6.3): the part, its face, the point and the
   * face's normal towards the viewer. */
  const hitPart = (x: number, y: number): PartHit | null => {
    const m = model(); if (!m?.assembly) return null;
    const [w, h] = size(), ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(x / w * 2 - 1, 1 - y / h * 2), camera);
    const hit = ray.intersectObjects(world.children.filter(o => o.userData.pick), false)[0]; if (!hit || hit.faceIndex == null) return null;
    const f = m.assembly.tface[hit.faceIndex]; if (f === undefined) return null;
    const n = hit.face ? hit.face.normal.clone() : new THREE.Vector3(0, 0, 1);
    if (n.dot(ray.ray.direction) > 0) n.negate();
    return { part: m.assembly.face_part[f], target: m.assembly.targets[f] ?? null, point: [hit.point.x, hit.point.y, hit.point.z], normal: [n.x, n.y, n.z] };
  };
  /** The face of a solid under the pointer (0.3.2): the back or front face, or the side face of the
   * section edge nearest the point hit. */
  /** The centre (area-weighted) of a meshed solid's face, from its surface triangles. */
  const faceCentre = (f: number): [number, number, number] => {
    const v = model()?.volume, c = [0, 0, 0]; let a = 0;
    if (!v) return [0, 0, 0];
    v.surface.tris.forEach((t, i) => {
      if (v.tface[i] !== f) return;
      const [p, q, r] = t.map(k => v.surface.nodes[k]);
      const u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]], w = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
      const ar = Math.hypot(u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]) / 2;
      for (let k = 0; k < 3; k++) c[k] += ar * (p[k] + q[k] + r[k]) / 3;
      a += ar;
    });
    return a > 0 ? [c[0] / a, c[1] / a, c[2] / a] : [0, 0, 0];
  };
  const pickFace = (x: number, y: number): Target | null => {
    const m = model(); if (!m) return null;
    const [w, h] = size(), ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(x / w * 2 - 1, 1 - y / h * 2), camera);
    const hit = ray.intersectObjects(world.children.filter(o => o.userData.pick), false)[0]; if (!hit) return null;
    // An imported mesh: the face of the triangle hit (results draw the same triangles in the same order).
    if (m.volume) { const f = hit.faceIndex != null ? m.volume.tface[hit.faceIndex] : undefined; return f === undefined ? null : { on: "face", face: f }; }
    // An assembly (0.6.0): the part face the triangle hit is on.
    if (m.assembly) { const f = hit.faceIndex != null ? m.assembly.tface[hit.faceIndex] : undefined, t = f === undefined ? null : m.assembly.targets[f]; return t ? { ...t } : null; }
    const p = hit.point, rev = m.revolve ?? 0, full = rev >= Math.PI * 2 - 1e-9;
    const u = rev ? Math.hypot(p.x, p.z) : p.x, v = p.y;
    const tol = Math.max(1e-6, (m.grid || 0.005) / 4);
    const span = Math.max(1e-6, ...m.shapes.flatMap(s => outline(s.geom, Infinity).map(q => Math.hypot(q[0], q[1]))));
    // How far the hit is from each end face.
    const th = rev ? ((Math.atan2(p.z, p.x) % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2) : 0;
    const ends = full ? [Infinity, Infinity] : rev ? [u * Math.sin(Math.min(th, Math.PI / 2)), u * Math.abs(Math.sin(Math.min(Math.abs(rev - th), Math.PI / 2)))] : [Math.abs(p.z), Math.abs(p.z - m.thickness)];
    let best: Target | null = null, bd = Infinity;
    for (const s of m.shapes) for (let e = 0; e < (s.geom.kind === "circle" ? 1 : s.geom.pts.length); e++) {
      const ps = edgePts(s.geom, e, tol);
      for (let i = 0; i + 1 < ps.length; i++) { const [d] = segDist([u, v], ps[i] as [number, number], ps[i + 1] as [number, number]); if (d < bd) { bd = d; best = { on: "edge", shape: s.id, edge: e }; } }
    }
    const k = ends[0] < ends[1] ? 0 : 1;
    if (ends[k] < Math.min(bd, span * 0.02)) return { on: "end", front: k === 1 };
    return best;
  };
  // The probe on a solid: the surface node nearest the pointer.
  const [probe3, setProbe3] = createSignal<{ i: number } | null>(null);
  const probeSolid = (x: number, y: number) => {
    const r = solidRes(); if (!r || stale()) return null;
    let best = -1, bd = 12;
    const disp = solidDisp(r);
    r.surface.nodes.forEach((p, i) => { const d = disp[i], s = screen([p[0] + d[0] * solidF, p[1] + d[1] * solidF, p[2] + d[2] * solidF]), dd = Math.hypot(s[0] - x, s[1] - y); if (dd < bd) { bd = dd; best = i; } });
    return best >= 0 ? { i: best } : null;
  };
  const onMove = (e: PointerEvent) => {
    if (e.buttons) { setProbe(null); setProbe3(null); return; }
    const rc = host.getBoundingClientRect();
    if (isSolidA(model()?.analysis)) setProbe3(probeSolid(e.clientX - rc.left, e.clientY - rc.top));
    else setProbe(probeAt(e.clientX - rc.left, e.clientY - rc.top));
  };

  onMount(() => {
    host.appendChild(renderer.domElement);
    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = false;
    controls.addEventListener("change", draw);
    const ro = new ResizeObserver(() => {
      const [w, h] = size();
      renderer.setSize(w, h, false); renderer.domElement.style.width = `${w}px`; renderer.domElement.style.height = `${h}px`;
      camera.aspect = w / h; camera.updateProjectionMatrix();
      lineMats.forEach(mt => mt.resolution.set(w, h));
      draw();
    });
    ro.observe(host);
    fit3d.run = () => fit(true);
    // For the UI checks: where a 3D point is on the screen.
    (window as any).__screen3 = screen;
    fit();
    onCleanup(() => { ro.disconnect(); controls?.dispose(); clear(); renderer.dispose(); renderer.domElement.remove(); if (frame) cancelAnimationFrame(frame); fit3d.run = null; });
  });
  createEffect(build);
  createEffect(() => {
    const md = mode3(), ms = modeSolid(), m = model(), k = modeAmp() * phase();
    themeTick();
    modeGroup.traverse(o => { const x = o as any; x.geometry?.dispose?.(); x.material?.dispose?.(); });
    modeGroup.clear();
    if (ms) {
      // The solid's surface moved by the mode, coloured by how far each point moves.
      const cs = palette(opts().contours, false).map(c => new THREE.Color(c));
      const pos = new Float32Array(ms.s.nodes.length * 3), col = new Float32Array(ms.s.nodes.length * 3);
      ms.s.nodes.forEach((p, i) => {
        const d = ms.d[i], a = Math.hypot(d[0], d[1], d[2]);
        for (let c = 0; c < 3; c++) pos[3 * i + c] = p[c] + d[c] * k;
        const cc = cs[Math.max(0, Math.min(BANDS - 1, Math.floor(a * BANDS)))];
        col[3 * i] = cc.r; col[3 * i + 1] = cc.g; col[3 * i + 2] = cc.b;
      });
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
      g.setAttribute("color", new THREE.BufferAttribute(col, 3));
      g.setIndex(ms.s.tris.flat());
      g.computeVertexNormals();
      modeGroup.add(new THREE.Mesh(g, new THREE.MeshLambertMaterial({ vertexColors: true, side: THREE.DoubleSide })));
      const pts: number[] = [];
      for (const [a, b] of ms.s.lines) pts.push(pos[3 * a], pos[3 * a + 1], pos[3 * a + 2], pos[3 * b], pos[3 * b + 1], pos[3 * b + 2]);
      if (pts.length) modeGroup.add(new THREE.LineSegments(new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array(pts), 3)), new THREE.LineBasicMaterial({ color: new THREE.Color(css("--text")) })));
    }
    const pm = modes(), pdisp = md && "plate_disp" in md ? md.plate_disp : undefined;
    if (md && m && pm?.kind === "space" && pm.plates && pdisp?.length) {
      const [pn, pt] = pm.plates, cs = palette(opts().contours, false).map(c => new THREE.Color(c));
      const pos = new Float32Array(pn.length * 3), col = new Float32Array(pn.length * 3);
      pn.forEach((p, i) => {
        const d = pdisp[i], a = Math.hypot(d[0], d[1], d[2]);
        for (let c = 0; c < 3; c++) pos[3 * i + c] = p[c] + d[c] * k;
        const cc = cs[Math.max(0, Math.min(BANDS - 1, Math.floor(a * BANDS)))];
        col[3 * i] = cc.r; col[3 * i + 1] = cc.g; col[3 * i + 2] = cc.b;
      });
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
      g.setAttribute("color", new THREE.BufferAttribute(col, 3));
      g.setIndex(pt.flat());
      g.computeVertexNormals();
      modeGroup.add(new THREE.Mesh(g, new THREE.MeshLambertMaterial({ vertexColors: true, side: THREE.DoubleSide })));
    }
    if (md && m) {
      const P = at3(m), pts: number[] = [];
      for (const sh of md.members) {
        const b = m.members.find(x => x.id === sh.id), a = b && P.get(b.a), c = b && P.get(b.b); if (!a || !c) continue;
        const n = sh.d.length - 1, at = (i: number): P3 => { const t = i / n, d = sh.d[i]; return [a[0] + (c[0] - a[0]) * t + d[0] * k, a[1] + (c[1] - a[1]) * t + d[1] * k, a[2] + (c[2] - a[2]) * t + d[2] * k]; };
        for (let i = 0; i < n; i++) pts.push(...at(i), ...at(i + 1));
      }
      if (pts.length) {
        const g = new LineSegmentsGeometry(); g.setPositions(pts);
        const [w, h] = size();
        const mt = new LineMaterial({ color: new THREE.Color(css("--blue")).getHex(), linewidth: 3.5, worldUnits: false });
        mt.resolution.set(w, h);
        lineMats.push(mt);
        modeGroup.add(new LineSegments2(g, mt));
      }
    }
    draw();
  });
  // A new model (another file or example) is framed afresh.
  let lastName = "";
  createEffect(() => { const m = model(); const key = m ? `${m.name}|${m.nodes.length === 0}|${m.shapes.length === 0}` : ""; if (key !== lastName) { const was = lastName; lastName = key; if (!((picking() || sketch()) && was.split("|")[0] === key.split("|")[0])) queueMicrotask(() => fit()); } });

  return (
    <div class="view3d canvas" ref={host} classList={{ [`t-${tool().split(":")[0]}`]: true }} onPointerDown={onDown} onPointerUp={onUp} onPointerMove={onMove} onPointerLeave={() => setProbe(null)} onContextMenu={e => { e.preventDefault(); if (!isSolidA(model()?.analysis)) return; const rc = host.getBoundingClientRect(), h = cadHit(e.clientX - rc.left, e.clientY - rc.top); if (h.face) bodyMenu(e, h.face.body); }} tabIndex={0}
      onKeyDown={e => { if ((e.target as HTMLElement).tagName === "INPUT") return; if (e.key === "Escape") { if (sketchPick()) setSketchPick(false); else if (tool() !== "select") setTool("select"); else setSel([]); } else if (e.key === "Delete") C.del(); else if (e.key === "f" || e.key === "F") fit(true); }}>
      <Show when={isSolidA(model()?.analysis) && !activeSketch()}><Timeline /></Show>
      <Show when={isSolidA(model()?.analysis) && !activeSketch()}><ExtrudePanel /></Show>
      <Show when={activeSketch()}><div class="cadover" onPointerDown={e => e.stopPropagation()} onPointerUp={e => e.stopPropagation()} onPointerMove={e => e.stopPropagation()}><SketchCanvas /></div></Show>
      <Show when={isSolidA(model()?.analysis) && !model()?.shapes.length && !model()?.volume && !model()?.parts?.length && !cad()?.sketches.length && !cad()?.bodies.length && !planePick()}><div class="empty corner"><b>Build a part</b><span>Click one of the three planes at the origin to sketch on it: the view turns to face it. Draw a closed shape (shapes inside it become holes), then Finish Sketch and set how far it is extruded.</span></div></Show>
      <Show when={planePick()}><div class="pickplane" onPointerDown={e => e.stopPropagation()} onPointerUp={e => e.stopPropagation()}><b>New Sketch</b><span>Click a plane at the origin or a flat face of a solid, or</span>
        <For each={[["XY", "Floor (XY)"], ["XZ", "Front (XZ)"], ["YZ", "Side (YZ)"]] as const}>{([k, l]) => <button class="btn sm" onClick={() => { setPlanePick(false); newSketch(k); }}>{l}</button>}</For>
        <button class="btn sm" onClick={() => setPlanePick(false)}>Cancel</button></div></Show>
      <Show when={model()?.analysis === "space" && !model()?.nodes.length && !sketch() && !activeSketch() && !planePick() && !cad()?.sketches.length}><Show when={!picking()}><div class="empty corner"><b>Draw a 3D frame</b><span>New Sketch, then click one of the three planes at the origin: the view turns to it. Draw the frame with lines (and arcs), sized with dimensions as for a part; Finish Sketch makes them beams (or truss bars), joined where they meet. Sketch on the next plane for the next frame. Or draw members one by one with Beam and Truss Bar.</span><button class="btn pri" onClick={() => { pick3(null); setPlanePick(true); }}>New Sketch</button></div></Show></Show>
      <SweepPanel />
      <Show when={picking()}><div class="draw3 pickbar" onPointerDown={e => e.stopPropagation()} onPointerUp={e => e.stopPropagation()}>
        <b>New sketch</b><span class="muted">{model()?.parts?.length ? "Pick a plane: a square at the origin, a flat face of a part, or" : "Pick a plane: click a square at the origin, or"}</span>
        <span class="seg">{(["xy", "zy", "xz"] as WP[]).map(k => <button title={`Sketch on the ${({ xy: "Front (X–Z)", zy: "Side (Y–Z)", xz: "Top (X–Y)" } as Record<WP, string>)[k]} plane`} onClick={() => sketchOn(k)}>{({ xy: "Front", zy: "Side", xz: "Top" } as Record<WP, string>)[k]}</button>)}</span>
        <button class="btn sm" onClick={() => { setSketchPick(false); setTool("select"); }}>Cancel</button>
      </div></Show>
      <Show when={probe3() && solidRes()}>{_ => { const r = solidRes()!, i = probe3()!.i, p = r.surface.nodes[i], fd = curField(), v = r.fields.find(f => f.key === fd.key)?.v[i] ?? 0, d = solidDisp(r)[i]; return (
        <div class="probe"><b>At {toD(p as P3).map(c => fmt("coord", c).split(" ")[0]).join(", ")} {unit("coord")}</b><span>{fd.short} {fmt(fd.q, v)}</span><Show when={r.kind === "solid"}><span>|u| {fmt("len", Math.hypot(d[0], d[1], d[2]))}</span></Show></div>); }}</Show>
      <Show when={probe()}>{p => { const mr = p().mr, i = p().i, d = mr.d[i]; return (
        <div class="probe"><b>Member {(model()?.members.findIndex(x => x.id === mr.id) ?? 0) + 1} at {fmt("len", mr.x[i])}</b>
          <span>N {fmt("force", clean(mr.n[i], 1e-3))}</span><span>Vy {fmt("force", clean(mr.vy[i], 1e-3))} · Vz {fmt("force", clean(mr.vz[i], 1e-3))}</span><span>T {fmt("moment", clean(mr.t[i], 1e-3))}</span><span>Mz {fmt("moment", clean(mr.mz[i], 1e-3))} · My {fmt("moment", clean(mr.my[i], 1e-3))}</span><span>δ {fmt("len", dlen(d))}</span></div>); }}</Show>
      <svg class="axkey3" ref={keyEl} viewBox="0 0 52 52" width="52" height="52" aria-label="Axes, Z up">
        <g><line x1="26" y1="26" x2="46" y2="26" stroke="#D64545" stroke-width="2.2" /><text fill="#D64545">X</text></g>
        <g><line x1="26" y1="26" x2="26" y2="26" stroke="#2E9D57" stroke-width="2.2" /><text fill="#2E9D57">Y</text></g>
        <g><line x1="26" y1="26" x2="26" y2="6" stroke="#2F6FD0" stroke-width="2.2" /><text fill="#2F6FD0">Z</text></g>
      </svg>
      <div class="vcube" on:pointerdown={e => e.stopPropagation()} on:pointermove={e => e.stopPropagation()} on:pointerup={(e: PointerEvent) => {
        // The face under the pointer, found here from the faces' outlines as drawn (some web views
        // do not hit-test the cube's SVG shapes).
        e.stopPropagation();
        if (!cubeEl) return;
        const b = cubeEl.getBoundingClientRect(), x = (e.clientX - b.left) * 64 / b.width, y = (e.clientY - b.top) * 64 / b.height;
        const inside = (p: [number, number][]) => { let c = false; for (let i = 0, j = p.length - 1; i < p.length; j = i++) if ((p[i][1] > y) !== (p[j][1] > y) && x < (p[j][0] - p[i][0]) * (y - p[i][1]) / (p[j][1] - p[i][1]) + p[i][0]) c = !c; return c; };
        const hit = [...cubePolys].sort((a, c) => c.depth - a.depth).find(f => inside(f.pts));
        if (hit) lookFrom(FACES[hit.k][1]);
      }}>
        <button class="vhome" aria-label="Home: the corner view of the whole model" onClick={() => fit()}><svg class="ic"><use href="#i-home" /></svg></button>
        <svg class="cube" ref={cubeEl} viewBox="0 0 64 64" width="84" height="84">
          {FACES.map(([n, d]) => <g class={`cf cf-${n.toLowerCase()}`} data-face={n} aria-label={`Look from the ${n.toLowerCase()}`} onClick={() => lookFrom(d)}><polygon /><text text-anchor="middle">{n}</text></g>)}
        </svg>
      </div>
      <div class="zoomctl"><button title="Fit the whole model in view (F)" onClick={() => fit(true)}>Fit</button></div>
    </div>
  );
};

