# Datum Mesh: notes for Claude

Read `docs/VISION.md` and `docs/MEMORY.md` first. The vision (2026-10-31): one open-source platform for structural and finite-element analysis, our own solver; one engine of a core and modules, organised in analysis steps; the structures side (0.4.x) first, then nonlinear and dynamic solids (0.5+). It is the project's memory: architecture, decisions, roadmap, and a log of every session. The family design guide is `FAMILY.md` in `Edgar-Mendonca/Datum` (read-only from here; `docs/FAMILY.md` links to it). The brief this app started from is `briefs/DATUM-MESH.md` in the same hub repo.
At the end of every session that changes anything, update `docs/MEMORY.md`:
- tick the roadmap
- add a session-log entry
- record any new decisions
- refresh the architecture section if files moved

User preferences:
- Rust and Tauri 2, small and efficient code, Windows first. The same look, ribbon, help, printing and updater as every Datum app (FAMILY §2–9); copy from Datum Plan (and Datum Sim, Plan's skeleton for a canvas app) rather than reinventing.
- Ship in small numbered releases (0.1.0, 0.1.1, …). Every release appears in the in-app updater.
- Engine logic belongs in `crates/core` (pure Rust, unit-tested), not in the UI. The window never calculates results.
- Our own solver: write the mechanics from published methods (Bathe's *Finite Element Procedures*, Simo and Hughes, Newmark/HHT, Crisfield); never port another program's code, even if permissive. Outside code only as infrastructure (faer, METIS).
- Modules plug into the core through its small interface (element: stiffness, internal force, mass; material: stress and tangent) and carry their own benchmarks; keep them independent.
- From 0.7.0 the program is `app/`: a fork of SolidCraft (egui + wgpu, its own cargo workspace; read `app/AGENTS.md`). The analysis is in `app/crates/analysis` and the Simulation workspace in `app/crates/ui-egui/src/sim.rs`. The 0.6 window (SolidJS in `ui/`, Tauri in `src-tauri/`) stays as the reference for analyses not yet brought across.
- The engine and the `.dmesh` file use SI base units (m, N, Pa, K, W); the window converts for display only (`ui/src/units.ts`).
- Public words (the repo is public: README, CHANGELOG, NOTICE, site.json, help, the window, docs, this file, code comments): never name, compare with, or say the app copies, combines or targets a commercial product (FAMILY §13); no "…-style" or "as … does" comparisons. File-format names only where they name a file the app reads or writes. Open solvers and benchmark sets (CalculiX, Gmsh, NAFEMS) may be named. The names are listed in `tools/commercial-names.txt`; `hardening::public_words_name_no_commercial_product` and two window checks enforce it.
- Licences: MIT/BSD/Apache/MPL only in the app (check `app/` with `cargo deny --manifest-path app/Cargo.toml check`). GPL solvers and meshers (Gmsh, bundled from 0.7.0) only ever run as separate programs. Record every dependency in `NOTICE`.

Quality bar (brief §5):
- Every element and solver path has a test against a known answer; each is a line of the verification report (`crates/core/src/bench.rs`, written to `docs/verification/` and shown in Help › Verification).
- Never cut the benchmarks, the equilibrium check or the report.
- Report only checks that actually ran; say what was not verified.

Checks before pushing:
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (includes the `hardening` tests and the benchmarks)
- `npm run check:ui`, then `npm run test:ui` (dumps the engine fixture, builds the window, runs the Playwright checks)
- `cargo deny check` (licences, advisories, sources; CI runs it)
- The program: `cd app && cargo clippy -p datum-analysis -p solidcraft-ui-egui --no-deps -- -D warnings` (`--no-deps`: the inherited crates follow upstream's lints), `cargo test -p datum-analysis`, and a driven run under Xvfb (`--control PORT`; docs/MEMORY.md §1)
- A change you can see: check it in the real app (`cargo build -p datum-mesh && tests_ui/realapp.sh steps.sh shots/`) and add a UI check for every bug

Releasing:
1. Bump the version in `app/Cargo.toml` (the release reads it), `src-tauri/Cargo.toml` and `crates/core/Cargo.toml`.
2. Add a CHANGELOG section (`## x.y.z — date`).
3. Keep `docs/site.json` true with each release (`about`, `steps`, `highlights`, `roadmap`).
4. Run `release.yml` from the Actions tab (workflow_dispatch; tag pushes are blocked from Claude sessions) or push the tag `vX.Y.Z`. The workflow creates the tag and publishes `DatumMesh-x.y.z-setup.exe` and `SHA256SUMS.txt`.
