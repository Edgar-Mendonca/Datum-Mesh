//! The open document: the model, its file, and undo/redo. The window sends edit operations;
//! each one runs on a copy and is kept only if the result passes `Model::check`, so a failed edit
//! leaves the model exactly as it was (Plan's pattern: ops in, whole-model snapshots for undo).

use crate::model::{dist3, Analysis, Geom, Load, LoadKind, Material, Member, MemberKind, MeshSettings, Model, Node, Section, Shape, Support, Target};
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
    /// The stress-free temperature (K); left as it is when not given.
    #[serde(default)]
    pub t_ref: Option<f64>,
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
    SetSettings { s: Settings },
    SetMeta { name: String, notes: String },
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
            m.members.push(Member { id, a, b, kind, section, material, roll: 0.0 });
            created.push(id);
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
                            m.members.push(Member { id, a, b: c, kind, section: sec, material: mat, roll: 0.0 });
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
        Op::SetSettings { s } => {
            m.analysis = s.analysis;
            m.units = s.units;
            m.grid = s.grid;
            m.thickness = s.thickness;
            m.mesh = s.mesh;
            if let Some(t) = s.t_ref {
                m.t_ref = t;
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
/// Move supports, loads and local sizes on the edges of `shape` to the edges `f` gives (several
/// when an edge was split); targets that land twice on one edge are kept once.
fn renumber(m: &mut Model, shape: u32, f: impl Fn(u32) -> Vec<u32>) {
    let map = |t: &Target| -> Vec<Target> {
        match t {
            Target::Edge { shape: s, edge } if *s == shape => f(*edge).into_iter().map(|e| Target::Edge { shape, edge: e }).collect(),
            t => vec![t.clone()],
        }
    };
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
            if let LoadKind::Traction { fx, fy, total: true } = kind {
                kind = LoadKind::Traction { fx: fx / n as f64, fy: fy / n as f64, total: true };
            }
            if let LoadKind::Bearing { fx, fy } = kind {
                kind = LoadKind::Bearing { fx: fx / n as f64, fy: fy / n as f64 };
            }
            let id = if k == 0 { x.id } else { fresh(m) };
            loads.push(crate::model::Load { id, at, kind });
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
        let nid = fresh(m);
        m.members[i].b = n;
        m.members.push(Member { id: nid, a: n, b: b.b, kind: b.kind, section: b.section, material: b.material, roll: b.roll });
        let mut extra = vec![];
        for l in m.loads.iter_mut() {
            if let (Target::Member { id }, LoadKind::Dist { w1, w2, dir }) = (&l.at, &mut l.kind) {
                if *id == b.id {
                    let wt = *w1 + (*w2 - *w1) * t;
                    extra.push((LoadKind::Dist { w1: wt, w2: *w2, dir: *dir }, nid));
                    *w2 = wt;
                }
            }
        }
        for (kind, mid) in extra {
            let id = fresh(m);
            m.loads.push(crate::model::Load { id, at: Target::Member { id: mid }, kind });
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
    use crate::model::SupportKind;

    #[test]
    fn members_share_nodes_and_delete_cascades() {
        let mut d = Doc::new(Model::new(Analysis::Frame));
        let c = d.apply(Op::AddMember { from: [0.0, 0.0], to: [0.0, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert_eq!(c.len(), 3);
        let c2 = d.apply(Op::AddMember { from: [0.0, 3.0], to: [4.0, 3.0], kind: MemberKind::Beam, section: None, material: None, z: [0.0; 2] }).unwrap();
        assert_eq!(c2.len(), 2, "the corner node is shared");
        assert_eq!(d.m.nodes.len(), 3);
        let top = c[1];
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Node { id: top }, kind: LoadKind::Force { fx: 1e3, fy: 0.0, fz: 0.0 } }) }).unwrap();
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
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Member { id: first }, kind: LoadKind::Dist { w1: -1e3, w2: -3e3, dir: crate::model::Dir::Y } }) }).unwrap();
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
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 3 }, kind: LoadKind::Pressure { p: 1e6 } }) }).unwrap();
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
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Member { id: beam }, kind: LoadKind::Dist { w1: -2.0, w2: -8.0, dir: crate::model::Dir::Y } }) }).unwrap();
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
        d.apply(Op::Add { item: Item::Load(Load { id: 0, at: Target::Edge { shape: s, edge: 1 }, kind: LoadKind::Pressure { p: 1e6 } }) }).unwrap();
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
}
