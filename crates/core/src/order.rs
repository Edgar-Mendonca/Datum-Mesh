//! The sparse Cholesky factor every solve uses (0.3.5), with the fill-reducing ordering chosen by
//! size: faer's approximate minimum degree for small and mid-size systems, and nested dissection
//! for large ones, where minimum degree fills in badly in 3D. The nested dissection works on the
//! matrix's own graph (no coordinates needed): a breadth-first search from a far node splits each
//! part into levels, the middle level is the separator, numbered last, and both sides are split
//! again until they are small.

use faer::dyn_stack::{MemBuffer, MemStack};
use faer::linalg::solvers::Solve;
use faer::perm::Perm;
use faer::sparse::linalg::cholesky::{factorize_symbolic_cholesky, LltRef, SymbolicCholesky, SymmetricOrdering};
use faer::sparse::SparseColMat;
use faer::{Conj, Mat, Side};

/// 3D solids with more unknowns than this are ordered by nested dissection (in long chains of
/// frame members it orders worse than minimum degree).
pub const ND_ABOVE: usize = 20_000;
/// Parts this small are not split further.
const LEAF: usize = 120;

pub(crate) enum Factor {
    Amd(faer::sparse::linalg::solvers::Llt<usize, f64>),
    Nd(Box<SymbolicCholesky<usize>>, Vec<f64>),
}

impl Factor {
    /// Factor a symmetric positive definite matrix given by its lower triangle.
    pub fn new(a: &SparseColMat<usize, f64>, nd: bool) -> Result<Factor, ()> {
        if !nd || a.nrows() <= ND_ABOVE {
            return a.sp_cholesky(Side::Lower).map(Factor::Amd).map_err(|_| ());
        }
        let (fwd, inv) = dissect(a);
        let perm = Perm::new_checked(fwd.into_boxed_slice(), inv.into_boxed_slice(), a.nrows());
        let sym = factorize_symbolic_cholesky(a.symbolic(), Side::Lower, SymmetricOrdering::Custom(perm.as_ref()), Default::default()).map_err(|_| ())?;
        let mut vals = vec![0.0; sym.len_val()];
        let par = faer::get_global_parallelism();
        let mut mem = MemBuffer::new(sym.factorize_numeric_llt_scratch::<f64>(par, Default::default()));
        sym.factorize_numeric_llt(&mut vals, a.as_ref(), Side::Lower, Default::default(), par, MemStack::new(&mut mem), Default::default()).map_err(|_| ())?;
        Ok(Factor::Nd(Box::new(sym), vals))
    }
    /// Solve for each column of `b`.
    pub fn solve(&self, b: &Mat<f64>) -> Mat<f64> {
        match self {
            Factor::Amd(l) => l.solve(b),
            Factor::Nd(sym, vals) => {
                let mut x = b.clone();
                let par = faer::get_global_parallelism();
                let mut mem = MemBuffer::new(sym.solve_in_place_scratch::<f64>(b.ncols(), par));
                LltRef::new(sym, vals).solve_in_place_with_conj(Conj::No, x.as_mut(), par, MemStack::new(&mut mem));
                x
            }
        }
    }
}

/// A nested dissection ordering of the matrix's graph: (new → old, old → new).
fn dissect(a: &SparseColMat<usize, f64>) -> (Vec<usize>, Vec<usize>) {
    let n = a.nrows();
    // The graph, both ways, from the stored lower triangle.
    let mut deg = vec![0usize; n];
    let cp = a.symbolic().col_ptr();
    let ri = a.symbolic().row_idx();
    for c in 0..n {
        for &r in &ri[cp[c]..cp[c + 1]] {
            if r != c {
                deg[r] += 1;
                deg[c] += 1;
            }
        }
    }
    let mut start = vec![0usize; n + 1];
    for i in 0..n {
        start[i + 1] = start[i] + deg[i];
    }
    let mut fill = start.clone();
    let mut adj = vec![0usize; start[n]];
    for c in 0..n {
        for &r in &ri[cp[c]..cp[c + 1]] {
            if r != c {
                adj[fill[r]] = c;
                fill[r] += 1;
                adj[fill[c]] = r;
                fill[c] += 1;
            }
        }
    }
    let nbrs = |v: usize| &adj[start[v]..start[v + 1]];
    // Which part each node is in now; a part is split by breadth-first levels.
    let mut part = vec![0u32; n];
    let mut next_part = 1u32;
    let mut order: Vec<usize> = Vec::with_capacity(n);
    enum Task {
        Split(Vec<usize>, u32),
        Emit(Vec<usize>),
    }
    let mut stack = vec![Task::Split((0..n).collect(), 0)];
    let mut level = vec![usize::MAX; n];
    let bfs = |from: usize, p: u32, part: &[u32], level: &mut [usize], seen: &mut Vec<usize>| {
        for &v in seen.iter() {
            level[v] = usize::MAX;
        }
        seen.clear();
        level[from] = 0;
        seen.push(from);
        let mut head = 0;
        while head < seen.len() {
            let v = seen[head];
            head += 1;
            for &w in nbrs(v) {
                if part[w] == p && level[w] == usize::MAX {
                    level[w] = level[v] + 1;
                    seen.push(w);
                }
            }
        }
    };
    let mut seen: Vec<usize> = vec![];
    while let Some(t) = stack.pop() {
        match t {
            Task::Emit(v) => order.extend(v),
            Task::Split(nodes, p) => {
                if nodes.len() <= LEAF {
                    order.extend(nodes);
                    continue;
                }
                // One connected piece at a time.
                bfs(nodes[0], p, &part, &mut level, &mut seen);
                if seen.len() < nodes.len() {
                    let rest: Vec<usize> = nodes.iter().copied().filter(|v| level[*v] == usize::MAX).collect();
                    let piece = std::mem::take(&mut seen);
                    let (q1, q2) = (next_part, next_part + 1);
                    next_part += 2;
                    for &v in &piece {
                        part[v] = q1;
                    }
                    for &v in &rest {
                        part[v] = q2;
                    }
                    for &v in &piece {
                        level[v] = usize::MAX;
                    }
                    stack.push(Task::Split(rest, q2));
                    stack.push(Task::Split(piece, q1));
                    continue;
                }
                // From a far node (twice, to find a near-peripheral one), split at the middle level.
                let far = *seen.last().unwrap();
                bfs(far, p, &part, &mut level, &mut seen);
                let total = seen.len();
                let mut count_at = vec![0usize; level[*seen.last().unwrap()] + 1];
                for &v in &seen {
                    count_at[level[v]] += 1;
                }
                // The middle level: half of the part before it.
                let mut acc = 0;
                let mut mid = 0;
                for (l, c) in count_at.iter().enumerate() {
                    acc += c;
                    if acc * 2 >= total {
                        mid = l;
                        break;
                    }
                }
                if mid == 0 || mid + 1 >= count_at.len() {
                    // A part with no middle to cut (a clique, a chain end): keep it whole.
                    order.extend(seen.iter().copied());
                    continue;
                }
                let (mut a1, mut b1, mut sep) = (vec![], vec![], vec![]);
                for &v in &seen {
                    match level[v].cmp(&mid) {
                        std::cmp::Ordering::Less => a1.push(v),
                        std::cmp::Ordering::Greater => b1.push(v),
                        // A middle node with nothing beyond it needs not separate: it joins the
                        // near side (the level is thinned to the nodes that touch both sides).
                        std::cmp::Ordering::Equal if nbrs(v).iter().any(|&w| part[w] == p && level[w] == mid + 1) => sep.push(v),
                        std::cmp::Ordering::Equal => a1.push(v),
                    }
                }
                let (qa, qb, qs) = (next_part, next_part + 1, next_part + 2);
                next_part += 3;
                for &v in &a1 {
                    part[v] = qa;
                }
                for &v in &b1 {
                    part[v] = qb;
                }
                for &v in &sep {
                    part[v] = qs;
                }
                stack.push(Task::Emit(sep));
                stack.push(Task::Split(b1, qb));
                stack.push(Task::Split(a1, qa));
            }
        }
    }
    let mut inv = vec![0usize; n];
    for (new, &old) in order.iter().enumerate() {
        inv[old] = new;
    }
    (order, inv)
}
