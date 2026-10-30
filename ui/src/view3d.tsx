// The 3D view of a 3D frame (0.2.4), drawn with three.js: members, nodes, supports and loads; once
// solved, the deflected shape coloured by a result, with a probe that reads the values under the
// pointer. Turn with the left button, pan with the right, zoom with the wheel. Clicking picks
// nodes and members (and places supports and loads); drawing happens on a working plane.
import { Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { LineSegments2 } from "three/examples/jsm/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { LineMaterial } from "three/examples/jsm/lines/LineMaterial.js";
import { Model, SpaceMember, Target, clean, isSolidA, solidDisp } from "./api";
import { model, sel, setSel, tool, setTool, act, spaceRes, stale, show, themeTick, opts, toast, setEdgeSel, modes, modeOn, modeSel, phase, solidRes, shownLoad } from "./store";
import { palette, BANDS, curField, rangeOf } from "./parts";
import { outline, edgePts, inside } from "./geom";
import { defaultLoad, defaultSupport } from "./panels";
import { fmt, unit } from "./units";
import { setPlane, fit3d } from "./plane3";
import { C } from "./cmds";
import { P3, at3, span3, dlen, deform3, vals, q3, range3, cut } from "./space3";
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
  const draw = () => { if (frame) return; frame = requestAnimationFrame(() => { frame = 0; renderer.render(scene, camera); }); };

  const clear = () => {
    lineMats.length = 0;
    world.traverse(o => { const x = o as any; x.geometry?.dispose?.(); (Array.isArray(x.material) ? x.material : [x.material]).forEach((mt: any) => mt?.dispose?.()); });
    world.clear();
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
  const buildSolid = (m: Model) => {
    const ink = css("--text"), green = css("--green"), load = css("--blue2"), accent = css("--blue");
    scene.background = new THREE.Color(css("--bg2"));
    const t = m.thickness, tol = Math.max(1e-6, (m.grid || 0.005) / 4), rev = m.revolve ?? 0, full = rev >= Math.PI * 2 - 1e-9;
    const bounds = m.shapes.flatMap(s => outline(s.geom, tol));
    const size = Math.max(rev ? 0 : t, ...(bounds.length ? [Math.max(...bounds.map(p => p[0])) - Math.min(...bounds.map(p => p[0])), Math.max(...bounds.map(p => p[1])) - Math.min(...bounds.map(p => p[1]))] : [1e-3]));
    world.add(new THREE.AxesHelper(size * 0.2));
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
      const disp = solidDisp(r), dmax = Math.max(0, ...disp.map(d => Math.hypot(d[0], d[1], d[2])));
      solidF = show().deformed && dmax > 1e-12 * size ? (() => { const f = 0.08 * size / dmax, p = 10 ** Math.floor(Math.log10(f)), k = f / p; return (k >= 5 ? 5 : k >= 2 ? 2 : 1) * p * show().scale; })() : 0;
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
    } else if (m.shapes.some(s => !s.hole) && (t > 0 || rev > 0)) {
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
    // Supported and loaded faces outlined: green for supports, crimson for loads.
    const selSet = new Set(sel());
    const faceLines = (at: Target, colour: string) => {
      const pts: number[] = [];
      if (at.on === "edge" || at.on === "line") {
        const s = m.shapes.find(x => x.id === at.shape); if (!s) return;
        const ps = edgePts(s.geom, at.edge, tol) as [number, number][];
        if (at.on === "line") loop(pts, ps, at.at);
        else {
          for (const f of full ? [0] : [0, 1]) loop(pts, ps, f);
          // The face's ends swept along (straight for an extrusion, round for a turn).
          const n = rev ? Math.max(2, Math.ceil(rev / (Math.PI / 48))) : 1;
          for (const p of [ps[0], ps[ps.length - 1]]) for (let i = 0; i < n; i++) pts.push(...sweep(p, i / n), ...sweep(p, (i + 1) / n));
        }
      } else if (at.on === "end") {
        for (const s of m.shapes) { const o = outline(s.geom, tol) as [number, number][]; loop(pts, [...o, o[0]], at.front ? 1 : 0); }
      }
      if (pts.length) fat(pts, null, colour, at.on === "line" ? 5 : 3.5);
    };
    for (const s of m.supports) faceLines(s.at, selSet.has(s.id) ? accent : green);
    for (const l of m.loads.filter(x => shownLoad(m, x))) faceLines(l.at, selSet.has(l.id) ? accent : load);
    draw();
  };

  const build = () => {
    themeTick();
    const m = model(); if (!m) return;
    clear();
    if (isSolidA(m.analysis)) return buildSolid(m);
    const P = at3(m), span = span3(m), selSet = new Set(sel());
    const ink = css("--text"), accent = css("--blue"), faint = css("--line3"), load = css("--blue2");
    scene.background = new THREE.Color(css("--bg2"));
    // The ground grid at the lowest node, and the axes at the origin.
    const ys = m.nodes.map(n => n.y), y0 = ys.length ? Math.min(...ys) : 0;
    if (opts().grid) {
      const step = m.grid > 0 ? m.grid : 1, n = Math.min(400, Math.max(4, Math.ceil(span * 1.6 / step)));
      const gh = new THREE.GridHelper(n * step, n, new THREE.Color(css("--line3")), new THREE.Color(css("--line2")));
      const cx = m.nodes.length ? (Math.min(...m.nodes.map(n => n.x)) + Math.max(...m.nodes.map(n => n.x))) / 2 : 0, cz = m.nodes.length ? (Math.min(...m.nodes.map(n => n.z ?? 0)) + Math.max(...m.nodes.map(n => n.z ?? 0))) / 2 : 0;
      gh.position.set(Math.round(cx / step) * step, y0, Math.round(cz / step) * step);
      world.add(gh);
    }
    const ax = new THREE.AxesHelper(span * 0.15);
    world.add(ax);
    const r = spaceRes(), f = r ? deform3(m, r) : 0, q = q3(), rg = range3(), cols = palette(opts().contours).map(c => new THREE.Color(c));
    const showRes = !!r && r.members.length === m.members.length;
    // Members: as drawn (faint when the deflected shape is on top), selected ones in the accent.
    const plain: number[] = [], chosen: number[] = [], bars: number[] = [];
    for (const b of m.members) {
      const a = P.get(b.a), c = P.get(b.b); if (!a || !c) continue;
      (selSet.has(b.id) ? chosen : b.kind === "truss" ? bars : plain).push(...a, ...c);
    }
    const resOn = (showRes && show().deformed) || !!mode3();
    if (plain.length) fat(plain, null, resOn ? faint : ink, resOn ? 1.5 : 3);
    if (bars.length) fat(bars, null, resOn ? faint : ink, resOn ? 1.2 : 2, true);
    if (chosen.length) fat(chosen, null, accent, 4);
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
  const fit = (keepDir = false, from?: P3) => {
    const m = model(); if (!m) return;
    const box = new THREE.Box3(); m.nodes.forEach(n => box.expandByPoint(new THREE.Vector3(n.x, n.y, n.z ?? 0)));
    // A solid: its outline at both ends of the extrusion.
    if (isSolidA(m.analysis)) for (const s of m.shapes) for (const p of outline(s.geom, m.revolve ? 1e-3 : Infinity)) {
      if (m.revolve) for (let i = 0; i <= 16; i++) { const a = m.revolve * i / 16; box.expandByPoint(new THREE.Vector3(p[0] * Math.cos(a), p[1], p[0] * Math.sin(a))); }
      else for (const z of [0, m.thickness]) box.expandByPoint(new THREE.Vector3(p[0], p[1], z));
    }
    if (box.isEmpty()) return;
    const c = box.getCenter(new THREE.Vector3()), rad = Math.max(box.getSize(new THREE.Vector3()).length() / 2, 1e-3);
    const dir = from ? new THREE.Vector3(...from).normalize() : keepDir && controls ? camera.position.clone().sub(controls.target).normalize() : new THREE.Vector3(1, 0.75, 1.3).normalize();
    camera.position.copy(c).addScaledVector(dir, rad / Math.sin(camera.fov * Math.PI / 360) * 1.15);
    camera.near = rad / 100; camera.far = rad * 200; camera.updateProjectionMatrix();
    controls?.target.copy(c); controls?.update();
    draw();
  };

  /** Screen position of a 3D point. */
  const screen = (p: P3): [number, number] => {
    const v = new THREE.Vector3(...p).project(camera), [w, h] = size();
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
    const rc = host.getBoundingClientRect(), t = tool();
    if (isSolidA(model()?.analysis)) {
      if (t === "select") { setSel([]); return; }
      const at = pickFace(e.clientX - rc.left, e.clientY - rc.top);
      const ok = model()?.analysis === "solidheat" ? ["sup:temperature", "load:flux", "load:convection"].includes(t) : (t.startsWith("sup:") && !["sup:temperature", "sup:rollerx", "sup:rollery", "sup:spring"].includes(t)) || ["load:pressure", "load:traction"].includes(t);
      if (!at || !ok) { toast(ok ? "Click on the part: a side face, or the back or front face." : "Put it on a face: an edge in Section (its side face), or the back or front face."); return; }
      const [kind, val] = t.startsWith("sup:") ? ["Support", defaultSupport(at.on === "end" && t === "sup:pinned" ? "fixed" : t.slice(4))] : ["Load", defaultLoad(t.slice(5), false)];
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
    // Drawing tools draw on a plane.
    setPlane("xy");
    toast("Members are drawn on a plane: Front (x-y) is open now; pick Side or Plan above, and the depth.", true);
  };
  /** The face of a solid under the pointer (0.3.2): the back or front face, or the side face of the
   * section edge nearest the point hit. */
  const pickFace = (x: number, y: number): Target | null => {
    const m = model(); if (!m) return null;
    const [w, h] = size(), ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(x / w * 2 - 1, 1 - y / h * 2), camera);
    const hit = ray.intersectObjects(world.children.filter(o => o.userData.pick), false)[0]; if (!hit) return null;
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
  createEffect(() => { const m = model(); const key = m ? `${m.name}|${m.nodes.length === 0}|${m.shapes.length === 0}` : ""; if (key !== lastName) { lastName = key; queueMicrotask(() => fit()); } });

  return (
    <div class="view3d canvas" ref={host} classList={{ [`t-${tool().split(":")[0]}`]: true }} onPointerDown={onDown} onPointerUp={onUp} onPointerMove={onMove} onPointerLeave={() => setProbe(null)} onContextMenu={e => e.preventDefault()} tabIndex={0}
      onKeyDown={e => { if (e.key === "Escape") { if (tool() !== "select") setTool("select"); else setSel([]); } else if (e.key === "Delete") C.del(); else if (e.key === "f" || e.key === "F") fit(true); }}>
      <Show when={isSolidA(model()?.analysis) && !model()?.shapes.length}><div class="empty"><b>Draw the part's section</b><span>A 3D solid is the outline of a part extruded along z by its thickness. Open Section above and draw the outline and its holes, as for a plane part; set the thickness in the Inspector.</span><button class="btn pri" onClick={() => { setPlane("xy"); setTool("rect"); }}>Draw a rectangle</button></div></Show>
      <Show when={model()?.analysis === "space" && !model()?.nodes.length}><div class="empty"><b>Draw a 3D frame</b><span>Members are drawn on a plane: pick Front, Side or Plan above and the depth, then Beam or Truss in Geometry. Repeat copies a frame along the depth and joins it (Geometry › Repeat).</span><button class="btn pri" onClick={() => { setPlane("xy"); setTool("beam"); }}>Draw on the front plane</button></div></Show>
      <Show when={probe3() && solidRes()}>{_ => { const r = solidRes()!, i = probe3()!.i, p = r.surface.nodes[i], fd = curField(), v = r.fields.find(f => f.key === fd.key)?.v[i] ?? 0, d = solidDisp(r)[i]; return (
        <div class="probe"><b>At {p.map(c => fmt("coord", c).split(" ")[0]).join(", ")} {unit("coord")}</b><span>{fd.short} {fmt(fd.q, v)}</span><Show when={r.kind === "solid"}><span>|u| {fmt("len", Math.hypot(d[0], d[1], d[2]))}</span></Show></div>); }}</Show>
      <Show when={probe()}>{p => { const mr = p().mr, i = p().i, d = mr.d[i]; return (
        <div class="probe"><b>Member {(model()?.members.findIndex(x => x.id === mr.id) ?? 0) + 1} at {fmt("len", mr.x[i])}</b>
          <span>N {fmt("force", clean(mr.n[i], 1e-3))}</span><span>Vy {fmt("force", clean(mr.vy[i], 1e-3))} · Vz {fmt("force", clean(mr.vz[i], 1e-3))}</span><span>T {fmt("moment", clean(mr.t[i], 1e-3))}</span><span>Mz {fmt("moment", clean(mr.mz[i], 1e-3))} · My {fmt("moment", clean(mr.my[i], 1e-3))}</span><span>δ {fmt("len", dlen(d))}</span></div>); }}</Show>
      <div class="axkey"><i class="ax-x">x</i><i class="ax-y">y</i><i class="ax-z">z</i></div>
      <div class="zoomctl"><button title="Fit (F)" onClick={() => fit(true)}>Fit</button><button title="Look from the front" onClick={() => fit(false, [0, 0, 1])}>Front</button><button title="Look from above" onClick={() => fit(false, [0, 1, 0.001])}>Top</button><button title="Look from the corner" onClick={() => fit()}>Iso</button></div>
    </div>
  );
};

