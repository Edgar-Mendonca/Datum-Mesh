//! Steady heat conduction on the triangle mesh: 3- and 6-node triangles, fixed temperatures on
//! edges and points, heat flux and convection to a fluid on edges, heat generated in the body;
//! temperatures, heat flux recovered to the nodes, and the heat balance (what flows in through
//! fluxes, convection and generation equals what leaves through the fixed temperatures).

use crate::mesh::{self, Mesh};
use crate::model::{Analysis, LoadKind, Model, SupportKind, Target};
use crate::plane::{edge_geo, gauss, shape, Field, NODES_REF, G3};
use crate::sparse::{Stats, System};
use serde::Serialize;

/// Heat flowing into the body at one support (a fixed temperature) or one load, in W.
#[derive(Clone, Debug, Serialize)]
pub struct Flow {
    pub id: u32,
    pub q: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeatResults {
    pub mesh: Mesh,
    #[serde(skip_serializing)]
    pub draw: Vec<[u32; 3]>,
    /// t (K), qx, qy, qmag (W/m²).
    pub fields: Vec<Field>,
    /// Heat flux per element, from the element's own values: qx, qy, qmag.
    pub elem: Vec<Field>,
    /// Heat flowing into the body through each fixed temperature (negative: leaving).
    pub supports: Vec<Flow>,
    /// Heat flowing into the body through each flux, convection and generation load.
    pub loads: Vec<Flow>,
    /// The heat balance: |Σ flows| over the sum of their sizes (0 when nothing flows).
    pub imbalance: f64,
    pub stats: Stats,
    pub warnings: Vec<String>,
}

impl HeatResults {
    pub fn field(&self, k: &str) -> &Field {
        self.fields.iter().find(|f| f.key == k).expect("field")
    }
}

/// Gradients of the shape functions in x, y at a point, and det J.
fn grad(xy: &[[f64; 2]], order: u8, xi: f64, eta: f64) -> (Vec<[f64; 2]>, Vec<f64>, f64) {
    let (n, d) = shape(order, xi, eta);
    let (mut j11, mut j12, mut j21, mut j22) = (0.0, 0.0, 0.0, 0.0);
    for (k, p) in xy.iter().enumerate() {
        j11 += d[k][0] * p[0];
        j12 += d[k][0] * p[1];
        j21 += d[k][1] * p[0];
        j22 += d[k][1] * p[1];
    }
    let det = j11 * j22 - j12 * j21;
    let g = (0..xy.len()).map(|k| [(j22 * d[k][0] - j12 * d[k][1]) / det, (-j21 * d[k][0] + j11 * d[k][1]) / det]).collect();
    (g, n, det)
}

/// Mesh and solve a heat conduction model.
pub fn solve(m: &Model) -> Result<HeatResults, String> {
    if m.analysis != Analysis::Heat {
        return Err("This solves heat conduction models.".into());
    }
    let has_t = m.supports.iter().any(|s| matches!(s.kind, SupportKind::Temperature { .. }));
    let has_conv = m.loads.iter().any(|l| matches!(l.kind, LoadKind::Convection { h, .. } if h > 0.0));
    if !has_t && !has_conv {
        return Err("Fix a temperature on an edge or point, or add convection to a fluid: without one the temperatures have no level to settle at (Loads tab).".into());
    }
    let ms = mesh::mesh(m)?;
    solve_mesh(m, ms)
}

pub fn solve_mesh(m: &Model, ms: Mesh) -> Result<HeatResults, String> {
    let t = m.thickness;
    let nn = ms.nodes.len();
    let ord = ms.order;
    let mut sys = System::new(nn);
    let mut warnings = vec![];
    let mats: std::collections::HashMap<u32, &crate::model::Material> = m.materials.iter().map(|x| (x.id, x)).collect();
    // What each load puts in, by load: generation and flux are known before the solve; convection
    // is h (T∞ − T) and is summed after it.
    let mut fload: Vec<(u32, Vec<f64>)> = vec![];
    let mut conv: Vec<(u32, Vec<(usize, usize, f64)>, Vec<f64>)> = vec![];
    let gen: Vec<(u32, f64)> = m.loads.iter().filter_map(|l| if let (LoadKind::Heat { q }, Target::Body) = (&l.kind, &l.at) { Some((l.id, *q)) } else { None }).collect();
    let mut fgen = vec![vec![0.0; nn]; gen.len()];
    for (k, tri) in ms.tris.iter().enumerate() {
        let mt = mats.get(&ms.mat[k]).ok_or("A shape's material is missing.")?;
        if mt.k <= 0.0 {
            return Err(format!("{}: heat conduction needs a thermal conductivity above zero (Home › Materials).", mt.name));
        }
        let xy: Vec<[f64; 2]> = tri.iter().map(|&i| ms.nodes[i as usize]).collect();
        let nd = xy.len();
        let mut ke = vec![0.0; nd * nd];
        for &(xi, eta, w) in gauss(ord) {
            let (g, n, det) = grad(&xy, ord, xi, eta);
            if det <= 0.0 {
                return Err("The mesh has a folded element; try a different element size.".into());
            }
            let f = w * det * t;
            for i in 0..nd {
                for j in 0..nd {
                    ke[i * nd + j] += mt.k * (g[i][0] * g[j][0] + g[i][1] * g[j][1]) * f;
                }
                for (gi, &(_, q)) in gen.iter().enumerate() {
                    fgen[gi][tri[i] as usize] += n[i] * q * f;
                }
            }
        }
        for i in 0..nd {
            for j in 0..nd {
                sys.add(tri[i] as usize, tri[j] as usize, ke[i * nd + j]);
            }
        }
    }
    for (gi, &(id, _)) in gen.iter().enumerate() {
        fload.push((id, std::mem::take(&mut fgen[gi])));
    }
    for l in &m.loads {
        match (&l.kind, &l.at) {
            (LoadKind::Flux { .. } | LoadKind::Convection { .. }, Target::Edge { shape, edge }) => {
                let mut f = vec![0.0; nn];
                let mut h = vec![];
                for e in ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge) {
                    let nodes: Vec<usize> = e.n.iter().filter(|n| **n != u32::MAX).map(|n| *n as usize).collect();
                    for &(s, w) in &G3 {
                        let (_, tg, nf) = edge_geo(&ms.nodes, e, s);
                        let nf: Vec<f64> = if e.n[1] == u32::MAX { vec![nf[0], nf[2]] } else { nf.to_vec() };
                        let jl = tg[0].hypot(tg[1]) * w * t;
                        match l.kind {
                            LoadKind::Flux { q } => {
                                for (a, &na) in nodes.iter().enumerate() {
                                    f[na] += nf[a] * q * jl;
                                }
                            }
                            LoadKind::Convection { h: hc, t_inf } => {
                                for (a, &na) in nodes.iter().enumerate() {
                                    f[na] += nf[a] * hc * t_inf * jl;
                                    for (b, &nb) in nodes.iter().enumerate() {
                                        h.push((na, nb, nf[a] * nf[b] * hc * jl));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if let LoadKind::Convection { .. } = l.kind {
                    for &(a, b, v) in &h {
                        sys.add(a, b, v);
                    }
                    conv.push((l.id, h, f.clone()));
                } else {
                    fload.push((l.id, f.clone()));
                }
                for i in 0..nn {
                    sys.f[i] += f[i];
                }
            }
            (LoadKind::Heat { .. }, Target::Body) => {}
            _ => warnings.push("A load that does not act in heat conduction (forces, pressures, gravity) was left out.".into()),
        }
    }
    // Generation (the first loads in `fload`) goes in now; fluxes and convection went in above.
    for (gi, _) in gen.iter().enumerate() {
        let f = &fload[gi].1;
        for i in 0..nn {
            sys.f[i] += f[i];
        }
    }
    // Fixed temperatures.
    let mut held = vec![false; nn];
    let mut fixed = vec![0.0; nn];
    let mut sup_nodes: Vec<(u32, Vec<usize>)> = vec![];
    let point_node = |x: f64, y: f64| ms.points.iter().find(|p| p.0 == [x, y]).map(|p| p.1 as usize);
    for s in &m.supports {
        let SupportKind::Temperature { t: tv } = s.kind else {
            warnings.push("A support that holds movement does not act in heat conduction; it was left out.".into());
            continue;
        };
        let nodes: Vec<usize> = match &s.at {
            Target::Edge { shape, edge } => ms.edges.iter().filter(|e| e.shape == *shape && e.edge == *edge).flat_map(|e| e.n.iter().filter(|n| **n != u32::MAX).map(|n| *n as usize).collect::<Vec<_>>()).collect(),
            Target::Point { x, y } => point_node(*x, *y).into_iter().collect(),
            _ => vec![],
        };
        let mut own = vec![];
        for n in nodes {
            if !held[n] {
                held[n] = true;
                fixed[n] = tv;
                own.push(n);
            }
        }
        sup_nodes.push((s.id, own));
    }
    let (u, stats) = sys.solve(&held, &fixed)?;
    // Heat into the body at fixed temperatures: K T − f there.
    let ku = sys.mul(&u);
    let supports: Vec<Flow> = sup_nodes.iter().map(|(id, ns)| Flow { id: *id, q: ns.iter().map(|&n| ku[n] - sys.f[n]).sum::<f64>() }).collect();
    let mut loads: Vec<Flow> = fload.iter().map(|(id, f)| Flow { id: *id, q: f.iter().sum() }).collect();
    for (id, h, f) in &conv {
        let mut q: f64 = f.iter().sum();
        for &(_, b, v) in h {
            q -= v * u[b];
        }
        loads.push(Flow { id: *id, q });
    }
    let total: f64 = supports.iter().chain(loads.iter()).map(|f| f.q).sum();
    let size: f64 = supports.iter().chain(loads.iter()).map(|f| f.q.abs()).sum();
    let imbalance = if size > 0.0 { total.abs() / size } else { 0.0 };
    // Heat flux q = −k ∇T at each element's nodes, averaged by area.
    let mut acc = vec![[0.0f64; 2]; nn];
    let mut wsum = vec![0.0f64; nn];
    let ne = ms.tris.len();
    let (mut eq, mut centres) = (vec![[0.0f64; 2]; ne], Vec::with_capacity(ne));
    for (k, tri) in ms.tris.iter().enumerate() {
        let kk = mats[&ms.mat[k]].k;
        let xy: Vec<[f64; 2]> = tri.iter().map(|&i| ms.nodes[i as usize]).collect();
        let (_, _, det0) = grad(&xy, ord, 1.0 / 3.0, 1.0 / 3.0);
        let area = det0.abs() / 2.0;
        for (q, &(xi, eta)) in NODES_REF.iter().take(tri.len()).enumerate() {
            let (g, _, _) = grad(&xy, ord, xi, eta);
            let (mut gx, mut gy) = (0.0, 0.0);
            for (i, gi) in g.iter().enumerate() {
                gx += gi[0] * u[tri[i] as usize];
                gy += gi[1] * u[tri[i] as usize];
            }
            let n = tri[q] as usize;
            acc[n][0] -= kk * gx * area;
            acc[n][1] -= kk * gy * area;
            wsum[n] += area;
            eq[k][0] -= kk * gx / tri.len() as f64;
            eq[k][1] -= kk * gy / tri.len() as f64;
        }
        centres.push([(xy[0][0] + xy[1][0] + xy[2][0]) / 3.0, (xy[0][1] + xy[1][1] + xy[2][1]) / 3.0]);
    }
    let elem = vec![
        crate::plane::field("qx", eq.iter().map(|q| q[0]).collect(), &centres),
        crate::plane::field("qy", eq.iter().map(|q| q[1]).collect(), &centres),
        crate::plane::field("qmag", eq.iter().map(|q| q[0].hypot(q[1])).collect(), &centres),
    ];
    let mut cols: Vec<(&'static str, Vec<f64>)> = ["t", "qx", "qy", "qmag"].iter().map(|k| (*k, Vec::with_capacity(nn))).collect();
    for n in 0..nn {
        let w = wsum[n].max(1e-300);
        let (qx, qy) = (acc[n][0] / w, acc[n][1] / w);
        for (k, v) in [u[n], qx, qy, qx.hypot(qy)].iter().enumerate() {
            cols[k].1.push(*v);
        }
    }
    let fields = cols.into_iter().map(|(key, v)| crate::plane::field(key, v, &ms.nodes)).collect();
    let draw = crate::plane::draw_tris(&ms);
    if size == 0.0 {
        warnings.push("No heat flows: every temperature is the same. Add a heat flux, convection to a fluid at another temperature, or a second fixed temperature.".into());
    }
    Ok(HeatResults { mesh: ms, draw, fields, elem, supports, loads, imbalance, stats, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Geom, Load, Shape, Support};

    /// A bar 0.1 × 0.02 m: 100 °C at the left end, a load on the right end.
    fn bar(order: u8, right: LoadKind) -> Doc {
        let mut d = Doc::new(Model::new(Analysis::Heat));
        d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.1, 0.02), ..Shape::default() }) }).unwrap();
        let s = d.m.shapes[0].id;
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: SupportKind::Temperature { t: 373.15 } }) }).unwrap();
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: right }) }).unwrap();
        d.m.mesh.order = order;
        d.m.mesh.size = 0.007;
        d
    }

    #[test]
    fn a_linear_temperature_is_reproduced_exactly() {
        // Patch test: a flux in at the right end gives T = T0 + q x / k along the bar, exactly.
        for order in [1, 2] {
            let d = bar(order, LoadKind::Flux { q: 5e4 });
            let r = solve(&d.m).unwrap();
            let k = d.m.materials[0].k;
            for (n, p) in r.mesh.nodes.iter().enumerate() {
                let exact = 373.15 + 5e4 * p[0] / k;
                assert!((r.field("t").v[n] - exact).abs() < 1e-9 * exact, "T{}: {} vs {exact}", order * 3, r.field("t").v[n]);
            }
            assert!((r.field("qx").min + 5e4).abs() < 1e-6 * 5e4 && (r.field("qx").max + 5e4).abs() < 1e-6 * 5e4);
            assert!(r.imbalance < 1e-9, "{}", r.imbalance);
        }
    }

    #[test]
    fn a_wall_cooled_by_convection() {
        // q = (T0 − T∞) / (L/k + 1/h); the cooled face sits at T∞ + q/h.
        let (h, tinf, l) = (500.0, 293.15, 0.1);
        let d = bar(2, LoadKind::Convection { h, t_inf: tinf });
        let r = solve(&d.m).unwrap();
        let k = d.m.materials[0].k;
        let q = (373.15 - tinf) / (l / k + 1.0 / h);
        let tr = tinf + q / h;
        let face = r.mesh.nodes.iter().enumerate().filter(|(_, p)| (p[0] - l).abs() < 1e-12).map(|(n, _)| r.field("t").v[n]).collect::<Vec<_>>();
        assert!(face.iter().all(|t| (t - tr).abs() < 1e-9 * tr), "{face:?} vs {tr}");
        // Heat in at the hot end equals heat out to the fluid: q × area of the end.
        let qin = r.supports[0].q;
        assert!((qin - q * 0.02 * d.m.thickness).abs() < 1e-9 * qin, "{qin}");
        assert!(r.imbalance < 1e-9);
    }

    #[test]
    fn no_fixed_temperature_and_no_fluid_is_explained() {
        let mut d = bar(1, LoadKind::Flux { q: 1.0 });
        d.m.supports.clear();
        assert!(solve(&d.m).unwrap_err().contains("Fix a temperature"));
    }
}
