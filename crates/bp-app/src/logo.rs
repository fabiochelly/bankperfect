//! Symbole BankPerfect, dessiné en vectoriel (même géométrie que
//! `assets/icon/bankperfect-symbole.svg`) : un signe infini dont deux barres
//! parallèles inclinées font un « b » (boucle gauche) et un « p » (boucle
//! droite).

use egui::{Color32, Pos2, Rect, Shape};

/// Demi-largeur de la lemniscate, longueur des barres et épaisseur du trait,
/// dans le repère du dessin d'origine (centré, 512 unités).
const A: f32 = 150.0;
const Y_SCALE: f32 = 1.4;
const STEM: f32 = 190.0;
const WIDTH: f32 = 40.0;
/// Encombrement du symbole (trait compris) dans ce repère.
const EXTENT: f32 = 440.0;

/// Point du dessin d'origine après l'inclinaison de 12°, ramené dans `rect`.
fn map(rect: Rect, x: f32, y: f32) -> Pos2 {
    let skew = (-12.0f32).to_radians().tan();
    let scale = rect.width().min(rect.height()) / EXTENT;
    rect.center() + egui::vec2((x + skew * y) * scale, y * scale)
}

/// Dessine le symbole dans `rect`, en dégradé de `from` (haut gauche) à
/// `to` (bas droite).
pub fn paint_symbol(painter: &egui::Painter, rect: Rect, from: Color32, to: Color32) {
    let scale = rect.width().min(rect.height()) / EXTENT;
    let width = WIDTH * scale;
    let gradient = move |p: Pos2| {
        let t = ((p.x - rect.left()) / rect.width() + (p.y - rect.top()) / rect.height()) / 2.0;
        from.lerp_to_gamma(to, t.clamp(0.0, 1.0))
    };
    let stroke = egui::epaint::PathStroke::new_uv(width, move |_, p| gradient(p));

    let loop_points: Vec<Pos2> = (0..180)
        .map(|k| {
            let t = (k as f32 * 2.0).to_radians();
            let d = 1.0 + t.sin().powi(2);
            map(rect, A * t.cos() / d, Y_SCALE * A * t.sin() * t.cos() / d)
        })
        .collect();
    painter.add(Shape::closed_line(loop_points, stroke.clone()));

    // Barres du b (vers le haut, au bord de la boucle gauche) et du p (vers
    // le bas, depuis le croisement), à bouts ronds.
    for (x, y0, y1) in [(-A, 0.0, -STEM), (0.0, 0.0, STEM)] {
        let (a, b) = (map(rect, x, y0), map(rect, x, y1));
        painter.add(Shape::line(vec![a, b], stroke.clone()));
        painter.circle_filled(b, width / 2.0, gradient(b));
    }
}

/// Icône de la fenêtre (barre des tâches, alt-tab).
pub fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon/bankperfect-256.png"))
        .unwrap_or_default()
}
