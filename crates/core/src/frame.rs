//! Frames and trusses in 2D: Euler–Bernoulli beam and truss elements, member loads as consistent
//! nodal loads, self-weight, supports and springs, the solve, and the results engineers read:
//! deflections, reactions, axial, shear and moment diagrams, stresses and utilisation, and the
//! equilibrium check.
//!
//! Signs: global x right, y up, moments counter-clockwise. Member local x runs from end a to end b,
//! local y is x turned 90° counter-clockwise. Axial force is positive in tension; the bending
//! moment is positive when it bends the member concave towards local +y (sagging for a beam drawn
//! left to right); shear V = dM/dx.

use crate::model::{Dir, LoadKind, MemberKind, Model, SupportKind, Target};
use crate::sections;
use crate::sparse::{Stats, System};
use serde::Serialize;

/// Points along each member where diagrams are given (ends included).
pub const STATIONS: usize = 41;

#[derive(Clone, Debug, Serialize)]
pub struct MemberResult {
    pub id: u32,
    pub length: f64,
    /// Positions along the member (m) and the internal forces there.
    pub x: Vec<f64>,
    pub n: Vec<f64>,
    pub v: Vec<f64>,
    pub m: Vec<f64>,
    /// Displacement (global x, y) at each station.
    pub d: Vec<[f64; 2]>,
    /// Largest |N|/A + |M|/Z along the member (Pa), and that over the yield strength.
    pub stress: f64,
    pub util: f64,
    /// Forces the nodes put on the member at end a and end b, local axes: [Fx, Fy, M] each.
    pub ends: [f64; 6],
}

#[derive(Clone, Debug, Serialize)]
pub struct Reaction {
    pub node: u32,
    pub fx: f64,
    pub fy: f64,
    pub m: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FrameResults {
    /// Node displacements in model order: ux, uy (m), rotation (rad).
    pub disp: Vec<[f64; 3]>,
    pub reactions: Vec<Reaction>,
    pub members: Vec<MemberResult>,
    /// The sum of every applied load (Fx, Fy, and M about the origin) and of every reaction:
    /// they balance when the solution is right.
    pub applied: [f64; 3],
    pub reacted: [f64; 3],
    pub stats: Stats,
    pub warnings: Vec<String>,
}

impl FrameResults {
    /// The largest imbalance between loads and reactions, relative to the loads.
    pub fn imbalance(&self) -> f64 {
        let scale = self.applied[0].abs().max(self.applied[1].abs()).max(1e-30);
        ((self.applied[0] + self.reacted[0]).abs().max((self.applied[1] + self.reacted[1]).abs())) / scale
    }
}

struct Elem {
    id: u32,
    kind: MemberKind,
    a: usize,
    b: usize,
    l: f64,
    c: f64,
    s: f64,
    ea: f64,
    ei: f64,
    area: f64,
    z: f64,
    fy: f64,
    /// Distributed load in local axes, linear from end a to end b: (px1, px2, qy1, qy2) in N/m.
    p: [f64; 4],
}

/// The 6×6 stiffness in local axes (dofs u_a, v_a, θ_a, u_b, v_b, θ_b).
fn k_local(e: &Elem) -> [[f64; 6]; 6] {
    let mut k = [[0.0; 6]; 6];
    let (l, ea, ei) = (e.l, e.ea, e.ei);
    let a = ea / l;
    k[0][0] = a;
    k[3][3] = a;
    k[0][3] = -a;
    k[3][0] = -a;
    if e.kind == MemberKind::Beam {
        let (k1, k2, k3, k4) = (12.0 * ei / l.powi(3), 6.0 * ei / l.powi(2), 4.0 * ei / l, 2.0 * ei / l);
        let t = [[k1, k2, -k1, k2], [k2, k3, -k2, k4], [-k1, -k2, k1, -k2], [k2, k4, -k2, k3]];
        let idx = [1, 2, 4, 5];
        for i in 0..4 {
            for j in 0..4 {
                k[idx[i]][idx[j]] = t[i][j];
            }
        }
    }
    k
}

/// Rotation from global to local for one node's (x, y, θ).
fn rot(e: &Elem, g: [f64; 3]) -> [f64; 3] {
    [e.c * g[0] + e.s * g[1], -e.s * g[0] + e.c * g[1], g[2]]
}
fn unrot(e: &Elem, l: [f64; 3]) -> [f64; 3] {
    [e.c * l[0] - e.s * l[1], e.s * l[0] + e.c * l[1], l[2]]
}

/// Nodal loads (local axes) equivalent to the member's distributed load: fixed-end values for a
/// beam, simple-span values (no end moments) for a truss bar.
fn equivalent(e: &Elem) -> [f64; 6] {
    let [p1, p2, q1, q2] = e.p;
    let l = e.l;
    // Axial: uniform p1 plus a triangle rising to (p2 − p1) at b.
    let (dp, dq) = (p2 - p1, q2 - q1);
    let fa = p1 * l / 2.0 + dp * l / 6.0;
    let fb = p1 * l / 2.0 + dp * l / 3.0;
    if e.kind == MemberKind::Truss {
        return [fa, q1 * l / 2.0 + dq * l / 6.0, 0.0, fb, q1 * l / 2.0 + dq * l / 3.0, 0.0];
    }
    // Transverse: uniform q1 (wL/2, wL²/12) plus a triangle 0 → dq at b (3qL/20, qL²/30; 7qL/20, −qL²/20).
    let va = q1 * l / 2.0 + 3.0 * dq * l / 20.0;
    let ma = q1 * l * l / 12.0 + dq * l * l / 30.0;
    let vb = q1 * l / 2.0 + 7.0 * dq * l / 20.0;
    let mb = -q1 * l * l / 12.0 - dq * l * l / 20.0;
    [fa, va, ma, fb, vb, mb]
}

/// Solve a frame or truss model.
pub fn solve(m: &Model) -> Result<FrameResults, String> {
    m.check()?;
    if m.members.is_empty() {
        return Err("There are no members to solve. Draw beams or truss bars first (Geometry › Beam).".into());
    }
    let idx: std::collections::HashMap<u32, usize> = m.nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
    let nn = m.nodes.len();
    let mut warnings = vec![];
    // Gravity for self-weight.
    let g = m.loads.iter().filter_map(|l| if let (LoadKind::Gravity { gx, gy }, Target::Body) = (&l.kind, &l.at) { Some([*gx, *gy]) } else { None }).fold([0.0, 0.0], |a, b| [a[0] + b[0], a[1] + b[1]]);
    let mut elems = Vec::with_capacity(m.members.len());
    for mb in &m.members {
        let (a, b) = (idx[&mb.a], idx[&mb.b]);
        let (na, nb) = (&m.nodes[a], &m.nodes[b]);
        let (dx, dy) = (nb.x - na.x, nb.y - na.y);
        let l = dx.hypot(dy);
        let sec = m.section(mb.section).ok_or("A member's section is missing.")?;
        let mat = m.material(mb.material).ok_or("A member's material is missing.")?;
        let pr = sections::props(&sec.shape)?;
        let mut e = Elem { id: mb.id, kind: mb.kind, a, b, l, c: dx / l, s: dy / l, ea: mat.e * pr.a, ei: mat.e * pr.i, area: pr.a, z: pr.z(), fy: mat.fy, p: [0.0; 4] };
        // Self-weight: ρ A g per length, resolved into the member's axes.
        let w = [mat.rho * pr.a * g[0], mat.rho * pr.a * g[1]];
        let (wx, wy) = (e.c * w[0] + e.s * w[1], -e.s * w[0] + e.c * w[1]);
        e.p = [wx, wx, wy, wy];
        elems.push(e);
    }
    for l in &m.loads {
        if let (LoadKind::Dist { w1, w2, dir }, Target::Member { id }) = (&l.kind, &l.at) {
            let e = elems.iter_mut().find(|e| e.id == *id).ok_or("A load is on a member that does not exist.")?;
            // Global loads are per length of the member; resolve them into its axes.
            let (gx, gy) = match dir {
                Dir::X => (1.0, 0.0),
                Dir::Y => (0.0, 1.0),
                Dir::Local => (-e.s, e.c),
            };
            let (ax, ay) = (e.c * gx + e.s * gy, -e.s * gx + e.c * gy);
            e.p[0] += w1 * ax;
            e.p[1] += w2 * ax;
            e.p[2] += w1 * ay;
            e.p[3] += w2 * ay;
        }
    }
    // Unknowns: ux, uy, θ for every node.
    let mut sys = System::new(3 * nn);
    let mut applied = [0.0; 3];
    let mut add_load = |sys: &mut System, node: usize, f: [f64; 3]| {
        let (x, y) = (m.nodes[node].x, m.nodes[node].y);
        for k in 0..3 {
            sys.f[3 * node + k] += f[k];
        }
        applied[0] += f[0];
        applied[1] += f[1];
        applied[2] += f[2] + x * f[1] - y * f[0];
    };
    for e in &elems {
        let k = k_local(e);
        // Global stiffness T' k T, with T the block rotation.
        let t = |i: usize, j: usize| -> f64 {
            let (bi, bj) = (i / 3, j / 3);
            if bi != bj {
                return 0.0;
            }
            match (i % 3, j % 3) {
                (0, 0) | (1, 1) => e.c,
                (0, 1) => e.s,
                (1, 0) => -e.s,
                (2, 2) => 1.0,
                _ => 0.0,
            }
        };
        let mut kt = [[0.0; 6]; 6];
        for i in 0..6 {
            for j in 0..6 {
                kt[i][j] = (0..6).map(|q| k[i][q] * t(q, j)).sum();
            }
        }
        let dof = |i: usize| if i < 3 { 3 * e.a + i } else { 3 * e.b + i - 3 };
        for i in 0..6 {
            for j in 0..6 {
                let v: f64 = (0..6).map(|q| t(q, i) * kt[q][j]).sum();
                sys.add(dof(i), dof(j), v);
            }
        }
        let f = equivalent(e);
        add_load(&mut sys, e.a, unrot(e, [f[0], f[1], f[2]]));
        add_load(&mut sys, e.b, unrot(e, [f[3], f[4], f[5]]));
    }
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Force { fx, fy }, Target::Node { id }) => add_load(&mut sys, idx[id], [*fx, *fy, 0.0]),
            (LoadKind::Moment { m: mz }, Target::Node { id }) => add_load(&mut sys, idx[id], [0.0, 0.0, *mz]),
            (LoadKind::Dist { .. } | LoadKind::Gravity { .. }, _) => {}
            (k, _) => warnings.push(format!("A {} load does not act on frames and was left out.", load_name(k))),
        }
    }
    // Supports.
    let mut held = vec![false; 3 * nn];
    for s in &m.supports {
        let Target::Node { id } = s.at else {
            warnings.push("A support that is not on a node was left out.".into());
            continue;
        };
        let i = idx[&id];
        match &s.kind {
            SupportKind::Fixed => held[3 * i..3 * i + 3].iter_mut().for_each(|h| *h = true),
            SupportKind::Pinned => held[3 * i..3 * i + 2].iter_mut().for_each(|h| *h = true),
            SupportKind::RollerX => held[3 * i + 1] = true,
            SupportKind::RollerY => held[3 * i] = true,
            SupportKind::Spring { kx, ky, kr } => {
                sys.add(3 * i, 3 * i, *kx);
                sys.add(3 * i + 1, 3 * i + 1, *ky);
                sys.add(3 * i + 2, 3 * i + 2, *kr);
            }
            k => warnings.push(format!("A {} support does not act on frames and was left out.", k_name(k))),
        }
    }
    // Rotations of nodes no beam holds (truss joints, loose nodes) are not unknowns; a node no
    // member reaches has no unknowns at all.
    let mut has_beam = vec![false; nn];
    let mut has_member = vec![false; nn];
    for e in &elems {
        for n in [e.a, e.b] {
            has_member[n] = true;
            has_beam[n] |= e.kind == MemberKind::Beam;
        }
    }
    let mut free_rot = vec![false; nn];
    for i in 0..nn {
        if !has_beam[i] {
            if sys.f[3 * i + 2].abs() > 0.0 {
                warnings.push(format!("Node {} has a moment on it but no beam to take it; the moment was left out.", i + 1));
                applied[2] -= sys.f[3 * i + 2];
                sys.f[3 * i + 2] = 0.0;
            }
            free_rot[i] = !held[3 * i + 2];
            held[3 * i + 2] = true;
        }
        if !has_member[i] {
            held[3 * i] = true;
            held[3 * i + 1] = true;
        }
    }
    let (u, stats) = sys.solve(&held, &vec![0.0; 3 * nn])?;
    // Reactions: K u − f at the held unknowns, and the spring forces.
    let ku = sys.mul(&u);
    let mut reactions = vec![];
    let mut reacted = [0.0; 3];
    for (i, n) in m.nodes.iter().enumerate() {
        let mut r = [0.0; 3];
        let sp = m.supports.iter().find(|s| s.at == Target::Node { id: n.id });
        for k in 0..3 {
            if held[3 * i + k] && !(k == 2 && free_rot[i]) && has_member[i] {
                r[k] = ku[3 * i + k] - sys.f[3 * i + k];
            }
        }
        if let Some(SupportKind::Spring { kx, ky, kr }) = sp.map(|s| &s.kind) {
            r = [-kx * u[3 * i], -ky * u[3 * i + 1], -kr * u[3 * i + 2]];
        }
        if sp.is_some() {
            // Tiny round-off is not a reaction.
            reactions.push(Reaction { node: n.id, fx: r[0], fy: r[1], m: r[2] });
            reacted[0] += r[0];
            reacted[1] += r[1];
            reacted[2] += r[2] + n.x * r[1] - n.y * r[0];
        }
    }
    let disp: Vec<[f64; 3]> = (0..nn).map(|i| [u[3 * i], u[3 * i + 1], if free_rot[i] || !has_beam[i] { 0.0 } else { u[3 * i + 2] }]).collect();
    let members = elems.iter().map(|e| member_result(e, &u)).collect();
    let span = m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(1.0);
    let umax = disp.iter().map(|d| d[0].hypot(d[1])).fold(0.0, f64::max);
    if umax > 0.1 * span {
        warnings.push(format!("The largest movement is {:.0} % of the model's size: far beyond small deflections, so this linear answer is not reliable. Check the supports, sections and units.", umax / span * 100.0));
    }
    Ok(FrameResults { disp, reactions, members, applied, reacted, stats, warnings })
}

fn load_name(k: &LoadKind) -> &'static str {
    match k {
        LoadKind::Pressure { .. } => "pressure",
        LoadKind::Traction { .. } => "edge",
        LoadKind::Flux { .. } => "heat flux",
        LoadKind::Convection { .. } => "convection",
        LoadKind::Heat { .. } => "heat generation",
        _ => "misplaced",
    }
}
fn k_name(k: &SupportKind) -> &'static str {
    match k {
        SupportKind::Roller => "roller-on-an-edge",
        SupportKind::Symmetry => "symmetry",
        SupportKind::Temperature { .. } => "temperature",
        _ => "misplaced",
    }
}

/// End forces, diagrams, deflected shape and stresses of one member.
fn member_result(e: &Elem, u: &[f64]) -> MemberResult {
    let ga = [u[3 * e.a], u[3 * e.a + 1], u[3 * e.a + 2]];
    let gb = [u[3 * e.b], u[3 * e.b + 1], u[3 * e.b + 2]];
    let (la, lb) = (rot(e, ga), rot(e, gb));
    let ul = [la[0], la[1], la[2], lb[0], lb[1], lb[2]];
    let k = k_local(e);
    let eq = equivalent(e);
    let mut ends = [0.0; 6];
    for i in 0..6 {
        ends[i] = (0..6).map(|j| k[i][j] * ul[j]).sum::<f64>() - eq[i];
    }
    let [p1, p2, q1, q2] = e.p;
    let l = e.l;
    let n_st = STATIONS;
    let x: Vec<f64> = (0..n_st).map(|i| l * i as f64 / (n_st - 1) as f64).collect();
    // Internal forces from the free body [0, x] (linear loads integrate exactly).
    let px = |s: f64| p1 * s + (p2 - p1) * s * s / (2.0 * l);
    let qv = |s: f64| q1 * s + (q2 - q1) * s * s / (2.0 * l);
    let qm = |s: f64| q1 * s * s / 2.0 + (q2 - q1) * s.powi(3) / (6.0 * l);
    let n: Vec<f64> = x.iter().map(|&s| -ends[0] - px(s)).collect();
    let v: Vec<f64> = x.iter().map(|&s| ends[1] + qv(s)).collect();
    let mut mm: Vec<f64> = x.iter().map(|&s| -ends[2] + ends[1] * s + qm(s)).collect();
    if e.kind == MemberKind::Truss {
        // A pinned bar carries end moments of zero; only its own span load bends it.
        mm = x.iter().map(|&s| ends[1] * s + qm(s)).collect();
    }
    // Deflection: with linear loads M is a cubic, so v'' = M/EI integrates exactly:
    // v(s) = v_a + θ_a s + (−M_a s²/2 + V_a s³/6 + q1 s⁴/24 + dq s⁵/120L) / EI, and the axial
    // movement u(s) = u_a + (−F_a s − p1 s²/2 − dp s³/6L) / EA. A truss bar's end rotation is not
    // an unknown: it is the one that brings the curve to v_b.
    let ma = if e.kind == MemberKind::Truss { 0.0 } else { -ends[2] };
    let (dq, dp) = (q2 - q1, p2 - p1);
    let vp = |s: f64| if e.ei > 0.0 { (ma * s * s / 2.0 + ends[1] * s.powi(3) / 6.0 + q1 * s.powi(4) / 24.0 + dq * s.powi(5) / (120.0 * l)) / e.ei } else { 0.0 };
    let (va, vb) = (ul[1], ul[4]);
    let theta_a = if e.kind == MemberKind::Beam { ul[2] } else { (vb - va - vp(l)) / l };
    let d: Vec<[f64; 2]> = x
        .iter()
        .map(|&s| {
            let vy = va + theta_a * s + vp(s);
            let ux = ul[0] + (-ends[0] * s - p1 * s * s / 2.0 - dp * s.powi(3) / (6.0 * l)) / e.ea;
            let g = unrot(e, [ux, vy, 0.0]);
            [g[0], g[1]]
        })
        .collect();
    let stress = (0..n_st).map(|i| n[i].abs() / e.area + mm[i].abs() / e.z).fold(0.0, f64::max);
    let util = if e.fy > 0.0 { stress / e.fy } else { 0.0 };
    MemberResult { id: e.id, length: l, x, n, v, m: mm, d, stress, util, ends }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, Load, Material, Section, SectionShape, Support};

    /// A model with one material (E = 200 GPa, no weight) and one rectangular section.
    pub(crate) fn base(e: f64) -> Doc {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let mut mt = Material::steel(1);
        mt.e = e;
        d.m.materials = vec![mt];
        d.m.sections = vec![Section { id: 2, name: "R".into(), shape: SectionShape::Rect { b: 0.1, h: 0.2 } }];
        d
    }
    fn sup(d: &mut Doc, x: f64, y: f64, kind: SupportKind) {
        let id = d.m.nodes.iter().find(|n| n.x == x && n.y == y).unwrap().id;
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind }) }).unwrap();
    }
    fn load(d: &mut Doc, at: Target, kind: LoadKind) {
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at, kind }) }).unwrap();
    }

    #[test]
    fn a_propped_cantilever_with_a_triangular_load() {
        // Fixed at x = 0, roller at x = L; load rising from 0 at the fixed end to q at the roller.
        // Compatibility (load peaking at the propped end): R_roller = 11qL/40; then statics about the
        // fixed end gives M_fixed = 11qL²/40 − qL²/3 = −7qL²/120 (hogging).
        let (l, q) = (4.0, -10e3);
        let mut d = base(200e9);
        for i in 0..4 {
            d.apply(Op::AddMember { from: [i as f64, 0.0], to: [i as f64 + 1.0, 0.0], kind: MemberKind::Beam, section: None, material: None }).unwrap();
        }
        sup(&mut d, 0.0, 0.0, SupportKind::Fixed);
        sup(&mut d, l, 0.0, SupportKind::RollerX);
        for (k, mb) in d.m.members.clone().iter().enumerate() {
            load(&mut d, Target::Member { id: mb.id }, LoadKind::Dist { w1: q * k as f64 / 4.0, w2: q * (k + 1) as f64 / 4.0, dir: Dir::Y });
        }
        let r = solve(&d.m).unwrap();
        let rr = r.reactions.iter().find(|x| x.fy.abs() > 0.0 && x.m == 0.0).unwrap();
        assert!((rr.fy - (-11.0 * q * l / 40.0)).abs() < 1e-6 * q.abs() * l, "{}", rr.fy);
        let m0 = r.members[0].m[0];
        assert!((m0 - 7.0 * q * l * l / 120.0).abs() < 1e-6 * q.abs() * l * l, "{m0}");
        assert!(r.imbalance() < 1e-9);
    }

    #[test]
    fn mechanisms_and_empty_models_say_why() {
        let mut d = base(200e9);
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [1.0, 0.0], kind: MemberKind::Beam, section: None, material: None }).unwrap();
        assert!(solve(&d.m).unwrap_err().contains("without resistance"));
        // A square of truss bars with no diagonal folds over.
        let mut t = base(200e9);
        for (a, b) in [([0.0, 0.0], [1.0, 0.0]), ([1.0, 0.0], [1.0, 1.0]), ([1.0, 1.0], [0.0, 1.0]), ([0.0, 1.0], [0.0, 0.0])] {
            t.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Truss, section: None, material: None }).unwrap();
        }
        sup(&mut t, 0.0, 0.0, SupportKind::Pinned);
        sup(&mut t, 1.0, 0.0, SupportKind::RollerX);
        let n2 = t.m.nodes[2].id;
        load(&mut t, Target::Node { id: n2 }, LoadKind::Force { fx: 1e3, fy: 0.0 });
        assert!(solve(&t.m).is_err());
        assert!(solve(&base(200e9).m).unwrap_err().contains("no members"));
    }

    #[test]
    fn a_rigid_movement_gives_no_force() {
        // A support settles by moving the whole (statically determinate) beam: no internal forces.
        // Here: springs instead, and the beam floats on two equal springs under a central load:
        // each spring takes half and the beam moves down rigidly by P/(2k) at its ends.
        let mut d = base(200e9);
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [2.0, 0.0], kind: MemberKind::Beam, section: None, material: None }).unwrap();
        d.apply(Op::AddMember { from: [2.0, 0.0], to: [4.0, 0.0], kind: MemberKind::Beam, section: None, material: None }).unwrap();
        sup(&mut d, 0.0, 0.0, SupportKind::Spring { kx: 1e9, ky: 1e6, kr: 0.0 });
        sup(&mut d, 4.0, 0.0, SupportKind::Spring { kx: 0.0, ky: 1e6, kr: 0.0 });
        let n1 = d.m.nodes[1].id;
        load(&mut d, Target::Node { id: n1 }, LoadKind::Force { fx: 0.0, fy: -2e3 });
        let r = solve(&d.m).unwrap();
        assert!((r.disp[0][1] + 1e-3).abs() < 1e-12);
        assert!(r.reactions.iter().all(|x| (x.fy - 1e3).abs() < 1e-6));
        assert!(r.imbalance() < 1e-12);
    }
}
