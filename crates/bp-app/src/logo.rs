//! Logo BankPerfect, dessiné en vectoriel (même géométrie que
//! `assets/icon/bankperfect.svg`) : carré arrondi en dégradé et monogramme
//! « bp » dont le b et le p partagent une tige.

use egui::{Color32, Mesh, Pos2, Rect, Shape, Stroke, pos2};

/// Couleurs du dégradé (diagonale haut-gauche → bas-droite).
const STOPS: [(f32, Color32); 3] = [
    (0.0, Color32::from_rgb(0x7a, 0xa2, 0xf7)),
    (0.55, Color32::from_rgb(0x5a, 0x7f, 0xe0)),
    (1.0, Color32::from_rgb(0x8b, 0x6f, 0xd6)),
];

fn gradient(t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    for pair in STOPS.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            return c0.lerp_to_gamma(c1, (t - t0) / (t1 - t0));
        }
    }
    STOPS[2].1
}

/// Point du dessin d'origine (512 × 512, y relatif au centre) après
/// l'inclinaison de 10°, ramené dans `rect`.
fn map(rect: Rect, x: f32, y: f32) -> Pos2 {
    let skew = (-10.0f32).to_radians().tan();
    let (sx, sy) = (x + skew * y, 256.0 + y);
    pos2(
        rect.left() + sx / 512.0 * rect.width(),
        rect.top() + sy / 512.0 * rect.height(),
    )
}

/// Dessine le logo dans le carré `rect`.
pub fn paint(painter: &egui::Painter, rect: Rect) {
    let size = rect.width();
    let scale = size / 512.0;
    let body = Rect::from_min_size(
        rect.min + egui::vec2(16.0, 16.0) * scale,
        egui::vec2(480.0, 480.0) * scale,
    );
    let radius = 116.0 * scale;

    // Fond : maillage en éventail sur le contour arrondi, couleurs du
    // dégradé et léger reflet clair en haut.
    let color_at = |p: Pos2| {
        let u = (p.x - body.left()) / body.width();
        let v = (p.y - body.top()) / body.height();
        let base = gradient((u + v) / 2.0);
        let shine = (0.22 * (1.0 - 2.0 * v)).max(0.0);
        base.lerp_to_gamma(Color32::WHITE, shine)
    };
    let mut outline = Vec::new();
    let corners = [
        (pos2(body.right() - radius, body.top() + radius), -90.0f32),
        (pos2(body.right() - radius, body.bottom() - radius), 0.0),
        (pos2(body.left() + radius, body.bottom() - radius), 90.0),
        (pos2(body.left() + radius, body.top() + radius), 180.0),
    ];
    for (center, start) in corners {
        for k in 0..=12 {
            let a = (start + 90.0 * k as f32 / 12.0).to_radians();
            outline.push(center + egui::vec2(a.cos(), a.sin()) * radius);
        }
    }
    let mut mesh = Mesh::default();
    let c = body.center();
    mesh.colored_vertex(c, color_at(c));
    for &p in &outline {
        mesh.colored_vertex(p, color_at(p));
    }
    let n = outline.len() as u32;
    for k in 0..n {
        mesh.add_triangle(0, 1 + k, 1 + (k + 1) % n);
    }
    painter.add(Shape::mesh(mesh));

    // Monogramme.
    let width = 40.0 * scale;
    let stroke = Stroke::new(width, Color32::WHITE);
    let stem = |x: f32, y0: f32, y1: f32| {
        let (a, b) = (map(rect, x, y0), map(rect, x, y1));
        painter.line_segment([a, b], stroke);
        painter.circle_filled(a, width / 2.0, Color32::WHITE);
        painter.circle_filled(b, width / 2.0, Color32::WHITE);
    };
    let bowl = |cx: f32| {
        let points: Vec<Pos2> = (0..72)
            .map(|k| {
                let a = (k as f32 * 5.0).to_radians();
                map(rect, cx + 62.0 * a.cos(), -20.0 + 62.0 * a.sin())
            })
            .collect();
        painter.add(Shape::closed_line(points, stroke));
    };
    stem(132.0, -170.0, -20.0);
    bowl(194.0);
    stem(256.0, -20.0, 170.0);
    bowl(318.0);
}

/// Icône de la fenêtre (barre des tâches, alt-tab).
pub fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon/bankperfect-256.png"))
        .unwrap_or_default()
}
