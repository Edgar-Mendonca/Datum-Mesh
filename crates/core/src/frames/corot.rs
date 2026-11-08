//! Large deflection and second-order (P-delta) effects in plane frames (0.4.0): the corotational
//! beam. Each piece of a member moves as a rigid body plus a small deformation measured in axes
//! that turn with its chord; in those axes it is the ordinary linear beam (Crisfield, *Non-linear
//! Finite Element Analysis of Solids and Structures*, vol. 1, 1991, ch. 7). So the axial force
//! acting on the deflected shape (P-delta), large rotations and snap-through all come from the
//! geometry, exactly, as the pieces get shorter.
//!
//! Local deformations of a piece with original length ℓ₀ and chord angle β₀, now of length ℓ and
//! angle β: ū = ℓ − ℓ₀, θ̄ₐ = θₐ − (β − β₀), θ̄_b = θ_b − (β − β₀). Local forces N = EA ū/ℓ₀ and
//! (Mₐ, M_b) from 4EI/ℓ₀ and 2EI/ℓ₀ (a hinge condenses its end out). With r = (−c, −s, 0, c, s, 0)
//! and z = (s, −c, 0, −s, c, 0) for the chord's present direction (c, s):
//!
//! - internal forces f = r N + (e₃ − z/ℓ) Mₐ + (e₆ − z/ℓ) M_b;
//! - tangent K = Bᵀ D B + (N/ℓ) z zᵀ + ((Mₐ + M_b)/ℓ²)(r zᵀ + z rᵀ), B the rows of f.
//!
//! Loads keep their directions (dead loads). A member's own loads go to its pieces' ends as in
//! the linear frame; a warmed member's free strain is in N.

use super::frame::{assemble, cond_load, unrot, Assembly, Elem, FrameResults, MemberResult, Reaction};
use crate::engine::path::Problem;
use crate::model::{MemberKind, Model, SupportKind, Target};
use crate::sparse::{Stats, System};
use serde::Serialize;

/// A load–deflection curve: one entry per converged increment.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Curve {
    /// The node it follows.
    pub node: u32,
    /// Step number (from 1) and the step's load factor (its case or combination times this).
    pub step: Vec<u32>,
    pub load: Vec<f64>,
    /// The node's movements: x, y (m) and rotation (rad).
    pub ux: Vec<f64>,
    pub uy: Vec<f64>,
    pub rz: Vec<f64>,
    /// 3D frames (0.5.8): the movement along z (empty in a plane frame).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uz: Vec<f64>,
    /// The structure was stable there (its tangent stiffness positive definite).
    pub stable: Vec<bool>,
    /// The first limit: the largest load factor before the load first falls (the largest of
    /// all if it never does), and the step it was in.
    pub peak: f64,
    pub peak_step: u32,
    /// Newton iterations in all.
    pub iterations: usize,
}

struct Piece {
    a: usize,
    b: usize,
    beam: bool,
    l0: f64,
    c0: f64,
    s0: f64,
    ea: f64,
    at: f64,
    /// Bending stiffness of the two end rotations, hinges condensed out.
    bend: [[f64; 2]; 2],
}

/// The frame as a nonlinear problem for the path solver.
pub struct Corot {
    pub(crate) asm: Assembly,
    pieces: Vec<Piece>,
    /// Loads on the pieces' ends that stand for their own loads, without warming (local axes, as
    /// `cond_load` gives them), to take off the end forces for the diagrams.
    eq: Vec<[f64; 6]>,
}

fn wrap(a: f64) -> f64 {
    let t = std::f64::consts::TAU;
    a - t * ((a + std::f64::consts::PI) / t).floor()
}

impl Corot {
    /// The model's frame cut into `pieces` per beam, and its load vector (warming taken out of
    /// the loads: it is in the pieces' axial force instead).
    pub fn new(m: &Model, pieces: usize) -> Result<(Corot, Vec<f64>), String> {
        let asm = assemble(m, pieces.max(1))?;
        let mut f = asm.sys.f.clone();
        let mut out = vec![];
        let mut eq = vec![];
        for e in &asm.parts {
            let cold = Elem { at: 0.0, ..e.clone() };
            let (hot, cl) = (cond_load(e), cond_load(&cold));
            for (node, k0) in [(e.a, 0), (e.b, 3)] {
                let d = unrot(e, [hot[k0] - cl[k0], hot[k0 + 1] - cl[k0 + 1], hot[k0 + 2] - cl[k0 + 2]]);
                for k in 0..3 {
                    f[3 * node + k] -= d[k];
                }
            }
            eq.push(cl);
            let beam = e.kind == MemberKind::Beam;
            let k = e.ei / e.l;
            let bend = match (beam, e.hinge) {
                (false, _) | (true, [true, true]) => [[0.0; 2]; 2],
                (true, [false, false]) => [[4.0 * k, 2.0 * k], [2.0 * k, 4.0 * k]],
                (true, [true, false]) => [[0.0, 0.0], [0.0, 3.0 * k]],
                (true, [false, true]) => [[3.0 * k, 0.0], [0.0, 0.0]],
            };
            out.push(Piece { a: e.a, b: e.b, beam, l0: e.l, c0: e.c, s0: e.s, ea: e.ea, at: e.at, bend });
        }
        // Rotations of nodes no beam holds are held by the assembly; loose ends of nothing too.
        Ok((Corot { asm, pieces: out, eq }, f))
    }

    /// One piece's present chord, deformations, local forces [N, Mₐ, M_b] and r, z vectors.
    fn local(&self, p: &Piece, u: &[f64]) -> (f64, [f64; 3], [f64; 3], [f64; 6], [f64; 6], f64, f64) {
        let (pa, pb) = (self.asm.pos[p.a], self.asm.pos[p.b]);
        // From the movements, not the positions, so small ones keep their digits.
        let (ux, uy) = (u[3 * p.b] - u[3 * p.a], u[3 * p.b + 1] - u[3 * p.a + 1]);
        let (x0, y0) = (pb[0] - pa[0], pb[1] - pa[1]);
        let (dx, dy) = (x0 + ux, y0 + uy);
        let l = dx.hypot(dy);
        let (c, s) = (dx / l, dy / l);
        let ubar = (2.0 * (x0 * ux + y0 * uy) + ux * ux + uy * uy) / (l + p.l0);
        let alpha = (p.c0 * uy - p.s0 * ux).atan2(p.l0 + p.c0 * ux + p.s0 * uy);
        let (ta, tb) = if p.beam { (wrap(u[3 * p.a + 2] - alpha), wrap(u[3 * p.b + 2] - alpha)) } else { (0.0, 0.0) };
        let n = p.ea / p.l0 * (ubar - p.at * p.l0);
        let ma = p.bend[0][0] * ta + p.bend[0][1] * tb;
        let mb = p.bend[1][0] * ta + p.bend[1][1] * tb;
        let r = [-c, -s, 0.0, c, s, 0.0];
        let z = [s, -c, 0.0, -s, c, 0.0];
        (l, [ubar, ta, tb], [n, ma, mb], r, z, c, s)
    }

    fn dofs(p: &Piece) -> [usize; 6] {
        [3 * p.a, 3 * p.a + 1, 3 * p.a + 2, 3 * p.b, 3 * p.b + 1, 3 * p.b + 2]
    }

    /// Each piece's internal forces on its nodes (global axes).
    fn forces(&self, p: &Piece, u: &[f64]) -> [f64; 6] {
        let (l, _, q, r, z, ..) = self.local(p, u);
        let mut f = [0.0; 6];
        for i in 0..6 {
            f[i] = r[i] * q[0] - z[i] / l * (q[1] + q[2]);
        }
        f[2] += q[1];
        f[5] += q[2];
        f
    }

    /// Results at movements `u` under the load vector `f` (the one the path ended on).
    pub fn results(&self, m: &Model, u: &[f64], f: &[f64], stats: Stats, mut warnings: Vec<String>) -> FrameResults {
        let asm = &self.asm;
        let nn = m.nodes.len();
        let mut fint = vec![0.0; u.len()];
        for p in &self.pieces {
            let fe = self.forces(p, u);
            for (k, d) in Self::dofs(p).iter().enumerate() {
                fint[*d] += fe[k];
            }
        }
        let at = |i: usize| [asm.pos[i][0] + u[3 * i], asm.pos[i][1] + u[3 * i + 1]];
        // Loads and reactions on the deflected structure: moments about the origin from where
        // the nodes are now.
        let mut applied = [0.0; 3];
        for i in 0..asm.pos.len() {
            let [x, y] = at(i);
            applied[0] += f[3 * i];
            applied[1] += f[3 * i + 1];
            applied[2] += f[3 * i + 2] + x * f[3 * i + 1] - y * f[3 * i];
        }
        let mut reactions = vec![];
        let mut reacted = [0.0; 3];
        for (i, n) in m.nodes.iter().enumerate() {
            let sp = m.supports.iter().find(|s| s.at == Target::Node { id: n.id });
            let Some(sp) = sp else { continue };
            let mut r = [0.0; 3];
            for k in 0..3 {
                if asm.held[3 * i + k] && !(k == 2 && asm.free_rot[i]) && asm.has_member[i] {
                    r[k] = fint[3 * i + k] - f[3 * i + k];
                }
            }
            if let SupportKind::Spring { kx, ky, kr, .. } = sp.kind {
                r = [-kx * u[3 * i], -ky * u[3 * i + 1], -kr * u[3 * i + 2]];
            }
            let [x, y] = at(i);
            reactions.push(Reaction { node: n.id, fx: r[0], fy: r[1], m: r[2] });
            reacted[0] += r[0];
            reacted[1] += r[1];
            reacted[2] += r[2] + x * r[1] - y * r[0];
        }
        let reach = (0..asm.pos.len()).map(|i| at(i)[0].hypot(at(i)[1])).fold(0.0, f64::max) + m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(0.0);
        let fscale: f64 = (0..nn).map(|i| f[3 * i].abs() + f[3 * i + 1].abs()).sum();
        let scale = [fscale, fscale * reach + (0..nn).map(|i| f[3 * i + 2].abs()).sum::<f64>()];
        let disp: Vec<[f64; 3]> = (0..nn).map(|i| [u[3 * i], u[3 * i + 1], if asm.free_rot[i] || !asm.has_beam[i] { 0.0 } else { u[3 * i + 2] }]).collect();
        let members = asm.elems.iter().zip(&asm.spans).map(|(e, span)| self.member(e, span.clone(), u)).collect();
        if fscale == 0.0 && scale[1] == 0.0 {
            warnings.push("There are no loads on the model, so nothing moves. Add loads on the Loads tab.".into());
        }
        let mut r = FrameResults { disp, reactions, members, applied, reacted, scale, imbalance: 0.0, stats, warnings, path: None, frames: None, dynamic: None, slack: vec![] };
        r.check_balance();
        r
    }

    /// One member's diagrams and deflected shape from its pieces.
    fn member(&self, e: &Elem, span: std::ops::Range<usize>, u: &[f64]) -> MemberResult {
        const SUB: usize = 4;
        let (mut x, mut nv, mut vv, mut mv, mut d) = (vec![], vec![], vec![], vec![], vec![]);
        let mut start = 0.0;
        let mut first_ends = [0.0; 6];
        let mut last_ends = [0.0; 6];
        let mut stress: f64 = 0.0;
        for (k, pi) in span.enumerate() {
            let p = &self.pieces[pi];
            let part = &self.asm.parts[pi];
            let jumps = k > 0 && self.asm.parts[pi - 1].tip[2..].iter().any(|t| *t != 0.0);
            let (l, def, _, _, _, c, s) = self.local(p, u);
            let fe = self.forces(p, u);
            // End forces in the piece's present axes, its own loads taken off.
            let loc = |g: [f64; 3]| [c * g[0] + s * g[1], -s * g[0] + c * g[1], g[2]];
            let (fa, fb) = (loc([fe[0], fe[1], fe[2]]), loc([fe[3], fe[4], fe[5]]));
            let eq = self.eq[pi];
            let ends = [fa[0] - eq[0], fa[1] - eq[1], fa[2] - eq[2], fb[0] - eq[3], fb[1] - eq[4], fb[2] - eq[5]];
            if k == 0 {
                first_ends = ends;
            }
            last_ends = ends;
            let [p1, p2, q1, q2] = part.p;
            let l0 = p.l0;
            let px = |t: f64| p1 * t + (p2 - p1) * t * t / (2.0 * l0);
            let qv = |t: f64| q1 * t + (q2 - q1) * t * t / (2.0 * l0);
            let qm = |t: f64| q1 * t * t / 2.0 + (q2 - q1) * t.powi(3) / (6.0 * l0);
            let (pa, pb) = (self.asm.pos[p.a], self.asm.pos[p.b]);
            let xa = [pa[0] + u[3 * p.a], pa[1] + u[3 * p.a + 1]];
            for j in 0..=SUB {
                let t = l0 * j as f64 / SUB as f64;
                let xi = j as f64 / SUB as f64;
                let n = -ends[0] - px(t);
                let v = ends[1] + qv(t);
                let mm = if p.beam { -ends[2] + ends[1] * t + qm(t) } else { ends[1] * t + qm(t) };
                // A piece's start repeats the last one's end; kept where a point load acts (0.10.3).
                if k > 0 && j == 0 && !jumps {
                    continue;
                }
                x.push(start + t);
                nv.push(n);
                vv.push(v);
                mv.push(mm);
                stress = stress.max(n.abs() / e.area + mm.abs() / e.z);
                // Where that point is now: along the chord, plus the cubic bending off it.
                let w = if p.beam { l * (def[1] * (xi - 2.0 * xi * xi + xi.powi(3)) + def[2] * (xi.powi(3) - xi * xi)) } else { 0.0 };
                let now = [xa[0] + c * l * xi - s * w, xa[1] + s * l * xi + c * w];
                let was = [pa[0] + (pb[0] - pa[0]) * xi, pa[1] + (pb[1] - pa[1]) * xi];
                d.push([now[0] - was[0], now[1] - was[1]]);
            }
            start += l0;
        }
        let util = if e.fy > 0.0 { stress / e.fy } else { 0.0 };
        let ends = [first_ends[0], first_ends[1], first_ends[2], last_ends[3], last_ends[4], last_ends[5]];
        MemberResult { id: e.id, length: e.l, x, n: nv, v: vv, m: mv, d, stress, util, ends }
    }
}

impl Problem for Corot {
    fn size(&self) -> usize {
        self.asm.sys.n
    }
    fn held(&self) -> &[bool] {
        &self.asm.held
    }
    fn state(&self, u: &[f64]) -> Result<(Vec<f64>, System), String> {
        let n = self.size();
        let mut fint = vec![0.0; n];
        let mut k = System::new(n);
        for p in &self.pieces {
            let (l, _, q, r, z, ..) = self.local(p, u);
            if !(l > 0.0 && l.is_finite()) {
                return Err("A member has folded to zero length.".into());
            }
            // B: rows for ū, θ̄ₐ, θ̄_b.
            let mut b = [[0.0; 6]; 3];
            for i in 0..6 {
                b[0][i] = r[i];
                b[1][i] = -z[i] / l;
                b[2][i] = -z[i] / l;
            }
            b[1][2] += 1.0;
            b[2][5] += 1.0;
            let dmat = [[p.ea / p.l0, 0.0, 0.0], [0.0, p.bend[0][0], p.bend[0][1]], [0.0, p.bend[1][0], p.bend[1][1]]];
            let dofs = Self::dofs(p);
            let mm = q[1] + q[2];
            for i in 0..6 {
                fint[dofs[i]] += b[0][i] * q[0] + b[1][i] * q[1] + b[2][i] * q[2];
                for j in 0..6 {
                    let mut v = 0.0;
                    for a in 0..3 {
                        for c in 0..3 {
                            v += b[a][i] * dmat[a][c] * b[c][j];
                        }
                    }
                    v += q[0] / l * z[i] * z[j] + mm / (l * l) * (r[i] * z[j] + z[i] * r[j]);
                    k.add(dofs[i], dofs[j], v);
                }
            }
        }
        for &(d, s) in &self.asm.springs {
            fint[d] += s * u[d];
            k.add(d, d, s);
        }
        Ok((fint, k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, Dir, Load, LoadKind, Material, Section, SectionShape, Support};

    pub(crate) fn frame(pts: &[[f64; 2]], e: f64) -> Doc {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let mut mt = Material::steel(1);
        mt.e = e;
        mt.rho = 0.0;
        d.m.materials = vec![mt];
        d.m.sections = vec![Section { id: 2, name: "R".into(), shape: SectionShape::Rect { b: 0.1, h: 0.2 } }];
        d.m.next = 3;
        for w in pts.windows(2) {
            d.apply(Op::AddMember { from: w[0], to: w[1], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        }
        d
    }
    fn node(d: &Doc, p: [f64; 2]) -> u32 {
        d.m.nodes.iter().find(|n| (n.x - p[0]).abs() < 1e-9 && (n.y - p[1]).abs() < 1e-9).unwrap().id
    }
    fn sup(d: &mut Doc, p: [f64; 2], kind: SupportKind) {
        let id = node(d, p);
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind }) }).unwrap();
    }

    #[test]
    fn the_tangent_is_the_derivative_of_the_internal_forces() {
        // A bent, turned, stretched two-member frame with a hinge, a truss bar and a warm member:
        // the tangent against central differences of the internal forces.
        let mut d = frame(&[[0.0, 0.0], [2.0, 1.0], [3.0, -0.5]], 200e9);
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [3.0, -0.5], kind: MemberKind::Truss, section: None, material: None, z: [0.0; 2] }).unwrap();
        d.m.members[1].hinge = [true, false];
        sup(&mut d, [0.0, 0.0], SupportKind::Pinned);
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Body, kind: LoadKind::Warming { dt: 30.0 } }) }).unwrap();
        let (c, _) = Corot::new(&d.m, 3).unwrap();
        let n = c.size();
        let u: Vec<f64> = (0..n).map(|i| 0.05 * ((i as f64 * 1.7).sin()) + if i % 3 == 2 { 0.4 } else { 0.0 }).collect();
        let (_, k) = c.state(&u).unwrap();
        let mut kd = vec![vec![0.0; n]; n];
        for &(i, j, v) in &k.k {
            kd[i][j] += v;
        }
        let h = 1e-6;
        for j in 0..n {
            let (mut up, mut dn) = (u.clone(), u.clone());
            up[j] += h;
            dn[j] -= h;
            let (fp, _) = c.state(&up).unwrap();
            let (fm, _) = c.state(&dn).unwrap();
            for i in 0..n {
                let fd = (fp[i] - fm[i]) / (2.0 * h);
                let big = kd[i].iter().fold(0.0f64, |a, b| a.max(b.abs()));
                assert!((fd - kd[i][j]).abs() < 1e-5 * big.max(1.0), "K[{i}][{j}] {} vs {fd}", kd[i][j]);
            }
        }
        // Symmetric (conservative loads, a potential).
        for i in 0..n {
            for j in 0..n {
                assert!((kd[i][j] - kd[j][i]).abs() < 1e-6 * kd[i][i].abs().max(kd[j][j].abs()).max(1.0));
            }
        }
    }

    #[test]
    fn a_rigid_turn_makes_no_forces() {
        let mut d = frame(&[[0.0, 0.0], [2.0, 1.0], [3.0, -0.5]], 200e9);
        sup(&mut d, [0.0, 0.0], SupportKind::Pinned);
        let (c, _) = Corot::new(&d.m, 2).unwrap();
        let t: f64 = 2.3;
        let u: Vec<f64> = c.asm.pos.iter().flat_map(|p| [p[0] * t.cos() - p[1] * t.sin() - p[0] + 0.7, p[0] * t.sin() + p[1] * t.cos() - p[1] - 0.2, t]).collect();
        let (f, _) = c.state(&u).unwrap();
        assert!(f.iter().all(|x| x.abs() < 1e-3), "{f:?}");
    }

    #[test]
    fn small_loads_give_the_linear_answer() {
        // A propped cantilever with a span load, an end moment and a side load: at a tiny load
        // the corotational frame agrees with the linear one.
        let mut d = frame(&[[0.0, 0.0], [2.0, 0.0], [4.0, 0.5]], 200e9);
        sup(&mut d, [0.0, 0.0], SupportKind::Fixed);
        sup(&mut d, [4.0, 0.5], SupportKind::RollerX);
        let id = d.m.members[0].id;
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Member { id }, kind: LoadKind::Dist { w1: -10.0, w2: -30.0, dir: Dir::Y } }) }).unwrap();
        let mid = node(&d, [2.0, 0.0]);
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Node { id: mid }, kind: LoadKind::Moment { m: 7.0, mx: 0.0, my: 0.0 } }) }).unwrap();
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Node { id: mid }, kind: LoadKind::Force { fx: 5.0, fy: 0.0, fz: 0.0 } }) }).unwrap();
        let lin = crate::frame::solve(&d.m).unwrap();
        let (c, f) = Corot::new(&d.m, 4).unwrap();
        let path = crate::engine::path::follow(&c, &vec![0.0; f.len()], &f, &vec![0.0; f.len()], &Default::default()).unwrap();
        assert!(path.reached, "{:?} {:?}", path.note, path.points.len());
        let u = &path.points.last().unwrap().u;
        let r = c.results(&d.m, u, &f, Stats::default(), vec![]);
        // Within the bowing of the deflected members (their ends draw together by about v²/L).
        let big = lin.disp.iter().flat_map(|d| d.iter()).fold(0.0f64, |a, b| a.max(b.abs()));
        for (a, b) in r.disp.iter().zip(&lin.disp) {
            for k in 0..3 {
                assert!((a[k] - b[k]).abs() < 1e-5 * big, "{a:?} {b:?}");
            }
        }
        for (a, b) in r.reactions.iter().zip(&lin.reactions) {
            assert!((a.fy - b.fy).abs() < 1e-4 && (a.m - b.m).abs() < 1e-4, "{a:?} {b:?}");
        }
        for (a, b) in r.members.iter().zip(&lin.members) {
            let (ma, mb) = (a.m.iter().fold(0.0f64, |x, y| x.max(y.abs())), b.m.iter().fold(0.0f64, |x, y| x.max(y.abs())));
            assert!((ma - mb).abs() < 1e-3 * mb, "{ma} {mb}");
            assert!((a.ends[2] - b.ends[2]).abs() < 1e-3 * mb, "{:?} {:?}", a.ends, b.ends);
        }
        assert!(r.imbalance < 1e-9, "{}", r.imbalance);
    }
}
