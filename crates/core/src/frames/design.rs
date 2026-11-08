//! Design members (0.10.4): a real beam or column made of a chain of analysis members, and the
//! intervals its restraints divide it into. Kept by its two end nodes, so the chain is found
//! again along the line after any edit: cutting a member or adding nodes never breaks it, and
//! never changes its segments (the specification's F07). It only reads the model.

use crate::model::{dist3, DesignMember, Model};
use serde::Serialize;

/// A design member's chain: the analysis members from end `a` to end `b`, each with the
/// distance along the design member where it starts and its length, and whether it runs a → b.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Chain {
    pub length: f64,
    pub members: Vec<Link>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Link {
    pub member: u32,
    pub start: f64,
    pub length: f64,
    pub forward: bool,
}

/// One interval between restraints (or ends), and why it ends where it does.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Segment {
    pub from: f64,
    pub to: f64,
    pub length: f64,
    pub why: String,
}

fn dir(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = dist3(a, b).max(1e-300);
    [d[0] / l, d[1] / l, d[2] / l]
}

/// Walk from end `a` to end `b` along members in one straight line, alike in kind, section and
/// material; or say why it cannot.
pub fn chain(m: &Model, a: u32, b: u32) -> Result<Chain, String> {
    let at = |id: u32| m.node(id).map(|n| n.at()).ok_or("A design member's end node is missing.".to_string());
    let (pa, pb) = (at(a)?, at(b)?);
    let total = dist3(pa, pb);
    if total <= 0.0 {
        return Err("A design member's ends are at one place.".into());
    }
    let u = dir(pa, pb);
    let mut out = vec![];
    let (mut cur, mut s) = (a, 0.0);
    let mut first: Option<(crate::model::MemberKind, u32, u32)> = None;
    while cur != b {
        let pc = at(cur)?;
        let next = m.members.iter().find(|mb| {
            let other = if mb.a == cur { mb.b } else if mb.b == cur { mb.a } else { return false };
            m.node(other).is_some_and(|n| { let d = dir(pc, n.at()); d[0] * u[0] + d[1] * u[1] + d[2] * u[2] > 1.0 - 1e-9 })
        });
        let Some(mb) = next else {
            return Err(format!("The chain stops at node {}: no member carries on along the line from there.", m.nodes.iter().position(|n| n.id == cur).map_or(0, |k| k + 1)));
        };
        let kind = (mb.kind, mb.section, mb.material);
        match first {
            None => first = Some(kind),
            Some(f) if f != kind => return Err(format!("Member {} is not like the rest of the chain (kind, section or material): a design member here is one uniform piece.", m.members.iter().position(|x| x.id == mb.id).map_or(0, |k| k + 1))),
            _ => {}
        }
        let forward = mb.a == cur;
        let other = if forward { mb.b } else { mb.a };
        let len = dist3(pc, at(other)?);
        out.push(Link { member: mb.id, start: s, length: len, forward });
        s += len;
        cur = other;
        if s > total * (1.0 + 1e-9) {
            return Err("The chain runs past the design member's far end.".into());
        }
    }
    Ok(Chain { length: total, members: out })
}

/// What the window shows of a design member: its chain and segments, or why it is broken.
#[derive(Clone, Debug, Serialize)]
pub struct Info {
    pub chain: Option<Chain>,
    pub segments: Vec<Segment>,
    pub broken: Option<String>,
}

pub fn info(m: &Model, id: u32) -> Result<Info, String> {
    let dm = m.design_members.iter().find(|x| x.id == id).ok_or("That design member does not exist.")?;
    Ok(match chain(m, dm.a, dm.b) {
        Ok(c) => Info { segments: segments(dm, c.length), chain: Some(c), broken: None },
        Err(e) => Info { chain: None, segments: vec![], broken: Some(e) },
    })
}

/// Form a design member from selected members: the chain's two far ends, checked.
pub fn form(m: &Model, ids: &[u32]) -> Result<(u32, u32), String> {
    let picked: Vec<&crate::model::Member> = m.members.iter().filter(|b| ids.contains(&b.id)).collect();
    if picked.is_empty() {
        return Err("Pick the members of the design member, end to end.".into());
    }
    // The ends: nodes used once among the picked members.
    let mut count: std::collections::HashMap<u32, usize> = Default::default();
    for b in &picked {
        *count.entry(b.a).or_default() += 1;
        *count.entry(b.b).or_default() += 1;
    }
    if count.values().any(|c| *c > 2) {
        return Err("The picked members branch: a design member is one straight run.".into());
    }
    let mut ends: Vec<u32> = count.iter().filter(|(_, c)| **c == 1).map(|(n, _)| *n).collect();
    ends.sort_by_key(|n| m.nodes.iter().position(|x| x.id == *n));
    let [a, b] = ends[..] else {
        return Err("The picked members do not make one run with two ends: pick members that follow on from each other.".into());
    };
    let c = chain(m, a, b).map_err(|e| if e.contains("not like the rest") { e } else { format!("The picked members are not one straight run from end to end. {e}") })?;
    if c.members.len() != picked.len() || !c.members.iter().all(|l| ids.contains(&l.member)) {
        return Err("The picked members are not all in one straight line: a design member is one straight run.".into());
    }
    Ok((a, b))
}

/// The intervals between the design member's ends and its restraints, in order.
pub fn segments(dm: &DesignMember, length: f64) -> Vec<Segment> {
    let mut at: Vec<(f64, String)> = dm.restraints.iter().filter(|r| r.at > 1e-9 && r.at < length - 1e-9).map(|r| (r.at, format!("restraint ({})", r.holds.words()))).collect();
    at.sort_by(|x, y| x.0.total_cmp(&y.0));
    at.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-9);
    at.push((length, "end".into()));
    let mut from = 0.0;
    at.into_iter().map(|(to, why)| { let s = Segment { from, to, length: to - from, why }; from = to; s }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Item, Op};
    use crate::model::{Analysis, Holds, MemberKind, Restraint};

    fn beam(d: &mut Doc, a: [f64; 2], b: [f64; 2]) {
        d.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    }

    #[test]
    fn segments_follow_restraints_not_the_mesh() {
        // F07: a 6 m design member from four analysis members, restrained at 3 m.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        for k in 0..4 {
            beam(&mut d, [1.5 * k as f64, 0.0], [1.5 * (k + 1) as f64, 0.0]);
        }
        let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
        let made = d.apply(Op::FormDesignMember { ids: ids.clone(), name: "DM1".into() }).unwrap();
        let id = made[0];
        let dm = d.m.design_members.iter().find(|x| x.id == id).unwrap().clone();
        let c = chain(&d.m, dm.a, dm.b).unwrap();
        assert!((c.length - 6.0).abs() < 1e-12 && c.members.len() == 4);
        let mut dm2 = dm.clone();
        dm2.restraints = vec![Restraint { at: 3.0, holds: Holds::Both }];
        d.apply(Op::Set { item: Item::DesignMember(dm2) }).unwrap();
        let seg = |d: &Doc| { let dm = &d.m.design_members[0]; segments(dm, chain(&d.m, dm.a, dm.b).unwrap().length).iter().map(|s| (s.from, s.to)).collect::<Vec<_>>() };
        assert_eq!(seg(&d), vec![(0.0, 3.0), (3.0, 6.0)]);
        // More analysis nodes: the same segments; the chain finds the new pieces.
        d.apply(Op::SplitMember { ids: ids.clone(), at: vec![0.5] }).unwrap();
        assert_eq!(seg(&d), vec![(0.0, 3.0), (3.0, 6.0)]);
        assert_eq!(chain(&d.m, d.m.design_members[0].a, d.m.design_members[0].b).unwrap().members.len(), 8);
        // Without the restraint: one interval 0–6 m.
        let mut dm3 = d.m.design_members[0].clone();
        dm3.restraints.clear();
        d.apply(Op::Set { item: Item::DesignMember(dm3) }).unwrap();
        assert_eq!(seg(&d), vec![(0.0, 6.0)]);
    }

    #[test]
    fn a_design_member_is_one_straight_uniform_run() {
        // F12: unlike sections, a bend, a branch and a gap are refused, each saying why.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        beam(&mut d, [0.0, 0.0], [3.0, 0.0]);
        beam(&mut d, [3.0, 0.0], [6.0, 0.0]);
        beam(&mut d, [6.0, 0.0], [8.0, 2.0]);
        let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
        assert!(d.apply(Op::FormDesignMember { ids: ids.clone(), name: String::new() }).unwrap_err().contains("straight"));
        let mut other = d.m.sections[0].clone();
        other.id = 0;
        other.name = "Other".into();
        let sid = d.apply(Op::Add { item: Item::Section(other) }).unwrap()[0];
        let mut mb = d.m.members[1].clone();
        mb.section = sid;
        d.apply(Op::Set { item: Item::Member(mb) }).unwrap();
        assert!(d.apply(Op::FormDesignMember { ids: ids[..2].to_vec(), name: String::new() }).unwrap_err().contains("not like the rest"));
        let mut d = Doc::new(Model::new(Analysis::Frame));
        beam(&mut d, [0.0, 0.0], [3.0, 0.0]);
        beam(&mut d, [4.0, 0.0], [6.0, 0.0]);
        let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
        assert!(d.apply(Op::FormDesignMember { ids, name: String::new() }).is_err());
        // Deleting a member of it leaves the design member broken: the model check says so.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        beam(&mut d, [0.0, 0.0], [3.0, 0.0]);
        beam(&mut d, [3.0, 0.0], [6.0, 0.0]);
        let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
        d.apply(Op::FormDesignMember { ids: ids.clone(), name: String::new() }).unwrap();
        d.apply(Op::Delete { ids: vec![ids[1]] }).unwrap();
        let found = crate::frames::check::model_check(&d.m, 0.005);
        assert!(found.iter().any(|f| f.what.contains("Design member") && f.level == "error"), "{found:?}");
    }
}
