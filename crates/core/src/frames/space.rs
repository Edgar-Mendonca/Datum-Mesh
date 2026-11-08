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
use super::plates;
use super::shell::Tri;

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
    /// The node held (0 for a plate's edge), and the support.
    pub node: u32,
    #[serde(default)]
    pub support: u32,
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
    /// Plates (0.4.1): their mesh, movements and results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plates: Option<PlateResults>,
    /// A dynamic solution (0.4.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<crate::engine::dynamics::DynInfo>,
    /// Tension-only or compression-only members slack under these loads (0.4.3).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slack: Vec<u32>,
    /// Large rotations (0.5.8): the load against the movement of the node followed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<super::corot::Curve>,
    /// Frames to play (0.6.4), as for plane frames.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub frames: Option<crate::engine::frames::Frames>,
}

/// Plate results (0.4.1): the mesh in 3D, the movements of its nodes and fields averaged at them:
/// movements (ux, uy, uz, umag), the largest von Mises stress of the two faces (vm), moments per
/// length (mx, my, mxy) and membrane forces per length (nx, ny, nxy) in each plate's u, v axes.
#[derive(Clone, Debug, Serialize)]
pub struct PlateResults {
    pub nodes: Vec<[f64; 3]>,
    pub tris: Vec<[u32; 3]>,
    pub disp: Vec<[f64; 3]>,
    pub fields: Vec<crate::solid::Field3>,
}

/// A plate triangle in the assembly.
#[derive(Clone)]
struct Shell {
    tri: super::shell::Tri,
    n: [usize; 3],
    /// Mass per area ρt.
    rho: f64,
    axes: [[f64; 3]; 2],
}

#[derive(Clone)]
pub(super) struct Elem {
    pub(super) id: u32,
    pub(super) kind: MemberKind,
    pub(super) a: usize,
    pub(super) b: usize,
    pub(super) l: f64,
    /// Rows: local x, y, z in global terms.
    pub(super) r: [[f64; 3]; 3],
    pub(super) ea: f64,
    pub(super) eiz: f64,
    pub(super) eiy: f64,
    pub(super) gj: f64,
    area: f64,
    zz: f64,
    zy: f64,
    fy: f64,
    /// Distributed load in local axes, linear from a to b: (px1, px2, qy1, qy2, qz1, qz2) N/m.
    pub(super) p: [f64; 6],
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
    /// Second order (0.4.3): the axial force (tension positive) whose geometric stiffness is
    /// in the member's stiffness; 0 for a first-order solve.
    pub(super) pn: f64,
    /// Rigid zones at the ends (m, 0.4.5), and whether this piece is one.
    off: [f64; 2],
    rigid: bool,
    /// Loads over part of the member (0.10.3): from and to as fractions of the length, and the
    /// local per-length loads at each, as `p`. `split` cuts there.
    segs: Vec<(f64, f64, [f64; 6])>,
    /// Point forces on the member (0.10.3): the fraction of the length and the local force.
    pts: Vec<(f64, [f64; 3])>,
    /// Point forces at the piece's ends after `split`, local axes: a's (x, y, z), then b's.
    tip: [f64; 6],
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
pub(super) fn cond(e: &Elem, k: [[f64; 12]; 12]) -> [[f64; 12]; 12] {
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
pub(super) fn k_local(e: &Elem) -> [[f64; 12]; 12] {
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
    if e.pn != 0.0 {
        let g = kg_local(e, e.pn);
        for i in 0..12 {
            for j in 0..12 {
                k[i][j] += g[i][j];
            }
        }
    }
    k
}

/// Second-order (P-delta) solve of a 3D frame (0.4.3): beams cut into `pieces`, each piece's
/// stiffness with the geometric stiffness of its axial force, the axial forces found again from
/// the movements and the solve repeated until they settle (the P-Δ of the frame's sway and the
/// P-δ of each member's bow). Linear in the material, small rotations.
pub fn pdelta(m: &Model, pieces: usize) -> Result<SpaceResults, String> {
    let t0 = std::time::Instant::now();
    let mut asm = assemble(m, pieces.max(1))?;
    if !asm.shells.is_empty() {
        asm.warnings.push("Plates are solved first-order here: their membrane forces add no geometric stiffness in this release.".into());
    }
    let base = asm.sys.k.clone();
    let mut u = asm.sys.solve(&asm.held, &asm.fixed)?.0;
    let mut its = 0;
    let mut settled = false;
    for it in 1..=60 {
        its = it;
        // Axial forces from the movements (linear stiffness), then the stiffness with them.
        let ns: Vec<f64> = asm
            .parts
            .iter()
            .map(|e| {
                let e0 = Elem { pn: 0.0, ..e.clone() };
                let ul = ends_of(&e0, &u, true);
                let (k, eq) = (k_local(&e0), equivalent(&e0));
                let end = |i: usize| (0..12).map(|j| k[i][j] * ul[j]).sum::<f64>() - eq[i];
                (-end(0) + end(6)) / 2.0
            })
            .collect();
        let mut sys = System::new(asm.sys.n);
        sys.k = base.clone();
        sys.f = asm.sys.f.clone();
        for (e, n) in asm.parts.iter_mut().zip(&ns) {
            e.pn = *n;
            let e0 = Elem { pn: 0.0, ..e.clone() };
            let (a, b) = (cond(e, k_local(e)), cond(&e0, k_local(&e0)));
            let mut d = [[0.0; 12]; 12];
            for i in 0..12 {
                for j in 0..12 {
                    d[i][j] = a[i][j] - b[i][j];
                }
            }
            scatter(e, &d, |i, j, v| sys.add(i, j, v));
        }
        let un = sys.solve(&asm.held, &asm.fixed).map_err(|_| "The frame cannot carry these loads in its deflected shape: they are at or beyond its buckling load (Solve › Buckling).".to_string())?.0;
        let du = un.iter().zip(&u).map(|(a, b)| (a - b) * (a - b)).sum::<f64>().sqrt();
        let un2 = un.iter().map(|a| a * a).sum::<f64>().sqrt();
        u = un;
        asm.sys.k = sys.k;
        if du <= 1e-10 * un2.max(f64::MIN_POSITIVE) {
            settled = true;
            break;
        }
        asm.sys.k = base.clone();
    }
    if !settled {
        asm.warnings.push(format!("The second-order solve did not settle in {its} rounds: the loads are close to the frame's buckling load."));
    }
    let mut r = results_at(m, &asm, &u, &asm.sys.f, 1.0, true, None);
    r.stats.ms = t0.elapsed().as_secs_f64() * 1e3;
    Ok(r)
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
pub(super) fn scatter(e: &Elem, k: &[[f64; 12]; 12], mut out: impl FnMut(usize, usize, f64)) {
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
pub(crate) struct Assembly {
    pub(crate) sys: System,
    pub(crate) held: Vec<bool>,
    elems: Vec<Elem>,
    pub(super) parts: Vec<Elem>,
    spans: Vec<std::ops::Range<usize>>,
    free_rot: Vec<bool>,
    has_member: Vec<bool>,
    has_beam: Vec<bool>,
    pub(super) warnings: Vec<String>,
    /// Plates: their triangles, the assembly node of each plate mesh node, and the nodes held
    /// by each support on a plate's edge.
    shells: Vec<Shell>,
    plate_nodes: Vec<usize>,
    edge_sup: Vec<(u32, Vec<usize>)>,
    pub(super) pos: Vec<[f64; 3]>,
    /// Prescribed movements at held unknowns (support settlements, 0.4.3).
    fixed: Vec<f64>,
}

/// Cut a member at plate nodes along it (`at`: the fraction along it and the node).
fn cut(e: &Elem, at: &[(f64, usize)]) -> Vec<Elem> {
    let ts: Vec<f64> = std::iter::once(0.0).chain(at.iter().map(|x| x.0)).chain(std::iter::once(1.0)).collect();
    let ns: Vec<usize> = std::iter::once(e.a).chain(at.iter().map(|x| x.1)).chain(std::iter::once(e.b)).collect();
    let last = ts.len() - 2;
    (0..=last)
        .map(|k| {
            let (s0, s1) = (ts[k], ts[k + 1]);
            let mut c = e.clone();
            c.a = ns[k];
            c.b = ns[k + 1];
            c.l = e.l * (s1 - s0);
            c.hinge = [k == 0 && e.hinge[0], k == last && e.hinge[1]];
            for j in 0..3 {
                let (q1, q2) = (e.p[2 * j], e.p[2 * j + 1]);
                c.p[2 * j] = q1 + (q2 - q1) * s0;
                c.p[2 * j + 1] = q1 + (q2 - q1) * s1;
            }
            c
        })
        .collect()
}

/// Cut a beam into `pieces` equal elements, and also where a part-length or point load starts,
/// ends or acts; inner nodes numbered from `first`. Its distributed load is shared out along the
/// pieces and a point load goes to the node at its place. A truss bar is not cut: its part-length
/// and point loads go to its two ends as on a simple span.
fn split(e: &Elem, pieces: usize, first: usize) -> Vec<Elem> {
    const EPS: f64 = 1e-9;
    if e.kind != MemberKind::Beam {
        let mut c = Elem { off: [0.0; 2], segs: vec![], pts: vec![], ..e.clone() };
        let mut to_ends = |t: f64, f: [f64; 3]| {
            for k in 0..3 {
                c.tip[k] += f[k] * (1.0 - t);
                c.tip[3 + k] += f[k] * t;
            }
        };
        for &(s0, s1, w) in &e.segs {
            let d = (s1 - s0) * e.l;
            to_ends((s0 + s1) / 2.0, [w[0] * d, w[2] * d, w[4] * d]);
            to_ends(s0 + 2.0 * (s1 - s0) / 3.0, [(w[1] - w[0]) * d / 2.0, (w[3] - w[2]) * d / 2.0, (w[5] - w[4]) * d / 2.0]);
        }
        for &(t, f) in &e.pts {
            to_ends(t, f);
        }
        return vec![c];
    }
    // The cuts, as fractions of the length: the rigid zones' ends (0.4.5), equal pieces of the
    // flexible part between them, and the loads' places.
    let (oa, ob) = (e.off[0] / e.l, e.off[1] / e.l);
    let n = pieces.max(1);
    let mut cuts = vec![0.0, 1.0, oa, 1.0 - ob];
    for k in 1..n {
        cuts.push(oa + (1.0 - oa - ob) * k as f64 / n as f64);
    }
    for &(s0, s1, _) in &e.segs {
        cuts.extend([s0, s1]);
    }
    cuts.extend(e.pts.iter().map(|p| p.0));
    cuts.retain(|t| (0.0..=1.0).contains(t));
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < EPS);
    if cuts.len() == 2 {
        return vec![Elem { off: [0.0; 2], segs: vec![], pts: vec![], ..e.clone() }];
    }
    let at_cut = |t: f64| cuts.iter().position(|c| (c - t).abs() < EPS).unwrap_or(0);
    let mut tips = vec![[0.0; 3]; cuts.len()];
    for &(t, f) in &e.pts {
        let k = at_cut(t);
        for j in 0..3 {
            tips[k][j] += f[j];
        }
    }
    let p = e.p;
    let last = cuts.len() - 2;
    let (fa, fb) = (at_cut(oa), at_cut(1.0 - ob) - 1);
    (0..=last)
        .map(|k| {
            let (s0, s1) = (cuts[k], cuts[k + 1]);
            let mut c = Elem { segs: vec![], pts: vec![], tip: [0.0; 6], ..e.clone() };
            c.a = if k == 0 { e.a } else { first + k - 1 };
            c.b = if k == last { e.b } else { first + k };
            c.l = e.l * (s1 - s0);
            c.off = [0.0; 2];
            c.rigid = s1 <= oa + EPS || s0 >= 1.0 - ob - EPS;
            if c.rigid {
                c.ea *= 1e5;
                c.eiz *= 1e5;
                c.eiy *= 1e5;
                c.gj *= 1e5;
                c.at = 0.0;
            }
            c.hinge = [k == fa && e.hinge[0], k == fb && e.hinge[1]];
            for j in 0..3 {
                let (q1, q2) = (p[2 * j], p[2 * j + 1]);
                c.p[2 * j] = q1 + (q2 - q1) * s0;
                c.p[2 * j + 1] = q1 + (q2 - q1) * s1;
            }
            for &(g0, g1, w) in &e.segs {
                if s0 >= g0 - EPS && s1 <= g1 + EPS {
                    for j in 0..3 {
                        let at = |t: f64| w[2 * j] + (w[2 * j + 1] - w[2 * j]) * (t - g0) / (g1 - g0);
                        c.p[2 * j] += at(s0);
                        c.p[2 * j + 1] += at(s1);
                    }
                }
            }
            if k == 0 {
                c.tip[..3].copy_from_slice(&tips[0]);
            }
            c.tip[3..].copy_from_slice(&tips[k + 1]);
            c
        })
        .collect()
}

impl Assembly {
    /// The frame carries plates.
    pub(super) fn plated(&self) -> bool {
        !self.shells.is_empty()
    }
}

pub(super) fn assemble(m: &Model, pieces: usize) -> Result<Assembly, String> {
    m.check()?;
    let pm = if m.shapes.iter().any(|s| s.plate.is_some()) { plates::mesh(m)? } else { plates::PlateMesh::default() };
    if m.members.is_empty() && pm.tris.is_empty() {
        return Err("There are no members or plates to solve. Draw beams, truss bars or plates first (Geometry).".into());
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
        let mut e = Elem { id: mb.id, kind: mb.kind, a, b, l: dist3(pa, pb), r: axes(pa, pb, mb.roll), ea: mat.e * pr.a, eiz: mat.e * pr.i, eiy: mat.e * pr.iy, gj: gm * pr.j, area: pr.a, zz: pr.z(), zy: pr.zy(), fy: mat.fy, p: [0.0; 6], at: mat.alpha * warming, rho_a: mat.rho * pr.a, rho_ip: mat.rho * (pr.i + pr.iy), eimin: mat.e * pr.i.min(pr.iy), hinge: mb.hinge, twist: [false; 2], pn: 0.0, off: mb.offset, rigid: false, segs: vec![], pts: vec![], tip: [0.0; 6] };
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
    // Part-length and point loads (0.10.3), resolved into the member's axes.
    for l in &m.loads {
        let (Target::Member { id }, LoadKind::Patch { dir, .. } | LoadKind::Concentrated { dir, .. }) = (&l.at, &l.kind) else { continue };
        let e = elems.iter_mut().find(|e| e.id == *id).ok_or("A load is on a member that does not exist.")?;
        let gdir = match dir {
            Dir::X => [1.0, 0.0, 0.0],
            Dir::Y => [0.0, 1.0, 0.0],
            Dir::Z => [0.0, 0.0, 1.0],
            Dir::Local => e.r[1],
        };
        let ax = to_local(e, gdir);
        match l.kind {
            LoadKind::Patch { w1, w2, from, to, projected, dir } => {
                // Per length of the projection across the load: the member's length times the
                // sine of the angle between the member and the load.
                let k = if projected && dir != Dir::Local { (1.0 - ax[0] * ax[0]).max(0.0).sqrt() } else { 1.0 };
                let (s0, s1) = ((from / e.l).clamp(0.0, 1.0), (to / e.l).clamp(0.0, 1.0));
                if s1 > s0 {
                    e.segs.push((s0, s1, [k * w1 * ax[0], k * w2 * ax[0], k * w1 * ax[1], k * w2 * ax[1], k * w1 * ax[2], k * w2 * ax[2]]));
                }
            }
            LoadKind::Concentrated { f, at, .. } => e.pts.push(((at / e.l).clamp(0.0, 1.0), [f * ax[0], f * ax[1], f * ax[2]])),
            _ => {}
        }
    }
    let mut parts = vec![];
    let mut spans = vec![];
    let mut pos: Vec<[f64; 3]> = m.nodes.iter().map(|n| n.at()).collect();
    // Plate nodes: the frame's own where they meet, new ones otherwise.
    let plate_nodes: Vec<usize> = pm
        .nodes
        .iter()
        .map(|p| {
            pos.iter().position(|q| dist3(*q, *p) < 1e-6).unwrap_or_else(|| {
                pos.push(*p);
                pos.len() - 1
            })
        })
        .collect();
    for e in &elems {
        // A member a plate runs along is cut at the plate's nodes on it, and joined there.
        let (a, b) = (pos[e.a], pos[e.b]);
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ll = ab.iter().map(|x| x * x).sum::<f64>();
        let mut on: Vec<(f64, usize)> = plate_nodes
            .iter()
            .filter(|&&k| k != e.a && k != e.b)
            .filter_map(|&k| {
                let p = pos[k];
                let t = ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / ll;
                let q = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
                (t > 1e-9 && t < 1.0 - 1e-9 && dist3(p, q) < 1e-6).then_some((t, k))
            })
            .collect();
        on.sort_by(|x, y| x.0.total_cmp(&y.0));
        on.dedup_by_key(|x| x.1);
        if !on.is_empty() {
            if !e.segs.is_empty() || !e.pts.is_empty() {
                warnings.push("A part-length or point load on a member that a plate runs along was left out: load the whole member, or the plate.".into());
            }
            let pcs = cut(e, &on);
            spans.push(parts.len()..parts.len() + pcs.len());
            parts.extend(pcs);
            continue;
        }
        if e.kind == MemberKind::Beam && !(e.off[0] >= 0.0 && e.off[1] >= 0.0 && e.off[0] + e.off[1] < e.l) {
            return Err(format!("Member {}'s rigid ends are as long as the member: make them shorter.", m.members.iter().position(|b| b.id == e.id).map_or(0, |k| k + 1)));
        }
        let pcs = split(e, pieces, pos.len());
        let (a, b) = (pos[e.a], pos[e.b]);
        let mut along = 0.0;
        for pc in &pcs[..pcs.len() - 1] {
            along += pc.l;
            let t = along / e.l;
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
    for &k in &plate_nodes {
        has_member[k] = true;
        has_beam[k] = true;
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
        let (ta, tb) = (to_global(e, [e.tip[0], e.tip[1], e.tip[2]]), to_global(e, [e.tip[3], e.tip[4], e.tip[5]]));
        add_load(&mut sys, e.a, [fa[0] + ta[0], fa[1] + ta[1], fa[2] + ta[2], ma[0], ma[1], ma[2]]);
        add_load(&mut sys, e.b, [fb[0] + tb[0], fb[1] + tb[1], fb[2] + tb[2], mb[0], mb[1], mb[2]]);
    }
    // Plates: stiffness, their own weight and pressure (lumped: a third of each triangle's load
    // on each corner).
    let mut shells = vec![];
    for (k, c) in pm.tris.iter().enumerate() {
        let mat = m.material(pm.mat[k]).ok_or("A plate's material is missing.")?;
        let n = [plate_nodes[c[0]], plate_nodes[c[1]], plate_nodes[c[2]]];
        let tri = Tri { p: [pos[n[0]], pos[n[1]], pos[n[2]]], t: pm.t[k], e: mat.e, nu: mat.nu };
        let kg = tri.k_global();
        for i in 0..18 {
            for j in 0..18 {
                sys.add(6 * n[i / 6] + i % 6, 6 * n[j / 6] + j % 6, kg[i][j]);
            }
        }
        let (_, _, area) = tri.frame();
        let mut w = [mat.rho * pm.t[k] * area / 3.0 * g[0], mat.rho * pm.t[k] * area / 3.0 * g[1], mat.rho * pm.t[k] * area / 3.0 * g[2]];
        for l in &m.loads {
            if let (LoadKind::Pressure { p }, Target::Plate { shape }) = (&l.kind, &l.at) {
                if *shape == pm.shape[k] {
                    let across = m.shape(*shape).and_then(|s| s.plate).map_or([0.0; 3], |p| p.across());
                    for d in 0..3 {
                        w[d] -= p * area / 3.0 * across[d];
                    }
                }
            }
        }
        for &node in &n {
            add_load(&mut sys, node, [w[0], w[1], w[2], 0.0, 0.0, 0.0]);
        }
        shells.push(Shell { tri, n, rho: mat.rho * pm.t[k], axes: pm.axes[k] });
    }
    if !shells.is_empty() && warming != 0.0 {
        warnings.push("A temperature change acts on the members only; plates are left out of it in this release.".into());
    }
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Pressure { .. }, Target::Plate { .. }) => {}
            (LoadKind::Force { fx, fy, fz }, Target::Node { id }) => add_load(&mut sys, idx[id], [*fx, *fy, *fz, 0.0, 0.0, 0.0]),
            (LoadKind::Moment { m: mz, mx, my }, Target::Node { id }) => add_load(&mut sys, idx[id], [0.0, 0.0, 0.0, *mx, *my, *mz]),
            (LoadKind::Dist { .. } | LoadKind::Patch { .. } | LoadKind::Concentrated { .. } | LoadKind::Gravity { .. } | LoadKind::Warming { .. } | LoadKind::Mass { .. } | LoadKind::Settlement { .. }, _) => {}
            (k, _) => warnings.push(format!("A {} load does not act on frames and was left out.", load_name(k))),
        }
    }
    let mut held = vec![false; 6 * nt];
    let mut edge_sup = vec![];
    for s in &m.supports {
        if let Target::Edge { shape, edge } = s.at {
            let mut nodes: Vec<usize> = pm.edges.iter().filter(|e| e.0 == shape && e.1 == edge).flat_map(|e| [plate_nodes[e.2], plate_nodes[e.3]]).collect();
            nodes.sort_unstable();
            nodes.dedup();
            let dofs: &[usize] = match s.kind {
                SupportKind::Fixed => &[0, 1, 2, 3, 4, 5],
                SupportKind::Pinned => &[0, 1, 2],
                _ => {
                    warnings.push("A plate's edge takes a fixed or a pinned support; another kind was left out.".into());
                    continue;
                }
            };
            for &k in &nodes {
                for &d in dofs {
                    held[6 * k + d] = true;
                }
            }
            edge_sup.push((s.id, nodes));
            continue;
        }
        let Target::Node { id } = s.at else {
            warnings.push("A support that is not on a node or a plate's edge was left out.".into());
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
    let _ = applied;
    // Rigid floors (0.4.4): at each height above the lowest, the nodes move together in the
    // floor's plane (x, z and the turn about y), tied to the first node there by stiff links (a
    // penalty, 10⁷ times the stiffest unknown: the floor flexes about a ten-millionth as much).
    if m.rigid_floors {
        let big = sys.k.iter().filter(|e| e.0 == e.1).map(|e| e.2.abs()).fold(0.0, f64::max).max(1.0);
        let kp = 1e7 * big;
        let lowest = m.nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
        let mut levels: Vec<(f64, Vec<usize>)> = vec![];
        for (i, n) in m.nodes.iter().enumerate() {
            if !has_member[i] || (n.y - lowest).abs() < 1e-6 {
                continue;
            }
            match levels.iter_mut().find(|l| (l.0 - n.y).abs() < 1e-6) {
                Some(l) => l.1.push(i),
                None => levels.push((n.y, vec![i])),
            }
        }
        for (_, nodes) in levels.iter().filter(|l| l.1.len() > 1) {
            let mi = nodes[0];
            let pm = m.nodes[mi].at();
            for &i in &nodes[1..] {
                let p = m.nodes[i].at();
                let (dx, dz) = (p[0] - pm[0], p[2] - pm[2]);
                let ties: [Vec<(usize, f64)>; 3] = [
                    vec![(6 * i, 1.0), (6 * mi, -1.0), (6 * mi + 4, -dz)],
                    vec![(6 * i + 2, 1.0), (6 * mi + 2, -1.0), (6 * mi + 4, dx)],
                    vec![(6 * i + 4, 1.0), (6 * mi + 4, -1.0)],
                ];
                for c in &ties {
                    for &(a, ca) in c {
                        for &(b, cb) in c {
                            sys.add(a, b, kp * ca * cb);
                        }
                    }
                }
            }
        }
    }
    // Support settlements (0.4.3): the held directions of a node move by the amount given.
    let mut fixed = vec![0.0; 6 * nt];
    for l in &m.loads {
        if let (LoadKind::Settlement { dx, dy, dz }, Target::Node { id }) = (&l.kind, &l.at) {
            let i = idx[id];
            for (k, v) in [(0, *dx), (1, *dy), (2, *dz)] {
                if v != 0.0 {
                    if held[6 * i + k] && has_member[i] {
                        fixed[6 * i + k] += v;
                    } else {
                        warnings.push(format!("A settlement at node {} moves it along {} where no support holds it; that part was left out.", i + 1, ["x", "y", "z"][k]));
                    }
                }
            }
        }
    }
    Ok(Assembly { sys, held, elems, parts, spans, free_rot, has_member, has_beam, warnings, shells, plate_nodes, edge_sup, pos, fixed })
}

/// Solve a 3D frame or truss model, with its plates (0.4.1).
pub fn solve(m: &Model) -> Result<SpaceResults, String> {
    let asm = assemble(m, 1)?;
    let (u, stats) = asm.sys.solve(&asm.held, &asm.fixed).map_err(|e| {
        if asm.elems.iter().any(|e| e.kind == MemberKind::Beam) {
            format!("{e} In 3D, a beam held only by pins at its ends can spin about its own axis: fix one end, or hold its turning with a spring.")
        } else {
            e
        }
    })?;
    let mut r = results_from(m, &asm, &u, &asm.sys.f, 1.0);
    r.stats = stats;
    if r.scale[0] == 0.0 && r.scale[1] == 0.0 {
        if asm.fixed.iter().any(|v| *v != 0.0) {
            // With only settlements the reactions balance each other: judge them by their size.
            let s: f64 = r.reactions.iter().map(|x| x.f[..3].iter().map(|v| v.abs()).sum::<f64>()).sum();
            let reach = asm.pos.iter().map(|q| dist3(*q, [0.0; 3])).fold(0.0, f64::max) + span(m);
            r.scale = [s, s * reach + r.reactions.iter().map(|x| x.f[3..].iter().map(|v| v.abs()).sum::<f64>()).sum::<f64>()];
            let fb = (0..3).map(|k| (r.applied[k] + r.reacted[k]).abs()).fold(0.0, f64::max) / s.max(f64::MIN_POSITIVE);
            let mo = (3..6).map(|k| (r.applied[k] + r.reacted[k]).abs()).fold(0.0, f64::max) / r.scale[1].max(f64::MIN_POSITIVE);
            r.imbalance = fb.max(mo);
        } else {
            r.warnings.push("There are no loads on the model, so nothing moves. Add loads on the Loads tab.".into());
        }
    }
    let umax = r.disp.iter().map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()).chain(r.plates.iter().flat_map(|p| p.disp.iter().map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()))).fold(0.0, f64::max);
    // An answer that does not balance and moves further than the model is big is no answer:
    // rounding hid a mechanism from the solver (0.10.3).
    if r.imbalance > 1e-6 && umax > span(m) {
        return Err(format!("{} In 3D, a beam held only by pins at its ends can spin about its own axis: fix one end, or hold its turning with a spring.", super::frame::MECHANISM));
    }
    if umax > 0.1 * span(m) {
        r.warnings.push(format!("The largest movement is {:.0} % of the model's size: far beyond small deflections, so this linear answer is not reliable. Check the supports, sections and units.", umax / span(m) * 100.0));
    }
    Ok(r)
}

/// Results for movements `u` under nodal loads `f`, the members' own loads times `p`.
fn results_from(m: &Model, asm: &Assembly, u: &[f64], f: &[f64], p: f64) -> SpaceResults {
    results_at(m, asm, u, f, p, false, None)
}

/// As `results_from`; `moved`: moments of loads and reactions about where the nodes are now
/// (second order: the frame balances in its deflected shape).
/// `corot` (0.5.8): after large rotations, the internal forces and each piece's end movements in
/// its own turned axes, in place of the linear K u and the movements read in the original axes.
pub(super) fn results_at(m: &Model, asm: &Assembly, u: &[f64], f: &[f64], p: f64, moved: bool, corot: Option<(&[f64], &[[f64; 12]])>) -> SpaceResults {
    let Assembly { sys, held, elems, parts, spans, free_rot, has_member, has_beam, warnings, shells, plate_nodes, edge_sup, pos, .. } = asm;
    let mut warnings = warnings.clone();
    let nn = m.nodes.len();
    let n_all = pos.len();
    let reach = pos.iter().map(|q| dist3(*q, [0.0; 3])).fold(0.0, f64::max) + span(m);
    let fscale: f64 = (0..n_all).map(|i| (0..3).map(|k| f[6 * i + k].abs()).sum::<f64>()).sum();
    let scale = [fscale, fscale * reach + (0..n_all).map(|i| (3..6).map(|k| f[6 * i + k].abs()).sum::<f64>()).sum::<f64>()];
    let arm = |i: usize| if moved { [pos[i][0] + u[6 * i], pos[i][1] + u[6 * i + 1], pos[i][2] + u[6 * i + 2]] } else { pos[i] };
    let mut applied = [0.0; 6];
    for i in 0..n_all {
        let mo = cross(arm(i), [f[6 * i], f[6 * i + 1], f[6 * i + 2]]);
        for k in 0..3 {
            applied[k] += f[6 * i + k];
            applied[3 + k] += f[6 * i + 3 + k] + mo[k];
        }
    }
    let ku = match corot {
        Some((fi, _)) => fi.to_vec(),
        None => sys.mul(u),
    };
    let mut reactions = vec![];
    let mut reacted = [0.0; 6];
    let react = |r: [f64; 6], at: [f64; 3], reacted: &mut [f64; 6]| {
        let mo = cross(at, [r[0], r[1], r[2]]);
        for k in 0..3 {
            reacted[k] += r[k];
            reacted[3 + k] += r[3 + k] + mo[k];
        }
    };
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
                    r[k] = ku[6 * i + k] - f[6 * i + k];
                }
            }
        }
        react(r, arm(i), &mut reacted);
        reactions.push(SpaceReaction { node: n.id, support: sp.id, f: r });
    }
    // A plate's edge support: the forces of every node on it, and their moments about the
    // middle of the edge. A node held by a support of its own is counted there, not here.
    let own: std::collections::HashSet<usize> = m.supports.iter().filter_map(|s| if let Target::Node { id } = s.at { m.nodes.iter().position(|n| n.id == id) } else { None }).collect();
    let mut counted = own.clone();
    for (sid, nodes) in edge_sup {
        let nodes: Vec<usize> = nodes.iter().copied().filter(|k| counted.insert(*k)).collect();
        if nodes.is_empty() {
            continue;
        }
        let c = nodes.iter().fold([0.0; 3], |a, &k| [a[0] + pos[k][0], a[1] + pos[k][1], a[2] + pos[k][2]]).map(|x| x / nodes.len() as f64);
        let mut tot = [0.0; 6];
        for &k in &nodes {
            let mut r = [0.0; 6];
            for d in 0..6 {
                if held[6 * k + d] {
                    r[d] = ku[6 * k + d] - f[6 * k + d];
                }
            }
            react(r, arm(k), &mut reacted);
            let mo = cross(sub3(pos[k], c), [r[0], r[1], r[2]]);
            for d in 0..3 {
                tot[d] += r[d];
                tot[3 + d] += r[3 + d] + mo[d];
            }
        }
        reactions.push(SpaceReaction { node: 0, support: *sid, f: tot });
    }
    let disp: Vec<[f64; 6]> = (0..nn).map(|i| std::array::from_fn(|k| if k >= 3 && (free_rot[i] || !has_beam[i]) { 0.0 } else { u[6 * i + k] })).collect();
    let members = elems
        .iter()
        .zip(spans)
        .map(|(e, sp)| {
            let pcs: Vec<Elem> = parts[sp.clone()].iter().map(|x| Elem { p: x.p.map(|v| v * p), at: x.at * p, ..x.clone() }).collect();
            let uls: Vec<[f64; 12]> = match corot {
                Some((_, ls)) => ls[sp.clone()].to_vec(),
                None => pcs.iter().map(|x| ends_of(x, u, true)).collect(),
            };
            let mut mr = if pcs.len() == 1 { member_result(&pcs[0], uls[0]) } else { stitched(e, &pcs, &uls) };
            // Large rotations (0.5.8): the local movements are in each piece's turned frame, so
            // the stations' movements come from the pieces' ends in global terms (straight
            // between them; the pieces are short).
            if corot.is_some() {
                for (x, d) in mr.x.iter().zip(mr.d.iter_mut()) {
                    let (mut k, mut at) = (0, 0.0);
                    while k + 1 < pcs.len() && x - at > pcs[k].l * (1.0 + 1e-9) {
                        at += pcs[k].l;
                        k += 1;
                    }
                    let t = ((x - at) / pcs[k].l).clamp(0.0, 1.0);
                    *d = std::array::from_fn(|c| (1.0 - t) * u[6 * pcs[k].a + c] + t * u[6 * pcs[k].b + c]);
                }
            }
            mr
        })
        .collect();
    let plates = (!shells.is_empty()).then(|| plate_results(shells, plate_nodes, pos, u));
    let fb = if scale[0] > 0.0 { (0..3).map(|k| (applied[k] + reacted[k]).abs()).fold(0.0, f64::max) / scale[0] } else { 0.0 };
    let mo = if scale[1] > 0.0 { (3..6).map(|k| (applied[k] + reacted[k]).abs()).fold(0.0, f64::max) / scale[1] } else { 0.0 };
    warnings.dedup();
    SpaceResults { disp, reactions, members, applied, reacted, scale, imbalance: fb.max(mo), stats: Stats { equations: held.iter().filter(|h| !**h).count(), ..Default::default() }, warnings, plates, dynamic: None, slack: vec![], path: None, frames: None }
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A member cut into pieces (where a plate joins it): each piece's results end to end.
fn stitched(e: &Elem, pieces: &[Elem], uls: &[[f64; 12]]) -> SpaceMember {
    let mut out = SpaceMember { id: e.id, length: e.l, x: vec![], n: vec![], vy: vec![], vz: vec![], t: vec![], my: vec![], mz: vec![], d: vec![], stress: 0.0, util: 0.0, axes: e.r };
    let mut at = 0.0;
    // Stations per piece so the member has about as many as an uncut one.
    for (k, p) in pieces.iter().enumerate() {
        let r = member_result(p, uls[k]);
        // Every piece keeps its ends and its middle: the step divides half the stations.
        let half = (STATIONS - 1) / 2;
        let step = (1..=half).rev().find(|s| half.is_multiple_of(*s) && *s <= (STATIONS - 1) / pieces.len()).unwrap_or(1);
        let idx: Vec<usize> = (0..STATIONS).step_by(step).chain(std::iter::once(STATIONS - 1)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        // A piece's first station repeats the last one's place; it is kept where a point load acts
        // (0.10.3), so the diagram drops straight down there.
        // Decided by the model, not the values, so every increment has the same stations.
        let jumps = k > 0 && pieces[k - 1].tip[3..].iter().any(|t| *t != 0.0);
        for &i in idx.iter().skip(usize::from(k > 0 && !jumps)) {
            out.x.push(at + r.x[i]);
            out.n.push(r.n[i]);
            out.vy.push(r.vy[i]);
            out.vz.push(r.vz[i]);
            out.t.push(r.t[i]);
            out.my.push(r.my[i]);
            out.mz.push(r.mz[i]);
            out.d.push(r.d[i]);
        }
        out.stress = out.stress.max(r.stress);
        out.util = out.util.max(r.util);
        at += p.l;
    }
    out
}

/// Plate results: movements at the plate nodes and fields averaged from the triangles round each.
fn plate_results(shells: &[Shell], plate_nodes: &[usize], pos: &[[f64; 3]], u: &[f64]) -> PlateResults {
    let np = plate_nodes.len();
    let back: std::collections::HashMap<usize, usize> = plate_nodes.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    // Averaged at the nodes from the triangles round each (weighted by area): moments, membrane
    // forces, and the stresses of the two faces, whose von Mises is taken after averaging.
    let keys = ["mx", "my", "mxy", "nx", "ny", "nxy"];
    let mut acc = vec![[0.0; 12]; np];
    let mut wsum = vec![0.0; np];
    let mut tris = vec![];
    for sh in shells {
        let ug: [f64; 18] = std::array::from_fn(|q| u[6 * sh.n[q / 6] + q % 6]);
        let (n, mm) = sh.tri.forces(&ug);
        // Into the plate's u, v axes.
        let (r, _, area) = sh.tri.frame();
        let a1 = [super::shell::dot(sh.axes[0], r[0]), super::shell::dot(sh.axes[0], r[1])];
        let a2 = [super::shell::dot(sh.axes[1], r[0]), super::shell::dot(sh.axes[1], r[1])];
        let turn = |t: [f64; 3]| {
            let q = |a: [f64; 2], b: [f64; 2]| a[0] * b[0] * t[0] + a[1] * b[1] * t[1] + (a[0] * b[1] + a[1] * b[0]) * t[2];
            [q(a1, a1), q(a2, a2), q(a1, a2)]
        };
        let (n, mm) = (turn(n), turn(mm));
        let t = sh.tri.t;
        let face = |sg: f64| std::array::from_fn::<f64, 3, _>(|i| n[i] / t + sg * 6.0 * mm[i] / (t * t));
        let (top, bot) = (face(1.0), face(-1.0));
        let v = [mm[0], mm[1], mm[2], n[0], n[1], n[2], top[0], top[1], top[2], bot[0], bot[1], bot[2]];
        let mut c = [0u32; 3];
        for (j, &k) in sh.n.iter().enumerate() {
            let i = back[&k];
            c[j] = i as u32;
            for q in 0..12 {
                acc[i][q] += area * v[q];
            }
            wsum[i] += area;
        }
        tris.push(c);
    }
    let avg = |i: usize, q: usize| if wsum[i] > 0.0 { acc[i][q] / wsum[i] } else { 0.0 };
    let vm = |s: [f64; 3]| (s[0] * s[0] - s[0] * s[1] + s[1] * s[1] + 3.0 * s[2] * s[2]).sqrt();
    let nodes: Vec<[f64; 3]> = plate_nodes.iter().map(|&k| pos[k]).collect();
    let disp: Vec<[f64; 3]> = plate_nodes.iter().map(|&k| [u[6 * k], u[6 * k + 1], u[6 * k + 2]]).collect();
    let mut fields = vec![];
    let mut push = |key: &'static str, v: Vec<f64>| {
        let (mut lo, mut hi) = (0, 0);
        for i in 0..v.len() {
            if v[i] < v[lo] {
                lo = i;
            }
            if v[i] > v[hi] {
                hi = i;
            }
        }
        if !v.is_empty() {
            fields.push(crate::solid::Field3 { key, min: v[lo], max: v[hi], at_min: nodes[lo], at_max: nodes[hi], v });
        }
    };
    push("umag", disp.iter().map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()).collect());
    push("ux", disp.iter().map(|d| d[0]).collect());
    push("uy", disp.iter().map(|d| d[1]).collect());
    push("uz", disp.iter().map(|d| d[2]).collect());
    push("vm", (0..np).map(|i| vm([avg(i, 6), avg(i, 7), avg(i, 8)]).max(vm([avg(i, 9), avg(i, 10), avg(i, 11)]))).collect());
    for (q, key) in keys.iter().enumerate() {
        push(key, (0..np).map(|i| avg(i, q)).collect());
    }
    PlateResults { nodes, tris, disp, fields }
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

fn member_result(e: &Elem, ul: [f64; 12]) -> SpaceMember {
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

// ---------- dynamics (0.4.2) ----------

/// A 3D frame (with its plates) for the dynamics.
pub struct Dyn {
    m: Model,
    asm: Assembly,
    mass: Vec<(usize, usize, f64)>,
}

impl Dyn {
    pub fn new(m: &Model, pieces: usize) -> Result<Dyn, String> {
        let asm = assemble(m, pieces.max(1))?;
        let mut mass = vec![];
        for e in &asm.parts {
            triplets(e, &cond(e, m_local(e)), &mut mass);
        }
        for sh in &asm.shells {
            let (_, _, area) = sh.tri.frame();
            let mt = sh.rho * area / 3.0;
            let mr = mt * sh.tri.t * sh.tri.t / 12.0;
            for &k in &sh.n {
                for d in 0..3 {
                    mass.push((6 * k + d, 6 * k + d, mt));
                    mass.push((6 * k + 3 + d, 6 * k + 3 + d, mr));
                }
            }
        }
        mass.extend(crate::frame::point_masses(m, 6, 3));
        if !mass.iter().any(|x| x.2 > 0.0) {
            return Err("The model has no mass: give the materials a density, or put masses on nodes (Loads › Mass).".into());
        }
        Ok(Dyn { m: m.clone(), asm, mass })
    }
    pub fn load(&self) -> &[f64] {
        &self.asm.sys.f
    }

    /// The modes' results combined value by value (CQC or SRSS by `rho`): peaks without signs.
    pub fn combine(&self, outs: &[SpaceResults], rho: &[Vec<f64>]) -> SpaceResults {
        let c = |f: &dyn Fn(&SpaceResults) -> f64| crate::engine::dynamics::combine(&outs.iter().map(f).collect::<Vec<_>>(), rho);
        let mut r = outs[0].clone();
        for i in 0..r.disp.len() {
            for k in 0..6 {
                r.disp[i][k] = c(&|x| x.disp[i][k]);
            }
        }
        for i in 0..r.reactions.len() {
            for k in 0..6 {
                r.reactions[i].f[k] = c(&|x| x.reactions[i].f[k]);
            }
        }
        for (j, e) in self.asm.elems.iter().enumerate() {
            let mr = &mut r.members[j];
            for i in 0..mr.x.len() {
                mr.n[i] = c(&|x| x.members[j].n[i]);
                mr.vy[i] = c(&|x| x.members[j].vy[i]);
                mr.vz[i] = c(&|x| x.members[j].vz[i]);
                mr.t[i] = c(&|x| x.members[j].t[i]);
                mr.my[i] = c(&|x| x.members[j].my[i]);
                mr.mz[i] = c(&|x| x.members[j].mz[i]);
                for k in 0..3 {
                    mr.d[i][k] = c(&|x| x.members[j].d[i][k]);
                }
            }
            mr.stress = (0..mr.x.len()).map(|i| mr.n[i] / e.area + mr.mz[i] / e.zz + mr.my[i] / e.zy).fold(0.0, f64::max);
            mr.util = if e.fy > 0.0 { mr.stress / e.fy } else { 0.0 };
        }
        if let Some(p) = r.plates.as_mut() {
            for i in 0..p.disp.len() {
                for k in 0..3 {
                    p.disp[i][k] = c(&|x| x.plates.as_ref().map_or(0.0, |q| q.disp[i][k]));
                }
            }
            for (fi, fl) in p.fields.iter_mut().enumerate() {
                for i in 0..fl.v.len() {
                    fl.v[i] = c(&|x| x.plates.as_ref().map_or(0.0, |q| q.fields[fi].v[i]));
                }
                let (mut lo, mut hi) = (0, 0);
                for i in 0..fl.v.len() {
                    if fl.v[i] < fl.v[lo] {
                        lo = i;
                    }
                    if fl.v[i] > fl.v[hi] {
                        hi = i;
                    }
                }
                if !fl.v.is_empty() {
                    fl.min = fl.v[lo];
                    fl.max = fl.v[hi];
                    fl.at_min = p.nodes[lo];
                    fl.at_max = p.nodes[hi];
                }
            }
        }
        for k in 0..6 {
            r.applied[k] = c(&|x| x.applied[k]);
            r.reacted[k] = c(&|x| x.reacted[k]);
        }
        r.imbalance = 0.0;
        r
    }
}

impl crate::engine::dynamics::Structure for Dyn {
    type Out = SpaceResults;
    fn k(&self) -> &System {
        &self.asm.sys
    }
    fn held(&self) -> &[bool] {
        &self.asm.held
    }
    fn mass(&self) -> &[(usize, usize, f64)] {
        &self.mass
    }
    fn ground(&self, dir: [f64; 3]) -> Vec<f64> {
        (0..self.asm.sys.n).map(|i| if self.asm.held[i] || i % 6 >= 3 { 0.0 } else { dir[i % 6] }).collect()
    }
    fn node(&self, i: usize) -> [usize; 3] {
        [6 * i, 6 * i + 1, 6 * i + 2]
    }
    fn results(&self, u: &[f64], f: &[f64], p: f64) -> SpaceResults {
        results_from(&self.m, &self.asm, u, f, p)
    }
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
    /// The plates' nodes' movements (0.4.1), in the order of `SpaceModes::plates`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub plate_disp: Vec<[f64; 3]>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpaceModes {
    pub modes: Vec<SpaceMode>,
    /// The plates' mesh (0.4.1): nodes and triangles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plates: Option<(Vec<[f64; 3]>, Vec<[u32; 3]>)>,
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
    mass.extend(crate::frame::point_masses(m, 6, 3));
    // Plates: lumped masses, a third of each triangle on each corner (and the rotary inertia of
    // its thickness, ρt·t²/12, so no rotation is without mass).
    for sh in &asm.shells {
        let (_, _, area) = sh.tri.frame();
        let mt = sh.rho * area / 3.0;
        let mr = mt * sh.tri.t * sh.tri.t / 12.0;
        for &k in &sh.n {
            for d in 0..3 {
                mass.push((6 * k + d, 6 * k + d, mt));
                mass.push((6 * k + 3 + d, 6 * k + 3 + d, mr));
            }
        }
    }
    let (found, iterations) = crate::modal::lowest(&asm.sys, &mass, &asm.held, count)?;
    let modes = found
        .into_iter()
        .map(|md| {
            let (disp, members) = shapes(m, &asm, &md.shape);
            let plate_disp = asm.plate_nodes.iter().map(|&k| [md.shape[6 * k], md.shape[6 * k + 1], md.shape[6 * k + 2]]).collect();
            SpaceMode { f: md.f, disp, members, plate_disp }
        })
        .collect();
    let back: std::collections::HashMap<usize, u32> = asm.plate_nodes.iter().enumerate().map(|(i, &k)| (k, i as u32)).collect();
    let plates = (!asm.shells.is_empty()).then(|| (asm.plate_nodes.iter().map(|&k| asm.pos[k]).collect(), asm.shells.iter().map(|s| s.n.map(|k| back[&k])).collect()));
    Ok(SpaceModes { modes, plates, equations: asm.held.iter().filter(|h| !**h).count(), iterations, ms: t0.elapsed().as_secs_f64() * 1e3, warnings: vec![] })
}

/// Linear buckling of a 3D frame: the lowest load factors λ at which every load times λ buckles
/// it, with each squeezed truss bar also checked on its own about its weaker axis.
pub fn buckling(m: &Model, count: usize) -> Result<SpaceBuckling, String> {
    let t0 = std::time::Instant::now();
    let asm = assemble(m, crate::frame::SPLIT)?;
    if !asm.shells.is_empty() {
        return Err("Buckling with plates comes in a later release: buckling here is for frames and trusses.".into());
    }
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
