//! The materials library (Home › Materials › Library): metals and common polymers copied from
//! Datum Atlas with their sources (`data/materials.json`, built by `tools/materials.py`).
//! Atlas has no Poisson's ratio; ours are typical values for each family.

use crate::model::Material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibMaterial {
    pub id: String,
    pub name: String,
    pub family: String,
    pub e: f64,
    pub nu: f64,
    pub rho: f64,
    pub fy: f64,
    pub k: f64,
    pub alpha: f64,
    pub source: String,
}

impl LibMaterial {
    /// The library entry as a model material with the given id.
    pub fn to_material(&self, id: u32) -> Material {
        Material { id, name: self.name.clone(), e: self.e, nu: self.nu, rho: self.rho, fy: self.fy, k: self.k, alpha: self.alpha, source: self.source.clone() }
    }
}

pub fn library() -> Vec<LibMaterial> {
    serde_json::from_str(include_str!("../data/materials.json")).expect("the materials library is valid JSON")
}

pub fn get(id: &str) -> Option<LibMaterial> {
    library().into_iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use crate::model::{Analysis, Model};

    #[test]
    fn every_library_material_is_valid_and_has_a_source() {
        let lib = super::library();
        assert!(lib.len() >= 50);
        for (i, l) in lib.iter().enumerate() {
            let mut m = Model::new(Analysis::Stress);
            m.materials = vec![l.to_material(1)];
            m.check().unwrap_or_else(|e| panic!("{}: {e}", l.name));
            assert!(l.e > 1e8 && l.k > 0.0 && l.rho > 100.0 && l.source.starts_with("Datum Atlas"), "{}", l.name);
            assert!(lib[i + 1..].iter().all(|x| x.id != l.id), "{} twice", l.id);
        }
        // Spot values against Atlas: 6061-T6 E 69 GPa, ν 0.33, yield 240 MPa (low end).
        let al = super::get("al-6061-t6").unwrap();
        assert_eq!((al.e, al.nu, al.fy), (69e9, 0.33, 240e6));
    }
}
