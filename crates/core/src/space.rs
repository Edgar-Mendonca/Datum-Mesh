//! Frames and trusses in 3D (0.2.4): beams that stretch, twist and bend about both axes of their
//! section, and truss bars. y is up, as in the plane; z is depth.
//!
//! Member axes: local x runs from end a to end b. Local y is the plane frame's local y (z × x)
//! for a member that does not run along z, and global y for one that does; `roll` turns local y
//! and z about x. The section's depth h lies along local y, so `i` (from h) bends about local z
//! and `iy` (from the width b) about local y. A frame drawn in the x-y plane therefore gives the
//! plane solver's numbers and signs.
//!
//! Internal forces at a station are those the far part puts on the near part: N (tension
//! positive), Vy and Mz as in the plane, Vz and My the same about the other axis (right-hand
//! rule), and the torque T about the member's axis.

use crate::frame::STATIONS;
use crate::model::{dist3, Dir, LoadKind, MemberKind, Model, SupportKind, Target};
use crate::sections;
use crate::release::Release;
use crate::sparse::{Stats, System};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SpaceMember {
    pub id: u32,
    pub length: f64,
    /// Positions along the member (m) and the internal forces there (N, N·m).
    pub x: Vec<f64>,
    pub n: Vec<f64>,
    pub vy: Vec<f64>,
    pub vz: Vec<f64>,
    pub t: Vec<f64>,
    pub my: Vec<f64>,
    pub mz: Vec<f64>,
    /// Displacement (global x, y, z) at each station.
    pub d: Vec<[f64; 3]>,
    /// Largest |N|/A + |Mz|/Z + |My|/Zy along the member (Pa), and that over the yield strength.
    pub stress: f64,
    pub util: f64,
    /// The member's local axes in global terms: x, y, z.
    pub axes: [[f64; 3]; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct SpaceReaction {
    pub node: u32,
    /// Fx, Fy, Fz (N) and Mx, My, Mz (N·m).
    pub f: [f64; 6],
}

#[derive(Clone, Debug, Serialize)]
pub struct SpaceResults {
    /// ux, uy, uz (m) and θx, θy, θz (rad) at every node.
    pub disp: Vec<[f64; 6]>,
    pub reactions: Vec<SpaceReaction>,
    pub members: Vec<SpaceMember>,
    /// Sums of the loads and of the reactions: forces along x, y, z and moments about the origin.
    pub applied: [f64; 6],
    pub reacted: [f64; 6],
    /// Force and moment scales for the equilibrium check.
    pub scale: [f64; 2],
    pub imbalance: f64,
    pub stats: Stats,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
struct Elem {
    id: u32,
    kind: MemberKind,
    a: usize,
    b: usize,
    l: f64,
    /// Rows: local x, y, z in global terms.
    r: [[f64; 3]; 3],
    ea: f64,
    eiz: f64,
    eiy: f64,
    gj: f64,
    area: f64,
    zz: f64,
    zy: f64,
    fy: f64,
    /// Distributed load in local axes, linear from a to b: (px1, px2, qy1, qy2, qz1, qz2) N/m.
    p: [f64; 6],
    at: f64,
    /// Mass per length ρA and polar mass per length ρ(Iz + Iy), for natural frequencies; the
    /// weaker EI, for a truss bar buckling on its own.
    rho_a: f64,
    rho_ip: f64,
    eimin: f64,
    /// Hinges at the start and end (beams): bending released both ways, twisting still carried,
    /// except where the node has no beam holding it rigidly (`twist`: a ball joint there).
    hinge: [bool; 2],
    twist: [bool; 2],
}

/// The condensation of a hinged beam (0.3.3), or None.
fn released(e: &Elem) -> Option<Release<12>> {
    if e.kind != MemberKind::Beam || !(e.hinge[0] || e.hinge[1]) {
        return None;
    }
    let mut rel = vec![];
    if e.hinge[0] {
        rel.extend(if e.twist[0] { &[3, 4, 5][..] } else { &[4, 5][..] });
    }
    if e.hinge[1] {
        rel.extend(if e.twist[1] { &[9, 10, 11][..] } else { &[10, 11][..] });
    }
    Some(Release::new(&k_local(e), &equivalent(e), &rel))
}
fn cond(e: &Elem, k: [[f64; 12]; 12]) -> [[f64; 12]; 12] {
    released(e).map_or(k, |r| r.mat(&k))
}
fn cond_load(e: &Elem) -> [f64; 12] {
    let f = equivalent(e);
    released(e).map_or(f, |r| r.load(&f))
}
/// The member's local end movements (θx, θy, θz in local axes), its hinged ends' own included.
fn ends_of(e: &Elem, u: &[f64], loaded: bool) -> [f64; 12] {
    let g = |n: usize, o: usize| to_local(e, [u[6 * n + o], u[6 * n + o + 1], u[6 * n + o + 2]]);
    let (ta, ra, tb, rb) = (g(e.a, 0), g(e.a, 3), g(e.b, 0), g(e.b, 3));
    let ul = [ta[0], ta[1], ta[2], ra[0], ra[1], ra[2], tb[0], tb[1], tb[2], rb[0], rb[1], rb[2]];
    released(e).map_or(ul, |r| r.full(&ul, loaded))
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A member's local axes (rows x, y, z) from its ends and roll.
pub fn axes(a: [f64; 3], b: [f64; 3], roll: f64) -> [[f64; 3]; 3] {
    let ex = unit([b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
    let zx = cross([0.0, 0.0, 1.0], ex);
    let ey0 = if dot(zx, zx).sqrt() < 1e-9 { [0.0, 1.0, 0.0] } else { unit(zx) };
    let ez0 = cross(ex, ey0);
    let (c, s) = (roll.cos(), roll.sin());
    let ey = [c * ey0[0] + s * ez0[0], c * ey0[1] + s * ez0[1], c * ey0[2] + s * ez0[2]];
    [ex, ey, cross(ex, ey)]
}

/// Global to local, and back, for a triple.
fn to_local(e: &Elem, g: [f64; 3]) -> [f64; 3] {
    [dot(e.r[0], g), dot(e.r[1], g), dot(e.r[2], g)]
}
fn to_global(e: &Elem, l: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|k| e.r[0][k] * l[0] + e.r[1][k] * l[1] + e.r[2][k] * l[2])
}

/// The 12×12 stiffness in local axes (u, v, w, θx, θy, θz at a, then at b).
fn k_local(e: &Elem) -> [[f64; 12]; 12] {
    let mut k = [[0.0; 12]; 12];
    let l = e.l;
    let mut put = |idx: &[usize], t: &[&[f64]]| {
        for (i, &p) in idx.iter().enumerate() {
            for (j, &q) in idx.iter().enumerate() {
                k[p][q] += t[i][j];
            }
        }
    };
    let a = e.ea / l;
    put(&[0, 6], &[&[a, -a], &[-a, a]]);
    if e.kind == MemberKind::Beam {
        let g = e.gj / l;
        put(&[3, 9], &[&[g, -g], &[-g, g]]);
        // Bending in the local x-y plane (v, θz), as in the plane frame.
        let ei = e.eiz;
        let (k1, k2, k3, k4) = (12.0 * ei / l.powi(3), 6.0 * ei / l.powi(2), 4.0 * ei / l, 2.0 * ei / l);
        put(&[1, 5, 7, 11], &[&[k1, k2, -k1, k2], &[k2, k3, -k2, k4], &[-k1, -k2, k1, -k2], &[k2, k4, -k2, k3]]);
        // Bending in the local x-z plane (w, θy): θy = −dw/dx, so the coupling terms change sign.
        let ei = e.eiy;
        let (k1, k2, k3, k4) = (12.0 * ei / l.powi(3), 6.0 * ei / l.powi(2), 4.0 * ei / l, 2.0 * ei / l);
        put(&[2, 4, 8, 10], &[&[k1, -k2, -k1, -k2], &[-k2, k3, k2, k4], &[-k1, k2, k1, k2], &[-k2, k4, k2, k3]]);
    }
    k
}

/// Nodal loads (local axes) equivalent to the member's distributed load and warming.
fn equivalent(e: &Elem) -> [f64; 12] {
    let [p1, p2, y1, y2, z1, z2] = e.p;
    let l = e.l;
    let dp = p2 - p1;
    let fa = p1 * l / 2.0 + dp * l / 6.0 - e.ea * e.at;
    let fb = p1 * l / 2.0 + dp * l / 3.0 + e.ea * e.at;
    let shear = |q1: f64, q2: f64| (q1 * l / 2.0 + 3.0 * (q2 - q1) * l / 20.0, q1 * l / 2.0 + 7.0 * (q2 - q1) * l / 20.0);
    let (va, vb) = shear(y1, y2);
    let (wa, wb) = shear(z1, z2);
    if e.kind == MemberKind::Truss {
        let simple = |q1: f64, q2: f64| (q1 * l / 2.0 + (q2 - q1) * l / 6.0, q1 * l / 2.0 + (q2 - q1) * l / 3.0);
        let ((va, vb), (wa, wb)) = (simple(y1, y2), simple(z1, z2));
        return [fa, va, wa, 0.0, 0.0, 0.0, fb, vb, wb, 0.0, 0.0, 0.0];
    }
    let ends = |q1: f64, q2: f64| (q1 * l * l / 12.0 + (q2 - q1) * l * l / 30.0, -q1 * l * l / 12.0 - (q2 - q1) * l * l / 20.0);
    let (mza, mzb) = ends(y1, y2);
    let (mya, myb) = ends(z1, z2);
    [fa, va, wa, 0.0, -mya, mza, fb, vb, wb, 0.0, -myb, mzb]
}

/// A local 12×12 matrix turned into global axes, entry by entry (global dof, global dof, value).
fn scatter(e: &Elem, k: &[[f64; 12]; 12], mut out: impl FnMut(usize, usize, f64)) {
    // T is block diagonal with R in each 3×3 block: K_global = Tᵀ k T.
    let t = |i: usize, j: usize| if i / 3 == j / 3 { e.r[i % 3][j % 3] } else { 0.0 };
    let mut kt = [[0.0; 12]; 12];
    for i in 0..12 {
        for j in 0..12 {
            let b = j / 3 * 3;
            kt[i][j] = (b..b + 3).map(|q| k[i][q] * t(q, j)).sum();
        }
    }
    let dof = |i: usize| if i < 6 { 6 * e.a + i } else { 6 * e.b + i - 6 };
    for i in 0..12 {
        let b = i / 3 * 3;
        for j in 0..12 {
            let v: f64 = (b..b + 3).map(|q| t(q, i) * kt[q][j]).sum();
            if v != 0.0 {
                out(dof(i), dof(j), v);
            }
        }
    }
}

fn load_name(k: &LoadKind) -> &'static str {
    match k {
        LoadKind::Pressure { .. } => "pressure",
        LoadKind::Traction { .. } => "edge force",
        LoadKind::Bearing { .. } => "pin",
        LoadKind::Flux { .. } => "heat flux",
        LoadKind::Convection { .. } => "convection",
        LoadKind::Heat { .. } => "heat generation",
        _ => "member or node",
    }
}

/// A 3D frame's equations before solving, as for plane frames (`frame::Assembly`): `parts` are
/// the elements solved for (beams cut into pieces on request), `spans[i]` those of member i;
/// nodes from `m.nodes.len()` on are the pieces' inner nodes, at `pos`.
struct Assembly {
    sys: System,
    held: Vec<bool>,
    elems: Vec<Elem>,
    parts: Vec<Elem>,
    spans: Vec<std::ops::Range<usize>>,
    free_rot: Vec<bool>,
    has_member: Vec<bool>,
    has_beam: Vec<bool>,
    applied: [f64; 6],
    warnings: Vec<String>,
}

/// Cut a beam into `pieces` equal elements, inner nodes numbered from `first`; its distributed
/// load is shared out along the pieces. A truss bar is not cut.
fn split(e: &Elem, pieces: usize, first: usize) -> Vec<Elem> {
    if e.kind != MemberKind::Beam || pieces <= 1 {
        return vec![e.clone()];
    }
    let p = e.p;
    (0..pieces)
        .map(|k| {
            let (s0, s1) = (k as f64 / pieces as f64, (k + 1) as f64 / pieces as f64);
            let mut c = e.clone();
            c.a = if k == 0 { e.a } else { first + k - 1 };
            c.b = if k == pieces - 1 { e.b } else { first + k };
            c.l = e.l / pieces as f64;
            c.hinge = [k == 0 && e.hinge[0], k == pieces - 1 && e.hinge[1]];
            for j in 0..3 {
                let (q1, q2) = (p[2 * j], p[2 * j + 1]);
                c.p[2 * j] = q1 + (q2 - q1) * s0;
                c.p[2 * j + 1] = q1 + (q2 - q1) * s1;
            }
            c
        })
        .collect()
}

fn assemble(m: &Model, pieces: usize) -> Result<Assembly, String> {
    m.check()?;
    if m.members.is_empty() {
        return Err("There are no members to solve. Draw beams or truss bars first (Geometry › Beam).".into());
    }
    let idx: std::collections::HashMap<u32, usize> = m.nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
    let nn = m.nodes.len();
    let mut warnings = vec![];
    let g = m.loads.iter().filter_map(|l| if let (LoadKind::Gravity { gx, gy, gz }, Target::Body) = (&l.kind, &l.at) { Some([*gx, *gy, *gz]) } else { None }).fold([0.0; 3], |a, b| [a[0] + b[0], a[1] + b[1], a[2] + b[2]]);
    let warming: f64 = m.loads.iter().filter_map(|l| if let (LoadKind::Warming { dt }, Target::Body) = (&l.kind, &l.at) { Some(*dt) } else { None }).sum();
    let mut elems = Vec::with_capacity(m.members.len());
    for mb in &m.members {
        let (a, b) = (idx[&mb.a], idx[&mb.b]);
        let (pa, pb) = (m.nodes[a].at(), m.nodes[b].at());
        let sec = m.section(mb.section).ok_or("A member's section is missing.")?;
        let mat = m.material(mb.material).ok_or("A member's material is missing.")?;
        let pr = sections::props(&sec.shape)?;
        let gm = mat.e / (2.0 * (1.0 + mat.nu));
        let mut e = Elem { id: mb.id, kind: mb.kind, a, b, l: dist3(pa, pb), r: axes(pa, pb, mb.roll), ea: mat.e * pr.a, eiz: mat.e * pr.i, eiy: mat.e * pr.iy, gj: gm * pr.j, area: pr.a, zz: pr.z(), zy: pr.zy(), fy: mat.fy, p: [0.0; 6], at: mat.alpha * warming, rho_a: mat.rho * pr.a, rho_ip: mat.rho * (pr.i + pr.iy), eimin: mat.e * pr.i.min(pr.iy), hinge: mb.hinge, twist: [false; 2] };
        let w = to_local(&e, [mat.rho * pr.a * g[0], mat.rho * pr.a * g[1], mat.rho * pr.a * g[2]]);
        e.p = [w[0], w[0], w[1], w[1], w[2], w[2]];
        elems.push(e);
    }
    for l in &m.loads {
        if let (LoadKind::Dist { w1, w2, dir }, Target::Member { id }) = (&l.kind, &l.at) {
            let e = elems.iter_mut().find(|e| e.id == *id).ok_or("A load is on a member that does not exist.")?;
            let gdir = match dir {
                Dir::X => [1.0, 0.0, 0.0],
                Dir::Y => [0.0, 1.0, 0.0],
                Dir::Z => [0.0, 0.0, 1.0],
                Dir::Local => e.r[1],
            };
            let ax = to_local(e, gdir);
            for k in 0..3 {
                e.p[2 * k] += w1 * ax[k];
                e.p[2 * k + 1] += w2 * ax[k];
            }
        }
    }
    let mut parts = vec![];
    let mut spans = vec![];
    let mut pos: Vec<[f64; 3]> = m.nodes.iter().map(|n| n.at()).collect();
    for e in &elems {
        let pcs = split(e, pieces, pos.len());
        let (a, b) = (pos[e.a], pos[e.b]);
        for k in 1..pcs.len() {
            let t = k as f64 / pcs.len() as f64;
            pos.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]);
        }
        spans.push(parts.len()..parts.len() + pcs.len());
        parts.extend(pcs);
    }
    let nt = pos.len();
    // Unknowns: ux, uy, uz, θx, θy, θz for every node (and every inner node of a cut beam).
    let mut sys = System::new(6 * nt);
    let mut applied = [0.0; 6];
    let mut add_load = |sys: &mut System, node: usize, f: [f64; 6]| {
        let r = pos[node];
        for k in 0..6 {
            sys.f[6 * node + k] += f[k];
        }
        let mo = cross(r, [f[0], f[1], f[2]]);
        for k in 0..3 {
            applied[k] += f[k];
            applied[3 + k] += f[3 + k] + mo[k];
        }
    };
    // Which nodes a beam holds rigidly; a hinged end at a node none does is a ball joint, free to
    // twist too (unless both ends would be: then the member keeps its twisting, as nothing turns
    // it at either end).
    let mut has_beam = vec![false; nt];
    let mut has_member = vec![false; nt];
    for e in &parts {
        for n in [e.a, e.b] {
            has_member[n] = true;
            has_beam[n] |= e.kind == MemberKind::Beam && !((n == e.a && e.hinge[0]) || (n == e.b && e.hinge[1]));
        }
    }
    for e in parts.iter_mut() {
        let t = [e.hinge[0] && !has_beam[e.a], e.hinge[1] && !has_beam[e.b]];
        e.twist = if t[0] && t[1] { [false; 2] } else { t };
    }
    // The whole members (for their results) take their end pieces' joints.
    for (e, sp) in elems.iter_mut().zip(&spans) {
        e.twist = [parts[sp.start].twist[0], parts[sp.end - 1].twist[1]];
    }
    for e in &parts {
        scatter(e, &cond(e, k_local(e)), |i, j, v| sys.add(i, j, v));
        let f = cond_load(e);
        let (fa, ma) = (to_global(e, [f[0], f[1], f[2]]), to_global(e, [f[3], f[4], f[5]]));
        let (fb, mb) = (to_global(e, [f[6], f[7], f[8]]), to_global(e, [f[9], f[10], f[11]]));
        add_load(&mut sys, e.a, [fa[0], fa[1], fa[2], ma[0], ma[1], ma[2]]);
        add_load(&mut sys, e.b, [fb[0], fb[1], fb[2], mb[0], mb[1], mb[2]]);
    }
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Force { fx, fy, fz }, Target::Node { id }) => add_load(&mut sys, idx[id], [*fx, *fy, *fz, 0.0, 0.0, 0.0]),
            (LoadKind::Moment { m: mz, mx, my }, Target::Node { id }) => add_load(&mut sys, idx[id], [0.0, 0.0, 0.0, *mx, *my, *mz]),
            (LoadKind::Dist { .. } | LoadKind::Gravity { .. } | LoadKind::Warming { .. }, _) => {}
            (k, _) => warnings.push(format!("A {} load does not act on frames and was left out.", load_name(k))),
        }
    }
    let mut held = vec![false; 6 * nt];
    for s in &m.supports {
        let Target::Node { id } = s.at else {
            warnings.push("A support that is not on a node was left out.".into());
            continue;
        };
        let i = idx[&id];
        let h = &mut held[6 * i..6 * i + 6];
        match &s.kind {
            SupportKind::Fixed => h.iter_mut().for_each(|h| *h = true),
            SupportKind::Pinned => h[..3].iter_mut().for_each(|h| *h = true),
            SupportKind::RollerX => h[1] = true,
            SupportKind::RollerY => h[0] = true,
            SupportKind::Spring { kx, ky, kr, kz, krx, kry } => {
                for (k, v) in [kx, ky, kz, krx, kry, kr].iter().enumerate() {
                    sys.add(6 * i + k, 6 * i + k, **v);
                }
            }
            _ => warnings.push("Roller, symmetry and temperature supports go on parts; one was left out.".into()),
        }
    }
    // Rotations of nodes no beam holds are not unknowns; a node no member reaches has none.
    let mut free_rot = vec![false; nt];
    for i in 0..nn {
        if !has_beam[i] {
            if sys.f[6 * i + 3..6 * i + 6].iter().any(|f| *f != 0.0) {
                warnings.push(format!("Node {} has a moment on it but no beam to take it; the moment was left out.", i + 1));
                for k in 3..6 {
                    applied[k] -= sys.f[6 * i + k];
                    sys.f[6 * i + k] = 0.0;
                }
            }
            free_rot[i] = !held[6 * i + 3..6 * i + 6].iter().all(|h| *h);
            held[6 * i + 3..6 * i + 6].iter_mut().for_each(|h| *h = true);
        }
        if !has_member[i] {
            if sys.f[6 * i..6 * i + 3].iter().any(|f| *f != 0.0) {
                warnings.push(format!("Node {} has a load on it but no member reaches it; the load was left out.", i + 1));
                let f = [sys.f[6 * i], sys.f[6 * i + 1], sys.f[6 * i + 2]];
                let mo = cross(m.nodes[i].at(), f);
                for k in 0..3 {
                    applied[k] -= f[k];
                    applied[3 + k] -= mo[k];
                    sys.f[6 * i + k] = 0.0;
                }
            }
            held[6 * i..6 * i + 3].iter_mut().for_each(|h| *h = true);
        }
    }
    Ok(Assembly { sys, held, elems, parts, spans, free_rot, has_member, has_beam, applied, warnings })
}

/// Solve a 3D frame or truss model.
pub fn solve(m: &Model) -> Result<SpaceResults, String> {
    let Assembly { sys, held, elems, free_rot, has_member, has_beam, applied, mut warnings, .. } = assemble(m, 1)?;
    let nn = m.nodes.len();
    let reach = m.nodes.iter().map(|n| dist3(n.at(), [0.0; 3])).fold(0.0, f64::max) + span(m);
    let fscale: f64 = (0..nn).map(|i| (0..3).map(|k| sys.f[6 * i + k].abs()).sum::<f64>()).sum();
    let scale = [fscale, fscale * reach + (0..nn).map(|i| (3..6).map(|k| sys.f[6 * i + k].abs()).sum::<f64>()).sum::<f64>()];
    if scale[0] == 0.0 && scale[1] == 0.0 {
        warnings.push("There are no loads on the model, so nothing moves. Add loads on the Loads tab.".into());
    }
    let (u, stats) = sys.solve(&held, &vec![0.0; 6 * nn]).map_err(|e| {
        if elems.iter().any(|e| e.kind == MemberKind::Beam) {
            format!("{e} In 3D, a beam held only by pins at its ends can spin about its own axis: fix one end, or hold its turning with a spring.")
        } else {
            e
        }
    })?;
    let ku = sys.mul(&u);
    let mut reactions = vec![];
    let mut reacted = [0.0; 6];
    for (i, n) in m.nodes.iter().enumerate() {
        let Some(sp) = m.supports.iter().find(|s| s.at == Target::Node { id: n.id }) else { continue };
        let mut r = [0.0; 6];
        if let SupportKind::Spring { kx, ky, kr, kz, krx, kry } = &sp.kind {
            for (k, v) in [kx, ky, kz, krx, kry, kr].iter().enumerate() {
                r[k] = -**v * u[6 * i + k];
            }
        } else if has_member[i] {
            for k in 0..6 {
                if held[6 * i + k] && !(k >= 3 && free_rot[i]) {
                    r[k] = ku[6 * i + k] - sys.f[6 * i + k];
                }
            }
        }
        let mo = cross(n.at(), [r[0], r[1], r[2]]);
        for k in 0..3 {
            reacted[k] += r[k];
            reacted[3 + k] += r[3 + k] + mo[k];
        }
        reactions.push(SpaceReaction { node: n.id, f: r });
    }
    let disp: Vec<[f64; 6]> = (0..nn).map(|i| std::array::from_fn(|k| if k >= 3 && (free_rot[i] || !has_beam[i]) { 0.0 } else { u[6 * i + k] })).collect();
    let members = elems.iter().map(|e| member_result(e, &u)).collect();
    let umax = disp.iter().map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()).fold(0.0, f64::max);
    if umax > 0.1 * span(m) {
        warnings.push(format!("The largest movement is {:.0} % of the model's size: far beyond small deflections, so this linear answer is not reliable. Check the supports, sections and units.", umax / span(m) * 100.0));
    }
    let f = if scale[0] > 0.0 { (0..3).map(|k| (applied[k] + reacted[k]).abs()).fold(0.0, f64::max) / scale[0] } else { 0.0 };
    let mo = if scale[1] > 0.0 { (3..6).map(|k| (applied[k] + reacted[k]).abs()).fold(0.0, f64::max) / scale[1] } else { 0.0 };
    Ok(SpaceResults { disp, reactions, members, applied, reacted, scale, imbalance: f.max(mo), stats, warnings })
}

/// The model's largest extent along x, y or z.
fn span(m: &Model) -> f64 {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for n in &m.nodes {
        for (k, v) in n.at().iter().enumerate() {
            lo[k] = lo[k].min(*v);
            hi[k] = hi[k].max(*v);
        }
    }
    (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max).max(1e-9)
}

fn member_result(e: &Elem, u: &[f64]) -> SpaceMember {
    let ul = ends_of(e, u, true);
    let k = k_local(e);
    let eq = equivalent(e);
    let f: Vec<f64> = (0..12).map(|i| (0..12).map(|j| k[i][j] * ul[j]).sum::<f64>() - eq[i]).collect();
    let [p1, p2, y1, y2, z1, z2] = e.p;
    let l = e.l;
    let x: Vec<f64> = (0..STATIONS).map(|i| l * i as f64 / (STATIONS - 1) as f64).collect();
    // Loads on [0, s]: their sum and their moment about the cut (linear loads integrate exactly).
    let sum = |q1: f64, q2: f64, s: f64| q1 * s + (q2 - q1) * s * s / (2.0 * l);
    let mom = |q1: f64, q2: f64, s: f64| q1 * s * s / 2.0 + (q2 - q1) * s.powi(3) / (6.0 * l);
    let truss = e.kind == MemberKind::Truss;
    let n: Vec<f64> = x.iter().map(|&s| -f[0] - sum(p1, p2, s)).collect();
    let vy: Vec<f64> = x.iter().map(|&s| f[1] + sum(y1, y2, s)).collect();
    let vz: Vec<f64> = x.iter().map(|&s| f[2] + sum(z1, z2, s)).collect();
    let t: Vec<f64> = x.iter().map(|_| if truss { 0.0 } else { -f[3] }).collect();
    // A pinned bar carries no end moments; only its own span load bends it.
    let (fmy, fmz) = if truss { (0.0, 0.0) } else { (f[4], f[5]) };
    let mz: Vec<f64> = x.iter().map(|&s| -fmz + f[1] * s + mom(y1, y2, s)).collect();
    let my: Vec<f64> = x.iter().map(|&s| -fmy - f[2] * s - mom(z1, z2, s)).collect();
    // Deflections: EI v'' = Mz and EIy w'' = −My integrate exactly for linear loads.
    let curve = |m0: f64, v0: f64, q1: f64, q2: f64, ei: f64, s: f64| if ei > 0.0 { (m0 * s * s / 2.0 + v0 * s.powi(3) / 6.0 + q1 * s.powi(4) / 24.0 + (q2 - q1) * s.powi(5) / (120.0 * l)) / ei } else { 0.0 };
    let vp = |s: f64| curve(-fmz, f[1], y1, y2, e.eiz, s);
    let wp = |s: f64| curve(fmy, f[2], z1, z2, e.eiy, s);
    let slope_v = if truss { (ul[7] - ul[1] - vp(l)) / l } else { ul[5] };
    let slope_w = if truss { (ul[8] - ul[2] - wp(l)) / l } else { -ul[4] };
    let d: Vec<[f64; 3]> = x
        .iter()
        .map(|&s| {
            let ux = ul[0] + (-f[0] * s - p1 * s * s / 2.0 - (p2 - p1) * s.powi(3) / (6.0 * l)) / e.ea + e.at * s;
            to_global(e, [ux, ul[1] + slope_v * s + vp(s), ul[2] + slope_w * s + wp(s)])
        })
        .collect();
    let stress = (0..STATIONS).map(|i| n[i].abs() / e.area + mz[i].abs() / e.zz + my[i].abs() / e.zy).fold(0.0, f64::max);
    let util = if e.fy > 0.0 { stress / e.fy } else { 0.0 };
    SpaceMember { id: e.id, length: l, x, n, vy, vz, t, my, mz, d, stress, util, axes: e.r }
}

// ---------- natural frequencies and buckling (0.2.6) ----------

/// Put a 4×4 bending block into a 12×12 matrix at the dofs `idx`, times `f`.
fn put4(k: &mut [[f64; 12]; 12], idx: [usize; 4], t: [[f64; 4]; 4], f: f64) {
    for i in 0..4 {
        for j in 0..4 {
            k[idx[i]][idx[j]] += f * t[i][j];
        }
    }
}
/// Flip the coupling signs of a bending block for the x-z plane, where θy = −dw/dx.
fn flip(t: [[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let s = [1.0, -1.0, 1.0, -1.0];
    std::array::from_fn(|i| std::array::from_fn(|j| s[i] * s[j] * t[i][j]))
}

/// The consistent mass in local axes: linear along the axis and in twist (with the polar mass),
/// cubic in bending both ways; a truss bar's mass moves with its ends in every direction.
fn m_local(e: &Elem) -> [[f64; 12]; 12] {
    let (l, m) = (e.l, e.rho_a * e.l);
    let mut k = [[0.0; 12]; 12];
    let lin = |k: &mut [[f64; 12]; 12], a: usize, b: usize, f: f64| {
        k[a][a] += 2.0 * f / 6.0;
        k[b][b] += 2.0 * f / 6.0;
        k[a][b] += f / 6.0;
        k[b][a] += f / 6.0;
    };
    lin(&mut k, 0, 6, m);
    if e.kind == MemberKind::Truss {
        lin(&mut k, 1, 7, m);
        lin(&mut k, 2, 8, m);
        return k;
    }
    lin(&mut k, 3, 9, e.rho_ip * l);
    let t = [[156.0, 22.0 * l, 54.0, -13.0 * l], [22.0 * l, 4.0 * l * l, 13.0 * l, -3.0 * l * l], [54.0, 13.0 * l, 156.0, -22.0 * l], [-13.0 * l, -3.0 * l * l, -22.0 * l, 4.0 * l * l]];
    put4(&mut k, [1, 5, 7, 11], t, m / 420.0);
    put4(&mut k, [2, 4, 8, 10], flip(t), m / 420.0);
    k
}

/// The geometric stiffness in local axes for an axial force n (tension positive): consistent
/// cubic bending both ways for a beam; the string stiffness n/L across a truss bar. Twisting
/// under axial force is left out: without warping stiffness an open section (an I, a channel)
/// would twist at a small fraction of its real load, so 3D buckling is flexural only.
fn kg_local(e: &Elem, n: f64) -> [[f64; 12]; 12] {
    let l = e.l;
    let mut k = [[0.0; 12]; 12];
    let lin = |k: &mut [[f64; 12]; 12], a: usize, b: usize, f: f64| {
        k[a][a] += f;
        k[b][b] += f;
        k[a][b] -= f;
        k[b][a] -= f;
    };
    if e.kind == MemberKind::Truss {
        lin(&mut k, 1, 7, n / l);
        lin(&mut k, 2, 8, n / l);
        return k;
    }
    let t = [[36.0, 3.0 * l, -36.0, 3.0 * l], [3.0 * l, 4.0 * l * l, -3.0 * l, -l * l], [-36.0, -3.0 * l, 36.0, -3.0 * l], [3.0 * l, -l * l, -3.0 * l, 4.0 * l * l]];
    put4(&mut k, [1, 5, 7, 11], t, n / (30.0 * l));
    put4(&mut k, [2, 4, 8, 10], flip(t), n / (30.0 * l));
    k
}

/// A local 12×12 matrix in global axes as (row, col, value) entries.
fn triplets(e: &Elem, k: &[[f64; 12]; 12], out: &mut Vec<(usize, usize, f64)>) {
    scatter(e, k, |i, j, v| out.push((i, j, v)));
}

/// A shape along a member from the movements of its pieces' ends: Hermite across each beam
/// piece in both planes, straight across a truss bar, in global x, y, z.
fn shape_along(pieces: &[Elem], u: &[f64]) -> Vec<[f64; 3]> {
    let np = pieces.len();
    (0..STATIONS)
        .map(|k| {
            let t = k as f64 / (STATIONS - 1) as f64 * np as f64;
            let i = (t.floor() as usize).min(np - 1);
            let (e, s) = (&pieces[i], t - i as f64);
            let ul = ends_of(e, u, false);
            let (ta, ra, tb, rb) = ([ul[0], ul[1], ul[2]], [ul[3], ul[4], ul[5]], [ul[6], ul[7], ul[8]], [ul[9], ul[10], ul[11]]);
            let lin = |a: f64, b: f64| a * (1.0 - s) + b * s;
            let (v, w) = if e.kind == MemberKind::Beam {
                let (h1, h2, h3, h4) = (1.0 - 3.0 * s * s + 2.0 * s.powi(3), e.l * (s - 2.0 * s * s + s.powi(3)), 3.0 * s * s - 2.0 * s.powi(3), e.l * (-s * s + s.powi(3)));
                (h1 * ta[1] + h2 * ra[2] + h3 * tb[1] + h4 * rb[2], h1 * ta[2] - h2 * ra[1] + h3 * tb[2] - h4 * rb[1])
            } else {
                (lin(ta[1], tb[1]), lin(ta[2], tb[2]))
            };
            to_global(e, [lin(ta[0], tb[0]), v, w])
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct MemberShape3 {
    pub id: u32,
    pub d: Vec<[f64; 3]>,
}
/// One natural mode of a 3D frame: its frequency, the six movements at every node and the shape
/// along each member, scaled so the largest movement is 1.
#[derive(Clone, Debug, Serialize)]
pub struct SpaceMode {
    pub f: f64,
    pub disp: Vec<[f64; 6]>,
    pub members: Vec<MemberShape3>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpaceModes {
    pub modes: Vec<SpaceMode>,
    pub equations: usize,
    pub iterations: usize,
    pub ms: f64,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpaceBuckle {
    pub lambda: f64,
    pub disp: Vec<[f64; 6]>,
    pub members: Vec<MemberShape3>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpaceBuckling {
    pub modes: Vec<SpaceBuckle>,
    pub bars: Vec<crate::frame::BarBuckle>,
    pub equations: usize,
    pub iterations: usize,
    pub ms: f64,
    pub warnings: Vec<String>,
}

/// Node movements and member shapes of one mode, for the window.
fn shapes(m: &Model, asm: &Assembly, u: &[f64]) -> (Vec<[f64; 6]>, Vec<MemberShape3>) {
    let disp = (0..m.nodes.len()).map(|i| std::array::from_fn(|k| if k >= 3 && (asm.free_rot[i] || !asm.has_beam[i]) { 0.0 } else { u[6 * i + k] })).collect();
    let members = asm.elems.iter().zip(&asm.spans).map(|(e, r)| MemberShape3 { id: e.id, d: shape_along(&asm.parts[r.clone()], u) }).collect();
    (disp, members)
}

/// The lowest natural frequencies of a 3D frame (beams cut into `frame::SPLIT` pieces).
pub fn modes(m: &Model, count: usize) -> Result<SpaceModes, String> {
    let t0 = std::time::Instant::now();
    let asm = assemble(m, crate::frame::SPLIT)?;
    let mut mass = vec![];
    for e in &asm.parts {
        triplets(e, &cond(e, m_local(e)), &mut mass);
    }
    let (found, iterations) = crate::modal::lowest(&asm.sys, &mass, &asm.held, count)?;
    let modes = found
        .into_iter()
        .map(|md| {
            let (disp, members) = shapes(m, &asm, &md.shape);
            SpaceMode { f: md.f, disp, members }
        })
        .collect();
    Ok(SpaceModes { modes, equations: asm.held.iter().filter(|h| !**h).count(), iterations, ms: t0.elapsed().as_secs_f64() * 1e3, warnings: vec![] })
}

/// Linear buckling of a 3D frame: the lowest load factors λ at which every load times λ buckles
/// it, with each squeezed truss bar also checked on its own about its weaker axis.
pub fn buckling(m: &Model, count: usize) -> Result<SpaceBuckling, String> {
    let t0 = std::time::Instant::now();
    let asm = assemble(m, crate::frame::SPLIT)?;
    if asm.sys.f.iter().all(|f| *f == 0.0) {
        return Err("There are no loads on the model, so nothing is squeezed and nothing can buckle. Add loads on the Loads tab.".into());
    }
    let (u, _) = asm.sys.solve(&asm.held, &vec![0.0; asm.sys.n])?;
    let axial = |e: &Elem| {
        let ul = ends_of(e, &u, true);
        let (k, eq) = (k_local(e), equivalent(e));
        let end = |i: usize| (0..12).map(|j| k[i][j] * ul[j]).sum::<f64>() - eq[i];
        (-end(0) + end(6)) / 2.0
    };
    let mut g = vec![];
    for e in &asm.parts {
        triplets(e, &cond(e, kg_local(e, axial(e))), &mut g);
    }
    let bars: Vec<crate::frame::BarBuckle> = asm
        .elems
        .iter()
        .filter(|e| e.kind == MemberKind::Truss)
        .map(|e| (e, axial(e)))
        .filter(|(e, n)| *n < 0.0 && e.eimin > 0.0)
        .map(|(e, n)| {
            let ncr = std::f64::consts::PI.powi(2) * e.eimin / (e.l * e.l);
            crate::frame::BarBuckle { id: e.id, n, ncr, lambda: ncr / -n }
        })
        .collect();
    let (found, iterations) = match crate::modal::buckling(&asm.sys, &g, &asm.held, count) {
        Err(x) if x.contains("squeezed") && !bars.is_empty() => (vec![], 0),
        r => r?,
    };
    let modes: Vec<SpaceBuckle> = found
        .into_iter()
        .map(|(lambda, u)| {
            let (disp, members) = shapes(m, &asm, &u);
            SpaceBuckle { lambda, disp, members }
        })
        .collect();
    let mut warnings = vec![];
    let first = modes.first().map(|b| b.lambda).unwrap_or(f64::INFINITY);
    if let Some(b) = bars.iter().filter(|b| b.lambda < first).min_by(|a, b| a.lambda.total_cmp(&b.lambda)) {
        let k = asm.elems.iter().position(|e| e.id == b.id).map_or(0, |k| k + 1);
        warnings.push(format!("Member {k}, a truss bar, buckles on its own at a load factor of {:.3}, before the frame as a whole{}.", b.lambda, if first.is_finite() { format!(" ({first:.3})") } else { String::new() }));
    }
    if first.min(bars.iter().map(|b| b.lambda).fold(f64::INFINITY, f64::min)) < 1.0 {
        warnings.push("A load factor is below 1: the model buckles before the loads reach the size given.".into());
    }
    if modes.is_empty() {
        warnings.push("The frame as a whole found no buckling mode under these loads.".into());
    }
    Ok(SpaceBuckling { modes, bars, equations: asm.held.iter().filter(|h| !**h).count(), iterations, ms: t0.elapsed().as_secs_f64() * 1e3, warnings })
}
