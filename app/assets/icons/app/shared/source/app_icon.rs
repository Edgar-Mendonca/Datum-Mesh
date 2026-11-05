//! The Datum Mesh icon, shared by the window, the Windows executable and the CLI.
type P = (f32, f32);


/// The Datum Mesh icon (docs/brand/datum-mesh-icon.svg, viewBox 256), drawn per pixel with
/// 4 x 4 samples: the pale tile, the meshed L-bracket with its shaded corner, the crimson
/// outline and mesh lines, the amber diamond.
pub fn rgba(size: usize) -> Result<Vec<u8>, String> {
    if !(1..=2048).contains(&size) {
        return Err("icon size must be 1..=2048".into());
    }
    type C = [f32; 4];
    let over = |dst: &mut C, src: [u8; 3], a: f32| {
        for k in 0..3 {
            dst[k] = dst[k] * (1.0 - a) + src[k] as f32 / 255.0 * a;
        }
        dst[3] = dst[3] * (1.0 - a) + a;
    };
    let inside = |pts: &[P], x: f32, y: f32| {
        let mut c = false;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + pts.len() - 1) % pts.len()]);
            if (a.1 > y) != (b.1 > y) && x < (b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0 {
                c = !c;
            }
        }
        c
    };
    let seg = |x: f32, y: f32, a: P, b: P| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((x - a.0 - t * dx).powi(2) + (y - a.1 - t * dy).powi(2)).sqrt()
    };
    const L: [P; 6] = [(58.0, 54.0), (104.0, 54.0), (104.0, 152.0), (202.0, 152.0), (202.0, 196.0), (58.0, 196.0)];
    const HOT: [[P; 3]; 2] = [[(104.0, 104.0), (104.0, 152.0), (58.0, 152.0)], [(104.0, 152.0), (152.0, 196.0), (152.0, 152.0)]];
    const MESH: [(P, P); 9] = [((58.0, 104.0), (104.0, 104.0)), ((58.0, 152.0), (104.0, 152.0)), ((104.0, 152.0), (104.0, 196.0)), ((152.0, 152.0), (152.0, 196.0)), ((58.0, 54.0), (104.0, 104.0)), ((58.0, 104.0), (104.0, 152.0)), ((58.0, 152.0), (104.0, 196.0)), ((104.0, 152.0), (152.0, 196.0)), ((152.0, 152.0), (202.0, 196.0))];
    const DIAMOND: [P; 4] = [(220.0, 112.0), (235.0, 127.0), (220.0, 142.0), (205.0, 127.0)];
    let ss = 4;
    let mut out = vec![0u8; size * size * 4];
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32 * 256.0;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32 * 256.0;
                    let mut c: C = [0.0; 4];
                    // The tile: a rounded square from 8 to 248, corner radius 56.
                    let (qx, qy) = ((x - 128.0).abs() - 64.0, (y - 128.0).abs() - 64.0);
                    let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - 56.0;
                    if d <= 0.0 {
                        let t = (x + y) / 512.0;
                        over(&mut c, [255, (255.0 - 20.0 * t) as u8, (255.0 - 16.0 * t) as u8], 1.0);
                        if d > -4.0 {
                            over(&mut c, [241, 179, 193], 1.0);
                        }
                    }
                    if inside(&L, x, y) {
                        over(&mut c, [241, 179, 193], 1.0);
                    }
                    if HOT.iter().any(|t| inside(t, x, y)) {
                        over(&mut c, [184, 50, 80], 0.55);
                    }
                    if MESH.iter().any(|&(a, b)| seg(x, y, a, b) <= 2.5) {
                        over(&mut c, [184, 50, 80], 1.0);
                    }
                    if (0..L.len()).any(|i| seg(x, y, L[i], L[(i + 1) % L.len()]) <= 5.5) {
                        over(&mut c, [184, 50, 80], 1.0);
                    }
                    if inside(&DIAMOND, x, y) {
                        over(&mut c, [242, 178, 51], 1.0);
                    }
                    for k in 0..4 {
                        acc[k] += c[k];
                    }
                }
            }
            let n = (ss * ss) as f32;
            let a = acc[3] / n;
            let o = (py * size + px) * 4;
            for k in 0..3 {
                out[o + k] = if a > 0.0 { (acc[k] / n / a * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            out[o + 3] = (a * 255.0).round() as u8;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_mark_has_transparency_and_original_colors_at_window_and_package_sizes() {
        for size in [16, 24, 32, 48, 64, 128, 256] {
            let pixels = super::rgba(size).expect("valid size");
            assert_eq!(pixels.len(), size * size * 4);
            assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
            assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255 && p[2] > p[0]));
        }
        assert!(super::rgba(0).is_err());
        assert!(super::rgba(2049).is_err());
    }

    /// Borderless: no white edge lines, and the gaps between faces are see-through. At 256 px
    /// the gap between the left and right faces (SVG x 494..530) is transparent on the row
    /// through the middle of the mark, and no opaque pixel is white.
    #[test]
    fn shared_mark_is_borderless() {
        let size = 256;
        let pixels = super::rgba(size).expect("valid size");
        let px = |x: usize, y: usize| pixels.as_chunks::<4>().0[y * size + x];
        let s = size as f32 / 904.436;
        let gap_x = ((size as f32 - 791.188 * s) / 2.0 + (512.0 - 116.406) * s) as usize;
        let row = ((750.0 - 53.364) * s) as usize;
        assert_eq!(px(gap_x, row)[3], 0, "the seam between the left and right faces is a gap");
        assert!(px(gap_x - 8, row)[3] == 255 && px(gap_x + 8, row)[3] == 255, "faces on both sides of the gap");
        assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0 || p[0] < 200), "no white edges");
    }
}
