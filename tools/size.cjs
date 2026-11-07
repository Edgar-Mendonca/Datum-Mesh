// Size budgets for the window (0.2.6), run by CI after `npm run build:ui`: the code loaded at
// startup and the whole window. Startup is what index.html loads (its script and preloads, 0.5.3);
// chunks loaded on demand (the 3D view with three.js, File › New) count in the total only. The installer's own budget is checked in release.yml.
const fs = require("fs"), path = require("path");
const dir = path.join(__dirname, "../ui/dist/assets");
// 0.7.1: startup 400 → 480 kB for the geometry tools (sketching, editing, 3D features) on the ribbon.
// 0.10.2: whole window 1.6 → 1.8 MB (the user): the map of planned features and the help words, all loaded after startup.
const BUDGET = { startup: 480e3, total: 1.8e6 };
const files = fs.readdirSync(dir).map(f => ({ f, n: fs.statSync(path.join(dir, f)).size }));
const html = fs.readFileSync(path.join(dir, "../index.html"), "utf8");
const loaded = new Set([...html.matchAll(/(?:src|href)="\.\/assets\/([^"]+\.js)"/g)].map(m => m[1]));
const startup = files.filter(x => loaded.has(x.f)).reduce((s, x) => s + x.n, 0);
const total = files.reduce((s, x) => s + x.n, 0) + fs.statSync(path.join(dir, "../index.html")).size;
const kb = n => `${Math.round(n / 1000)} kB`;
console.log(`startup code ${kb(startup)} (budget ${kb(BUDGET.startup)}), whole window ${kb(total)} (budget ${kb(BUDGET.total)})`);
if (startup > BUDGET.startup || total > BUDGET.total) { console.error("Over the size budget: see tools/size.cjs and docs/MEMORY.md."); process.exit(1); }
