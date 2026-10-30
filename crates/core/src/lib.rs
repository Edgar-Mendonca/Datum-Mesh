//! Datum Mesh engine: the model and its edits, sections, meshing, finite elements, the solver,
//! results and the verification benchmarks. Pure Rust; the window only shows what this computes.

// Numerical code reads best with index loops and explicit NaN-rejecting comparisons.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::type_complexity, clippy::neg_cmp_op_on_partial_ord)]

pub mod bench;
pub mod cases;
pub mod doc;
pub mod dxf;
pub mod examples;
pub mod frame;
mod hardening;
pub mod heat;
pub mod materials;
pub mod modal;
pub mod mesh;
pub mod model;
mod order;
pub mod plane;
mod release;
pub mod sections;
pub mod sketch;
pub mod solid;
pub mod space;
pub mod steel;
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
    Space(space::SpaceResults),
    Solid(solid::SolidResults),
    /// 3D heat conduction (0.3.4).
    SolidHeat(solid::SolidHeatResults),
}

/// Natural frequencies of a frame or a part (0.2.2), or a frame's buckling (0.2.3), tagged by
/// kind for the window.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Modes {
    Frame(frame::FrameModes),
    Plane(plane::PartModes),
    Buckle(frame::FrameBuckling),
    /// 3D frames (0.2.6): natural frequencies, and buckling.
    Space(space::SpaceModes),
    Buckle3(space::SpaceBuckling),
    /// 3D solids (0.3.4).
    Solid(solid::SolidModes),
}

/// The lowest `count` natural frequencies and mode shapes of the model.
pub fn modes(m: &model::Model, count: usize) -> Result<Modes, String> {
    let count = count.clamp(1, 30);
    match m.analysis {
        model::Analysis::Frame => frame::modes(m, count).map(Modes::Frame),
        model::Analysis::Space => space::modes(m, count).map(Modes::Space),
        model::Analysis::Solid => solid::modes(m, count).map(Modes::Solid),
        model::Analysis::SolidHeat => Err("Natural frequencies are for 3D solids in stress, not heat conduction.".into()),
        model::Analysis::Heat => Err("Natural frequencies are for frames and for parts in stress (plane stress, plane strain or axisymmetric), not heat conduction.".into()),
        _ => plane::modes(m, count).map(Modes::Plane),
    }
}

/// The lowest `count` buckling load factors of a frame, plane (0.2.3) or 3D (0.2.6): every load
/// times λ buckles it.
pub fn buckling(m: &model::Model, count: usize) -> Result<Modes, String> {
    let count = count.clamp(1, 30);
    match m.analysis {
        model::Analysis::Frame => frame::buckling(m, count).map(Modes::Buckle),
        model::Analysis::Space => space::buckling(m, count).map(Modes::Buckle3),
        _ => Err("Buckling is for frames and trusses in this release; parts come later.".into()),
    }
}

/// Solve one load case or combination (0.3.3; None: every load as it is).
pub fn solve_for(m: &model::Model, which: Option<cases::Which>) -> Result<Results, String> {
    solve(&cases::select(m, which)?)
}

/// Buckling under one load case or combination (0.3.3).
pub fn buckling_for(m: &model::Model, count: usize, which: Option<cases::Which>) -> Result<Modes, String> {
    buckling(&cases::select(m, which)?, count)
}

/// Solve the model as its analysis says: frames, plane stress or strain, or heat conduction.
pub fn solve(m: &model::Model) -> Result<Results, String> {
    match m.analysis {
        model::Analysis::Frame => frame::solve(m).map(Results::Frame),
        model::Analysis::Stress | model::Analysis::Strain | model::Analysis::Axi => plane::solve(m).map(Results::Plane),
        model::Analysis::Heat => heat::solve(m).map(Results::Heat),
        model::Analysis::Space => space::solve(m).map(Results::Space),
        model::Analysis::Solid => solid::solve(m).map(Results::Solid),
        model::Analysis::SolidHeat => solid::heat(m).map(Results::SolidHeat),
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
                crate::Results::Space(r) => (r.imbalance, &r.warnings),
                crate::Results::Solid(r) => (r.imbalance, &r.warnings),
                crate::Results::SolidHeat(r) => (r.imbalance, &r.warnings),
            };
            assert!(imb < 1e-6, "{}: imbalance {imb:e}", e.id);
            assert!(warnings.is_empty(), "{}: {warnings:?}", e.id);
        }
    }

    #[test]
    fn every_example_vibrates_with_rising_frequencies() {
        for e in crate::examples::list().into_iter().filter(|e| !e.id.starts_with("blank") && !matches!(e.analysis, crate::model::Analysis::Heat | crate::model::Analysis::SolidHeat)) {
            let m = crate::examples::get(e.id).unwrap();
            let r = crate::modes(&m, 4).unwrap_or_else(|x| panic!("{}: {x}", e.id));
            let f: Vec<f64> = match &r { crate::Modes::Frame(x) => x.modes.iter().map(|m| m.f).collect(), crate::Modes::Plane(x) => x.modes.iter().map(|m| m.f).collect(), crate::Modes::Space(x) => x.modes.iter().map(|m| m.f).collect(), crate::Modes::Solid(x) => x.modes.iter().map(|m| m.f).collect(), crate::Modes::Buckle(_) | crate::Modes::Buckle3(_) => unreachable!() };
            assert_eq!(f.len(), 4, "{}", e.id);
            assert!(f[0] > 0.0 && f.windows(2).all(|w| w[1] >= w[0] * (1.0 - 1e-9)), "{}: {f:?}", e.id);
        }
        // A heat model has no frequencies, and says why.
        assert!(crate::modes(&crate::examples::get("fin").unwrap(), 3).unwrap_err().contains("heat"));
    }

    #[test]
    fn the_frame_examples_buckle_with_rising_factors() {
        for id in ["portal", "truss"] {
            let Ok(crate::Modes::Buckle(b)) = crate::buckling(&crate::examples::get(id).unwrap(), 3) else { panic!("{id}") };
            let all: Vec<f64> = b.modes.iter().map(|x| x.lambda).chain(b.bars.iter().map(|x| x.lambda)).collect();
            assert!(!all.is_empty() && all.iter().all(|l| *l > 0.0 && l.is_finite()), "{id}: {all:?}");
            assert!(b.modes.windows(2).all(|w| w[1].lambda >= w[0].lambda * (1.0 - 1e-9)), "{id}");
        }
        assert!(crate::buckling(&crate::examples::get("bracket").unwrap(), 3).unwrap_err().contains("frames"));
        // The 3D platform buckles too, with positive rising factors.
        let Ok(crate::Modes::Buckle3(b)) = crate::buckling(&crate::examples::get("platform").unwrap(), 3) else { panic!("platform") };
        assert!(!b.modes.is_empty() && b.modes.iter().all(|x| x.lambda > 0.0) && b.modes.windows(2).all(|w| w[1].lambda >= w[0].lambda * (1.0 - 1e-9)));
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
