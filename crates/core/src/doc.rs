//! The open document: the model, its file, and undo/redo. The window sends edit operations;
//! each one runs on a copy and is kept only if the result passes `Model::check`, so a failed edit
//! leaves the model exactly as it was (Plan's pattern: ops in, whole-model snapshots for undo).

use crate::model::{Analysis, Load, Material, Member, MemberKind, MeshSettings, Model, Node, Section, Shape, Support, Target};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One item of the model, for adding or replacing.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Item {
    Node(Node),
    Member(Member),
    Shape(Shape),
    Section(Section),
    Material(Material),
    Support(Support),
    Load(Load),
}

/// The model-wide settings, edited together.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub analysis: Analysis,
    pub units: String,
    pub grid: f64,
    pub thickness: f64,
    pub mesh: MeshSettings,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Op {
    /// Add an item; the model gives it a new id.
    Add { item: Item },
    /// Replace the item that has the same id.
    Set { item: Item },
    /// A member between two points: a node already there is used, else one is made.
    AddMember { from: [f64; 2], to: [f64; 2], kind: MemberKind, section: Option<u32>, material: Option<u32> },
    /// Move nodes and shapes; a member moves its two nodes.
    Move { ids: Vec<u32>, dx: f64, dy: f64 },
    /// Delete items, with what hangs on them (a node's members, an edge's loads, …). A material or
    /// section still in use is not deleted.
    Delete { ids: Vec<u32> },
    SetSettings { s: Settings },
    SetMeta { name: String, notes: String },
    Batch(Vec<Op>),
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub m: Model,
    pub path: Option<String>,
    pub dirty: bool,
    undo: Vec<Model>,
    redo: Vec<Model>,
}

const UNDO_LIMIT: usize = 200;
/// Points closer than this are the same node (m).
pub const SAME: f64 = 1e-7;

impl Doc {
    pub fn new(m: Model) -> Doc {
        Doc { m, ..Default::default() }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Apply an edit. Returns the ids it created.
    pub fn apply(&mut self, op: Op) -> Result<Vec<u32>, String> {
        let mut m = self.m.clone();
        let mut created = vec![];
        exec(&mut m, op, &mut created)?;
        prune(&mut m);
        m.check()?;
        if m == self.m {
            return Ok(created);
        }
        let old = std::mem::replace(&mut self.m, m);
        self.undo.push(old);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        Ok(created)
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(m) => {
                self.redo.push(std::mem::replace(&mut self.m, m));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(m) => {
                self.undo.push(std::mem::replace(&mut self.m, m));
                self.dirty = true;
                true
            }
            None => false,
        }
    }
}

fn fresh(m: &mut Model) -> u32 {
    let id = m.next;
    m.next += 1;
    id
}

/// The node at (x, y), made if there is none.
fn node_at(m: &mut Model, p: [f64; 2], created: &mut Vec<u32>) -> u32 {
    if let Some(n) = m.nodes.iter().find(|n| (n.x - p[0]).hypot(n.y - p[1]) < SAME) {
        return n.id;
    }
    let id = fresh(m);
    m.nodes.push(Node { id, x: p[0], y: p[1] });
    created.push(id);
    id
}

fn exec(m: &mut Model, op: Op, created: &mut Vec<u32>) -> Result<(), String> {
    match op {
        Op::Add { item } => {
            let id = fresh(m);
            created.push(id);
            match item {
                Item::Node(mut x) => {
                    if !(x.x.is_finite() && x.y.is_finite()) {
                        return Err("A node needs numbers for x and y.".into());
                    }
                    if m.nodes.iter().any(|n| (n.x - x.x).hypot(n.y - x.y) < SAME) {
                        return Err("There is already a node here.".into());
                    }
                    x.id = id;
                    m.nodes.push(x);
                }
                Item::Member(mut x) => {
                    x.id = id;
                    m.members.push(x);
                }
                Item::Shape(mut x) => {
                    x.id = id;
                    if x.name.trim().is_empty() {
                        x.name = format!("{} {}", if x.hole { "Hole" } else { "Shape" }, m.shapes.iter().filter(|s| s.hole == x.hole).count() + 1);
                    }
                    m.shapes.push(x);
                }
                Item::Section(mut x) => {
                    x.id = id;
                    m.sections.push(x);
                }
                Item::Material(mut x) => {
                    x.id = id;
                    m.materials.push(x);
                }
                Item::Support(mut x) => {
                    x.id = id;
                    // One support per place: a new one replaces the old.
                    m.supports.retain(|s| s.at != x.at || is_heat(&s.kind) != is_heat(&x.kind));
                    m.supports.push(x);
                }
                Item::Load(mut x) => {
                    x.id = id;
                    m.loads.push(x);
                }
            }
        }
        Op::Set { item } => {
            fn put<T: Clone>(v: &mut [T], id: impl Fn(&T) -> u32, x: T) -> Result<(), String> {
                let want = id(&x);
                match v.iter_mut().find(|y| id(y) == want) {
                    Some(y) => {
                        *y = x;
                        Ok(())
                    }
                    None => Err("That item no longer exists.".into()),
                }
            }
            match item {
                Item::Node(x) => put(&mut m.nodes, |y| y.id, x)?,
                Item::Member(x) => put(&mut m.members, |y| y.id, x)?,
                Item::Shape(x) => put(&mut m.shapes, |y| y.id, x)?,
                Item::Section(x) => put(&mut m.sections, |y| y.id, x)?,
                Item::Material(x) => put(&mut m.materials, |y| y.id, x)?,
                Item::Support(x) => put(&mut m.supports, |y| y.id, x)?,
                Item::Load(x) => put(&mut m.loads, |y| y.id, x)?,
            }
        }
        Op::AddMember { from, to, kind, section, material } => {
            if from.iter().chain(to.iter()).any(|v| !v.is_finite()) {
                return Err("A member needs numbers for its ends.".into());
            }
            if (from[0] - to[0]).hypot(from[1] - to[1]) < SAME {
                return Err("A member's ends must be at different places.".into());
            }
            let section = section.or(m.sections.first().map(|s| s.id)).ok_or("Add a section first (Geometry › Sections).")?;
            let material = material.or(m.materials.first().map(|s| s.id)).ok_or("Add a material first.")?;
            let a = node_at(m, from, created);
            let b = node_at(m, to, created);
            let id = fresh(m);
            m.members.push(Member { id, a, b, kind, section, material });
            created.push(id);
        }
        Op::Move { ids, dx, dy } => {
            if !(dx.is_finite() && dy.is_finite()) {
                return Err("Move needs numbers.".into());
            }
            let mut nodes: HashSet<u32> = ids.iter().copied().collect();
            for mb in m.members.iter().filter(|mb| ids.contains(&mb.id)) {
                nodes.insert(mb.a);
                nodes.insert(mb.b);
            }
            let mut found = false;
            for n in m.nodes.iter_mut().filter(|n| nodes.contains(&n.id)) {
                n.x += dx;
                n.y += dy;
                found = true;
            }
            for s in m.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                for p in s.geom.points_mut() {
                    p[0] += dx;
                    p[1] += dy;
                }
                found = true;
            }
            // Point supports and loads move with the selection when selected themselves.
            for t in m.supports.iter_mut().filter(|s| ids.contains(&s.id)).map(|s| &mut s.at).chain(m.loads.iter_mut().filter(|s| ids.contains(&s.id)).map(|s| &mut s.at)) {
                if let Target::Point { x, y } = t {
                    *x += dx;
                    *y += dy;
                    found = true;
                }
            }
            if !found && !ids.is_empty() {
                return Err("Nothing that can be moved is selected.".into());
            }
            // Nodes that now sit on top of each other cannot be told apart.
            let mut ps: Vec<(f64, f64)> = m.nodes.iter().map(|n| (n.x, n.y)).collect();
            ps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if ps.windows(2).any(|w| (w[0].0 - w[1].0).hypot(w[0].1 - w[1].1) < SAME) {
                return Err("Two nodes would land on the same place.".into());
            }
        }
        Op::Delete { ids } => {
            let set: HashSet<u32> = ids.iter().copied().collect();
            for mt in m.materials.iter().filter(|x| set.contains(&x.id)) {
                let n = m.members.iter().filter(|mb| mb.material == mt.id && !set.contains(&mb.id)).count() + m.shapes.iter().filter(|s| s.material == mt.id && !set.contains(&s.id)).count();
                if n > 0 {
                    return Err(format!("{} is still used by {n} item{}.", mt.name, if n > 1 { "s" } else { "" }));
                }
            }
            for s in m.sections.iter().filter(|x| set.contains(&x.id)) {
                let n = m.members.iter().filter(|mb| mb.section == s.id && !set.contains(&mb.id)).count();
                if n > 0 {
                    return Err(format!("{} is still used by {n} member{}.", s.name, if n > 1 { "s" } else { "" }));
                }
            }
            m.nodes.retain(|x| !set.contains(&x.id));
            m.members.retain(|x| !set.contains(&x.id));
            m.shapes.retain(|x| !set.contains(&x.id));
            m.sections.retain(|x| !set.contains(&x.id));
            m.materials.retain(|x| !set.contains(&x.id));
            m.supports.retain(|x| !set.contains(&x.id));
            m.loads.retain(|x| !set.contains(&x.id));
        }
        Op::SetSettings { s } => {
            m.analysis = s.analysis;
            m.units = s.units;
            m.grid = s.grid;
            m.thickness = s.thickness;
            m.mesh = s.mesh;
        }
        Op::SetMeta { name, notes } => {
            let name = name.trim();
            if name.is_empty() {
                return Err("A model needs a name.".into());
            }
            m.name = name.into();
            m.notes = notes;
        }
        Op::Batch(ops) => {
            for op in ops {
                exec(m, op, created)?;
            }
        }
    }
    Ok(())
}

fn is_heat(k: &crate::model::SupportKind) -> bool {
    matches!(k, crate::model::SupportKind::Temperature { .. })
}

/// Remove what hangs on deleted items: members on missing nodes, supports and loads on missing
/// nodes, members or edges (also edges a shape no longer has).
fn prune(m: &mut Model) {
    let nodes: HashSet<u32> = m.nodes.iter().map(|n| n.id).collect();
    m.members.retain(|mb| nodes.contains(&mb.a) && nodes.contains(&mb.b));
    let members: HashSet<u32> = m.members.iter().map(|n| n.id).collect();
    let edges: std::collections::HashMap<u32, usize> = m.shapes.iter().map(|s| (s.id, s.geom.edges())).collect();
    let alive = |t: &Target| match t {
        Target::Node { id } => nodes.contains(id),
        Target::Member { id } => members.contains(id),
        Target::Edge { shape, edge } => edges.get(shape).is_some_and(|n| (*edge as usize) < *n),
        Target::Point { .. } | Target::Body => true,
    };
    m.supports.retain(|s| alive(&s.at));
    m.loads.retain(|s| alive(&s.at));
    m.mesh.refine.retain(|s| alive(&s.at));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Geom, LoadKind, SupportKind};

    #[test]
    fn members_share_nodes_and_delete_cascades() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let c = d.apply(Op::AddMember { from: [0.0, 0.0], to: [0.0, 3.0], kind: MemberKind::Beam, section: None, material: None }).unwrap();
        assert_eq!(c.len(), 3);
        let c2 = d.apply(Op::AddMember { from: [0.0, 3.0], to: [4.0, 3.0], kind: MemberKind::Beam, section: None, material: None }).unwrap();
        assert_eq!(c2.len(), 2, "the corner node is shared");
        assert_eq!(d.m.nodes.len(), 3);
        let top = c[1];
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Node { id: top }, kind: LoadKind::Force { fx: 1e3, fy: 0.0 } }) }).unwrap();
        d.apply(Op::Delete { ids: vec![top] }).unwrap();
        assert!(d.m.members.is_empty() && d.m.loads.is_empty());
        assert!(d.undo());
        assert_eq!(d.m.members.len(), 2);
    }

    #[test]
    fn a_support_replaces_the_one_already_there() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let c = d.apply(Op::AddMember { from: [0.0, 0.0], to: [5.0, 0.0], kind: MemberKind::Beam, section: None, material: None }).unwrap();
        for k in [SupportKind::Pinned, SupportKind::Fixed] {
            d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id: c[0] }, kind: k }) }).unwrap();
        }
        assert_eq!(d.m.supports.len(), 1);
        assert_eq!(d.m.supports[0].kind, SupportKind::Fixed);
    }

    #[test]
    fn editing_a_polygon_drops_loads_on_edges_it_lost() {
        let mut d = Doc::new(Model::new(Analysis::Stress));
        let s = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.1, 0.05), ..Shape::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: LoadKind::Pressure { p: 1e6 } }) }).unwrap();
        let mut sh = d.m.shape(s).unwrap().clone();
        sh.geom = Geom::Poly { pts: vec![[0.0, 0.0], [0.1, 0.0], [0.0, 0.1]], bulge: vec![0.0; 3] };
        d.apply(Op::Set { item: Item::Shape(sh) }).unwrap();
        assert!(d.m.loads.is_empty());
        assert_eq!(d.m.shapes[0].name, "Shape 1");
    }

    #[test]
    fn used_materials_stay() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [5.0, 0.0], kind: MemberKind::Truss, section: None, material: None }).unwrap();
        assert!(d.apply(Op::Delete { ids: vec![1] }).unwrap_err().contains("still used"));
        assert!(d.apply(Op::Delete { ids: vec![2] }).unwrap_err().contains("still used"));
    }
}
