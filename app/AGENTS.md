# Datum Mesh application (app/): notes for agents

This folder is the Datum Mesh desktop application: a fork of SolidCraft (MIT or Apache-2.0, upstream
`tangency-labs/solidcraft` at commit f8a98ee, 0.40.0-rc.2) with Datum Mesh's analysis added. Read the
repository's root `CLAUDE.md` and `docs/MEMORY.md` first; they govern this folder too.

## What is ours here
- `crates/analysis` (datum-analysis): the study (material, supports and loads on named faces, mesh
  size), meshing through Gmsh (a separate program), the trace from mesh faces back to model faces,
  and the solve with `datum-core` (the root `crates/core`, our verified engine).
- `crates/ui-egui/src/sim.rs`: the Simulation workspace (stage bar: Material, Supports & Loads, Mesh,
  Solve, Results), its hooks in `toolbar.rs`, `context_menu.rs`, `viewport.rs` and `lib.rs`.
- Branding: the name Datum Mesh in user-visible text, the mark (`assets/icons/ui/source/glyphs.rs`
  `brand_mark_styled`, `assets/icons/app/shared/source/app_icon.rs`), the crimson accent
  (`assets/styles/ui/palettes.rs`), the app ID, the updater (our GitHub releases,
  `DatumMesh-x.y.z-setup.exe`), the Windows setup (`packaging/windows/datum-mesh.nsi`), design files
  `.dmx`.
- Turned off: AI-client connections and agent chat at start-up, the control channel by default.

## Rules kept from upstream (they keep the app solid)
- No unwrap/expect/panic in non-test code; errors are `Result` with a message a user can act on.
- Colours from theme tokens; one camera path; layering geom → sketch/kernel/render → doc → io →
  engine → ui-egui → apps (analysis sits beside engine, used by ui-egui).
- Everything a user does is a command or a stored change of the design (undo works).
- Keep the licence files (LICENSE-MIT, LICENSE-APACHE, NOTICE, THIRD-PARTY-LICENSES.txt,
  ATTRIBUTION.md) and the About box's notices.

## Not kept
- Upstream's contribution process (claims, draft PRs, xtask CI), the wasm build, the rule that the
  product name is SolidCraft. Public text still never names or compares with commercial products
  (root CLAUDE.md).

## Building
`cd app && cargo build --release -p solidcraft --bin datum-mesh` (about 25 minutes from scratch).
Gmsh for development: `DATUM_GMSH=/path/to/gmsh`. The release workflow fetches `gmsh.exe` and builds
the setup.
