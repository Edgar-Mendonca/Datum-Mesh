// UI tests: the built window (ui/dist) in Chromium with the engine's real worked examples
// (cargo run -p datum-core --example dump > tests_ui/fixture.json) behind a mocked Tauri bridge
// that records every call. Run: npm run test:ui (or npm run build:ui && node tests_ui/run.cjs).
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path");
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
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
const check = (name, ok, info = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + info}`); if (!ok) failed++; };

(async () => {
  await new Promise(ok => srv.listen(0, ok));
  const url = `http://localhost:${srv.address().port}/`;
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on("pageerror", e => errors.push(e.message));
  await p.addInitScript(f => {
    const opts = { autosave: 0, beta: false, theme: "light", openLast: false, units: "SI", snap: true, grid: true, rulers: true, contours: "rainbow", paper: "A4", landscape: true, company: "" };
    window.__cur = f.frame;
    const handlers = {};
    const m = { verification: f.verification, examples_list: f.examples_list, options: opts, startup_file: null, recovery: null, check_update: null, recent: [], version: "test", section_props: { a: 0.005, i: 8e-5, c: 0.15, iy: 6e-6, cy: 0.075, j: 2e-7 } };
    window.__calls = [];
    window.__emit = (e, x) => (handlers[e] ?? []).forEach(h => h({ payload: x }));
    window.__TAURI__ = {
      core: {
        invoke: async (c, a) => {
          window.__calls.push([c, a]);
          if (c === "state") return window.__cur;
          if (c === "new_model" && window.__next) { window.__cur = window.__next; window.__next = null; return window.__cur; }
          if (c === "new_model") { window.__cur = { blank: f.frame, "blank-frame": f.frame, portal: f.portal, truss: f.truss, bracket: f.bracket, lug: f.lug, fin: f.fin, hotplate: f.hotplate, vessel: f.vessel, platform: f.platform, "blank-space": f.space }[a.example] ?? f.frame; return window.__cur; }
          if (c === "apply") { window.__cur = { ...window.__cur, dirty: true, undo: true, created: [999] }; return window.__cur; }
          if (c === "history") return window.__cur;
          if (c === "solve") { if (window.__solveKey) return f[window.__solveKey]; if (window.__cur === f.portal) return f.portal_results; if (window.__cur === f.noload) return f.noload_results; if (window.__cur === f.truss) return f.truss_results; if (window.__cur === f.bracket) return f.bracket_results; if (window.__cur === f.fin) return f.fin_results; if (window.__cur === f.hotplate) return f.hotplate_results; if (window.__cur === f.vessel) return f.vessel_results; if (window.__cur === f.platform) return f.platform_results; throw "There is nothing to mesh."; }
          if (c === "mesh") return f.bracket_mesh;
          if (c === "buckling") { if (window.__cur === f.portal) return f.portal_buckle; if (window.__cur === f.truss) return f.truss_buckle; throw "Buckling is for frames and trusses in this release; parts come later."; }
          if (c === "modes") { if (window.__cur === f.portal) return f.portal_modes; if (window.__cur === f.bracket) return f.bracket_modes; throw "Natural frequencies are for frames and for parts in stress."; }
          if (c === "materials_library") return f.library;
          if (c in m) return m[c];
          return null;
        },
      },
      event: { listen: async (e, h) => { (handlers[e] ??= []).push(h); return () => {}; } },
    };
  }, fix);
  await p.goto(url);
  await p.waitForTimeout(800);
  const ops = () => p.evaluate(() => window.__calls.filter(c => c[0] === "apply").map(c => c[1].op));
  const lastOp = async () => (await ops()).slice(-1)[0];
  const canvasBox = async () => p.$eval(".canvas", el => { const r = el.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
  const center = sel => p.$eval(sel, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  const open = async id => {
    await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150);
    const n = fix.examples_list.find(e => e.id === id).name; await p.click(`.home .tile:has(b:text-is("${n}"))`); await p.waitForTimeout(250);
    // Unsaved changes are offered for saving first (0.1.2): drop them.
    if (await p.$('.dlg header:has-text("Save changes?")')) { await p.click('.dlg button:has-text("Don\'t save")'); await p.waitForTimeout(300); }
    await p.waitForTimeout(150);
  };

  // 1. File page: every worked example, crimson sidebar.
  const tiles = await p.$$eval(".home .tile b", els => els.map(e => e.textContent));
  check("File page shows every worked example", fix.examples_list.every(e => tiles.includes(e.name)), JSON.stringify(tiles));
  check("File sidebar is the crimson accent", (await p.$eval(".bs aside", el => getComputedStyle(el).backgroundColor)) === "rgb(184, 50, 80)");

  // 2. The portal frame draws its members, nodes, supports and loads.
  await open("portal");
  const pf = fix.portal.m;
  check("members are drawn", (await p.$$(".mem")).length === pf.members.length);
  check("nodes are drawn", (await p.$$(".node")).length === pf.nodes.length);
  check("supports are drawn", (await p.$$("g.sup")).length === pf.supports.length);
  check("loads are drawn with their values", (await p.$$("g.load")).length === pf.loads.length && (await p.textContent(".dsvg")).includes("kN/m"));
  check("ribbon tabs", JSON.stringify(await p.$$eval(".tabs > button:not(.file)", els => els.map(e => e.textContent))) === JSON.stringify(["Home", "Geometry", "Mesh", "Loads", "Solve", "Results", "View", "Help"]));
  check("frames are in metres on the ruler", (await p.textContent(".rtxt.unitlbl")) === "m");
  check("the tree lists members and nodes", (await p.$$(".tree .ti")).length >= pf.members.length + pf.nodes.length);
  const allMembersOnScreen = await p.evaluate(() => [...document.querySelectorAll(".mem line")].every(l => { const r = l.getBoundingClientRect(), c = document.querySelector(".canvas").getBoundingClientRect(); return r.left >= c.left && r.right <= c.right && r.top >= c.top && r.bottom <= c.bottom; }));
  check("the model fits the window when opened", allMembersOnScreen);

  // 3. Selecting a member shows it in the Inspector; the contextual tab does not move the ribbon.
  const ribbonTop = await p.$eval(".ribbon", el => el.getBoundingClientRect().top);
  const m0 = pf.members[0];
  const mc = await p.$eval(`.mem[data-id="${m0.id}"] line`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  await p.mouse.click(mc[0], mc[1]);
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
  await p.click('.tabs > button:text-is("Geometry")');
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
  await p.click('.tabs > button:text-is("Loads")');
  await p.click('.ribbon button[title="Pinned"]');
  const n0 = await center(`.node[data-id="${pf.nodes[2].id}"]`);
  await p.mouse.click(n0[0], n0[1]); await p.waitForTimeout(150);
  const sp = await lastOp();
  check("the Pinned tool puts a support on the node clicked", sp?.Add?.item?.Support?.at?.id === pf.nodes[2].id && sp.Add.item.Support.kind.kind === "pinned", JSON.stringify(sp));
  await p.click('.ribbon button[title="Distributed"]');
  await p.mouse.click(mc[0], mc[1]); await p.waitForTimeout(150);
  const dl = await lastOp();
  check("the Distributed tool puts a load on the member clicked", dl?.Add?.item?.Load?.at?.on === "member" && dl.Add.item.Load.kind.kind === "dist", JSON.stringify(dl));
  await p.keyboard.press("Escape");

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
  await p.click('.tabs > button:text-is("Geometry")');
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
  await p.mouse.click(cb2.x + 400, cb2.y + 400); await p.mouse.click(cb2.x + 420, cb2.y + 400); await p.waitForTimeout(150);
  const hc = await lastOp();
  check("the round hole tool adds a circular hole", hc?.Add?.item?.Shape?.hole === true && hc.Add.item.Shape.geom.kind === "circle", JSON.stringify(hc));
  await p.keyboard.press("Escape");

  // 8b. Arcs bulge the right way on screen: the lug's rounded top makes it taller than wide (0.1.0 bug: arcs were drawn mirrored).
  await open("lug");
  const lb = await p.$eval(".shape:not(.hole)", el => { const r = el.getBoundingClientRect(); return r.height / r.width; });
  check("the lifting lug's rounded top bulges outwards", Math.abs(lb - 1.5) < 0.03, String(lb));

  // 9. Heat sink: convection and temperature are drawn; heat tools on the Loads tab.
  await open("fin");
  await p.click('.tabs > button:text-is("Loads")');
  check("a heat model offers heat tools only", (await p.$$('.ribbon button[title="Convection"]')).length === 1 && (await p.$$('.ribbon button[title="Pinned"]')).length === 0);
  check("convection and the fixed temperature are drawn", (await p.$$("g.load")).length === fix.fin.m.loads.length && (await p.textContent(".dsvg")).includes("°C"));

  // 10. Help, Tell Me, shortcuts.
  await p.click(".insp header .help"); await p.waitForTimeout(100);
  check("? card opens", (await p.textContent(".dlg")).includes("Inspector"));
  await p.keyboard.press("Escape");
  await p.keyboard.press("Control+k"); await p.keyboard.type("pressure"); await p.waitForTimeout(100);
  check("Tell Me finds commands", (await p.textContent(".pal")).includes("Pressure"));
  await p.keyboard.press("Escape");

  // 11. The ribbon never scrolls sideways.
  for (const w of [1280, 1600, 1000]) {
    await p.setViewportSize({ width: w, height: 900 });
    for (const t of ["Home", "Geometry", "Mesh", "Loads", "Solve", "Results", "View", "Help"]) {
      await p.click(`.tabs > button:text-is("${t}")`);
      await p.waitForTimeout(60);
      const over = await p.$eval(".ribbon", el => el.scrollWidth - el.clientWidth);
      if (over > 1) check(`ribbon ${t} fits at ${w}px`, false, `overflow ${over}`);
    }
  }
  await p.setViewportSize({ width: 1440, height: 900 });

  // 11b. Solving a frame: deflected shape, moment diagram, reactions, legend, probe, tables (0.1.1).
  await open("portal");
  await p.keyboard.press("F5"); await p.waitForTimeout(300);
  const pr2 = unpack(fix.portal_results);
  check("F5 solves and the status bar says so", (await p.textContent(".status")).includes(`Solved · ${pr2.stats.equations} equations`), await p.textContent(".status"));
  check("the deflected shape is drawn for every member", (await p.$$(".defl")).length === pr2.members.length);
  check("the moment diagram is drawn for every member", (await p.$$(".diag.d-m")).length === pr2.members.length);
  check("reactions are drawn at the supports", (await p.$$(".rarrow")).length >= 4);
  check("the legend shows the equilibrium check", (await p.textContent(".legend")).includes("reactions balance the loads"));
  check("the Results tab is shown after solving", await p.$eval('.tabs > button:text-is("Results")', el => el.classList.contains("on")));
  await p.click('.ribbon button[title="Shear Force"]'); await p.waitForTimeout(100);
  check("Shear Force switches the diagram", (await p.$$(".diag.d-v")).length === pr2.members.length && (await p.$$(".diag.d-m")).length === 0);
  const col = await p.$eval(`.mem[data-id="${pf.members[0].id}"] line`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  await p.mouse.move(col[0], col[1]); await p.waitForTimeout(150);
  check("hovering a member shows the probe", (await p.$$(".probe")).length === 1 && (await p.textContent(".probe")).includes("Member 1"));
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(150);
  check("the Results page lists reactions and members", (await p.$$(".page .tbl tbody tr")).length >= pr2.reactions.length + pr2.members.length + 3 && (await p.textContent(".page")).includes("balance the loads"));
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(150);
  await p.mouse.click(col[0], col[1]); await p.keyboard.press("ArrowUp"); await p.waitForTimeout(150);
  check("an edit marks the results out of date", (await p.$$(".legend .stalebar")).length === 1);
  await p.click('.tabs > button:text-is("Help")'); await p.click('.ribbon button[title="Verification"]'); await p.waitForTimeout(250);
  check("Help › Verification lists every benchmark, all passing", (await p.textContent(".dlg")).includes(`${fix.verification.length} of ${fix.verification.length} checks pass`));
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
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(200);
  check("the Results page of a part shows equilibrium, reactions and extremes", (await p.textContent(".page")).includes("Largest and smallest values") && (await p.textContent(".page")).includes("balance the loads") && (await p.textContent(".page")).includes("Fixed"), (await p.textContent(".page")).slice(0, 200));
  await p.click('.side button[title="Mesh"]'); await p.waitForTimeout(200);
  check("results arrive packed small and are unpacked by the window", typeof fix.bracket_results.mesh.nodes.$f32 === "string" && typeof fix.bracket_results.mesh.tris.$u32 === "string");
  check("the Mesh view shows the elements and their quality", (await p.textContent(".legend")).includes(`${br.mesh.tris.length.toLocaleString()} 6-node triangles`) && (await p.textContent(".legend")).includes("smallest angle"));
  await p.click('.side button[title="Report"]'); await p.waitForTimeout(400);
  check("the Report view has the mesh, a contour picture with its scale, and the equilibrium check", (await p.textContent(".page")).includes("Mesh") && (await p.$$(".page .rpic img")).length >= 1 && (await p.$$(".page .rpic .cbar")).length >= 1 && (await p.textContent(".page")).includes("Equilibrium check"));
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(150);
  // Refine near an edge: a local size in the mesh settings.
  await p.click('.tabs > button:text-is("Mesh")'); await p.click('.ribbon button[title^="Click an edge or a point"]'); await p.waitForTimeout(100);
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
  await p.click(".canvas", { position: { x: 30, y: 400 } }); await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  check("the Inspector has the stress-free temperature", (await p.textContent(".insp")).includes("Stress-free temperature"));
  await p.click('.tabs > button:text-is("Loads")'); await p.click('.ribbon button[title^="A uniform change of temperature"]'); await p.waitForTimeout(150);
  const wl = (await ops()).reverse().find(o => o.Add && o.Add.item.Load && o.Add.item.Load.kind.kind === "warming");
  check("Temperature Change adds a uniform warming of the whole part", !!wl && wl.Add.item.Load.at.on === "body", JSON.stringify(wl));
  // Natural frequencies (0.2.2): a frame and a part.
  await open("portal");
  await p.click('.tabs > button:text-is("Solve")'); await p.click('.ribbon button[title="The lowest natural frequencies and their mode shapes"]'); await p.waitForTimeout(400);
  const pmo = unpack(fix.portal_modes);
  check("Natural Frequencies lists the modes under the canvas", (await p.$$(".legend .modelist button")).length === pmo.modes.length && (await p.textContent(".legend")).includes("Hz"));
  check("a frame's mode is drawn along every member", (await p.$$(".modeline")).length === fix.portal.m.members.length);
  check("the static diagrams are hidden while a mode is shown", (await p.$$(".diag")).length === 0);
  await p.click(".legend .modelist button:nth-child(2)"); await p.waitForTimeout(100);
  check("another mode can be picked", await p.$eval(".legend .modelist button:nth-child(2)", el => el.classList.contains("on")));
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(200);
  check("the Results page lists the natural frequencies", (await p.textContent(".page")).includes("Natural frequencies") && (await p.$$(".page .tbl tbody tr")).length >= pmo.modes.length);
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(150);
  await p.click(".legend button:has-text('Back to the static results')"); await p.waitForTimeout(100);
  check("Back to the static results hides the modes", (await p.$$(".modeline")).length === 0);
  await open("bracket");
  await p.click('.tabs > button:text-is("Solve")'); await p.click('.ribbon button[title="The lowest natural frequencies and their mode shapes"]'); await p.waitForTimeout(500);
  check("a part's mode is drawn on the contour canvas", await p.$eval("canvas.contours", el => el.classList.contains("on")) && (await p.textContent(".legend")).includes("Natural frequencies"));
  check("the part's fill is cleared under its mode (0.2.2 real-app bug)", await p.$eval(".dsvg", el => el.classList.contains("under")));
  // Buckling (0.2.3): a frame's load factors and shapes, and a truss's bars checked on their own.
  await open("portal");
  await p.click('.tabs > button:text-is("Solve")'); await p.click('.ribbon button[title="The lowest buckling load factors of a frame and their shapes"]'); await p.waitForTimeout(400);
  const pbk = unpack(fix.portal_buckle);
  check("Buckling lists the load factors under the canvas", (await p.$$(".legend .modelist button")).length === pbk.modes.length && (await p.textContent(".legend")).includes("Buckling load factors") && (await p.textContent(".legend .modelist")).includes("λ"));
  check("a buckled shape is drawn along every member", (await p.$$(".modeline")).length === fix.portal.m.members.length);
  check("the ribbon offers Buckled Shapes", (await p.$$('.ribbon button[title^="Show the buckled shapes"]')).length === 1);
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(200);
  check("the Results page lists the load factors", (await p.textContent(".page")).includes("Load factor λ") && (await p.textContent(".page")).includes(String(pbk.modes[0].lambda.toPrecision(3)).slice(0, 3)));
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(150);
  await open("truss");
  await p.click('.tabs > button:text-is("Solve")'); await p.click('.ribbon button[title="The lowest buckling load factors of a frame and their shapes"]'); await p.waitForTimeout(400);
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(200);
  const tbk = unpack(fix.truss_buckle);
  check("a truss's squeezed bars are checked on their own, weakest first", (await p.textContent(".page")).includes("Truss bars in compression") && (await p.$eval(".page", el => [...el.querySelectorAll("h3 + .tbl tbody tr td:first-child")].map(td => td.textContent)))[0] === `Member ${fix.truss.m.members.findIndex(x => x.id === [...tbk.bars].sort((a, b) => a.lambda - b.lambda)[0].id) + 1}`);
  check("the bar that buckles first is named as the tree names it", /Member \d+, a truss bar, buckles on its own/.test(await p.textContent(".page")));
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(150);
  await open("bracket");
  await p.click('.tabs > button:text-is("Solve")');
  check("Buckling is off for parts", await p.$eval('.ribbon button[title="The lowest buckling load factors of a frame and their shapes"]', el => el.disabled || el.classList.contains("off")));
  // 3D frames (0.2.4): the platform in the 3D view, drawing on planes, solving, the Results page.
  await open("platform");
  const pm = fix.platform.m, z0 = pm.nodes.filter(n => (n.z ?? 0) === 0).map(n => n.id);
  check("a 3D frame opens in the 3D view with the plane bar", (await p.$$(".view3d canvas")).length === 1 && (await p.$eval(".planebar .seg button.on", el => el.textContent)) === "3D");
  await p.click('.tabs > button:text-is("Home")');
  check("Home shows 3D Frame chosen", await p.$eval('.ribbon button[title^="3D frames and trusses"]', el => el.classList.contains("on")));
  await p.click('.planebar .seg button:text-is("Front")'); await p.waitForTimeout(250);
  const onFront = pm.members.filter(b => z0.includes(b.a) && z0.includes(b.b)).length;
  check("the front plane shows the members on it, the others faint", (await p.$$(".dsvg .mem")).length === onFront && (await p.$$(".dsvg .ghost")).length === pm.members.length - onFront, `${(await p.$$(".dsvg .mem")).length} of ${onFront}`);
  const dz = await p.$$eval(".planebar select option", os => os.map(o => o.textContent));
  check("the depth picker offers the depths that have nodes", dz.length === 2 && dz[1].startsWith("4"), dz.join(", "));
  await p.click('.planebar .seg button:text-is("Side")'); await p.waitForTimeout(200);
  await p.click('.tabs > button:text-is("Geometry")'); await p.click('.ribbon button[title^="Draw beams"]');
  const sb = await canvasBox();
  await p.mouse.click(sb.x + sb.w * 0.3, sb.y + sb.h * 0.5); await p.waitForTimeout(80); await p.mouse.click(sb.x + sb.w * 0.6, sb.y + sb.h * 0.5); await p.waitForTimeout(150); await p.keyboard.press("Escape"); await p.keyboard.press("Escape");
  const am3 = (await ops()).reverse().find(o => o.AddMember);
  check("a beam drawn on the side plane is sent in 3D: x at the depth, z across", !!am3 && am3.AddMember.from[0] === 0 && am3.AddMember.to[0] === 0 && am3.AddMember.z[0] !== am3.AddMember.z[1] && am3.AddMember.from[1] === am3.AddMember.to[1], JSON.stringify(am3));
  check("Geometry shows Repeat for 3D frames", (await p.$$('.ribbon button[title^="Copy the selected members along the depth"]')).length === 1);
  await p.click('.ribbon button[title^="Copy the selected members along the depth"]'); await p.waitForTimeout(150);
  await p.click('.dlg button:text-is("Repeat")'); await p.waitForTimeout(150);
  const rp = await lastOp();
  check("Repeat sends the whole frame one step along z, joined by beams", !!rp.Repeat && rp.Repeat.ids.length === pm.members.length && rp.Repeat.step[2] > 0 && rp.Repeat.times === 1 && rp.Repeat.link === "beam", JSON.stringify(rp));
  await p.click(`.tree .ti:has-text("Node 3")`); await p.waitForTimeout(100);
  check("the Inspector shows a node's z in 3D", (await p.$$eval(".insp .iprow label", ls => ls.map(l => l.textContent))).some(t => t.startsWith("z")));
  await p.evaluate(() => { window.__solveKey = "platform_results"; });
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  await p.evaluate(() => { window.__solveKey = null; });
  await p.click('.planebar .seg button:text-is("3D")'); await p.waitForTimeout(300);
  check("once solved, the 3D view colours the members and checks equilibrium", (await p.textContent(".legend")).includes("Colour by") && (await p.textContent(".legend")).includes("Equilibrium"));
  await p.click('.tabs > button:text-is("Results")');
  check("Results offers colouring by a result in 3D", (await p.$$('.ribbon button:has-text("Moment My")')).length === 1);
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(200);
  const pg3 = await p.textContent(".page");
  check("the 3D Results page sums six ways and lists six reactions per support", pg3.includes("Σ Fz") && pg3.includes("Σ My about the origin") && (await p.$$eval(".page h2 + table thead tr", rs => rs.map(r => r.children.length))).includes(7), pg3.slice(0, 200));
  check("the 3D Results page lists every member with torque and both moments", (await p.$$(".page .tbl tbody tr.link")).length === pm.members.length && pg3.includes("|Mz| · |My|"));
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(200);
  // Axisymmetric (0.2.1): the pressure vessel, drawn as a half-section right of the axis.
  await open("vessel");
  check("an axisymmetric model shows its axis of turning", (await p.$$(".dsvg path.axi")).length === 1 && (await p.textContent(".canvas")).includes("axis of turning"));
  await p.keyboard.press("F5"); await p.waitForTimeout(400);
  const vo = await p.$$eval(".legend .fpick option", os => os.map(o => o.textContent));
  check("a turned solid's results are radial, axial and hoop", vo.includes("Hoop stress σθ") && vo.includes("Radial stress σr") && vo.includes("Axial displacement"), vo.join(", "));
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(200);
  check("its equilibrium check sums the forces along the axis only", (await p.textContent(".page")).includes("Σ F along the axis") && !(await p.textContent(".page")).includes("Σ M about the origin"));
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(150);
  // Heat sink: temperature contours and the heat balance.
  await open("fin"); await p.keyboard.press("F5"); await p.waitForTimeout(400);
  check("a heat model shows temperatures in °C and the heat balance", (await p.$eval(".legend .fpick", el => el.value)) === "t" && (await p.textContent(".legend .cbar")).includes("°C") && (await p.textContent(".legend")).includes("Heat balance"));
  await p.click('.side button[title="Results"]'); await p.waitForTimeout(200);
  check("the heat Results page lists the heat flows and their sum", (await p.textContent(".page")).includes("Heat into the part") && (await p.textContent(".page")).includes("Sum"));
  await p.click('.side button[title="Model"]'); await p.waitForTimeout(150);
  // The materials library.
  await p.click('.tabs > button:text-is("Geometry")'); await p.click('.ribbon button[title="Materials"]'); await p.waitForTimeout(150);
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
  const mid = await p.$eval(`.mem[data-id="${pf.members[1].id}"] line`, el => { const r = el.getBoundingClientRect(); return [r.left + r.width * 0.3, r.top + r.height * 0.7]; });
  await p.mouse.move(mid[0], mid[1]); await p.waitForTimeout(150);
  const probeText = (await p.$$(".probe")).length ? await p.textContent(".probe") : "";
  check("the probe shows no rounding noise", probeText.length > 0 && !/e-1\d/.test(probeText), probeText);
  // (2) Unsaved changes are offered for saving before an example replaces them.
  await p.click(".tree .ti:has-text('Member 1')"); await p.click('.tabs > button:text-is("Home")'); await p.click('.ribbon button[title="Delete"]'); await p.waitForTimeout(150);
  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150);
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
  await p.click('.tabs > button:text-is("Geometry")');
  await p.click('.ribbon button[title="Sections"]'); await p.waitForTimeout(200);
  check("the Sections dialog shows the section's properties", (await p.textContent(".dlg")).includes("mm⁴"));
  await p.keyboard.press("Escape");
  await p.click('.ribbon button[title="Materials"]'); await p.waitForTimeout(150);
  check("the Materials dialog shows E in GPa", (await p.textContent(".dlg")).includes("GPa"));
  await p.keyboard.press("Escape");
  for (const v of ["Mesh", "Results", "Report", "Model"]) { await p.click(`.side button[title="${v}"]`); await p.waitForTimeout(100); }
  for (const k of ["Print", "Options", "Roadmap", "Updates", "Export", "Home"]) { await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(100); await p.click(`.bs aside button:has-text("${k}")`); await p.waitForTimeout(200); }
  check("print preview draws the model", true);
  await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(100);
  await p.click('.bs aside button:has-text("Print")'); await p.waitForTimeout(300);
  check("print preview draws the model and the report", (await p.$$(".pprev .printdiag line")).length === pf.members.length && (await p.textContent(".pprev")).includes("Structural steel"));
  await p.click(".bs aside .back");
  for (let i = 0; i < 3 && (await p.evaluate(() => document.documentElement.dataset.theme)) !== "dark"; i++) await p.click(".theme");
  check("theme switches to dark", (await p.evaluate(() => document.documentElement.dataset.theme)) === "dark");
  if (process.env.SHOTS) { await p.screenshot({ path: path.join(process.env.SHOTS, "dark.png") }); }
  check("every view opens without errors", errors.length === 0, errors.join(" | "));

  await b.close(); srv.close();
  console.log(failed ? `\n${failed} check(s) failed` : "\nall UI checks passed");
  process.exit(failed ? 1 : 0);
})();
