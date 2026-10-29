//! The Datum Mesh model: nodes and members (frames and trusses), shapes (plane solids),
//! sections, materials, supports, loads and settings. All values are SI base units (m, N, Pa, K,
//! W); the window converts for display. `.dmesh` files are this model as JSON, with a format
//! number, migrations and a reader that opens damaged files as far as it can.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The file format this version writes. Older formats are brought up to date by `migrate`.
pub const FORMAT: u32 = 1;

/// What the model is solved as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Analysis {
    /// 2D frames and trusses (line elements).
    #[default]
    Frame,
    /// Plane stress: thin plates loaded in their plane (uses the thickness).
    Stress,
    /// Plane strain: long bodies with the same section along their length.
    Strain,
    /// Steady heat conduction.
    Heat,
    /// Axisymmetric solids (0.2.1): a half-section drawn right of the y axis (x is the radius,
    /// y runs along the axis), turned a full circle about it. Loads and reactions are totals for
    /// the whole circle.
    Axi,
    /// 3D frames and trusses (0.2.4): members bend about both axes and twist; y is up.
    Space,
}

impl Analysis {
    pub fn is_plane(self) -> bool {
        !self.is_frame()
    }
    /// Frames and trusses, in the plane or in space.
    pub fn is_frame(self) -> bool {
        matches!(self, Analysis::Frame | Analysis::Space)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    /// Depth, for 3D frames (0.2.4); zero in the plane.
    #[serde(default)]
    pub z: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberKind {
    /// Euler–Bernoulli beam: axial force, shear and bending.
    #[default]
    Beam,
    /// Truss bar: axial force only.
    Truss,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub id: u32,
    pub a: u32,
    pub b: u32,
    #[serde(default)]
    pub kind: MemberKind,
    pub section: u32,
    pub material: u32,
    /// 3D frames: the turn of the section about the member's axis (rad), from its local y axis
    /// lying as level as it can (0.2.4).
    #[serde(default)]
    pub roll: f64,
}

impl Node {
    /// Where the node is: (x, y, z).
    pub fn at(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

/// The distance between two points in space.
pub fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// A cross-section's shape; its properties come from `sections::props`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SectionShape {
    /// Solid rectangle b wide, h deep (h is in the plane of bending).
    Rect { b: f64, h: f64 },
    Circle { d: f64 },
    /// Circular tube: outside diameter and wall thickness.
    Tube { d: f64, t: f64 },
    /// Rectangular hollow section.
    Box { b: f64, h: f64, t: f64 },
    /// Doubly symmetric I: depth, flange width, web and flange thickness.
    I { h: f64, b: f64, tw: f64, tf: f64 },
    /// Channel (bending about its strong axis).
    Channel { h: f64, b: f64, tw: f64, tf: f64 },
    /// Equal or unequal angle, legs h (in the plane) and b, thickness t, bent about the axis parallel to b.
    Angle { h: f64, b: f64, t: f64 },
    /// Your own values: area, second moment of area and the distance to the extreme fibre; for
    /// 3D frames the second moment about the other axis and the torsion constant (zero: taken as
    /// for a round bar, iy = i and j = 2i).
    Custom {
        a: f64,
        i: f64,
        c: f64,
        #[serde(default)]
        iy: f64,
        #[serde(default)]
        j: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: u32,
    pub name: String,
    pub shape: SectionShape,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Material {
    pub id: u32,
    pub name: String,
    /// Young's modulus (Pa).
    pub e: f64,
    /// Poisson's ratio.
    pub nu: f64,
    /// Density (kg/m³).
    pub rho: f64,
    /// Yield strength (Pa); 0 = not known (no safety factor).
    pub fy: f64,
    /// Thermal conductivity (W/m·K).
    pub k: f64,
    /// Thermal expansion (1/K).
    pub alpha: f64,
    /// Where the values come from.
    pub source: String,
}

impl Default for Material {
    fn default() -> Self {
        Material::steel(0)
    }
}

impl Material {
    pub fn steel(id: u32) -> Material {
        Material { id, name: "Structural steel S355".into(), e: 210e9, nu: 0.3, rho: 7850.0, fy: 355e6, k: 50.0, alpha: 12e-6, source: "EN 1993-1-1 §3.2 (E, ν, α, density); EN 10025-2 (yield)".into() }
    }
}

/// Plane geometry. Polygons are closed; segment i runs from pts[i] to pts[i+1] (the last back to
/// the first) and bulges into an arc when bulge[i] ≠ 0 (bulge = tan(θ/4), θ the arc's angle,
/// positive counter-clockwise, as in DXF).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Geom {
    Poly {
        pts: Vec<[f64; 2]>,
        #[serde(default)]
        bulge: Vec<f64>,
    },
    Circle { c: [f64; 2], r: f64 },
}

impl Geom {
    pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Geom {
        let (a, b) = (x0.min(x1), x0.max(x1));
        let (c, d) = (y0.min(y1), y0.max(y1));
        Geom::Poly { pts: vec![[a, c], [b, c], [b, d], [a, d]], bulge: vec![0.0; 4] }
    }
    /// Number of edges supports and loads can be put on.
    pub fn edges(&self) -> usize {
        match self {
            Geom::Poly { pts, .. } => pts.len(),
            Geom::Circle { .. } => 1,
        }
    }
    pub fn points_mut(&mut self) -> Vec<&mut [f64; 2]> {
        match self {
            Geom::Poly { pts, .. } => pts.iter_mut().collect(),
            Geom::Circle { c, .. } => vec![c],
        }
    }
    /// Signed area (positive counter-clockwise), arcs included.
    pub fn area(&self) -> f64 {
        match self {
            Geom::Circle { r, .. } => std::f64::consts::PI * r * r,
            Geom::Poly { pts, bulge } => {
                let n = pts.len();
                let mut a = 0.0;
                for i in 0..n {
                    let (p, q) = (pts[i], pts[(i + 1) % n]);
                    a += 0.5 * (p[0] * q[1] - q[0] * p[1]);
                    let b = bulge.get(i).copied().unwrap_or(0.0);
                    if b != 0.0 {
                        // The circular segment between the chord and the arc.
                        let th = 4.0 * b.atan();
                        let c = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
                        let r = c / (2.0 * (th / 2.0).sin().abs());
                        a += 0.5 * r * r * (th - th.sin());
                    }
                }
                a
            }
        }
    }
    /// The outline as points, arcs split so no piece is longer than `seg` (and at least `min`
    /// pieces per full circle). Edge i starts at index starts[i].
    pub fn outline(&self, seg: f64, min: usize) -> (Vec<[f64; 2]>, Vec<usize>) {
        match self {
            Geom::Circle { c, r } => {
                let n = ((2.0 * std::f64::consts::PI * r / seg).ceil() as usize).clamp(min.max(8), 4096);
                ((0..n).map(|i| { let t = i as f64 / n as f64 * std::f64::consts::TAU; [c[0] + r * t.cos(), c[1] + r * t.sin()] }).collect(), vec![0])
            }
            Geom::Poly { pts, bulge } => {
                let n = pts.len();
                let (mut out, mut starts) = (vec![], vec![]);
                for i in 0..n {
                    starts.push(out.len());
                    let (p, q) = (pts[i], pts[(i + 1) % n]);
                    out.push(p);
                    let b = bulge.get(i).copied().unwrap_or(0.0);
                    if b != 0.0 {
                        let (c, r, a0, th) = arc(p, q, b);
                        let k = ((r * th.abs() / seg).ceil() as usize).max(((th.abs() / std::f64::consts::TAU) * min as f64).ceil() as usize).clamp(2, 4096);
                        for j in 1..k {
                            let t = a0 + th * j as f64 / k as f64;
                            out.push([c[0] + r * t.cos(), c[1] + r * t.sin()]);
                        }
                    }
                }
                (out, starts)
            }
        }
    }
}

/// The arc of a bulged segment p→q: centre, radius, start angle and swept angle (signed).
pub fn arc(p: [f64; 2], q: [f64; 2], bulge: f64) -> ([f64; 2], f64, f64, f64) {
    let th = 4.0 * bulge.atan();
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let c = (dx * dx + dy * dy).sqrt();
    let r = c / (2.0 * (th / 2.0).sin()).abs();
    // Centre: from the chord's midpoint, along the chord's left normal by the signed distance.
    let d = c * 0.5 / (th / 2.0).tan();
    let (mx, my) = ((p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0);
    let (nx, ny) = (-dy / c, dx / c);
    let cen = [mx + nx * d, my + ny * d];
    let a0 = (p[1] - cen[1]).atan2(p[0] - cen[0]);
    (cen, r, a0, th)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shape {
    pub id: u32,
    pub name: String,
    /// A hole is cut out of the solid shapes it lies in.
    pub hole: bool,
    pub material: u32,
    pub geom: Geom,
}

impl Default for Shape {
    fn default() -> Self {
        Shape { id: 0, name: String::new(), hole: false, material: 0, geom: Geom::rect(0.0, 0.0, 1.0, 1.0) }
    }
}

/// Where a support or load acts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "on", rename_all = "lowercase")]
pub enum Target {
    Node { id: u32 },
    Member { id: u32 },
    /// Edge `edge` of shape `shape`.
    Edge { shape: u32, edge: u32 },
    /// A point of a plane solid (snapped to the nearest mesh node when solving).
    Point { x: f64, y: f64 },
    /// The whole model (gravity, heat generation).
    Body,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SupportKind {
    /// No movement and, for beams, no rotation.
    Fixed,
    /// No movement; free to rotate.
    Pinned,
    /// Free to move along x (y held).
    RollerX,
    /// Free to move along y (x held).
    RollerY,
    /// Edges: the movement normal to the edge is held.
    Roller,
    /// Edges: a plane of symmetry (the normal movement is held).
    Symmetry,
    /// Springs to ground (N/m, N/m, N·m/rad); `kr` turns about z. 3D frames add z and the turns
    /// about x and y (0.2.4).
    Spring {
        kx: f64,
        ky: f64,
        kr: f64,
        #[serde(default)]
        kz: f64,
        #[serde(default)]
        krx: f64,
        #[serde(default)]
        kry: f64,
    },
    /// Heat: a fixed temperature (K).
    Temperature { t: f64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Support {
    pub id: u32,
    pub at: Target,
    pub kind: SupportKind,
}

/// The direction of a distributed member load.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    /// Global y (down is negative).
    #[default]
    Y,
    X,
    /// Perpendicular to the member (its local y).
    Local,
    /// Global z (3D frames).
    Z,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LoadKind {
    /// Nodes and points: a force (N); fz in 3D frames.
    Force {
        fx: f64,
        fy: f64,
        #[serde(default)]
        fz: f64,
    },
    /// Nodes: a moment (N·m, counter-clockwise positive) about z; in 3D frames also about x and y
    /// (right-hand rule).
    Moment {
        m: f64,
        #[serde(default)]
        mx: f64,
        #[serde(default)]
        my: f64,
    },
    /// Members: load per length varying linearly from w1 at end a to w2 at end b (N/m).
    Dist { w1: f64, w2: f64, dir: Dir },
    /// Edges: pressure (Pa), positive pushing into the body.
    Pressure { p: f64 },
    /// Edges: a force spread along the edge; `total` = (fx, fy) is the whole force (N), else per length (N/m).
    Traction { fx: f64, fy: f64, total: bool },
    /// Edges (usually a hole): a pin bearing on it with the whole force (fx, fy) (N). The pin
    /// presses only where it pushes into the part, with a pressure that peaks in line with the
    /// force and falls as the cosine away from it (0.1.4).
    Bearing { fx: f64, fy: f64 },
    /// Body: a uniform change of temperature from the stress-free one (K): thermal stress in
    /// parts, thermal strain in members (0.2.0).
    Warming { dt: f64 },
    /// Body: gravity (m/s²), self-weight from the density.
    Gravity {
        gx: f64,
        gy: f64,
        #[serde(default)]
        gz: f64,
    },
    /// Edges: heat flux into the body (W/m²).
    Flux { q: f64 },
    /// Edges: convection to a fluid (W/m²·K, K).
    Convection { h: f64, t_inf: f64 },
    /// Body: heat generated per volume (W/m³).
    Heat { q: f64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Load {
    pub id: u32,
    pub at: Target,
    pub kind: LoadKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Refine {
    pub at: Target,
    /// Element size near the target (m).
    pub size: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeshSettings {
    /// Global element size (m); 0 = automatic (about 1/20 of the model's size).
    pub size: f64,
    /// 2 = quadratic 6-node triangles, 1 = linear 3-node triangles.
    pub order: u8,
    /// The smallest angle refinement aims for (degrees).
    pub min_angle: f64,
    pub refine: Vec<Refine>,
}

impl Default for MeshSettings {
    fn default() -> Self {
        MeshSettings { size: 0.0, order: 2, min_angle: 25.0, refine: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Model {
    pub format: u32,
    pub name: String,
    pub notes: String,
    pub analysis: Analysis,
    /// Display units: "SI" (mm, kN, MPa) or "US" (in, kip, ksi).
    pub units: String,
    /// Snap step on the canvas (m).
    pub grid: f64,
    /// Plane-stress thickness (m); plane strain uses 1 m.
    pub thickness: f64,
    /// The temperature at which the part is free of thermal stress (K), for thermal stress.
    pub t_ref: f64,
    pub nodes: Vec<Node>,
    pub members: Vec<Member>,
    pub shapes: Vec<Shape>,
    pub sections: Vec<Section>,
    pub materials: Vec<Material>,
    pub supports: Vec<Support>,
    pub loads: Vec<Load>,
    pub mesh: MeshSettings,
    /// The next free id (ids are unique across every kind of item).
    pub next: u32,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            format: FORMAT,
            name: "Untitled".into(),
            notes: String::new(),
            analysis: Analysis::Frame,
            units: "SI".into(),
            grid: 0.5,
            thickness: 0.01,
            t_ref: 293.15,
            nodes: vec![],
            members: vec![],
            shapes: vec![],
            sections: vec![],
            materials: vec![],
            supports: vec![],
            loads: vec![],
            mesh: MeshSettings::default(),
            next: 1,
        }
    }
}

impl Model {
    /// A new, empty model with one material (structural steel) and, for frames, one section.
    pub fn new(analysis: Analysis) -> Model {
        let mut m = Model { analysis, ..Default::default() };
        m.materials.push(Material::steel(1));
        m.sections.push(Section { id: 2, name: "I 300 × 150".into(), shape: SectionShape::I { h: 0.3, b: 0.15, tw: 0.0071, tf: 0.0107 } });
        m.next = 3;
        if analysis.is_plane() {
            m.grid = 0.005;
            m.name = "Untitled part".into();
        } else {
            m.name = "Untitled frame".into();
        }
        m
    }

    pub fn max_id(&self) -> u32 {
        self.nodes.iter().map(|x| x.id)
            .chain(self.members.iter().map(|x| x.id))
            .chain(self.shapes.iter().map(|x| x.id))
            .chain(self.sections.iter().map(|x| x.id))
            .chain(self.materials.iter().map(|x| x.id))
            .chain(self.supports.iter().map(|x| x.id))
            .chain(self.loads.iter().map(|x| x.id))
            .max()
            .unwrap_or(0)
    }

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn shape(&self, id: u32) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }
    pub fn material(&self, id: u32) -> Option<&Material> {
        self.materials.iter().find(|s| s.id == id)
    }
    pub fn section(&self, id: u32) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// The extent of everything drawn: (x0, y0, x1, y1), or None for an empty model.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        let mut add = |x: f64, y: f64| {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        };
        for n in &self.nodes {
            add(n.x, n.y);
        }
        for s in &self.shapes {
            match &s.geom {
                Geom::Circle { c, r } => {
                    add(c[0] - r, c[1] - r);
                    add(c[0] + r, c[1] + r);
                }
                g => {
                    for p in g.outline(f64::INFINITY, 16).0 {
                        add(p[0], p[1]);
                    }
                }
            }
        }
        b[0].is_finite().then_some(b)
    }

    /// Everything that must hold for a model to be usable; edits are refused if they break it.
    pub fn check(&self) -> Result<(), String> {
        let fin = |v: f64, what: &str| if v.is_finite() { Ok(()) } else { Err(format!("{what} must be a number.")) };
        let pos = |v: f64, what: &str| if v.is_finite() && v > 0.0 { Ok(()) } else { Err(format!("{what} must be greater than zero.")) };
        let mut ids = std::collections::HashSet::new();
        for id in self.nodes.iter().map(|x| x.id).chain(self.members.iter().map(|x| x.id)).chain(self.shapes.iter().map(|x| x.id)).chain(self.sections.iter().map(|x| x.id)).chain(self.materials.iter().map(|x| x.id)).chain(self.supports.iter().map(|x| x.id)).chain(self.loads.iter().map(|x| x.id)) {
            if id == 0 || !ids.insert(id) {
                return Err("Two items share an id.".into());
            }
        }
        if self.next <= self.max_id() {
            return Err("The id counter is behind.".into());
        }
        pos(self.grid, "The grid step")?;
        pos(self.thickness, "The thickness")?;
        fin(self.t_ref, "The stress-free temperature")?;
        if self.mesh.size < 0.0 || !self.mesh.size.is_finite() {
            return Err("The element size must be zero (automatic) or more.".into());
        }
        if !(1..=2).contains(&self.mesh.order) {
            return Err("Elements are linear (1) or quadratic (2).".into());
        }
        if !(5.0..=33.0).contains(&self.mesh.min_angle) {
            return Err("The minimum angle must be between 5° and 33°.".into());
        }
        for r in &self.mesh.refine {
            pos(r.size, "A local element size")?;
            self.target_ok(&r.at)?;
        }
        if self.units != "SI" && self.units != "US" {
            return Err("Units are SI or US.".into());
        }
        if self.materials.is_empty() {
            return Err("A model needs at least one material.".into());
        }
        let mut cells: std::collections::HashMap<(i64, i64, i64), Vec<[f64; 3]>> = std::collections::HashMap::new();
        let c = crate::doc::SAME * 4.0;
        for n in &self.nodes {
            fin(n.x, "A node's x")?;
            fin(n.y, "A node's y")?;
            fin(n.z, "A node's z")?;
            let k = ((n.x / c).floor() as i64, (n.y / c).floor() as i64, (n.z / c).floor() as i64);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if cells.get(&(k.0 + dx, k.1 + dy, k.2 + dz)).is_some_and(|v| v.iter().any(|p| dist3(*p, n.at()) < crate::doc::SAME)) {
                            return Err("Two nodes would be at the same place.".into());
                        }
                    }
                }
            }
            cells.entry(k).or_default().push(n.at());
        }
        for mt in &self.materials {
            pos(mt.e, &format!("{}: Young's modulus", mt.name))?;
            if !(mt.nu > -1.0 && mt.nu < 0.5) {
                return Err(format!("{}: Poisson's ratio must be between −1 and 0.5.", mt.name));
            }
            for (v, w) in [(mt.rho, "density"), (mt.fy, "yield strength"), (mt.k, "conductivity")] {
                if !(v.is_finite() && v >= 0.0) {
                    return Err(format!("{}: the {w} must be zero or more.", mt.name));
                }
            }
            fin(mt.alpha, "Thermal expansion")?;
        }
        for s in &self.sections {
            crate::sections::props(&s.shape).map_err(|e| format!("{}: {e}", s.name))?;
        }
        let mut pairs = std::collections::HashSet::new();
        let at: std::collections::HashMap<u32, &Node> = self.nodes.iter().map(|n| (n.id, n)).collect();
        for mb in &self.members {
            let (Some(a), Some(b)) = (at.get(&mb.a), at.get(&mb.b)) else { return Err("A member ends at a node that does not exist.".into()) };
            fin(mb.roll, "A member's roll")?;
            if mb.a == mb.b || dist3(a.at(), b.at()) <= 0.0 {
                return Err("A member must join two nodes at different places.".into());
            }
            if !pairs.insert((mb.a.min(mb.b), mb.a.max(mb.b))) {
                return Err("There is already a member between these nodes.".into());
            }
            self.section(mb.section).ok_or("A member uses a section that does not exist.")?;
            self.material(mb.material).ok_or("A member uses a material that does not exist.")?;
        }
        for s in &self.shapes {
            self.material(s.material).ok_or("A shape uses a material that does not exist.")?;
            match &s.geom {
                Geom::Circle { c, r } => {
                    fin(c[0], "A circle's centre")?;
                    fin(c[1], "A circle's centre")?;
                    pos(*r, "A circle's radius")?;
                }
                Geom::Poly { pts, bulge } => {
                    if pts.len() < 3 || pts.len() > 10_000 {
                        return Err("A polygon needs at least three corners.".into());
                    }
                    if bulge.len() != pts.len() {
                        return Err("A polygon's arcs do not match its corners.".into());
                    }
                    for (i, p) in pts.iter().enumerate() {
                        fin(p[0], "A corner's x")?;
                        fin(p[1], "A corner's y")?;
                        let q = pts[(i + 1) % pts.len()];
                        if p == &q {
                            return Err("A polygon has two corners at the same place.".into());
                        }
                        if !bulge[i].is_finite() || bulge[i].abs() > 1.0 {
                            return Err("An arc can turn at most half a circle.".into());
                        }
                    }
                    if s.geom.area().abs() < 1e-18 {
                        return Err("A polygon must enclose an area.".into());
                    }
                }
            }
        }
        for s in &self.supports {
            self.target_ok(&s.at)?;
            match (&s.kind, &s.at) {
                (SupportKind::Spring { kx, ky, kr, kz, krx, kry }, _) => {
                    for v in [kx, ky, kr, kz, krx, kry] {
                        if !(v.is_finite() && *v >= 0.0) {
                            return Err("Spring stiffnesses must be zero or more.".into());
                        }
                    }
                }
                (SupportKind::Temperature { t }, _) => fin(*t, "A temperature")?,
                (SupportKind::Roller | SupportKind::Symmetry, Target::Edge { .. }) => {}
                (SupportKind::Roller | SupportKind::Symmetry, _) => return Err("Roller and symmetry supports go on edges.".into()),
                (_, Target::Member { .. } | Target::Body) => return Err("Supports go on nodes, edges or points.".into()),
                _ => {}
            }
        }
        for l in &self.loads {
            self.target_ok(&l.at)?;
            let ok = match (&l.kind, &l.at) {
                (LoadKind::Force { fx, fy, fz }, Target::Node { .. } | Target::Point { .. }) => fx.is_finite() && fy.is_finite() && fz.is_finite(),
                (LoadKind::Moment { m, mx, my }, Target::Node { .. }) => m.is_finite() && mx.is_finite() && my.is_finite(),
                (LoadKind::Dist { w1, w2, .. }, Target::Member { .. }) => w1.is_finite() && w2.is_finite(),
                (LoadKind::Pressure { p }, Target::Edge { .. }) => p.is_finite(),
                (LoadKind::Traction { fx, fy, .. } | LoadKind::Bearing { fx, fy }, Target::Edge { .. }) => fx.is_finite() && fy.is_finite(),
                (LoadKind::Gravity { gx, gy, gz }, Target::Body) => gx.is_finite() && gy.is_finite() && gz.is_finite(),
                (LoadKind::Flux { q }, Target::Edge { .. }) => q.is_finite(),
                (LoadKind::Convection { h, t_inf }, Target::Edge { .. }) => h.is_finite() && *h >= 0.0 && t_inf.is_finite(),
                (LoadKind::Heat { q }, Target::Body) => q.is_finite(),
                (LoadKind::Warming { dt }, Target::Body) => dt.is_finite(),
                _ => return Err("That load cannot go there.".into()),
            };
            if !ok {
                return Err("A load's values must be numbers.".into());
            }
        }
        Ok(())
    }

    fn target_ok(&self, t: &Target) -> Result<(), String> {
        match t {
            Target::Node { id } => self.node(*id).map(|_| ()).ok_or_else(|| "That node does not exist.".into()),
            Target::Member { id } => self.members.iter().any(|m| m.id == *id).then_some(()).ok_or_else(|| "That member does not exist.".into()),
            Target::Edge { shape, edge } => match self.shape(*shape) {
                Some(s) if (*edge as usize) < s.geom.edges() => Ok(()),
                _ => Err("That edge does not exist.".into()),
            },
            Target::Point { x, y } => (x.is_finite() && y.is_finite()).then_some(()).ok_or_else(|| "A point needs numbers.".into()),
            Target::Body => Ok(()),
        }
    }
}

/// Read a `.dmesh` file. A damaged file opens as far as possible: items that cannot be read are
/// left out and listed in the warnings.
pub fn from_json(text: &str) -> Result<(Model, Vec<String>), String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("This is not a Datum Mesh model: {e}"))?;
    let mut v = migrate(v)?;
    let mut warn = vec![];
    // Read lists item by item so one bad item does not lose the rest.
    let lists = ["nodes", "members", "shapes", "sections", "materials", "supports", "loads"];
    let mut keep: Vec<(String, Vec<Value>)> = vec![];
    if let Some(o) = v.as_object_mut() {
        for k in lists {
            match o.remove(k) {
                Some(Value::Array(a)) => keep.push((k.into(), a)),
                Some(Value::Null) | None => {}
                Some(_) => warn.push(format!("The {k} list was damaged and was left out.")),
            }
        }
        if !o.get("mesh").is_none_or(|m| serde_json::from_value::<MeshSettings>(m.clone()).is_ok()) {
            o.remove("mesh");
            warn.push("The mesh settings were damaged; the defaults are used.".into());
        }
        // Settings that cannot be read (the wrong type, a negative id counter, an unknown kind of
        // analysis) fall back to their defaults, one by one (0.1.1 review: they refused the file).
        let keys: Vec<String> = o.keys().filter(|k| !lists.contains(&k.as_str()) && k.as_str() != "mesh").cloned().collect();
        for k in keys {
            let one = serde_json::json!({ k.clone(): o[&k].clone() });
            if serde_json::from_value::<Model>(one).is_err() {
                warn.push(format!("The setting \"{k}\" was damaged; the default is used."));
                o.remove(&k);
            }
        }
    }
    let mut m: Model = serde_json::from_value(v).map_err(|e| format!("The model file is damaged: {e}"))?;
    fn read<T: serde::de::DeserializeOwned>(a: Vec<Value>, what: &str, warn: &mut Vec<String>) -> Vec<T> {
        let n = a.len();
        let out: Vec<T> = a.into_iter().filter_map(|x| serde_json::from_value(x).ok()).collect();
        if out.len() < n {
            warn.push(format!("{} of the {what} could not be read and were left out.", n - out.len()));
        }
        out
    }
    for (k, a) in keep {
        match k.as_str() {
            "nodes" => m.nodes = read(a, "nodes", &mut warn),
            "members" => m.members = read(a, "members", &mut warn),
            "shapes" => m.shapes = read(a, "shapes", &mut warn),
            "sections" => m.sections = read(a, "sections", &mut warn),
            "materials" => m.materials = read(a, "materials", &mut warn),
            "supports" => m.supports = read(a, "supports", &mut warn),
            "loads" => m.loads = read(a, "loads", &mut warn),
            _ => {}
        }
    }
    repair(&mut m, &mut warn);
    Ok((m, warn))
}

pub fn to_json(m: &Model) -> String {
    serde_json::to_string_pretty(m).unwrap_or_default()
}

/// Bring older formats up to date. Format 1 is the first; later versions add steps here.
fn migrate(v: Value) -> Result<Value, String> {
    if !v.is_object() {
        return Err("This is not a Datum Mesh model.".into());
    }
    let f = v.get("format").and_then(|f| f.as_u64()).unwrap_or(1);
    if f > FORMAT as u64 {
        return Err(format!("This model was saved by a newer Datum Mesh (file format {f}). Update the app to open it."));
    }
    Ok(v)
}

/// Make a read model usable: unique ids, references that exist, numbers in range. Whatever is
/// dropped or changed is listed in `warn`. The result always passes `Model::check`.
pub fn repair(m: &mut Model, warn: &mut Vec<String>) {
    m.format = FORMAT;
    let d = Model::default();
    let n0 = (m.nodes.len(), m.members.len(), m.shapes.len(), m.supports.len(), m.loads.len());
    // Ids: unique and non-zero, across every kind.
    let mut seen = std::collections::HashSet::new();
    let mut next = m.max_id().saturating_add(1).max(m.next).max(1);
    let mut fix = |id: &mut u32| {
        if *id == 0 || !seen.insert(*id) {
            *id = next;
            seen.insert(next);
            next += 1;
        }
    };
    // Materials and sections first, so references to them stay right where ids were fine.
    m.materials.iter_mut().for_each(|x| fix(&mut x.id));
    m.sections.iter_mut().for_each(|x| fix(&mut x.id));
    m.nodes.iter_mut().for_each(|x| fix(&mut x.id));
    m.members.iter_mut().for_each(|x| fix(&mut x.id));
    m.shapes.iter_mut().for_each(|x| fix(&mut x.id));
    m.supports.iter_mut().for_each(|x| fix(&mut x.id));
    m.loads.iter_mut().for_each(|x| fix(&mut x.id));
    m.next = next;
    // Nodes with bad coordinates go, and so does what hangs on them; nodes at one place become one.
    m.nodes.retain(|n| n.x.is_finite() && n.y.is_finite());
    let before = m.nodes.len();
    crate::doc::merge_all(m);
    if m.nodes.len() < before {
        warn.push(format!("{} nodes that sat on others were joined to them.", before - m.nodes.len()));
    }
    let mut fresh = || {
        let id = m.next;
        m.next += 1;
        id
    };
    // Settings.
    if !(m.grid.is_finite() && m.grid > 0.0) {
        m.grid = d.grid;
    }
    if !(m.thickness.is_finite() && m.thickness > 0.0) {
        m.thickness = d.thickness;
    }
    if m.units != "SI" && m.units != "US" {
        m.units = "SI".into();
    }
    if !(m.mesh.size.is_finite() && m.mesh.size >= 0.0) {
        m.mesh.size = 0.0;
    }
    if !(1..=2).contains(&m.mesh.order) {
        m.mesh.order = 2;
    }
    if !(5.0..=33.0).contains(&m.mesh.min_angle) {
        m.mesh.min_angle = 25.0;
    }
    // Materials and sections with bad values are dropped; a model always keeps one material.
    m.materials.retain(|mt| {
        let probe = Model { materials: vec![mt.clone()], next: mt.id + 1, ..Model::default() };
        probe.check().is_ok()
    });
    if m.materials.is_empty() {
        let id = fresh();
        m.materials.push(Material::steel(id));
    }
    m.sections.retain(|s| crate::sections::props(&s.shape).is_ok());
    let mat0 = m.materials[0].id;
    if m.sections.is_empty() && !m.members.is_empty() {
        let id = fresh();
        m.sections.push(Section { id, name: "Rectangle 100 × 200".into(), shape: SectionShape::Rect { b: 0.1, h: 0.2 } });
    }
    let sec0 = m.sections.first().map(|s| s.id).unwrap_or(0);
    let nodes: std::collections::HashSet<u32> = m.nodes.iter().map(|n| n.id).collect();
    let (mats, secs): (std::collections::HashSet<u32>, std::collections::HashSet<u32>) = (m.materials.iter().map(|x| x.id).collect(), m.sections.iter().map(|x| x.id).collect());
    let mut pairs = std::collections::HashSet::new();
    let pos: std::collections::HashMap<u32, (f64, f64)> = m.nodes.iter().map(|n| (n.id, (n.x, n.y))).collect();
    m.members.retain_mut(|mb| {
        if !mats.contains(&mb.material) {
            mb.material = mat0;
        }
        if !secs.contains(&mb.section) {
            mb.section = sec0;
        }
        nodes.contains(&mb.a) && nodes.contains(&mb.b) && mb.a != mb.b && pos[&mb.a] != pos[&mb.b] && pairs.insert((mb.a.min(mb.b), mb.a.max(mb.b)))
    });
    m.shapes.retain_mut(|s| {
        if !mats.contains(&s.material) {
            s.material = mat0;
        }
        if let Geom::Poly { pts, bulge } = &mut s.geom {
            bulge.resize(pts.len(), 0.0);
        }
        let probe = Model { shapes: vec![s.clone()], materials: m.materials.clone(), next: m.next, ..Model::default() };
        probe.check().is_ok()
    });
    // Supports and loads must point at something that still exists and make sense there.
    let base = Model { supports: vec![], loads: vec![], ..m.clone() };
    m.supports.retain(|s| Model { supports: vec![s.clone()], ..base.clone() }.check().is_ok());
    m.loads.retain(|l| Model { loads: vec![l.clone()], ..base.clone() }.check().is_ok());
    m.mesh.refine.retain(|r| r.size.is_finite() && r.size > 0.0 && Model { mesh: MeshSettings { refine: vec![r.clone()], ..MeshSettings::default() }, ..base.clone() }.check().is_ok());
    let n1 = (m.nodes.len(), m.members.len(), m.shapes.len(), m.supports.len(), m.loads.len());
    for (what, a, b) in [("nodes", n0.0, n1.0), ("members", n0.1, n1.1), ("shapes", n0.2, n1.2), ("supports", n0.3, n1.3), ("loads", n0.4, n1.4)] {
        if b < a {
            warn.push(format!("{} {what} were damaged and were left out.", a - b));
        }
    }
    debug_assert!(m.check().is_ok(), "{:?}", m.check());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_model_is_valid_and_round_trips() {
        for a in [Analysis::Frame, Analysis::Stress, Analysis::Strain, Analysis::Heat] {
            let m = Model::new(a);
            m.check().unwrap();
            let (back, warn) = from_json(&to_json(&m)).unwrap();
            assert_eq!(back, m);
            assert!(warn.is_empty());
        }
    }

    #[test]
    fn areas_with_arcs() {
        // A unit square: 1. A circle of radius 2 made of two half-circle arcs: 4π.
        assert!((Geom::rect(0.0, 0.0, 1.0, 1.0).area() - 1.0).abs() < 1e-12);
        let c = Geom::Poly { pts: vec![[2.0, 0.0], [-2.0, 0.0]], bulge: vec![1.0, 1.0] };
        assert!((c.area() - 4.0 * std::f64::consts::PI).abs() < 1e-9, "{}", c.area());
        // The outline of that circle lies on the circle and splits into short pieces.
        let (pts, starts) = c.outline(0.1, 16);
        assert_eq!(starts.len(), 2);
        assert!(pts.len() > 120);
        assert!(pts.iter().all(|p| ((p[0] * p[0] + p[1] * p[1]).sqrt() - 2.0).abs() < 1e-9));
        // A quarter-circle fillet bulging outward: square 1×1 with its top edge replaced by an arc.
        let (cen, r, _, th) = arc([1.0, 0.0], [0.0, 1.0], (std::f64::consts::PI / 8.0).tan());
        assert!(cen[0].abs() < 1e-12 && cen[1].abs() < 1e-12 && (r - 1.0).abs() < 1e-12 && (th - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn newer_formats_and_garbage_are_refused_clearly() {
        assert!(from_json("{\"format\": 99}").unwrap_err().contains("newer"));
        assert!(from_json("[1,2]").is_err());
        assert!(from_json("not json").is_err());
    }
}
