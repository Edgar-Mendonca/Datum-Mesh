//! The model check (0.10.3): what would spoil a frame's solve or its answer, found before solving,
//! each with the items to go to and why it matters. It only reads the model.

use crate::model::{dist3, Model, Target};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Finding {
    /// "error" stops a solve or makes it wrong; "warning" is likely a mistake; "note" is worth a look.
    pub level: &'static str,
    pub what: String,
    /// The items it is about (nodes, members), for the window to select.
    pub ids: Vec<u32>,
    pub why: String,
}

fn f(level: &'static str, what: impl Into<String>, ids: Vec<u32>, why: impl Into<String>) -> Finding {
    Finding { level, what: what.into(), ids, why: why.into() }
}

/// Check a frame model; `near` is the distance (m) under which two nodes look like one.
pub fn model_check(m: &Model, near: f64) -> Vec<Finding> {
    let mut out = vec![];
    let near = if near.is_finite() && near > 0.0 { near } else { 0.005 };
    // Items are named as the window's tree names them: by their place in the list.
    let node = |id: u32| format!("Node {}", m.nodes.iter().position(|n| n.id == id).map_or(0, |k| k + 1));
    let member = |id: u32| format!("Member {}", m.members.iter().position(|b| b.id == id).map_or(0, |k| k + 1));
    let used: HashSet<u32> = m.members.iter().flat_map(|b| [b.a, b.b]).collect();
    // Nodes nothing joins.
    for n in &m.nodes {
        if !used.contains(&n.id) {
            out.push(f("warning", format!("Loose node: {}", node(n.id)), vec![n.id], "No member joins it: a load or support on it does nothing, and it may be a click that missed."));
        }
    }
    // Nodes almost on top of each other: members meant to meet that do not.
    let mut by_cell: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    let cell = |p: [f64; 3]| ((p[0] / near).floor() as i64, (p[1] / near).floor() as i64, (p[2] / near).floor() as i64);
    for (i, n) in m.nodes.iter().enumerate() {
        by_cell.entry(cell(n.at())).or_default().push(i);
    }
    let mut seen = HashSet::new();
    for (i, n) in m.nodes.iter().enumerate() {
        let (cx, cy, cz) = cell(n.at());
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &j in by_cell.get(&(cx + dx, cy + dy, cz + dz)).into_iter().flatten() {
                        let o = &m.nodes[j];
                        let d = dist3(n.at(), o.at());
                        if j > i && d < near && seen.insert((n.id, o.id)) {
                            out.push(f("warning", format!("{} and {} nearly meet", node(n.id), node(o.id)), vec![n.id, o.id], format!("They are {:.1} mm apart, so the members on them are not joined. Merge them (Geometry › Merge Nodes) if they should be one.", d * 1e3)));
                        }
                    }
                }
            }
        }
    }
    // Members on the same two nodes.
    let mut pairs: HashMap<(u32, u32), u32> = HashMap::new();
    for b in &m.members {
        if let Some(o) = pairs.insert((b.a.min(b.b), b.a.max(b.b)), b.id) {
            out.push(f("error", format!("{} and {} overlap", member(o), member(b.id)), vec![o, b.id], "Two members join the same two nodes: the stiffness counts twice. Delete one."));
        }
    }
    // A beam hinged at both ends that meets only hinged or truss ends at a node: free to spin there.
    for b in &m.members {
        if b.hinge == [true, true] {
            out.push(f("note", format!("{} is hinged at both ends", member(b.id)), vec![b.id], "It carries no moment at its ends, like a truss bar, but still bends under loads along it. Make it a truss bar if it only carries axial force."));
        }
    }
    // Supports, loads and pieces that nothing holds.
    if !m.members.is_empty() && m.supports.is_empty() {
        out.push(f("error", "No supports", vec![], "Nothing holds the model, so it can slide away: add supports on the Loads tab."));
    }
    if !m.members.is_empty() && !m.loads.iter().any(|l| !matches!(l.kind, crate::model::LoadKind::Mass { .. })) {
        out.push(f("note", "No loads", vec![], "A static solve will find nothing moving; frequencies and buckling still work."));
    }
    // Groups of members not joined to each other; each needs supports of its own.
    let mut parent: HashMap<u32, u32> = m.nodes.iter().map(|n| (n.id, n.id)).collect();
    fn root(p: &mut HashMap<u32, u32>, x: u32) -> u32 {
        let mut r = x;
        while p.get(&r).is_some_and(|q| *q != r) {
            r = p[&r];
        }
        p.insert(x, r);
        r
    }
    for b in &m.members {
        let (ra, rb) = (root(&mut parent, b.a), root(&mut parent, b.b));
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let mut groups: HashMap<u32, Vec<u32>> = HashMap::new();
    for b in &m.members {
        let r = root(&mut parent, b.a);
        groups.entry(r).or_default().push(b.id);
    }
    if groups.len() > 1 && !m.supports.is_empty() {
        let held: HashSet<u32> = m.supports.iter().filter_map(|s| if let Target::Node { id } = s.at { Some(id) } else { None }).collect();
        for ids in groups.values() {
            let on_held = m.members.iter().filter(|b| ids.contains(&b.id)).any(|b| held.contains(&b.a) || held.contains(&b.b));
            if !on_held {
                out.push(f("error", if ids.len() == 1 { format!("{} is not joined to the rest", member(ids[0])) } else { format!("{} members are not joined to the rest", ids.len()) }, ids.clone(), "This group touches no support and no other member: it would fly off. Join it or support it."));
            }
        }
    }
    // Design members (0.10.4): each chain found again, its restraints on it.
    for dm in &m.design_members {
        match crate::frames::design::chain(m, dm.a, dm.b) {
            Err(e) => out.push(f("error", format!("Design member {} is broken", dm.name), vec![dm.id], e)),
            Ok(c) => {
                if dm.restraints.iter().any(|r| !(r.at >= 0.0 && r.at <= c.length)) {
                    out.push(f("warning", format!("Design member {} has a restraint off its length", dm.name), vec![dm.id], format!("Restraints are measured from its start, 0 to {:.3} m.", c.length)));
                }
            }
        }
    }
    // Very short members: often a slip of the mouse.
    let span = m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(0.0);
    for b in &m.members {
        if let Some(l) = m.member_length(b.id) {
            if span > 0.0 && l < 1e-3 * span {
                out.push(f("warning", format!("{} is very short", member(b.id)), vec![b.id], format!("It is {:.1} mm long against a model {:.1} m across: probably not meant.", l * 1e3, span)));
            }
        }
    }
    let rank = |l: &str| match l {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    };
    out.sort_by_key(|x| rank(x.level));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, MemberKind, Node, Support, SupportKind};

    fn member(d: &mut Doc, a: [f64; 2], b: [f64; 2]) {
        d.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    }

    #[test]
    fn a_clean_frame_has_nothing_to_report_and_each_fault_is_found() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        member(&mut d, [0.0, 0.0], [0.0, 3.0]);
        member(&mut d, [0.0, 3.0], [4.0, 3.0]);
        let n0 = d.m.nodes[0].id;
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: n0 }, kind: SupportKind::Fixed }) }).unwrap();
        let found = model_check(&d.m, 0.005);
        assert!(found.iter().all(|x| x.level == "note"), "{found:?}");
        // A loose node, a near miss, a group on its own.
        d.apply(Op::Add { item: Item::Node(Node { id: 0, x: 9.0, y: 9.0, z: 0.0 }) }).unwrap();
        member(&mut d, [4.003, 3.0], [6.0, 3.0]);
        let found = model_check(&d.m, 0.005);
        let has = |s: &str| found.iter().any(|x| x.what.contains(s));
        assert!(has("Loose node") && has("nearly meet") && has("not joined to the rest"), "{found:?}");
        assert_eq!(found[0].level, "error");
        // Merging the near nodes clears both the near miss and the loose group.
        d.apply(Op::MergeNodes { ids: vec![], tol: 0.005 }).unwrap();
        let found = model_check(&d.m, 0.005);
        assert!(!found.iter().any(|x| x.what.contains("nearly meet") || x.what.contains("not joined")), "{found:?}");
    }
}
