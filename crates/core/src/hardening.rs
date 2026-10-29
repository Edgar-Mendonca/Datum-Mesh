//! The family hardening standard (FAMILY §11): failed edits change nothing; random models and
//! edits never panic and keep their invariants; damaged files open as far as possible or give a
//! clear error; and a speed gate on a large model.

#[cfg(test)]
mod tests {
    use crate::doc::{Doc, Item, Op, Settings};
    use crate::examples;
    use crate::model::{self, Analysis, Dir, Geom, Load, LoadKind, MemberKind, Model, Node, Shape, Support, SupportKind, Target};

    /// A small deterministic generator (no extra crate).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
        fn f(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * (self.next() % 1_000_000) as f64 / 1e6
        }
        fn id(&mut self, m: &Model) -> u32 {
            // Mostly real ids, sometimes ones that do not exist.
            if self.below(8) == 0 {
                return self.below(1000) as u32;
            }
            let all: Vec<u32> = m.nodes.iter().map(|x| x.id).chain(m.members.iter().map(|x| x.id)).chain(m.shapes.iter().map(|x| x.id)).chain(m.supports.iter().map(|x| x.id)).chain(m.loads.iter().map(|x| x.id)).collect();
            if all.is_empty() { 1 } else { all[self.below(all.len())] }
        }
    }

    fn random_op(r: &mut Rng, m: &Model) -> Op {
        let p = |r: &mut Rng| [(r.f(-5.0, 5.0) * 2.0).round() / 2.0, (r.f(-5.0, 5.0) * 2.0).round() / 2.0];
        let target = |r: &mut Rng, m: &Model| match r.below(5) {
            0 => Target::Node { id: r.id(m) },
            1 => Target::Member { id: r.id(m) },
            2 => Target::Edge { shape: r.id(m), edge: r.below(5) as u32 },
            3 => Target::Point { x: r.f(-5.0, 5.0), y: r.f(-5.0, 5.0) },
            _ => Target::Body,
        };
        match r.below(12) {
            0..=2 => Op::AddMember { from: p(r), to: p(r), kind: if r.below(2) == 0 { MemberKind::Beam } else { MemberKind::Truss }, section: None, material: None },
            3 => Op::Add { item: Item::Node(Node { id: 0, x: p(r)[0], y: p(r)[1] }) },
            4 => {
                let c = p(r);
                let g = match r.below(3) {
                    0 => Geom::Circle { c, r: r.f(-0.5, 2.0) },
                    1 => Geom::rect(c[0], c[1], c[0] + r.f(-2.0, 2.0), c[1] + r.f(-2.0, 2.0)),
                    _ => {
                        let n = 3 + r.below(5);
                        Geom::Poly { pts: (0..n).map(|_| p(r)).collect(), bulge: (0..n).map(|_| if r.below(3) == 0 { r.f(-1.2, 1.2) } else { 0.0 }).collect() }
                    }
                };
                Op::Add { item: Item::Shape(Shape { material: 1, hole: r.below(3) == 0, geom: g, ..Shape::default() }) }
            }
            5 => Op::Add { item: Item::Support(Support { id: 0, at: target(r, m), kind: [SupportKind::Fixed, SupportKind::Pinned, SupportKind::RollerX, SupportKind::Roller, SupportKind::Spring { kx: r.f(-1.0, 1e6), ky: 0.0, kr: 0.0 }, SupportKind::Temperature { t: 300.0 }][r.below(6)].clone() }) },
            6 => Op::Add { item: Item::Load(Load { id: 0, at: target(r, m), kind: [LoadKind::Force { fx: 1.0, fy: -2.0 }, LoadKind::Moment { m: 3.0 }, LoadKind::Dist { w1: 1.0, w2: 2.0, dir: Dir::Local }, LoadKind::Pressure { p: 1e5 }, LoadKind::Gravity { gx: 0.0, gy: -9.81 }, LoadKind::Convection { h: r.f(-5.0, 50.0), t_inf: 290.0 }][r.below(6)].clone() }) },
            7 => Op::Move { ids: vec![r.id(m), r.id(m)], dx: r.f(-1.0, 1.0).round(), dy: 0.5 },
            8 => Op::Delete { ids: vec![r.id(m)] },
            9 => if r.below(2) == 0 { Op::Delete { ids: vec![r.id(m)] } } else { Op::JoinCrossings { ids: m.members.iter().map(|b| b.id).collect() } },
            10 => {
                let mut n = m.nodes.first().cloned().unwrap_or(Node { id: 1, x: 0.0, y: 0.0 });
                n.x = if r.below(4) == 0 { f64::NAN } else { r.f(-3.0, 3.0) };
                Op::Set { item: Item::Node(n) }
            }
            _ => Op::SetSettings { s: Settings { analysis: [Analysis::Frame, Analysis::Stress, Analysis::Heat][r.below(3)], units: if r.below(5) == 0 { "x".into() } else { "US".into() }, grid: r.f(-0.1, 1.0), thickness: 0.01, mesh: m.mesh.clone(), t_ref: if r.below(4) == 0 { Some(r.f(250.0, 400.0)) } else { None } } },
        }
    }

    #[test]
    fn failed_edits_change_nothing() {
        let mut d = Doc::new(examples::get("portal").unwrap());
        let before = d.m.clone();
        let n = d.m.nodes[0].id;
        let bad = vec![
            Op::Add { item: Item::Node(Node { id: 0, x: 0.0, y: 0.0 }) },
            Op::Add { item: Item::Node(Node { id: 0, x: f64::NAN, y: 0.0 }) },
            Op::AddMember { from: [1.0, 1.0], to: [1.0, 1.0], kind: MemberKind::Beam, section: None, material: None },
            Op::AddMember { from: [0.0, 0.0], to: [0.0, 5.0], kind: MemberKind::Beam, section: None, material: None },
            Op::AddMember { from: [0.0, 0.0], to: [1.0, 5.0], kind: MemberKind::Beam, section: Some(999), material: None },
            Op::Move { ids: vec![999], dx: 1.0, dy: 0.0 },
            Op::Move { ids: vec![n], dx: f64::INFINITY, dy: 0.0 },
            Op::Set { item: Item::Node(Node { id: 999, x: 0.0, y: 0.0 }) },
            Op::Delete { ids: vec![1] },
            Op::Add { item: Item::Support(Support { id: 0, at: Target::Member { id: d.m.members[0].id }, kind: SupportKind::Fixed }) },
            Op::Add { item: Item::Load(Load { id: 0, at: Target::Node { id: n }, kind: LoadKind::Pressure { p: 1.0 } }) },
            Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::Poly { pts: vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]], bulge: vec![0.0; 3] }, ..Shape::default() }) },
            Op::SetMeta { name: "  ".into(), notes: String::new() },
            // A batch that fails half way must undo its first half too.
            Op::Batch(vec![Op::AddMember { from: [20.0, 0.0], to: [21.0, 0.0], kind: MemberKind::Beam, section: None, material: None }, Op::Delete { ids: vec![2] }]),
        ];
        for op in bad {
            let desc = format!("{op:?}");
            assert!(d.apply(op).is_err(), "should fail: {desc}");
            assert_eq!(d.m, before, "a failed edit changed the model: {desc}");
            assert!(!d.dirty && !d.can_undo());
        }
    }

    #[test]
    fn random_edits_never_panic_and_keep_the_model_valid() {
        for seed in 1..=40u64 {
            let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut d = Doc::new(Model::new(if seed % 2 == 0 { Analysis::Frame } else { Analysis::Stress }));
            for _ in 0..150 {
                let op = random_op(&mut r, &d.m);
                let before = d.m.clone();
                if d.apply(op).is_err() {
                    assert_eq!(d.m, before);
                }
                d.m.check().unwrap();
                if r.below(10) == 0 {
                    d.undo();
                    d.m.check().unwrap();
                }
                if r.below(15) == 0 {
                    d.redo();
                }
            }
            // And whatever we made saves and opens unchanged.
            let (back, warn) = model::from_json(&model::to_json(&d.m)).unwrap();
            assert_eq!(back, d.m, "seed {seed}");
            assert!(warn.is_empty(), "{warn:?}");
        }
    }

    #[test]
    fn damaged_files_open_as_far_as_possible_or_say_why() {
        let mut r = Rng(7);
        for ex in examples::list() {
            let text = model::to_json(&examples::get(ex.id).unwrap());
            let bytes = text.as_bytes();
            // Cut short at every few bytes: a clear error, never a panic.
            for cut in (0..bytes.len()).step_by(37) {
                let s = String::from_utf8_lossy(&bytes[..cut]);
                if let Ok((m, _)) = model::from_json(&s) {
                    m.check().unwrap();
                }
            }
            // Garbled bytes.
            for _ in 0..200 {
                let mut b = bytes.to_vec();
                for _ in 0..1 + r.below(4) {
                    let i = r.below(b.len());
                    b[i] = b"0123456789-.e,:{}[]\"aZ \n"[r.below(24)];
                }
                if let Ok((m, _)) = model::from_json(&String::from_utf8_lossy(&b)) {
                    m.check().unwrap();
                }
            }
        }
        // Hand-made damage: bad items are left out and named; the rest opens.
        let mut v: serde_json::Value = serde_json::from_str(&model::to_json(&examples::get("portal").unwrap())).unwrap();
        v["members"][0]["a"] = serde_json::json!(4242);
        v["nodes"][1] = serde_json::json!({"id": "x"});
        v["loads"][0]["kind"] = serde_json::json!({"kind": "warp", "x": 1});
        v["grid"] = serde_json::json!("big");
        v["materials"] = serde_json::json!([]);
        let (m, warn) = model::from_json(&v.to_string()).unwrap();
        m.check().unwrap();
        assert!(m.members.len() < 4 && !m.members.is_empty() && m.materials.len() == 1);
        assert!(warn.iter().any(|w| w.contains("grid")) && warn.iter().any(|w| w.contains("loads")) && warn.iter().any(|w| w.contains("nodes")), "{warn:?}");
        // Duplicate ids and a counter that is behind are repaired.
        let (m, _) = model::from_json(r#"{"nodes":[{"id":5,"x":0,"y":0},{"id":5,"x":1,"y":0}],"next":1}"#).unwrap();
        m.check().unwrap();
        assert_eq!(m.nodes.len(), 2);
        // Settings with values that cannot be read open with their defaults (0.1.1 review: a
        // negative counter or an unknown analysis refused the whole file).
        let (m, warn) = model::from_json(r#"{"analysis":"cfd","next":-4,"units":"SI","nodes":[{"id":3,"x":0,"y":0}],"thickness":-1,"grid":0}"#).unwrap_or_else(|e| panic!("{e}"));
        m.check().unwrap();
        assert_eq!(m.analysis, Analysis::Frame);
        assert!(m.next > 3 && m.nodes.len() == 1);
        assert!(warn.iter().any(|w| w.contains("analysis")) && warn.iter().any(|w| w.contains("next")), "{warn:?}");
        // A file from a newer version says so.
        assert!(model::from_json(r#"{"format": 7}"#).unwrap_err().contains("newer"));
    }

    #[test]
    fn random_frames_solve_or_say_why_and_always_balance() {
        // Random frames with random supports and loads: the solver never panics; when it answers,
        // the reactions balance the loads and every member's curve meets its nodes.
        let mut solved = 0;
        for seed in 1..=120u64 {
            let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
            let mut d = Doc::new(Model::new(Analysis::Frame));
            for _ in 0..(3 + r.below(10)) {
                let op = random_op(&mut r, &d.m);
                if matches!(op, Op::AddMember { .. } | Op::Add { item: Item::Support(_) | Item::Load(_) } | Op::Delete { .. } | Op::JoinCrossings { .. }) {
                    let _ = d.apply(op);
                }
            }
            // Give most models a fixed support so some solve.
            if let Some(n) = d.m.nodes.first().map(|n| n.id) {
                if r.below(4) > 0 {
                    let _ = d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: n }, kind: SupportKind::Fixed }) });
                }
            }
            let Ok(res) = crate::frame::solve(&d.m) else { continue };
            solved += 1;
            let scale = res.applied[0].abs().max(res.applied[1].abs()).max(1.0);
            assert!((res.applied[0] + res.reacted[0]).abs() < 1e-6 * scale && (res.applied[1] + res.reacted[1]).abs() < 1e-6 * scale, "seed {seed}: {:?} vs {:?}", res.applied, res.reacted);
            for (mr, mb) in res.members.iter().zip(&d.m.members) {
                let ia = d.m.nodes.iter().position(|n| n.id == mb.a).unwrap();
                let ib = d.m.nodes.iter().position(|n| n.id == mb.b).unwrap();
                let (da, db) = (mr.d[0], *mr.d.last().unwrap());
                let tol = 1e-9 + 1e-6 * res.disp.iter().map(|x| x[0].abs().max(x[1].abs())).fold(0.0, f64::max);
                assert!((da[0] - res.disp[ia][0]).abs() < tol && (da[1] - res.disp[ia][1]).abs() < tol && (db[0] - res.disp[ib][0]).abs() < tol && (db[1] - res.disp[ib][1]).abs() < tol, "seed {seed}: member ends do not meet their nodes");
            }
        }
        assert!(solved >= 8, "only {solved} random frames solved");
    }

    #[test]
    fn random_parts_mesh_and_solve_or_say_why_and_balance() {
        // Random plates with random holes, arcs, supports and loads, as stress, strain and heat
        // models: meshing and solving never panic; when they answer, loads and reactions (or heat
        // in and out) balance and every element is the right way round.
        let mut solved = 0;
        for seed in 1..=60u64 {
            let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xABCD);
            let analysis = [Analysis::Stress, Analysis::Strain, Analysis::Heat, Analysis::Axi][r.below(4)];
            let mut d = Doc::new(Model::new(analysis));
            let w = r.f(0.05, 2.0);
            let h = r.f(0.05, 2.0);
            let base = if r.below(3) == 0 { Geom::Circle { c: [0.0, 0.0], r: w / 2.0 } } else {
                let mut g = Geom::rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
                if let Geom::Poly { bulge, .. } = &mut g {
                    for b in bulge.iter_mut() {
                        if r.below(3) == 0 {
                            *b = r.f(-0.9, 0.9);
                        }
                    }
                }
                g
            };
            let s = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: base, ..Shape::default() }) }).unwrap()[0];
            for _ in 0..r.below(4) {
                let c = [r.f(-w / 2.0, w / 2.0), r.f(-h / 2.0, h / 2.0)];
                let _ = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, hole: true, geom: Geom::Circle { c, r: r.f(0.001, w.min(h) / 3.0) }, ..Shape::default() }) });
            }
            let edges = d.m.shape(s).unwrap().geom.edges() as u32;
            for _ in 0..1 + r.below(3) {
                let at = Target::Edge { shape: s, edge: r.below(edges as usize) as u32 };
                let kind = if analysis == Analysis::Heat { SupportKind::Temperature { t: r.f(250.0, 400.0) } } else { [SupportKind::Fixed, SupportKind::Roller, SupportKind::Symmetry, SupportKind::Pinned][r.below(4)].clone() };
                let _ = d.apply(Op::Add { item: Item::Support(Support { id: 0, at, kind }) });
            }
            for _ in 0..1 + r.below(3) {
                let at = Target::Edge { shape: s, edge: r.below(edges as usize) as u32 };
                let kind = match (analysis, r.below(4)) {
                    (Analysis::Heat, 0) => LoadKind::Flux { q: r.f(-1e4, 1e4) },
                    (Analysis::Heat, _) => LoadKind::Convection { h: r.f(0.0, 100.0), t_inf: 293.0 },
                    (_, 0) => LoadKind::Pressure { p: r.f(-1e7, 1e7) },
                    (_, 1) => LoadKind::Traction { fx: r.f(-1e4, 1e4), fy: r.f(-1e4, 1e4), total: true },
                    (_, 3) => LoadKind::Bearing { fx: r.f(-1e4, 1e4), fy: r.f(-1e4, 1e4) },
                    _ => LoadKind::Traction { fx: r.f(-1e5, 1e5), fy: 0.0, total: false },
                };
                let _ = d.apply(Op::Add { item: Item::Load(Load { id: 0, at, kind }) });
            }
            if r.below(3) == 0 {
                let _ = d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Point { x: r.f(-w, w) / 2.0, y: r.f(-h, h) / 2.0 }, kind: LoadKind::Force { fx: 1e3, fy: -1e3 } }) });
            }
            if analysis != Analysis::Heat && r.below(3) == 0 {
                // Thermal stress: a uniform change, or fixed temperatures solved first.
                let _ = d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Body, kind: LoadKind::Warming { dt: r.f(-100.0, 100.0) } }) });
                if r.below(2) == 0 {
                    let _ = d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: r.below(edges as usize) as u32 }, kind: SupportKind::Temperature { t: r.f(250.0, 500.0) } }) });
                }
            }
            if analysis == Analysis::Axi && r.below(4) > 0 {
                // Mostly to the right of the axis (sometimes touching it); now and then across it,
                // which must be refused with a reason.
                let x0 = d.m.bounds().unwrap()[0];
                let ids: Vec<u32> = d.m.shapes.iter().map(|s| s.id).collect();
                let _ = d.apply(Op::Move { ids, dx: -x0 + if r.below(2) == 0 { 0.0 } else { w * 0.1 }, dy: 0.0 });
            }
            d.m.mesh.size = w.max(h) / r.f(4.0, 30.0);
            d.m.mesh.order = 1 + r.below(2) as u8;
            let Ok(res) = crate::solve(&d.m) else { continue };
            solved += 1;
            let (imb, mesh) = match &res {
                crate::Results::Plane(p) => (p.imbalance, &p.mesh),
                crate::Results::Heat(p) => (p.imbalance, &p.mesh),
                crate::Results::Frame(_) => unreachable!(),
            };
            let barely = match &res { crate::Results::Plane(p) => p.warnings.iter().any(|w| w.contains("barely hold")), _ => false };
            if analysis == Analysis::Axi {
                assert!(d.m.bounds().unwrap()[0] >= -1e-9, "seed {seed}: an axisymmetric part across the axis was solved");
            }
            assert!(imb < 1e-6 || barely, "seed {seed}: imbalance {imb:e} without a warning");
            for t in &mesh.tris {
                let p = [mesh.nodes[t[0] as usize], mesh.nodes[t[1] as usize], mesh.nodes[t[2] as usize]];
                let a2 = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
                assert!(a2 > 0.0, "seed {seed}: a folded element");
            }
        }
        assert!(solved >= 15, "only {solved} random parts solved");
    }

    #[test]
    fn speed_gate_on_a_large_part() {
        // About 25 000 six-node triangles (50 000 nodes, 100 000 unknowns): meshed and solved.
        let mut d = Doc::new(Model::new(Analysis::Stress));
        let s = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 1.0, 1.0), ..Shape::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Shape(Shape { material: 1, hole: true, geom: Geom::Circle { c: [0.5, 0.5], r: 0.1 }, ..Shape::default() }) }).unwrap();
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: SupportKind::Fixed }) }).unwrap();
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: -1e6 } }) }).unwrap();
        d.m.mesh.size = 0.0095;
        let t = std::time::Instant::now();
        let r = crate::plane::solve(&d.m).unwrap();
        assert!(r.mesh.nodes.len() > 40_000, "{}", r.mesh.nodes.len());
        assert!(r.imbalance < 1e-9);
        let limit = if cfg!(debug_assertions) { 120.0 } else { 10.0 };
        assert!(t.elapsed().as_secs_f64() < limit, "took {:?}", t.elapsed());
    }

    #[test]
    fn speed_gate_on_a_large_frame_solve() {
        // A 60 × 60 grid of beams (7 320 members, 11 163 equations), fixed along the bottom.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let mut ops = vec![];
        for i in 0..=60 {
            for j in 0..=60 {
                let (x, y) = (i as f64, j as f64);
                if i < 60 { ops.push(Op::AddMember { from: [x, y], to: [x + 1.0, y], kind: MemberKind::Beam, section: None, material: None }); }
                if j < 60 { ops.push(Op::AddMember { from: [x, y], to: [x, y + 1.0], kind: MemberKind::Beam, section: None, material: None }); }
            }
        }
        d.apply(Op::Batch(ops)).unwrap();
        let bottom: Vec<u32> = d.m.nodes.iter().filter(|n| n.y == 0.0).map(|n| n.id).collect();
        let top = d.m.nodes.iter().find(|n| n.y == 60.0 && n.x == 60.0).unwrap().id;
        let mut ops: Vec<Op> = bottom.iter().map(|id| Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: *id }, kind: SupportKind::Fixed }) }).collect();
        ops.push(Op::Add { item: Item::Load(Load { id: 0, at: Target::Node { id: top }, kind: LoadKind::Force { fx: 1e3, fy: 0.0 } }) });
        d.apply(Op::Batch(ops)).unwrap();
        let t = std::time::Instant::now();
        let r = crate::frame::solve(&d.m).unwrap();
        assert!(r.imbalance() < 1e-9);
        let limit = if cfg!(debug_assertions) { 30.0 } else { 3.0 };
        assert!(t.elapsed().as_secs_f64() < limit, "solve took {:?}", t.elapsed());
    }

    #[test]
    fn speed_gate_on_a_large_model() {
        // 10 000 members in a grid, saved and opened, then 200 edits with undo.
        let t = std::time::Instant::now();
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let mut ops = vec![];
        for i in 0..70 {
            for j in 0..70 {
                let (x, y) = (i as f64, j as f64);
                ops.push(Op::AddMember { from: [x, y], to: [x + 1.0, y], kind: MemberKind::Beam, section: None, material: None });
                ops.push(Op::AddMember { from: [x, y], to: [x, y + 1.0], kind: MemberKind::Beam, section: None, material: None });
            }
        }
        // Nodes are found by position, so build in one batch (the slow part is the search).
        d.apply(Op::Batch(ops)).unwrap();
        assert!(d.m.members.len() >= 9800);
        let text = model::to_json(&d.m);
        let (back, _) = model::from_json(&text).unwrap();
        assert_eq!(back.members.len(), d.m.members.len());
        for k in 0..200 {
            let id = d.m.nodes[k].id;
            d.apply(Op::Move { ids: vec![id], dx: 0.0, dy: 0.25 }).unwrap();
        }
        for _ in 0..50 {
            d.undo();
        }
        let limit = if cfg!(debug_assertions) { 60.0 } else { 10.0 };
        assert!(t.elapsed().as_secs_f64() < limit, "took {:?}", t.elapsed());
    }
}
