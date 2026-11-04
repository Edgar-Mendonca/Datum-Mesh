//! Frames for animating results (0.6.4): snapshots of a solution as it goes, the load factor of a
//! nonlinear solve's increments or the times of a time history, with the movements of the points
//! the window draws and one colour value at each (von Mises stress for parts and solids). Kept in
//! single precision and thinned to at most `MAX` frames, so a long solve stays small to send.

use serde::Serialize;

/// The most frames kept (the first and last always).
pub const MAX: usize = 60;

/// One snapshot as a solver takes it, before thinning: the load factor or time, every node's
/// movements (3 per node, or 2 in the plane) and a colour value per node.
#[derive(Clone, Debug, Default)]
pub struct Snap {
    pub at: f64,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Frames {
    /// What `at` counts: "load" (the load factor, 0 to 1) or "time" (s).
    pub what: &'static str,
    /// The colour value's key (e.g. "vm"), or "" when there is none.
    pub field: &'static str,
    pub at: Vec<f64>,
    /// For each frame, the movements of the drawn points (x, y, z).
    pub disp: Vec<Vec<[f32; 3]>>,
    /// For each frame, the colour value at the drawn points (empty without a field).
    pub v: Vec<Vec<f32>>,
}

/// Which of `n` snapshots to keep: all of them up to `MAX`, else evenly spread, with the first and last.
pub fn pick(n: usize) -> Vec<usize> {
    if n <= MAX {
        return (0..n).collect();
    }
    let mut out: Vec<usize> = (0..MAX).map(|k| ((k as f64) * (n - 1) as f64 / (MAX - 1) as f64).round() as usize).collect();
    out.dedup();
    out
}

impl Frames {
    /// Frames from snapshots of every node (`dim` movements per node), kept at the drawn points
    /// `ids` (node numbers), with a still first frame at `at` = 0 unless the snapshots have one.
    pub fn from_snaps(what: &'static str, field: &'static str, snaps: &[Snap], ids: &[usize], dim: usize) -> Option<Frames> {
        if snaps.is_empty() {
            return None;
        }
        let mut f = Frames { what, field, ..Default::default() };
        if snaps[0].at > 0.0 {
            f.at.push(0.0);
            f.disp.push(vec![[0.0; 3]; ids.len()]);
            if !field.is_empty() {
                f.v.push(vec![0.0; ids.len()]);
            }
        }
        for k in pick(snaps.len()) {
            let s = &snaps[k];
            f.at.push(s.at);
            f.disp.push(ids.iter().map(|&q| {
                let mut d = [0.0f32; 3];
                for (c, x) in d.iter_mut().enumerate().take(dim) {
                    *x = s.u.get(dim * q + c).copied().unwrap_or(0.0);
                }
                d
            }).collect());
            if !field.is_empty() {
                f.v.push(ids.iter().map(|&q| s.v.get(q).copied().unwrap_or(0.0)).collect());
            }
        }
        Some(f)
    }
}

/// Von Mises stress of (xx, yy, zz, xy, yz, zx).
pub fn vm(s: &[f64; 6]) -> f64 {
    (0.5 * ((s[0] - s[1]).powi(2) + (s[1] - s[2]).powi(2) + (s[2] - s[0]).powi(2)) + 3.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5])).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonlinear_solves_and_histories_carry_frames_ending_at_the_result() {
        let close = |a: f32, b: f64| (a as f64 - b).abs() <= 1e-6 * b.abs().max(1e-9);
        // A 3D solid pulled past yield (8 increments): a still start, then up to the full load,
        // the last frame the result shown.
        let Ok(crate::Results::Solid(r)) = crate::solve_for(&crate::examples::get("yield3d").unwrap(), None) else { panic!() };
        let f = r.frames.unwrap();
        assert!(f.at.len() >= 3 && f.at[0] == 0.0 && (f.at.last().unwrap() - 1.0).abs() < 1e-9 && f.at.windows(2).all(|w| w[1] > w[0]), "{:?}", f.at);
        assert!(f.disp.iter().all(|d| d.len() == r.disp.len()) && f.v.iter().all(|v| v.len() == r.disp.len()));
        let last = f.disp.last().unwrap();
        assert!(last.iter().zip(&r.disp).all(|(a, b)| (0..3).all(|c| close(a[c], b[c]))));
        // A plane frame past its limit (arc length) and the quake's time history.
        let Ok(crate::Results::Frame(r)) = crate::solve_for(&crate::examples::get("lee").unwrap(), None) else { panic!() };
        let f = r.frames.unwrap();
        let stations: usize = r.members.iter().map(|m| m.d.len()).sum();
        assert!(f.at.len() > 3 && f.disp.iter().all(|d| d.len() == stations));
        let mut m = crate::examples::get("quake").unwrap();
        let st = &mut m.steps[0];
        st.kind = crate::model::StepKind::History;
        st.ground = true;
        st.time = vec![[0.0, 0.0], [0.2, 3.0], [0.4, -3.0], [0.6, 2.0], [0.8, 0.0]];
        st.dt = 0.005;
        st.duration = 3.0;
        let Ok(r) = crate::solve_for(&m, None) else { panic!() };
        let f = match r { crate::Results::Frame(r) => r.frames.unwrap(), crate::Results::Space(r) => r.frames.unwrap(), _ => panic!() };
        assert!(f.what == "time" && f.at.len() > 10 && f.at.len() <= MAX + 1 && f.at.windows(2).all(|w| w[1] > w[0]), "{:?}", f.at.len());
    }

    #[test]
    fn long_solves_are_thinned_to_the_limit_keeping_both_ends() {
        assert_eq!(pick(5), vec![0, 1, 2, 3, 4]);
        let p = pick(1000);
        assert!(p.len() <= MAX && p[0] == 0 && *p.last().unwrap() == 999);
        let snaps: Vec<Snap> = (1..=3).map(|k| Snap { at: k as f64 / 3.0, u: vec![k as f32; 6], v: vec![k as f32 * 10.0; 2] }).collect();
        let f = Frames::from_snaps("load", "vm", &snaps, &[1], 3).unwrap();
        assert_eq!(f.at, vec![0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
        assert_eq!(f.disp[3], vec![[3.0, 3.0, 3.0]]);
        assert_eq!(f.v[2], vec![20.0]);
        assert_eq!(f.disp[0], vec![[0.0; 3]]);
    }
}
