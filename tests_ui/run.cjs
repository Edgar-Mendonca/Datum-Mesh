// UI tests: the built window (ui/dist) in Chromium with the engine's real worked examples
// (cargo run -p datum-core --example dump > tests_ui/fixture.json) behind a mocked Tauri bridge
// that records every call. Run: npm run test:ui (or npm run build:ui && node tests_ui/run.cjs).
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path");
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
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
    const m = { examples_list: f.examples_list, options: opts, startup_file: null, recovery: null, check_update: null, recent: [], version: "test", section_props: { a: 0.005, i: 8e-5, c: 0.15 } };
    window.__calls = [];
    window.__emit = (e, x) => (handlers[e] ?? []).forEach(h => h({ payload: x }));
    window.__TAURI__ = {
      core: {
        invoke: async (c, a) => {
          window.__calls.push([c, a]);
          if (c === "state") return window.__cur;
          if (c === "new_model") { window.__cur = { blank: f.frame, "blank-frame": f.frame, portal: f.portal, truss: f.truss, bracket: f.bracket, lug: f.lug, fin: f.fin }[a.example] ?? f.frame; return window.__cur; }
          if (c === "apply") return { ...window.__cur, dirty: true, undo: true, created: [999] };
          if (c === "history") return window.__cur;
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
  const open = async id => { await p.evaluate(() => document.querySelector(".tabs .file").click()); await p.waitForTimeout(150); const n = fix.examples_list.find(e => e.id === id).name; await p.click(`.home .tile:has(b:text-is("${n}"))`); await p.waitForTimeout(400); };

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
