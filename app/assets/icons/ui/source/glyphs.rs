//! Collected glyph artwork. Caption geometry adapted from VectorCraft (MIT OR Apache-2.0).
use crate::titlebar::Caption;
use egui::{Color32, Pos2, Rect, Stroke, Ui, pos2, vec2};

pub(crate) fn transport_glyph(p: &egui::Painter, c: egui::Pos2, what: &str, col: Color32) {
    let tri = |dir: f32, dx: f32| {
        let pts = vec![pos2(c.x + dx - 4.0 * dir, c.y - 5.0), pos2(c.x + dx + 4.0 * dir, c.y), pos2(c.x + dx - 4.0 * dir, c.y + 5.0)];
        p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    };
    let bar = |dx: f32| p.line_segment([pos2(c.x + dx, c.y - 5.0), pos2(c.x + dx, c.y + 5.0)], Stroke::new(1.6, col));
    match what {
        "first" => {
            bar(-5.0);
            tri(-1.0, 1.0);
        }
        "back" => tri(-1.0, 0.0),
        "forward" => tri(1.0, 0.0),
        // Play History: larger than the step arrows so it doesn't read as Forward, and two bars
        // while it plays.
        "play" => {
            let pts = vec![pos2(c.x - 4.5, c.y - 7.0), pos2(c.x + 6.5, c.y), pos2(c.x - 4.5, c.y + 7.0)];
            p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        "pause" => {
            for dx in [-3.0, 3.0] {
                p.rect_filled(Rect::from_center_size(pos2(c.x + dx, c.y), vec2(3.0, 12.0)), 0.5, col);
            }
        }
        _ => {
            tri(1.0, -1.0);
            bar(5.0);
        }
    }
}

pub(crate) fn paint_glyph(ui: &Ui, c: Caption, maximized: bool, at: egui::Pos2, s: Stroke) {
    let p = ui.painter();
    let g = Rect::from_center_size(at, vec2(10.0, 10.0));
    match c {
        Caption::Minimize => {
            p.line_segment([g.left_center(), g.right_center()], s);
        }
        // Restore: a front square with the corner of the one behind it.
        Caption::Maximize if maximized => {
            let front = Rect::from_min_max(g.min + vec2(0.0, 2.0), g.max - vec2(2.0, 0.0));
            p.rect_stroke(front, 0.0, s, egui::StrokeKind::Middle);
            p.line(vec![g.min + vec2(2.0, 2.0), g.min + vec2(2.0, 0.0), g.right_top(), g.max - vec2(0.0, 2.0), g.max - vec2(2.0, 2.0)], s);
        }
        Caption::Maximize => {
            p.rect_stroke(g, 0.0, s, egui::StrokeKind::Middle);
        }
        Caption::Close => {
            p.line_segment([g.left_top(), g.right_bottom()], s);
            p.line_segment([g.right_top(), g.left_bottom()], s);
        }
    }
}

pub(crate) fn badge(p: &egui::Painter, r: Rect, name: &str, c: Color32) {
    let s = Stroke::new(1.3, c);
    match name {
        "lock" => {
            let body = Rect::from_min_max(pos2(r.left() + 1.5, r.center().y - 0.5), pos2(r.right() - 1.5, r.bottom() - 0.5));
            p.rect_filled(body, 1.5, c);
            let top = pos2(r.center().x, r.top() + 4.0);
            p.add(egui::Shape::line(
                (0..=8)
                    .map(|k| {
                        let a = std::f32::consts::PI * (k as f32 / 8.0);
                        pos2(top.x - 3.2 * a.cos(), top.y - 3.2 * a.sin() + 1.0)
                    })
                    .chain([pos2(top.x + 3.2, body.top())])
                    .collect(),
                s,
            ));
            p.line_segment([pos2(top.x - 3.2, top.y + 1.0), pos2(top.x - 3.2, body.top())], s);
        }
        "pin" => {
            let head = pos2(r.center().x + 2.0, r.top() + 4.0);
            p.circle_filled(head, 3.2, c);
            p.line_segment([head, pos2(r.left() + 1.5, r.bottom() - 1.0)], Stroke::new(1.6, c));
        }
        _ => {}
    }
}

pub(crate) fn pin_icon(p: &egui::Painter, r: Rect, on: bool, col: Color32) {
    let c = r.center();
    let (sin, cos) = if on { (0.0, 1.0) } else { std::f32::consts::FRAC_PI_4.sin_cos() };
    // Local coordinates: the pin points down, its head on top.
    let at = |x: f32, y: f32| c + vec2(x * cos - y * sin, x * sin + y * cos);
    let head = vec![at(-2.5, -7.0), at(2.5, -7.0), at(3.0, -2.0), at(-3.0, -2.0)];
    let stroke = Stroke::new(1.4, col);
    if on {
        p.add(egui::Shape::convex_polygon(head, col, stroke));
    } else {
        p.add(egui::Shape::closed_line(head, stroke));
    }
    p.line_segment([at(-5.5, -2.0), at(5.5, -2.0)], stroke);
    p.line_segment([at(0.0, -2.0), at(0.0, 7.0)], stroke);
}

/// A breadcrumb separator ("Sketch › Create"): a thin chevron about 3 by 6 px, centred on `c`.
/// Drawn rather than typed, because a font's "›" sits on the baseline in some languages
/// (Japanese), where it reads as a comma.
pub(crate) fn breadcrumb(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    let s = Stroke::new(1.0, col);
    p.line_segment([pos2(c.x - 1.5, c.y - 3.0), pos2(c.x + 1.5, c.y)], s);
    p.line_segment([pos2(c.x + 1.5, c.y), pos2(c.x - 1.5, c.y + 3.0)], s);
}

pub(crate) fn brand_mark(p: &egui::Painter, c: Pos2) {
    brand_mark_styled(p, c, 20.0, crate::theme::Tokens::get().logo, true);
}

/// The same original mark as a single quiet silhouette, without face seams or outlines.
pub(crate) fn brand_mark_silhouette(p: &egui::Painter, c: Pos2, height: f32, ink: Color32) {
    brand_mark_styled(p, c, height, [ink; 3], false);
}

/// The Datum Mesh mark (docs/brand/datum-mesh-icon.svg, viewBox 256): an L-bracket meshed in
/// triangles on a pale tile, the inside corner shaded (the stress hot spot), outlined in the
/// accent, with the family's amber diamond beside the loaded end. `edges` false: the bracket
/// alone, as one silhouette in `colours[0]`.
fn brand_mark_styled(p: &egui::Painter, c: Pos2, height: f32, colours: [Color32; 3], edges: bool) {
    let s = height / 240.0;
    let at = |x: f32, y: f32| pos2(c.x + (x - 128.0) * s, c.y + (y - 128.0) * s);
    let m = |pts: &[(f32, f32)]| -> Vec<egui::Pos2> { pts.iter().map(|&(x, y)| at(x, y)).collect() };
    let fill = |col: Color32, pts: &[(f32, f32)]| {
        p.add(egui::Shape::convex_polygon(m(pts), col, Stroke::NONE));
    };
    // The bracket is not convex: its upright and its foot as two pieces.
    let bracket = |col: Color32| {
        fill(col, &[(58.0, 54.0), (104.0, 54.0), (104.0, 152.0), (58.0, 152.0)]);
        fill(col, &[(58.0, 152.0), (202.0, 152.0), (202.0, 196.0), (58.0, 196.0)]);
    };
    if !edges {
        bracket(colours[0]);
        return;
    }
    let accent = Color32::from_rgb(184, 50, 80);
    p.rect(Rect::from_min_max(at(8.0, 8.0), at(248.0, 248.0)), 56.0 * s, Color32::from_rgb(251, 235, 239), Stroke::new(4.0 * s, Color32::from_rgb(241, 179, 193)), egui::StrokeKind::Inside);
    bracket(Color32::from_rgb(241, 179, 193));
    let hot = Color32::from_rgba_unmultiplied(184, 50, 80, 140);
    fill(hot, &[(104.0, 104.0), (104.0, 152.0), (58.0, 152.0)]);
    fill(hot, &[(104.0, 152.0), (152.0, 196.0), (152.0, 152.0)]);
    let mesh = Stroke::new(5.0 * s, accent);
    for (a, b) in [((58.0, 104.0), (104.0, 104.0)), ((58.0, 152.0), (104.0, 152.0)), ((104.0, 152.0), (104.0, 196.0)), ((152.0, 152.0), (152.0, 196.0)), ((58.0, 54.0), (104.0, 104.0)), ((58.0, 104.0), (104.0, 152.0)), ((58.0, 152.0), (104.0, 196.0)), ((104.0, 152.0), (152.0, 196.0)), ((152.0, 152.0), (202.0, 196.0))] {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], mesh);
    }
    p.add(egui::Shape::closed_line(m(&[(58.0, 54.0), (104.0, 54.0), (104.0, 152.0), (202.0, 152.0), (202.0, 196.0), (58.0, 196.0)]), Stroke::new(11.0 * s, accent)));
    fill(Color32::from_rgb(242, 178, 51), &[(220.0, 112.0), (235.0, 127.0), (220.0, 142.0), (205.0, 127.0)]);
    let _ = colours;
}

pub(crate) fn cross(p: &egui::Painter, r: Rect, s: Stroke) {
    p.line_segment([r.left_top(), r.right_bottom()], s);
    p.line_segment([r.right_top(), r.left_bottom()], s);
}
