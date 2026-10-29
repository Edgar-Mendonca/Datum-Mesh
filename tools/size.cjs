// Size budgets for the window (0.2.6), run by CI after `npm run build:ui`: the code loaded at
// startup and the whole window. The 3D view (three.js) is its own chunk, loaded only for 3D, and
// is counted in the total but not at startup. The installer's own budget is checked in release.yml.
const fs = require("fs"), path = require("path");
const dir = path.join(__dirname, "../ui/dist/assets");
const BUDGET = { startup: 400e3, total: 1.6e6 };
const files = fs.readdirSync(dir).map(f => ({ f, n: fs.statSync(path.join(dir, f)).size }));
const startup = files.filter(x => x.f.endsWith(".js") && !x.f.startsWith("view3d")).reduce((s, x) => s + x.n, 0);
const total = files.reduce((s, x) => s + x.n, 0) + fs.statSync(path.join(dir, "../index.html")).size;
const kb = n => `${Math.round(n / 1000)} kB`;
console.log(`startup code ${kb(startup)} (budget ${kb(BUDGET.startup)}), whole window ${kb(total)} (budget ${kb(BUDGET.total)})`);
if (startup > BUDGET.startup || total > BUDGET.total) { console.error("Over the size budget: see tools/size.cjs and docs/MEMORY.md."); process.exit(1); }
