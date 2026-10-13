//! Mise en page en îles : chaque zone de la fenêtre (comptes, soldes,
//! tableau) est une île arrondie posée sur le fond de la fenêtre (le
//! canevas). En sombre, une vitre fumée : un voile et une bordure fine, sans
//! ombre ni relief ; en clair, un fond plus clair avec une ombre douce.
//!
//! egui ne sait pas découper un dessin selon un rectangle arrondi : les
//! dégradés en maillage d'une île sont donc construits directement à
//! l'intérieur de son contour (`rounded_span`, `rounded_gradient`).

use egui::{
    Color32, CornerRadius, Frame, Painter, Pos2, Rect, Shadow, Stroke, StrokeKind, pos2, vec2,
};

use crate::theme::{Palette, backdrop};

/// Rayon des coins d'une île.
pub const RADIUS: f32 = 16.0;
/// Espace entre deux îles, et entre une île et le bord de la fenêtre.
pub const GAP: f32 = 10.0;

/// Fond de la fenêtre, sous les îles.
pub fn canvas(p: &Palette, dark: bool) -> Color32 {
    if dark {
        backdrop(p.bg, 0.84)
    } else {
        backdrop(p.bg.lerp_to_gamma(Color32::BLACK, 0.07), 0.76)
    }
}

/// Fond d'une île : en sombre, un voile de vitre fumée posé sur le canevas ;
/// en clair, un fond plus clair que la fenêtre, avec une ombre douce.
pub fn fill(p: &Palette, dark: bool) -> Color32 {
    if dark {
        Color32::from_black_alpha(70)
    } else {
        backdrop(p.bg.lerp_to_gamma(Color32::WHITE, 0.55), 0.92)
    }
}

fn stroke(p: &Palette, dark: bool) -> Stroke {
    if dark {
        Stroke::new(1.0, Color32::from_white_alpha(20))
    } else {
        Stroke::new(1.0, p.border.gamma_multiply(0.6))
    }
}

/// Ombre portée : en clair seulement (la vitre fumée n'a pas de relief).
fn shadow(dark: bool) -> Shadow {
    if dark {
        Shadow::NONE
    } else {
        Shadow {
            offset: [0, 4],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(22),
        }
    }
}

/// Cadre d'un panneau en île (sans marge intérieure : à l'appelant).
pub fn frame(p: &Palette, dark: bool) -> Frame {
    Frame::new()
        .fill(fill(p, dark))
        .stroke(stroke(p, dark))
        .corner_radius(CornerRadius::same(RADIUS as u8))
        .shadow(shadow(dark))
}

/// Ombre et fond d'une île dessinée à la main.
pub fn paint(painter: &Painter, rect: Rect, p: &Palette, dark: bool) {
    painter.add(shadow(dark).as_shape(rect, RADIUS as u8));
    painter.rect_filled(rect, RADIUS, fill(p, dark));
}

/// Liseré d'une île dessinée à la main, à poser après son contenu.
pub fn outline(painter: &Painter, rect: Rect, p: &Palette, dark: bool) {
    painter.rect_stroke(rect, RADIUS, stroke(p, dark), StrokeKind::Inside);
}

/// Haut et bas du rectangle arrondi `rect` (rayon `r`) à l'abscisse `x`.
pub fn rounded_span(rect: Rect, r: f32, x: f32) -> (f32, f32) {
    let dx = if x < rect.left() + r {
        rect.left() + r - x
    } else if x > rect.right() - r {
        x - (rect.right() - r)
    } else {
        0.0
    };
    let dx = dx.min(r);
    let inset = r - (r * r - dx * dx).max(0.0).sqrt();
    (rect.top() + inset, rect.bottom() - inset)
}

/// Normale sortante du rectangle arrondi `rect` (rayon `r`) au point `p` de
/// son contour.
pub fn rounded_normal(rect: Rect, r: f32, p: Pos2) -> egui::Vec2 {
    let inner = rect.shrink(r);
    let nearest = pos2(
        p.x.clamp(inner.left(), inner.right()),
        p.y.clamp(inner.top(), inner.bottom()),
    );
    let n = p - nearest;
    if n.length_sq() > 1e-6 {
        return n.normalized();
    }
    // Sur un côté droit : le bord le plus proche.
    let d = [
        (p.x - rect.left(), vec2(-1.0, 0.0)),
        (rect.right() - p.x, vec2(1.0, 0.0)),
        (p.y - rect.top(), vec2(0.0, -1.0)),
        (rect.bottom() - p.y, vec2(0.0, 1.0)),
    ];
    d.iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|d| d.1)
        .unwrap_or(vec2(0.0, 1.0))
}

/// Contour du rectangle arrondi (sens horaire depuis le haut à droite), avec
/// la normale sortante de chaque point.
fn rounded_outline(rect: Rect, r: f32, arc: usize) -> Vec<(Pos2, egui::Vec2)> {
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -90.0f32),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
        (pos2(rect.left() + r, rect.top() + r), 180.0),
    ];
    let mut outline = Vec::with_capacity(4 * (arc + 1));
    for (center, start) in corners {
        for i in 0..=arc {
            let a = (start + 90.0 * i as f32 / arc as f32).to_radians();
            let n = vec2(a.cos(), a.sin());
            outline.push((center + n * r, n));
        }
    }
    outline
}

/// Rectangle arrondi rempli d'un dégradé libre : `color_at` donne la couleur
/// en chaque point. Le maillage suit le contour puis des anneaux concentriques
/// vers le centre, assez serrés pour un dégradé doux. Le bord est adouci sur
/// un pixel physique, comme les formes d'egui (sinon les coins crénellent).
pub fn rounded_gradient(painter: &Painter, rect: Rect, r: f32, color_at: impl Fn(Pos2) -> Color32) {
    const ARC: usize = 16;
    const RINGS: usize = 10;
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let feather = 1.0 / painter.ctx().pixels_per_point();
    let outline = rounded_outline(rect, r, ARC);
    let center = rect.center();
    let n = outline.len() as u32;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color_at(center));
    // Anneau k (1 = le plus proche du centre, RINGS = le contour, rentré d'un
    // demi-pixel), puis l'anneau transparent, sorti d'un demi-pixel.
    for k in 1..=RINGS {
        let s = k as f32 / RINGS as f32;
        for (p, normal) in &outline {
            let q = center + (*p - *normal * feather / 2.0 - center) * s;
            mesh.colored_vertex(q, color_at(q));
        }
    }
    for (p, normal) in &outline {
        mesh.colored_vertex(*p + *normal * feather / 2.0, Color32::TRANSPARENT);
    }
    let ring = |k: u32, i: u32| 1 + (k - 1) * n + (i % n);
    for i in 0..n {
        mesh.add_triangle(0, ring(1, i), ring(1, i + 1));
    }
    for k in 1..=RINGS as u32 {
        for i in 0..n {
            mesh.add_triangle(ring(k, i), ring(k + 1, i), ring(k, i + 1));
            mesh.add_triangle(ring(k, i + 1), ring(k + 1, i), ring(k + 1, i + 1));
        }
    }
    painter.add(mesh);
}

/// Liseré fin à l'intérieur du rectangle arrondi, blanc plus opaque en haut
/// (`top_alpha`) qu'en bas (`bottom_alpha`) : un léger relief, sans ombre.
pub fn lit_edge(painter: &Painter, rect: Rect, r: f32, width: f32, top_alpha: u8, bottom_alpha: u8) {
    let edge = rect.shrink(width / 2.0);
    let points: Vec<Pos2> = rounded_outline(edge, (r - width / 2.0).max(0.0), 16)
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    let (top, height) = (edge.top(), edge.height().max(1.0));
    let stroke = egui::epaint::PathStroke::new_uv(width, move |_, p: Pos2| {
        let t = ((p.y - top) / height).clamp(0.0, 1.0);
        let a = top_alpha as f32 + (bottom_alpha as f32 - top_alpha as f32) * t;
        Color32::from_white_alpha(a.round() as u8)
    });
    painter.add(egui::epaint::PathShape::closed_line(points, stroke));
}

/// Mélange linéaire de couleurs (alpha compris) le long de `stops`
/// (positions croissantes de 0 à 1).
pub fn gradient(stops: &[(f32, Color32)], t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if t <= b {
            let u = if b > a { (t - a) / (b - a) } else { 0.0 };
            return lerp(ca, cb, u);
        }
    }
    stops.last().map(|s| s.1).unwrap_or(Color32::TRANSPARENT)
}

/// Mélange de deux couleurs prémultipliées (alpha compris).
pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}
