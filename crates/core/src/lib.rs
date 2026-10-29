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
pub mod wire;

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
        model::Analysis::Stress | model::Analysis::Strain | model::Analysis::Axi => plane::solve(m).map(Results::Plane),
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
        // Through the hole's centre the net section is (120 − 40) × 20 mm: 37.5 MPa on average;
        // the pin's bearing raises the peak well above that, but not to yield (355 MPa).
        let vm = r.field("vm").max;
        assert!(vm > 37.5e6 && vm < 355e6, "lug peak {vm:e}");
        // Heated plate: temperatures between the wall (20 °C) and the hot edge (220 °C); the stress
        // is below EαΔT = 504 MPa (the stress if the whole 200 K rise were held back) but not zero.
        let Ok(crate::Results::Plane(r)) = crate::solve(&crate::examples::get("hotplate").unwrap()) else { panic!() };
        let t = r.field("t");
        assert!((t.min - 293.15).abs() < 1e-6 && (t.max - 493.15).abs() < 1e-6, "{} {}", t.min, t.max);
        let vm = r.field("vm").max;
        assert!(vm > 10e6 && vm < 504e6, "hot plate peak {vm:e}");
        // Pressure vessel: halfway along the cylinder the hoop stress is close to the membrane value
        // pr/t at the mid radius (2 MPa × 105 mm / 10 mm = 21 MPa); the axial forces balance.
        let Ok(crate::Results::Plane(r)) = crate::solve(&crate::examples::get("vessel").unwrap()) else { panic!() };
        let mid = r.mesh.nodes.iter().enumerate().filter(|(_, p)| (p[1] - 0.1).abs() < 0.01).map(|(i, _)| r.field("sz").v[i]).collect::<Vec<_>>();
        let avg = mid.iter().sum::<f64>() / mid.len() as f64;
        assert!((avg - 21e6).abs() < 0.05 * 21e6, "vessel hoop {avg:e}");
        assert!(r.imbalance < 1e-6);
        // Heat sink: every temperature between the air (25 °C) and the base (80 °C).
        let Ok(crate::Results::Heat(h)) = crate::solve(&crate::examples::get("fin").unwrap()) else { panic!() };
        let t = h.field("t");
        assert!(t.min > 298.15 && t.max <= 353.15 + 1e-9, "{} {}", t.min, t.max);
    }
}
