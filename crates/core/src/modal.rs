//! Natural frequencies (0.2.2) and buckling (0.2.3): the lowest eigenvalues of K φ = ω² M φ by subspace iteration with
//! one sparse Cholesky factorisation of K (faer), and a small dense eigenproblem on the subspace
//! each step (Cholesky and Jacobi rotations). Mass matrices come from each analysis (frame.rs,
//! plane.rs); this file only does the numbers.

use crate::sparse::System;
use faer::sparse::{SparseColMat, Triplet};
use faer::Mat;
use serde::Serialize;

/// One mode: its frequency and its shape over every unknown (held ones zero), scaled so the
/// largest movement is 1.
#[derive(Clone, Debug, Serialize)]
pub struct Mode {
    /// Hz.
    pub f: f64,
    /// ω² (rad²/s²), the eigenvalue.
    pub lambda: f64,
    pub shape: Vec<f64>,
}

/// The lowest `count` modes of K φ = λ M φ, with the unknowns in `held` fixed at zero. `m` holds
/// the mass entries as (row, col, value) triplets, both halves of the symmetric matrix.
pub fn lowest(k: &System, m: &[(usize, usize, f64)], held: &[bool], count: usize) -> Result<(Vec<Mode>, usize), String> {
    let (found, iters) = subspace(k, m, held, count, Kind::Vibration)?;
    let modes = found.into_iter().map(|(lambda, shape)| Mode { f: lambda.max(0.0).sqrt() / std::f64::consts::TAU, lambda, shape }).collect();
    Ok((modes, iters))
}

/// The lowest positive load factors λ of K φ = λ (−K_G) φ (0.2.3): `g` holds the geometric
/// stiffness K_G of the loads as applied (both halves), which may be of either sign. Returns
/// (λ, shape) pairs, lowest first, the shapes scaled so the largest movement is 1.
pub fn buckling(k: &System, g: &[(usize, usize, f64)], held: &[bool], count: usize) -> Result<(Vec<(f64, Vec<f64>)>, usize), String> {
    let neg: Vec<(usize, usize, f64)> = g.iter().map(|&(i, j, v)| (i, j, -v)).collect();
    subspace(k, &neg, held, count, Kind::Buckling)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// K φ = λ M φ with M positive: the lowest λ.
    Vibration,
    /// K φ = λ B φ with B of either sign: the lowest positive λ, found as the largest μ = 1/λ of
    /// B φ = μ K φ (K is positive, so the small problem stays well posed).
    Buckling,
}

/// Subspace iteration with one sparse Cholesky factorisation of K: X ← K⁻¹ B X, then
/// Rayleigh–Ritz on the subspace. Returns (λ, shape) pairs and the iterations taken.
fn subspace(k: &System, m: &[(usize, usize, f64)], held: &[bool], count: usize, kind: Kind) -> Result<(Vec<(f64, Vec<f64>)>, usize), String> {
    let n = k.n;
    let mut map = vec![usize::MAX; n];
    let mut nf = 0;
    for i in 0..n {
        if !held[i] {
            map[i] = nf;
            nf += 1;
        }
    }
    if nf == 0 {
        return Err(match kind {
            Kind::Vibration => "Everything is held: nothing can vibrate.",
            Kind::Buckling => "Everything is held: nothing can buckle.",
        }
        .into());
    }
    let mut tri = vec![];
    let mut diag = vec![0.0f64; nf];
    for &(i, j, v) in &k.k {
        let (a, b) = (map[i], map[j]);
        if a != usize::MAX && b != usize::MAX {
            if a >= b {
                tri.push(Triplet::new(a, b, v));
            }
            if a == b {
                diag[a] += v;
            }
        }
    }
    let unsupported = match kind {
        Kind::Vibration => "Natural frequencies need the model held so it cannot move as a whole: add supports (a free body has zero frequencies).",
        Kind::Buckling => "Buckling needs the model held so it cannot move as a whole: add supports.",
    };
    if !diag.iter().all(|d| *d > 0.0 && d.is_finite()) {
        return Err(unsupported.into());
    }
    let mr: Vec<(usize, usize, f64)> = m.iter().filter(|&&(i, j, _)| map[i] != usize::MAX && map[j] != usize::MAX).map(|&(i, j, v)| (map[i], map[j], v)).collect();
    let mut mdiag = vec![0.0f64; nf];
    for &(i, j, v) in &mr {
        if i == j {
            mdiag[i] += v;
        }
    }
    match kind {
        Kind::Vibration if !mdiag.iter().any(|v| *v > 0.0) => return Err("The model has no mass: give the materials a density (Home › Materials).".into()),
        Kind::Buckling if !mdiag.iter().any(|v| *v > 0.0) => return Err("Nothing in the model is squeezed by the loads, so it cannot buckle. Buckling needs members in compression.".into()),
        _ => {}
    }
    // Start vectors grow from where B is large against K; for buckling that is |B|.
    let mdiag: Vec<f64> = mdiag.iter().map(|v| v.abs()).collect();
    let a = SparseColMat::<usize, f64>::try_new_from_triplets(nf, nf, &tri).map_err(|e| format!("Could not build the stiffness matrix: {e:?}"))?;
    let llt = crate::order::Factor::new(&a, k.half).map_err(|_| unsupported.to_string())?;
    // Probe for a hidden zero pivot, as the static solver does.
    let mut probe = Mat::<f64>::zeros(nf, 1);
    for i in 0..nf {
        probe[(i, 0)] = diag[i];
    }
    let px = llt.solve(&probe);
    if (0..nf).any(|i| !(px[(i, 0)].abs() < 1e15)) {
        return Err(unsupported.into());
    }
    let mmul = |x: &Mat<f64>| -> Mat<f64> {
        let mut y = Mat::<f64>::zeros(nf, x.ncols());
        for &(i, j, v) in &mr {
            for c in 0..x.ncols() {
                y[(i, c)] += v * x[(j, c)];
            }
        }
        y
    };
    let count = count.min(nf);
    let p = match kind {
        Kind::Vibration => (2 * count).max(count + 8),
        // Negative μ (buckling under the loads reversed) compete for the subspace too.
        Kind::Buckling => (3 * count).max(count + 12),
    }
    .min(nf);
    // Start: the B diagonal, then unit pushes where B is large against K, then a fixed
    // pseudo-random mix (so every mode has something to grow from).
    let mut x = Mat::<f64>::zeros(nf, p);
    let mut order: Vec<usize> = (0..nf).collect();
    order.sort_by(|&i, &j| (mdiag[j] / diag[j]).total_cmp(&(mdiag[i] / diag[i])));
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for c in 0..p {
        for i in 0..nf {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            x[(i, c)] = if c == 0 { mdiag[i] } else { ((seed % 2001) as f64 / 1000.0 - 1.0) * 1e-3 * mdiag[i] };
        }
        if c > 0 && c - 1 < order.len() {
            x[(order[c - 1], c)] += mdiag[order[c - 1]].max(1e-30);
        }
    }
    let mut lam = vec![0.0; p];
    let mut iters = 0;
    for it in 0..300 {
        iters = it + 1;
        let mx = mmul(&x);
        let y = llt.solve(&mx);
        // Rayleigh–Ritz: Kr = Yᵀ K Y = Yᵀ B X, Br = Yᵀ B Y.
        let my = mmul(&y);
        let mut kr = vec![vec![0.0; p]; p];
        let mut mrr = vec![vec![0.0; p]; p];
        for a in 0..p {
            for b in 0..p {
                let (mut s1, mut s2) = (0.0, 0.0);
                for i in 0..nf {
                    s1 += y[(i, a)] * mx[(i, b)];
                    s2 += y[(i, a)] * my[(i, b)];
                }
                kr[a][b] = s1;
                mrr[a][b] = s2;
            }
        }
        for a in 0..p {
            for b in 0..a {
                let (u, v) = ((kr[a][b] + kr[b][a]) / 2.0, (mrr[a][b] + mrr[b][a]) / 2.0);
                kr[a][b] = u;
                kr[b][a] = u;
                mrr[a][b] = v;
                mrr[b][a] = v;
            }
        }
        // The values, in the order wanted (lowest λ first), and their vectors in the same order.
        let (l2, q) = match kind {
            Kind::Vibration => general_eigen(&kr, &mrr)?,
            Kind::Buckling => {
                let (mu, q) = general_eigen(&mrr, &kr)?;
                let order: Vec<usize> = (0..p).rev().collect();
                (order.iter().map(|&c| if mu[c] > 0.0 { 1.0 / mu[c] } else { f64::INFINITY }).collect(), q.iter().map(|row| order.iter().map(|&c| row[c]).collect()).collect())
            }
        };
        let mut xn = Mat::<f64>::zeros(nf, p);
        for c in 0..p {
            for k2 in 0..p {
                let w = q[k2][c];
                if w != 0.0 {
                    for i in 0..nf {
                        xn[(i, c)] += y[(i, k2)] * w;
                    }
                }
            }
        }
        let done = it > 1 && (0..count).all(|i| l2[i] == lam[i] || (l2[i] - lam[i]).abs() <= 1e-12 * l2[i].abs().max(1e-300));
        lam = l2;
        x = xn;
        if done {
            break;
        }
    }
    let found = (0..count)
        .filter(|&c| lam[c].is_finite())
        .map(|c| {
            let mut shape = vec![0.0; n];
            for i in 0..n {
                if map[i] != usize::MAX {
                    shape[i] = x[(map[i], c)];
                }
            }
            let big = shape.iter().fold(0.0f64, |a, v| if v.abs() > a.abs() { *v } else { a });
            if big != 0.0 {
                shape.iter_mut().for_each(|v| *v /= big);
            }
            (lam[c], shape)
        })
        .collect();
    Ok((found, iters))
}

/// The eigenvalues (ascending) and vectors of A z = λ B z for small dense symmetric A and positive
/// definite B: B = L Lᵀ, then L⁻¹ A L⁻ᵀ by Jacobi rotations; the vectors are in the columns.
fn general_eigen(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<(Vec<f64>, Vec<Vec<f64>>), String> {
    let p = a.len();
    let mut l = vec![vec![0.0; p]; p];
    for i in 0..p {
        for j in 0..=i {
            let s: f64 = b[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>();
            if i == j {
                if s <= 0.0 {
                    return Err("The solve lost its way (a matrix that should be positive is not); try a finer mesh or check the densities.".into());
                }
                l[i][i] = s.sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    // C = L⁻¹ A L⁻ᵀ: solve L W = A, then L Cᵀ = Wᵀ.
    let fwd = |l: &[Vec<f64>], rhs: &[f64]| { let mut x = vec![0.0; p]; for i in 0..p { x[i] = (rhs[i] - (0..i).map(|k| l[i][k] * x[k]).sum::<f64>()) / l[i][i]; } x };
    let w: Vec<Vec<f64>> = (0..p).map(|c| fwd(&l, &(0..p).map(|r| a[r][c]).collect::<Vec<_>>())).collect(); // columns of L⁻¹A
    let mut c = vec![vec![0.0; p]; p];
    for r in 0..p {
        let row: Vec<f64> = (0..p).map(|k| w[k][r]).collect(); // row r of L⁻¹A
        let col = fwd(&l, &row);
        c[r][..p].copy_from_slice(&col[..p]);
    }
    for i in 0..p {
        for j in 0..i {
            let v = (c[i][j] + c[j][i]) / 2.0;
            c[i][j] = v;
            c[j][i] = v;
        }
    }
    // Jacobi.
    let mut v = vec![vec![0.0; p]; p];
    for i in 0..p {
        v[i][i] = 1.0;
    }
    for _ in 0..100 {
        let off: f64 = (0..p).flat_map(|i| (0..p).filter(move |j| *j != i).map(move |j| (i, j))).map(|(i, j)| c[i][j] * c[i][j]).sum();
        let tot: f64 = (0..p).map(|i| c[i][i] * c[i][i]).sum::<f64>() + off;
        if off <= 1e-30 * tot {
            break;
        }
        for i in 0..p {
            for j in i + 1..p {
                if c[i][j].abs() <= 1e-300 {
                    continue;
                }
                let th = (c[j][j] - c[i][i]) / (2.0 * c[i][j]);
                let t = th.signum() / (th.abs() + (th * th + 1.0).sqrt());
                let t = if th == 0.0 { 1.0 } else { t };
                let (cs, sn) = (1.0 / (t * t + 1.0).sqrt(), t / (t * t + 1.0).sqrt());
                for k in 0..p {
                    let (a1, a2) = (c[k][i], c[k][j]);
                    c[k][i] = cs * a1 - sn * a2;
                    c[k][j] = sn * a1 + cs * a2;
                }
                for k in 0..p {
                    let (a1, a2) = (c[i][k], c[j][k]);
                    c[i][k] = cs * a1 - sn * a2;
                    c[j][k] = sn * a1 + cs * a2;
                }
                for k in 0..p {
                    let (a1, a2) = (v[k][i], v[k][j]);
                    v[k][i] = cs * a1 - sn * a2;
                    v[k][j] = sn * a1 + cs * a2;
                }
            }
        }
    }
    let mut idx: Vec<usize> = (0..p).collect();
    idx.sort_by(|&i, &j| c[i][i].total_cmp(&c[j][j]));
    let vals: Vec<f64> = idx.iter().map(|&i| c[i][i]).collect();
    // z = L⁻ᵀ y for each eigenvector y.
    let mut q = vec![vec![0.0; p]; p];
    for (col, &i) in idx.iter().enumerate() {
        let y: Vec<f64> = (0..p).map(|r| v[r][i]).collect();
        let mut z = vec![0.0; p];
        for r in (0..p).rev() {
            z[r] = (y[r] - (r + 1..p).map(|k| l[k][r] * z[k]).sum::<f64>()) / l[r][r];
        }
        for r in 0..p {
            q[r][col] = z[r];
        }
    }
    Ok((vals, q))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_of_masses_and_springs() {
        // n equal masses on n equal springs, the first spring to the ground, the last mass free:
        // ω² = (4k/m) sin²((2j − 1)π / (2(2n + 1))), j = 1..n.
        let (n, k, ms) = (12, 1000.0, 2.0);
        let mut s = System::new(n + 1);
        for i in 0..n {
            s.add(i, i, k);
            s.add(i + 1, i + 1, k);
            s.add(i, i + 1, -k);
            s.add(i + 1, i, -k);
        }
        let mass: Vec<(usize, usize, f64)> = (1..=n).map(|i| (i, i, ms)).collect();
        let mut held = vec![false; n + 1];
        held[0] = true;
        let (modes, _) = lowest(&s, &mass, &held, 4).unwrap();
        for (j, md) in modes.iter().enumerate() {
            let x = (2 * j + 1) as f64 * std::f64::consts::PI / (2.0 * (2 * n + 1) as f64);
            let exact = 4.0 * k / ms * x.sin().powi(2);
            assert!((md.lambda - exact).abs() < 1e-9 * exact, "mode {}: {} vs {exact}", j + 1, md.lambda);
            assert!((md.f - exact.sqrt() / std::f64::consts::TAU).abs() < 1e-9 * md.f);
        }
        // Nothing held: refused with a reason.
        assert!(lowest(&s, &mass, &vec![false; n + 1], 2).unwrap_err().contains("supports"));
    }

    #[test]
    fn a_chain_of_springs_buckles_like_its_eigenvalues() {
        // K = tridiagonal springs, B = I: the buckling factors are K's eigenvalues; with B = −I
        // (everything in tension) there is no buckling at all.
        let (n, k) = (10, 50.0);
        let mut s = System::new(n + 1);
        for i in 0..n {
            s.add(i, i, k);
            s.add(i + 1, i + 1, k);
            s.add(i, i + 1, -k);
            s.add(i + 1, i, -k);
        }
        let mut held = vec![false; n + 1];
        held[0] = true;
        let g: Vec<(usize, usize, f64)> = (1..=n).map(|i| (i, i, -1.0)).collect();
        let (b, _) = buckling(&s, &g, &held, 3).unwrap();
        for (j, (l, _)) in b.iter().enumerate() {
            let x = (2 * j + 1) as f64 * std::f64::consts::PI / (2.0 * (2 * n + 1) as f64);
            let exact = 4.0 * k * x.sin().powi(2);
            assert!((l - exact).abs() < 1e-9 * exact, "factor {}: {l} vs {exact}", j + 1);
        }
        // Half squeezed, half stretched: only the squeezed half can buckle, still positive.
        let g: Vec<(usize, usize, f64)> = (1..=n).map(|i| (i, i, if i % 2 == 0 { -1.0 } else { 1.0 })).collect();
        let (b, _) = buckling(&s, &g, &held, 2).unwrap();
        assert!(b.iter().all(|(l, _)| *l > 0.0) && b[0].0 <= b[1].0);
        let g: Vec<(usize, usize, f64)> = (1..=n).map(|i| (i, i, 1.0)).collect();
        assert!(buckling(&s, &g, &held, 2).unwrap_err().contains("squeezed"));
    }

    #[test]
    fn the_small_eigen_solver_matches_a_known_pair() {
        // A = [[2, 1], [1, 2]], B = I: λ = 1, 3.
        let (v, q) = general_eigen(&[vec![2.0, 1.0], vec![1.0, 2.0]], &[vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
        assert!((v[0] - 1.0).abs() < 1e-14 && (v[1] - 3.0).abs() < 1e-14);
        assert!((q[0][0] + q[1][0]).abs() < 1e-14, "first vector is (1, −1)/√2");
    }
}
