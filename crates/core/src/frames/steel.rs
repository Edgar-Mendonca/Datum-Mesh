//! Steel section tables (0.3.3): European rolled I and H sections (IPE, HEA, HEB) by their
//! nominal dimensions in EN 10365. The properties are worked out from the dimensions (with the
//! root fillets) by `sections::props`, never copied; each row keeps the table's area so a test
//! checks the dimensions against it.

use crate::model::SectionShape;
use serde::Serialize;

/// A section from the tables, ready to add to a model.
#[derive(Clone, Debug, Serialize)]
pub struct LibSection {
    pub name: String,
    pub family: &'static str,
    pub shape: SectionShape,
    /// The table's cross-section area (m²), for checking.
    pub table_area: f64,
}

/// (family, size, h, b, tw, tf, r in mm, A in cm²).
const ROWS: &[(&str, u32, f64, f64, f64, f64, f64, f64)] = &[
    ("IPE", 80, 80.0, 46.0, 3.8, 5.2, 5.0, 7.64),
    ("IPE", 100, 100.0, 55.0, 4.1, 5.7, 7.0, 10.3),
    ("IPE", 120, 120.0, 64.0, 4.4, 6.3, 7.0, 13.2),
    ("IPE", 140, 140.0, 73.0, 4.7, 6.9, 7.0, 16.4),
    ("IPE", 160, 160.0, 82.0, 5.0, 7.4, 9.0, 20.1),
    ("IPE", 180, 180.0, 91.0, 5.3, 8.0, 9.0, 23.9),
    ("IPE", 200, 200.0, 100.0, 5.6, 8.5, 12.0, 28.5),
    ("IPE", 220, 220.0, 110.0, 5.9, 9.2, 12.0, 33.4),
    ("IPE", 240, 240.0, 120.0, 6.2, 9.8, 15.0, 39.1),
    ("IPE", 270, 270.0, 135.0, 6.6, 10.2, 15.0, 45.9),
    ("IPE", 300, 300.0, 150.0, 7.1, 10.7, 15.0, 53.8),
    ("IPE", 330, 330.0, 160.0, 7.5, 11.5, 18.0, 62.6),
    ("IPE", 360, 360.0, 170.0, 8.0, 12.7, 18.0, 72.7),
    ("IPE", 400, 400.0, 180.0, 8.6, 13.5, 21.0, 84.5),
    ("IPE", 450, 450.0, 190.0, 9.4, 14.6, 21.0, 98.8),
    ("IPE", 500, 500.0, 200.0, 10.2, 16.0, 21.0, 116.0),
    ("IPE", 550, 550.0, 210.0, 11.1, 17.2, 24.0, 134.0),
    ("IPE", 600, 600.0, 220.0, 12.0, 19.0, 24.0, 156.0),
    ("HEA", 100, 96.0, 100.0, 5.0, 8.0, 12.0, 21.2),
    ("HEA", 120, 114.0, 120.0, 5.0, 8.0, 12.0, 25.3),
    ("HEA", 140, 133.0, 140.0, 5.5, 8.5, 12.0, 31.4),
    ("HEA", 160, 152.0, 160.0, 6.0, 9.0, 15.0, 38.8),
    ("HEA", 180, 171.0, 180.0, 6.0, 9.5, 15.0, 45.3),
    ("HEA", 200, 190.0, 200.0, 6.5, 10.0, 18.0, 53.8),
    ("HEA", 220, 210.0, 220.0, 7.0, 11.0, 18.0, 64.3),
    ("HEA", 240, 230.0, 240.0, 7.5, 12.0, 21.0, 76.8),
    ("HEA", 260, 250.0, 260.0, 7.5, 12.5, 24.0, 86.8),
    ("HEA", 280, 270.0, 280.0, 8.0, 13.0, 24.0, 97.3),
    ("HEA", 300, 290.0, 300.0, 8.5, 14.0, 27.0, 112.5),
    ("HEA", 320, 310.0, 300.0, 9.0, 15.5, 27.0, 124.4),
    ("HEA", 340, 330.0, 300.0, 9.5, 16.5, 27.0, 133.5),
    ("HEA", 360, 350.0, 300.0, 10.0, 17.5, 27.0, 142.8),
    ("HEA", 400, 390.0, 300.0, 11.0, 19.0, 27.0, 159.0),
    ("HEA", 450, 440.0, 300.0, 11.5, 21.0, 27.0, 178.0),
    ("HEA", 500, 490.0, 300.0, 12.0, 23.0, 27.0, 197.5),
    ("HEA", 550, 540.0, 300.0, 12.5, 24.0, 27.0, 211.8),
    ("HEA", 600, 590.0, 300.0, 13.0, 25.0, 27.0, 226.5),
    ("HEB", 100, 100.0, 100.0, 6.0, 10.0, 12.0, 26.0),
    ("HEB", 120, 120.0, 120.0, 6.5, 11.0, 12.0, 34.0),
    ("HEB", 140, 140.0, 140.0, 7.0, 12.0, 12.0, 43.0),
    ("HEB", 160, 160.0, 160.0, 8.0, 13.0, 15.0, 54.3),
    ("HEB", 180, 180.0, 180.0, 8.5, 14.0, 15.0, 65.3),
    ("HEB", 200, 200.0, 200.0, 9.0, 15.0, 18.0, 78.1),
    ("HEB", 220, 220.0, 220.0, 9.5, 16.0, 18.0, 91.0),
    ("HEB", 240, 240.0, 240.0, 10.0, 17.0, 21.0, 106.0),
    ("HEB", 260, 260.0, 260.0, 10.0, 17.5, 24.0, 118.4),
    ("HEB", 280, 280.0, 280.0, 10.5, 18.0, 24.0, 131.4),
    ("HEB", 300, 300.0, 300.0, 11.0, 19.0, 27.0, 149.1),
    ("HEB", 320, 320.0, 300.0, 11.5, 20.5, 27.0, 161.3),
    ("HEB", 340, 340.0, 300.0, 12.0, 21.5, 27.0, 170.9),
    ("HEB", 360, 360.0, 300.0, 12.5, 22.5, 27.0, 180.6),
    ("HEB", 400, 400.0, 300.0, 13.5, 24.0, 27.0, 197.8),
    ("HEB", 450, 450.0, 300.0, 14.0, 26.0, 27.0, 218.0),
    ("HEB", 500, 500.0, 300.0, 14.5, 28.0, 27.0, 238.6),
    ("HEB", 550, 550.0, 300.0, 15.0, 29.0, 27.0, 254.1),
    ("HEB", 600, 600.0, 300.0, 15.5, 30.0, 27.0, 270.0),
];

/// Every section in the tables, in SI units.
pub fn library() -> Vec<LibSection> {
    let mm = 1e-3;
    ROWS.iter()
        .map(|&(family, size, h, b, tw, tf, r, a)| LibSection { name: format!("{family} {size}"), family, shape: SectionShape::I { h: h * mm, b: b * mm, tw: tw * mm, tf: tf * mm, r: r * mm }, table_area: a * 1e-4 })
        .collect()
}

/// The section of the tables with this name ("IPE 300").
pub fn find(name: &str) -> Option<LibSection> {
    library().into_iter().find(|s| s.name.eq_ignore_ascii_case(name.trim()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_table_section_has_the_area_the_table_gives() {
        for s in super::library() {
            let p = crate::sections::props(&s.shape).unwrap();
            let e = (p.a - s.table_area).abs() / s.table_area;
            assert!(e < 5e-3, "{}: area {} against {} ({e:.1e})", s.name, p.a, s.table_area);
        }
        for n in ["IPE 300", "HEA 200", "HEB 200", "IPE 200", "HEB 300", "IPE 400"] {
            let p = crate::sections::props(&super::find(n).unwrap().shape).unwrap();
            println!("{n}: A {:.2} cm², Iy {:.0} cm⁴, Iz {:.1} cm⁴", p.a * 1e4, p.i * 1e8, p.iy * 1e8);
        }
    }
}
