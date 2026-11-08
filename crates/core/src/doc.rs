//! The open document: the model, its file, and undo/redo. The window sends edit operations;
//! each one runs on a copy and is kept only if the result passes `Model::check`, so a failed edit
//! leaves the model exactly as it was (Plan's pattern: ops in, whole-model snapshots for undo).

use crate::sketch::{How, Make};
use crate::model::{Case, Combo, Step, dist3, Analysis, Geom, Load, LoadKind, Material, Member, MemberKind, MeshSettings, Model, Node, Section, Shape, Support, Target};
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
    /// Load cases and combinations (0.3.3).
    Case(Case),
    Combo(Combo),
    /// Analysis steps (0.4.0).
    Step(Step),
    /// Parts of an assembly (0.6.0).
    Part(crate::model::Part),
    /// Contact between two faces (0.8.0).
    Contact(crate::model::Contact),
    /// Design members (0.10.4).
    DesignMember(crate::model::DesignMember),
}

/// The model-wide settings, edited together.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub analysis: Analysis,
    pub units: String,
    pub grid: f64,
    pub thickness: f64,
    pub mesh: MeshSettings,
    /// The stress-free temperature (K); left as it is when not given.
    #[serde(default)]
    pub t_ref: Option<f64>,
    /// 3D solids: the angle to revolve through (0 = extrude); left as it is when not given.
    #[serde(default)]
    pub revolve: Option<f64>,
    /// 3D frames (0.4.4): rigid floors; left as it is when not given.
    #[serde(default)]
    pub rigid_floors: Option<bool>,
    /// What the model is (0.6.0, `Model::study`); left as it is when not given.
    #[serde(default)]
    pub study: Option<String>,
    /// An imported mesh's material (0.5.5); left as it is when not given.
    #[serde(default)]
    pub material: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Op {
    /// Add an item; the model gives it a new id.
    Add { item: Item },
    /// Replace the item that has the same id.
    Set { item: Item },
    /// A member between two points: a node already there is used, else one is made. `z` is the
    /// depth of each end (3D frames; zero in the plane).
    AddMember {
        from: [f64; 2],
        to: [f64; 2],
        kind: MemberKind,
        section: Option<u32>,
        material: Option<u32>,
        #[serde(default)]
        z: [f64; 2],
    },
    /// Move nodes and shapes; a member moves its two nodes.
    Move {
        ids: Vec<u32>,
        dx: f64,
        dy: f64,
        #[serde(default)]
        dz: f64,
    },
    /// 3D frames (0.2.4): copy these members (with their member loads, and the supports and
    /// loads on their nodes) `times` times, each `step` further on, and join each copied node to
    /// the one it came from with a member of `link` kind (none if `link` is None).
    Repeat { ids: Vec<u32>, step: [f64; 3], times: u32, link: Option<MemberKind> },
    /// Put a node where any two of these members cross, joining them there (members split at it,
    /// with their loads). Crossing members are otherwise left apart, as X-bracing usually is.
    JoinCrossings { ids: Vec<u32> },
    /// Delete items, with what hangs on them (a node's members, an edge's loads, …). A material or
    /// section still in use is not deleted.
    Delete { ids: Vec<u32> },
    /// Split edge `edge` of a polygon at its middle (on the arc, if it is one), making a new corner.
    /// Supports and loads on the edge cover both halves; later edges keep theirs.
    InsertCorner { shape: u32, edge: u32 },
    /// Remove a polygon corner, joining the two edges that meet there.
    RemoveCorner { shape: u32, corner: u32 },
    /// Sketch tools (0.3.1): round these corners of a polygon to radius `r`. Each must lie between
    /// two straight edges; each adds an arc edge after the corner.
    Fillet { shape: u32, corners: Vec<u32>, r: f64 },
    /// Bevel these corners, cutting `d` off each edge that meets there.
    Chamfer { shape: u32, corners: Vec<u32>, d: f64 },
    /// Union, subtract (`a` − `b`) or intersect two shapes. The result replaces them (keeping
    /// `a`'s id, name and material); rings inside it become holes; supports and loads move to the
    /// edges they lie on.
    Combine { a: u32, b: u32, how: How },
    /// A new shape: the outline moved out by `d` (in, for a negative `d`).
    Offset { shape: u32, d: f64 },
    /// Copies of shapes (with the supports and loads on their edges): copy k is turned through
    /// k·`turn` (radians, counter-clockwise) about `about`, then moved by k·`step`.
    Pattern { ids: Vec<u32>, times: u32, step: [f64; 2], turn: f64, about: [f64; 2] },
    /// A ready-made shape: a slot, a rounded rectangle or a regular polygon.
    Make {
        make: Make,
        hole: bool,
        material: Option<u32>,
        /// The part the shape goes in (assemblies, 0.6.0; 0: the model's own solid).
        #[serde(default)]
        part: u32,
    },
    SetSettings { s: Settings },
    SetMeta { name: String, notes: String },
    /// Parts (0.6.3): move, turn or mirror parts (`copies` 0), or make `copies` copies of them
    /// with their sketches, copy k placed by the change applied k times (a mirror makes one).
    PlaceParts {
        ids: Vec<u32>,
        how: crate::solids::modelling::Placing,
        #[serde(default)]
        copies: u32,
    },
    /// Cut one part through another: the outlines of `tool` go right through `from` as holes,
    /// along `from`'s depth (both extruded, their sketches parallel); `tool` goes.
    Cut { tool: u32, from: u32 },
    /// A round hole of diameter `d` right through an extruded part, at a point on its front or
    /// back face.
    Hole { part: u32, at: [f64; 3], d: f64 },
    /// Round (radius `size`) or bevel (`size` off each side) the part's edge along its sweep
    /// nearest the point `near`: the sketch corner under it.
    PartEdge { part: u32, near: [f64; 3], size: f64, round: bool },
    /// Frames (0.10.3): cut each member at these fractions of its length (0 < t < 1); its
    /// loads, hinges and offsets go with the pieces.
    SplitMember { ids: Vec<u32>, at: Vec<f64> },
    /// Frames (0.10.3): turn members end for end; their hinges, offsets and member loads follow,
    /// so the structure and its loads are unchanged.
    ReverseMember { ids: Vec<u32> },
    /// Frames (0.10.3): join two members in line that meet at a node nothing else uses into one.
    MergeMembers { ids: Vec<u32> },
    /// Design members (0.10.4): one from these members, end to end in a straight line and alike.
    FormDesignMember { ids: Vec<u32>, name: String },
    /// The mechanics checks' settings (0.10.4).
    CheckSettings { s: crate::model::CheckSettings },
    /// Combinations by rule (0.10.3, `cases::by_rule`): replace the combinations made by a rule
    /// before (never the user's own) with those of this one, and remember it.
    CombosByRule { rule: crate::model::ComboRule },
    /// 3D frames (0.10.3): turn members' sections about their own axes. `angle` (radians) sets the
    /// roll; `towards` (a point) or `along` (a direction) turns each so its local y (the section's
    /// depth) faces it, as near as the member's own direction allows.
    Orient {
        ids: Vec<u32>,
        #[serde(default)]
        angle: Option<f64>,
        #[serde(default)]
        towards: Option<[f64; 3]>,
        #[serde(default)]
        along: Option<[f64; 3]>,
    },
    /// Frames (0.10.3): bring nodes closer than `tol` (m) together, onto the first of each group
    /// (a supported one first); members, supports and loads follow it.
    MergeNodes { ids: Vec<u32>, tol: f64 },
    Batch(Vec<Op>),
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub m: Model,
    pub path: Option<String>,
    pub dirty: bool,
    /// The model as last saved (or opened): the document is changed when it differs from this,
    /// so undoing back to it clears the mark. None: never saved (a recovered model).
    saved: Option<Model>,
    undo: Vec<Model>,
    redo: Vec<Model>,
}

const UNDO_LIMIT: usize = 200;
/// Points closer than this are the same node (m).
pub const SAME: f64 = 1e-7;

impl Doc {
    pub fn new(m: Model) -> Doc {
        Doc { saved: Some(m.clone()), m, ..Default::default() }
    }
    /// The model was just saved: unchanged from here.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.m.clone());
        self.dirty = false;
    }
    /// A model that has never been saved anywhere (recovered work): changed whatever is undone.
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
        self.dirty = true;
    }
    fn update_dirty(&mut self) {
        self.dirty = self.saved.as_ref() != Some(&self.m);
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
        // Nodes that were added or moved merge with a node already there, and split any member
        // they land on; members that were added or moved are split at the nodes along them.
        let old_n: std::collections::HashMap<u32, [f64; 3]> = self.m.nodes.iter().map(|n| (n.id, n.at())).collect();
        let old_m: std::collections::HashMap<u32, (u32, u32)> = self.m.members.iter().map(|b| (b.id, (b.a, b.b))).collect();
        let tn: Vec<u32> = m.nodes.iter().filter(|n| old_n.get(&n.id) != Some(&n.at())).map(|n| n.id).collect();
        let tm: Vec<u32> = m.members.iter().filter(|b| old_m.get(&b.id) != Some(&(b.a, b.b)) || tn.contains(&b.a) || tn.contains(&b.b)).map(|b| b.id).collect();
        if !tn.is_empty() || !tm.is_empty() {
            merge_nodes(&mut m, &tn);
            join(&mut m, &tn, &tm);
            prune(&mut m);
        }
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
        self.update_dirty();
        Ok(created)
    }

    /// Replace the whole model by one made outside the edit operations (a mesh of the design's
    /// solids, 0.7.3), checked, as one step that Undo takes back.
    pub fn replace(&mut self, m: Model) -> Result<(), String> {
        m.check()?;
        if m == self.m {
            return Ok(());
        }
        let old = std::mem::replace(&mut self.m, m);
        self.undo.push(old);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.update_dirty();
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(m) => {
                self.redo.push(std::mem::replace(&mut self.m, m));
                self.update_dirty();
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(m) => {
                self.undo.push(std::mem::replace(&mut self.m, m));
                self.update_dirty();
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

/// The node at (x, y, z), made if there is none.
fn node_at(m: &mut Model, p: [f64; 3], created: &mut Vec<u32>) -> u32 {
    if let Some(n) = m.nodes.iter().find(|n| dist3(n.at(), p) < SAME) {
        return n.id;
    }
    let id = fresh(m);
    m.nodes.push(Node { id, x: p[0], y: p[1], z: p[2] });
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
                    if !(x.x.is_finite() && x.y.is_finite() && x.z.is_finite()) {
                        return Err("A node needs numbers for x, y and z.".into());
                    }
                    if m.nodes.iter().any(|n| dist3(n.at(), x.at()) < SAME) {
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
                        x.name = shape_name(m, x.hole);
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
                Item::Case(mut x) => {
                    x.id = id;
                    if x.name.trim().is_empty() {
                        x.name = format!("Case {}", m.cases.len() + 1);
                    }
                    // The loads so far were in the one implicit case: they join the first case.
                    if m.cases.is_empty() {
                        m.loads.iter_mut().for_each(|l| l.case = id);
                    }
                    m.cases.push(x);
                }
                Item::Combo(mut x) => {
                    x.id = id;
                    if x.name.trim().is_empty() {
                        x.name = format!("Combination {}", m.combos.len() + 1);
                    }
                    m.combos.push(x);
                }
                Item::Step(mut x) => {
                    x.id = id;
                    if x.name.trim().is_empty() {
                        x.name = format!("Step {}", m.steps.len() + 1);
                    }
                    m.steps.push(x);
                }
                Item::DesignMember(mut x) => {
                    x.id = id;
                    if x.name.trim().is_empty() {
                        x.name = format!("DM{}", m.design_members.len() + 1);
                    }
                    m.design_members.push(x);
                }
                Item::Contact(mut x) => {
                    x.id = id;
                    // One contact per pair of faces.
                    m.contacts.retain(|c| !((c.a == x.a && c.b == x.b) || (c.a == x.b && c.b == x.a)));
                    m.contacts.push(x);
                }
                Item::Part(mut x) => {
                    x.id = id;
                    if x.name.trim().is_empty() {
                        x.name = format!("Part {}", m.parts.len() + 1);
                    }
                    if !m.materials.iter().any(|y| y.id == x.material) {
                        x.material = m.materials.first().map(|y| y.id).ok_or("Add a material first.")?;
                    }
                    m.parts.push(x);
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
                Item::Case(x) => put(&mut m.cases, |y| y.id, x)?,
                Item::Combo(x) => put(&mut m.combos, |y| y.id, x)?,
                Item::Step(x) => put(&mut m.steps, |y| y.id, x)?,
                Item::Part(x) => put(&mut m.parts, |y| y.id, x)?,
                Item::Contact(x) => put(&mut m.contacts, |y| y.id, x)?,
                Item::DesignMember(x) => put(&mut m.design_members, |y| y.id, x)?,
            }
        }
        Op::AddMember { from, to, kind, section, material, z } => {
            let (from, to) = ([from[0], from[1], z[0]], [to[0], to[1], z[1]]);
            if from.iter().chain(to.iter()).any(|v| !v.is_finite()) {
                return Err("A member needs numbers for its ends.".into());
            }
            if dist3(from, to) < SAME {
                return Err("A member's ends must be at different places.".into());
            }
            let section = section.or(m.sections.first().map(|s| s.id)).ok_or("Add a section first (Geometry › Sections).")?;
            let material = material.or(m.materials.first().map(|s| s.id)).ok_or("Add a material first.")?;
            let a = node_at(m, from, created);
            let b = node_at(m, to, created);
            let id = fresh(m);
            m.members.push(Member { id, a, b, kind, section, material, roll: 0.0, hinge: [false; 2], only: Default::default(), offset: [0.0; 2] });
            created.push(id);
        }
        Op::SplitMember { ids, at } => {
            if at.is_empty() || !at.iter().all(|t| t.is_finite() && *t > 0.0 && *t < 1.0) {
                return Err("Split a member at places between its ends (more than 0 and less than 1 of its length).".into());
            }
            let pos: std::collections::HashMap<u32, [f64; 3]> = m.nodes.iter().map(|n| (n.id, n.at())).collect();
            let ends: Vec<([f64; 3], [f64; 3])> = m.members.iter().filter(|b| ids.contains(&b.id)).filter_map(|b| Some((*pos.get(&b.a)?, *pos.get(&b.b)?))).collect();
            if ends.is_empty() {
                return Err("Pick the members to split.".into());
            }
            // New nodes along each member; `apply` then cuts the member at them, loads and all.
            for (a, b) in ends {
                for &t in &at {
                    node_at(m, [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t], created);
                }
            }
        }
        Op::ReverseMember { ids } => {
            let space = m.analysis == crate::model::Analysis::Space;
            for id in ids {
                let Some(i) = m.members.iter().position(|b| b.id == id) else { continue };
                let len = m.member_length(id).unwrap_or(0.0);
                let b = m.members[i].clone();
                let (Some(na), Some(nb)) = (m.node(b.a).map(|n| n.at()), m.node(b.b).map(|n| n.at())) else { continue };
                let roll = if space { b.roll } else { 0.0 };
                // A load across the member keeps its side in the world: its sign follows its local y.
                let (y0, y1) = (crate::space::axes(na, nb, roll)[1], crate::space::axes(nb, na, roll)[1]);
                let flip = if y0[0] * y1[0] + y0[1] * y1[1] + y0[2] * y1[2] < 0.0 { -1.0 } else { 1.0 };
                let mb = &mut m.members[i];
                (mb.a, mb.b) = (b.b, b.a);
                mb.hinge = [b.hinge[1], b.hinge[0]];
                mb.offset = [b.offset[1], b.offset[0]];
                for l in m.loads.iter_mut().filter(|l| l.at == Target::Member { id }) {
                    let s = |d: &crate::model::Dir| if *d == crate::model::Dir::Local { flip } else { 1.0 };
                    l.kind = match l.kind {
                        LoadKind::Dist { w1, w2, dir } => LoadKind::Dist { w1: w2 * s(&dir), w2: w1 * s(&dir), dir },
                        LoadKind::Patch { w1, w2, dir, from, to, projected } => LoadKind::Patch { w1: w2 * s(&dir), w2: w1 * s(&dir), dir, from: (len - to).max(0.0), to: len - from, projected },
                        LoadKind::Concentrated { f, dir, at } => LoadKind::Concentrated { f: f * s(&dir), dir, at: (len - at).max(0.0) },
                        ref k => k.clone(),
                    };
                }
            }
        }
        Op::MergeMembers { ids } => {
            let [i, j] = match ids.as_slice() {
                [a, b] => [m.members.iter().position(|x| x.id == *a), m.members.iter().position(|x| x.id == *b)].map(|x| x.ok_or("Pick two members to merge.")),
                _ => return Err("Pick two members to merge.".into()),
            };
            let (i, j) = (i?, j?);
            let (p, q) = (m.members[i].clone(), m.members[j].clone());
            let shared = [p.a, p.b].into_iter().find(|n| *n == q.a || *n == q.b).ok_or("These two members do not meet: pick two that share a node.")?;
            if p.kind != q.kind || p.section != q.section || p.material != q.material || p.only != q.only || p.roll != q.roll {
                return Err("These members differ (kind, section, material, orientation or tension/compression only): make them alike first.".into());
            }
            let far_p = if p.a == shared { p.b } else { p.a };
            let far_q = if q.a == shared { q.b } else { q.a };
            let (Some(a), Some(c), Some(o)) = (m.node(far_p).map(|n| n.at()), m.node(far_q).map(|n| n.at()), m.node(shared).map(|n| n.at())) else { return Err("A member's node is missing.".into()) };
            let (u, v) = ([o[0] - a[0], o[1] - a[1], o[2] - a[2]], [c[0] - o[0], c[1] - o[1], c[2] - o[2]]);
            let cr = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let (lu, lv) = (dist3(a, o), dist3(o, c));
            if (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt() > 1e-9 * lu * lv || u[0] * v[0] + u[1] * v[1] + u[2] * v[2] <= 0.0 {
                return Err("These members are not in line: only members along one straight line merge.".into());
            }
            let others = m.members.iter().filter(|b| b.id != p.id && b.id != q.id && (b.a == shared || b.b == shared)).count();
            let held = m.supports.iter().any(|s| s.at == Target::Node { id: shared }) || m.loads.iter().any(|l| l.at == Target::Node { id: shared });
            if others > 0 || held {
                return Err("The node between them carries another member, a support or a load: it has to stay.".into());
            }
            let hinged_mid = (if p.a == shared { p.hinge[0] } else { p.hinge[1] }) || (if q.a == shared { q.hinge[0] } else { q.hinge[1] });
            if hinged_mid {
                return Err("There is a hinge at the node between them: it would be lost. Remove it first.".into());
            }
            // The merged member runs from p's far end to q's far end; each piece's loads move onto
            // it as loads over its part (a whole-length uniform pair stays one uniform load).
            let total = lu + lv;
            let mut nb = p.clone();
            nb.a = far_p;
            nb.b = far_q;
            nb.hinge = [if p.a == far_p { p.hinge[0] } else { p.hinge[1] }, if q.a == far_q { q.hinge[0] } else { q.hinge[1] }];
            nb.offset = [if p.a == far_p { p.offset[0] } else { p.offset[1] }, if q.a == far_q { q.offset[0] } else { q.offset[1] }];
            let space = m.analysis == crate::model::Analysis::Space;
            let ly = |x: [f64; 3], y: [f64; 3]| crate::space::axes(x, y, if space { p.roll } else { 0.0 })[1];
            let ny = ly(a, c);
            for (old, start, rev) in [(p.id, 0.0, p.a != far_p), (q.id, lu, q.a != shared)] {
                let len = if old == p.id { lu } else { lv };
                let (oa, ob) = if old == p.id { if rev { (o, a) } else { (a, o) } } else if rev { (c, o) } else { (o, c) };
                let oy = ly(oa, ob);
                let flip = if oy[0] * ny[0] + oy[1] * ny[1] + oy[2] * ny[2] < 0.0 { -1.0 } else { 1.0 };
                for l in m.loads.iter_mut().filter(|l| l.at == Target::Member { id: old }) {
                    let sg = |d: &crate::model::Dir| if *d == crate::model::Dir::Local { flip } else { 1.0 };
                    // Along the new member: a place s from the old member's start.
                    let at = |s: f64| if rev { start + len - s } else { start + s };
                    l.kind = match l.kind {
                        LoadKind::Dist { w1, w2, dir } => {
                            let (f, t) = (at(0.0).min(at(len)), at(0.0).max(at(len)));
                            let (ws, we) = if rev { (w2, w1) } else { (w1, w2) };
                            LoadKind::Patch { w1: ws * sg(&dir), w2: we * sg(&dir), dir, from: f, to: t, projected: false }
                        }
                        LoadKind::Patch { w1, w2, dir, from, to, projected } => {
                            let (ws, we) = if rev { (w2, w1) } else { (w1, w2) };
                            LoadKind::Patch { w1: ws * sg(&dir), w2: we * sg(&dir), dir, from: at(from).min(at(to)), to: at(from).max(at(to)), projected }
                        }
                        LoadKind::Concentrated { f, dir, at: x } => LoadKind::Concentrated { f: f * sg(&dir), dir, at: at(x).clamp(0.0, total) },
                        ref k => k.clone(),
                    };
                    l.at = Target::Member { id: nb.id };
                }
            }
            // Two halves of one uniform load over the whole member are one load again.
            let whole: Vec<usize> = m.loads.iter().enumerate().filter(|(_, l)| l.at == Target::Member { id: nb.id } && matches!(l.kind, LoadKind::Patch { projected: false, .. })).map(|(k, _)| k).collect();
            if let [x, y] = whole[..] {
                if let (LoadKind::Patch { w1, w2, dir, from, to, .. }, LoadKind::Patch { w1: v1, w2: v2, dir: d2, from: f2, to: t2, .. }) = (m.loads[x].kind.clone(), m.loads[y].kind.clone()) {
                    let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * total.max(1.0);
                    let joined = m.loads[x].case == m.loads[y].case && dir == d2 && w1 == w2 && v1 == v2 && w1 == v1 && ((near(from, 0.0) && near(to, f2) && near(t2, total)) || (near(f2, 0.0) && near(t2, from) && near(to, total)));
                    if joined {
                        m.loads[x].kind = LoadKind::Dist { w1, w2: w1, dir };
                        m.loads.remove(y);
                    }
                }
            }
            m.members.retain(|b| b.id != q.id);
            let k = m.members.iter().position(|b| b.id == p.id).unwrap();
            m.members[k] = nb;
            m.nodes.retain(|n| n.id != shared);
        }
        Op::FormDesignMember { ids, name } => {
            let (a, b) = crate::frames::design::form(m, &ids)?;
            if m.design_members.iter().any(|x| (x.a == a && x.b == b) || (x.a == b && x.b == a)) {
                return Err("These members are a design member already.".into());
            }
            let id = fresh(m);
            let name = if name.trim().is_empty() { format!("DM{}", m.design_members.len() + 1) } else { name };
            m.design_members.push(crate::model::DesignMember { id, name, a, b, restraints: vec![] });
            created.push(id);
        }
        Op::CheckSettings { s } => {
            if !(s.k > 0.0 && s.k.is_finite() && s.span_ratio > 0.0 && s.span_ratio.is_finite() && s.drift > 0.0 && s.drift.is_finite()) {
                return Err("The effective length factor, the deflection ratio and the drift limit must be numbers above zero.".into());
            }
            m.check_settings = s;
        }
        Op::CombosByRule { rule } => {
            let made = crate::cases::by_rule(m, &rule)?;
            m.combos.retain(|c| !c.auto);
            for mut c in made {
                c.id = fresh(m);
                created.push(c.id);
                m.combos.push(c);
            }
            m.combo_rule = Some(rule);
        }
        Op::Orient { ids, angle, towards, along } => {
            let picked = [angle.is_some(), towards.is_some(), along.is_some()].iter().filter(|x| **x).count();
            if picked != 1 {
                return Err("Give the members an angle, a point to face or a direction to face: one of them.".into());
            }
            let fin = |v: [f64; 3]| v.iter().all(|x| x.is_finite());
            if angle.is_some_and(|a| !a.is_finite()) || towards.is_some_and(|v| !fin(v)) || along.is_some_and(|v| !fin(v)) {
                return Err("The angle, point or direction must be numbers.".into());
            }
            for id in ids {
                let Some(i) = m.members.iter().position(|b| b.id == id) else { continue };
                let (Some(a), Some(b)) = (m.node(m.members[i].a).map(|n| n.at()), m.node(m.members[i].b).map(|n| n.at())) else { continue };
                let roll = if let Some(r) = angle {
                    r
                } else {
                    // The direction to face, square to the member: roll it from its zero-roll axes.
                    let [ex, ey0, ez0] = crate::space::axes(a, b, 0.0);
                    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
                    let v = towards.map(|p| [p[0] - mid[0], p[1] - mid[1], p[2] - mid[2]]).or(along).unwrap();
                    let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
                    let vx = dot(v, ex);
                    let w = [v[0] - vx * ex[0], v[1] - vx * ex[1], v[2] - vx * ex[2]];
                    if dot(w, w).sqrt() < 1e-9 * dot(v, v).sqrt().max(1e-300) {
                        // It points straight at it: no way to face it, so it stays as it is.
                        continue;
                    }
                    dot(w, ez0).atan2(dot(w, ey0))
                };
                // Kept between −180° and 180°.
                let t = std::f64::consts::TAU;
                m.members[i].roll = roll - t * ((roll + std::f64::consts::PI) / t).floor();
            }
        }
        Op::MergeNodes { ids, tol } => {
            if !(tol.is_finite() && tol >= 0.0) {
                return Err("The tolerance must be a length of zero or more.".into());
            }
            let pick: Vec<u32> = if ids.is_empty() { m.nodes.iter().map(|n| n.id).collect() } else { ids };
            let supported = |m: &Model, id: u32| m.supports.iter().any(|s| s.at == Target::Node { id });
            let mut order: Vec<u32> = pick.clone();
            order.sort_by_key(|id| (!supported(m, *id), *id));
            let mut moved = std::collections::HashSet::new();
            for &keep in &order {
                if moved.contains(&keep) {
                    continue;
                }
                let Some(p) = m.node(keep).map(|n| n.at()) else { continue };
                for &other in &order {
                    if other == keep || moved.contains(&other) {
                        continue;
                    }
                    if let Some(n) = m.nodes.iter_mut().find(|n| n.id == other) {
                        let d = dist3(n.at(), p);
                        if d <= tol.max(SAME) && d > 0.0 {
                            (n.x, n.y, n.z) = (p[0], p[1], p[2]);
                            moved.insert(other);
                        }
                    }
                }
                moved.insert(keep);
            }
        }
        Op::JoinCrossings { ids } => {
            // New nodes where two members meet (closest points of the two lines closer than SAME);
            // `apply` then splits every member a new node lies on.
            let pos: std::collections::HashMap<u32, [f64; 3]> = m.nodes.iter().map(|n| (n.id, n.at())).collect();
            let segs: Vec<([f64; 3], [f64; 3])> = m.members.iter().filter(|b| ids.contains(&b.id)).filter_map(|b| Some((*pos.get(&b.a)?, *pos.get(&b.b)?))).collect();
            let mut pts: Vec<[f64; 3]> = vec![];
            let sub3 = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let at = |a: [f64; 3], r: [f64; 3], t: f64| [a[0] + r[0] * t, a[1] + r[1] * t, a[2] + r[2] * t];
            for i in 0..segs.len() {
                for j in i + 1..segs.len() {
                    let ((a, b), (c, d)) = (segs[i], segs[j]);
                    let (r, q, w) = (sub3(b, a), sub3(d, c), sub3(a, c));
                    let (rr, qq, rq, rw, qw) = (dot(r, r), dot(q, q), dot(r, q), dot(r, w), dot(q, w));
                    let den = rr * qq - rq * rq;
                    if den <= 1e-24 * rr * qq {
                        continue; // parallel
                    }
                    let t = (rq * qw - qq * rw) / den;
                    let u = (rr * qw - rq * rw) / den;
                    let (p, p2) = (at(a, r, t), at(c, q, u));
                    let inner = |s: f64, l: f64| s * l > SAME && (1.0 - s) * l > SAME;
                    if dist3(p, p2) < SAME && inner(t, rr.sqrt()) && inner(u, qq.sqrt()) && !pts.iter().any(|x| dist3(*x, p) < SAME) {
                        pts.push(p);
                    }
                }
            }
            if pts.is_empty() {
                return Err("None of these members cross each other between their ends.".into());
            }
            for p in pts {
                let id = fresh(m);
                m.nodes.push(Node { id, x: p[0], y: p[1], z: p[2] });
                created.push(id);
            }
        }
        Op::Repeat { ids, step, times, link } => {
            if !step.iter().all(|v| v.is_finite()) || dist3(step, [0.0; 3]) < SAME {
                return Err("Repeat needs a step that moves the copies.".into());
            }
            if !(1..=100).contains(&times) {
                return Err("Repeat makes 1 to 100 copies.".into());
            }
            let src: Vec<Member> = m.members.iter().filter(|b| ids.contains(&b.id)).cloned().collect();
            if src.is_empty() {
                return Err("Select the members to repeat.".into());
            }
            let pos: std::collections::HashMap<u32, [f64; 3]> = m.nodes.iter().map(|n| (n.id, n.at())).collect();
            let mut nodes: Vec<u32> = src.iter().flat_map(|b| [b.a, b.b]).collect();
            nodes.sort_unstable();
            nodes.dedup();
            let (sec, mat) = (src[0].section, src[0].material);
            let mut prev: std::collections::HashMap<u32, u32> = nodes.iter().map(|n| (*n, *n)).collect();
            for k in 1..=times {
                let shift = |p: [f64; 3]| [p[0] + step[0] * k as f64, p[1] + step[1] * k as f64, p[2] + step[2] * k as f64];
                let mut now = std::collections::HashMap::new();
                for &n in &nodes {
                    let id = node_at(m, shift(pos[&n]), created);
                    now.insert(n, id);
                    // Supports and node loads come along.
                    let sups: Vec<_> = m.supports.iter().filter(|s| s.at == Target::Node { id: n }).cloned().collect();
                    for mut s in sups {
                        s.id = fresh(m);
                        s.at = Target::Node { id };
                        created.push(s.id);
                        m.supports.push(s);
                    }
                    let lds: Vec<_> = m.loads.iter().filter(|l| l.at == Target::Node { id: n }).cloned().collect();
                    for mut l in lds {
                        l.id = fresh(m);
                        l.at = Target::Node { id };
                        created.push(l.id);
                        m.loads.push(l);
                    }
                }
                for b in &src {
                    let (a, c) = (now[&b.a], now[&b.b]);
                    if m.members.iter().any(|x| (x.a == a && x.b == c) || (x.a == c && x.b == a)) {
                        continue;
                    }
                    let id = fresh(m);
                    m.members.push(Member { id, a, b: c, ..b.clone() });
                    created.push(id);
                    let lds: Vec<_> = m.loads.iter().filter(|l| l.at == Target::Member { id: b.id }).cloned().collect();
                    for mut l in lds {
                        l.id = fresh(m);
                        l.at = Target::Member { id };
                        created.push(l.id);
                        m.loads.push(l);
                    }
                }
                if let Some(kind) = link {
                    for &n in &nodes {
                        let (a, c) = (prev[&n], now[&n]);
                        if a != c && !m.members.iter().any(|x| (x.a == a && x.b == c) || (x.a == c && x.b == a)) {
                            let id = fresh(m);
                            m.members.push(Member { id, a, b: c, kind, section: sec, material: mat, roll: 0.0, hinge: [false; 2], only: Default::default(), offset: [0.0; 2] });
                            created.push(id);
                        }
                    }
                }
                prev = now;
            }
        }
        Op::Move { ids, dx, dy, dz } => {
            if !(dx.is_finite() && dy.is_finite() && dz.is_finite()) {
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
                n.z += dz;
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
            m.contacts.retain(|x| !set.contains(&x.id));
            m.design_members.retain(|x| !set.contains(&x.id));
            // A deleted load case takes its loads with it, and leaves the combinations.
            let gone: Vec<u32> = m.cases.iter().filter(|c| set.contains(&c.id)).map(|c| c.id).collect();
            if !gone.is_empty() {
                let mm = m.clone();
                m.loads.retain(|l| !gone.contains(&mm.case_of(l)));
                m.cases.retain(|c| !gone.contains(&c.id));
                for c in m.combos.iter_mut() {
                    c.parts.retain(|p| !gone.contains(&p.case));
                }
            }
            m.combos.retain(|x| !set.contains(&x.id));
            m.steps.retain(|x| !set.contains(&x.id));
            // A deleted part takes its sketch with it.
            let parts_gone: Vec<u32> = m.parts.iter().filter(|p| set.contains(&p.id)).map(|p| p.id).collect();
            m.shapes.retain(|x| !parts_gone.contains(&x.part));
            m.parts.retain(|x| !set.contains(&x.id));
            // A step whose case or combination went loses it (it takes the window's choice).
            for st in m.steps.iter_mut() {
                if let Some(crate::cases::Which::Case { id } | crate::cases::Which::Combo { id }) = st.load {
                    if set.contains(&id) || gone.contains(&id) {
                        st.load = None;
                    }
                }
            }
            m.loads.retain(|x| !set.contains(&x.id));
        }
        Op::InsertCorner { shape, edge } => {
            let sh = m.shapes.iter_mut().find(|x| x.id == shape).ok_or("That shape no longer exists.")?;
            let Geom::Poly { pts, bulge } = &mut sh.geom else { return Err("Circles have no corners; draw a polygon to shape the outline.".into()) };
            let i = edge as usize;
            if i >= pts.len() {
                return Err("That edge does not exist.".into());
            }
            let (p, q, b) = (pts[i], pts[(i + 1) % pts.len()], bulge[i]);
            let (mid, half) = if b == 0.0 {
                ([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], 0.0)
            } else {
                // The middle of the arc, and each half turning through half the angle.
                let (c, r, a0, th) = crate::model::arc(p, q, b);
                let t = a0 + th / 2.0;
                ([c[0] + r * t.cos(), c[1] + r * t.sin()], (th / 8.0).tan())
            };
            pts.insert(i + 1, mid);
            bulge[i] = half;
            bulge.insert(i + 1, half);
            renumber(m, shape, |e| if e > edge { vec![e + 1] } else if e == edge { vec![e, e + 1] } else { vec![e] });
        }
        Op::RemoveCorner { shape, corner } => {
            let sh = m.shapes.iter_mut().find(|x| x.id == shape).ok_or("That shape no longer exists.")?;
            let Geom::Poly { pts, bulge } = &mut sh.geom else { return Err("Circles have no corners.".into()) };
            let n = pts.len() as u32;
            if corner >= n {
                return Err("That corner does not exist.".into());
            }
            if n <= 3 {
                return Err("A polygon needs at least three corners.".into());
            }
            // The edge before the corner and the one after it become one straight edge.
            let before = (corner + n - 1) % n;
            pts.remove(corner as usize);
            bulge.remove(corner as usize);
            let b = if corner == 0 { bulge.len() - 1 } else { corner as usize - 1 };
            bulge[b] = 0.0;
            // Old edge `corner` joins old edge `before`; later edges move down one.
            let merged = if corner == 0 { n - 2 } else { before };
            renumber(m, shape, |e| {
                let ne = if e == corner { merged } else if e > corner { e - 1 } else { e };
                vec![if corner == 0 && e == n - 1 { n - 2 } else { ne }]
            });
        }
        Op::Fillet { shape, corners, r } => round_corners(m, shape, corners, r, true)?,
        Op::Chamfer { shape, corners, d } => round_corners(m, shape, corners, d, false)?,
        Op::Combine { a, b, how } => {
            if a == b {
                return Err("Pick two different shapes to combine.".into());
            }
            let gone = "That shape no longer exists.";
            let sa = m.shapes.iter().find(|s| s.id == a).cloned().ok_or(gone)?;
            let sb = m.shapes.iter().find(|s| s.id == b).cloned().ok_or(gone)?;
            let mut out = crate::sketch::combine(&sa.geom, &sb.geom, how)?;
            out.sort_by_key(|p| p.inner);
            m.shapes.retain(|s| s.id != b);
            let mut to: std::collections::HashMap<(usize, u32), Vec<Target>> = Default::default();
            for (k, p) in out.into_iter().enumerate() {
                let hole = sa.hole != p.inner;
                let id = if k == 0 { a } else { fresh(m) };
                for (e, from) in p.from.iter().enumerate() {
                    for f in from {
                        to.entry(*f).or_default().push(Target::Edge { shape: id, edge: e as u32 });
                    }
                }
                if k == 0 {
                    let s = m.shapes.iter_mut().find(|s| s.id == a).ok_or(gone)?;
                    s.geom = p.geom;
                } else {
                    let name = shape_name(m, hole);
                    m.shapes.push(Shape { id, name, hole, material: sa.material, geom: p.geom, plate: sa.plate, part: sa.part });
                    created.push(id);
                }
            }
            retarget(m, |t| match t {
                Target::Edge { shape, edge } if *shape == a || *shape == b => Some(to.get(&(usize::from(*shape == b), *edge)).cloned().unwrap_or_default()),
                Target::Line { shape, edge, at } if *shape == a || *shape == b => Some(to.get(&(usize::from(*shape == b), *edge)).map(|v| v.iter().map(|t| match t { Target::Edge { shape, edge } => Target::Line { shape: *shape, edge: *edge, at: *at }, t => t.clone() }).collect()).unwrap_or_default()),
                _ => None,
            });
        }
        Op::Offset { shape, d } => {
            let s = m.shapes.iter().find(|s| s.id == shape).cloned().ok_or("That shape no longer exists.")?;
            let geom = crate::sketch::offset(&s.geom, d)?;
            let id = fresh(m);
            let name = shape_name(m, s.hole);
            m.shapes.push(Shape { id, name, geom, ..s });
            created.push(id);
        }
        Op::Pattern { ids, times, step, turn, about } => {
            if !(1..=1000).contains(&times) {
                return Err("A pattern makes 1 to 1000 copies.".into());
            }
            if !(step.iter().chain(about.iter()).all(|v| v.is_finite()) && turn.is_finite()) || (step[0].hypot(step[1]) < SAME && turn.abs() < 1e-9) {
                return Err("A pattern needs a step or a turn that moves the copies.".into());
            }
            let src: Vec<Shape> = m.shapes.iter().filter(|s| ids.contains(&s.id)).cloned().collect();
            if src.is_empty() {
                return Err("Select the shapes to copy.".into());
            }
            for k in 1..=times {
                for s in &src {
                    let mut geom = s.geom.clone();
                    crate::sketch::turn(&mut geom, about, turn * k as f64);
                    for p in geom.points_mut() {
                        p[0] += step[0] * k as f64;
                        p[1] += step[1] * k as f64;
                    }
                    let id = fresh(m);
                    let name = shape_name(m, s.hole);
                    m.shapes.push(Shape { id, name, geom, ..s.clone() });
                    created.push(id);
                    // Supports and loads on its edges come along (turned with it).
                    let on = |t: &Target| matches!(t, Target::Edge { shape, .. } | Target::Line { shape, .. } if *shape == s.id);
                    let moved = |t: &Target| match t {
                        Target::Edge { edge, .. } => Target::Edge { shape: id, edge: *edge },
                        Target::Line { edge, at, .. } => Target::Line { shape: id, edge: *edge, at: *at },
                        t => t.clone(),
                    };
                    let sups: Vec<_> = m.supports.iter().filter(|x| on(&x.at)).cloned().collect();
                    for mut x in sups {
                        x.id = fresh(m);
                        x.at = moved(&x.at);
                        created.push(x.id);
                        m.supports.push(x);
                    }
                    let lds: Vec<_> = m.loads.iter().filter(|x| on(&x.at)).cloned().collect();
                    for mut x in lds {
                        x.id = fresh(m);
                        x.at = moved(&x.at);
                        x.kind = turn_load(x.kind, turn * k as f64);
                        created.push(x.id);
                        m.loads.push(x);
                    }
                    let refs: Vec<_> = m.mesh.refine.iter().filter(|x| on(&x.at)).cloned().collect();
                    for mut x in refs {
                        x.at = moved(&x.at);
                        m.mesh.refine.push(x);
                    }
                }
            }
        }
        Op::Make { make, hole, material, part } => {
            let geom = crate::sketch::make(&make)?;
            let material = material.or(m.materials.first().map(|s| s.id)).ok_or("Add a material first.")?;
            let id = fresh(m);
            let name = shape_name(m, hole);
            m.shapes.push(Shape { id, name, hole, material, geom, plate: None, part });
            created.push(id);
        }
        Op::SetSettings { s } => {
            m.analysis = s.analysis;
            m.units = s.units;
            m.grid = s.grid;
            m.thickness = s.thickness;
            m.mesh = s.mesh;
            if let Some(t) = s.t_ref {
                m.t_ref = t;
            }
            if let Some(r) = s.revolve {
                m.revolve = r;
            }
            if let (Some(id), Some(v)) = (s.material, m.volume.as_mut()) {
                if m.materials.iter().any(|x| x.id == id) {
                    v.material = id;
                }
            }
            if let Some(st) = &s.study {
                m.study = st.chars().filter(|c| c.is_ascii_alphanumeric()).take(16).collect();
            }
            if let Some(r) = s.rigid_floors {
                m.rigid_floors = r;
            }
        }
        Op::SetMeta { name, notes } => {
            let name = name.trim();
            if name.is_empty() {
                return Err("A model needs a name.".into());
            }
            m.name = name.into();
            m.notes = notes;
        }
        Op::PlaceParts { ids, how, copies } => {
            use crate::solids::modelling::{placed, Placing};
            if matches!(how, Placing::Mirror { .. }) && copies > 1 {
                return Err("A mirror makes one copy.".into());
            }
            if copies > 1000 {
                return Err("A pattern makes 1 to 1000 copies.".into());
            }
            let src: Vec<crate::model::Part> = m.parts.iter().filter(|p| ids.contains(&p.id)).cloned().collect();
            if src.is_empty() {
                return Err("Select the parts first.".into());
            }
            if copies == 0 {
                for p in &src {
                    let q = placed(p, &how, 1)?;
                    if let Some(x) = m.parts.iter_mut().find(|x| x.id == p.id) {
                        *x = q;
                    }
                }
            }
            for k in 1..=copies {
                for p in &src {
                    let mut q = placed(p, &how, k)?;
                    let id = fresh(m);
                    q.id = id;
                    q.name = format!("{} copy {k}", p.name);
                    created.push(id);
                    let sk: Vec<Shape> = m.shapes.iter().filter(|s| s.part == p.id).cloned().collect();
                    for mut s in sk {
                        s.id = fresh(m);
                        s.part = id;
                        created.push(s.id);
                        m.shapes.push(s);
                    }
                    m.parts.push(q);
                }
            }
        }
        Op::Cut { tool, from } => {
            use crate::solids::modelling::{local, rot};
            if tool == from {
                return Err("Pick another part to cut.".into());
            }
            let gone = "That part no longer exists.";
            let pt = m.parts.iter().find(|p| p.id == tool).cloned().ok_or(gone)?;
            let pf = m.parts.iter().find(|p| p.id == from).cloned().ok_or(gone)?;
            if pt.angle > 0.0 || pf.angle > 0.0 {
                return Err("A cut goes straight through an extruded part, along its depth: both parts must be extruded.".into());
            }
            let (a, b) = (rot(pt.turn), rot(pf.turn));
            let dot: f64 = (0..3).map(|i| a[i][2] * b[i][2]).sum();
            if dot.abs() < 1.0 - 1e-6 {
                return Err(format!("A cut goes through {} along its depth: sketch it on the part's front or back face, or on a plane parallel to its sketch.", pf.name));
            }
            let map = |q: [f64; 2]| {
                let l = local(&pf, pt.place([q[0], q[1], 0.0]));
                [l[0], l[1]]
            };
            let cuts: Vec<Shape> = m.shapes.iter().filter(|s| s.part == tool && !s.hole && s.plate.is_none()).cloned().collect();
            if cuts.is_empty() {
                return Err(format!("{} has nothing in its sketch to cut with.", pt.name));
            }
            for s in cuts {
                let mut geom = s.geom.clone();
                match &mut geom {
                    Geom::Poly { pts, bulge } => {
                        pts.iter_mut().for_each(|p| *p = map(*p));
                        // Seen from the other side, the arcs bulge the other way.
                        if dot < 0.0 {
                            bulge.iter_mut().for_each(|b| *b = -*b);
                        }
                    }
                    Geom::Circle { c, .. } => *c = map(*c),
                }
                // Inside an outline of the part: a hole. Across its edge: a notch, cut out of the
                // outline. Clear of it: refused.
                use crate::solids::modelling::contains;
                let pts = geom.outline(f64::MAX, 0).0;
                let size = pts.iter().fold(0.0_f64, |s, a| s.max(a[0].abs()).max(a[1].abs())).max(1e-9);
                let ring = geom.outline(size / 200.0, 64).0;
                let solid: Vec<Shape> = m.shapes.iter().filter(|s| s.part == from && !s.hole && s.plate.is_none()).cloned().collect();
                // A cut that only touches an edge (or another hole) leaves a sliver of no width.
                let ext = solid.iter().flat_map(|s| s.geom.outline(f64::MAX, 0).0).fold(0.0_f64, |e, q| e.max(q[0].abs()).max(q[1].abs()));
                let near = 1e-3 * ext.max(size);
                let others: Vec<&Shape> = m.shapes.iter().filter(|s| s.part == from && s.plate.is_none()).collect();
                if others.iter().any(|s| crate::solids::modelling::gap(&s.geom, &ring) < near) && !others.iter().any(|s| !s.hole && ring.iter().any(|q| !contains(&s.geom, *q)) && ring.iter().any(|q| contains(&s.geom, *q))) {
                    return Err(format!("The cut touches an edge of {} or one of its holes: move it in a little, or right across the edge for a notch.", pf.name));
                }
                let within = solid.iter().find(|s| ring.iter().all(|q| contains(&s.geom, *q)));
                let across = solid.iter().find(|s| ring.iter().any(|q| contains(&s.geom, *q)) || s.geom.outline(f64::MAX, 0).0.iter().any(|q| contains(&geom, *q)));
                let id = fresh(m);
                let name = shape_name(m, within.is_some());
                match (within, across) {
                    (Some(_), _) => {
                        m.shapes.push(Shape { id, name, hole: true, material: pf.material, geom, plate: None, part: from });
                        created.push(id);
                    }
                    (None, Some(s)) => {
                        let a = s.id;
                        m.shapes.push(Shape { id, name, hole: false, material: pf.material, geom, plate: None, part: from });
                        exec(m, Op::Combine { a, b: id, how: How::Subtract }, created)?;
                    }
                    (None, None) => return Err(format!("The cut misses {}: draw it over the part.", pf.name)),
                }
            }
            exec(m, Op::Delete { ids: vec![tool] }, &mut vec![])?;
        }
        Op::Hole { part, at, d } => {
            use crate::solids::modelling::{contains, local};
            let p = m.parts.iter().find(|p| p.id == part).cloned().ok_or("That part no longer exists.")?;
            if p.angle > 0.0 {
                return Err("A hole goes straight through an extruded part; for a turned part, draw the groove in its sketch.".into());
            }
            if !(d.is_finite() && d > 0.0 && at.iter().all(|v| v.is_finite())) {
                return Err("A hole needs a diameter.".into());
            }
            let l = local(&p, at);
            let tol = 1e-6_f64.max(1e-4 * p.depth);
            if (l[2] - p.from).abs() > tol && (l[2] - p.from - p.depth).abs() > tol {
                return Err(format!("Click the front or back face of {}: a hole goes through along its depth.", p.name));
            }
            let c = [l[0], l[1]];
            let on = |hole: bool| m.shapes.iter().any(|s| s.part == part && s.hole == hole && s.plate.is_none() && contains(&s.geom, c));
            if !on(false) || on(true) {
                return Err("The hole's centre must be on the part.".into());
            }
            // The whole hole on the face: a hole that reaches the edge is a notch (draw it in the sketch).
            let r = d / 2.0;
            let rim: Vec<[f64; 2]> = (0..48).map(|k| { let t = k as f64 / 48.0 * std::f64::consts::TAU; [c[0] + r * t.cos(), c[1] + r * t.sin()] }).collect();
            let inside = m.shapes.iter().any(|s| s.part == part && !s.hole && s.plate.is_none() && rim.iter().all(|q| contains(&s.geom, *q)));
            let clear = !m.shapes.iter().any(|s| s.part == part && s.hole && s.plate.is_none() && (rim.iter().any(|q| contains(&s.geom, *q)) || s.geom.outline(f64::MAX, 0).0.iter().any(|q| (q[0] - c[0]).hypot(q[1] - c[1]) < r)));
            let ext = m.shapes.iter().filter(|s| s.part == part && s.plate.is_none()).flat_map(|s| s.geom.outline(f64::MAX, 0).0).fold(0.0_f64, |e, q| e.max(q[0].abs()).max(q[1].abs()));
            let touches = m.shapes.iter().filter(|s| s.part == part && s.plate.is_none()).any(|s| crate::solids::modelling::gap(&s.geom, &rim) < 1e-3 * ext.max(r));
            if !inside || !clear || touches {
                return Err("The hole reaches the face's edge or another hole: make it smaller, or move it.".into());
            }
            let id = fresh(m);
            let name = shape_name(m, true);
            m.shapes.push(Shape { id, name, hole: true, material: p.material, geom: Geom::Circle { c, r: d / 2.0 }, plate: None, part });
            created.push(id);
        }
        Op::PartEdge { part, near, size, round } => {
            let p = m.parts.iter().find(|p| p.id == part).cloned().ok_or("That part no longer exists.")?;
            let q = crate::solids::modelling::sketch_point(&p, near);
            let mut best: Option<(f64, u32, u32)> = None;
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for s in m.shapes.iter().filter(|s| s.part == part && s.plate.is_none()) {
                if let Geom::Poly { pts, .. } = &s.geom {
                    for (i, c) in pts.iter().enumerate() {
                        for k in 0..2 {
                            lo[k] = lo[k].min(c[k]);
                            hi[k] = hi[k].max(c[k]);
                        }
                        let dd = (c[0] - q[0]).hypot(c[1] - q[1]);
                        if best.is_none_or(|b| dd < b.0) {
                            best = Some((dd, s.id, i as u32));
                        }
                    }
                }
            }
            let (dd, shape, corner) = best.ok_or(format!("{} has no corners to round: its sketch is circles only.", p.name))?;
            if dd > 0.2 * (hi[0] - lo[0]).hypot(hi[1] - lo[1]) {
                return Err("Click nearer an edge along the part's depth (where two of its side faces meet).".into());
            }
            round_corners(m, shape, vec![corner], size, round)?;
        }
        Op::Batch(ops) => {
            for op in ops {
                exec(m, op, created)?;
            }
        }
    }
    Ok(())
}

/// The next free name for a new shape or hole ("Shape 3", "Hole 2").
fn shape_name(m: &Model, hole: bool) -> String {
    format!("{} {}", if hole { "Hole" } else { "Shape" }, m.shapes.iter().filter(|s| s.hole == hole).count() + 1)
}

/// Round or bevel corners of a polygon, highest first (each adds a corner after itself), moving
/// supports and loads to the renumbered edges.
fn round_corners(m: &mut Model, shape: u32, mut cs: Vec<u32>, size: f64, round: bool) -> Result<(), String> {
    cs.sort_unstable();
    cs.dedup();
    if cs.is_empty() {
        return Err("Pick the corners first.".into());
    }
    for &i in cs.iter().rev() {
        let sh = m.shapes.iter_mut().find(|x| x.id == shape).ok_or("That shape no longer exists.")?;
        sh.geom = crate::sketch::corner(&sh.geom, i as usize, size, round)?;
        renumber(m, shape, |e| vec![if e < i { e } else { e + 1 }]);
    }
    Ok(())
}

/// A load on a turned copy: in-plane forces turn with it.
fn turn_load(k: LoadKind, ang: f64) -> LoadKind {
    let (s, c) = ang.sin_cos();
    let t = |x: f64, y: f64| (c * x - s * y, s * x + c * y);
    match k {
        LoadKind::Traction { fx, fy, total, fz } => {
            let (fx, fy) = t(fx, fy);
            LoadKind::Traction { fx, fy, total, fz }
        }
        LoadKind::Bearing { fx, fy } => {
            let (fx, fy) = t(fx, fy);
            LoadKind::Bearing { fx, fy }
        }
        k => k,
    }
}

fn is_heat(k: &crate::model::SupportKind) -> bool {
    matches!(k, crate::model::SupportKind::Temperature { .. })
}

/// Remove what hangs on deleted items: members on missing nodes, supports and loads on missing
/// nodes, members or edges (also edges a shape no longer has).
/// Move supports, loads and local sizes on the edges of `shape` to the edges `f` gives (several
/// when an edge was split); targets that land twice on one edge are kept once.
fn renumber(m: &mut Model, shape: u32, f: impl Fn(u32) -> Vec<u32>) {
    retarget(m, |t| match t {
        Target::Edge { shape: s, edge } if *s == shape => Some(f(*edge).into_iter().map(|e| Target::Edge { shape, edge: e }).collect()),
        Target::Line { shape: s, edge, at } if *s == shape => Some(f(*edge).into_iter().map(|e| Target::Line { shape, edge: e, at: *at }).collect()),
        _ => None,
    });
}

/// Move supports, loads and local sizes to the targets `f` gives (None: left where it is; an
/// empty list: removed). A total edge load is shared between its new edges.
fn retarget(m: &mut Model, f: impl Fn(&Target) -> Option<Vec<Target>>) {
    let map = |t: &Target| -> Vec<Target> { f(t).unwrap_or_else(|| vec![t.clone()]) };
    let mut sups = vec![];
    for x in std::mem::take(&mut m.supports) {
        for (k, at) in map(&x.at).into_iter().enumerate() {
            if sups.iter().any(|s: &crate::model::Support| s.at == at && s.kind == x.kind) {
                continue;
            }
            let id = if k == 0 { x.id } else { fresh(m) };
            sups.push(crate::model::Support { id, at, kind: x.kind.clone() });
        }
    }
    m.supports = sups;
    let mut loads = vec![];
    for x in std::mem::take(&mut m.loads) {
        let ats = map(&x.at);
        let n = ats.len();
        for (k, at) in ats.into_iter().enumerate() {
            if loads.iter().any(|s: &crate::model::Load| s.at == at && s.kind == x.kind) {
                continue;
            }
            let mut kind = x.kind.clone();
            // A total edge force is shared between the halves.
            if let LoadKind::Traction { fx, fy, total: true, fz } = kind {
                kind = LoadKind::Traction { fx: fx / n as f64, fy: fy / n as f64, total: true, fz: fz / n as f64 };
            }
            if let LoadKind::Bearing { fx, fy } = kind {
                kind = LoadKind::Bearing { fx: fx / n as f64, fy: fy / n as f64 };
            }
            let id = if k == 0 { x.id } else { fresh(m) };
            loads.push(crate::model::Load { id, at, kind, case: x.case });
        }
    }
    m.loads = loads;
    let refine = std::mem::take(&mut m.mesh.refine);
    for r in refine {
        for at in map(&r.at) {
            if !m.mesh.refine.iter().any(|x| x.at == at) {
                m.mesh.refine.push(crate::model::Refine { at, size: r.size });
            }
        }
    }
}

/// A uniform grid of buckets for finding nodes and members near a point quickly.
struct Grid {
    cell: f64,
    map: std::collections::HashMap<(i64, i64), Vec<u32>>,
}
impl Grid {
    fn new(m: &Model) -> Grid {
        let span = m.bounds().map(|b| (b[2] - b[0]).max(b[3] - b[1])).unwrap_or(1.0).max(1e-6);
        let n = (m.nodes.len() as f64).sqrt().max(1.0);
        Grid { cell: span / n, map: Default::default() }
    }
    fn key(&self, x: f64, y: f64) -> (i64, i64) {
        ((x / self.cell).floor() as i64, (y / self.cell).floor() as i64)
    }
    fn add_box(&mut self, id: u32, a: (f64, f64), b: (f64, f64)) {
        let (k0, k1) = (self.key(a.0.min(b.0) - SAME, a.1.min(b.1) - SAME), self.key(a.0.max(b.0) + SAME, a.1.max(b.1) + SAME));
        // Very long members in a fine grid: cap the work (they are then found by the slow path).
        if (k1.0 - k0.0 + 1) * (k1.1 - k0.1 + 1) > 20_000 {
            self.map.entry((i64::MIN, i64::MIN)).or_default().push(id);
            return;
        }
        for x in k0.0..=k1.0 {
            for y in k0.1..=k1.1 {
                self.map.entry((x, y)).or_default().push(id);
            }
        }
    }
    fn near(&self, x: f64, y: f64) -> Vec<u32> {
        let k = self.key(x, y);
        let mut v: Vec<u32> = self.map.get(&k).cloned().unwrap_or_default();
        v.extend(self.map.get(&(i64::MIN, i64::MIN)).cloned().unwrap_or_default());
        v
    }
}

/// Merge every node that shares its place with another (repairing old or damaged files).
pub(crate) fn merge_all(m: &mut Model) {
    let ids: Vec<u32> = m.nodes.iter().map(|n| n.id).collect();
    merge_nodes(m, &ids);
}

/// Merge each of `nodes` into a node already at its place: members, supports and loads follow it.
fn merge_nodes(m: &mut Model, nodes: &[u32]) {
    if nodes.is_empty() {
        return;
    }
    let mut grid = Grid::new(m);
    for n in &m.nodes {
        grid.add_box(n.id, (n.x, n.y), (n.x, n.y));
    }
    let pos: std::collections::HashMap<u32, [f64; 3]> = m.nodes.iter().map(|n| (n.id, n.at())).collect();
    let mut into: std::collections::HashMap<u32, u32> = Default::default();
    for &t in nodes {
        let Some(&p0) = pos.get(&t) else { continue };
        if into.contains_key(&t) {
            continue;
        }
        if let Some(o) = grid.near(p0[0], p0[1]).into_iter().find(|o| *o != t && !into.contains_key(o) && pos.get(o).is_some_and(|p| dist3(*p, p0) < SAME)) {
            into.insert(t, o);
        }
    }
    if into.is_empty() {
        return;
    }
    let to = |id: u32| *into.get(&id).unwrap_or(&id);
    for b in m.members.iter_mut() {
        b.a = to(b.a);
        b.b = to(b.b);
    }
    // Members that shrank to a point, or now repeat another, go.
    let mut pairs = HashSet::new();
    m.members.retain(|b| b.a != b.b && pairs.insert((b.a.min(b.b), b.a.max(b.b))));
    for t in m.supports.iter_mut().map(|s| &mut s.at).chain(m.loads.iter_mut().map(|l| &mut l.at)) {
        if let Target::Node { id } = t {
            *id = to(*id);
        }
    }
    let mut seen = HashSet::new();
    let heat = |k: &crate::model::SupportKind| matches!(k, crate::model::SupportKind::Temperature { .. });
    m.supports.retain(|s| !matches!(s.at, Target::Node { .. }) || seen.insert((format!("{:?}", s.at), heat(&s.kind))));
    m.nodes.retain(|n| !into.contains_key(&n.id));
}

/// Split members at nodes lying along them: every node in `nodes` against all members, and every
/// member in `members` against all nodes. Distributed loads are split with them.
fn join(m: &mut Model, nodes: &[u32], members: &[u32]) {
    let mut queue: Vec<(u32, u32)> = vec![]; // (member, node)
    let pos: std::collections::HashMap<u32, [f64; 3]> = m.nodes.iter().map(|n| (n.id, n.at())).collect();
    let on = |b: &Member, n: u32| -> Option<f64> {
        if b.a == n || b.b == n {
            return None;
        }
        let (a, c, p) = (pos.get(&b.a)?, pos.get(&b.b)?, pos.get(&n)?);
        let d = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let l2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1] + (p[2] - a[2]) * d[2]) / l2;
        let l = l2.sqrt();
        if t * l <= SAME || (1.0 - t) * l <= SAME {
            return None;
        }
        (dist3([a[0] + t * d[0], a[1] + t * d[1], a[2] + t * d[2]], *p) < SAME).then_some(t)
    };
    if !nodes.is_empty() {
        let mut grid = Grid::new(m);
        for b in &m.members {
            if let (Some(a), Some(c)) = (pos.get(&b.a), pos.get(&b.b)) {
                grid.add_box(b.id, (a[0], a[1]), (c[0], c[1]));
            }
        }
        for &n in nodes {
            let Some(&p) = pos.get(&n) else { continue };
            for id in grid.near(p[0], p[1]) {
                if let Some(b) = m.members.iter().find(|b| b.id == id) {
                    if on(b, n).is_some() {
                        queue.push((id, n));
                    }
                }
            }
        }
    }
    if !members.is_empty() {
        let mut grid = Grid::new(m);
        for n in &m.nodes {
            grid.add_box(n.id, (n.x, n.y), (n.x, n.y));
        }
        for &id in members {
            let Some(b) = m.members.iter().find(|b| b.id == id) else { continue };
            let (Some(a), Some(c)) = (pos.get(&b.a), pos.get(&b.b)) else { continue };
            // Nodes in the cells along the member (the grid is in x and y; depth is checked by `on`).
            let steps = (((c[0] - a[0]).abs().max((c[1] - a[1]).abs()) / grid.cell).ceil() as usize).clamp(1, 100_000);
            let mut cand = HashSet::new();
            for k in 0..=steps {
                let t = k as f64 / steps as f64;
                let (x, y) = (a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t);
                for dx in [-1.0, 0.0, 1.0] {
                    for dy in [-1.0, 0.0, 1.0] {
                        cand.extend(grid.near(x + dx * grid.cell, y + dy * grid.cell));
                    }
                }
            }
            for n in cand {
                if on(b, n).is_some() {
                    queue.push((id, n));
                }
            }
        }
    }
    // Split, nearest node first along each member, re-finding which piece holds each node.
    queue.sort_unstable();
    queue.dedup();
    for (_, n) in queue {
        let Some(i) = m.members.iter().position(|b| on(b, n).is_some()) else { continue };
        let t = on(&m.members[i], n).unwrap();
        let b = m.members[i].clone();
        // Where the cut is along the old member (m from end a), for part-length and point loads.
        let cut = t * m.member_length(b.id).unwrap_or(0.0);
        let nid = fresh(m);
        m.members[i].b = n;
        // A hinge at the far end goes with the far piece.
        m.members[i].hinge[1] = false;
        m.members[i].offset[1] = 0.0;
        m.members.push(Member { id: nid, a: n, b: b.b, kind: b.kind, section: b.section, material: b.material, roll: b.roll, hinge: [false, b.hinge[1]], only: b.only, offset: [0.0, b.offset[1]] });
        let mut extra = vec![];
        for l in m.loads.iter_mut() {
            if l.at != (Target::Member { id: b.id }) {
                continue;
            }
            match &mut l.kind {
                LoadKind::Dist { w1, w2, dir } => {
                    let wt = *w1 + (*w2 - *w1) * t;
                    extra.push((LoadKind::Dist { w1: wt, w2: *w2, dir: *dir }, nid, l.case));
                    *w2 = wt;
                }
                LoadKind::Patch { w1, w2, dir, from, to, projected } if *to > cut => {
                    if *from >= cut {
                        // Wholly on the far piece: it moves there.
                        *from -= cut;
                        *to -= cut;
                        l.at = Target::Member { id: nid };
                    } else {
                        let wt = *w1 + (*w2 - *w1) * (cut - *from) / (*to - *from);
                        extra.push((LoadKind::Patch { w1: wt, w2: *w2, dir: *dir, from: 0.0, to: *to - cut, projected: *projected }, nid, l.case));
                        *w2 = wt;
                        *to = cut;
                    }
                }
                LoadKind::Concentrated { at, .. } if *at > cut => {
                    *at -= cut;
                    l.at = Target::Member { id: nid };
                }
                _ => {}
            }
        }
        for (kind, mid, case) in extra {
            let id = fresh(m);
            m.loads.push(crate::model::Load { id, at: Target::Member { id: mid }, kind, case });
        }
    }
}

fn prune(m: &mut Model) {
    let nodes: HashSet<u32> = m.nodes.iter().map(|n| n.id).collect();
    m.members.retain(|mb| nodes.contains(&mb.a) && nodes.contains(&mb.b));
    let members: HashSet<u32> = m.members.iter().map(|n| n.id).collect();
    let edges: std::collections::HashMap<u32, usize> = m.shapes.iter().map(|s| (s.id, s.geom.edges())).collect();
    let alive = |t: &Target| match t {
        Target::Node { id } => nodes.contains(id),
        Target::Member { id } => members.contains(id),
        Target::Edge { shape, edge } | Target::Line { shape, edge, .. } => edges.get(shape).is_some_and(|n| (*edge as usize) < *n),
        Target::Plate { shape } => edges.contains_key(shape),
        Target::Point { .. } | Target::Body | Target::End { .. } => true,
        Target::Face { face } => m.volume.as_ref().is_some_and(|v| (*face as usize) < v.faces.len()),
        Target::PartEnd { part, .. } => m.parts.iter().any(|p| p.id == *part),
    };
    m.supports.retain(|s| alive(&s.at));
    m.loads.retain(|s| alive(&s.at));
    m.mesh.refine.retain(|s| alive(&s.at));
    m.contacts.retain(|c| alive(&c.a) && alive(&c.b));
    // A design member whose end nodes are gone goes too; one whose chain broke stays for the model
    // check to point out.
    m.design_members.retain(|x| nodes.contains(&x.a) && nodes.contains(&x.b));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dir, SupportKind};

    /// A 6 m beam on a pin and a roller with a tapering load, a point load and a part load, and
    /// (for `local`) a load across the member; what a solve says about it.
    fn loaded_beam(local: bool) -> Doc {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [6.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        let (n0, n1, id) = (d.m.nodes[0].id, d.m.nodes[1].id, d.m.members[0].id);
        for (n, kind) in [(n0, SupportKind::Pinned), (n1, SupportKind::RollerX)] {
            d.apply(Op::Add { item: Item::Support(crate::model::Support { id: 0, at: Target::Node { id: n }, kind }) }).unwrap();
        }
        let dir = if local { Dir::Local } else { Dir::Y };
        for kind in [LoadKind::Dist { w1: -2e3, w2: -5e3, dir }, LoadKind::Concentrated { f: -8e3, dir, at: 4.0 }, LoadKind::Patch { w1: -1e3, w2: -3e3, dir, from: 1.0, to: 2.5, projected: false }] {
            d.apply(Op::Add { item: Item::Load(crate::model::Load { case: 0, id: 0, at: Target::Member { id }, kind }) }).unwrap();
        }
        d
    }
    fn answer(d: &Doc) -> (f64, f64, f64) {
        let r = crate::frame::solve(&d.m).unwrap();
        let ra = r.reactions.iter().find(|x| d.m.node(x.node).is_some_and(|n| n.x == 0.0)).unwrap().fy;
        // The deflection under the point load, 4 m along: a station in every piece layout.
        let dmax = r.members.iter().zip(&d.m.members).find_map(|(mr, b)| {
            let (xa, xb) = (d.m.node(b.a)?.x, d.m.node(b.b)?.x);
            mr.x.iter().position(|s| (xa + (xb - xa) * s / mr.length - 4.0).abs() < 1e-9).map(|i| mr.d[i][1])
        }).unwrap();
        let mmax = r.members.iter().flat_map(|m| m.m.iter().copied()).fold(0.0f64, |a, v| a.max(v.abs()));
        (ra, dmax, mmax)
    }
    fn same(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
        let c = |x: f64, y: f64| (x - y).abs() <= 1e-6 * x.abs().max(y.abs()).max(1e-12);
        c(a.0, b.0) && c(a.1, b.1)
    }

    #[test]
    fn splitting_reversing_and_merging_members_keep_the_answer() {
        for local in [false, true] {
            let d0 = loaded_beam(local);
            let a0 = answer(&d0);
            let id = d0.m.members[0].id;
            // Split in three: the loads follow the pieces.
            let mut d = d0.clone();
            d.apply(Op::SplitMember { ids: vec![id], at: vec![1.0 / 3.0, 2.0 / 3.0] }).unwrap();
            assert_eq!(d.m.members.len(), 3);
            assert!(same(a0, answer(&d)), "split, local {local}: {a0:?} vs {:?}", answer(&d));
            // Reverse the middle piece and the first: still the same structure and loads.
            let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).take(2).collect();
            d.apply(Op::ReverseMember { ids }).unwrap();
            assert!(same(a0, answer(&d)), "reversed, local {local}: {a0:?} vs {:?}", answer(&d));
            // Merge them back into one member, two at a time.
            for _ in 0..2 {
                let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).take(2).collect();
                let ok = d.apply(Op::MergeMembers { ids: ids.clone() });
                if ok.is_err() {
                    let ids = vec![d.m.members[0].id, d.m.members[2].id];
                    d.apply(Op::MergeMembers { ids }).unwrap();
                }
            }
            assert_eq!((d.m.members.len(), d.m.nodes.len()), (1, 2));
            assert!(same(a0, answer(&d)), "merged, local {local}: {a0:?} vs {:?}", answer(&d));
            // Reversing the whole member twice is no change at all.
            let mut e = d0.clone();
            e.apply(Op::ReverseMember { ids: vec![id] }).unwrap();
            assert!(same(a0, answer(&e)));
            e.apply(Op::ReverseMember { ids: vec![id] }).unwrap();
            assert_eq!(e.m.loads, d0.m.loads);
        }
    }

    #[test]
    fn merging_refuses_what_would_change_the_structure() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [3.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        d.apply(Op::AddMember { from: [3.0, 0.0], to: [5.0, 2.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
        assert!(d.apply(Op::MergeMembers { ids: ids.clone() }).unwrap_err().contains("not in line"));
        let mut d = Doc::new(Model::new(Analysis::Frame));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [3.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        d.apply(Op::AddMember { from: [3.0, 0.0], to: [6.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        let mid = d.m.nodes.iter().find(|n| n.x == 3.0).unwrap().id;
        d.apply(Op::Add { item: Item::Support(crate::model::Support { id: 0, at: Target::Node { id: mid }, kind: SupportKind::Pinned }) }).unwrap();
        let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
        assert!(d.apply(Op::MergeMembers { ids }).unwrap_err().contains("has to stay"));
    }

    #[test]
    fn members_turn_to_an_angle_a_point_or_a_direction() {
        let mut d = Doc::new(Model::new(Analysis::Space));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [4.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        let id = d.m.members[0].id;
        let ly = |d: &Doc| { let b = &d.m.members[0]; crate::space::axes(d.m.node(b.a).unwrap().at(), d.m.node(b.b).unwrap().at(), b.roll)[1] };
        // Facing along +z (the depth): local y turns to +z.
        d.apply(Op::Orient { ids: vec![id], angle: None, towards: None, along: Some([0.0, 0.0, 1.0]) }).unwrap();
        let y = ly(&d);
        assert!((y[2] - 1.0).abs() < 1e-12, "{y:?}");
        // Facing a point below and behind the middle: local y points at it, square to the member.
        d.apply(Op::Orient { ids: vec![id], angle: None, towards: Some([2.0, -3.0, -3.0]), along: None }).unwrap();
        let y = ly(&d);
        let s = 1.0 / 2.0f64.sqrt();
        assert!((y[0]).abs() < 1e-12 && (y[1] + s).abs() < 1e-12 && (y[2] + s).abs() < 1e-12, "{y:?}");
        // An angle, kept between −180° and 180°.
        d.apply(Op::Orient { ids: vec![id], angle: Some(3.0 * std::f64::consts::PI), towards: None, along: None }).unwrap();
        assert!((d.m.members[0].roll.abs() - std::f64::consts::PI).abs() < 1e-12);
        assert!(d.apply(Op::Orient { ids: vec![id], angle: Some(1.0), towards: Some([0.0; 3]), along: None }).is_err());
    }

    #[test]
    fn making_combinations_again_replaces_the_made_ones_and_keeps_the_users() {
        use crate::model::{Case, CaseKind, Combo, ComboRule, Factor};
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let g = d.apply(Op::Add { item: Item::Case(Case { name: "Dead".into(), ..Default::default() }) }).unwrap()[0];
        let q = d.apply(Op::Add { item: Item::Case(Case { name: "Imposed".into(), kind: CaseKind::Variable, ..Default::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Combo(Combo { id: 0, name: "Mine".into(), parts: vec![Factor { case: g, f: 2.0 }], auto: false }) }).unwrap();
        let rule = ComboRule { g_unfav: 1.2, q: 1.5, ..Default::default() };
        let made = d.apply(Op::CombosByRule { rule: rule.clone() }).unwrap();
        assert_eq!(made.len(), 2, "strength and service, one variable case each");
        assert_eq!(d.m.combos.len(), 3);
        let again = d.apply(Op::CombosByRule { rule: ComboRule { long_term: true, ..rule } }).unwrap();
        assert_eq!(again.len(), 3);
        assert_eq!(d.m.combos.len(), 4, "the first made ones went; Mine stays");
        assert!(d.m.combos.iter().any(|c| c.name == "Mine" && !c.auto));
        assert!(d.m.combo_rule.as_ref().is_some_and(|r| r.long_term));
        let s = d.m.combos.iter().find(|c| c.name == "Strength: Imposed leading").unwrap();
        assert!(s.parts.iter().any(|p| p.case == q && (p.f - 1.5).abs() < 1e-12));
    }

    #[test]
    fn nodes_closer_than_the_tolerance_merge() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [3.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        d.apply(Op::AddMember { from: [3.003, 0.0], to: [3.0, 2.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert_eq!(d.m.nodes.len(), 4);
        assert!(d.apply(Op::MergeNodes { ids: vec![], tol: 0.001 }).is_err() || d.m.nodes.len() == 4);
        d.apply(Op::MergeNodes { ids: vec![], tol: 0.005 }).unwrap();
        assert_eq!(d.m.nodes.len(), 3);
        assert!(d.m.members.iter().all(|b| d.m.node(b.a).is_some() && d.m.node(b.b).is_some()));
    }

    #[test]
    fn members_share_nodes_and_delete_cascades() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let c = d.apply(Op::AddMember { from: [0.0, 0.0], to: [0.0, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert_eq!(c.len(), 3);
        let c2 = d.apply(Op::AddMember { from: [0.0, 3.0], to: [4.0, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert_eq!(c2.len(), 2, "the corner node is shared");
        assert_eq!(d.m.nodes.len(), 3);
        let top = c[1];
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Node { id: top }, kind: LoadKind::Force { fx: 1e3, fy: 0.0, fz: 0.0 } }) }).unwrap();
        d.apply(Op::Delete { ids: vec![top] }).unwrap();
        assert!(d.m.members.is_empty() && d.m.loads.is_empty());
        assert!(d.undo());
        assert_eq!(d.m.members.len(), 2);
    }

    #[test]
    fn undoing_back_to_the_saved_model_is_unchanged() {
        // 0.1.1 review: undo to the saved state still marked the model changed.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        assert!(!d.dirty);
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [1.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert!(d.dirty);
        d.undo();
        assert!(!d.dirty, "back to the opened model");
        d.redo();
        d.mark_saved();
        d.apply(Op::Move { ids: vec![d.m.nodes[0].id], dx: 0.5, dy: 0.0, dz: 0.0 }).unwrap();
        assert!(d.dirty);
        d.undo();
        assert!(!d.dirty, "back to the saved model");
        d.undo();
        assert!(d.dirty, "before the save");
        d.mark_unsaved();
        d.redo();
        assert!(d.dirty, "recovered work stays changed");
    }

    #[test]
    fn crossing_members_join_only_when_asked() {
        // An X: two diagonals of a 4 × 3 panel, with a load along one of them.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [4.0, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        d.apply(Op::AddMember { from: [0.0, 3.0], to: [4.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert_eq!((d.m.nodes.len(), d.m.members.len()), (4, 2), "drawn apart, as X-bracing usually is");
        let first = d.m.members[0].id;
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Member { id: first }, kind: LoadKind::Dist { w1: -1e3, w2: -3e3, dir: crate::model::Dir::Y } }) }).unwrap();
        let ids: Vec<u32> = d.m.members.iter().map(|b| b.id).collect();
        let made = d.apply(Op::JoinCrossings { ids }).unwrap();
        assert_eq!(made.len(), 1);
        assert_eq!((d.m.nodes.len(), d.m.members.len()), (5, 4));
        let c = d.m.nodes.iter().find(|n| n.id == made[0]).unwrap();
        assert!((c.x - 2.0).abs() < 1e-12 && (c.y - 1.5).abs() < 1e-12);
        // The load is split at the crossing, linearly: −1 → −2 → −3 kN/m.
        let mut ws: Vec<(f64, f64)> = d.m.loads.iter().filter_map(|l| if let LoadKind::Dist { w1, w2, .. } = l.kind { Some((w1, w2)) } else { None }).collect();
        ws.sort_by(|a, b| b.0.total_cmp(&a.0));
        assert_eq!(ws.len(), 2);
        assert!((ws[0].0 + 1e3).abs() < 1e-9 && (ws[0].1 + 2e3).abs() < 1e-9 && (ws[1].1 + 3e3).abs() < 1e-9, "{ws:?}");
        // Members that only touch at an end, or are parallel, do not cross.
        let mut e = Doc::new(Model::new(Analysis::Frame));
        e.apply(Op::AddMember { from: [0.0, 0.0], to: [1.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        e.apply(Op::AddMember { from: [0.0, 1.0], to: [1.0, 1.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        let ids: Vec<u32> = e.m.members.iter().map(|b| b.id).collect();
        assert!(e.apply(Op::JoinCrossings { ids }).unwrap_err().contains("cross"));
    }

    #[test]
    fn a_support_replaces_the_one_already_there() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let c = d.apply(Op::AddMember { from: [0.0, 0.0], to: [5.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
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
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: LoadKind::Pressure { p: 1e6 } }) }).unwrap();
        let mut sh = d.m.shape(s).unwrap().clone();
        sh.geom = Geom::Poly { pts: vec![[0.0, 0.0], [0.1, 0.0], [0.0, 0.1]], bulge: vec![0.0; 3] };
        d.apply(Op::Set { item: Item::Shape(sh) }).unwrap();
        assert!(d.m.loads.is_empty());
        assert_eq!(d.m.shapes[0].name, "Shape 1");
    }

    fn member(d: &mut Doc, a: [f64; 2], b: [f64; 2]) {
        d.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
    }
    fn node_id(d: &Doc, x: f64, y: f64) -> u32 {
        d.m.nodes.iter().find(|n| (n.x - x).abs() < 1e-9 && (n.y - y).abs() < 1e-9).unwrap().id
    }

    #[test]
    fn a_member_drawn_to_the_middle_of_another_joins_it() {
        // 0.1.1 review: a column drawn from the middle of a beam was not connected to it.
        let mut d = Doc::new(Model::new(Analysis::Frame));
        member(&mut d, [0.0, 0.0], [6.0, 0.0]);
        let beam = d.m.members[0].id;
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Member { id: beam }, kind: LoadKind::Dist { w1: -2.0, w2: -8.0, dir: crate::model::Dir::Y } }) }).unwrap();
        member(&mut d, [3.0, 0.0], [3.0, -3.0]);
        assert_eq!(d.m.members.len(), 3, "the beam is split at the column");
        let mid = node_id(&d, 3.0, 0.0);
        assert_eq!(d.m.members.iter().filter(|b| b.a == mid || b.b == mid).count(), 3);
        // The linearly varying load is split at the same point: −2 → −5 and −5 → −8.
        let mut ws: Vec<(f64, f64)> = d.m.loads.iter().map(|l| match l.kind { LoadKind::Dist { w1, w2, .. } => (w1, w2), _ => (0.0, 0.0) }).collect();
        ws.sort_by(|a, b| b.0.total_cmp(&a.0));
        assert_eq!(ws, vec![(-2.0, -5.0), (-5.0, -8.0)]);
        // And a long member drawn across existing nodes is split at each.
        let mut e = Doc::new(Model::new(Analysis::Frame));
        for x in [1.0, 2.0, 3.0] {
            e.apply(Op::Add { item: Item::Node(Node { id: 0, x, y: 0.0, z: 0.0 }) }).unwrap();
        }
        member(&mut e, [0.0, 0.0], [4.0, 0.0]);
        assert_eq!(e.m.members.len(), 4);
        e.m.check().unwrap();
    }

    #[test]
    fn nodes_moved_onto_others_merge() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        member(&mut d, [0.0, 0.0], [2.0, 0.0]);
        member(&mut d, [3.0, 0.0], [3.0, 2.0]);
        // Dragging the column's foot onto the beam's end joins them.
        let foot = node_id(&d, 3.0, 0.0);
        d.apply(Op::Move { ids: vec![foot], dx: -1.0, dy: 0.0, dz: 0.0 }).unwrap();
        assert_eq!(d.m.nodes.len(), 3);
        // Setting a node's coordinates in the Inspector onto another node joins them too.
        let top = node_id(&d, 3.0, 2.0);
        d.apply(Op::Set { item: Item::Node(Node { id: top, x: 0.0, y: 0.0, z: 0.0 }) }).unwrap();
        assert_eq!(d.m.nodes.len(), 2);
        d.m.check().unwrap();
        // 0.1.1 review: arrow-key moves on a 0.1 grid (0.1 + 0.2 ≠ 0.3) with another node in between.
        let mut e = Doc::new(Model::new(Analysis::Frame));
        for (x, y) in [(0.1, 0.0), (0.3, 0.0), (0.3, 2.0)] {
            e.apply(Op::Add { item: Item::Node(Node { id: 0, x, y, z: 0.0 }) }).unwrap();
        }
        let a = node_id(&e, 0.1, 0.0);
        e.apply(Op::Move { ids: vec![a], dx: 0.2, dy: 0.0, dz: 0.0 }).unwrap();
        assert_eq!(e.m.nodes.len(), 2);
    }

    #[test]
    fn corners_added_and_removed_keep_loads_on_their_edges() {
        let mut d = Doc::new(Model::new(Analysis::Stress));
        let s = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.1, 0.05), ..Shape::default() }) }).unwrap()[0];
        // Fixed on the left edge (3), pressure on the right edge (1).
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: SupportKind::Fixed }) }).unwrap();
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: 1e6 } }) }).unwrap();
        // A corner in the bottom edge (0): the right edge becomes 2, the left 4.
        d.apply(Op::InsertCorner { shape: s, edge: 0 }).unwrap();
        assert_eq!(d.m.loads[0].at, Target::Edge { shape: s, edge: 2 });
        assert_eq!(d.m.supports[0].at, Target::Edge { shape: s, edge: 4 });
        // A corner in the right edge: the pressure covers both halves.
        d.apply(Op::InsertCorner { shape: s, edge: 2 }).unwrap();
        assert_eq!(d.m.loads.iter().map(|l| l.at.clone()).collect::<Vec<_>>(), vec![Target::Edge { shape: s, edge: 2 }, Target::Edge { shape: s, edge: 3 }]);
        assert_eq!(d.m.supports[0].at, Target::Edge { shape: s, edge: 5 });
        // Removing them again brings everything back where it was.
        d.apply(Op::RemoveCorner { shape: s, corner: 3 }).unwrap();
        d.apply(Op::RemoveCorner { shape: s, corner: 1 }).unwrap();
        assert_eq!(d.m.loads.len(), 1);
        assert_eq!(d.m.loads[0].at, Target::Edge { shape: s, edge: 1 });
        assert_eq!(d.m.supports[0].at, Target::Edge { shape: s, edge: 3 });
        // Removing corner 0 joins the last edge and the first.
        d.apply(Op::RemoveCorner { shape: s, corner: 0 }).unwrap();
        assert_eq!(d.m.supports[0].at, Target::Edge { shape: s, edge: 2 });
        assert_eq!(d.m.loads[0].at, Target::Edge { shape: s, edge: 0 });
        assert!(d.apply(Op::RemoveCorner { shape: s, corner: 0 }).is_err(), "a triangle keeps its corners");
        // A corner in an arc keeps the arc: the area does not change.
        let mut e = Doc::new(Model::new(Analysis::Stress));
        let c = e.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::Poly { pts: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], bulge: vec![0.0, 0.0, 0.5, 0.0] }, ..Shape::default() }) }).unwrap()[0];
        let a0 = e.m.shapes[0].geom.area();
        e.apply(Op::InsertCorner { shape: c, edge: 2 }).unwrap();
        assert!((e.m.shapes[0].geom.area() - a0).abs() < 1e-12, "{} vs {a0}", e.m.shapes[0].geom.area());
    }

    #[test]
    fn used_materials_stay() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        d.apply(Op::AddMember { from: [0.0, 0.0], to: [5.0, 0.0], kind: MemberKind::Truss, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert!(d.apply(Op::Delete { ids: vec![1] }).unwrap_err().contains("still used"));
        assert!(d.apply(Op::Delete { ids: vec![2] }).unwrap_err().contains("still used"));
    }

    #[test]
    fn repeat_builds_bays_in_3d_and_members_split_only_where_they_meet() {
        // A portal bay in the x-y plane, repeated twice 4 m along z and joined by beams.
        let mut d = Doc::new(Model::new(Analysis::Space));
        for (a, b) in [([0.0, 0.0], [0.0, 3.0]), ([0.0, 3.0], [6.0, 3.0]), ([6.0, 3.0], [6.0, 0.0])] {
            d.apply(Op::AddMember { from: a, to: b, kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        }
        let feet: Vec<u32> = d.m.nodes.iter().filter(|n| n.y == 0.0).map(|n| n.id).collect();
        for id in feet {
            d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Node { id }, kind: crate::model::SupportKind::Fixed }) }).unwrap();
        }
        let ids: Vec<u32> = d.m.members.iter().map(|m| m.id).collect();
        d.apply(Op::Repeat { ids, step: [0.0, 0.0, 4.0], times: 2, link: Some(MemberKind::Beam) }).unwrap();
        assert_eq!(d.m.nodes.len(), 12);
        assert_eq!(d.m.members.len(), 9 + 8, "three bays and the beams joining their four nodes twice");
        assert_eq!(d.m.supports.len(), 6);
        assert!(d.m.nodes.iter().any(|n| n.z == 8.0));
        // A member along z at the same x and y as a node of the first bay does not split it:
        // only nodes on the member in 3D count.
        let before = d.m.members.len();
        d.apply(Op::AddMember { from: [3.0, 3.0], to: [3.0, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [1.0, 3.0] }).unwrap();
        assert_eq!(d.m.members.len(), before + 1);
        // Two members crossing in space join where they meet, and not where they only pass.
        let mut e = Doc::new(Model::new(Analysis::Space));
        e.apply(Op::AddMember { from: [0.0, 0.0], to: [2.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [1.0, 1.0] }).unwrap();
        e.apply(Op::AddMember { from: [1.0, 0.0], to: [1.0, 0.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0, 2.0] }).unwrap();
        e.apply(Op::AddMember { from: [1.0, 1.0], to: [1.0, 1.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0, 2.0] }).unwrap();
        let ids: Vec<u32> = e.m.members.iter().map(|m| m.id).collect();
        e.apply(Op::JoinCrossings { ids }).unwrap();
        assert_eq!(e.m.members.len(), 5, "the two that meet split; the one passing above does not");
        // Nothing to repeat, or no step: said why.
        assert!(d.apply(Op::Repeat { ids: vec![], step: [0.0, 0.0, 1.0], times: 1, link: None }).unwrap_err().contains("Select"));
        let ids: Vec<u32> = d.m.members.iter().map(|m| m.id).collect();
        assert!(d.apply(Op::Repeat { ids, step: [0.0; 3], times: 1, link: None }).unwrap_err().contains("step"));
    }

    #[test]
    fn sketch_tools_keep_supports_and_loads_on_their_edges() {
        use crate::sketch::{How, Make};
        let mut d = Doc::new(Model::new(Analysis::Stress));
        let s = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::rect(0.0, 0.0, 0.1, 0.05), ..Shape::default() }) }).unwrap()[0];
        d.apply(Op::Add { item: Item::Support(Support { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: SupportKind::Fixed }) }).unwrap();
        d.apply(Op::Add { item: Item::Load(Load { case: 0, id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: 1e6 } }) }).unwrap();
        // Rounding the two right-hand corners (1 and 2): the right edge becomes 2, the left 5.
        d.apply(Op::Fillet { shape: s, corners: vec![2, 1], r: 0.01 }).unwrap();
        assert_eq!(d.m.shapes[0].geom.edges(), 6);
        assert_eq!(d.m.loads[0].at, Target::Edge { shape: s, edge: 2 });
        assert_eq!(d.m.supports[0].at, Target::Edge { shape: s, edge: 5 });
        assert!(d.apply(Op::Chamfer { shape: s, corners: vec![2], d: 0.001 }).unwrap_err().contains("straight"), "corner 2 is between the right edge and an arc");
        d.apply(Op::Chamfer { shape: s, corners: vec![0], d: 0.005 }).unwrap();
        assert_eq!((d.m.loads[0].at.clone(), d.m.supports[0].at.clone()), (Target::Edge { shape: s, edge: 3 }, Target::Edge { shape: s, edge: 6 }));
        // A slot cut into the right edge: the pressure stays on what is left of it (both pieces),
        // the support on the left edge; the slot shape is gone.
        let slot = d.apply(Op::Make { make: Make::Slot { a: [0.08, 0.025], b: [0.2, 0.025], w: 0.01 }, hole: false, material: None, part: 0 }).unwrap()[0];
        d.apply(Op::Combine { a: s, b: slot, how: How::Subtract }).unwrap();
        assert_eq!(d.m.shapes.len(), 1);
        let Geom::Poly { pts, .. } = &d.m.shapes[0].geom else { panic!() };
        let right: Vec<u32> = (0..pts.len() as u32).filter(|&i| (pts[i as usize][0] - 0.1).abs() < 1e-12 && (pts[(i as usize + 1) % pts.len()][0] - 0.1).abs() < 1e-12).collect();
        assert_eq!(right.len(), 2, "the right edge is cut in two");
        assert_eq!(d.m.loads.iter().map(|l| l.at.clone()).collect::<Vec<_>>(), right.iter().map(|&e| Target::Edge { shape: s, edge: e }).collect::<Vec<_>>());
        let left = (0..pts.len() as u32).find(|&i| pts[i as usize][0].abs() < 1e-12 && pts[(i as usize + 1) % pts.len()][0].abs() < 1e-12).unwrap();
        assert_eq!(d.m.supports[0].at, Target::Edge { shape: s, edge: left });
        // A hole inside, subtracted: it stays a circle, now a hole shape.
        let c = d.apply(Op::Add { item: Item::Shape(Shape { material: 1, geom: Geom::Circle { c: [0.03, 0.025], r: 0.005 }, ..Shape::default() }) }).unwrap()[0];
        d.apply(Op::Combine { a: s, b: c, how: How::Subtract }).unwrap();
        assert!(d.m.shapes.iter().any(|x| x.hole && matches!(x.geom, Geom::Circle { .. })));
        // A pattern of three more holes along x, and an offset outline.
        let h = d.m.shapes.iter().find(|x| x.hole).unwrap().id;
        let made = d.apply(Op::Pattern { ids: vec![h], times: 3, step: [0.01, 0.0], turn: 0.0, about: [0.0, 0.0] }).unwrap();
        assert_eq!(made.len(), 3);
        assert_eq!(d.m.shapes.iter().filter(|x| x.hole).count(), 4);
        let o = d.apply(Op::Offset { shape: h, d: 0.001 }).unwrap()[0];
        assert!(matches!(d.m.shapes.iter().find(|x| x.id == o).unwrap().geom, Geom::Circle { r, .. } if (r - 0.006).abs() < 1e-12));
        // A circular pattern: four copies a quarter turn apart about the origin.
        let made = d.apply(Op::Pattern { ids: vec![h], times: 3, step: [0.0, 0.0], turn: std::f64::consts::FRAC_PI_2, about: [0.0, 0.0] }).unwrap();
        let Geom::Circle { c, .. } = d.m.shapes.iter().find(|x| x.id == made[1]).unwrap().geom else { panic!() };
        assert!((c[0] + 0.03).abs() < 1e-12 && (c[1] + 0.025).abs() < 1e-12, "half a turn: {c:?}");
        assert!(d.apply(Op::Pattern { ids: vec![h], times: 2, step: [0.0, 0.0], turn: 0.0, about: [0.0, 0.0] }).is_err());
        assert!(d.apply(Op::Combine { a: s, b: s, how: How::Union }).is_err());
        // Undo takes each tool back in one step.
        let before = d.m.shapes.len();
        d.undo();
        assert!(d.m.shapes.len() < before);
    }
}
