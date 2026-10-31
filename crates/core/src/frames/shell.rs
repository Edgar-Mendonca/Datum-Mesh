//! Plates and shells (0.4.1): the flat shell triangle, three nodes with six unknowns each
//! (movements u, v, w and rotations θx, θy, θz), so it joins 3D beams node to node.
//!
//! - **Membrane**: Allman's triangle (Allman 1984, in the form of Cook, *Concepts and Applications
//!   of Finite Element Analysis*): the six-node (linear strain) triangle whose midside movements
//!   come from the corners' movements and their in-plane (drilling) rotations,
//!   u_k = (u_i + u_j)/2 + (y_j − y_i)(θ_j − θ_i)/8, v_k = (v_i + v_j)/2 + (x_i − x_j)(θ_j − θ_i)/8.
//!   Its one spurious mode (every θ the same, nothing moving) is held by a light penalty on the
//!   difference between the mean drilling rotation and the element's own rotation.
//! - **Bending**: the Discrete Kirchhoff Triangle (Batoz, Bathe and Ho 1980): normal rotations
//!   quadratic, the Kirchhoff condition (no transverse shear) imposed at the corners and the
//!   middles of the sides, w cubic along the sides. Thin plates.
//!
//! In the element's axes x along side 1–2, z the normal (right-hand from 1, 2, 3): rotations θx,
//! θy are about x and y (right-hand rule), so ∂w/∂y = θx, ∂w/∂x = −θy. Moments per length, Mx
//! bending about y (stress along x), positive when the +z face is in tension.

/// One triangle's geometry and section.
#[derive(Clone, Debug)]
pub struct Tri {
    /// Corner positions (global).
    pub p: [[f64; 3]; 3],
    pub t: f64,
    pub e: f64,
    pub nu: f64,
}

pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

impl Tri {
    /// The element's axes (rows: x, y, z = normal) and corner coordinates in them.
    pub fn frame(&self) -> ([[f64; 3]; 3], [[f64; 2]; 3], f64) {
        let ex = unit(sub(self.p[1], self.p[0]));
        let n = cross(sub(self.p[1], self.p[0]), sub(self.p[2], self.p[0]));
        let area = dot(n, n).sqrt() / 2.0;
        let ez = unit(n);
        let ey = cross(ez, ex);
        let loc = std::array::from_fn(|i| {
            let d = sub(self.p[i], self.p[0]);
            [dot(d, ex), dot(d, ey)]
        });
        ([ex, ey, ez], loc, area)
    }

    fn dm(&self) -> [[f64; 3]; 3] {
        let c = self.e * self.t / (1.0 - self.nu * self.nu);
        [[c, c * self.nu, 0.0], [c * self.nu, c, 0.0], [0.0, 0.0, c * (1.0 - self.nu) / 2.0]]
    }
    fn db(&self) -> [[f64; 3]; 3] {
        let d = self.e * self.t.powi(3) / (12.0 * (1.0 - self.nu * self.nu));
        [[d, d * self.nu, 0.0], [d * self.nu, d, 0.0], [0.0, 0.0, d * (1.0 - self.nu) / 2.0]]
    }

    /// Membrane strain–movement matrix at area coordinates `l` (3 × 9: u, v, θz per corner).
    fn bm(xy: &[[f64; 2]; 3], area: f64, l: [f64; 3]) -> [[f64; 9]; 3] {
        // The six-node triangle's B (3 × 12: u, v per node 1..6), then Allman's map to corners.
        let b = |i: usize| xy[(i + 1) % 3][1] - xy[(i + 2) % 3][1];
        let c = |i: usize| xy[(i + 2) % 3][0] - xy[(i + 1) % 3][0];
        let dl = |i: usize| [b(i) / (2.0 * area), c(i) / (2.0 * area)];
        let mut dn = [[0.0; 2]; 6];
        for i in 0..3 {
            let d = dl(i);
            for k in 0..2 {
                dn[i][k] = (4.0 * l[i] - 1.0) * d[k];
            }
        }
        // Midsides: 3 on side 0–1, 4 on 1–2, 5 on 2–0.
        for (m, (i, j)) in [(0, 1), (1, 2), (2, 0)].into_iter().enumerate() {
            let (di, dj) = (dl(i), dl(j));
            for k in 0..2 {
                dn[3 + m][k] = 4.0 * (l[i] * dj[k] + l[j] * di[k]);
            }
        }
        // T: LST unknowns (u1..u6, v1..v6) from corner unknowns.
        let mut tu = [[0.0; 9]; 6];
        let mut tv = [[0.0; 9]; 6];
        for i in 0..3 {
            tu[i][3 * i] = 1.0;
            tv[i][3 * i + 1] = 1.0;
        }
        for (m, (i, j)) in [(0, 1), (1, 2), (2, 0)].into_iter().enumerate() {
            let (xi, yi, xj, yj) = (xy[i][0], xy[i][1], xy[j][0], xy[j][1]);
            tu[3 + m][3 * i] = 0.5;
            tu[3 + m][3 * j] = 0.5;
            tu[3 + m][3 * j + 2] += (yj - yi) / 8.0;
            tu[3 + m][3 * i + 2] -= (yj - yi) / 8.0;
            tv[3 + m][3 * i + 1] = 0.5;
            tv[3 + m][3 * j + 1] = 0.5;
            tv[3 + m][3 * j + 2] += (xi - xj) / 8.0;
            tv[3 + m][3 * i + 2] -= (xi - xj) / 8.0;
        }
        let mut out = [[0.0; 9]; 3];
        for q in 0..9 {
            for n in 0..6 {
                out[0][q] += dn[n][0] * tu[n][q];
                out[1][q] += dn[n][1] * tv[n][q];
                out[2][q] += dn[n][1] * tu[n][q] + dn[n][0] * tv[n][q];
            }
        }
        out
    }

    /// Bending curvature–movement matrix at (ξ, η) (3 × 9: w, θx, θy per corner).
    fn bb(xy: &[[f64; 2]; 3], xi: f64, eta: f64) -> [[f64; 9]; 3] {
        let (x, y) = (|i: usize| xy[i][0], |i: usize| xy[i][1]);
        // Sides k = 4, 5, 6 (here 0, 1, 2) join corners 2–3, 3–1, 1–2.
        let sides = [(1, 2), (2, 0), (0, 1)];
        let mut a = [0.0; 3];
        let mut bc = [0.0; 3];
        let mut c = [0.0; 3];
        let mut d = [0.0; 3];
        let mut e = [0.0; 3];
        for (k, (i, j)) in sides.into_iter().enumerate() {
            let (xij, yij) = (x(i) - x(j), y(i) - y(j));
            let l2 = xij * xij + yij * yij;
            a[k] = -xij / l2;
            bc[k] = 0.75 * xij * yij / l2;
            c[k] = (0.25 * xij * xij - 0.5 * yij * yij) / l2;
            d[k] = -yij / l2;
            e[k] = (0.25 * yij * yij - 0.5 * xij * xij) / l2;
        }
        // Derivatives of N1..N6 (corners, then sides 2–3, 3–1, 1–2) by ξ and η.
        let n_xi = [-3.0 + 4.0 * (xi + eta), 4.0 * xi - 1.0, 0.0, 4.0 * eta, -4.0 * eta, 4.0 * (1.0 - 2.0 * xi - eta)];
        let n_eta = [-3.0 + 4.0 * (xi + eta), 0.0, 4.0 * eta - 1.0, 4.0 * xi, 4.0 * (1.0 - xi - 2.0 * eta), -4.0 * xi];
        // H_x and H_y (Batoz, Bathe and Ho 1980, eq. 8), differentiated: index [s][9].
        let h = |dn: &[f64; 6]| -> ([f64; 9], [f64; 9]) {
            let (n1, n2, n3, n4, n5, n6) = (dn[0], dn[1], dn[2], dn[3], dn[4], dn[5]);
            let hx = [
                1.5 * (a[2] * n6 - a[1] * n5),
                bc[1] * n5 + bc[2] * n6,
                n1 - c[1] * n5 - c[2] * n6,
                1.5 * (a[0] * n4 - a[2] * n6),
                bc[2] * n6 + bc[0] * n4,
                n2 - c[2] * n6 - c[0] * n4,
                1.5 * (a[1] * n5 - a[0] * n4),
                bc[0] * n4 + bc[1] * n5,
                n3 - c[0] * n4 - c[1] * n5,
            ];
            let hy = [
                1.5 * (d[2] * n6 - d[1] * n5),
                -n1 + e[1] * n5 + e[2] * n6,
                -bc[1] * n5 - bc[2] * n6,
                1.5 * (d[0] * n4 - d[2] * n6),
                -n2 + e[2] * n6 + e[0] * n4,
                -bc[2] * n6 - bc[0] * n4,
                1.5 * (d[1] * n5 - d[0] * n4),
                -n3 + e[0] * n4 + e[1] * n5,
                -bc[0] * n4 - bc[1] * n5,
            ];
            (hx, hy)
        };
        let (hx_xi, hy_xi) = h(&n_xi);
        let (hx_eta, hy_eta) = h(&n_eta);
        let (x31, x12, y31, y12) = (x(2) - x(0), x(0) - x(1), y(2) - y(0), y(0) - y(1));
        let two_a = x31 * y12 - x12 * y31;
        let mut out = [[0.0; 9]; 3];
        for q in 0..9 {
            out[0][q] = (y31 * hx_xi[q] + y12 * hx_eta[q]) / two_a;
            out[1][q] = (-x31 * hy_xi[q] - x12 * hy_eta[q]) / two_a;
            out[2][q] = (-x31 * hx_xi[q] - x12 * hx_eta[q] + y31 * hy_xi[q] + y12 * hy_eta[q]) / two_a;
        }
        out
    }

    /// The stiffness in the element's axes, 18 × 18 (u, v, w, θx, θy, θz per corner).
    pub fn k_local(&self) -> [[f64; 18]; 18] {
        let (_, xy, area) = self.frame();
        let mut k = [[0.0; 18]; 18];
        // Membrane: three points (exact for the quadratic B·B of the six-node triangle).
        let mi = |q: usize| 6 * (q / 3) + [0, 1, 5][q % 3];
        let dm = self.dm();
        for l in [[2.0 / 3.0, 1.0 / 6.0, 1.0 / 6.0], [1.0 / 6.0, 2.0 / 3.0, 1.0 / 6.0], [1.0 / 6.0, 1.0 / 6.0, 2.0 / 3.0]] {
            let b = Self::bm(&xy, area, l);
            add_btdb(&mut k, &b, &dm, area / 3.0, mi);
        }
        // The drilling penalty: the mean θz against the element's rotation ½(∂v/∂x − ∂u/∂y).
        let g = self.e / (2.0 * (1.0 + self.nu));
        let mut r = [0.0; 9];
        let bcst = |i: usize| xy[(i + 1) % 3][1] - xy[(i + 2) % 3][1];
        let ccst = |i: usize| xy[(i + 2) % 3][0] - xy[(i + 1) % 3][0];
        for i in 0..3 {
            r[3 * i] -= 0.5 * ccst(i) / (2.0 * area);
            r[3 * i + 1] += 0.5 * bcst(i) / (2.0 * area);
            r[3 * i + 2] -= 1.0 / 3.0;
        }
        let pen = 1e-3 * g * self.t * area;
        for p in 0..9 {
            for q in 0..9 {
                k[mi(p)][mi(q)] += pen * r[p] * r[q];
            }
        }
        // Bending: three points inside.
        let bi = |q: usize| 6 * (q / 3) + 2 + q % 3;
        let db = self.db();
        for (s, t) in [(1.0 / 6.0, 1.0 / 6.0), (2.0 / 3.0, 1.0 / 6.0), (1.0 / 6.0, 2.0 / 3.0)] {
            let b = Self::bb(&xy, s, t);
            add_btdb(&mut k, &b, &db, area / 3.0, bi);
        }
        k
    }

    /// The rotation from global to the element's axes for all 18 unknowns, as 3 × 3 blocks.
    pub fn k_global(&self) -> [[f64; 18]; 18] {
        let (r, _, _) = self.frame();
        let kl = self.k_local();
        // K_g = Tᵀ K_l T with T block-diagonal R (rows the axes).
        let mut kt = [[0.0; 18]; 18];
        for i in 0..18 {
            for j in 0..18 {
                let (bj, cj) = (j / 3, j % 3);
                kt[i][j] = (0..3).map(|a| kl[i][3 * bj + a] * r[a][cj]).sum();
            }
        }
        let mut kg = [[0.0; 18]; 18];
        for i in 0..18 {
            let (bi, ci) = (i / 3, i % 3);
            for j in 0..18 {
                kg[i][j] = (0..3).map(|a| r[a][ci] * kt[3 * bi + a][j]).sum();
            }
        }
        kg
    }

    /// Membrane forces (Nx, Ny, Nxy, per length) and moments (Mx, My, Mxy) at the centroid, in
    /// the element's axes, from its 18 global movements.
    pub fn forces(&self, ug: &[f64; 18]) -> ([f64; 3], [f64; 3]) {
        let (r, xy, area) = self.frame();
        let mut ul = [0.0; 18];
        for b in 0..6 {
            for a in 0..3 {
                ul[3 * b + a] = (0..3).map(|c| r[a][c] * ug[3 * b + c]).sum();
            }
        }
        let um: [f64; 9] = std::array::from_fn(|q| ul[6 * (q / 3) + [0, 1, 5][q % 3]]);
        let ub: [f64; 9] = std::array::from_fn(|q| ul[6 * (q / 3) + 2 + q % 3]);
        let bm = Self::bm(&xy, area, [1.0 / 3.0; 3]);
        let eps: [f64; 3] = std::array::from_fn(|i| (0..9).map(|q| bm[i][q] * um[q]).sum());
        let mut kap = [0.0; 3];
        for (s, t) in [(1.0 / 6.0, 1.0 / 6.0), (2.0 / 3.0, 1.0 / 6.0), (1.0 / 6.0, 2.0 / 3.0)] {
            let b = Self::bb(&xy, s, t);
            for i in 0..3 {
                kap[i] += (0..9).map(|q| b[i][q] * ub[q]).sum::<f64>() / 3.0;
            }
        }
        let (dm, db) = (self.dm(), self.db());
        let n = std::array::from_fn(|i| (0..3).map(|j| dm[i][j] * eps[j]).sum());
        let m = std::array::from_fn(|i| (0..3).map(|j| db[i][j] * kap[j]).sum());
        (n, m)
    }
}

/// K += w Bᵀ D B, B's columns placed by `at`.
fn add_btdb(k: &mut [[f64; 18]; 18], b: &[[f64; 9]; 3], d: &[[f64; 3]; 3], w: f64, at: impl Fn(usize) -> usize) {
    let mut db = [[0.0; 9]; 3];
    for i in 0..3 {
        for q in 0..9 {
            db[i][q] = (0..3).map(|j| d[i][j] * b[j][q]).sum();
        }
    }
    for p in 0..9 {
        for q in 0..9 {
            let v: f64 = (0..3).map(|i| b[i][p] * db[i][q]).sum();
            k[at(p)][at(q)] += w * v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> Tri {
        Tri { p: [[0.1, 0.2, 0.0], [1.3, 0.1, 0.0], [0.4, 0.9, 0.0]], t: 0.02, e: 200e9, nu: 0.3 }
    }
    fn energy(k: &[[f64; 18]; 18], u: &[f64; 18]) -> f64 {
        (0..18).map(|i| (0..18).map(|j| u[i] * k[i][j] * u[j]).sum::<f64>()).sum::<f64>() / 2.0
    }
    /// Global movements from a field: (u, v, w, θx, θy, θz) at each corner.
    fn field(t: &Tri, f: impl Fn([f64; 3]) -> [f64; 6]) -> [f64; 18] {
        let mut u = [0.0; 18];
        for i in 0..3 {
            let v = f(t.p[i]);
            u[6 * i..6 * i + 6].copy_from_slice(&v);
        }
        u
    }

    #[test]
    fn rigid_movements_take_no_energy() {
        for t in [tri(), Tri { p: [[0.0, 0.0, 0.0], [1.0, 0.5, 0.3], [0.2, 1.0, -0.4]], ..tri() }] {
            let k = t.k_global();
            let big = (0..18).map(|i| k[i][i].abs()).fold(0.0, f64::max);
            for (name, f) in [
                ("x", Box::new(|_p: [f64; 3]| [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]) as Box<dyn Fn([f64; 3]) -> [f64; 6]>),
                ("y", Box::new(|_p| [0.0, 1.0, 0.0, 0.0, 0.0, 0.0])),
                ("z", Box::new(|_p| [0.0, 0.0, 1.0, 0.0, 0.0, 0.0])),
                // Rotation ω about x: movement ω × r = (0, −z, y).
                ("rx", Box::new(|p: [f64; 3]| [0.0, -p[2], p[1], 1.0, 0.0, 0.0])),
                ("ry", Box::new(|p: [f64; 3]| [p[2], 0.0, -p[0], 0.0, 1.0, 0.0])),
                ("rz", Box::new(|p: [f64; 3]| [-p[1], p[0], 0.0, 0.0, 0.0, 1.0])),
            ] {
                let u = field(&t, f);
                let w = energy(&k, &u);
                assert!(w.abs() < 1e-9 * big, "{name}: {w:e}");
            }
        }
    }

    #[test]
    fn constant_strain_and_curvature_give_the_exact_energy() {
        let t = tri();
        let (_, _, area) = t.frame();
        let k = t.k_global();
        // Membrane: u = εx·x (plus θz = 0: no rotation).
        let ex = 1e-3;
        let u = field(&t, |p| [ex * p[0], 0.0, 0.0, 0.0, 0.0, 0.0]);
        let exact = 0.5 * t.e * t.t / (1.0 - t.nu * t.nu) * ex * ex * area;
        assert!((energy(&k, &u) - exact).abs() < 1e-9 * exact, "{} {exact}", energy(&k, &u));
        // Shear: u = γ y / 2, v = γ x / 2 (no rotation).
        let g = 1e-3;
        let u = field(&t, |p| [g * p[1] / 2.0, g * p[0] / 2.0, 0.0, 0.0, 0.0, 0.0]);
        let exact = 0.5 * t.e / (2.0 * (1.0 + t.nu)) * t.t * g * g * area;
        assert!((energy(&k, &u) - exact).abs() < 1e-9 * exact, "{} {exact}", energy(&k, &u));
        // Bending: w = κ x²/2, so θy = −∂w/∂x = −κx; w = κ y²/2, θx = κy; w = κxy (twist).
        let kp = 1e-2;
        let d = t.e * t.t.powi(3) / (12.0 * (1.0 - t.nu * t.nu));
        for (name, f, exact) in [
            ("xx", Box::new(move |p: [f64; 3]| [0.0, 0.0, kp * p[0] * p[0] / 2.0, 0.0, -kp * p[0], 0.0]) as Box<dyn Fn([f64; 3]) -> [f64; 6]>, 0.5 * d * kp * kp * area),
            ("yy", Box::new(move |p: [f64; 3]| [0.0, 0.0, kp * p[1] * p[1] / 2.0, kp * p[1], 0.0, 0.0]), 0.5 * d * kp * kp * area),
            ("xy", Box::new(move |p: [f64; 3]| [0.0, 0.0, kp * p[0] * p[1], kp * p[0], -kp * p[1], 0.0]), 0.5 * d * (1.0 - t.nu) / 2.0 * (2.0 * kp) * (2.0 * kp) * area),
        ] {
            let u = field(&t, f);
            assert!((energy(&k, &u) - exact).abs() < 1e-9 * exact, "{name}: {} {exact}", energy(&k, &u));
        }
    }

    #[test]
    fn forces_of_a_constant_state() {
        let t = tri();
        let kp = 1e-2;
        let u = field(&t, |p| [0.0, 0.0, kp * p[0] * p[0] / 2.0, 0.0, -kp * p[0], 0.0]);
        let (_, m) = t.forces(&u);
        let d = t.e * t.t.powi(3) / (12.0 * (1.0 - t.nu * t.nu));
        // In the element's axes (x along side 1–2, here nearly global x): the moment tensor of
        // κxx = −w,xx … turned. Check its invariants: trace and determinant.
        let (mx, my, mxy) = (m[0], m[1], m[2]);
        let tr = mx + my;
        assert!((tr.abs() - d * kp * (1.0 + t.nu)).abs() < 1e-9 * d * kp, "{m:?}");
        let det = mx * my - mxy * mxy;
        assert!((det - d * d * kp * kp * t.nu).abs() < 1e-9 * d * d * kp * kp, "{m:?}");
    }
}

/// A mesh of shell triangles solved on its own (for the benchmarks): node positions, triangles
/// with their section, held unknowns (6 per node) and nodal loads. Returns the movements.
pub fn solve_mesh(nodes: &[[f64; 3]], tris: &[([usize; 3], f64, f64, f64)], held: &[bool], f: &[f64]) -> Result<Vec<f64>, String> {
    let n = nodes.len();
    let mut sys = crate::sparse::System::new(6 * n);
    for (c, t, e, nu) in tris {
        let tri = Tri { p: [nodes[c[0]], nodes[c[1]], nodes[c[2]]], t: *t, e: *e, nu: *nu };
        let k = tri.k_global();
        for i in 0..18 {
            for j in 0..18 {
                sys.add(6 * c[i / 6] + i % 6, 6 * c[j / 6] + j % 6, k[i][j]);
            }
        }
    }
    sys.f = f.to_vec();
    Ok(sys.solve(held, &vec![0.0; 6 * n])?.0)
}

/// Shell benchmarks on structured meshes (benchmark 22).
pub(crate) mod cases {
    use super::*;

    /// An n × n grid of a × b, two triangles per square (alternating diagonals), z = 0.
    pub(crate) fn grid(nx: usize, ny: usize, a: f64, b: f64) -> (Vec<[f64; 3]>, Vec<[usize; 3]>) {
        let mut p = vec![];
        for j in 0..=ny {
            for i in 0..=nx {
                p.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64, 0.0]);
            }
        }
        let id = |i: usize, j: usize| j * (nx + 1) + i;
        let mut t = vec![];
        for j in 0..ny {
            for i in 0..nx {
                let (p00, p10, p01, p11) = (id(i, j), id(i + 1, j), id(i, j + 1), id(i + 1, j + 1));
                if (i + j) % 2 == 0 {
                    t.push([p00, p10, p11]);
                    t.push([p00, p11, p01]);
                } else {
                    t.push([p00, p10, p01]);
                    t.push([p10, p11, p01]);
                }
            }
        }
        (p, t)
    }

    /// Square plate a = 1 on its edges (simply supported or clamped) under uniform pressure: the
    /// middle's deflection over qa⁴/D.
    pub fn plate(n: usize, clamped: bool) -> f64 {
        // A square plate a = 1, t = 0.01, E = 200 GPa, ν = 0.3, q = 1 kPa, on its edges.
        let (a, t, e, nu, q) = (1.0, 0.01, 200e9, 0.3, 1e3);
        let (p, tr) = grid(n, n, a, a);
        let tris: Vec<_> = tr.iter().map(|c| (*c, t, e, nu)).collect();
        let mut held = vec![false; 6 * p.len()];
        let mut f = vec![0.0; 6 * p.len()];
        for (i, x) in p.iter().enumerate() {
            let edge = x[0] < 1e-9 || x[0] > a - 1e-9 || x[1] < 1e-9 || x[1] > a - 1e-9;
            if edge {
                held[6 * i + 2] = true;
                if clamped {
                    held[6 * i + 3] = true;
                    held[6 * i + 4] = true;
                }
            }
            // In-plane: nothing loads it; hold u, v, θz everywhere so it is not loose.
            held[6 * i] = true;
            held[6 * i + 1] = true;
            held[6 * i + 5] = true;
        }
        for c in &tr {
            let (_, _, area) = Tri { p: [p[c[0]], p[c[1]], p[c[2]]], t, e, nu }.frame();
            for k in c {
                f[6 * k + 2] -= q * area / 3.0;
            }
        }
        let u = solve_mesh(&p, &tris, &held, &f).unwrap();
        let mid = p.iter().position(|x| (x[0] - 0.5).abs() < 1e-9 && (x[1] - 0.5).abs() < 1e-9).unwrap();
        let d = e * t.powi(3) / (12.0 * (1.0 - nu * nu));
        -u[6 * mid + 2] / (q * a.powi(4) / d)
    }

    /// Scordelis–Lo roof: a cylindrical shell R = 25, L = 50, 80° arc, t = 0.25, E = 4.32e8,
    /// ν = 0, its own weight 90 per area; ends on diaphragms (uy = uz = 0 there... here x along the
    /// axis), sides free. A quarter by symmetry. Reference: vertical movement at the middle of the
    /// free edge 0.3024 (MacNeal and Harder 1985).
    pub fn scordelis(n: usize) -> f64 {
        let (r, l, t, e, nu, q) = (25.0f64, 50.0, 0.25, 4.32e8, 0.0, 90.0);
        let th = 40f64.to_radians();
        // Quarter: x from 0 (diaphragm) to L/2 (symmetry), angle from 0 (crown, symmetry) to 40° (free edge).
        let mut p = vec![];
        for j in 0..=n {
            for i in 0..=n {
                let (x, a) = (l / 2.0 * i as f64 / n as f64, th * j as f64 / n as f64);
                p.push([x, r * a.cos(), r * a.sin()]);
            }
        }
        let (_, tr) = grid(n, n, 1.0, 1.0);
        let tris: Vec<_> = tr.iter().map(|c| (*c, t, e, nu)).collect();
        let mut held = vec![false; 6 * p.len()];
        let mut f = vec![0.0; 6 * p.len()];
        for (k, x) in p.iter().enumerate() {
            let (i, j) = (k % (n + 1), k / (n + 1));
            if i == 0 {
                // Diaphragm: no movement in its plane (y, z), free along x; θx held (rigid in its plane).
                held[6 * k + 1] = true;
                held[6 * k + 2] = true;
                held[6 * k + 3] = true;
            }
            if i == n {
                // Symmetry across x = L/2: ux = 0, θy = θz = 0.
                held[6 * k] = true;
                held[6 * k + 4] = true;
                held[6 * k + 5] = true;
            }
            if j == 0 {
                // Symmetry across the crown plane z = 0: uz = 0, θx = θy = 0.
                held[6 * k + 2] = true;
                held[6 * k + 3] = true;
                held[6 * k + 4] = true;
            }
            let _ = x;
        }
        for c in &tr {
            let (_, _, area) = Tri { p: [p[c[0]], p[c[1]], p[c[2]]], t, e, nu }.frame();
            for k in c {
                f[6 * k + 1] -= q * area / 3.0;
            }
        }
        let u = solve_mesh(&p, &tris, &held, &f).unwrap();
        // Middle of the free edge: i = n (x = L/2), j = n (40°).
        -u[6 * ((n + 1) * (n + 1) - 1) + 1]
    }

    /// Pinched cylinder with diaphragms: R = 300, L = 600, t = 3, E = 3e6, ν = 0.3, two loads
    /// P = 1 pinching it at the middle. An eighth by symmetry. Reference: movement under the load
    /// 1.8248e-5 (MacNeal and Harder 1985).
    pub fn pinched(n: usize) -> f64 {
        let (r, l, t, e, nu, pl) = (300.0f64, 600.0, 3.0, 3e6, 0.3, 1.0);
        let mut p = vec![];
        for j in 0..=n {
            for i in 0..=n {
                let (x, a) = (l / 2.0 * i as f64 / n as f64, std::f64::consts::FRAC_PI_2 * j as f64 / n as f64);
                p.push([x, r * a.cos(), r * a.sin()]);
            }
        }
        let (_, tr) = grid(n, n, 1.0, 1.0);
        let tris: Vec<_> = tr.iter().map(|c| (*c, t, e, nu)).collect();
        let mut held = vec![false; 6 * p.len()];
        let mut f = vec![0.0; 6 * p.len()];
        for k in 0..p.len() {
            let (i, j) = (k % (n + 1), k / (n + 1));
            if i == 0 {
                held[6 * k + 1] = true;
                held[6 * k + 2] = true;
                held[6 * k + 3] = true;
            }
            if i == n {
                held[6 * k] = true;
                held[6 * k + 4] = true;
                held[6 * k + 5] = true;
            }
            if j == 0 {
                // Plane z = 0.
                held[6 * k + 2] = true;
                held[6 * k + 3] = true;
                held[6 * k + 4] = true;
            }
            if j == n {
                // Plane y = 0.
                held[6 * k + 1] = true;
                held[6 * k + 3] = true;
                held[6 * k + 5] = true;
            }
        }
        // The load at x = L/2, the crown (j = 0), a quarter of P on the eighth.
        let at = n;
        f[6 * at + 1] = -pl / 4.0;
        let u = solve_mesh(&p, &tris, &held, &f).unwrap();
        -u[6 * at + 1]
    }

    /// In-plane pure bending of a 10 × 1 panel (E = 1, ν = 0.25, t = 1): the right end loaded by
    /// σx = −M y/I, the left end held along x (and one point across): plane-stress elasticity gives
    /// the axis' end deflection M L²/2EI exactly. Returns it over that.
    pub fn bending(n: usize) -> f64 {
        let (nx, ny) = (10 * n, n);
        let (p, tr) = grid(nx, ny, 10.0, 1.0);
        let tris: Vec<_> = tr.iter().map(|c| (*c, 1.0, 1.0, 0.25)).collect();
        let mut held = vec![false; 6 * p.len()];
        let mut f = vec![0.0; 6 * p.len()];
        let (mm, i) = (1.0, 1.0 / 12.0);
        for (k, x) in p.iter().enumerate() {
            for d in [2, 3, 4] {
                held[6 * k + d] = true;
            }
            let j = k / (nx + 1);
            if x[0] < 1e-9 {
                held[6 * k] = true;
                if j == ny / 2 {
                    held[6 * k + 1] = true;
                }
            }
            if x[0] > 10.0 - 1e-9 {
                // σx = −M (y − ½)/I along the edge, shared to the nodes (linear: exact by thirds).
                let h = 1.0 / ny as f64;
                let s = |y: f64| -mm * (y - 0.5) / i;
                let y = x[1];
                let mut fx = 0.0;
                if j > 0 {
                    fx += h * (2.0 * s(y) + s(y - h)) / 6.0;
                }
                if j < ny {
                    fx += h * (2.0 * s(y) + s(y + h)) / 6.0;
                }
                f[6 * k] = fx;
            }
        }
        let u = solve_mesh(&p, &tris, &held, &f).unwrap();
        let tip = p.iter().position(|x| x[0] > 10.0 - 1e-9 && (x[1] - 0.5).abs() < 1e-9).unwrap();
        u[6 * tip + 1] / (mm * 100.0 / (2.0 * i))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn the_shell_cases_converge() {
            let (a, b) = (plate(8, false), plate(16, false));
            assert!((b - 0.004062).abs() < (a - 0.004062).abs() && (b - 0.004062).abs() < 0.005 * 0.004062, "{a} {b}");
            for n in [2, 4, 8] {
                println!("bending {n}: {}", bending(n));
            }
        }
    }
}
