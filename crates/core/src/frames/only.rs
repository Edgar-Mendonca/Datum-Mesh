//! Members that only pull or only push (0.4.3): tension-only braces and cables, compression-only
//! struts and bearings. The model is solved with every member in; a tension-only member found
//! squeezed (or a compression-only one stretched) is taken out and the model solved again; one
//! taken out that its ends would now stretch (or squeeze) is put back. That repeats until nothing
//! changes. Each solve is linear, so the answer is exact for the members' final state.

use crate::frame::{FrameResults, MemberResult, STATIONS};
use crate::model::{Model, Only};
use crate::space::{SpaceMember, SpaceResults};

const MAX: usize = 40;

/// The members left out, and the model without them.
fn without(m: &Model, out: &[u32]) -> Model {
    let mut mm = m.clone();
    mm.members.retain(|b| !out.contains(&b.id));
    mm.loads.retain(|l| !matches!(l.at, crate::model::Target::Member { id } if out.contains(&id)));
    mm
}

/// Decide again which members are in: (changed, members out).
fn settle(m: &Model, out: &[u32], axial: &dyn Fn(u32) -> Option<f64>, stretch: &dyn Fn(u32) -> f64, scale: f64) -> (bool, Vec<u32>) {
    let tol = 1e-9 * scale.max(f64::MIN_POSITIVE);
    let mut next = vec![];
    for b in m.members.iter().filter(|b| b.only != Only::Both) {
        let is_out = out.contains(&b.id);
        let drop = if is_out {
            let d = stretch(b.id);
            // Put back where the ends would now pull it (tension) or push it (compression).
            !((b.only == Only::Tension && d > 0.0) || (b.only == Only::Compression && d < 0.0))
        } else {
            let n = axial(b.id).unwrap_or(0.0);
            (b.only == Only::Tension && n < -tol) || (b.only == Only::Compression && n > tol)
        };
        if drop {
            next.push(b.id);
        }
    }
    let mut a = out.to_vec();
    a.sort_unstable();
    next.sort_unstable();
    (a != next, next)
}

fn note(_m: &Model, _out: &[u32], it: usize, settled: bool) -> Vec<String> {
    let mut w = vec![];
    if !settled {
        w.push(format!("The tension-only and compression-only members did not settle in {it} rounds; the last state is shown."));
    }
    w
}

/// A plane frame.
pub fn frame(m: &Model) -> Result<FrameResults, String> {
    let mut out: Vec<u32> = vec![];
    for it in 1..=MAX {
        let mm = without(m, &out);
        let r = crate::frame::solve(&mm)?;
        let scale = r.members.iter().flat_map(|x| x.n.iter()).fold(0.0f64, |a, b| a.max(b.abs()));
        let pos = |id: u32| m.nodes.iter().position(|n| n.id == id);
        let axial = |id: u32| r.members.iter().find(|x| x.id == id).map(|x| x.n.iter().sum::<f64>() / x.n.len() as f64);
        let stretch = |id: u32| {
            let b = m.members.iter().find(|b| b.id == id).expect("member");
            let (Some(i), Some(j)) = (pos(b.a), pos(b.b)) else { return 0.0 };
            let (pa, pb) = (&m.nodes[i], &m.nodes[j]);
            let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
            let l = dx.hypot(dy);
            ((r.disp[j][0] - r.disp[i][0]) * dx + (r.disp[j][1] - r.disp[i][1]) * dy) / l
        };
        let (changed, next) = settle(m, &out, &axial, &stretch, scale);
        if !changed || it == MAX {
            return Ok(finish_frame(m, r, &out, it, !changed));
        }
        out = next;
    }
    unreachable!()
}

fn finish_frame(m: &Model, mut r: FrameResults, out: &[u32], it: usize, settled: bool) -> FrameResults {
    // The slack members' results: no force, their ends' movements straight between.
    let mut members = vec![];
    for b in &m.members {
        if let Some(x) = r.members.iter().find(|x| x.id == b.id) {
            members.push(x.clone());
            continue;
        }
        let (i, j) = (m.nodes.iter().position(|n| n.id == b.a).unwrap_or(0), m.nodes.iter().position(|n| n.id == b.b).unwrap_or(0));
        let (pa, pb) = (&m.nodes[i], &m.nodes[j]);
        let l = (pb.x - pa.x).hypot(pb.y - pa.y);
        let x: Vec<f64> = (0..STATIONS).map(|k| l * k as f64 / (STATIONS - 1) as f64).collect();
        let d = x.iter().map(|s| { let t = s / l; [r.disp[i][0] * (1.0 - t) + r.disp[j][0] * t, r.disp[i][1] * (1.0 - t) + r.disp[j][1] * t] }).collect();
        members.push(MemberResult { id: b.id, length: l, x, n: vec![0.0; STATIONS], v: vec![0.0; STATIONS], m: vec![0.0; STATIONS], d, stress: 0.0, util: 0.0, ends: [0.0; 6] });
    }
    r.members = members;
    r.slack = out.to_vec();
    r.warnings.extend(note(m, out, it, settled));
    r
}

/// A 3D frame (with its plates).
pub fn space(m: &Model) -> Result<SpaceResults, String> {
    let mut out: Vec<u32> = vec![];
    for it in 1..=MAX {
        let mm = without(m, &out);
        let r = crate::space::solve(&mm)?;
        let scale = r.members.iter().flat_map(|x| x.n.iter()).fold(0.0f64, |a, b| a.max(b.abs()));
        let pos = |id: u32| m.nodes.iter().position(|n| n.id == id);
        let axial = |id: u32| r.members.iter().find(|x| x.id == id).map(|x| x.n.iter().sum::<f64>() / x.n.len() as f64);
        let stretch = |id: u32| {
            let b = m.members.iter().find(|b| b.id == id).expect("member");
            let (Some(i), Some(j)) = (pos(b.a), pos(b.b)) else { return 0.0 };
            let (pa, pb) = (m.nodes[i].at(), m.nodes[j].at());
            let e = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
            let l = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
            (0..3).map(|k| (r.disp[j][k] - r.disp[i][k]) * e[k]).sum::<f64>() / l
        };
        let (changed, next) = settle(m, &out, &axial, &stretch, scale);
        if !changed || it == MAX {
            let mut r = r;
            let mut members = vec![];
            for b in &m.members {
                if let Some(x) = r.members.iter().find(|x| x.id == b.id) {
                    members.push(x.clone());
                    continue;
                }
                let (i, j) = (pos(b.a).unwrap_or(0), pos(b.b).unwrap_or(0));
                let (pa, pb) = (m.nodes[i].at(), m.nodes[j].at());
                let l = crate::model::dist3(pa, pb);
                let x: Vec<f64> = (0..STATIONS).map(|k| l * k as f64 / (STATIONS - 1) as f64).collect();
                let d = x.iter().map(|s| { let t = s / l; std::array::from_fn(|k| r.disp[i][k] * (1.0 - t) + r.disp[j][k] * t) }).collect();
                let z = vec![0.0; STATIONS];
                members.push(SpaceMember { id: b.id, length: l, x, n: z.clone(), vy: z.clone(), vz: z.clone(), t: z.clone(), my: z.clone(), mz: z, d, stress: 0.0, util: 0.0, axes: crate::space::axes(pa, pb, b.roll) });
            }
            r.members = members;
            r.slack = out.clone();
            r.warnings.extend(note(m, &out, it, !changed));
            return Ok(r);
        }
        out = next;
    }
    unreachable!()
}
