//! Datum Mesh engine: the model and its edits, sections, meshing, finite elements, the solver,
//! results and the verification benchmarks. Pure Rust; the window only shows what this computes.

// Numerical code reads best with index loops and explicit NaN-rejecting comparisons.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::type_complexity, clippy::neg_cmp_op_on_partial_ord)]

pub mod bench;
pub mod doc;
pub mod examples;
pub mod frame;
mod hardening;
pub mod heat;
pub mod materials;
pub mod mesh;
pub mod model;
pub mod plane;
pub mod sections;
pub mod sparse;

use serde::Serialize;

/// The results of any analysis, tagged by kind for the window.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Results {
    Frame(frame::FrameResults),
    Plane(plane::PlaneResults),
    Heat(heat::HeatResults),
}

/// Solve the model as its analysis says: frames, plane stress or strain, or heat conduction.
pub fn solve(m: &model::Model) -> Result<Results, String> {
    match m.analysis {
        model::Analysis::Frame => frame::solve(m).map(Results::Frame),
        model::Analysis::Stress | model::Analysis::Strain => plane::solve(m).map(Results::Plane),
        model::Analysis::Heat => heat::solve(m).map(Results::Heat),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_example_solves_and_balances() {
        for e in crate::examples::list().into_iter().filter(|e| !e.id.starts_with("blank")) {
            let m = crate::examples::get(e.id).unwrap();
            let r = crate::solve(&m).unwrap_or_else(|x| panic!("{}: {x}", e.id));
            let (imb, warnings) = match &r {
                crate::Results::Frame(r) => (r.imbalance, &r.warnings),
                crate::Results::Plane(r) => (r.imbalance, &r.warnings),
                crate::Results::Heat(r) => (r.imbalance, &r.warnings),
            };
            assert!(imb < 1e-6, "{}: imbalance {imb:e}", e.id);
            assert!(warnings.is_empty(), "{}: {warnings:?}", e.id);
        }
    }

    #[test]
    fn the_examples_agree_with_hand_checks() {
        // L-bracket: bending in the horizontal leg just past the fillet (x = 30 mm): 1.5 kN × 70 mm
        // over Z = t h²/6 = 10 × 20² / 6 mm³ is 158 MPa; the fillet raises the peak above that.
        let Ok(crate::Results::Plane(r)) = crate::solve(&crate::examples::get("bracket").unwrap()) else { panic!() };
        let vm = r.field("vm").max;
        assert!(vm > 158e6 && vm < 400e6, "bracket peak {vm:e}");
        // Lifting lug: 60 kN through a 120 × 20 mm base is 25 MPa on average.
        let Ok(crate::Results::Plane(r)) = crate::solve(&crate::examples::get("lug").unwrap()) else { panic!() };
        let fy: f64 = r.reactions.iter().map(|x| x.fy).sum();
        assert!((fy + 60e3).abs() < 1e-6 * 60e3, "{fy}");
        // Heat sink: every temperature between the air (25 °C) and the base (80 °C).
        let Ok(crate::Results::Heat(h)) = crate::solve(&crate::examples::get("fin").unwrap()) else { panic!() };
        let t = h.field("t");
        assert!(t.min > 298.15 && t.max <= 353.15 + 1e-9, "{} {}", t.min, t.max);
    }
}
