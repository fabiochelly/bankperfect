//! Symbole BankPerfect, dessiné en vectoriel (même géométrie que
//! `assets/icon/bankperfect-symbole.svg`) : un signe infini dont deux barres
//! parallèles inclinées font un « b » (boucle gauche) et un « p » (boucle
//! droite).

use egui::{Color32, Pos2, Rect, Shape};

/// Demi-largeur de la lemniscate, longueur des barres et épaisseur du trait,
/// dans le repère du dessin d'origine (centré, 512 unités).
const A: f32 = 150.0;
const Y_SCALE: f32 = 1.4;
const STEM: f32 = 160.0;
const WIDTH: f32 = 40.0;
/// Encombrement du symbole (trait compris) dans ce repère.
const EXTENT: f32 = 368.0;

/// Point du dessin d'origine après l'inclinaison de 12°, ramené dans `rect`.
fn map(rect: Rect, x: f32, y: f32) -> Pos2 {
    let skew = (-12.0f32).to_radians().tan();
    let scale = rect.width().min(rect.height()) / EXTENT;
    rect.center() + egui::vec2((x + skew * y) * scale, y * scale)
}

/// Dégradé d'une lettre : couleur du haut à gauche, couleur du bas à droite.
pub type Gradient = (Color32, Color32);

/// Dessine le symbole dans `rect`. Le « b » (boucle et barre de gauche) et
/// le « p » (boucle droite et barre du centre) ont chacun leur dégradé, pour
/// qu'on lise deux lettres.
pub fn paint_symbol(painter: &egui::Painter, rect: Rect, b: Gradient, p: Gradient) {
    let scale = rect.width().min(rect.height()) / EXTENT;
    let width = WIDTH * scale;
    let shade = move |(from, to): Gradient| {
        move |pt: Pos2| {
            let t =
                ((pt.x - rect.left()) / rect.width() + (pt.y - rect.top()) / rect.height()) / 2.0;
            from.lerp_to_gamma(to, t.clamp(0.0, 1.0))
        }
    };
    let point = |degrees: f32| {
        let t = degrees.to_radians();
        let d = 1.0 + t.sin().powi(2);
        map(rect, A * t.cos() / d, Y_SCALE * A * t.sin() * t.cos() / d)
    };
    // Boucle d'une lettre : demi-lemniscate (de -90° à 90° pour la droite,
    // de 90° à 270° pour la gauche), qui part du croisement et y revient.
    let half =
        |from: f32| -> Vec<Pos2> { (0..=90).map(|k| point(from + k as f32 * 2.0)).collect() };
    let center = map(rect, 0.0, 0.0);
    for (gradient, loop_start, stem) in [(p, -90.0, (0.0, 0.0, STEM)), (b, 90.0, (-A, 0.0, -STEM))]
    {
        let color = shade(gradient);
        let stroke = egui::epaint::PathStroke::new_uv(width, move |_, pt| color(pt));
        painter.add(Shape::line(half(loop_start), stroke.clone()));
        // Coin arrondi au croisement, dans la couleur de la lettre.
        painter.circle_filled(center, width / 2.0, color(center));
        let (x, y0, y1) = stem;
        let (a, end) = (map(rect, x, y0), map(rect, x, y1));
        painter.add(Shape::line(vec![a, end], stroke));
        painter.circle_filled(end, width / 2.0, color(end));
    }
}

/// Icône de la fenêtre (barre des tâches, alt-tab).
pub fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon/bankperfect-256.png"))
        .unwrap_or_default()
}
