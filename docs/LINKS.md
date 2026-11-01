# Datum Link: how the family's apps talk to each other (proposal, 2026-11-01)

Eight desktop apps (Plan, Plot, Atlas, Sim, Calc, Mesh, Pipe, Schema), installed separately, no
server. They talk the simplest way that works offline on Windows: **one small JSON file, opened in
the other app**. Every app reads and writes the same envelope; each app understands only the
`kind`s it needs. This file is the proposal for `FAMILY.md` (a new section, "Links between
apps"); until the hub adopts it, each app copies this text.

## 1. The packet

A UTF-8 JSON file with the extension **`.dlink`**, SI units always (as in every Datum file):

```json
{
  "datum_link": 1,
  "from": { "app": "mesh", "version": "0.6.0", "file": "Bracket.dmesh" },
  "to": "plot",
  "kind": "table",
  "title": "L-bracket: load against deflection",
  "made": "2026-11-01T14:03:00Z",
  "reply": null,
  "data": { }
}
```

- `to` names the app meant to open it (one of `plan plot atlas sim calc mesh pipe schema`).
- `reply`: when an app asks another to pick something (a material, a value), it puts a reply
  token here and the answer comes back in a packet with the same token (§4).
- Unknown fields are ignored; an app that cannot use a `kind` says so in one line and opens
  nothing.

## 2. Kinds

| `kind` | `data` | Who writes it | Who reads it |
| --- | --- | --- | --- |
| `table` | `{ "columns": [{ "name", "unit" }], "rows": [[…]] }` (SI units) | Mesh, Pipe, Sim, Calc, Schema | **Plot** (a new sheet and a ready graph), Calc |
| `value` | `{ "name", "value", "unit", "note" }` or a list of them | Mesh, Pipe, Sim, Plot (a fitted parameter) | **Calc** (variables), Plan (a KPI) |
| `material` | `{ "name", "e", "nu", "rho", "fy", "k", "alpha", "cp", "source" }` | **Atlas** | Mesh, Pipe, Calc, Sim |
| `model` | the target app's own file, embedded (`.dmesh`, `.dpipe`, `.dsim` JSON) | Pipe, Schema, Mesh | Mesh, Pipe, Sim |
| `task` | `{ "name", "start", "finish", "note", "link" }` | any app (a review, a check to do) | **Plan** |
| `pick` | `{ "want": "material" \| "value", "hint" }` (a request) | Mesh, Pipe, Calc, Sim | Atlas, Calc |

## 3. How a packet travels

1. **Send to …** in the app writes the packet to `%LOCALAPPDATA%\Datum\links\<time>-<kind>.dlink`.
2. It opens the target app with that file (`DatumPlot.exe <path>`, found by the app's registered
   identifier, or by the `.dlink` file type, which every app's installer registers with "Open
   with" entries for each app).
3. The target app, on start or when already open (single instance, the path passed to the running
   window), reads the packet and shows what it got: "From Datum Mesh: a table, 34 rows. Make a
   graph?".
4. If the target app is not installed, the sender says so and offers the download page
   (`https://edgar-mendonca.github.io/Datum/apps/<id>`).

**Copy and paste** works too: "Copy for Datum apps" puts the same JSON on the clipboard as text
starting with `{"datum_link":1`, and any Datum app's Paste recognises it. Spreadsheets still get
plain TSV.

## 4. Pick and answer

To choose a material in Atlas from Mesh: Mesh writes a `pick` packet with `reply` set to a token
and opens Atlas. Atlas shows a banner "Datum Mesh wants a material: pick one" and, on Pick, writes
a `material` packet with the same `reply` token, `to: "mesh"`, and opens Mesh with it. Mesh puts
the material on the shapes that were selected.

## 5. The links worth making (all eight apps)

| From → to | What travels | Why |
| --- | --- | --- |
| Atlas → Mesh, Pipe, Calc, Sim | `material` | One trusted source of properties, with its reference |
| Mesh → Plot | `table`: load–deflection curves, probe lines, frequencies, time histories, mesh convergence | Publication graphs of FE results |
| Mesh → Calc | `value`: largest stress, deflection, reaction, frequency | A hand check beside the FE answer |
| Pipe → Mesh | `model`: a nozzle, branch or support region with its loads | Local FE where code formulas run out |
| Mesh → Pipe | `value`: a stiffness or flexibility factor | Back into the pipe model |
| Schema → Pipe | `model`: a line with its design data | Pipe stress from the P&ID |
| Schema → Mesh | `model`: a vessel's size and design pressure | A vessel to check in 3D |
| Mesh → Sim | `model`: a modal model (frequencies, modal masses) or a stiffness | A structure inside a control loop |
| Sim → Plot | `table`: simulation results | Graphs |
| Calc ↔ Plot | `table` / `value` | Fitted parameters into calcs, calc tables into graphs |
| Any → Plan | `task` | "Review this analysis" as a task with a link to the file |

## 6. In Datum Mesh (planned)

- 0.6.x: **Send to Datum Plot** (curves, probe lines, frequencies) and **Send to Datum Calc**
  (results as values), replacing the two greyed buttons; **Open a `.dlink`** (materials from
  Atlas, models from Pipe and Schema); **Pick in Datum Atlas** in the material editor.
- The engine side is small: `crates/core/src/link.rs` (the envelope, its checks, tables and values
  from results), unit-tested; the window only writes and opens files.
