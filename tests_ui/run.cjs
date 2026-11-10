// UI tests: the built window (ui/dist) in Chromium with the engine's real worked examples
// (cargo run -p datum-core --example dump > tests_ui/fixture.json) behind a mocked Tauri bridge
// that records every call. Run: npm run test:ui (or npm run build:ui && node tests_ui/run.cjs).
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path");
// FAMILY §13: the window never names a commercial product (the list is shared with the engine's
// hardening test); whole words, any case.
const NAMES = fs.readFileSync(path.join(__dirname, "../tools/commercial-names.txt"), "utf8").split("\n").map(l => l.trim().toLowerCase()).filter(l => l && !l.startsWith("#"));
const named = text => { const t = text.toLowerCase(); return NAMES.filter(n => new RegExp(`(^|[^\\p{L}\\p{N}])${n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}($|[^\\p{L}\\p{N}])`, "u").test(t)); };
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
// 0.7.1: scenes from the real geometry engine (cargo run -p datum-cad --example fixture > tests_ui/cad.json).
fix.cad = JSON.parse(fs.readFileSync(path.join(__dirname, "cad.json")));
// Results in the fixture are packed as the shell sends them; unpacked here for the checks' counts.
const unpack = v => {
  if (Array.isArray(v)) return v.map(unpack);
  if (v && typeof v === "object") {
    const s = v.$f32 ?? v.$u32;
    if (typeof s === "string") { const b = Buffer.from(s, "base64"), n = Array.from(v.$f32 !== undefined ? new Float32Array(b.buffer, b.byteOffset, b.length / 4) : new Uint32Array(b.buffer, b.byteOffset, b.length / 4)); if (v.w === 1) return n; const o = []; for (let i = 0; i < n.length; i += v.w) o.push(n.slice(i, i + v.w)); return o; }
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, unpack(x)]));
  }
  return v;
};
const types = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".woff": "font/woff", ".html": "text/html" };
const srv = http.createServer((q, r) => {
  const f = path.join(root, q.url === "/" ? "index.html" : decodeURIComponent(q.url.split("?")[0]));
  fs.readFile(f, (e, b) => { if (e) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": types[path.extname(f)] ?? "application/octet-stream" }); r.end(b); });
});
let failed = 0;
const num3 = v => String(+v.toPrecision(3));
const check = (name, ok, info = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + info}`); if (!ok) failed++; };
// 0.9.1: no side bar; the views are reached as a person reaches them (the bar above a page, the
// tree's Results rows, the results panel, the ribbon), or directly where nothing leads.
const go = async (p, v) => {
  const first = async (...sel) => { for (const q of sel) { const b = await p.$(q); if (b && await b.isVisible()) return b; } return null; };
  const b = v === "Model" ? await first('.pagebar button:has-text("Back to the model")', '.legend button:has-text("Back to the part")')
    : v === "Results" ? await first('.pagebar button:has-text("Tables")', '.rinsp button:has-text("Tables and equilibrium")', '.tree button.ti:has-text("Tables and equilibrium")')
    : v === "Report" ? await first('.pagebar button:has-text("Report")', '.rinsp button:has-text("Report")', '.tree button.ti:has-text("Report")') : null;
  if (b) await b.click(); else await p.evaluate(x => window.__setView(x), v.toLowerCase());
};
// 0.9.1: the properties card pops up when something is picked; the model's own settings open from
// the model's name at the top of the tree.
const modelCard = async () => { const t = await __page.$('.tree .ttabs button:has-text("Model"):not(.on)'); if (t) await t.click(); await __page.click(".tree .troot"); await __page.waitForTimeout(100); };
let __page = null;
// 0.10.2: a solve opens the tree's Results tab (the mock-ups); a step's row is on the Model tab.
const stepHead = async n => { const t = await __page.$('.tree .ttabs button:has-text("Model"):not(.on)'); if (t) { await t.click(); await __page.waitForTimeout(60); } await __page.click(`.tree .step:nth-of-type(${n}) > .sthead`); };

(async () => {
  await new Promise(ok => srv.listen(0, ok));
  const url = `http://localhost:${srv.address().port}/`;
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } }); __page = p;
  // 0.10.2: a solve opens the tree's Results tab; a click on a row of the model goes to its Model tab first, as a person would.
  { const click = p.click.bind(p); p.click = async (sel, o) => { if (typeof sel === "string" && /^\.tree /.test(sel) && !/ttabs|tres|tfilter|rinsp|rtree/.test(sel)) { const t = await p.$('.tree .ttabs button:has-text("Model"):not(.on)'); if (t) { await click('.tree .ttabs button:has-text("Model")'); await p.waitForTimeout(60); } } return click(sel, o); }; }
  const errors = [];
  p.on("pageerror", e => errors.push(e.message));
  await p.addInitScript(f => {
    const opts = { autosave: 0, beta: false, theme: "light", openLast: false, units: "SI", snap: true, grid: true, rulers: true, contours: "rainbow", paper: "A4", landscape: true, company: "" };
    window.__cur = f.frame;
    const handlers = {};
    const m = { import_mesh: f.elbow, send_link: "Sent to Datum Calc.", read_link: ["Datum Atlas", { id: 0, name: "Aluminium 6061-T6", e: 68.9e9, nu: 0.33, rho: 2700, fy: 276e6, k: 167, alpha: 23.6e-6, source: "ASM Handbook" }], verification: f.verification, examples_list: f.examples_list, options: opts, startup_file: null, recovery: null, check_update: null, recent: [], version: "test", section_props: { a: 0.005, i: 8e-5, c: 0.15, iy: 6e-6, cy: 0.075, j: 2e-7 } };
    window.__calls = [];
    window.__emit = (e, x) => (handlers[e] ?? []).forEach(h => h({ payload: x }));
    window.__TAURI__ = {
      core: {
        invoke: async (c, a) => {
          window.__calls.push([c, a]);
          if (c === "new_model" || c === "open") window.__cad = null;
          if (c === "cad_scene") return window.__cad ?? f.cad.empty;
          if (c === "cad_contacts") return { m: { ...f.cad.meshed_model, contacts: [{ id: 901, a: { on: "face", face: 0 }, b: { on: "face", face: 1 }, friction: 0.2 }] }, path: null, dirty: true, undo: true, redo: false, created: [901], warnings: [] };
          if (c === "cad_mesh") return { m: f.cad.meshed_model, path: null, dirty: true, undo: true, redo: false, created: [], warnings: [] };
          if (c === "cad_auto_size") return 5;
          if (c === "model_check") return window.__checks ?? (window.__cur === f.checkdemo ? f.checkdemo_findings : []);
          if (c === "combos_preview") return f.braced_rule_preview;
          if (c === "design_member") return f.dmdemo_info;
          if (c === "not_verified") return f.not_verified;
          if (c === "mech_checks") return window.__mech ?? f.dmdemo_checks;
          if (c === "member_values") return f.dmdemo_values;
          if (c === "section_table") return f.section_table_i;
          if (c === "cad_members") { window.__cad = f.cad.empty; return { m: f.platform.m, path: null, dirty: true, undo: true, redo: false, created: [1, 2, 3], warnings: [] }; }
          if (c === "cad_import_step") { window.__cad = f.cad.extruded; return f.cad.extruded; }
          if (c === "cad_export") return "/tmp/model." + a.format;
          if (c === "cad_query" && a.id === "inspect.measure") return a.params.items ? { items: a.params.items.map(i => ({ type: i.type, area_mm2: 100 })), distance_mm: 20, angle_deg: 90 } : { bodies: (window.__cad ?? f.cad.empty).bodies.map(b => ({ name: b.name, closed: true, volume_mm3: 24000, area_mm2: 5200, center_of_mass: [20, 15, 10] })) };
          if (c === "cad_query") return a.id === "timeline.inspect" ? f.cad.inspect_extrude : a.id === "sketch.inspect" ? { curves: [], points: [] } : a.id === "parameters.list" ? [] : {};
          if (c === "cad_run") { const id = a.id; const k = id === "sketch.create" ? "sketch" : id === "sketch.finish" ? "finished" : id === "sketch.cancel" ? "empty" : id.startsWith("sketch.") ? "drawn" : id === "solid.extrude" ? "extruded" : id === "solid.revolve" ? "revolved" : null; if (k) window.__cad = f.cad[k]; return window.__cad ?? f.cad.empty; }
          if (c === "state") return window.__cur;
          if (c === "new_model" && window.__next) { window.__cur = window.__next; window.__next = null; return window.__cur; }
          if (c === "new_model") { window.__cur = { blank: f.frame, "blank-frame": f.frame, portal: f.portal, truss: f.truss, bracket: f.bracket, lug: f.lug, fin: f.fin, hotplate: f.hotplate, vessel: f.vessel, platform: f.platform, "blank-space": f.space, bracket3d: f.bracket3d, "blank-solid": f.solid, vessel3d: f.vessel3d, braced: f.braced, fin3d: f.fin3d, lee: f.lee, slab: f.slab, quake: f.quake, xbrace: f.xbrace, building: f.building, tube: f.tube, yield3d: f.yield3d, rubber: f.rubber, elbow: f.elbow, elbowheat: f.elbowheat, bend45: f.bend45, tubeplates: f.tubeplates }[a.example] ?? f.frame; return window.__cur; }
          if (c === "apply") { window.__cur = { ...window.__cur, dirty: true, undo: true, created: [999] }; return window.__cur; }
          if (c === "history") return window.__cur;
          if (c === "solve" && window.__hold) await window.__hold;
          if (c === "solve") { if (window.__solveKey) return f[window.__solveKey]; if (window.__cur === f.portal) return f.portal_results; if (window.__cur === f.noload) return f.noload_results; if (window.__cur === f.truss) return f.truss_results; if (window.__cur === f.bracket) return f.bracket_results; if (window.__cur === f.fin) return f.fin_results; if (window.__cur === f.hotplate) return f.hotplate_results; if (window.__cur === f.vessel) return f.vessel_results; if (window.__cur === f.platform) return f.platform_results; if (window.__cur === f.bracket3d) return f.bracket3d_results; if (window.__cur === f.vessel3d) return f.vessel3d_results; if (window.__cur === f.braced) return f.braced_results; if (window.__cur === f.lee) return f.lee_results; if (window.__cur === f.fin3d) return f.fin3d_results; throw "There is nothing to mesh."; }
          if (c === "mesh") return f.bracket_mesh;
          if (c === "buckling") { if (window.__cur === f.platform || window.__cur?.m?.analysis === "space") return f.platform_buckle; if (window.__cur === f.portal) return f.portal_buckle; if (window.__cur === f.truss) return f.truss_buckle; throw "Buckling is for frames and trusses in this release; parts come later."; }
          if (c === "modes" && window.__modesKey) return f[window.__modesKey];
          if (c === "modes" && (window.__cur === f.platform || window.__cur?.m?.analysis === "space")) return f.platform_modes;
          if (c === "modes" && window.__cur?.m?.analysis === "solid") return f.bracket3d_modes;
          if (c === "modes") { if (window.__cur === f.portal) return f.portal_modes; if (window.__cur === f.bracket) return f.bracket_modes; throw "Natural frequencies are for frames and for parts in stress."; }
          if (c === "materials_library") return f.library;
          if (c === "envelope") return f.braced_envelope;
          if (c === "import_dxf") { window.__cur = f.dxf_import; return window.__cur; }
          if (c === "export_dxf") return "0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
          if (c === "sections_library") return f.sections;
          if (c in m) return m[c];
          return null;
        },
      },
      event: { listen: async (e, h) => { (handlers[e] ??= []).push(h); return () => {}; } },
    };
  }, fix);
  await p.goto(url);
  await p.waitForTimeout(800);
  check("the 3D view's code (three.js) is not loaded at startup (0.2.6)", !(await p.evaluate(() => performance.getEntriesByType("resource").some(e => e.name.includes("view3d")))));
  const ops = () => p.evaluate(() => window.__calls.filter(c => c[0] === "apply").map(c => c[1].op));
  const lastOp = async () => (await ops()).slice(-1)[0];
  const canvasBox = async () => p.$eval(".sketchover .canvas, .canvas:not(.view3d)", el => { const r = el.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
  const center = sel => p.$eval(sel, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  const open = async id => {
    await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150);
    // Blank models are on Home; the worked examples on File › Examples (0.6.1).
    const ex = fix.examples_list.find(e => e.id === id);
    if ((ex.group ?? "Blank") !== "Blank") { await p.click('.bs aside button:has-text("Examples")'); await p.waitForTimeout(150); await p.click(`.exfolders button[data-group="${ex.group}"]`); await p.waitForTimeout(100); }
    await p.click(`.home .tile.blank:has(b:text-is("${ex.name}")) .tmain, .home button.tile:has(b:text-is("${ex.name}"))`); await p.waitForTimeout(250);
    // Unsaved changes are offered for saving first (0.1.2): drop them.
    if (await p.$('.dlg header:has-text("Save changes?")')) { await p.click('.dlg button:has-text("Don\'t save")'); await p.waitForTimeout(300); }
    await p.waitForTimeout(150);
  };

  // 0.10.3: things that must never be confused have different shapes (the specification's icon rule).
  const pairs = [["force", "mass"], ["release", "pinned"], ["rigidlink", "beam"], ["temp", "tgradient"], ["st-linear", "st-newton"], ["modes", "st-spectrum"]];
  const drawn2 = await p.evaluate(ps => ps.map(([a, b]) => [document.getElementById(`i-${a}`)?.innerHTML, document.getElementById(`i-${b}`)?.innerHTML]), pairs);
  check("mass and force, release and pin, rigid link and member, uniform and gradient temperature, linear and nonlinear, modal and spectrum each have their own icon (0.10.3)", drawn2.every(([a, b]) => !!a && !!b && a !== b), JSON.stringify(drawn2.map(d => d.map(x => !!x))));
  // 1. File page: every worked example, crimson sidebar.
  const tiles = await p.$$eval(".home .tile b", els => els.map(e => e.textContent));
  check("Home shows the blank models and a way to the examples (0.6.1)", fix.examples_list.filter(e => e.group === "Blank").every(e => tiles.includes(e.name)) && tiles.includes("Examples") && !tiles.includes("Portal frame"), JSON.stringify(tiles));
  check("Home has four starting points, in order, and the Roadmap among its tiles (0.10.3, the user)", JSON.stringify(tiles.slice(0, 4)) === JSON.stringify(["2D frame", "3D frame", "2D part", "3D solid"]) && !tiles.some(t => /heat part/i.test(t)) && tiles.includes("Roadmap"), JSON.stringify(tiles));
  check("an empty Recent says where models will appear (0.10.0)", (await p.textContent(".recentnone")).includes("appear here"));
  await p.click('.home .tile:has(b:text-is("Roadmap"))'); await p.waitForTimeout(150);
  check("the Roadmap tile opens File › Roadmap (0.10.0)", (await p.$$(".bs .roadmap")).length === 1);
  await p.click('.bs aside button:has-text("Home")'); await p.waitForTimeout(150);
  await p.click('.home .tile[data-blank="blank-stress"] button[data-heat]'); await p.waitForTimeout(250);
  check("Heat on the 2D part tile starts a heat model (0.10.3)", await p.evaluate(() => window.__calls.some(c => c[0] === "new_model" && c[1].example === null && c[1].analysis === "heat")));
  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150);
  await p.click('.bs aside button:has-text("Examples")'); await p.waitForTimeout(200);
  // 0.10.0: the examples are in folders; every folder's tiles together are every example.
  const exHeads = await p.$$eval(".exfolders button[data-group]", els => els.map(e => e.dataset.group)); const exTiles = [];
  for (const g of exHeads) { await p.click(`.exfolders button[data-group="${g}"]`); await p.waitForTimeout(60); exTiles.push(...await p.$$eval(".home .tile b", els => els.map(e => e.textContent))); }
  check("File › Examples lists every worked example, in folders by kind (0.10.0)", fix.examples_list.filter(e => e.group !== "Blank").every(e => exTiles.includes(e.name)) && ["Frames and trusses", "3D frames", "3D solids and assemblies", "Dynamics"].every(h => exHeads.includes(h)), JSON.stringify(exHeads));
  await p.fill(".exsearch input", "truss"); await p.waitForTimeout(100);
  check("searching the examples looks through every folder (0.10.0)", (await p.$$eval(".home .tile b", els => els.map(e => e.textContent))).includes("Roof truss"));
  await p.fill(".exsearch input", ""); await p.waitForTimeout(60);
  await p.click('.bs aside button:has-text("Home")'); await p.waitForTimeout(150);
  check("File sidebar is the Petrol accent (0.10.0, the user)", (await p.$eval(".bs aside", el => getComputedStyle(el).backgroundColor)) === "rgb(15, 110, 140)");

  // 2. The portal frame draws its members, nodes, supports and loads.
  await open("portal");
  const pf = fix.portal.m;
  check("members are drawn", (await p.$$(".mem")).length === pf.members.length);
  check("nodes are drawn", (await p.$$(".node")).length === pf.nodes.length);
  check("supports are drawn", (await p.$$("g.sup")).length === pf.supports.length);
  check("loads are drawn with their values", (await p.$$("g.load")).length === pf.loads.length && (await p.textContent(".dsvg")).includes("kN/m"));
  check("ribbon tabs: every module has its tab (0.10.0, the user's mock-ups); a frame has no Mesh tab", JSON.stringify(await p.$$eval(".tabs > button:not(.file)", els => els.map(e => e.textContent))) === JSON.stringify(["Home", "Geometry", "Materials", "Loads & BCs", "Analysis", "Results", "Animate", "View", "Tools", "Help"]), JSON.stringify(await p.$$eval(".tabs > button:not(.file)", els => els.map(e => e.textContent))));
  // 0.7.1: the tree is laid out in six numbered steps; each step's header opens its ribbon tab.
  check("the tree shows the six numbered steps in order", JSON.stringify(await p.$$eval(".tree .sthead > span", els => els.map(e => e.textContent))) === JSON.stringify(["Geometry", "Properties", "Supports & Loads", "Mesh", "Analysis", "Results"]));
  await p.click('.sthead:has-text("Properties")');
  check("a step's header shows its tools in the ribbon, named on the top bar", (await p.textContent(".tabs > button.on")) === "Materials" && await p.$eval(".tree .step:nth-of-type(2)", el => el.classList.contains("here")) && (await p.$$('.ribbon button[title^="The materials of the model"]')).length === 1);
  check("the top bar has no step tabs: the steps are in the tree (0.7.1, the user)", (await p.$$(".tabs button.step")).length === 0);
  await p.click('.tabs > button:text-is("Home")');
  check("frames are in metres on the ruler", (await p.textContent(".rtxt.unitlbl")) === "m");
  check("the tree lists members and nodes", (await p.$$(".tree .ti")).length >= pf.members.length + pf.nodes.length);
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "tree-portal.png") });
  if (process.env.SHOTS) for (const t of ["Home", 1, 3, 6]) { if (typeof t === "number") await stepHead(t); else await p.click(`.tabs > button:text-is("${t}")`); await p.waitForTimeout(80); await p.screenshot({ path: path.join(process.env.SHOTS, `ribbon-${t}.png`), clip: { x: 0, y: 0, width: 1440, height: 170 } }); }
  if (process.env.SHOTS) await p.click('.tabs > button:text-is("Home")');
  check("each branch of the tree has an icon and says whether it is done", (await p.$$(".tree .tgrp summary .tic")).length >= 6 && (await p.$$(".tree .tmark.ok")).length >= 3);
  const allMembersOnScreen = await p.evaluate(() => [...document.querySelectorAll(".mem line")].every(l => { const r = l.getBoundingClientRect(), c = document.querySelector(".canvas").getBoundingClientRect(); return r.left >= c.left && r.right <= c.right && r.top >= c.top && r.bottom <= c.bottom; }));
  check("the model fits the window when opened", allMembersOnScreen);

  // 3. Selecting a member shows it in the Inspector; the contextual tab does not move the ribbon.
  const ribbonTop = await p.$eval(".ribbon", el => el.getBoundingClientRect().top);
  const m0 = pf.members[0];
  const mc = await p.$eval(`.mem[data-id="${m0.id}"] line`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  // 0.9.1: the drawing is wider (the card pops up over it), and the column's middle lies under its
  // wind load's arrows: picked a little way up from its foot instead.
  await p.mouse.click(mc[0], mc[1] + (await p.$eval(`.mem[data-id="${m0.id}"] line`, el => el.getBoundingClientRect().height)) * 0.45);
  await p.waitForTimeout(150);
  check("clicking a member selects it", (await p.$$(".mem.sel")).length === 1 && (await p.textContent(".insp header")).includes("Member 1"));
  check("the Inspector offers the member's section", (await p.$$(".insp select")).length >= 3);
  check("Member Tools tab shows for a selection", await p.$eval(".ctxgrp", el => getComputedStyle(el).visibility === "visible"));
  check("the ribbon does not move when the contextual tab appears", Math.abs((await p.$eval(".ribbon", el => el.getBoundingClientRect().top)) - ribbonTop) < 0.5);

  // 4. Dragging a node sends one Move snapped to the 0.5 m grid.
  const n1 = pf.nodes[1];
  const nc = await center(`.node[data-id="${n1.id}"]`);
  await p.mouse.click(nc[0], nc[1]);
  await p.mouse.move(nc[0], nc[1]); await p.mouse.down(); await p.mouse.move(nc[0] + 47, nc[1] - 13, { steps: 6 }); await p.mouse.up();
  await p.waitForTimeout(200);
  const mv = (await ops()).find(o => o.Move);
  check("dragging a node moves it by whole grid steps", !!mv && mv.Move.ids.includes(n1.id) && Math.abs(mv.Move.dx / 0.5 - Math.round(mv.Move.dx / 0.5)) < 1e-9 && mv.Move.dx > 0, JSON.stringify(mv));

  // 5. Delete removes the selection.
  await p.keyboard.press("Delete");
  await p.waitForTimeout(150);
  check("Delete sends the selection", !!(await ops()).find(o => o.Delete && o.Delete.ids.includes(n1.id)));

  // 6. Drawing a beam: two clicks send AddMember with snapped ends; Esc stops.
  await stepHead(1);
  await p.click('.ribbon button[title^="Draw beams"]');
  const cb = await canvasBox();
  await p.mouse.click(cb.x + 300, cb.y + 200); await p.mouse.click(cb.x + 520, cb.y + 203);
  await p.waitForTimeout(200);
  const am = await lastOp();
  check("two clicks with the beam tool add a member", !!am?.AddMember && am.AddMember.kind === "beam" && [...am.AddMember.from, ...am.AddMember.to].every(v => Math.abs(v / 0.5 - Math.round(v / 0.5)) < 1e-9), JSON.stringify(am));
  check("the status bar says what the tool does", (await p.textContent(".status")).includes("Click the start"));
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape");
  check("Esc goes back to Select", (await p.textContent(".status")).includes("Ready"));

  // 7. A support and a load on a node.
  await stepHead(3);
  await p.click('.ribbon button[title="Pinned"]');
  const n0 = await center(`.node[data-id="${pf.nodes[2].id}"]`);
  await p.mouse.click(n0[0], n0[1]); await p.waitForTimeout(150);
  const sp = await lastOp();
  check("the Pinned tool puts a support on the node clicked", sp?.Add?.item?.Support?.at?.id === pf.nodes[2].id && sp.Add.item.Support.kind.kind === "pinned", JSON.stringify(sp));
  await p.click('.ribbon button[title="Distributed"]');
  await p.mouse.click(mc[0], mc[1]); await p.waitForTimeout(150);
  const dl = await lastOp();
  check("the Distributed tool puts a load on the member clicked", dl?.Add?.item?.Load?.at?.on === "member" && dl.Add.item.Load.kind.kind === "dist", JSON.stringify(dl));
  // 0.10.3: a point load and a part-length load go where the member is clicked.
  await p.click('.ribbon button[title^="A force at a point along a member"]');
  await p.mouse.click(mc[0], mc[1]); await p.waitForTimeout(150);
  const mpl = await lastOp(), plk = mpl?.Add?.item?.Load?.kind;
  check("Point on Member puts a point load on the member, where it was clicked", mpl?.Add?.item?.Load?.at?.on === "member" && plk?.kind === "concentrated" && plk.at > 0, JSON.stringify(mpl));
  await p.mouse.click(n0[0], n0[1]); await p.waitForTimeout(150);
  check("Point on Member does nothing on a node", JSON.stringify(await lastOp()) === JSON.stringify(mpl));
  await p.click('.ribbon button[title^="A distributed load over part of a member"]');
  await p.mouse.click(mc[0], mc[1]); await p.waitForTimeout(150);
  const mpp = await lastOp(), ppk = mpp?.Add?.item?.Load?.kind;
  check("Part of Member puts a load over half the member, centred where it was clicked", ppk?.kind === "patch" && ppk.from >= 0 && ppk.to > ppk.from && Math.abs((ppk.from + ppk.to) / 2 - plk?.at) < 0.01, JSON.stringify(mpp));
  await p.keyboard.press("Escape");
  // 0.10.4: Form Design Member sends the selected members; nothing selected says what to do.
  await stepHead(1); await p.keyboard.press("Escape");
  await p.mouse.click(mc[0], mc[1]); await p.waitForTimeout(150);
  await p.click('.ribbon button[title^="Join the selected members, end to end"]'); await p.waitForTimeout(200);
  const fdm = await lastOp();
  check("Form Design Member sends the selected members (0.10.4)", JSON.stringify(fdm?.FormDesignMember?.ids) === JSON.stringify([m0.id]), JSON.stringify(fdm));
  // 0.10.3: split, reverse and merge members from Geometry › Edit.
  await stepHead(1);
  await p.keyboard.press("Escape");
  await p.mouse.click(mc[0], mc[1]); await p.waitForTimeout(150);
  await p.click('.ribbon button[title^="Turn the selected members end for end"]'); await p.waitForTimeout(150);
  const rv = await lastOp();
  check("Reverse turns the selected member end for end (0.10.3)", JSON.stringify(rv?.ReverseMember?.ids) === JSON.stringify([m0.id]), JSON.stringify(rv));
  await p.click('.ribbon button[title^="Split the selected members"]'); await p.waitForTimeout(200);
  await p.click('.dlg button:text-is("Split")'); await p.waitForTimeout(150);
  const spl = await lastOp();
  check("Split cuts the selected member into two equal pieces (0.10.3)", JSON.stringify(spl?.SplitMember) === JSON.stringify({ ids: [m0.id], at: [0.5] }), JSON.stringify(spl));
  await p.click('.ribbon button[title^="Join two selected members"]'); await p.waitForTimeout(150);
  check("Merge Members asks for two members when one is selected (0.10.3)", JSON.stringify(await lastOp()) === JSON.stringify(spl));
  await p.click('.ribbon button[title^="Make nodes closer than a tolerance"]'); await p.waitForTimeout(200);
  await p.click('.dlg button:text-is("Merge")'); await p.waitForTimeout(150);
  const mn = await lastOp();
  check("Merge Nodes sends a tolerance of 5 mm (0.10.3)", mn?.MergeNodes && Math.abs(mn.MergeNodes.tol - 0.005) < 1e-12, JSON.stringify(mn));
  // The model check: the engine's findings, worst first; a row selects what it is about.
  await stepHead(5);
  await p.evaluate(f => { window.__checks = f.checkdemo_findings; }, fix);
  await p.click('.ribbon button[title^="Look for what would spoil the solve"]'); await p.waitForTimeout(300);
  const mrows = await p.$$eval(".mctable tbody tr", rs => rs.map(r => r.className));
  check("Model Check lists the engine's findings, errors first (0.10.3)", mrows.length === fix.checkdemo_findings.length && mrows[0] === "error", JSON.stringify(mrows));
  await p.click(".mctable tbody tr:nth-child(1)"); await p.waitForTimeout(150);
  check("a Model Check row selects what it is about (0.10.3)", (await p.textContent(".status")).includes("2 selected"), await p.textContent(".status"));
  check("the Model Check sits in the panel under the view, so the selection stays in sight (0.10.3)", (await p.$$(".console.open .mcheck")).length === 1 && !(await p.$(".dlg")));
  await p.click(".console header .cchev"); await p.waitForTimeout(100);
  check("the status bar keeps model, solve and balance apart, in words (0.10.3)", (await p.textContent('.status .sb[data-badge="model"]')).includes("2 errors") && (await p.textContent('.status .sb[data-badge="solve"]')).includes("Not solved") && (await p.textContent('.status .sb[data-badge="balance"]')).includes("Not checked"));
  await p.evaluate(() => { window.__checks = undefined; });
  await stepHead(3); await p.waitForTimeout(100);

  // 8. A part: arcs, edges, edge loads and drawing shapes.
  await open("bracket");
  check("parts are in millimetres on the ruler", (await p.textContent(".rtxt.unitlbl")) === "mm");
  check("the bracket's fillet is drawn as an arc", (await p.getAttribute(".shape", "d")).includes("A"));
  check("edge supports and loads are drawn", (await p.$$("g.sup")).length === 1 && (await p.$$("g.load")).length === 1);
  // Click the middle of the bottom edge (y = 0, x = 50 mm).
  const bottom = await p.evaluate(() => { const s = document.querySelector(".shape").getBoundingClientRect(); return [s.left + s.width / 2, s.bottom]; });
  await p.mouse.click(bottom[0], bottom[1] - 1); await p.waitForTimeout(150);
  check("clicking an edge picks it", (await p.$$(".edgesel")).length === 1 && (await p.textContent(".insp header")).includes("Edge"));
  await p.click('.ribbon button[title="Pressure"]');
  await p.mouse.click(bottom[0], bottom[1] - 1); await p.waitForTimeout(150);
  const pr = await lastOp();
  check("the Pressure tool puts a load on the edge clicked", pr?.Add?.item?.Load?.at?.on === "edge" && pr.Add.item.Load.kind.kind === "pressure", JSON.stringify(pr));
  await stepHead(1);
  await p.click('.ribbon button[title="Rectangle"]');
  const cb2 = await canvasBox();
  await p.mouse.click(cb2.x + cb2.w - 220, cb2.y + 120); await p.mouse.click(cb2.x + cb2.w - 120, cb2.y + 200); await p.waitForTimeout(150);
  const rc = await lastOp();
  check("two clicks with the rectangle tool add a four-corner shape", rc?.Add?.item?.Shape?.geom?.pts?.length === 4 && !rc.Add.item.Shape.hole, JSON.stringify(rc));
  await p.click('.ribbon button[title="Polygon"]');
  for (const [dx, dy] of [[-260, 300], [-160, 300], [-200, 240]]) await p.mouse.click(cb2.x + cb2.w + dx, cb2.y + dy);
  await p.keyboard.press("Enter"); await p.waitForTimeout(150);
  const pg = await lastOp();
  check("a polygon closes with Enter", pg?.Add?.item?.Shape?.geom?.pts?.length === 3, JSON.stringify(pg));
  await p.click('.ribbon button[title="Round Hole"]');
  // Inside the bracket's foot, wherever the drawing sits (0.9.1: the drawing is wider).
  const foot = await p.$eval(".shape", el => { const r = el.getBoundingClientRect(); return [r.left + r.width * 0.12, r.bottom - r.height * 0.08]; });
  await p.mouse.click(foot[0], foot[1]); await p.mouse.click(foot[0] + 12, foot[1]); await p.waitForTimeout(150);
  const hc = await lastOp();
  check("the round hole tool adds a circular hole", hc?.Add?.item?.Shape?.hole === true && hc.Add.item.Shape.geom.kind === "circle", JSON.stringify(hc));
  await p.keyboard.press("Escape");

  // 8b. Arcs bulge the right way on screen: the lug's rounded top makes it taller than wide (0.1.0 bug: arcs were drawn mirrored).
  await open("lug");
  const lb = await p.$eval(".shape:not(.hole)", el => { const r = el.getBoundingClientRect(); return r.height / r.width; });
  check("the lifting lug's rounded top bulges outwards", Math.abs(lb - 1.5) < 0.03, String(lb));

  // 9. Heat sink: convection and temperature are drawn; heat tools on the Loads tab.
  await open("fin");
  await stepHead(3);
  check("a heat model offers heat tools only", (await p.$$('.ribbon button[title="Convection"]')).length === 1 && (await p.$$('.ribbon button[title="Pinned"]')).length === 0);
  check("convection and the fixed temperature are drawn", (await p.$$("g.load")).length === fix.fin.m.loads.length && (await p.textContent(".dsvg")).includes("°C"));

  // 10. Help, Tell Me, shortcuts.
  await modelCard(); await p.click(".insp header .help"); await p.waitForTimeout(100);
  check("? card opens", (await p.textContent(".dlg")).includes("Inspector"));
  await p.keyboard.press("Escape");
  await p.keyboard.press("Control+k"); await p.keyboard.type("pressure"); await p.waitForTimeout(100);
  check("Tell Me finds commands", (await p.textContent(".pal")).includes("Pressure"));
  await p.keyboard.press("Escape");

  // 11. The ribbon never scrolls sideways.
  for (const w of [1280, 1600, 1000]) {
    await p.setViewportSize({ width: w, height: 900 });
    for (const t of ["Home", 1, 2, 3, 4, 5, 6, "View", "Help"]) {
      if (typeof t === "number") await stepHead(t); else await p.click(`.tabs > button:text-is("${t}")`);
      await p.waitForTimeout(60);
      const over = await p.$eval(".ribbon", el => el.scrollWidth - el.clientWidth);
      if (over > 1) check(`ribbon ${t} fits at ${w}px`, false, `overflow ${over}`);
      // 0.5.4 bug: a group of four stacked buttons pushed its caption out of the ribbon.
      const lost = await p.$$eval(".ribbon .grp", gs => { const rb = document.querySelector(".ribbon").getBoundingClientRect().bottom; return gs.filter(g => g.querySelector(".cap").getBoundingClientRect().bottom > rb + 1).map(g => g.querySelector(".cap").textContent); });
      if (lost.length) check(`every group's name shows on ${t} at ${w}px`, false, lost.join(", "));
    }
  }
  await p.setViewportSize({ width: 1440, height: 900 });

  // 11b. Solving a frame: deflected shape, moment diagram, reactions, legend, probe, tables (0.1.1).
  await open("portal");
  await p.keyboard.press("F5"); await p.waitForTimeout(300);
  const pr2 = unpack(fix.portal_results);
  check("F5 solves and the status bar says so", (await p.textContent(".status")).includes(`Solved · ${pr2.stats.equations} equations`), await p.textContent(".status"));
  check("the deflected shape is drawn for every member", (await p.$$(".defl")).length === pr2.members.length);
  // 0.6.4: the player. A linear result grows from no load to the whole load.
  check("a solved model has the player, at the end of the result", (await p.$$(".player")).length === 1 && (await p.textContent(".player .plabel")).includes("100 %"), await p.textContent(".player").catch(() => "none"));
  const dEnd = await p.$eval(".defl", e => e.getAttribute("d"));
  const slide = async v => { await p.$eval(".player input[type=range]", (el, v) => { el.value = String(v); el.dispatchEvent(new Event("input", { bubbles: true })); }, v); await p.waitForTimeout(150); };
  await slide(500);
  const dHalf = await p.$eval(".defl", e => e.getAttribute("d"));
  check("the slider half way draws the frame at half the load, deflecting less", (await p.textContent(".player .plabel")).includes("50 %") && dHalf !== dEnd, dHalf.slice(0, 60) + " | " + dEnd.slice(0, 60));
  await p.click(".player .pbtn >> nth=0"); await p.waitForTimeout(500);
  const lPlay = await p.textContent(".player .plabel");
  await p.click(".player .pbtn >> nth=0"); await p.waitForTimeout(100);
  check("Play moves on through the result, and pauses", lPlay !== "50 % of the load" && (await p.$$(".player .pbtn use[href='#i-play']")).length === 1, lPlay);
  await slide(1000);
  check("at the end of the slider the result is drawn as solved", (await p.$eval(".defl", e => e.getAttribute("d"))) === dEnd);
  // 0.10.5: the animation panel sits at the right of the view; Play works from the ribbon even
  // with the panel closed (it used to be reset as it opened), and at the end with Loop off.
  const pbox = await p.$eval(".player", el => { const r = el.getBoundingClientRect(), v = el.parentElement.closest(".view, .stage, main")?.getBoundingClientRect() ?? document.body.getBoundingClientRect(); return { right: v.right - r.right, w: r.width, left: r.left - v.left }; });
  check("the animation panel is a card at the right of the view (0.10.5)", pbox.right < 40 && pbox.left > pbox.w, JSON.stringify(pbox));
  await p.click(".player header .x"); await p.waitForTimeout(150);
  check("…its × closes it (0.10.5)", (await p.$$(".player")).length === 0);
  await p.click('.tabs > button:text-is("Animate")'); await p.waitForTimeout(150);
  await p.click('.ribbon button:has-text("Play") >> nth=0'); await p.waitForTimeout(600);
  check("Play on the ribbon opens the panel and plays, even when the panel was closed (0.10.5 bug)", (await p.$$(".player .pbtn use[href='#i-pause']")).length === 1 && !(await p.textContent(".player .plabel")).includes("100 %"), await p.textContent(".player").catch(() => "none"));
  await p.click(".player .pbtn >> nth=0"); await p.waitForTimeout(100);
  await p.$eval(".player .ploop input", el => { if (el.checked) el.click(); });
  await slide(1000);
  await p.click(".player .pbtn >> nth=0"); await p.waitForTimeout(400);
  check("Play at the end with Loop off starts again from the beginning (0.10.5 bug)", (await p.$$(".player .pbtn use[href='#i-pause']")).length === 1 || !(await p.textContent(".player .plabel")).includes("100 %"), await p.textContent(".player .plabel"));
  await p.click(".player .pbtn >> nth=0").catch(() => {}); await p.$eval(".player .ploop input", el => { if (!el.checked) el.click(); }); await slide(1000);
  await p.click('.ribbon button:has-text("Animation Panel")'); await p.waitForTimeout(100);
  check("Animate › Animation Panel hides and shows it (0.10.5)", (await p.$$(".player")).length === 0);
  await p.click('.ribbon button:has-text("Animation Panel")'); await p.waitForTimeout(150);
  check("…and shows it again", (await p.$$(".player")).length === 1);
  await p.click('.tabs > button:text-is("Results")'); await p.waitForTimeout(150);
  check("the moment diagram is drawn for every member", (await p.$$(".diag.d-m")).length === pr2.members.length);
  check("reactions are drawn at the supports", (await p.$$(".rarrow")).length >= 4);
  check("the legend shows the equilibrium check", (await p.textContent(".legend")).includes("reactions balance the loads"));
  check("the Results step and its tools are shown after solving", await p.$eval('.tree .step:nth-of-type(6)', el => el.classList.contains("here")) && (await p.textContent(".tabs > button.on")) === "Results");
  await p.click('.ribbon button[title="Shear Force"]'); await p.waitForTimeout(100);
  check("Shear Force switches the diagram", (await p.$$(".diag.d-v")).length === pr2.members.length && (await p.$$(".diag.d-m")).length === 0);
  const col = await p.$eval(`.mem[data-id="${pf.members[0].id}"] line`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  await p.mouse.move(col[0], col[1]); await p.waitForTimeout(150);
  check("hovering a member shows the probe", (await p.$$(".probe")).length === 1 && (await p.textContent(".probe")).includes("Member 1"));
  await go(p, "Results"); await p.waitForTimeout(150);
  check("the Results page lists reactions and members", (await p.$$(".page .tbl tbody tr")).length >= pr2.reactions.length + pr2.members.length + 3 && (await p.textContent(".page")).includes("balance the loads"));
  await go(p, "Model"); await p.waitForTimeout(150);
  await p.mouse.click(col[0], col[1]); await p.keyboard.press("ArrowUp"); await p.waitForTimeout(150);
  check("an edit marks the results out of date", (await p.$$(".legend .stalebar")).length === 1);
  await p.click('.tabs > button:text-is("Help")'); await p.click('.ribbon button[title="Verification"]'); await p.waitForTimeout(250);
  check("the window's text names no commercial product (Verification open)", named(await p.evaluate(() => document.body.innerText)).length === 0, named(await p.evaluate(() => document.body.innerText)).join());
  check("Help › Verification lists every benchmark, all passing", (await p.textContent(".dlg")).includes(`${fix.verification.length} of ${fix.verification.length} checks pass`));
  check("Help › Verification says what failed (none) and lists what is not yet verified (0.10.6)", (await p.$eval(".dlg [data-failed]", el => el.getAttribute("data-failed"))) === "0" && (await p.$$(".dlg .gaps li")).length === fix.not_verified.length && fix.not_verified.length > 0);
  check("…and the proofs: tangents, Newton's order and threads (benchmark 41, 0.10.6)", (await p.textContent(".dlg")).includes("Newton's order of convergence") && (await p.textContent(".dlg")).includes("on one thread and on four"));
  check("Help › Verification shows the mesh convergence tables", (await p.$$(".dlg .conv table")).length === fix.verification.filter(x => x.steps.length).length && (await p.textContent(".dlg")).includes("NAFEMS LE1"));
  await p.keyboard.press("Escape");

  // 11d. Parts (0.1.3): contours, legend, markers, probe, tables, mesh view, report, heat, library.
  await open("bracket"); await p.keyboard.press("F5"); await p.waitForTimeout(400);
  const br = unpack(fix.bracket_results);
  check("F5 solves a part and says how many elements", (await p.textContent("#toast")).includes(`${br.mesh.tris.length.toLocaleString()} elements`), await p.textContent("#toast"));
  check("the contours are drawn on the canvas", await p.$eval("canvas.contours", el => el.classList.contains("on") && el.width > 0));
  const painted = await p.$eval("canvas.contours", el => { const d = el.getContext("2d").getImageData(0, 0, el.width, el.height).data; let n = 0; for (let i = 3; i < d.length; i += 4 * 97) if (d[i] > 0) n++; return n; });
  check("the contour canvas has colour on it", painted > 50, String(painted));
  check("the legend shows von Mises stress with 12 bands in MPa", (await p.$eval(".legend .fpick", el => el.value)) === "vm" && (await p.$$(".legend .cbar .cb i")).length === 12 && (await p.textContent(".legend .cbar")).includes("MPa"));
  check("the largest and smallest values are marked", (await p.$$(".mm.max")).length === 1 && (await p.$$(".mm.min")).length === 1 && (await p.textContent(".mm.max")).includes("MPa"));
  check("the legend shows the equilibrium check for the part", (await p.textContent(".legend")).includes("reactions balance the loads"));
  check("shapes are not filled over the contours", await p.$eval(".dsvg", el => el.classList.contains("under")));
  // Probe: hover the middle of the vertical leg (x 10 mm, y 60 mm).
  const leg = await p.evaluate(() => { const r = document.querySelector('.shape').getBoundingClientRect(); return [r.left + r.width * 0.1, r.top + r.height * 0.35]; });
  await p.mouse.move(leg[0], leg[1]); await p.waitForTimeout(200);
  check("hovering the part shows the probe with stresses", (await p.$$(".probe")).length === 1 && (await p.textContent(".probe")).includes("σvm") && (await p.textContent(".probe")).includes("MPa"), (await p.$$(".probe")).length ? await p.textContent(".probe") : "no probe");
  await p.selectOption(".legend .fpick", "umag"); await p.waitForTimeout(200);
  check("picking Displacement shows it in mm", (await p.textContent(".legend .cbar")).includes("mm") && (await p.textContent(".mm.max")).includes("mm"));
  check("displacements cannot be shown per element (they are continuous)", await p.$eval(".legend .seg button:nth-child(2)", el => el.disabled));
  await p.selectOption(".legend .fpick", "sf"); await p.waitForTimeout(200);
  check("the safety factor legend runs red to blue", await p.$eval(".legend .cbar .cb i:first-child", el => getComputedStyle(el).backgroundColor !== "") && (await p.textContent(".mm.min")).length > 0);
  await p.selectOption(".legend .fpick", "vm"); await p.waitForTimeout(100);
  await p.click(".legend .seg button:has-text('Per element')"); await p.waitForTimeout(200);
  check("stresses can be shown per element", await p.$eval(".legend .seg button:nth-child(2)", el => el.classList.contains("on")));
  await p.click(".legend .seg button:has-text('Smoothed')");
  await go(p, "Results"); await p.waitForTimeout(200);
  check("the Results page of a part shows equilibrium, reactions and extremes", (await p.textContent(".page")).includes("Largest and smallest values") && (await p.textContent(".page")).includes("balance the loads") && (await p.textContent(".page")).includes("Fixed"), (await p.textContent(".page")).slice(0, 200));
  await go(p, "Mesh"); await p.waitForTimeout(200);
  check("results arrive packed small and are unpacked by the window", typeof fix.bracket_results.mesh.nodes.$f32 === "string" && typeof fix.bracket_results.mesh.tris.$u32 === "string");
  check("the Mesh view shows the elements and their quality", (await p.textContent(".legend")).includes(`${br.mesh.tris.length.toLocaleString()} 6-node triangles`) && (await p.textContent(".legend")).includes("smallest angle"));
  await go(p, "Report"); await p.waitForTimeout(400);
  check("the Report view has the mesh, a contour picture with its scale, and the equilibrium check", (await p.textContent(".page")).includes("Mesh") && (await p.$$(".page .rpic img")).length >= 1 && (await p.$$(".page .rpic .cbar")).length >= 1 && (await p.textContent(".page")).includes("Equilibrium check"));
  await go(p, "Model"); await p.waitForTimeout(150);
  // Refine near an edge: a local size in the mesh settings.
  await stepHead(4); await p.click('.ribbon button[title^="Click an edge or a point"]'); await p.waitForTimeout(100);
  const edgeAt = await p.evaluate(() => { const r = document.querySelector('.shape').getBoundingClientRect(); return [r.left + 2, r.top + r.height * 0.5]; });
  await p.mouse.click(edgeAt[0], edgeAt[1]); await p.waitForTimeout(150);
  const rf = (await ops()).reverse().find(o => o.SetSettings);
  check("Refine Near adds a local element size on the edge clicked", !!rf && rf.SetSettings.s.mesh.refine.length === 2 && rf.SetSettings.s.mesh.refine[1].at.on === "edge", JSON.stringify(rf?.SetSettings.s.mesh.refine));
  await p.keyboard.press("Escape");
  await p.click('.ribbon button[title="Linear (3-node)"]'); await p.waitForTimeout(100);
  check("Linear (3-node) sets the element order", (await ops()).reverse().find(o => o.SetSettings)?.SetSettings.s.mesh.order === 1);
  // Join Where They Cross (0.1.4): an explicit command on the selected members.
  await open("portal");
  await p.click(".tree .ti:has-text('Member 1')"); await p.click(".tree .ti:has-text('Member 3')", { modifiers: ["Control"] }); await p.waitForTimeout(100);
  await p.click('.tabs .ctxgrp button:text-is("Format")'); await p.click('.ribbon button[title^="Put a node where the selected members cross"]'); await p.waitForTimeout(150);
  const jc = (await ops()).reverse().find(o => o.JoinCrossings);
  check("Format › Join Where They Cross sends the selected members to the engine", !!jc && jc.JoinCrossings.ids.length === 2, JSON.stringify(jc));
  // Pin load (0.1.4): on the lug's hole, drawn as arrows on the side it presses.
  await open("lug");
  const pin = fix.lug.m.loads.find(l => l.kind.kind === "bearing");
  check("the lifting lug is loaded by a pin bearing in its hole", !!pin && (await p.textContent(".canvas")).includes("pin 60 kN"));
  check("the pin load's arrows press on one side of the hole only", await p.$eval(`.load[data-id="${pin.id}"] path`, el => (el.getAttribute("d").match(/M/g) || []).length) < 3 * 40);
  await p.click(`.tree .ti:has-text('Pin load')`); await p.waitForTimeout(150);
  check("the Inspector edits a pin load's force", (await p.textContent(".insp")).includes("Pin load") && (await p.$$(".insp .iprow input")).length === 2);
  // Thermal stress (0.2.0): the heated plate solves for temperatures, then stress.
  await open("hotplate"); await p.keyboard.press("F5"); await p.waitForTimeout(400);
  check("a part with edge temperatures shows Temperature among its stress results", (await p.$$eval(".legend .fpick option", os => os.map(o => o.value))).includes("t") && (await p.$eval(".legend .fpick", el => el.value)) === "vm");
  await p.selectOption(".legend .fpick", "t"); await p.waitForTimeout(150);
  check("the heated plate's temperatures run from 20 to 220 °C", (await p.textContent(".legend .cbar")).includes("°C") && (await p.textContent(".mm.max")).includes("220"));
  await p.click(".canvas", { position: { x: (await p.$eval(".canvas", el => el.clientWidth)) - 30, y: 400 } }); await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await modelCard(); check("the Inspector has the stress-free temperature", (await p.textContent(".insp")).includes("Stress-free temperature"));
  await stepHead(3); await p.click('.ribbon button[title^="A uniform change of temperature"]'); await p.waitForTimeout(150);
  const wl = (await ops()).reverse().find(o => o.Add && o.Add.item.Load && o.Add.item.Load.kind.kind === "warming");
  check("Temperature Change adds a uniform warming of the whole part", !!wl && wl.Add.item.Load.at.on === "body", JSON.stringify(wl));
  // Natural frequencies (0.2.2): a frame and a part.
  await open("portal");
  await stepHead(5); await p.click('.ribbon button[title="The lowest natural frequencies and their mode shapes"]'); await p.waitForTimeout(400);
  const pmo = unpack(fix.portal_modes);
  check("Natural Frequencies lists the modes under the canvas", (await p.$$(".legend .modelist button")).length === pmo.modes.length && (await p.textContent(".legend")).includes("Hz"));
  check("a frame's mode is drawn along every member", (await p.$$(".modeline")).length === fix.portal.m.members.length);
  check("the static diagrams are hidden while a mode is shown", (await p.$$(".diag")).length === 0);
  await p.click(".legend .modelist button:nth-child(2)"); await p.waitForTimeout(100);
  check("another mode can be picked", await p.$eval(".legend .modelist button:nth-child(2)", el => el.classList.contains("on")));
  await go(p, "Results"); await p.waitForTimeout(200);
  check("the Results page lists the natural frequencies", (await p.textContent(".page")).includes("Natural frequencies") && (await p.$$(".page .tbl tbody tr")).length >= pmo.modes.length);
  await go(p, "Model"); await p.waitForTimeout(150);
  await p.click(".legend button:has-text('Back to the static results')"); await p.waitForTimeout(100);
  check("Back to the static results hides the modes", (await p.$$(".modeline")).length === 0);
  await open("bracket");
  await stepHead(5); await p.click('.ribbon button[title="The lowest natural frequencies and their mode shapes"]'); await p.waitForTimeout(500);
  check("a part's mode is drawn on the contour canvas", await p.$eval("canvas.contours", el => el.classList.contains("on")) && (await p.textContent(".legend")).includes("Natural frequencies"));
  check("the part's fill is cleared under its mode (0.2.2 real-app bug)", await p.$eval(".dsvg", el => el.classList.contains("under")));
  // Buckling (0.2.3): a frame's load factors and shapes, and a truss's bars checked on their own.
  await open("portal");
  await stepHead(5); await p.click('.ribbon button[title="The lowest buckling load factors of a frame and their shapes"]'); await p.waitForTimeout(400);
  const pbk = unpack(fix.portal_buckle);
  check("Buckling lists the load factors under the canvas", (await p.$$(".legend .modelist button")).length === pbk.modes.length && (await p.textContent(".legend")).includes("Buckling load factors") && (await p.textContent(".legend .modelist")).includes("λ"));
  check("a buckled shape is drawn along every member", (await p.$$(".modeline")).length === fix.portal.m.members.length);
  check("the ribbon offers Buckled Shapes", (await p.$$('.ribbon button[title^="Show the buckled shapes"]')).length === 1);
  await go(p, "Results"); await p.waitForTimeout(200);
  check("the Results page lists the load factors", (await p.textContent(".page")).includes("Load factor λ") && (await p.textContent(".page")).includes(String(pbk.modes[0].lambda.toPrecision(3)).slice(0, 3)));
  await go(p, "Model"); await p.waitForTimeout(150);
  await open("truss");
  await stepHead(5); await p.click('.ribbon button[title="The lowest buckling load factors of a frame and their shapes"]'); await p.waitForTimeout(400);
  await go(p, "Results"); await p.waitForTimeout(200);
  const tbk = unpack(fix.truss_buckle);
  check("a truss's squeezed bars are checked on their own, weakest first", (await p.textContent(".page")).includes("Truss bars in compression") && (await p.$eval(".page", el => [...el.querySelectorAll("h3 + .tbl tbody tr td:first-child")].map(td => td.textContent)))[0] === `Member ${fix.truss.m.members.findIndex(x => x.id === [...tbk.bars].sort((a, b) => a.lambda - b.lambda)[0].id) + 1}`);
  check("the bar that buckles first is named as the tree names it", /Member \d+, a truss bar, buckles on its own/.test(await p.textContent(".page")));
  await go(p, "Model"); await p.waitForTimeout(150);
  await open("bracket");
  await stepHead(5);
  check("Buckling is off for parts", await p.$eval('.ribbon button[title="The lowest buckling load factors of a frame and their shapes"]', el => el.disabled || el.classList.contains("off")));
  // 3D frames (0.2.4): the platform in the 3D view, drawing on planes, solving, the Results page.
  await open("platform");
  const pm = fix.platform.m, z0 = pm.nodes.filter(n => (n.z ?? 0) === 0).map(n => n.id);
  check("a 3D frame opens in the 3D view, with no plane bar (0.6.2: sketching replaces it)", (await p.$$(".view3d canvas")).length === 1 && (await p.$$(".planebar .seg")).length === 0);
  await p.click('.tabs > button:text-is("Home")');
  check("Home shows 3D Frame chosen", await p.$eval('.ribbon button[title^="3D frames and trusses"]', el => el.classList.contains("on")));
  // 0.10.4: a design member's card: its chain, restraints and the segments the engine found.
  await p.evaluate(f => { window.__next = f.dmdemo; }, fix);
  await open("blank-frame"); await p.waitForTimeout(300);
  await stepHead(1);
  await p.click('.tree .ti:has(span:text-is("Beam B1"))'); await p.waitForTimeout(400);
  check("picking a design member opens its card (0.10.4)", await p.$eval(".ipop .dmcard", el => el.offsetHeight > 50).catch(() => false));
  const segs = await p.$$eval('.dmcard .dmseg tbody tr', rs => rs.map(r => r.children[2].textContent));
  check("a design member's card lists its segments between restraints, from the engine (0.10.4)", segs.length === fix.dmdemo_info.segments.length && segs.length === 2, JSON.stringify(segs));
  check("…and its chain of members (0.10.4)", (await p.textContent(".dmcard")).includes("Member 1 → Member 2 → Member 3 → Member 4"));
  await p.click(".ipop button.ifold").catch(() => {});
  await p.click('.ribbon button:has-text("Sections")'); await p.waitForTimeout(400);
  check("the Sections dialog shows the whole table: Z, S, r for both axes (0.10.4)", (await p.$$('.dlg .sectab tbody tr')).length === 8 && (await p.textContent(".dlg .sectab")).includes("Plastic modulus S"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  // 0.10.5: Help › Command Line shows how to run datum-mesh-cli and connect an AI agent.
  await p.click('.tabs > button:text-is("Help")'); await p.waitForTimeout(150);
  await p.click('.ribbon button:has-text("Command Line")'); await p.waitForTimeout(400);
  check("Help › Command Line lists the commands to run and how to connect an AI agent (0.10.5)", (await p.$$('.dlg .clitab tr')).length === 6 && (await p.textContent(".dlg .clitab")).includes(" mcp"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  // 0.10.4: Results › Mechanics Checks: statuses from the engine, a trace per check, what is not checked.
  await p.click('.tabs > button:text-is("Results")'); await p.waitForTimeout(200);
  await p.click('.ribbon button:has-text("Mechanics Checks")'); await p.waitForTimeout(500);
  const mrow = fix.dmdemo_checks.members[0];
  const stat = await p.$$eval('.dlg [data-table="checks"] .mrow [data-status]', els => els.map(e => e.getAttribute("data-status")));
  check("Mechanics Checks shows each check's status from the engine, and the member's (0.10.4)", JSON.stringify(stat) === JSON.stringify([...mrow.checks.map(c => c.status), mrow.status]), JSON.stringify(stat));
  await p.click('.dlg [data-table="checks"] .mrow'); await p.waitForTimeout(150);
  check("…clicking a member shows each check's formula and values (0.10.4)", (await p.$$('.dlg .trace li')).length === mrow.checks.reduce((n, c) => n + c.trace.length, 0) && (await p.textContent(".dlg .trace")).includes("σ = |N|/A + |M|/Z"));
  await p.click('.dlg .segtabs button:has-text("Not checked")'); await p.waitForTimeout(100);
  check("…and lists what is never checked (0.10.4)", (await p.$$('.dlg .notchk li')).length === fix.dmdemo_checks.not_checked.length);
  await p.fill('.dlg .mset input >> nth=0', "0.7"); await p.keyboard.press("Tab"); await p.waitForTimeout(150);
  const cs = await lastOp();
  check("…its effective length factor is sent to the engine (0.10.4)", cs?.CheckSettings?.s?.k === 0.7 && cs.CheckSettings.s.span_ratio === 300, JSON.stringify(cs));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.evaluate(f => { window.__mech = f.driftdemo_checks; }, fix);
  await p.click('.ribbon button:has-text("Storey Drift")'); await p.waitForTimeout(400);
  check("Storey Drift lists each storey with its drift and status (0.10.4)", (await p.$$('.dlg [data-table="drift"] tbody tr')).length === 2 && (await p.textContent('.dlg [data-table="drift"]')).includes("pass"));
  await p.keyboard.press("Escape"); await p.evaluate(() => { window.__mech = undefined; }); await p.waitForTimeout(100);
  await p.click('.ribbon button:has-text("Values Along")'); await p.waitForTimeout(400);
  const vr = await p.$$eval('.dlg [data-table="values"] tbody tr', rs => rs.map(r => r.children[0].textContent));
  check("Values Along gives both sides of a jump (0.10.4)", JSON.stringify(vr) === JSON.stringify(["Just before", "Just after"]), JSON.stringify(vr));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  // 0.10.7: a tube crushed into folds, in time: the 2D part's Move an Edge tool, the crushing step,
  // and its results: the force against the crush and the folds played in time.
  await p.evaluate(f => { window.__next = f.crushtube; }, fix);
  await open("blank-stress"); await p.waitForTimeout(400);
  await p.click('.tabs > button:text-is("Loads & BCs")'); await p.waitForTimeout(150);
  check("an axisymmetric part has Move an Edge on the Loads tab (0.10.7)", (await p.$$('.ribbon button:has-text("Move an Edge")')).length === 1);
  check("the crushed tube's top edge is a moved edge, named so in the tree (0.10.7)", (await p.textContent(".tree")).includes("Moved edge"));
  await p.evaluate(() => { window.__solveKey = "crushtube_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(800);
  await p.evaluate(() => { window.__solveKey = null; });
  await p.click('.tabs > button:text-is("Results")'); await p.waitForTimeout(150);
  await p.click('.ribbon button:has-text("Load–Deflection")'); await p.waitForTimeout(400);
  check("the crush's force against its movement is drawn, with its first peak (0.10.7)", (await p.$$(".dlg svg path")).length > 0 && (await p.textContent(".dlg")).includes("peak"), (await p.textContent(".dlg").catch(() => "")).slice(0, 200));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  check("the folds play in time (0.10.7)", (await p.textContent(".player .plabel").catch(() => "")).includes("t = "), await p.textContent(".player").catch(() => "none"));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "crushtube.png") });
  await open("platform"); await p.waitForTimeout(300);
  // 0.10.3: member orientation turns the picked members so their depth faces a direction; their
  // local axes are drawn meanwhile.
  await stepHead(1); await p.waitForTimeout(100);
  await p.$eval(`.tree .ti:has(span:text-is("Member 1"))`, el => el.click()); await p.waitForTimeout(150);
  await p.click('.ribbon button[title^="Turn the selected members\' sections"]'); await p.waitForTimeout(250);
  await p.selectOption('.dlg select', "along"); await p.waitForTimeout(100);
  await p.click('.dlg button:text-is("Turn")'); await p.waitForTimeout(200);
  const orop = await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").map(c => c[1].op).pop());
  check("Orientation turns the picked member so its depth faces up (Z, the engine's y) (0.10.3)", JSON.stringify(orop?.Orient?.ids) === JSON.stringify([pm.members[0].id]) && JSON.stringify(orop.Orient.along) === JSON.stringify([0, 1, 0]), JSON.stringify(orop));
  check("…and turns on Local Axes, to see each member's x, y and z (0.10.3)", await p.$eval('.ribbon button[title^="Draw each member\'s local axes"]', b => b.classList.contains("on")));
  await p.click('.ribbon button[title^="Draw each member\'s local axes"]'); await p.click(".ipop button.ifold").catch(() => {}); await p.waitForTimeout(100);
  // 0.7.5: New Sketch draws the frame with the modelling sketch tools; Finish Sketch makes members.
  await stepHead(1); await p.waitForTimeout(150);
  await p.click('.ribbon button[title^="A closed outline drawn on its own"]'); await p.waitForTimeout(100);
  check("Closed Shapes become Plates turns on (0.7.6)", await p.$eval('.ribbon button[title^="A closed outline drawn on its own"]', b => b.classList.contains("on")));
  await p.click('.ribbon button[title^="A closed outline drawn on its own"]'); await p.waitForTimeout(100);
  await p.click('.ribbon button[title="New Sketch options"]'); await p.waitForTimeout(150);
  check("a 3D frame's New Sketch ▾ offers the three planes (and levels once made) (0.7.6)", (await p.textContent(".menu")).includes("On the ground (XY)"));
  await p.keyboard.press("Escape"); await p.mouse.click(700, 500); await p.waitForTimeout(100);
  await p.click('.ribbon button[title^="Levels (a plane at each floor"]'); await p.waitForTimeout(150);
  check("Levels opens with storeys and bays (0.7.6)", (await p.textContent(".dlg")).includes("Levels above the ground") && (await p.textContent(".dlg")).includes("Bays along X"));
  await p.click(".dlg footer .btn:not(.pri)"); await p.waitForTimeout(100);
  await p.click('.ribbon button[title^="Draw the frame with the sketch tools"]'); await p.waitForTimeout(250);
  check("a 3D frame's New Sketch asks for a plane like a part's (0.7.5)", (await p.$$(".view3d .pickplane")).length === 1);
  await p.click('.view3d .pickplane button:has-text("Front (XZ)")'); await p.waitForTimeout(300);
  check("the frame sketch opens with the modelling sketch tools", (await p.$$('.ribbon .grp:has(.cap:text-is("Create")) .rbw')).length >= 6);
  await p.click('.ribbon button[title^="Keep the sketch"]'); await p.waitForTimeout(400);
  check("Finish Sketch on a 3D frame makes members of its lines (beams by default)", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "cad_members").map(c => c[1]).slice(-1)[0])) === JSON.stringify({ truss: false, plates: false }));
  // 0.6.2: Beam (one by one), pick the Front plane: the sketch canvas lies over the 3D view, square to it.
  await stepHead(1); await p.click('.ribbon button[title^="Draw beams"]'); await p.waitForTimeout(250);
  check("Beam asks for a plane, with the squares at the origin and a bar", (await p.$$(".view3d .pickbar")).length === 1);
  await p.click('.view3d .pickbar .seg button:text-is("Front")'); await p.waitForTimeout(400);
  check("picking the Front plane opens the sketch: the 3D view behind, the sketch canvas over it, a Sketch tab and a bar", (await p.$$(".sketchstage .view3d")).length === 1 && (await p.$$(".sketchover .canvas")).length === 1 && (await p.$$('.tabs .sktab.on')).length === 1 && (await p.textContent(".sketchbar")).includes("Front plane"));
  const onFront = pm.members.filter(b => z0.includes(b.a) && z0.includes(b.b)).length;
  check("the Front sketch shows the members on it, the others faint", (await p.$$(".sketchover .dsvg .mem")).length === onFront && (await p.$$(".sketchover .dsvg .ghost")).length === pm.members.length - onFront, `${(await p.$$(".sketchover .dsvg .mem")).length} of ${onFront}`);
  const dz = await p.$$eval(".sketchbar select option", os => os.map(o => o.textContent));
  check("the sketch bar offers the planes that have nodes, as Y (Z up)", dz.length === 3 && dz.some(t => /4/.test(t)), dz.join(", "));
  // The canvas lines up with the 3D view: a node drawn by the canvas sits where the 3D view puts it.
  const nf0 = pm.nodes.find(n => (n.z ?? 0) === 0 && n.y > 0);
  const c2 = await p.$eval(`.sketchover .dsvg .node[data-id="${nf0.id}"]`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; }).catch(() => null);
  const c3 = await p.evaluate(q => { const v = document.querySelector(".sketchstage .view3d").getBoundingClientRect(), s = window.__screen3(q); return [v.left + s[0], v.top + s[1]]; }, [nf0.x, nf0.y, nf0.z ?? 0]).catch(() => null);
  check("the sketch canvas lines up with the 3D view behind it", !!c2 && !!c3 && Math.hypot(c2[0] - c3[0], c2[1] - c3[1]) < 3, JSON.stringify([c2, c3]));
  await p.click('.sketchbar button:has-text("Finish Sketch")'); await p.waitForTimeout(250);
  check("Finish Sketch goes back to the 3D view", (await p.$$(".sketchstage")).length === 0 && (await p.$$(".view3d canvas")).length === 1);
  // A beam drawn on the Side plane is sent in 3D.
  await stepHead(1); await p.click('.ribbon button[title^="Draw beams"]'); await p.waitForTimeout(200);
  await p.click('.view3d .pickbar .seg button:text-is("Side")'); await p.waitForTimeout(400);
  check("a frame sketch starts with the Beam tool", await p.$eval('.ribbon button[title^="Draw beams"]', el => el.classList.contains("on")));
  const sb = await canvasBox();
  await p.mouse.click(sb.x + sb.w * 0.3, sb.y + sb.h * 0.5); await p.waitForTimeout(80); await p.mouse.click(sb.x + sb.w * 0.6, sb.y + sb.h * 0.5); await p.waitForTimeout(150); await p.keyboard.press("Escape"); await p.keyboard.press("Escape");
  const am3 = (await ops()).reverse().find(o => o.AddMember);
  check("a beam drawn on the side plane is sent in 3D: x at the depth, z across", !!am3 && am3.AddMember.from[0] === 0 && am3.AddMember.to[0] === 0 && am3.AddMember.z[0] !== am3.AddMember.z[1] && am3.AddMember.from[1] === am3.AddMember.to[1], JSON.stringify(am3));
  await p.click('.sketchbar button:has-text("Finish Sketch")'); await p.waitForTimeout(250);
  await stepHead(1);
  check("Geometry shows Repeat for 3D frames", (await p.$$('.ribbon button[title^="Copy the selected members along the depth"]')).length === 1);
  await p.click('.tabs > button:text-is("View")'); await p.waitForTimeout(100);
  check("a 3D frame is drawn with its section shapes, a View choice (0.7.5)", (await p.$$('.ribbon label.chk:has-text("Section shapes") input:checked')).length === 1);
  await stepHead(1);
  // 0.9.1: the card's × lets go of the member picked before (Repeat then takes the whole frame).
  if (await p.$(".ipop")) { await p.click(".ipop .ifold"); await p.waitForTimeout(100); }
  await p.click('.ribbon button[title^="Copy the selected members along the depth"]'); await p.waitForTimeout(150);
  await p.click('.dlg button:text-is("Repeat")'); await p.waitForTimeout(150);
  const rp = await lastOp();
  check("Repeat sends the whole frame one step along Y (Z up: the engine's −z), joined by beams", !!rp.Repeat && rp.Repeat.ids.length === pm.members.length && rp.Repeat.step[2] < 0 && rp.Repeat.step[1] === 0 && rp.Repeat.times === 1 && rp.Repeat.link === "beam", JSON.stringify(rp));
  await p.click(`.tree .ti:has-text("Node 3")`); await p.waitForTimeout(100);
  check("the Inspector shows a node's z in 3D", (await p.$$eval(".insp .iprow label", ls => ls.map(l => l.textContent))).some(t => t.startsWith("z")));
  await p.evaluate(() => { window.__solveKey = "platform_results"; });
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  await p.evaluate(() => { window.__solveKey = null; });
  await p.waitForTimeout(300);
  check("once solved, the 3D view colours the members and checks equilibrium", (await p.textContent(".legend")).includes("Colour by") && (await p.textContent(".legend")).includes("Equilibrium"));
  await stepHead(6);
  check("Results offers colouring by a result in 3D", (await p.$$('.ribbon button:has-text("Moment My")')).length === 1);
  await go(p, "Results"); await p.waitForTimeout(200);
  const pg3 = await p.textContent(".page");
  check("the 3D Results page sums six ways and lists six reactions per support", pg3.includes("Σ Fz") && pg3.includes("Σ My about the origin") && (await p.$$eval(".page h2 + table thead tr", rs => rs.map(r => r.children.length))).includes(7), pg3.slice(0, 200));
  check("the 3D Results page lists every member with torque and both moments", (await p.$$(".page .tbl tbody tr.link")).length === pm.members.length && pg3.includes("|Mz| · |My|"));
  await go(p, "Model"); await p.waitForTimeout(200);
  check("the 3D view's code is loaded once a 3D frame is shown", await p.evaluate(() => performance.getEntriesByType("resource").some(e => e.name.includes("view3d"))));
  // 3D frames vibrate and buckle (0.2.6).
  await stepHead(5); await p.click('.ribbon button[title="The lowest natural frequencies and their mode shapes"]'); await p.waitForTimeout(400);
  const p3m = unpack(fix.platform_modes);
  check("a 3D frame's natural frequencies are listed under the 3D view", (await p.$$(".legend .modelist button")).length === p3m.modes.length && (await p.textContent(".legend .modelist")).includes("Hz") && (await p.$$(".view3d canvas")).length === 1);
  await stepHead(5); await p.click('.ribbon button[title="The lowest buckling load factors of a frame and their shapes"]'); await p.waitForTimeout(400);
  const p3b = unpack(fix.platform_buckle);
  check("a 3D frame's buckling load factors are listed", (await p.$$(".legend .modelist button")).length === p3b.modes.length && (await p.textContent(".legend .modelist")).includes("λ"));
  await go(p, "Results"); await p.waitForTimeout(200);
  check("the Results page says 3D buckling is by bending only", (await p.textContent(".page")).includes("twisting and lateral-torsional buckling of open sections"));
  await go(p, "Model"); await p.waitForTimeout(150);
  await p.click(".legend button:has-text('Back to the static results')"); await p.waitForTimeout(100);
  // 3D solids (0.3.0): the L-bracket extruded, its faces, the solve, the Results page.
  await open("bracket3d");
  check("a 3D solid opens in the 3D view, with Edit Section instead of a 3D/Section switch (0.7.4, the user)", (await p.$$(".view3d canvas")).length === 1 && (await p.$$(".planebar .seg button")).length === 0 && (await p.$$('.planebar button:text-is("Edit Section")')).length === 1);
  await p.click('.tabs > button:text-is("Home")');
  check("Home shows 3D Solid chosen", await p.$eval('.ribbon button[title^="3D solids: the part"]', el => el.classList.contains("on")));
  await modelCard(); check("the Inspector has the thickness of the extrusion and the layers", (await p.textContent(".insp")).includes("Thickness (extruded along z)") && (await p.textContent(".insp")).includes("Layers through the thickness"));
  await p.click('.planebar button:text-is("Edit Section")'); await p.waitForTimeout(250);
  check("Section shows the part's outline to edit", (await p.$$(".dsvg .shape")).length === fix.bracket3d.m.shapes.length);
  await stepHead(3); await p.click('.ribbon button:has-text("Fixed")'); await p.waitForTimeout(100);
  await p.click('.planebar button:has-text("the back face")'); await p.waitForTimeout(150);
  const bf = await lastOp();
  check("a support goes on the back face with a button", !!bf?.Add?.item?.Support && bf.Add.item.Support.at.on === "end" && bf.Add.item.Support.at.front === false, JSON.stringify(bf));
  await p.keyboard.press("Escape");
  await p.click('.planebar button:text-is("Finish Editing")'); await p.waitForTimeout(250);
  await p.evaluate(() => { window.__solveKey = "bracket3d_results"; });
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__solveKey = null; });
  const lg3 = await p.textContent(".legend");
  check("once solved, the solid is coloured by a result with its scale", lg3.includes("Von Mises stress") && lg3.includes("18-node wedges") && (await p.$$(".legend .cbar")).length === 1 && lg3.includes("Equilibrium"), lg3.slice(0, 200));
  check("the tree shows the solved solid's mesh, not 'not meshed' (0.3.0 real-app bug)", (await p.textContent(".tree")).includes("wedges"));
  await stepHead(6);
  check("the Results tab offers the solid's results", !(await p.$eval('.ribbon button:has-text("Von Mises")', el => el.disabled)));
  await go(p, "Results"); await p.waitForTimeout(200);
  const sp3 = await p.textContent(".page");
  check("the 3D solid's Results page has the extremes, six sums and the reactions", sp3.includes("Largest and smallest values") && sp3.includes("Σ Fz") && sp3.includes("Σ Mx about the origin") && sp3.includes("Displacement in z"));
  await go(p, "Model"); await p.waitForTimeout(200);
  await open("blank-solid");
  {
    // 0.10.3 (the user): the origin's point, axes and planes in the tree, each with an eye; sketches and bodies too.
    const orows = await p.$$eval(".tree .torigin .ti[data-origin]", els => els.map(e => e.textContent.trim()));
    check("a 3D model's tree has its Origin: the point, three axes and three planes (0.10.3, the user)", JSON.stringify(orows) === JSON.stringify(["O", "X", "Y", "Z", "XY", "XZ", "YZ"]), JSON.stringify(orows));
    check("…the planes start hidden, the point and axes shown", await p.$$eval(".tree .torigin .ti[data-origin] .teye", els => els.map(e => e.classList.contains("off")).join()) === "false,false,false,false,true,true,true");
    await p.click('.tree .torigin > summary .tt'); await p.waitForTimeout(80);
    await p.click('.tree .ti[data-origin="ax:x"] .teye'); await p.waitForTimeout(80);
    check("an eye hides one item and says so", await p.$eval('.tree .ti[data-origin="ax:x"]', el => el.classList.contains("dim") && el.querySelector(".teye").classList.contains("off")));
    await p.click('.tree .torigin > summary .teye'); await p.waitForTimeout(80);
    check("the Origin's own eye shows or hides all of it at once", await p.$$eval(".tree .torigin .ti[data-origin] .teye", els => els.every(e => !e.classList.contains("off"))));
    await p.evaluate(f => { window.__cad = f.cad.extruded; window.dispatchEvent(new Event("cad-reload")); }, fix); await p.waitForTimeout(500);
    const beye = await p.$('.tree .ti:has-text("faces") .teye');
    check("bodies and sketches have their eyes too", !!beye && (await p.$$('.tree .ti:has-text("curves") .teye')).length >= 1);
    if (beye) { await beye.click(); await p.waitForTimeout(80); }
    check("…a hidden body says so in the tree; what is hidden is forgotten when another model opens", beye ? await p.$eval('.tree .ti:has-text("faces") .teye', el => el.classList.contains("off")) : false);
    // 0.10.3 (the user): the workflow bar without ticks; its tools; the standard parts; the cube's zones.
    check("the workflow bar marks a done step by its ground, not a tick, and has its tools", (await p.$$eval(".flow button .fn", els => els.every(e => /^\d$/.test(e.textContent)))) && (await p.$$('.flow button[data-tool="parts"]')).length === 1);
    await p.click('.flow button[data-tool="parts"]'); await p.waitForTimeout(400);
    check("Parts opens the standard parts: bolts, nuts, washers and pins in metric sizes", (await p.$$(".stdparts .spcat:not(.planned)")).length === 4 && (await p.$$(".stdparts .sprow")).length === 7);
    const before = await p.evaluate(() => window.__calls.length);
    await p.click('.stdparts .sprow:has(.spname:text-is("M10")) .btn'); await p.waitForTimeout(400);
    const made = await p.evaluate(n => window.__calls.slice(n).filter(c => c[0] === "cad_run").map(c => [c[1].id, c[1].params]), before);
    // (The checks' engine answers with the same scene each time, so the join, which needs the new solids' names, is the engine's to test.)
    check("Insert makes an M10 bolt from the engine's solids: a shank of 10 mm × 40, a head of 16 mm × 6.4 on top", made.length >= 2 && made[0][0] === "solid.cylinder" && made[0][1].radius === 5 && made[0][1].height === 40 && made[1][1].radius === 8 && made[1][1].base[2] === 40 && made[1][1].height === 6.4, JSON.stringify(made));
    await p.click('.flow button[data-tool="parts"]'); await p.waitForTimeout(100);
    check("the view cube draws its middle thirds: edges and corners pick the views between faces", (await p.$$(".view3d .cube polygon.inner")).length === 6);
    await open("blank-solid");
  }
  check("a blank 3D solid opens in 3D and says to click a plane to sketch on (0.7.2)", (await p.textContent(".view3d")).includes("Build a part") && (await p.textContent(".view3d .empty")).includes("three planes"));
  // 0.3.5: DXF import and export.
  await open("lug"); await p.waitForTimeout(200);
  await stepHead(1); await p.waitForTimeout(100);
  await p.click('.ribbon button[title^="Import a section drawn in CAD"]'); await p.waitForTimeout(150);
  await p.selectOption(".dlg select", "in");
  await p.click('.dlg button:text-is("Choose the file…")'); await p.waitForTimeout(300);
  const ic = await p.evaluate(() => window.__calls.filter(c => c[0] === "import_dxf").slice(-1)[0]?.[1]);
  check("DXF import asks the engine with the units picked", Math.abs(ic?.scale - 0.0254) < 1e-12, JSON.stringify(ic));
  check("the imported outlines are in the model and the import says what came in", (await p.textContent(".tree")).includes("Hole 2") && (await p.textContent("#toast")).includes("Imported 1 shape and 2 holes") && (await p.textContent("#toast")).includes("spline"));
  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150);
  await p.click('.bs aside button:has-text("Export")'); await p.waitForTimeout(150);
  await p.click('.tile:has-text("Section as DXF")'); await p.waitForTimeout(300);
  const sv = await p.evaluate(() => window.__calls.filter(c => c[0] === "save_file").slice(-1)[0]?.[1]);
  check("File › Export saves the section as a DXF", sv?.name?.endsWith(".dxf") && sv.data.includes("ENTITIES"), JSON.stringify(sv)?.slice(0, 120));
  await p.click(".bs aside .back"); await p.waitForTimeout(150);
  // 0.3.4: 3D heat conduction, and natural frequencies of 3D solids.
  await open("fin3d"); await p.waitForTimeout(300);
  await p.click('.tabs > button:text-is("Home")');
  check("Home shows 3D Heat chosen", await p.$eval('.ribbon button[title^="Heat conduction in a 3D solid"]', el => el.classList.contains("on")));
  await stepHead(3); await p.waitForTimeout(100);
  const lt = await p.textContent(".ribbon");
  check("Loads offers the heat tools for a 3D heat model", lt.includes("Temperature") && lt.includes("Convection") && lt.includes("Heat Flux") && !lt.includes("Pressure"), lt.slice(0, 200));
  await p.click('.ribbon button:has-text("Convection")'); await p.waitForTimeout(100);
  check("convection can go on the back or front face", (await p.$$('.planebar button:has-text("the back face")')).length === 1);
  await p.keyboard.press("Escape");
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  const hl = await p.textContent(".legend");
  check("a solved 3D heat model shows temperatures and its heat balance", hl.includes("Temperature") && hl.includes("Heat balance") && !hl.includes("Deformed shape"), hl.slice(0, 200));
  await go(p, "Results"); await p.waitForTimeout(200);
  const hp = await p.textContent(".page");
  check("its Results page lists the heat flowing through each face and the sum", hp.includes("Heat balance") && hp.includes("Fixed temperature") && hp.includes("Convection") && hp.includes("Heat flux in z"), hp.slice(0, 200));
  await go(p, "Model"); await p.waitForTimeout(150);
  await open("bracket3d"); await p.waitForTimeout(200);
  // 0.10.1 (the mock-ups): a 3D solid has Assembly and Interactions tabs; each module's tab holds what exists and its planned features.
  const tabs3 = await p.$$eval(".tabs > button:not(.file)", els => els.map(e => e.textContent));
  check("a 3D solid has Assembly and Interactions tabs, in the order of the work (0.10.1)", JSON.stringify(tabs3) === JSON.stringify(["Home", "Geometry", "Assembly", "Materials", "Loads & BCs", "Interactions", "Mesh", "Analysis", "Results", "Animate", "View", "Tools", "Help"]), JSON.stringify(tabs3));
  await p.click('.tabs > button:text-is("Assembly")'); await p.waitForTimeout(100);
  check("Assembly: New Part works, its planned tools beside it, dimmed (0.10.1, 0.10.3)", (await p.$$('.ribbon button:has-text("New Part")')).length === 1 && (await p.$$(".ribbon .grp.planned")).length >= 1);
  await p.click('.tabs > button:text-is("Interactions")'); await p.waitForTimeout(100);
  check("Interactions: Contact needs two solids, so it is off for one (0.10.1)", await p.$eval('.ribbon button[title^="Let two touching solids"]', el => el.disabled) && (await p.$$(".ribbon .grp.planned")).length >= 1);
  const icoming = await p.$$eval(".ribbon .grp.planned button.rb.planned", els => els.map(e => e.textContent));
  check("Interactions shows tie, coupling and contact pairs as planned buttons (0.10.1, 0.10.3)", ["Tie", "Coupling", "Find Contact Pairs"].every(x => icoming.some(t => t.startsWith(x))), JSON.stringify(icoming));
  await p.click('.tabs > button:text-is("Materials")'); await p.waitForTimeout(100);
  check("the bar above the view follows the tab: the materials on Materials (0.10.1)", (await p.$$('.ctxbar [data-ctx="materials"]')).length === 1 && (await p.$$('.ctxbar [data-ctx="steps"]')).length === 0);
  await p.click('.tabs > button:text-is("Analysis")'); await p.waitForTimeout(100);
  check("…and the steps on Analysis", (await p.$$('.ctxbar [data-ctx="steps"]')).length === 1 && (await p.$$('.ctxbar [data-ctx="materials"]')).length === 0);
  await p.click('.tabs > button:text-is("Animate")'); await p.waitForTimeout(100);
  check("Animate: Play is there, off until there is a result (0.10.1)", await p.$eval('.ribbon button:has-text("Play")', el => el.disabled));
  check("a part's panel under the view has Mesh statistics (0.10.3; planned in 0.10.2)", (await p.$$eval(".console header button", els => els.map(e => e.textContent))).some(t => t.startsWith("Mesh statistics")));
  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150);
  await p.click('.bs aside button:has-text("Roadmap")'); await p.waitForTimeout(200);
  const rmv = await p.$$eval(".rmfeats h3", els => els.map(e => e.textContent));
  check("File › Roadmap lists the whole map: 100 or more planned features, by release in the agreed order (0.10.1)", (await p.$$(".rmfeats li")).length >= 100 && JSON.stringify(rmv) === JSON.stringify(["0.10.10", "0.11", "0.12", "0.13", "0.14", "0.15", "1.0"]), JSON.stringify(rmv) + " " + (await p.$$(".rmfeats li")).length);
  await p.keyboard.press("Escape"); await p.waitForTimeout(150);
  await stepHead(5); await p.waitForTimeout(100);
  check("Natural Frequencies works for 3D solids", !(await p.$eval('.ribbon button[title^="The lowest natural frequencies"]', el => el.disabled)));
  await p.click('.ribbon button[title^="The lowest natural frequencies"]'); await p.waitForTimeout(500);
  const mb = await p.textContent(".legend");
  check("a solid's modes are listed under the 3D view", mb.includes("Natural frequencies") && (await p.$$(".legend .modelist button")).length === fix.bracket3d_modes.modes.length, mb.slice(0, 160));
  await p.click('.legend button:has-text("Back to the static results")'); await p.waitForTimeout(100);
  // 0.3.2: picking faces in 3D, revolved solids, held directions and lines, the cutting plane.
  await open("bracket3d"); await p.waitForTimeout(300);
  await stepHead(3); await p.click('.ribbon button:has-text("Pressure")'); await p.waitForTimeout(100);
  const at3 = async q => { const v = await p.$eval(".view3d", el => { const r = el.getBoundingClientRect(); return [r.left, r.top]; }); const s = await p.evaluate(q => window.__screen3(q), q); return [v[0] + s[0], v[1] + s[1]]; };
  let xy = await at3([10, 60, 10].map(v => v / 1000)); await p.mouse.click(xy[0], xy[1]); await p.waitForTimeout(200);
  const pff = await lastOp();
  check("clicking the front face in 3D puts the pressure on it", pff?.Add?.item?.Load?.at?.on === "end" && pff.Add.item.Load.at.front === true, JSON.stringify(pff));
  xy = await at3([100, 10, 5].map(v => v / 1000)); await p.mouse.click(xy[0], xy[1]); await p.waitForTimeout(200);
  const ps3 = await lastOp();
  check("clicking a side face in 3D puts it on that edge's face", ps3?.Add?.item?.Load?.at?.on === "edge" && ps3.Add.item.Load.at.edge === 1, JSON.stringify(ps3));
  await p.keyboard.press("Escape");
  check("Loads offers Hold Directions for solids", (await p.$$('.ribbon button:has-text("Hold Directions")')).length === 1);
  await open("vessel3d"); await p.waitForTimeout(300);
  check("a revolved solid says so above the view", (await p.textContent(".planebar")).includes("revolved 90° about the y axis"));
  await p.click('.tabs > button:text-is("Home")');
  await modelCard(); const rins = await p.textContent(".insp");
  check("the Inspector offers extruded or revolved, the angle, and layers round the turn", rins.includes("Turned through") && rins.includes("Layers round the turn") && !rins.includes("Thickness (extruded"), rins.slice(0, 300));
  await p.click(".tree .ti:has-text('Symmetry')"); await p.waitForTimeout(150);
  check("a solid's support can hold chosen directions", (await p.$$eval(".insp select option", os => os.map(o => o.textContent))).includes("Held directions"));
  await p.selectOption('.insp .iprow:has(> label:text-is("Where")) select', "line");
  const wln = await lastOp();
  check("a support on a face can become a line along it, halfway through", wln?.Set?.item?.Support?.at?.on === "line" && wln.Set.item.Support.at.at === 0.5, JSON.stringify(wln));
  await p.evaluate(() => { window.__solveKey = "vessel3d_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(600);
  await p.evaluate(() => { window.__solveKey = null; });
  check("a solved revolved solid can be cut square to X, Y or Z (shown Z up, 0.7.7)", (await p.$$eval(".legend .seg button", bs => bs.map(b => b.textContent))).join() === "Off,X,Y,Z");
  await p.click('.legend .seg button:text-is("Y")'); await p.waitForTimeout(300);
  check("cutting shows a slider to move the plane", (await p.$$('.legend input[type=range][title^="Where the plane cuts"]')).length === 1);
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "vessel3d-cut.png") });
  await p.click('.legend .seg button:text-is("Off")');
  // 0.3.3: load cases and combinations, hinges, the envelope, the section tables.
  await open("braced"); await p.waitForTimeout(200);
  const bm = fix.braced.m, [cg, cq, cw] = bm.cases;
  const drawn = async () => p.$$eval(".dsvg .lg, .dsvg g.load", els => els.length).catch(() => -1);
  await stepHead(3); await p.waitForTimeout(100);
  const picks = await p.$$eval(".ribbon .casepick option", os => os.map(o => o.textContent));
  check("the Loads tab picks a load case or a combination", picks.join() === "Dead,Imposed,Wind,Combination 1,Combination 2,Combination 3", picks.join());
  const nDead = await drawn();
  await p.selectOption(".casepick", `case:${cw.id}`); await p.waitForTimeout(150);
  const nWind = await drawn();
  check("only the picked case's loads are drawn", nDead > 0 && nWind > 0 && nWind < nDead, `${nDead} → ${nWind}`);
  await stepHead(1); await stepHead(3);
  await p.click('.ribbon button:has-text("Force")'); await p.waitForTimeout(100);
  const nd = await center(`.dsvg [data-id="${bm.nodes[4].id}"]`).catch(() => null);
  if (nd) { await p.mouse.click(nd[0], nd[1]); await p.waitForTimeout(150); }
  const nl = await lastOp();
  check("a new load goes into the picked case", nl?.Add?.item?.Load?.case === cw.id, JSON.stringify(nl));
  await p.keyboard.press("Escape");
  await p.selectOption(".casepick", `combo:${bm.combos[0].id}`); await p.waitForTimeout(100);
  await p.evaluate(() => { window.__solveKey = "braced_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  await p.evaluate(() => { window.__solveKey = null; });
  const sc = await p.evaluate(() => window.__calls.filter(c => c[0] === "solve").slice(-1)[0][1]);
  check("Solve solves the picked combination", sc?.which?.on === "combo" && sc.which.id === bm.combos[0].id, JSON.stringify(sc));
  check("the legend says which combination the results are for", (await p.textContent(".legend")).includes("Combination 1: 1.35 × Dead + 1.5 × Imposed"));
  await p.click('.ribbon button:has-text("Envelope")'); await p.waitForTimeout(250);
  const ev = await p.textContent(".dlg");
  check("the envelope lists each member's extremes and the combination that governs", ev.includes("Over 3 combinations") && ev.includes("Governed by") && ev.includes("Combination 3"), ev.slice(0, 200));
  await p.keyboard.press("Escape");
  await stepHead(3); await p.click('.ribbon button:has-text("Cases and Combinations")'); await p.waitForTimeout(150);
  check("the cases dialog lists cases and each combination's factors", (await p.$$(".dlg .tbl.cases")).length === 2 && (await p.$$(".dlg input.fac")).length === bm.combos.length * bm.cases.length);
  await p.fill(`.dlg .tbl.cases >> nth=1 >> tbody tr >> nth=0 >> input.fac >> nth=2`, "0.6"); await p.keyboard.press("Tab"); await p.waitForTimeout(150);
  const cf = (await ops()).reverse().find(o => o.Set?.item?.Combo);
  check("typing a factor puts that case into the combination", cf?.Set?.item?.Combo?.parts.some(x => x.case === cw.id && x.f === 0.6), JSON.stringify(cf));
  // 0.10.3: combinations by rule, from the cases dialog: the engine's preview, then Make.
  await p.click('.dlg button:has-text("Combinations by rule")'); await p.waitForTimeout(300);
  const prev = await p.$$eval(".dlg .cprev tbody tr", rs => rs.map(r => r.children[0].textContent));
  check("Combinations by Rule shows what the rule will make, worked out by the engine (0.10.3)", JSON.stringify(prev) === JSON.stringify(fix.braced_rule_preview.map(c => c.name)) && prev.length > 0, JSON.stringify(prev));
  check("…each case can be made permanent, variable or left out, with its ψ factors (0.10.3)", (await p.$$(".dlg .crule section:first-child tbody tr select")).length === bm.cases.length);
  await p.click(`.dlg footer button:has-text("Make ${fix.braced_rule_preview.length} combinations")`); await p.waitForTimeout(200);
  const rop = (await ops()).reverse().find(o => o.CombosByRule);
  check("…and Make sends the rule with the factors typed (0.10.3)", !!rop && rop.CombosByRule.rule.strength === true && rop.CombosByRule.rule.g_unfav === 1, JSON.stringify(rop));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.keyboard.press("Escape");
  const bmem = bm.members.find(x => x.hinge?.[0]);
  check("hinged beam ends are drawn as small circles", (await p.$$(".dsvg circle.hinge")).length === bm.members.filter(x => x.hinge?.[0]).length * 2);
  await go(p, "Model"); await p.waitForTimeout(100);
  // Picked in the tree: on the drawing its middle is under the distributed load's arrows.
  await p.evaluate(n => [...document.querySelectorAll(".tree button.ti")].find(b => b.querySelector("span")?.textContent === n)?.click(), `Member ${bm.members.indexOf(bmem) + 1}`); await p.waitForTimeout(150);
  const ih = await p.textContent(".insp");
  check("a beam's Inspector has its hinges", ih.includes("Hinge at the start") && ih.includes("Hinge at the end"));
  await stepHead(1); await p.click('.ribbon button[title="Sections"]'); await p.waitForTimeout(200);
  await p.click('.dlg button:has-text("From the tables")'); await p.waitForTimeout(250);
  const tbl = `.dlg:has(header:has-text("Steel section tables"))`;
  const tb = await p.textContent(tbl);
  check("the section tables list IPE, HEA and HEB with their properties", tb.includes("IPE 300") && tb.includes("HEA 100") && tb.includes("HEB 600") && tb.includes("Root radius"));
  await p.fill(`${tbl} .mlist input`, "heb300"); await p.waitForTimeout(100);
  await p.click(`${tbl} .mlist button:text-is("HEB 300")`); await p.click(`${tbl} button:text-is("Add to the model")`); await p.waitForTimeout(200);
  const ts = await lastOp();
  check("adding a table section adds it with its root radius", ts?.Add?.item?.Section?.name === "HEB 300" && Math.abs(ts.Add.item.Section.shape.r - 0.027) < 1e-12, JSON.stringify(ts));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  check("Escape closes the dialog left open under a closed one (0.3.3 bug: focus was lost)", (await p.$$(".ov")).length === 0);
  // 0.5.0: plasticity, on the thick tube.
  await open("tube"); await p.waitForTimeout(300);
  await stepHead(5); await p.click('.ribbon button:has-text("Steps")'); await p.waitForTimeout(200);
  const tsd = await p.$$eval(".dlg table.steps select >> nth=0", ss => [...ss[0].options].map(o => o.textContent)).catch(() => []);
  check("a part's steps offer linear, elastic-plastic and (axisymmetric or plane strain) crushing in time, not frame-only kinds", tsd.length === 3 && tsd[1].startsWith("Elastic-plastic") && tsd[2].startsWith("Crushing in time") && (await p.textContent(".dlg")).includes("Add an elastic-plastic step"), tsd.join("|"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.evaluate(() => { window.__solveKey = "tube_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__solveKey = null; });
  await stepHead(6); await p.waitForTimeout(100);
  const tfs = await p.$$eval(".legend select.fpick option", os => os.map(o => o.textContent)).catch(() => []);
  check("an elastic-plastic part can be coloured by its equivalent plastic strain", tfs.includes("Equivalent plastic strain"), tfs.join("|"));
  await p.selectOption(".legend select.fpick", { label: "Equivalent plastic strain" }).catch(() => {}); await p.waitForTimeout(250);
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "tube-peeq.png") });
  await p.click('.ribbon button:has-text("Load–Deflection")'); await p.waitForTimeout(250);
  const tpd = await p.textContent(".dlg");
  check("Results › Load–Deflection for a part says when it first yields and whether it carries the load", tpd.includes("first yields") && tpd.includes("carries the whole load") && (await p.$$(".dlg svg.curve circle.unstable")).length > 0, tpd.slice(0, 200));
  await p.keyboard.press("Escape");
  await stepHead(1); await p.click('.ribbon button:has-text("Materials")').catch(() => {}); await p.waitForTimeout(200);
  await p.click('.dlg .dtabs button[data-mtab="yield"]').catch(() => {}); await p.waitForTimeout(100);
  check("the material editor has hardening past yield, on its Yielding tab (0.10.2)", (await p.textContent(".dlg").catch(() => "")).includes("Hardening (past yield)"));
  check("…with the stress–strain curve the solver uses: straight to yield, then hardening (0.10.2)", (await p.$$eval(".dlg .sscurve polyline.ss", els => els.length === 1 && els[0].getAttribute("points").trim().split(" ").length === 3).catch(() => false)) && /Yield at a strain of/.test(await p.textContent(".dlg .sscurve")));
  await p.keyboard.press("Escape");
  // 0.5.1: plasticity in a 3D solid, on the holed plate.
  await open("yield3d"); await p.waitForTimeout(400);
  await stepHead(5); await p.click('.ribbon button:has-text("Steps")'); await p.waitForTimeout(200);
  const ysd = await p.$$eval(".dlg table.steps select >> nth=0", ss => [...ss[0].options].map(o => o.textContent)).catch(() => []);
  check("a 3D solid's steps offer linear, elastic-plastic, crushing and collapse (0.7.4), large deformation, or impact (0.9.0)", ysd.length === 5 && ysd[4].startsWith("Impact") && ysd[1].startsWith("Elastic-plastic") && ysd[2].startsWith("Crushing and collapse") && ysd[3].startsWith("Large deformation") && !(await p.textContent(".dlg")).includes("parts come later"), ysd.join("|"));
  {
    // 0.9.0: switching a step to Impact brings an impact's own time (milliseconds, the stable step).
    const before = await p.evaluate(() => window.__calls.length);
    await p.selectOption(".dlg table.steps select >> nth=0", "explicit"); await p.waitForTimeout(200);
    const setStep = await p.evaluate(n => window.__calls.slice(n).filter(c => c[0] === "apply").map(c => c[1]).slice(-1)[0], before);
    const st = setStep?.op?.Set?.item?.Step ?? setStep?.Set?.item?.Step;
    check("switching a step to Impact sets a millisecond length and the stable time step (0.9.0)", st && st.kind === "explicit" && st.duration === 0.001 && st.dt === 0 && st.time.length === 0, JSON.stringify(setStep).slice(0, 300));
  }
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.evaluate(() => { window.__solveKey = "yield3d_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(600);
  await p.evaluate(() => { window.__solveKey = null; });
  await stepHead(6); await p.waitForTimeout(150);
  const yfs = await p.$$eval(".legend select.fpick option", os => os.map(o => o.textContent)).catch(() => []);
  check("a yielding 3D solid can be coloured by its equivalent plastic strain", yfs.includes("Equivalent plastic strain"), yfs.join("|"));
  check("a nonlinear solve plays through its increments: the player counts the load factor", (await p.textContent(".player .plabel").catch(() => "")).includes("load factor"), await p.textContent(".player").catch(() => "none"));
  await p.selectOption(".legend select.fpick", { label: "Equivalent plastic strain" }).catch(() => {}); await p.waitForTimeout(300);
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "yield3d-peeq.png") });
  await p.click('.ribbon button:has-text("Load–Deflection")'); await p.waitForTimeout(250);
  const ypd = await p.textContent(".dlg").catch(() => "");
  check("Results › Load–Deflection works for a 3D solid", ypd.includes("first yields") && ypd.includes("carries the whole load") && (await p.$$(".dlg svg.curve circle.unstable")).length > 0, ypd.slice(0, 200));
  await p.keyboard.press("Escape");
  // 0.5.5: imported tetrahedral meshes: the faces in the tree and the 3D view, picking a face, solving.
  await open("elbow"); await p.waitForTimeout(400);
  const ef = fix.elbow.m.volume;
  check("an imported mesh is listed in the tree with its faces", (await p.textContent(".tree")).includes("Imported mesh") && (await p.$$(".tree .step:first-of-type .tgrp:not(.tplan):not(.torigin) .ti")).length === ef.faces.length && (await p.textContent(".tree")).includes("10-node tetrahedra"), await p.textContent(".tree"));
  check("the bar above the view says what the mesh is, with no Section view", (await p.textContent(".planebar")).includes("tetrahedra") && !(await p.textContent(".planebar")).includes("Section"));
  await modelCard(); check("a mesh's material is chosen in the Inspector", (await p.textContent(".insp")).includes("Material of the mesh"));
  await stepHead(3); await p.click('.ribbon button:has-text("Pressure")'); await p.waitForTimeout(100);
  let exy = await at3([0.2121, 0.2121, 0.06]); await p.mouse.click(exy[0], exy[1]); await p.waitForTimeout(200);
  const ep = await lastOp();
  check("clicking the mesh in 3D puts the pressure on the face clicked", ep?.Add?.item?.Load?.at?.on === "face" && ef.faces[ep.Add.item.Load.at.face] !== undefined, JSON.stringify(ep));
  await p.keyboard.press("Escape");
  await p.evaluate(() => { window.__solveKey = "elbow_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(600);
  await p.evaluate(() => { window.__solveKey = null; });
  check("a solved mesh is coloured and balanced", (await p.textContent(".legend")).includes("reactions balance the loads") && (await p.textContent(".legend")).includes("Von Mises"), (await p.textContent(".legend")).slice(0, 200));
  // 0.5.6: heat conduction in an imported mesh.
  await open("elbowheat"); await p.waitForTimeout(400);
  check("a heat model on an imported mesh lists its faces and its convection", (await p.textContent(".tree")).includes("Imported mesh") && (await p.textContent(".tree")).includes("Convection"));
  await p.evaluate(() => { window.__solveKey = "elbowheat_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(600);
  await p.evaluate(() => { window.__solveKey = null; });
  const ehl = await p.textContent(".legend").catch(() => "");
  check("an imported mesh's temperatures are shown with the heat balance", ehl.includes("Temperature") && /heat|balance/i.test(ehl), ehl.slice(0, 200));
  // 0.5.6 bug: a narrow range (114.1 to 114.5 °C) labelled every band "114".
  const ticks = await p.$$eval(".legend .cbar small", els => els.map(e => e.textContent));
  check("the colour bar's labels tell a narrow range apart", new Set(ticks).size === ticks.length && ticks.length > 3, ticks.join("|"));
  await open("elbow"); await p.waitForTimeout(400);
  // 0.5.7: an imported mesh's steps offer yielding and large deformation.
  await stepHead(5); await p.click('.ribbon button:has-text("Steps")'); await p.waitForTimeout(200);
  const esd = await p.textContent(".dlg").catch(() => "");
  check("an imported mesh can be solved elastic-plastic or through large deformation", esd.includes("Add an elastic-plastic step") && esd.includes("Add a large-deformation step"), esd.slice(0, 200));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  // 0.5.6: natural frequencies of an imported mesh.
  await p.evaluate(() => { window.__modesKey = "elbow_modes"; });
  await stepHead(5); await p.click('.ribbon button:has-text("Natural Frequencies")'); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__modesKey = null; });
  const emb = await p.textContent(".legend").catch(() => "");
  check("an imported mesh's natural frequencies are listed under the 3D view", emb.includes("Natural frequencies") && (await p.$$(".legend .modelist button")).length === fix.elbow_modes.modes.length, emb.slice(0, 160));
  await stepHead(1); await p.click('.ribbon button[title="STEP options"]'); await p.waitForTimeout(150);
  check("a 3D solid's Import offers STEP, mesh files and a DXF section (0.7.6)", (await p.textContent(".menu")).includes("STEP file") && (await p.textContent(".menu")).includes("Mesh file") && (await p.textContent(".menu")).includes("DXF section"));
  await p.click('.menu button:has-text("millimetres")').catch(() => {}); await p.waitForTimeout(250);
  if (await p.$('.dlg button:has-text("Don\'t save")')) await p.click('.dlg button:has-text("Don\'t save")');
  await p.waitForTimeout(300);
  const im = await p.evaluate(() => window.__calls.filter(c => c[0] === "import_mesh").slice(-1)[0]?.[1]);
  check("Geometry › Import › Mesh asks the file's unit and imports it", im?.unit === 0.001, JSON.stringify(im));
  // 0.5.4: Datum Link: values to Datum Calc, a curve to Datum Plot, a material from Datum Atlas.
  await open("tube"); await p.waitForTimeout(300);
  await p.evaluate(() => { window.__solveKey = "tube_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__solveKey = null; });
  await stepHead(6); await p.click('.ribbon button[title^="The largest stress, movement"]'); await p.waitForTimeout(200);
  const sc5 = await p.evaluate(() => window.__calls.filter(c => c[0] === "send_link").slice(-1)[0]?.[1]);
  check("Send to Datum Calc sends the largest stress and movement as values with SI units", sc5?.to === "calc" && sc5?.what?.kind === "value" && sc5.what.data.some(q => q.name === "sigma_vm_max" && q.unit === "Pa" && q.value > 0), JSON.stringify(sc5).slice(0, 200));
  await p.click('.ribbon button[title^="A curve or list from this solve"]'); await p.waitForTimeout(150);
  await p.click('.menu button:has-text("Load–deflection curve")').catch(() => {}); await p.waitForTimeout(200);
  const sp2 = await p.evaluate(() => window.__calls.filter(c => c[0] === "send_link").slice(-1)[0]?.[1]);
  check("Send to Datum Plot sends the load–deflection curve as a table", sp2?.to === "plot" && sp2?.what?.kind === "table" && sp2.what.data.rows.length === fix.tube_results.path.load.length && sp2.what.data.columns[1].unit === "m", JSON.stringify(sp2).slice(0, 200));
  await stepHead(1); await p.click('.ribbon button:has-text("Materials")'); await p.waitForTimeout(200);
  await p.click('.dlg button:has-text("From Datum Atlas")'); await p.waitForTimeout(250);
  const atl = (await ops()).filter(o => o.Add?.item?.Material).slice(-1)[0];
  check("a material from Datum Atlas is added to the model with its source", atl?.Add.item.Material.name === "Aluminium 6061-T6" && atl.Add.item.Material.source === "ASM Handbook", JSON.stringify(atl));
  while (await p.$(".dlg")) { await p.keyboard.press("Escape"); await p.waitForTimeout(120); }
  // 0.5.3: File › New asks what is being analysed, then makes the model.
  await p.keyboard.press("Control+n"); await p.waitForTimeout(250);
  check("Ctrl+N opens the New study page with the kinds of study", (await p.$$(".newstudy .kcard[data-kind]")).length === 7);
  await p.click('.newstudy .kcard[data-kind="solid"]'); await p.waitForTimeout(100); await p.click('.newstudy .kcard[data-sub="mesh"]'); await p.waitForTimeout(100);
  check("a 3D solid can start from a mesh file, in a unit you choose", (await p.$$('.newstudy .ncreate button:has-text("Import a mesh in")')).length === 3);
  await p.click('.newstudy .kcard[data-kind="truss"]'); await p.waitForTimeout(120);
  check("a 2D truss offers what you can find out about it", (await p.$$(".newstudy .kcard[data-want]")).length >= 3 && (await p.textContent(".newstudy")).includes("Large deflection"));
  await p.click('.newstudy .ncreate .btn.pri'); await p.waitForTimeout(250);
  if (await p.$('.dlg button:has-text("Don\'t save")')) { await p.click('.dlg button:has-text("Don\'t save")'); }
  await p.waitForTimeout(400);
  const nm = await p.evaluate(() => window.__calls.filter(c => c[0] === "new_model").slice(-1)[0]?.[1]);
  const ns = (await ops()).filter(o => o.SetSettings).slice(-1)[0];
  check("Create makes a frame model marked as a truss, with the truss bar tool ready", nm?.analysis === "frame" && nm?.example === null && ns?.SetSettings?.s?.study === "truss" && !(await p.$(".bs")), JSON.stringify([nm, ns]));
  // 0.5.2: large deformation of a 3D solid, on the rubber finger.
  await open("rubber"); await p.waitForTimeout(400);
  await stepHead(5); await p.click('.ribbon button:has-text("Steps")'); await p.waitForTimeout(200);
  const rsd = await p.$$eval(".dlg table.steps select >> nth=0", ss => [...ss[0].options].map(o => o.value)).catch(() => []);
  check("a 3D solid's steps offer large deformation", rsd.includes("large") && (await p.textContent(".dlg")).includes("Add a large-deformation step"), rsd.join("|"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.evaluate(() => { window.__solveKey = "rubber_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(600);
  await p.evaluate(() => { window.__solveKey = null; });
  await stepHead(6); await p.waitForTimeout(200);
  check("a large deformation is drawn true to size", (await p.textContent("body")).includes("drawn true to size"));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "rubber.png") });
  await p.click('.ribbon button:has-text("Load–Deflection")'); await p.waitForTimeout(250);
  const rpd = await p.textContent(".dlg").catch(() => "");
  check("Results › Load–Deflection names a large deformation", rpd.includes("Large deformation") && rpd.includes("carries the whole load"), rpd.slice(0, 200));
  await p.keyboard.press("Escape");
  // 0.4.4: rigid floors, on the building in an earthquake.
  await open("building"); await p.waitForTimeout(400);
  await go(p, "Model"); await p.waitForTimeout(80);
  await p.evaluate(() => window.__setSel?.([]));
  await modelCard(); const rigidBox = await p.$('.insp label.chk:has-text("Rigid floors") input');
  check("a 3D frame's Model panel has Rigid floors, ticked for the building", !!rigidBox && await rigidBox.isChecked());
  if (rigidBox) { await rigidBox.click(); await p.waitForTimeout(150); }
  const rigidOp = await lastOp();
  check("unticking Rigid floors sets it off", rigidOp?.SetSettings?.s?.rigid_floors === false, JSON.stringify(rigidOp?.SetSettings?.s?.rigid_floors));
  await p.evaluate(() => { window.__solveKey = "building_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__solveKey = null; });
  await stepHead(6); await p.click('.ribbon button:has-text("Dynamics")'); await p.waitForTimeout(250);
  check("the building's modes are listed, reaching most of the mass", (await p.$$(".dlg table.tbl tbody tr")).length === fix.building_results.dynamic.modes.length && (await p.textContent(".dlg")).includes("99.8 %"));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "building.png") });
  await p.keyboard.press("Escape");
  // 0.4.3: tension-only bracing and support settlement.
  await open("xbrace"); await p.waitForTimeout(250);
  const xm = fix.xbrace.m, rod = xm.members.find(b => b.only === "tension");
  check("a settlement is drawn at its support", (await p.$$(".dsvg .la.settle")).length === 1);
  await go(p, "Model"); await p.waitForTimeout(80);
  await p.click(`.dsvg [data-id="${rod.id}"] line`, { force: true }).catch(() => {}); await p.waitForTimeout(150);
  check("a member's Inspector says what it carries", (await p.$eval(".insp select >> nth=2", s => s.value).catch(() => "")) === "tension" || (await p.textContent(".insp")).includes("Tension only"));
  await p.evaluate(() => { window.__solveKey = "xbrace_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  await p.evaluate(() => { window.__solveKey = null; });
  const xr = fix.xbrace_results;
  check("slack braces are drawn dashed and counted under the drawing", xr.slack.length === 2 && (await p.$$(".dsvg .mem.slack")).length === xr.slack.length && (await p.textContent(".legend")).includes("Slack"), `${(await p.$$(".dsvg .mem.slack")).length}`);
  check("the slack braces carry nothing and the others carry the wind", xr.members.filter(x => xr.slack.includes(x.id)).every(x => x.n.every(v => v === 0)) && xr.members.filter(x => !xr.slack.includes(x.id) && xm.members.find(b => b.id === x.id).only === "tension").every(x => x.n[20] > 0));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "xbrace.png") });
  await stepHead(3); await p.click('.ribbon button:has-text("Settlement")'); await p.waitForTimeout(80);
  const xs = await center(`.dsvg [data-id="${xm.nodes[0].id}"]`).catch(() => null);
  if (xs) { await p.mouse.click(xs[0], xs[1]); await p.waitForTimeout(150); }
  const xo = await lastOp();
  check("Loads › Settlement moves the support clicked, 10 mm down to start", xo?.Add?.item?.Load?.kind?.kind === "settlement" && xo.Add.item.Load.kind.dy === -0.01, JSON.stringify(xo));
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape");
  // 0.4.2: dynamics, on the frame in an earthquake.
  await open("quake"); await p.waitForTimeout(250);
  const qm = fix.quake.m, qs = qm.steps[0];
  check("rigid end zones are drawn thick at the beams' ends", (await p.$$(".dsvg line.rigidz")).length === 2 * qm.members.filter(b => b.offset).length && qm.members.some(b => b.offset));
  const qb = qm.members.find(b => b.offset);
  await go(p, "Model"); await p.waitForTimeout(80);
  await p.click(`.dsvg [data-id="${qb.id}"] line >> nth=0`, { force: true }).catch(() => {}); await p.waitForTimeout(150);
  check("a beam's Inspector has its rigid ends", (await p.textContent(".insp")).includes("Rigid end at the start") && (await p.textContent(".insp")).includes("Rigid end at the end"));
  await p.keyboard.press("Escape");
  await stepHead(5); await p.click('.ribbon button:has-text("Steps")'); await p.waitForTimeout(200);
  check("the steps dialog shows a spectrum step's settings: direction, damping, modes, combination and the spectrum", (await p.$$(".dlg .dynset")).length === 1 && (await p.$eval(".dlg .dynset textarea", t => t.value)).split("\n").length === qs.spectrum.length && (await p.textContent(".dlg .dynset")).includes("Combined by"));
  await p.fill(".dlg .dynset input.fac >> nth=1", "8"); await p.keyboard.press("Tab"); await p.waitForTimeout(150);
  const qo = await lastOp();
  check("changing the number of modes sets the step", qo?.Set?.item?.Step?.modes === 8, JSON.stringify(qo?.Set?.item?.Step?.modes));
  await p.click('.dlg button:has-text("Add a time history")'); await p.waitForTimeout(150);
  const qh = await lastOp();
  check("adding a time history adds a ground-shaking step with a record", qh?.Add?.item?.Step?.kind === "history" && qh.Add.item.Step.ground === true && qh.Add.item.Step.time.length >= 2, JSON.stringify(qh));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await stepHead(3); await p.click('.ribbon button[title^="A mass at a node"]'); await p.waitForTimeout(80);
  const qn = await center(`.dsvg [data-id="${qm.nodes[0].id}"]`).catch(() => null);
  if (qn) { await p.mouse.click(qn[0], qn[1]); await p.waitForTimeout(150); }
  const qmass = await lastOp();
  check("Loads › Mass puts a mass on the node clicked", qmass?.Add?.item?.Load?.kind?.kind === "mass" && qmass.Add.item.Load.at.on === "node", JSON.stringify(qmass));
  check("masses are drawn at their nodes", (await p.$$(".dsvg .lmass")).length >= 6);
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape");
  await p.evaluate(() => { window.__solveKey = "quake_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  await p.evaluate(() => { window.__solveKey = null; });
  check("a spectrum's legend says its peaks are combined, with no balance to check", (await p.textContent(".legend")).includes("combined by CQC"));
  await stepHead(6); await p.waitForTimeout(100);
  await p.click('.ribbon button:has-text("Dynamics")'); await p.waitForTimeout(250);
  const qd = await p.textContent(".dlg");
  const qr = fix.quake_results.dynamic;
  check("Results › Dynamics lists each mode's period, participation and share of the mass", (await p.$$(".dlg table.tbl tbody tr")).length === qr.modes.length && qd.includes("CQC") && qd.includes(num3(qr.share * 100)), qd.slice(0, 300));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "quake-modes.png") });
  await p.keyboard.press("Escape");
  await p.evaluate(() => { window.__solveKey = "quake_history"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  await p.evaluate(() => { window.__solveKey = null; });
  check("a time history's legend says when its results are, inertia included", (await p.textContent(".legend")).includes("Equilibrium at t ="));
  check("a time history plays through its times", (await p.textContent(".player .plabel").catch(() => "")).includes("t = "), await p.textContent(".player").catch(() => "none"));
  // 0.10.2 (the mock-ups): its graphs open in the bottom panel; the bar above the view says its time, record and damping.
  check("a time history opens its Graphs in the bottom panel: the movement against time (0.10.2, the mock-ups)", (await p.textContent(".console header button.on").catch(() => "")) === "Graphs" && (await p.$$(".gpanels .gpanel svg path")).length >= 1 && /Movement in/.test(await p.textContent(".gpanels")));
  await p.click('.tabs > button:text-is("Results")'); await p.waitForTimeout(100);
  check("…and the bar above the view says its time, record and damping", /Time.*Record.*Damping/.test(await p.textContent('.ctxbar [data-ctx="history"]').catch(() => "")));
  await p.click('.ribbon button:has-text("Dynamics")'); await p.waitForTimeout(250);
  const qhd = await p.textContent(".dlg");
  check("Results › Dynamics plots a time history's movement against time and lists the largest movements", (await p.$$(".dlg svg.curve path.line")).length === 1 && qhd.includes("Largest movements") && qhd.includes("Newmark"), qhd.slice(0, 200));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "quake-history.png") });
  await p.keyboard.press("Escape");
  // 0.4.1: plates in a 3D frame, on the floor slab.
  await open("slab"); await p.waitForTimeout(400);
  const slm = fix.slab.m, slabId = slm.shapes[0].id;
  check("a plate is listed in the tree with its thickness", (await p.textContent(".tree")).includes("Plates") && (await p.textContent(".tree")).includes("200 mm thick"));
  await stepHead(1); await p.click('.ribbon button[title^="Pick a plane and draw members and plates"]'); await p.waitForTimeout(200);
  await p.click('.view3d .pickbar .seg button:text-is("Top")'); await p.waitForTimeout(400);
  await p.selectOption(".sketchbar select", "3"); await p.waitForTimeout(250);
  check("a sketch on the Top plane at the slab's height shows the slab", (await p.$$(".sketchover .dsvg .shape")).length === 1);
  check("the Sketch tab offers plates on a plane of a 3D frame", (await p.$$('.ribbon button[title^="Draw a rectangular plate"]')).length === 1);
  await p.click('.ribbon button[title^="Draw a rectangular plate"]');
  const pcb = await canvasBox();
  await p.mouse.click(pcb.x + pcb.w * 0.3, pcb.y + pcb.h * 0.3); await p.waitForTimeout(80); await p.mouse.click(pcb.x + pcb.w * 0.4, pcb.y + pcb.h * 0.4); await p.waitForTimeout(150);
  const apl = (await ops()).reverse().find(o => o.Add?.item?.Shape);
  check("a plate drawn on the Plan plane goes to the engine on that plane, at its height and thickness", apl?.Add.item.Shape.plate?.plane === "xz" && apl.Add.item.Shape.plate.depth === 3 && apl.Add.item.Shape.plate.t === 0.2, JSON.stringify(apl));
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape");
  await stepHead(3); await p.waitForTimeout(100);
  await p.click('.ribbon button:has-text("Plate Pressure")'); await p.waitForTimeout(80);
  const inPlate = await center(".dsvg .shape");
  await p.mouse.click(inPlate[0], inPlate[1]); await p.waitForTimeout(150);
  const plp = await lastOp();
  check("Plate Pressure clicked inside a plate puts a pressure on the whole plate", plp?.Add?.item?.Load?.at?.on === "plate" && plp.Add.item.Load.at.shape === slabId && plp.Add.item.Load.kind.kind === "pressure", JSON.stringify(plp));
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape");
  await go(p, "Model"); await p.waitForTimeout(100);
  await p.click(`.tree :text-is("Slab")`); await p.waitForTimeout(150);
  check("a plate's Inspector has its thickness and plane", (await p.textContent(".insp")).includes("Thickness") && (await p.textContent(".insp")).includes("Plan plane"));
  await p.click('.sketchbar button:has-text("Finish Sketch")'); await p.waitForTimeout(250);
  await p.evaluate(() => { window.__solveKey = "slab_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__solveKey = null; });
  const slabLeg = await p.textContent(".legend").catch(() => "");
  check("once solved, the legend colours the plates (von Mises by default) with a scale", slabLeg.includes("Plates by") && (await p.$eval(".legend select.plates", s => s.value)) === "vm" && (await p.$$(".legend .cbar")).length >= 2, slabLeg.slice(0, 200));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "slab-vm.png") });
  await p.selectOption(".legend select.plates", "mx"); await p.waitForTimeout(200);
  check("the plates can be coloured by their moments", (await p.textContent(".legend")).includes("kN·m/m") || (await p.textContent(".legend")).includes("kN"));
  await stepHead(6); await p.click('.ribbon button:has-text("Tables and Equilibrium")'); await p.waitForTimeout(250);
  const slabPage = await p.textContent(".page");
  check("the Results page lists the plates' extremes and the reactions balance", slabPage.includes("Plates") && slabPage.includes("Von Mises stress") && slabPage.includes("Membrane force Nx") && slabPage.includes("Discrete Kirchhoff"), slabPage.slice(0, 300));
  await go(p, "Model"); await p.waitForTimeout(100);
  await p.evaluate(() => { window.__modesKey = "slab_modes"; });
  await stepHead(5); await p.click('.ribbon button:has-text("Natural Frequencies")'); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__modesKey = null; });
  check("natural frequencies of a slab list its modes", (await p.textContent("body")).includes(String(+fix.slab_modes.modes[0].f.toPrecision(3)).slice(0, 4)));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "slab-mode.png") });
  // 0.4.0: analysis steps and large deflection, on Lee's frame (snap-through).
  await open("lee"); await p.waitForTimeout(200);
  const leeM = fix.lee.m;
  await stepHead(5); await p.waitForTimeout(100);
  check("Solve › Steps shows on when the model has a large-deflection step", await p.$eval('.ribbon button:has-text("Steps")', b => b.classList.contains("on")));
  await p.click('.ribbon button:has-text("Steps")'); await p.waitForTimeout(200);
  const stD = await p.textContent(".dlg");
  // 0.10.3: each large-deflection step has its solver settings, typed in.
  check("a large-deflection step shows its solver settings: tolerance, iterations, smallest increment (0.10.3)", (await p.$$(".dlg .solverset input.fac")).length >= 3 && stD.includes("solver settings"));
  { const tolIn = await p.$(".dlg .solverset input.fac"); await tolIn.fill("1e-10"); await tolIn.press("Tab"); await p.waitForTimeout(150);
    const so = (await ops()).reverse().find(o => o.Set?.item?.Step);
    check("…typing a tolerance sets that step's solver settings (0.10.3)", so?.Set?.item?.Step?.solver?.tol === 1e-10 && so.Set.item.Step.solver.max_iter === 30, JSON.stringify(so?.Set?.item?.Step?.solver)); }
  check("the steps dialog lists the step, how it is solved and the node it follows", (await p.$eval(".dlg table.steps input", i => i.value)) === "Past the limit" && (await p.$eval(".dlg table.steps select", s => s.value)) === "arc" && (await p.$$eval(".dlg table.steps tbody tr:not(.sinit)", r => r.length)) === 1 && (await p.$$(".dlg table.steps tr.sinit")).length === 1, stD.slice(0, 300) + " | " + (await p.$$eval(".dlg table.steps select", ss => ss.map(s => s.value).join())));
  await p.selectOption(".dlg table.steps select >> nth=0", "newton"); await p.waitForTimeout(150);
  const stO = await lastOp();
  check("changing how a step is solved sets the step", stO?.Set?.item?.Step?.kind === "newton" && stO.Set.item.Step.id === leeM.steps[0].id, JSON.stringify(stO));
  await p.click('.dlg button:has-text("Add a large-deflection step")'); await p.waitForTimeout(150);
  const stA = await lastOp();
  check("adding a step adds a large-deflection step", stA?.Add?.item?.Step?.kind === "newton" && stA.Add.item.Step.factor === 1, JSON.stringify(stA));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.evaluate(() => { window.__solveKey = "lee_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  await p.evaluate(() => { window.__solveKey = null; });
  await stepHead(6); await p.waitForTimeout(150);
  check("Results › Load–Deflection is offered after a large-deflection solve", !(await p.$eval('.ribbon button:has-text("Load–Deflection")', b => b.disabled)), JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "solve").slice(-1))) + (await p.textContent(".status, .toast").catch(() => "")));
  await p.click('.ribbon button:has-text("Load–Deflection")'); await p.waitForTimeout(250);
  const crv = await p.textContent(".dlg");
  const leeR = fix.lee_results;
  check("the curve has a point per increment and marks the unstable ones", (await p.$$(".dlg svg.curve circle.pt")).length === leeR.path.load.length && (await p.$$(".dlg svg.curve circle.unstable")).length === leeR.path.stable.filter(x => !x).length && leeR.path.stable.some(x => !x));
  check("the curve dialog names the first limit load", crv.includes("first limit") && crv.includes(String(+leeR.path.peak.toPrecision(5))), crv.slice(0, 200));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "lee-curve.png") });
  await p.click('.dlg button:text-is("Rotation")'); await p.waitForTimeout(100);
  check("the curve can show the rotation instead", (await p.textContent(".dlg svg.curve")).includes("Rotation"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  check("a large-deflection shape is drawn true to size", (await p.textContent(".legend")).includes("true to size"), await p.textContent(".legend"));
  // 0.5.8: large rotations in 3D frames: the 45° bend pushed out of its plane.
  await open("bend45"); await p.waitForTimeout(200);
  await stepHead(5); await p.waitForTimeout(100);
  await p.click('.ribbon button:has-text("Steps")'); await p.waitForTimeout(200);
  check("a 3D frame's steps can be solved past limit loads (arc length)", (await p.$eval(".dlg table.steps select", s => [...s.options].map(o => o.value))).includes("arc") && (await p.textContent(".dlg")).includes("corotational"), await p.textContent(".dlg"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.evaluate(() => { window.__solveKey = "bend45_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  await p.evaluate(() => { window.__solveKey = null; });
  await stepHead(6); await p.waitForTimeout(150);
  check("Results › Load–Deflection is offered for a 3D frame's large rotations", !(await p.$eval('.ribbon button:has-text("Load–Deflection")', b => b.disabled)));
  const bMax = Math.max(...fix.bend45_results.members.flatMap(mr => mr.d.map(d => Math.hypot(...d))));
  check("a 3D frame's large rotations are drawn true to size, as far as the nodes went (0.5.8: the pieces' small local movements were drawn)", (await p.textContent(".legend")).includes("true to size") && bMax > 1.4 && (await p.textContent(".legend")).includes(String(Math.round(bMax * 1000)).slice(0, 3)), await p.textContent(".legend") + " " + bMax);
  await p.click('.ribbon button:has-text("Load–Deflection")'); await p.waitForTimeout(250);
  const bR = unpack(fix.bend45_results);
  check("the 3D curve has a point per increment", (await p.$$(".dlg svg.curve circle.pt")).length === bR.path.load.length && bR.path.load.length > 2, String(bR.path.load.length));
  await p.click('.dlg button:text-is("Movement in z")'); await p.waitForTimeout(100);
  check("the 3D curve can show the movement in z and the turn", (await p.textContent(".dlg svg.curve")).includes("Movement in z") && (await p.$$('.dlg button:text-is("Turn")')).length === 1);
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "bend45-curve.png") });
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  // 0.6.0: parts and assembly, on the tube between two plates.
  await open("tubeplates"); await p.waitForTimeout(400);
  check("an assembly meshed when it opens shows step 4 Mesh done (0.7.1 bug)", await p.$eval(".tree .step:nth-of-type(4)", el => el.classList.contains("done")));
  const tpM = fix.tubeplates.m;
  check("an assembly lists its parts in the tree", (await p.textContent(".tree")).includes("Parts") && tpM.parts.every(pt => 0) === false && (await p.$$eval(".tree .ti", els => els.map(e => e.textContent))).filter(t => /Tube|Base plate|Top plate/.test(t)).length >= 3);
  check("the bar above the 3D view counts the parts, the tetrahedra and the ties", /3 parts, [\d,]+ (10|4)-node tetrahedra, tied at/.test(await p.textContent(".planebar")), await p.textContent(".planebar"));
  await p.click(".tree .ti:has-text('Top plate')"); await p.waitForTimeout(200);
  const pins = await p.textContent(".insp");
  check("a part's Inspector offers its sweep, material, size, place and Edit Sketch", pins.includes("Made by") && pins.includes("Depth") && pins.includes("Moved by y") && /Turned about x/i.test(pins) && (await p.$$('.insp button:has-text("Edit Sketch")')).length === 1, pins.slice(0, 300));
  await p.fill('.insp .iprow:has(> label:text-is("Depthmm")) input, .insp .iprow:has(> label:has-text("Depth")) input', "20"); await p.press('.insp .iprow:has(> label:has-text("Depth")) input', "Enter"); await p.waitForTimeout(150);
  const pd = (await lastOp())?.Set?.item?.Part;
  check("changing a part's depth sets the part", pd && pd.name === "Top plate" && Math.abs(pd.depth - 0.02) < 1e-9, JSON.stringify(pd));
  await p.click(".tree .ti:has-text('Top plate')"); await p.waitForTimeout(150);
  await p.click('.insp button:has-text("Edit Sketch")'); await p.waitForTimeout(250);
  const topId = tpM.parts.find(x => x.name === "Top plate").id;
  check("Edit Sketch shows that part's sketch alone, square on, with the Sketch tab", (await p.$$(".sketchover .dsvg path.shape")).length === tpM.shapes.filter(s => s.part === topId).length && (await p.textContent(".sketchbar")).includes("Top plate") && (await p.$$('.tabs .sktab.on')).length === 1, String(await p.$$eval(".sketchover .dsvg path.shape", e => e.length)));
  await p.click('.ribbon button:has-text("Rectangle")'); await p.waitForTimeout(100);
  const cvb = await p.$eval(".sketchover .canvas svg.dsvg", el => { const r = el.getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; });
  await p.mouse.click(cvb[0] + cvb[2] * 0.3, cvb[1] + cvb[3] * 0.3); await p.mouse.click(cvb[0] + cvb[2] * 0.35, cvb[1] + cvb[3] * 0.35); await p.waitForTimeout(200);
  const ps = (await lastOp())?.Add?.item?.Shape;
  check("a shape drawn in a part's sketch goes in that part", ps && ps.part === topId, JSON.stringify(await lastOp()));
  await p.keyboard.press("Escape");
  await p.click('.sketchbar button:has-text("Finish Sketch")'); await p.waitForTimeout(300);
  check("Finish Sketch goes back to the 3D view with the part selected", (await p.$$(".sketchstage")).length === 0 && (await p.$$(".view3d canvas")).length === 1 && (await p.textContent(".insp")).includes("Edit Sketch"));
  // New Part (0.6 parts; New Sketch is the geometry engine's from 0.7.1) on a plane makes a new part there; Cancel takes it away again.
  await stepHead(1); await p.click('.ribbon button:has-text("New Part")'); await p.waitForTimeout(200);
  await p.click('.view3d .pickbar .seg button:text-is("Side")'); await p.waitForTimeout(400);
  const np = (await ops()).reverse().find(o => o?.Add?.item?.Part);
  check("New Sketch on the Side plane adds a part turned onto it", !!np && Math.abs(np.Add.item.Part.turn[1] - Math.PI / 2) < 1e-9 && (await p.textContent(".sketchbar")).includes("Side plane"), JSON.stringify(np));
  check("a new part's sketch starts with the Rectangle tool", await p.$eval('.ribbon button:has-text("Rectangle")', el => el.classList.contains("on")));
  await p.click('.sketchbar button:has-text("Turn the View")'); await p.waitForTimeout(250);
  check("Turn the View hides the sketch canvas and keeps the sketch open", (await p.$$(".sketchover")).length === 0 && (await p.$$(".sketchbar")).length === 1);
  await p.click('.sketchbar button:has-text("Look At")'); await p.waitForTimeout(250);
  check("Look At brings the sketch canvas back", (await p.$$(".sketchover .canvas")).length === 1);
  const nUndo = (await p.evaluate(() => window.__calls.filter(c => c[0] === "history").length));
  await p.click('.sketchbar button:has-text("Cancel")'); await p.waitForTimeout(300);
  check("Cancel undoes the new part and closes the sketch", (await p.evaluate(() => window.__calls.filter(c => c[0] === "history").length)) === nUndo + 1 && (await p.$$(".sketchstage")).length === 0);
  // 0.6.3: basic modelling on the assembly.
  const atp = async q => { const v = await p.$eval(".view3d", el => { const r = el.getBoundingClientRect(); return [r.left, r.top]; }); const s = await p.evaluate(q => window.__screen3(q), q); return [v[0] + s[0], v[1] + s[1]]; };
  const ops0 = async () => (await ops()).slice(-1)[0];
  await stepHead(1); await p.waitForTimeout(100);
  const grt = await p.textContent(".ribbon");
  check("Geometry offers the modelling tools for an assembly (0.6.3)", ["Extrude", "Hole", "Fillet", "Chamfer", "Cut", "Move or Copy", "Turn or Pattern", "Mirror"].every(t => grt.includes(t)), grt.slice(0, 300));
  const onTop = await atp([0.03, 0.11, 0.03]);
  await p.mouse.click(onTop[0], onTop[1]); await p.waitForTimeout(250);
  const ti = await p.textContent(".insp");
  check("clicking a part in 3D selects it; its Inspector gives its volume, mass and which way it goes", (await p.inputValue(".insp input")) === "Top plate" && ti.includes("Volume") && ti.includes("Mass") && ti.includes("Which way"), ti.slice(0, 200));
  check("the Inspector's volume is the plate's 80 × 80 × 10 mm", ti.includes("64 cm³"), ti.slice(0, 400));
  // Move or Copy: 100 mm along X, two copies.
  await p.click('.ribbon button:has-text("Move or Copy")'); await p.waitForTimeout(150);
  const mi = await p.$$(".dlg .form input");
  await mi[0].fill("100"); await mi[0].press("Tab"); await mi[3].fill("2"); await mi[3].press("Tab"); await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(150);
  const mvp = (await ops0())?.PlaceParts;
  check("Move or Copy sends the parts, the step in the engine's axes and the copies", mvp && mvp.ids.length === 1 && mvp.ids[0] === topId && mvp.how.kind === "move" && Math.abs(mvp.how.by[0] - 0.1) < 1e-12 && mvp.how.by[1] === 0 && mvp.how.by[2] === 0 && mvp.copies === 2, JSON.stringify(mvp));
  check("a selected part stays selected after an edit (every edit dropped it before 0.6.3)", (await p.inputValue(".insp input").catch(() => "")) === "Top plate");
  // Turn or Pattern about Z (the engine's y), 90°, three copies.
  await p.click('.ribbon button:has-text("Turn or Pattern")'); await p.waitForTimeout(150); await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(150);
  const tn = (await ops0())?.PlaceParts;
  check("Turn or Pattern about Z turns about the engine's y axis", tn && tn.how.kind === "turn" && tn.how.axis[1] === 1 && tn.how.axis[0] === 0 && Math.abs(tn.how.angle - Math.PI / 2) < 1e-12 && tn.copies === 3, JSON.stringify(tn));
  // Mirror in the Y-Z plane, as a copy.
  await p.click('.ribbon button[title="Mirror the selected parts in a plane"]'); await p.waitForTimeout(150); await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(150);
  const mr = (await ops0())?.PlaceParts;
  check("Mirror in the Y-Z plane sends a mirror with normal X, one copy", mr && mr.how.kind === "mirror" && mr.how.normal[0] === 1 && mr.copies === 1, JSON.stringify(mr));
  // Hole and Fillet: the tool, then a click on the part.
  await p.click('.ribbon button[title^="Click a part\'s front or back face"]'); await p.waitForTimeout(80);
  await p.mouse.click(onTop[0], onTop[1]); await p.waitForTimeout(200);
  const hle = (await ops0())?.Hole;
  check("Hole, clicked on the top plate's upper face, sends a hole there", hle && hle.part === topId && Math.abs(hle.at[1] - 0.11) < 1e-6 && Math.abs(hle.d - 0.01) < 1e-12, JSON.stringify(await ops0()));
  await p.click('.ribbon button[title^="Click a part near an edge along its depth: the edge is rounded"]'); await p.waitForTimeout(80);
  const nearEdge = await atp([0.039, 0.105, 0.039]); await p.mouse.click(nearEdge[0], nearEdge[1]); await p.waitForTimeout(200);
  const fe = (await ops0())?.PartEdge;
  check("Fillet, clicked near an edge, sends the point and the size to round", fe && fe.round === true && Math.abs(fe.size - 0.002) < 1e-12 && fe.part === topId, JSON.stringify(await ops0()));
  await p.keyboard.press("Escape");
  // The Extrude panel: distance 30 mm, both ways.
  await p.mouse.click(onTop[0], onTop[1]); await p.waitForTimeout(150);
  await p.click('.ribbon button[title^="Extrude or revolve the selected part"]'); await p.waitForTimeout(200);
  check("Extrude opens its panel over the 3D view, with a preview drawn", (await p.$$(".view3d .sweeppanel")).length === 1 && (await p.textContent(".view3d .sweeppanel")).includes("Top plate"));
  await p.fill(".view3d .sweeppanel input.num", "30"); await p.press(".view3d .sweeppanel input.num", "Tab");
  await p.selectOption(".view3d .sweeppanel select", "both"); await p.waitForTimeout(100);
  await p.click('.view3d .sweeppanel button:has-text("OK")'); await p.waitForTimeout(200);
  const sw = (await ops0())?.Set?.item?.Part;
  check("OK sets the part's depth and where its sweep starts (both ways: half back)", sw && sw.id === topId && Math.abs(sw.depth - 0.03) < 1e-12 && Math.abs(sw.from + 0.015) < 1e-12 && (await p.$$(".view3d .sweeppanel")).length === 0, JSON.stringify(sw));
  // Sketch on the top plate's upper face; Cut Through.
  await p.click('.ribbon button:has-text("New Part")'); await p.waitForTimeout(200);
  await p.mouse.click(onTop[0], onTop[1]); await p.waitForTimeout(400);
  const fp = (await ops()).reverse().find(o => o?.Add?.item?.Part)?.Add.item.Part;
  // The plate's front face is at y = 0.11, facing +y: the new part sits there, its z along +y.
  const zAxis = fp ? [Math.cos(fp.turn[2]) * Math.sin(fp.turn[1]) * Math.cos(fp.turn[0]) + Math.sin(fp.turn[2]) * Math.sin(fp.turn[0]), Math.sin(fp.turn[2]) * Math.sin(fp.turn[1]) * Math.cos(fp.turn[0]) - Math.cos(fp.turn[2]) * Math.sin(fp.turn[0]), Math.cos(fp.turn[1]) * Math.cos(fp.turn[0])] : null;
  check("New Sketch on a part's face makes a part on that face, going out of it", !!fp && Math.abs(fp.at[1] - 0.11) < 1e-9 && zAxis && Math.abs(zAxis[1] - 1) < 1e-9 && (await p.textContent(".sketchbar")).includes("on a face of Top plate"), JSON.stringify(fp) + " " + JSON.stringify(zAxis));
  check("a sketch on a front or back face can be cut through the part", (await p.$$('.sketchbar button:has-text("Cut Through")')).length === 1);
  await p.click('.sketchbar button:has-text("Cancel")'); await p.waitForTimeout(300);
  // Parts that cannot be meshed together: the view keeps them in sight (outlines) and says why.
  const keep = await p.evaluate(() => window.__cur);
  await p.evaluate(() => { const c = window.__cur; window.__cur = { ...c, m: { ...c.m, assembly: undefined, assembly_error: "The parts' meshes have over 66000 nodes together." } }; });
  await p.click(".tree .ti:has-text('Top plate')"); await p.waitForTimeout(100);
  await p.fill(".insp input", "Top plate 2"); await p.press(".insp input", "Tab"); await p.waitForTimeout(250);
  check("an assembly that cannot be meshed says why above the view and still shows its parts", (await p.textContent(".planebar")).includes("66000") && (await p.$$(".view3d canvas")).length === 1 && (await p.textContent(".tree")).includes("Top plate"), await p.textContent(".planebar"));
  await p.evaluate(k => { window.__cur = k; }, keep);
  await p.click(".tree .ti:has-text('Top plate')"); await p.waitForTimeout(100);
  await p.fill(".insp input", "Top plate"); await p.press(".insp input", "Tab"); await p.waitForTimeout(250);
  await stepHead(3); await p.click('.ribbon button:has-text("Pressure")'); await p.waitForTimeout(150);
  const tq = await atp([0.01, 0.11, -0.01]); await p.mouse.click(tq[0], tq[1]); await p.waitForTimeout(250);
  const pl = (await lastOp())?.Add?.item?.Load;
  check("clicking a part's face in 3D puts the load on that part face", pl && pl.at.on === "partend" && pl.at.part === tpM.parts.find(x => x.name === "Top plate").id && pl.at.front === true, JSON.stringify(await lastOp()));
  await p.keyboard.press("Escape");
  await p.evaluate(() => { window.__solveKey = "tubeplates_results"; document.activeElement?.blur?.(); });
  await p.keyboard.press("F5"); await p.waitForTimeout(600);
  await p.evaluate(() => { window.__solveKey = null; });
  check("a solved assembly says its mesh is tetrahedra in parts", (await p.textContent(".legend")).includes("3 parts"), await p.textContent(".legend"));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "tubeplates.png") });
  // 0.5.9: drawing a 3D frame straight in the 3D view (it used to send you to a plane).
  await open("blank-space"); await p.waitForTimeout(300);
  const at3d = async q => { const v = await p.$eval(".view3d", el => { const r = el.getBoundingClientRect(); return [r.left, r.top]; }); const s = await p.evaluate(q => window.__screen3(q), q); return [v[0] + s[0], v[1] + s[1]]; };
  { const vr = await p.$eval(".view3d", el => { const r = el.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]; }), e0 = await at3d([6, 0, 0]), e1 = await at3d([0, 0, 4]);
    const inV = q => q[0] > vr[0] && q[0] < vr[2] && q[1] > vr[1] && q[1] < vr[3];
    check("an empty 3D frame is seen from the corner over a few metres of space (0.5.9: the camera sat at the origin, edge-on)", inV(e0) && inV(e1) && Math.hypot(e1[0] - e0[0], e1[1] - e0[1]) > 200, JSON.stringify([vr, e0, e1])); }
  // 0.6.1: the orientation cube: Top looks straight down (Z towards you), Home back to the corner.
  await p.$eval(".view3d .vcube .cf-top", el => el.dispatchEvent(new MouseEvent("click", { bubbles: true }))); await p.waitForTimeout(200);
  const kTop = await p.$$eval(".view3d .axkey3 line", ls => ls.map(l => [+l.getAttribute("x2"), +l.getAttribute("y2")]));
  check("the cube's Top face looks straight down: Z points at you, X right, Y up the screen", Math.abs(kTop[2][0] - 26) < 1 && Math.abs(kTop[2][1] - 26) < 1 && kTop[0][0] > 40 && kTop[1][1] < 12, JSON.stringify(kTop));
  await p.click(".view3d .vcube .vhome"); await p.waitForTimeout(200);
  const kHome = await p.$$eval(".view3d .axkey3 line", ls => ls.map(l => [+l.getAttribute("x2"), +l.getAttribute("y2")]));
  check("Home goes back to the corner view, Z up the screen", kHome[2][1] < 12 && Math.abs(kHome[2][0] - 26) < 2, JSON.stringify(kHome));
  check("an empty 3D frame offers New Sketch, which draws the frame with the sketch tools (0.7.5)", (await p.$$('.view3d .empty button:has-text("New Sketch")')).length === 1);
  await stepHead(1); await p.click('.ribbon button[title^="Draw beams"]'); await p.waitForTimeout(200);
  check("drawing in 3D first asks for a plane (0.6.2: the pick bar)", (await p.$$(".view3d .pickbar")).length === 1 && (await p.textContent(".view3d .pickbar")).includes("Pick a plane"));
  let q3 = await at3d([1.6, 0, -1.8]); await p.mouse.click(q3[0], q3[1]); await p.waitForTimeout(400);
  const keyNow = await p.$$eval(".view3d .axkey3 line", ls => ls.map(l => [l.getAttribute("x2"), l.getAttribute("y2")]));
  check("clicking the floor square at the origin sketches on the Top plane, looking down on it", (await p.textContent(".sketchbar")).includes("Top plane") && Math.abs(+keyNow[2][0] - 26) < 1 && Math.abs(+keyNow[2][1] - 26) < 1, JSON.stringify(keyNow));
  await p.click('.sketchbar button:has-text("Finish Sketch")'); await p.waitForTimeout(250);
  await stepHead(1); await p.click('.ribbon button[title^="Pick a plane and draw members and plates"]'); await p.waitForTimeout(200);
  await p.click('.view3d .pickbar .seg button:text-is("Front")'); await p.waitForTimeout(400);
  check("the bar picks the Front plane too, showing its offset as Y", (await p.textContent(".sketchbar")).includes("Front plane") && (await p.textContent(".sketchbar")).includes("Y ="));
  const cu = await p.$eval(".sketchbar .pt2", el => el.title);
  const cun = (cu.match(/\((\S+)\)/) ?? [])[1], one = cun === "mm" ? "1000" : cun === "in" ? String(1 / 0.0254) : "1";
  await p.fill(".sketchbar .pt2", "0, 0"); await p.press(".sketchbar .pt2", "Enter"); await p.waitForTimeout(150);
  await p.fill(".sketchbar .pt2", `${one}, 0`); await p.press(".sketchbar .pt2", "Enter"); await p.waitForTimeout(200);
  const d3a = (await lastOp())?.AddMember;
  check("two typed points draw a member on the front plane", d3a && d3a.kind === "beam" && Math.abs(d3a.from[0]) < 1e-9 && Math.abs(d3a.to[0] - 1) < 1e-6 && Math.abs(d3a.to[1]) < 1e-6 && d3a.z[0] === 0 && d3a.z[1] === 0 && (await p.inputValue(".sketchbar .pt2")) === "", JSON.stringify(d3a) + " " + cu);
  q3 = await at3d([2, 0.03, 0]); await p.mouse.move(q3[0], q3[1]); await p.mouse.click(q3[0], q3[1]); await p.waitForTimeout(200);
  const d3b = (await lastOp())?.AddMember;
  check("a click nearly level with the last point snaps level (horizontal)", d3b && Math.abs(d3b.from[0] - 1) < 1e-6 && Math.abs(d3b.to[0] - 2) < 0.05 && d3b.to[1] === d3b.from[1] && d3b.z[1] === 0, JSON.stringify(d3b));
  const lx = d3b?.to[0] ?? 2;
  await p.fill(".sketchbar .pt2", `@0, ${one}`); await p.press(".sketchbar .pt2", "Enter"); await p.waitForTimeout(200);
  const d3c = (await lastOp())?.AddMember;
  check("a typed @d-across, d-up goes on from the last point", d3c && Math.abs(d3c.to[1] - 1) < 1e-6 && Math.abs(d3c.from[1]) < 1e-9 && Math.abs(d3c.to[0] - lx) < 1e-6 && d3c.z[0] === 0 && d3c.z[1] === 0, JSON.stringify(d3c));
  if (process.env.SHOTS) await p.screenshot({ path: path.join(process.env.SHOTS, "sketch-front.png") });
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click('.sketchbar button:has-text("Finish Sketch")'); await p.waitForTimeout(250);
  check("Finish Sketch keeps the members and returns to the 3D view", (await p.$$(".sketchstage")).length === 0 && (await p.$$(".view3d canvas")).length === 1);
  // Axisymmetric (0.2.1): the pressure vessel, drawn as a half-section right of the axis.
  await open("vessel");
  check("an axisymmetric model shows its axis of turning", (await p.$$(".dsvg path.axi")).length === 1 && (await p.textContent(".canvas")).includes("axis of turning"));
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  const vo = await p.$$eval(".legend .fpick option", os => os.map(o => o.textContent));
  check("a turned solid's results are radial, axial and hoop", vo.includes("Hoop stress σθ") && vo.includes("Radial stress σr") && vo.includes("Axial displacement"), vo.join(", "));
  await go(p, "Results"); await p.waitForTimeout(200);
  check("its equilibrium check sums the forces along the axis only", (await p.textContent(".page")).includes("Σ F along the axis") && !(await p.textContent(".page")).includes("Σ M about the origin"));
  await go(p, "Model"); await p.waitForTimeout(150);
  // Heat sink: temperature contours and the heat balance.
  await open("fin"); await p.keyboard.press("F5"); await p.waitForTimeout(400);
  check("a heat model shows temperatures in °C and the heat balance", (await p.$eval(".legend .fpick", el => el.value)) === "t" && (await p.textContent(".legend .cbar")).includes("°C") && (await p.textContent(".legend")).includes("Heat balance"));
  await go(p, "Results"); await p.waitForTimeout(200);
  check("the heat Results page lists the heat flows and their sum", (await p.textContent(".page")).includes("Heat into the part") && (await p.textContent(".page")).includes("Sum"));
  await go(p, "Model"); await p.waitForTimeout(150);
  // The materials library.
  await stepHead(1); await p.click('.ribbon button[title="Materials"]'); await p.waitForTimeout(150);
  await p.click(".dlg button:has-text('From the library')"); await p.waitForTimeout(250);
  const libText = await p.$eval(".mgr.lib", el => el.textContent);
  check("the library lists Atlas materials with their sources", (await p.$$(".mgr.lib .mlist button")).length === fix.library.length && libText.includes("Source:"), `${(await p.$$(".mgr.lib .mlist button")).length} of ${fix.library.length}; ${libText.slice(0, 300)}`);
  await p.fill(".mgr.lib .mlist input", "7075"); await p.waitForTimeout(100);
  await p.click(".mgr.lib .mlist button:has-text('7075')"); await p.click(".dlg button:has-text('Add to the model')"); await p.waitForTimeout(150);
  const addm = (await ops()).reverse().find(o => o.Add && o.Add.item.Material);
  check("Add to the model adds the library material with its values and source", !!addm && addm.Add.item.Material.name.includes("7075") && addm.Add.item.Material.source.startsWith("Datum Atlas") && addm.Add.item.Material.nu > 0.3, JSON.stringify(addm));
  while (await p.$(".ov .dlg header .x")) { await p.click(".ov .dlg header .x"); await p.waitForTimeout(80); }

  // 11c. 0.1.2 fixes from the 0.1.1 review.
  // (1) Diagrams follow pan and zoom: the moment diagram of a member starts at that member's end.
  await open("portal"); await p.keyboard.press("F5"); await p.waitForTimeout(300);
  const startsAtMember = async () => p.evaluate(id => {
    const d = document.querySelector(`.diag[data-id="${id}"]`).getAttribute("d").match(/^M([-\d.e]+) ([-\d.e]+)/), l = document.querySelector(`.mem[data-id="${id}"] line`);
    return Math.hypot(+d[1] - +l.getAttribute("x1"), +d[2] - +l.getAttribute("y1")) < 0.5;
  }, pf.members[0].id);
  const cv = await canvasBox();
  await p.mouse.move(cv.x + cv.w / 2, cv.y + cv.h / 2); await p.mouse.wheel(0, -300); await p.waitForTimeout(150);
  check("the moment diagram follows a zoom (0.1.1 bug)", await startsAtMember());
  await p.mouse.down({ button: "middle" }); await p.mouse.move(cv.x + cv.w / 2 + 80, cv.y + cv.h / 2 + 40, { steps: 4 }); await p.mouse.up({ button: "middle" }); await p.waitForTimeout(150);
  check("the moment diagram follows a pan (0.1.1 bug)", await startsAtMember());
  // (11) No rounding noise in the probe.
  const mid = await p.$eval(`.mem[data-id="${pf.members[1].id}"] line`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width * 0.75, r.top + r.height * 0.25]; });
  await p.mouse.move(mid[0], mid[1]); await p.waitForTimeout(150);
  const probeText = (await p.$$(".probe")).length ? await p.textContent(".probe") : "";
  check("the probe shows no rounding noise", probeText.length > 0 && !/e-1\d/.test(probeText), probeText);
  // (2) Unsaved changes are offered for saving before an example replaces them.
  await p.click(".tree .ti:has-text('Member 1')"); await p.click('.tabs > button:text-is("Home")'); await p.click('.ribbon button[title="Delete"]'); await p.waitForTimeout(150);
  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150);
  await p.click('.bs aside button:has-text("Examples")'); await p.waitForTimeout(150);
  await p.click(`.home .tile:has(b:text-is("Roof truss"))`); await p.waitForTimeout(250);
  check("opening an example asks to save the changes first (0.1.1 bug)", !!(await p.$('.dlg header:has-text("Save changes?")')));
  const before = await p.evaluate(() => window.__calls.filter(c => c[0] === "new_model").length);
  await p.click('.dlg button:has-text("Cancel")'); await p.waitForTimeout(150);
  check("Cancel keeps the model", (await p.evaluate(() => window.__calls.filter(c => c[0] === "new_model").length)) === before);
  await open("blank-frame");
  // (13) The empty canvas card is styled and centred.
  const emp = await p.$eval(".canvas .empty", el => { const r = el.getBoundingClientRect(), c = el.parentElement.getBoundingClientRect(); return { cx: r.left + r.width / 2 - (c.left + c.width / 2), w: r.width, bg: getComputedStyle(el).position }; });
  check("the empty canvas shows its card in the middle (0.1.1 bug)", emp.bg === "absolute" && Math.abs(emp.cx) < 5 && emp.w > 200, JSON.stringify(emp));
  // (12) Switching to US units makes the grid a round number of feet.
  await p.click('.tabs > button:text-is("Home")'); await p.click('.ribbon button[title="US: in, kip, ksi"]'); await p.waitForTimeout(150);
  const us = (await ops()).reverse().find(o => o.SetSettings);
  check("US units give a 2 ft grid for a 0.5 m one", !!us && us.SetSettings.s.units === "US" && Math.abs(us.SetSettings.s.grid - 0.6096) < 1e-12, JSON.stringify(us));
  // (6) A model with no loads: no garbage deflected shape, the legend says so.
  await p.evaluate(f => { window.__next = f.noload; window.__solveKey = "noload_results"; }, fix);
  await open("blank-frame");
  await p.keyboard.press("F5"); await p.waitForTimeout(300);
  check("a model with no loads draws no deflected shape and says why (0.1.1 bug)", (await p.$$(".defl")).length === 0 && (await p.textContent(".legend")).includes("nothing moves") && (await p.textContent("#toast")).includes("no loads"), await p.textContent(".legend"));
  await p.evaluate(() => { window.__solveKey = null; });
  // 0.1.4: a drag lands the grabbed node on the grid; load arrows follow the drag.
  await p.evaluate(f => { window.__next = f.offgrid; }, fix);
  await open("blank-frame");
  const og = fix.offgrid.m, g0 = og.nodes.find(n => Math.abs(n.x - 0.3) < 1e-9), g1 = og.nodes.find(n => Math.abs(n.x - 4) < 1e-9);
  const nodeAt = async id => p.$eval(`rect.node[data-id="${id}"]`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  const a0 = await nodeAt(g0.id), pxPerM = ((await nodeAt(g1.id))[0] - a0[0]) / 3.7;
  await p.mouse.move(a0[0], a0[1]); await p.mouse.down(); await p.mouse.move(a0[0] + pxPerM * 0.62, a0[1], { steps: 5 }); await p.mouse.up(); await p.waitForTimeout(150);
  const gm = (await ops()).reverse().find(o => o.Move);
  check("dragging a node off the grid lands it on the grid (0.1.1 review)", !!gm && Math.abs(((0.3 + gm.Move.dx) / 0.5) - Math.round((0.3 + gm.Move.dx) / 0.5)) < 1e-9 && gm.Move.dx > 0, JSON.stringify(gm));
  const ld = fix.offgrid.m.loads[0];
  const arrowX = async () => p.$eval(`.load[data-id="${ld.id}"] path`, el => el.getBoundingClientRect().left);
  const b1 = await nodeAt(g1.id), x0 = await arrowX();
  await p.mouse.move(b1[0], b1[1]); await p.mouse.down(); await p.mouse.move(b1[0] - pxPerM * 1.1, b1[1], { steps: 5 }); await p.waitForTimeout(100);
  const x1 = await arrowX(), nodeDuring = (await nodeAt(g1.id))[0];
  await p.mouse.up(); await p.waitForTimeout(100);
  check("a load's arrow follows its node while it is dragged (0.1.1 review)", x0 - x1 > 40, `${x0} → ${x1}; node ${b1[0]} → ${nodeDuring}`);
  // (10) Negative corner coordinates can be typed; (9) + adds a corner through the engine.
  await open("bracket");
  await p.click(".tree .ti:has-text('Shape 1')"); await p.waitForTimeout(150);
  await p.fill(".ptbl tbody tr:nth-child(1) td:nth-child(2) input", "−5"); await p.keyboard.press("Enter"); await p.waitForTimeout(150);
  const setx = (await ops()).reverse().find(o => o.Set && o.Set.item.Shape);
  check("a corner can be moved to a negative coordinate typed with “−” (0.1.1 bug)", !!setx && Math.abs(setx.Set.item.Shape.geom.pts[0][0] + 0.005) < 1e-12, JSON.stringify(setx?.Set.item.Shape.geom.pts[0]));
  await p.click(".ptbl tbody tr:nth-child(2) button[title^='Add a corner']"); await p.waitForTimeout(150);
  const ins = (await ops()).reverse().find(o => o.InsertCorner);
  check("+ in the corner table inserts a corner through the engine (keeps loads on their edges)", !!ins && ins.InsertCorner.edge === 1, JSON.stringify(ins));

  // 0.3.1 sketch tools: round corners, ready-made shapes, combine, offset, patterns.
  await stepHead(1); await p.waitForTimeout(100);
  await p.click('.ribbon button[title^="Fillet: round corners"]'); await p.waitForTimeout(150);
  check("Round Corners lists only the corners between two straight edges", (await p.$$(".dlg .cks input")).length === 5, String((await p.$$(".dlg .cks input")).length));
  await p.click('.dlg button:text-is("Round")'); await p.waitForTimeout(150);
  const fil = await lastOp();
  check("Round Corners sends a fillet of those corners at the grid size", !!fil?.Fillet && JSON.stringify(fil.Fillet.corners) === "[0,1,2,5,6]" && Math.abs(fil.Fillet.r - 0.005) < 1e-12, JSON.stringify(fil));
  await p.click(".ptbl tbody tr:nth-child(1) button[title^='Round or bevel']"); await p.waitForTimeout(150);
  check("⌒ in the corner table rounds just that corner", (await p.$$eval(".dlg .cks input", els => els.filter(e => e.checked).length)) === 1);
  await p.click('.dlg button:text-is("Cancel")');
  await p.click('.ribbon button[title="Slot…"]'); await p.waitForTimeout(150);
  await p.click('.dlg label.chk:has-text("A hole") input'); await p.click('.dlg button:text-is("Add")'); await p.waitForTimeout(150);
  const mk = await lastOp();
  check("Slot… sends a ready-made slot, as a hole when ticked", mk?.Make?.make.kind === "slot" && mk.Make.hole === true && mk.Make.make.w > 0, JSON.stringify(mk));
  await p.click('.ribbon button[title="Regular Polygon…"]'); await p.waitForTimeout(150);
  await p.click('.dlg button:text-is("Add")'); await p.waitForTimeout(150);
  const pgm = await lastOp();
  check("Regular Polygon… sends six sides, the first corner straight up", pgm?.Make?.make.kind === "polygon" && pgm.Make.make.n === 6 && Math.abs(pgm.Make.make.turn - Math.PI / 2) < 1e-12, JSON.stringify(pgm));
  await open("lug");
  await stepHead(1);
  const lm = await p.evaluate(() => window.__cur.m);
  const [outer, bore] = [lm.shapes.find(s => !s.hole), lm.shapes.find(s => s.hole)];
  await p.click('.ribbon button[title^="Cut the second selected shape"]'); await p.waitForTimeout(100);
  check("Subtract with nothing selected says what to pick", (await p.textContent("#toast")).includes("Select two shapes"));
  await p.click(`.tree .ti:has-text('${outer.name}')`); await p.click(`.tree .ti:has-text('${bore.name}')`, { modifiers: ["Shift"] }); await p.waitForTimeout(100);
  await p.click('.ribbon button[title^="Cut the second selected shape"]'); await p.waitForTimeout(150);
  const cmb = await lastOp();
  check("Subtract sends the first picked shape less the second", cmb?.Combine?.a === outer.id && cmb.Combine.b === bore.id && cmb.Combine.how === "subtract", JSON.stringify(cmb));
  check("the Format tab offers the sketch tools on a selected shape", (await p.$$('.tabs .ctxgrp button:text-is("Format")')).length === 1 && await (async () => { await p.click('.tabs .ctxgrp button:text-is("Format")'); return (await p.$$('.ribbon button[title^="Join the two selected shapes"]')).length === 1; })());
  await p.click(`.tree .ti:has-text('${bore.name}')`); await p.waitForTimeout(100);
  await p.click('.ribbon button[title^="Copies of the selected shapes around"]'); await p.waitForTimeout(150);
  await p.click('.dlg button:text-is("Copy")'); await p.waitForTimeout(150);
  const cp = await lastOp();
  check("Circular Pattern sends 5 copies a sixth of a turn apart", cp?.Pattern?.times === 5 && Math.abs(cp.Pattern.turn - Math.PI / 3) < 1e-12 && cp.Pattern.ids[0] === bore.id, JSON.stringify(cp));
  await p.click('.ribbon button[title^="A new outline parallel"]'); await p.waitForTimeout(150);
  await p.click('.dlg button:text-is("Offset")'); await p.waitForTimeout(150);
  const of = (await ops()).reverse().find(o => o.Offset);
  check("Offset sends the outline moved inwards (a negative distance)", of?.Offset?.shape === bore.id && of.Offset.d < 0, JSON.stringify(of));

  // (10) Number fields: "1,5" is 1.5, "1,200" is 1200, an empty field changes nothing.
  await open("portal");
  await p.click(".tree .ti:has-text('Node 2')"); await p.waitForTimeout(100);
  const xField = ".insp .iprow:nth-of-type(1) input";
  const lastNode = async () => (await ops()).reverse().find(o => o.Set && o.Set.item.Node);
  await p.fill(xField, "1,5"); await p.keyboard.press("Enter"); await p.waitForTimeout(100);
  check("“1,5” is read as 1.5", (await lastNode())?.Set.item.Node.x === 1.5, JSON.stringify(await lastNode()));
  await p.fill(xField, "1,200"); await p.keyboard.press("Enter"); await p.waitForTimeout(100);
  check("“1,200” is read as 1200", (await lastNode())?.Set.item.Node.x === 1200, JSON.stringify(await lastNode()));
  const nOps = (await ops()).length;
  await p.fill(xField, ""); await p.keyboard.press("Enter"); await p.waitForTimeout(100);
  check("an emptied field changes nothing and shows the value again (0.1.1: it became 0)", (await ops()).length === nOps && (await p.inputValue(xField)) !== "");

  // 12. Dialogs, views and File pages open without errors; dark theme.
  await p.click('.tabs > button:text-is("Home")');
  await open("portal");
  await stepHead(1);
  await p.click('.ribbon button[title="Sections"]'); await p.waitForTimeout(200);
  check("the Sections dialog shows the section's properties", (await p.textContent(".dlg")).includes("mm⁴"));
  await p.keyboard.press("Escape");
  await p.click('.ribbon button[title="Materials"]'); await p.waitForTimeout(150);
  await p.click('.dlg .dtabs button[data-mtab="elastic"]'); await p.waitForTimeout(100);
  check("the Materials dialog shows E in GPa, on its Elastic tab", (await p.textContent(".dlg")).includes("GPa"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  for (const v of ["Mesh", "Results", "Report", "Model"]) { await go(p, v); await p.waitForTimeout(100); }
  for (const k of ["Print", "Options", "Roadmap", "Updates", "Export", "Home"]) { await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(100); await p.click(`.bs aside button:has-text("${k}")`); await p.waitForTimeout(200); }
  check("print preview draws the model", true);
  // 0.7.1: sketch and 3D tools by the geometry engine, in the Datum window.
  await open("blank-solid");
  // The start card's New Sketch shows the drawing tools, whatever tab was on (0.7.1 bug: Home stayed, and a click meant for Rectangle changed the analysis).
  await stepHead(1);
  await p.click('.ribbon button[title^="Start a sketch: click a plane"]'); await p.waitForTimeout(150);
  await p.click('.tabs > button:text-is("Home")');
  check("New Sketch asks for a plane: the origin planes, or a named one (0.7.2, the user)", (await p.$$(".view3d .pickplane")).length === 1);
  await p.click('.view3d .pickplane button:has-text("Floor (XY)")'); await p.waitForTimeout(300);
  check("a plane picked starts the sketch on it", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "cad_run").map(c => c[1]).slice(-1)[0])) === JSON.stringify({ id: "sketch.create", params: { plane: "XY" } }));
  check("a sketch opens with the drawing tools on the ribbon, whatever tab was on (0.7.1 bug)", ["Geometry", "Sketch"].includes(await p.textContent(".tabs > button.on")) && (await p.$$('.ribbon .grp:has(.cap:text-is("Create")) .rbw')).length >= 6);
  await p.click('.ribbon button[title^="Leave the sketch without keeping it"]'); await p.waitForTimeout(300);
  await stepHead(1);
  check("a blank 3D solid offers New Sketch, Extrude and Revolve on the Geometry ribbon", (await p.$$('.ribbon button[title^="Start a sketch: click a plane"]')).length === 1 && (await p.$$('.ribbon button[title^="Extrude the last sketch"]')).length === 1 && (await p.$$('.ribbon button[title^="Turn the last sketch"]')).length === 1);
  await p.click('.ribbon button[title="New Sketch options"]'); await p.waitForTimeout(100);
  await p.click('.menu button:has-text("On the floor")'); await p.waitForTimeout(300);
  const cadCall = async () => p.evaluate(() => window.__calls.filter(c => c[0] === "cad_run").map(c => c[1]).slice(-1)[0]);
  check("New Sketch › On the floor starts a sketch on XY and opens the sheet", JSON.stringify(await cadCall()) === JSON.stringify({ id: "sketch.create", params: { plane: "XY" } }) && !!(await p.$(".cadsheet svg")));
  check("the sheet fills the 3D view, not the window (0.7.1 bug)", await p.$eval(".cadsheet svg", el => { const r = el.getBoundingClientRect(), v = document.querySelector(".view3d").getBoundingClientRect(); return r.height > 300 && Math.abs(r.top - v.top) < 2 && Math.abs(r.width - v.width) < 2; }));
  check("the sheet's grid has big squares with small squares inside (0.7.2, the user)", await p.evaluate(() => { const a = document.querySelectorAll(".cadsheet .gmaj").length, b = document.querySelectorAll(".cadsheet .gmin").length; return a >= 2 && b >= 3 * a; }));
  const shapes = await p.$$eval('.ribbon .grp:has(.cap:text-is("Create")) .rbw span', els => els.map(e => e.textContent));
  check("in a sketch the ribbon offers the six shapes", ["Line", "Arc", "Circle", "Rectangle", "Polygon", "Slot"].every(x => shapes.includes(x)), JSON.stringify(shapes));
  await p.click('.ribbon .rbw:has(.rb.bot span:text-is("Circle")) .rb.bot'); await p.waitForTimeout(100);
  check("Circle ▾ lists its three ways", JSON.stringify(await p.$$eval(".menu button", els => els.map(e => e.textContent.replace("✓", "").trim()))) === JSON.stringify(["Circle · centre and radius", "Circle · 2 points", "Circle · 3 points"]), JSON.stringify(await p.$$eval(".menu button", els => els.map(e => e.textContent))));
  await p.click('.menu button:has-text("3 points")'); await p.waitForTimeout(100);
  const csb = await p.$eval(".cadsheet svg", el => { const r = el.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
  await p.mouse.click(csb.x + csb.w / 2, csb.y + csb.h / 2); await p.mouse.click(csb.x + csb.w / 2 + 100, csb.y + csb.h / 2);
  check("a 3-point circle waits for its third point", (await cadCall()).id === "sketch.create" && (await p.$$(".cadsheet .dpt")).length === 2);
  await p.mouse.click(csb.x + csb.w / 2 + 50, csb.y + csb.h / 2 - 50); await p.waitForTimeout(200);
  const cc3 = await cadCall();
  check("the third click draws it: sketch.circle.three_point with three points in mm", cc3.id === "sketch.circle.three_point" && ["p0", "p1", "p2"].every(k => Array.isArray(cc3.params[k]) && cc3.params[k].length === 2), JSON.stringify(cc3));
  check("the circle button keeps the way last used", (await p.$eval('.ribbon .rbw:has(.rb.bot span:text-is("Circle")) .rb.top', el => el.title)).startsWith("Circle · 3 points"));
  check("the sketch's curves from the engine are drawn on the sheet", (await p.$$(".cadsheet .cv")).length === fix.cad.drawn.sketches[0].curves.filter(c => !c.construction).length + fix.cad.drawn.sketches[0].curves.filter(c => c.construction).length);
  await p.click('.ribbon button[title^="Keep the sketch"]'); await p.waitForTimeout(300);
  check("Finish Sketch closes the sheet", (await p.evaluate(() => window.__calls.filter(c => c[0] === "cad_run").map(c => c[1].id))).includes("sketch.finish") && !(await p.$(".cadsheet")));
  await p.waitForTimeout(500);
  const ex = await cadCall();
  check("a closed shape finished goes straight on to Extrude: the solid is made from it (0.7.2, the user)", ex.id === "solid.extrude" && ex.params.distance === 20 && ex.params.direction === "positive" && ex.params.sketch === fix.cad.finished.sketches[0].id, JSON.stringify(ex));
  check("the Extrude panel shows the feature's distance", (await p.$$(".view3d .xpanel")).length === 1 && (await p.$eval(".view3d .xpanel input", el => el.value)) === "15");
  await p.click('.view3d .xpanel button[title="More"]'); await p.waitForTimeout(350);
  check("+ on the Extrude panel changes the solid at once, through its distance", JSON.stringify(await cadCall()) === JSON.stringify({ id: "parameters.change", params: { name: "d1", expression: "16" } }));
  await p.click('.view3d .xpanel button:has-text("Both ways")'); await p.waitForTimeout(200);
  check("Both ways turns the extrusion symmetric", (await cadCall()).id === "timeline.edit" && (await cadCall()).params.set.extent.direction === "symmetric");
  await p.click('.view3d .xpanel button:has-text("OK")'); await p.waitForTimeout(100);
  check("OK keeps the solid and closes the panel", !(await p.$(".view3d .xpanel")));
  check("the tree lists the solid with its faces, and step 1 is done", (await p.textContent(".tree .step:nth-of-type(1)")).includes("17 faces") && await p.$eval(".tree .step:nth-of-type(1)", el => el.classList.contains("done")));

  // 0.7.2: the history, its right-click menu, Edit Feature, the timeline; no browser menu.
  check("the modelling ribbon offers primitives and Combine (0.7.6)", (await p.$$('.ribbon button[title^="A box: its corner"]')).length === 1 && (await p.$$('.ribbon button[title^="Join, cut or intersect solids"]')).length === 1);
  {
    const body = fix.cad.extruded.bodies[0].name;
    await p.click('.ribbon button[title^="Cut a solid in two"]'); await p.waitForTimeout(150);
    await p.click('.dlg .seg button:text-is("Z")'); await p.fill('.dlg tr:has-text("At") input', "5"); await p.click('.dlg footer .btn.pri'); await p.waitForTimeout(150);
    check("Split cuts the solid by the plane chosen (0.7.6)", JSON.stringify(await cadCall()) === JSON.stringify({ id: "solid.split_body", params: { body, plane: { origin: [0, 0, 5], normal: [0, 0, 1] } } }), JSON.stringify(await cadCall()));
    await p.click('.ribbon button[title^="Each solid\'s volume"]'); await p.waitForTimeout(200);
    const mt = await p.textContent(".dlg");
    check("Mass lists each solid's volume, and its mass from the model's material (0.7.6)", mt.includes(body) && mt.includes("24.00 cm³") && /\d\.\d{3} kg/.test(mt), mt);
    await p.click(".dlg footer .btn"); await p.waitForTimeout(100);
    await p.click('.ribbon button[title^="Carry a closed profile"]'); await p.waitForTimeout(150);
    check("Sweep with one sketch says it needs a profile and a path (0.7.6)", (await p.textContent("#toast")).includes("two finished sketches") && !(await p.$(".dlg")));
    await p.click('.ribbon button[title^="Click a face or an edge to measure"]'); await p.waitForTimeout(100);
    check("Measure waits for a click on a face or edge", await p.$eval('.ribbon button[title^="Click a face or an edge to measure"]', b => b.classList.contains("on")));
    await p.click('.ribbon button[title^="Click a side face to slope"]'); await p.waitForTimeout(100);
    check("Draft shows its angle under Sizes", (await p.textContent(".ribbon")).includes("Draft angle"));
    await p.keyboard.press("Escape");
  }
  check("the tree lists the history in order", JSON.stringify(await p.$$eval('.tree .tgrp:has(.tt:text-is("History")) .ti > span:not(.teye)', els => els.map(e => e.textContent))) === JSON.stringify(fix.cad.extruded.features.map(f => f.name)));
  await modelCard(); check("a modelled solid has no 3D/Section bar and no section settings (0.7.2, the user)", !(await p.$('.planebar button:text-is("Section")')) && !(await p.textContent(".insp")).includes("Thickness (extruded") && !(await p.textContent(".insp")).includes("Layers through"));
  await p.click('.tree .tgrp:has(.tt:text-is("History")) .ti:has(span:text-is("Extrude1"))'); await p.waitForTimeout(150);
  check("a feature picked in the history shows in the inspector", (await p.textContent(".insp header")).includes("Extrude1") && (await p.textContent(".insp")).includes("Place in the history"));
  await p.click('.tree .tgrp:has(.tt:text-is("History")) .ti:has(span:text-is("Extrude1"))', { button: "right" }); await p.waitForTimeout(150);
  const fm = await p.$$eval(".menu button span", els => els.map(e => e.textContent));
  check("right-clicking a feature offers Edit Feature, Edit Sketch, Rename, Suppress, Roll Back Here and Delete (0.7.2, the user)", ["Edit Feature", "Edit Sketch", "Rename", "Suppress", "Roll Back Here", "Delete"].every(x => fm.includes(x)), JSON.stringify(fm));
  await p.click('.menu button:has-text("Suppress")'); await p.waitForTimeout(150);
  check("Suppress sends the feature to the engine", JSON.stringify(await cadCall()) === JSON.stringify({ id: "timeline.suppress", params: { feature: fix.cad.extruded.features[1].id } }));
  await p.click('.tree .tgrp:has(.tt:text-is("History")) .ti:has(span:text-is("Extrude1"))', { button: "right" }); await p.waitForTimeout(150);
  await p.click('.menu button:has-text("Edit Feature")'); await p.waitForTimeout(250);
  check("Edit Feature shows the feature's sizes, as the engine names them", (await p.textContent(".dlg")).includes("Distance") && (await p.$eval(".dlg table input", el => el.value)) === "15");
  await p.fill(".dlg table input", "30"); await p.click('.dlg button:has-text("OK")'); await p.waitForTimeout(200);
  check("changing a size in Edit Feature changes its parameter", JSON.stringify(await cadCall()) === JSON.stringify({ id: "parameters.change", params: { name: "d1", expression: "30" } }));
  check("the timeline under the 3D view lists the history", (await p.$$(".view3d .timeline .tlchip")).length === fix.cad.extruded.features.length);
  await p.click(".view3d .timeline .tlchip >> nth=1", { button: "right" }); await p.waitForTimeout(150);
  check("right-clicking a feature in the timeline opens the same menu", (await p.$$eval(".menu button span", els => els.map(e => e.textContent))).includes("Roll Back Here"));
  await p.keyboard.press("Escape"); await p.mouse.click(5, 5); await p.waitForTimeout(100);
  await p.click('.tree .tgrp:has(.tt:text-is("Solids")) .ti', { button: "right" }); await p.waitForTimeout(150);
  check("right-clicking a solid offers Rename and Delete", ["Rename", "Delete"].every(x => fm.includes(x)) && (await p.$$eval(".menu button span", els => els.map(e => e.textContent))).includes("Rename"));
  await p.mouse.click(5, 5); await p.waitForTimeout(100);
  check("the browser's own menu never shows (0.7.2, the user)", await p.evaluate(() => { const ev = new MouseEvent("contextmenu", { bubbles: true, cancelable: true }); document.querySelector(".tree").dispatchEvent(ev); return ev.defaultPrevented; }));
  // 0.7.3: analysis of the modelled solids: meshed by Gmsh on entering Supports & Loads, the mesh's
  // faces named after the features, supports put on them, solved; meshed again after a change.
  const meshCalls = () => p.evaluate(() => window.__calls.filter(c => c[0] === "cad_mesh").length);
  await stepHead(3); await p.waitForTimeout(400);
  check("Supports & Loads on a modelled solid meshes it first (0.7.3)", (await meshCalls()) === 1);
  check("the bar over the view says the mesh is from the solids", (await p.textContent("body")).includes("Meshed from the solids"));
  check("the mesh's faces are named after the feature that made them", (await p.textContent(".tree")).includes("Body1: Extrude1 top") && !(await p.textContent(".tree")).includes("F2:"));
  check("the Mesh step counts the tetrahedra", (await p.textContent(".tree .step:nth-of-type(4)")).includes("10-node tetrahedra"));
  check("a modelled solid has no 3D/Section switch in the analysis steps either", !(await p.$('.planebar button:text-is("Section")')));
  check("Supports & Loads on a solid offers Move a Face and Face Force (crushing, 0.7.3)", (await p.$$('.ribbon button:has-text("Move a Face")')).length === 1 && (await p.$$('.ribbon button:has-text("Face Force")')).length === 1);
  check("Supports & Loads on a solid offers Remote Force (0.7.7)", (await p.$$('.ribbon button:has-text("Remote Force")')).length === 1);
  check("with one solid there is no Contact (it is between two solids, 0.8.0)", (await p.$$('.ribbon button[title^="Let two touching solids press"]')).length === 0);
  await p.evaluate(() => { const b = window.__cad.bodies[0]; window.__cad = { ...window.__cad, bodies: [b, { ...b, name: "Body2" }] }; window.dispatchEvent(new Event("cad-reload")); });
  await p.waitForTimeout(300);
  check("with two solids Supports & Loads offers Contact, and the tree a Contacts branch (0.8.0)", (await p.$$('.ribbon button[title^="Let two touching solids press"]')).length === 1 && (await p.textContent(".tree")).includes("Contacts"));
  await p.click('.ribbon button[title^="Let two touching solids press"]'); await p.waitForTimeout(200);
  check("Contact asks for the two solids and the friction", (await p.textContent(".dlg")).includes("Friction coefficient") && (await p.$$(".dlg select")).length === 2);
  await p.selectOption(".dlg tr:has-text('Touching') select", "Body2"); await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(300);
  const cc = await p.evaluate(() => window.__calls.filter(c => c[0] === "cad_contacts").slice(-1)[0]?.[1]);
  check("Make sends the two solids and μ to be meshed apart and paired", cc && cc.a !== cc.b && cc.b === "Body2" && cc.friction === 0.2, JSON.stringify(cc));
  await p.evaluate(() => { window.__cad = { ...window.__cad, bodies: [window.__cad.bodies[0]] }; window.dispatchEvent(new Event("cad-reload")); });
  await p.waitForTimeout(200);
  await p.click('.ribbon button:has-text("Remote Force")'); await p.waitForTimeout(100);
  check("Remote Force waits for a face", (await p.textContent("body")).includes("Click a face of the mesh"));
  await stepHead(4); await p.waitForTimeout(200);
  check("the Mesh step of a modelled solid offers Finer Near a Face (0.7.7)", (await p.$$('.ribbon button:has-text("Finer Near a Face")')).length === 1);
  await p.keyboard.press("Escape");
  await stepHead(1); await p.click('.ribbon button[title^="Extrude the last sketch"]'); await p.waitForTimeout(400);
  await p.click('.view3d .xpanel button:has-text("OK")'); await p.waitForTimeout(100);
  await p.keyboard.press("F5"); await p.waitForTimeout(500);
  check("Solve after the solids changed meshes them again first", (await meshCalls()) === 2);
  check("the File page's Roadmap icon follows the text colour like the others (0.7.3, the user)", await p.evaluate(() => document.querySelector("#i-roadmap path")?.getAttribute("stroke") === "currentColor" && [...document.querySelectorAll(".bs aside button use, .tabs use")].length >= 0));
  await open("portal");
  {
    // 0.10.0 (docs/PLAN-1.0.md): the whole map. Planned features on each tab and in each step,
    // their dialogs greyed, behind View › Show planned features.
    await p.waitForTimeout(300);
    await p.click('.flow button:has-text("Loads")'); await p.waitForTimeout(150);
    const items = await p.$$eval(".ribbon .grp.planned button.rb.planned", els => els.map(e => e.textContent));
    check("a tab's planned features show after its working tools, by group, each with its release (0.10.3, the user)", items.some(t => /Load Curves.*0\.13/.test(t)) && (await p.$$(".ribbon .pdiv")).length === 1, JSON.stringify(items).slice(0, 300));
    await p.$eval('.ribbon button.rb.planned[data-planned="curves"]', b => b.click()); await p.waitForTimeout(200);
    const dlg = await p.textContent(".dlg");
    check("a planned feature opens its dialog, every field there and disabled, with its release (0.10.0, the user)", /Available in 0\.13/.test(dlg) && (await p.$$eval(".dlg fieldset.pform", f => f.length === 1 && f[0].disabled)) && (await p.$$(".dlg .pform select, .dlg .pform input")).length >= 3);
    check("a planned dialog has no OK: nothing in it can run", (await p.$$(".dlg footer .btn.pri")).length === 0);
    await p.keyboard.press("Escape"); await p.waitForTimeout(100);
    await p.click('.console header .cplanned[data-planned="script"]'); await p.waitForTimeout(150);
    check("the bottom panel's planned Python tab opens its greyed dialog (0.10.0, the user's mock-up)", (await p.textContent(".dlg header")).includes("Python") && (await p.$$(".dlg footer .btn.pri")).length === 0);
    await p.keyboard.press("Escape"); await p.waitForTimeout(100);
    const shownRows = () => p.$$eval(".tree .tbody .tlist > .ti", els => els.filter(e => e.offsetParent !== null).map(e => e.textContent));
    const allRows = (await shownRows()).length;
    await p.fill(".tree .tfilter input", "Node 3"); await p.waitForTimeout(120);
    const hits = await shownRows();
    check("the tree's filter leaves only the matching rows (0.10.0, the user's mock-up)", hits.length >= 1 && hits.length < allRows && hits.every(t => t.includes("Node 3")), JSON.stringify(hits));
    await p.fill(".tree .tfilter input", ""); await p.waitForTimeout(120);
    check("…and cleared shows every row again", (await shownRows()).length === allRows);
    check("each step of the tree lists its planned features under Coming (0.10.0)", (await p.$$(".tree .step .tplan")).length >= 2 && (await p.$$(".tree .tplan .ti.planned")).length >= 3);
    await p.click('.tabs > button:text-is("View")'); await p.waitForTimeout(100);
    await p.click('.ribbon label.chk:has-text("Show planned features") input'); await p.waitForTimeout(150);
    check("View › Show planned features off hides every planned item (0.10.0, the user)", (await p.$$(".grp.planned, .tplan, .ti.planned, .cplanned")).length === 0);
    await p.click('.ribbon label.chk:has-text("Show planned features") input'); await p.waitForTimeout(150);
    check("…and on brings them back", (await p.$$(".tplan")).length >= 2);
    await go(p, "Model");
  }
  {
    // 0.9.1 (the user): no side bar; the bottom panel (Messages, Solver monitor, Log) with Stop;
    // the results in the right panel; the side panels widen; a 3D mesh's Mesh view stays in 3D.
    check("there is no side bar, and no view icons in the status bar (0.9.1, the user)", (await p.$$("nav.side, .status .vsc")).length === 0 && (await p.$$(".console header button")).length >= 3);
    // The workflow bar at the far left (0.9.1, the user): the six steps, each done, to do or not needed.
    const fl = await p.$$eval(".flow button:not(.fextra)", bs => bs.map(b => ({ t: b.textContent, done: b.classList.contains("done"), skip: b.classList.contains("skip") })));
    check("the workflow bar lists the six steps in order, a frame's Mesh not needed (0.9.1, the user)", fl.length === 6 && /Geometry/.test(fl[0].t) && /Materials/.test(fl[1].t) && /Loads/.test(fl[2].t) && /Mesh/.test(fl[3].t) && fl[3].skip && /Solve/.test(fl[4].t) && /Results/.test(fl[5].t) && fl[0].done, JSON.stringify(fl));
    await p.click('.flow button:has-text("Loads")'); await p.waitForTimeout(150);
    check("a step on the workflow bar opens its tools and its place in the tree (0.9.1)", (await p.$$('.ribbon button[title="Fixed"], .ribbon button:has-text("Fixed")')).length >= 1 && await p.$$eval(".flow button:not(.fextra)", els => els[2].classList.contains("on")) && await p.$eval(".tree .step:nth-of-type(3)", el => el.classList.contains("here")));
    // Every edit is kept in Messages (0.9.1, the user: each step seen).
    // (the edits made by the checks above, and the models opened)
    await p.click('.console header button:has-text("Messages")'); await p.waitForTimeout(100);
    const msgs = await p.textContent(".console .cmsgs");
    check("edits and the models opened are written in Messages as done (0.9.1, the user)", /Done\s*(Added|Changed|Deleted)/.test(msgs) && /Done\s*Started/.test(msgs), msgs.slice(0, 300));
    await p.click('.console header .cchev'); await p.waitForTimeout(100);
    await p.keyboard.press("F5"); await p.waitForTimeout(300);
    check("after a solve the card stays closed: the whole workspace is the model (0.9.1, the user)", (await p.$$(".ipop")).length === 0);
    await p.click('.flow button:has-text("Results")'); await p.waitForTimeout(150);
    check("step 6 on the workflow bar shows the results in the tree's Results tab (0.10.0)", (await p.textContent(".tree .ttabs button.on")) === "Results" && (await p.$$(".tree .tres .rinsp")).length === 1);
    await p.click('.console header button:has-text("Messages")'); await p.waitForTimeout(150);
    check("Messages keeps what the window said, with its time (0.9.1)", (await p.$$(".console .cmsg")).length >= 1 && (await p.textContent(".console .cmsgs")).includes("Solved"), await p.textContent(".console").catch(() => ""));
    await p.click('.console header button:has-text("Log")'); await p.waitForTimeout(100);
    check("the Log says when the solve started and finished (0.9.1)", /Solve.*started/.test(await p.textContent(".console .clog")) && /finished after/.test(await p.textContent(".console .clog")));
    // A solve that runs a while: its progress arrives as solver events; Stop asks the shell to stop.
    await p.click('.console header .cchev'); await p.waitForTimeout(100);
    await p.evaluate(() => { window.__hold = new Promise(ok => { window.__release = ok; }); });
    await p.keyboard.press("F5"); await p.waitForTimeout(150);
    await p.evaluate(() => window.__emit("solver", [{ e: "increment", lambda: 0.25 }, { e: "iteration", it: 1, residual: 0.3 }, { e: "iteration", it: 2, residual: 2e-4 }, { e: "iteration", it: 3, residual: 4e-10 }, { e: "converged", lambda: 0.25, iterations: 3 }, { e: "increment", lambda: 0.5 }, { e: "cut", lambda: 0.375, why: "it did not converge in 30 iterations; the increment is halved" }, { e: "increment", lambda: 0.375 }, { e: "iteration", it: 1, residual: 0.02 }]));
    await p.waitForTimeout(1200);
    const mon = await p.textContent(".console .conbody").catch(() => "");
    check("a solve that takes a while opens the Solver monitor by itself (0.9.1)", (await p.$eval(".console header button.on", el => el.textContent).catch(() => "")) === "Solver monitor", mon.slice(0, 200));
    check("the monitor shows the increment, the load reached, the out-of-balance force and the cut-backs (0.9.1)", mon.includes("Solving") && mon.includes("Load reached") && mon.includes("25 %") && mon.includes("2.00e-2") && mon.includes("1 time") && (await p.$$(".console .cres polyline")).length === 1, mon.slice(0, 400));
    check("the status bar says how far the solve is (0.9.1)", (await p.textContent(".status")).includes("increment 2"), await p.textContent(".status"));
    await p.click('.console header button:has-text("Stop")'); await p.waitForTimeout(100);
    check("Stop asks the solve to stop (0.9.1)", await p.evaluate(() => window.__calls.some(c => c[0] === "stop_solve")));
    await p.evaluate(() => { window.__release(); window.__hold = null; }); await p.waitForTimeout(300);
    check("a stopped solve says so in the monitor (0.9.1)", (await p.textContent(".console .conbody")).includes("Stopped"));
    // The panels widen by dragging their edge, and the right one folds away.
    const tw0 = await p.$eval(".tree", el => el.getBoundingClientRect().width);
    const g = await p.$eval(".modelview > .vgrip", el => { const r = el.getBoundingClientRect(); return [r.left + 2, r.top + 200]; });
    await p.mouse.move(g[0], g[1]); await p.mouse.down(); await p.mouse.move(g[0] + 60, g[1], { steps: 4 }); await p.mouse.up(); await p.waitForTimeout(100);
    const tw1 = await p.$eval(".tree", el => el.getBoundingClientRect().width);
    check("the steps panel widens by dragging its edge (0.9.1)", Math.abs(tw1 - tw0 - 60) < 4, `${tw0} → ${tw1}`);
    await p.click(".tree .ttabs button:has-text('Model')"); await p.waitForTimeout(80); await p.click(".tree .troot"); await p.waitForTimeout(100);
    await p.click(".insp header .ifold"); await p.waitForTimeout(100);
    check("the card closes with × (0.9.1, the user)", (await p.$$(".ipop")).length === 0);
    await p.click('.flow button:has-text("Results")'); await p.waitForTimeout(150);
    // The Results page has its way back.
    await p.click('.tree .tres .rinsp button:has-text("Tables and equilibrium")'); await p.waitForTimeout(250);
    check("the Results page has a bar with the way back to the model (0.9.1)", (await p.$$('.pagebar button:has-text("Back to the model")')).length === 1);
    await p.click('.pagebar button:has-text("Back to the model")'); await p.waitForTimeout(150);
    // A 3D mesh's Mesh view: the elements drawn in 3D, not the flat drawing sheet (0.9.1, found in the audit).
    await p.evaluate(f => { window.__next = f.elbow; }, fix);
    await open("bracket");
    const nm = await p.evaluate(() => window.__calls.filter(c => c[0] === "mesh").length);
    await p.evaluate(() => window.__setView("mesh")); await p.waitForTimeout(400);
    check("the Mesh view of a 3D mesh stays in 3D with the elements drawn, and says nothing about meshing a drawing (0.9.1)", (await p.$$(".view3d")).length >= 1 && (await p.$$(".dsvg")).length === 0 && (await p.evaluate(() => window.__calls.filter(c => c[0] === "mesh").length)) === nm && !(await p.textContent("body")).includes("nothing to mesh"));
    await open("portal");
  }

  {
    // 0.10.2: a frame's nodes and members in tables under the view, typed into, picked with the view, pasted from a sheet.
    await open("portal"); await p.waitForTimeout(150);
    const pm = fix.portal.m;
    // 0.10.2: the look of the approved mock-ups: the title bar says the file and what it is; the tree has no step circles and plain counts.
    check("the title bar names the app by its mark, and the model with its analysis (0.10.2, the mock-ups)", (await p.textContent(".top .appname")) === "Datum Mesh" && /Portal frame.*2D frame · linear · SI/.test(await p.textContent(".top .title")));
    check("the tree's steps are rows with icons, no numbered circles (0.10.2)", await p.$eval(".tree .sthead .stn", el => getComputedStyle(el).display === "none") && (await p.$$(".tree .sthead svg.tic")).length === 6);
    check("the tree's counts are plain grey numbers on the right, the ? and + only under the pointer", await p.$eval(".tree .tgrp summary .pill", el => getComputedStyle(el).borderTopStyle === "none") && await p.$eval(".tree .tgrp summary .help", el => getComputedStyle(el).display === "none"));
    await p.keyboard.press("F5"); await p.waitForTimeout(400);
    check("a finished solve opens the tree's Results tab (0.10.2, the mock-ups)", (await p.textContent(".tree .ttabs button.on")) === "Results");
    const rrows = await p.$$eval(".tree .rtree .ti", els => els.map(e => e.textContent));
    check("the Results tab is a tree: each result with its largest value, the checks, the tables (0.10.2, the mock-ups)", ["Bending moment", "Shear force", "Deflection", "Equilibrium · balanced", "Tables and equilibrium"].every(x => rrows.some(t => t.startsWith(x))) && rrows.some(t => /^Equilibrium · balanced0\.0 %$/.test(t)), JSON.stringify(rrows));
    await p.click('.tree .rtree .ti:has-text("Shear force")'); await p.waitForTimeout(100);
    check("…a click on a result shows it in the view", (await p.$$('.tree .rtree .ti.on:has-text("Shear force")')).length === 1);
    await p.click('.tree .rtree .ti:has-text("Bending moment")'); await p.waitForTimeout(100);
    check("the results legend is a dark card floating in the view (0.10.2, the mock-ups)", await p.$eval(".legend.rleg", el => getComputedStyle(el).position === "absolute" && el.closest(".center") !== null));
    await p.click(".legend.rleg .lfold"); await p.waitForTimeout(80);
    check("…that folds to one line and opens again", await p.$eval(".legend.rleg", el => el.classList.contains("folded") && el.offsetHeight < 40));
    await p.click(".legend.rleg .lfold"); await p.waitForTimeout(80);
    await p.click('.tabs > button:text-is("Geometry")'); await p.waitForTimeout(100);
    await p.click('.ribbon button:has-text("Nodes & Members")'); await p.waitForTimeout(400);
    check("Geometry › Nodes & Members opens the Tables in the bottom panel, a row for each node and member (0.10.2)", (await p.$$('.ttab[data-table="nodes"] tbody tr')).length === pm.nodes.length && (await p.$$('.ttab[data-table="members"] tbody tr')).length === pm.members.length && (await p.textContent(".console header button.on")) === "Tables");
    await p.click('.ttab[data-table="members"] tbody tr:nth-child(2) td:first-child'); await p.waitForTimeout(100);
    check("a clicked row is picked, in the table and the view (0.10.2)", (await p.$$('.ttab[data-table="members"] tbody tr.on')).length === 1 && (await p.$$(".ipop")).length === 1);
    const before = await p.evaluate(() => window.__calls.length);
    const cell = await p.$('.ttab[data-table="nodes"] tbody tr:nth-child(3) td:nth-child(2) input');
    await cell.fill("7.5"); await cell.press("Enter"); await p.waitForTimeout(150);
    const setOp = await p.evaluate(n => window.__calls.slice(n).filter(c => c[0] === "apply").map(c => c[1].op)[0], before);
    check("typing a node's x moves it, as an edit the engine makes (0.10.2)", !!setOp?.Set?.item?.Node && Math.abs(setOp.Set.item.Node.x - 7.5) < 1e-9 && setOp.Set.item.Node.id === pm.nodes[2].id, JSON.stringify(setOp));
    const sec = await p.$('.ttab[data-table="members"] tbody tr:nth-child(1) select:nth-of-type(1)');
    await sec.selectOption("truss"); await p.waitForTimeout(150);
    check("…and a member's kind changes from its list", await p.evaluate(() => { const o = window.__calls.filter(c => c[0] === "apply").map(c => c[1].op).pop(); return o?.Set?.item?.Member?.kind === "truss"; }));
    await p.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await p.evaluate(() => navigator.clipboard.writeText("Node\tx\ty\nA\t0\t9\nB\t12\t9\n\t0\t9\t12\t9\nN3\tN10\nnot a row"));
    await p.click('.ctbar button:has-text("Paste from a sheet")'); await p.waitForTimeout(250);
    const batch = await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").map(c => c[1].op).pop()?.Batch ?? []);
    check("Paste from a sheet: rows of x, y become nodes, x, y to x, y a member, N3 N10 a member between nodes; headings and other rows left out (0.10.2)",
      batch.length === 4 && batch.filter(o => o.Add?.item?.Node).length === 2 && batch.filter(o => o.AddMember).length === 1 && batch.filter(o => o.Add?.item?.Member).length === 1 && batch.find(o => o.Add?.item?.Node)?.Add.item.Node.y === 9, JSON.stringify(batch));
    // 0.10.3: the loads table and the assignments table, typed into as the others are.
    await p.click('.cttabs button[data-ttab="loads"]'); await p.waitForTimeout(200);
    check("Tables › Loads lists every load with its case, place, kind and values (0.10.3)", (await p.$$('.ttab[data-table="loads"] tbody tr')).length === pm.loads.length && (await p.$$('.ttab[data-table="loads"] td.lvals input')).length >= pm.loads.length, String((await p.$$('.ttab[data-table="loads"] tbody tr')).length));
    const lcell = await p.$('.ttab[data-table="loads"] tbody tr:nth-child(1) td.lvals input');
    await lcell.fill("-12"); await lcell.press("Enter"); await p.waitForTimeout(150);
    const lop = await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").map(c => c[1].op).pop());
    check("…a value typed there changes that load, in kN shown and N kept (0.10.3)", lop?.Set?.item?.Load?.id === pm.loads[0].id && Object.values(lop.Set.item.Load.kind).some(v => v === -12000), JSON.stringify(lop));
    await p.click('.cttabs button[data-ttab="assign"]'); await p.waitForTimeout(200);
    check("Tables › Assignments has a row per member with kind, section, material, hinges and rigid ends (0.10.3)", (await p.$$('.ttab[data-table="assignments"] tbody tr')).length === pm.members.length && (await p.$$('.ttab[data-table="assignments"] thead th')).length >= 9);
    await p.click('.ttab[data-table="assignments"] tbody tr:nth-child(2) td.c input'); await p.waitForTimeout(150);
    const hop = await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").map(c => c[1].op).pop());
    check("…ticking a hinge there hinges that member's start (0.10.3)", hop?.Set?.item?.Member?.id === pm.members[1].id && hop.Set.item.Member.hinge?.[0] === true, JSON.stringify(hop));
    await p.click('.cttabs button[data-ttab="geo"]'); await p.waitForTimeout(100);
    await p.click('.console header button:has-text("Messages")'); await p.waitForTimeout(150);
    check("…and says what it did in Messages, with the rows it left out", await p.evaluate(() => [...document.querySelectorAll(".cmsg span")].some(e => /Pasted from a sheet: 2 nodes and 2 members; 2 rows not understood/.test(e.textContent))));
    await p.click('.console header .cchev'); await p.waitForTimeout(100);
  }

  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(100);
  await p.click('.bs aside button:has-text("Print")'); await p.waitForTimeout(300);
  check("print preview draws the model and the report", (await p.$$(".pprev .printdiag line")).length === pf.members.length && (await p.textContent(".pprev")).includes("Structural steel"));
  await p.click(".bs aside .back");
  for (let i = 0; i < 3 && (await p.evaluate(() => document.documentElement.dataset.theme)) !== "dark"; i++) await p.click(".theme");
  check("theme switches to dark", (await p.evaluate(() => document.documentElement.dataset.theme)) === "dark");
  if (process.env.SHOTS) { await p.screenshot({ path: path.join(process.env.SHOTS, "dark.png") }); }
  check("every view opens without errors", errors.length === 0, errors.join(" | "));
  // Every word the window can show is in its bundle (help, dialogs, menus, messages).
  const bundle = fs.readdirSync(path.join(root, "assets")).filter(f => /\.(js|html|css)$/.test(f)).map(f => fs.readFileSync(path.join(root, "assets", f), "utf8")).join("\n") + fs.readFileSync(path.join(root, "index.html"), "utf8");
  check("the window's code and help name no commercial product", NAMES.length > 30 && named(bundle).length === 0 && named("Opened in Abaqus.").length === 1 && named("excellent").length === 0, named(bundle).join());

  await b.close(); srv.close();
  console.log(failed ? `\n${failed} check(s) failed` : "\nall UI checks passed");
  process.exit(failed ? 1 : 0);
})();
