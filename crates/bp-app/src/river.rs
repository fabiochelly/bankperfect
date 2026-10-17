//! Courbe du solde derrière la colonne Solde de la grille : chaque ligne
//! dessine sa part d'une aire continue qui suit le solde d'une ligne à
//! l'autre. Au survol de la cellule, une infobulle montre l'évolution du
//! solde un mois avant et après l'opération.

use bp_core::model::{Amount, Date};
use egui::{Color32, Pos2, Rangef, Stroke};

use crate::format::money;
use crate::theme::Palette;

/// Points par demi-ligne pour lisser le passage d'un solde au suivant.
const SAMPLES: usize = 8;
/// Durée du déploiement de la courbe à l'ouverture d'un compte (secondes).
const GROW_TIME: f64 = 0.45;
/// Demi-largeur de la fenêtre de l'infobulle, en jours.
const TOOLTIP_DAYS: i32 = 30;

/// Échelle des soldes des lignes à l'écran.
pub struct River {
    lo: f64,
    hi: f64,
    /// Déploiement à l'ouverture du compte, de 0 à 1.
    grow: f32,
}

impl River {
    /// Zéro est sur l'échelle quand le compte passe sous zéro ; sinon le bas
    /// est un peu sous le plus petit solde, pour que les variations se voient.
    /// L'échelle suit le défilement en douceur. `None` si le solde ne varie
    /// pas.
    pub fn new(
        ctx: &egui::Context,
        key: (u64, usize),
        range: Option<(Amount, Amount)>,
    ) -> Option<River> {
        let (min, max) = range?;
        let (min, max) = (min.as_f64(), max.as_f64());
        let span = max - min;
        if span < 0.01 {
            return None;
        }
        let lo = if min >= 0.0 {
            (min - span * 0.25).max(0.0)
        } else {
            min
        };
        // Nouvelle échelle à chaque compte, sans transition depuis l'ancien.
        let ease = |name: &str, value: f64| {
            f64::from(ctx.animate_value_with_time(egui::Id::new((name, key)), value as f32, 0.3))
        };
        let (lo, hi) = (ease("river_lo", lo), ease("river_hi", max.max(0.0)));
        (hi - lo > 0.0).then(|| River {
            lo,
            hi,
            grow: grow(ctx, key),
        })
    }

    /// Position du solde `value` dans `span`.
    fn x(&self, span: Rangef, value: f64) -> f32 {
        let t = ((value - self.lo) / (self.hi - self.lo)).clamp(0.0, 1.0) as f32;
        span.min + t * span.span()
    }

    /// Part de la courbe pour une ligne : de sa limite haute `band.min` à sa
    /// limite basse `band.max`, en passant par son solde au milieu
    /// (`center`). Aux limites, la courbe est à mi-chemin des soldes des
    /// lignes voisines (`values` : dessus, ligne, dessous), ce qui la rend
    /// continue d'une ligne à l'autre.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        painter: &egui::Painter,
        span: Rangef,
        band: Rangef,
        center: f32,
        values: [f64; 3],
        hovered: bool,
        pal: &Palette,
        dark: bool,
    ) {
        let base = self.x(span, 0.0);
        let [above, own, below] = values.map(|v| base + (self.x(span, v) - base) * self.grow);
        let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
        // Moitié haute : fin du passage depuis la ligne du dessus ; moitié
        // basse : début du passage vers celle du dessous. Un point de plus de
        // chaque côté, coupé par le clip, évite une couture entre les lignes.
        let mut points = Vec::with_capacity(2 * SAMPLES + 3);
        for k in 0..=SAMPLES + 1 {
            let u = k as f32 / SAMPLES as f32 - 1.0 / SAMPLES as f32;
            let t = 0.5 + 0.5 * u;
            let x = above + (own - above) * smooth(t.clamp(0.0, 1.0));
            points.push(Pos2::new(x, band.min + u * (center - band.min)));
        }
        for k in 1..=SAMPLES + 1 {
            let u = k as f32 / SAMPLES as f32;
            let t = 0.5 * u;
            let x = own + (below - own) * smooth(t.clamp(0.0, 1.0));
            points.push(Pos2::new(x, center + u * (band.max - center)));
        }
        let points = with_crossings(&points, base);

        // Dégradé proportionnel à la distance à la base (linéaire, donc rendu
        // exact par les triangles) : plus le solde s'éloigne, plus l'aire est
        // vive au bord de la courbe.
        let alpha = if dark { 0.34 } else { 0.26 };
        let reach = (span.max - base).max(base - span.min).max(1.0);
        let shade = |color: Color32, x: f32| color.gamma_multiply(alpha * (x - base).abs() / reach);
        let side = |a: Pos2, b: Pos2| {
            if (a.x + b.x) / 2.0 >= base {
                pal.accent
            } else {
                pal.negative
            }
        };
        let mut mesh = egui::Mesh::default();
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let color = side(a, b);
            let i = mesh.vertices.len() as u32;
            mesh.colored_vertex(Pos2::new(base, a.y), Color32::TRANSPARENT);
            mesh.colored_vertex(a, shade(color, a.x));
            mesh.colored_vertex(b, shade(color, b.x));
            mesh.colored_vertex(Pos2::new(base, b.y), Color32::TRANSPARENT);
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        }
        painter.add(mesh);

        // Axe du zéro quand le compte passe sous zéro, en pointillés (pour ne
        // pas le confondre avec un séparateur de colonne) calés sur la
        // fenêtre, donc continus d'une ligne à l'autre.
        if self.lo < 0.0 && self.hi > 0.0 {
            const DASH: f32 = 3.0;
            let pixels = painter.pixels_per_point();
            let x = (base * pixels).round() / pixels + 0.5 / pixels;
            let stroke = Stroke::new(1.0 / pixels, pal.muted.gamma_multiply(0.6));
            let mut y = (band.min / (2.0 * DASH)).floor() * 2.0 * DASH;
            while y < band.max {
                painter.vline(x, Rangef::new(y, y + DASH), stroke);
                y += 2.0 * DASH;
            }
        }

        // Liseré : la courbe elle-même, par tronçons d'un même côté du zéro.
        let opacity = if dark { 0.75 } else { 0.7 };
        let mut run: Vec<Pos2> = Vec::new();
        let mut run_color = None;
        for pair in points.windows(2) {
            let color = side(pair[0], pair[1]);
            if run_color != Some(color) {
                if let Some(c) = run_color {
                    painter.line(
                        std::mem::take(&mut run),
                        Stroke::new(1.25, c.gamma_multiply(opacity)),
                    );
                }
                run.push(pair[0]);
                run_color = Some(color);
            }
            run.push(pair[1]);
        }
        if let Some(c) = run_color {
            painter.line(run, Stroke::new(1.25, c.gamma_multiply(opacity)));
        }

        if hovered {
            let color = if values[1] < 0.0 {
                pal.negative
            } else {
                pal.accent
            };
            painter.circle_filled(Pos2::new(own, center), 3.5, color);
        }
    }
}

/// Déploiement de la courbe, de 0 à 1, relancé à chaque changement de
/// fichier ou de compte (`key`).
fn grow(ctx: &egui::Context, key: (u64, usize)) -> f32 {
    let id = egui::Id::new("balance_river_grow");
    let now = ctx.input(|i| i.time);
    let start = match ctx.data(|d| d.get_temp::<((u64, usize), f64)>(id)) {
        Some((k, start)) if k == key => start,
        _ => {
            ctx.data_mut(|d| d.insert_temp(id, (key, now)));
            now
        }
    };
    let t = ((now - start) / GROW_TIME).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    // Sortie amortie.
    1.0 - (1.0 - t).powi(3)
}

/// La courbe ne se redéploiera qu'au prochain affichage (courbe masquée).
pub fn reset(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<((u64, usize), f64)>(egui::Id::new("balance_river_grow")));
}

/// Ajoute les points où la courbe croise `base`, pour que chaque tronçon
/// reste d'un seul côté (couleur du positif ou du négatif).
fn with_crossings(points: &[Pos2], base: f32) -> Vec<Pos2> {
    let mut out = Vec::with_capacity(points.len() + 2);
    for (i, &p) in points.iter().enumerate() {
        if i > 0 {
            let a = points[i - 1];
            if (a.x - base) * (p.x - base) < 0.0 {
                let t = (base - a.x) / (p.x - a.x);
                out.push(Pos2::new(base, a.y + t * (p.y - a.y)));
            }
        }
        out.push(p);
    }
    out
}

/// Infobulle à afficher pour la cellule `cell` : pointeur dessus et immobile
/// depuis un instant. Gérée ici plutôt que par `on_hover_ui`, dont les
/// règles (une seule infobulle par calque, rien juste après un clic…) la
/// masquaient sur la grille.
pub fn wants_tooltip(cell: &egui::Response, clip: egui::Rect) -> bool {
    const DELAY: f32 = 0.35;
    let ctx = &cell.ctx;
    if !ctx.rect_contains_pointer(cell.layer_id, cell.rect.intersect(clip)) {
        return false;
    }
    let (still, busy) = ctx.input(|i| {
        (
            i.pointer
                .time_since_last_movement()
                .min(i.time_since_last_scroll()),
            i.pointer.any_down(),
        )
    });
    if busy {
        return false;
    }
    if still < DELAY {
        ctx.request_repaint_after_secs(DELAY - still);
        return false;
    }
    true
}

/// Une opération de la fenêtre de l'infobulle.
pub struct Point {
    pub date: Date,
    pub balance: Amount,
}

/// Infobulle du solde : évolution sur un mois de part et d'autre de
/// l'opération `marked` de `points` (par date croissante, une entrée par
/// opération).
pub fn tooltip(ui: &mut egui::Ui, points: &[Point], marked: usize, currency: &str, pal: &Palette) {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return;
    };
    ui.label(
        egui::RichText::new(format!(
            "Solde du {} au {}",
            crate::settings::shared::date(first.date),
            crate::settings::shared::date(last.date)
        ))
        .small()
        .color(pal.muted),
    );
    ui.add_space(4.0);

    // Opérations d'un même jour réparties dans la journée : la courbe passe
    // par le solde de chacune.
    let mut xs = Vec::with_capacity(points.len());
    let mut start = 0;
    for i in 0..=points.len() {
        if i == points.len() || points[i].date != points[start].date {
            let n = (i - start) as f64;
            for (k, point) in points[start..i].iter().enumerate() {
                xs.push(f64::from(point.date.0) + k as f64 / n);
            }
            start = i;
        }
    }
    let values: Vec<f64> = points.iter().map(|p| p.balance.as_f64()).collect();
    let (min_i, max_i) = values.iter().enumerate().fold((0, 0), |(lo, hi), (i, &v)| {
        (
            if v < values[lo] { i } else { lo },
            if v > values[hi] { i } else { hi },
        )
    });
    let (lo, hi) = (values[min_i], values[max_i]);
    let pad = ((hi - lo) * 0.12).max(1.0);
    let (lo, hi) = (lo - pad, hi + pad);
    let (x0, x1) = (xs[0], xs[xs.len() - 1].max(xs[0] + 1.0));

    let (rect, _) = ui.allocate_exact_size(egui::vec2(300.0, 110.0), egui::Sense::hover());
    let painter = ui.painter_at(rect.expand(4.0));
    let to_screen = |x: f64, v: f64| {
        Pos2::new(
            rect.left() + ((x - x0) / (x1 - x0)) as f32 * rect.width(),
            rect.bottom() - ((v - lo) / (hi - lo)) as f32 * rect.height(),
        )
    };
    let line: Vec<Pos2> = xs
        .iter()
        .zip(&values)
        .map(|(&x, &v)| to_screen(x, v))
        .collect();

    // Aire en dégradé vertical sous la courbe (linéaire en hauteur : rendu
    // exact par les triangles).
    let mut mesh = egui::Mesh::default();
    let shade = |p: Pos2| {
        pal.accent
            .gamma_multiply(0.4 * (rect.bottom() - p.y) / rect.height())
    };
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(Pos2::new(a.x, rect.bottom()), Color32::TRANSPARENT);
        mesh.colored_vertex(a, shade(a));
        mesh.colored_vertex(b, shade(b));
        mesh.colored_vertex(Pos2::new(b.x, rect.bottom()), Color32::TRANSPARENT);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    painter.add(mesh);

    let faint = Stroke::new(1.0, pal.muted.gamma_multiply(0.5));
    if lo < 0.0 && hi > 0.0 {
        let y = to_screen(x0, 0.0).y;
        painter.extend(egui::Shape::dashed_line(
            &[Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            faint,
            4.0,
            3.0,
        ));
    }
    let today = f64::from(Date::today().0);
    if today > x0 && today < x1 {
        let x = to_screen(today, lo).x;
        painter.extend(egui::Shape::dashed_line(
            &[Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            faint,
            2.0,
            3.0,
        ));
    }
    painter.line(line.clone(), Stroke::new(1.75, pal.accent));

    // Opération survolée, plus bas et plus haut de la fenêtre.
    let marker = line[marked];
    painter.vline(
        marker.x,
        rect.y_range(),
        Stroke::new(1.0, pal.accent.gamma_multiply(0.35)),
    );
    for (i, color) in [(min_i, pal.negative), (max_i, pal.positive)] {
        if i != marked {
            painter.circle_filled(line[i], 2.5, color);
        }
    }
    let own = if values[marked] < 0.0 {
        pal.negative
    } else {
        pal.accent
    };
    painter.circle(marker, 4.5, own, Stroke::new(2.0, ui.visuals().window_fill));

    ui.add_space(6.0);
    let small = |text: String, color: Color32| egui::RichText::new(text).small().color(color);
    ui.horizontal(|ui| {
        ui.label(small(
            format!(
                "{} {} le {}",
                egui_phosphor::regular::ARROW_DOWN,
                money(points[min_i].balance, currency),
                crate::settings::shared::date(points[min_i].date)
            ),
            pal.negative,
        ));
        ui.add_space(10.0);
        ui.label(small(
            format!(
                "{} {} le {}",
                egui_phosphor::regular::ARROW_UP,
                money(points[max_i].balance, currency),
                crate::settings::shared::date(points[max_i].date)
            ),
            pal.positive,
        ));
    });
}

/// Opérations de la fenêtre de l'infobulle autour de la ligne `pos` de
/// `rows` (lignes affichées, triées par date), par date croissante, avec la
/// place de cette ligne parmi elles.
pub fn window(
    rows: &[usize],
    pos: usize,
    date_of: impl Fn(usize) -> Date,
    balance_of: impl Fn(usize) -> Amount,
    ascending: bool,
) -> (Vec<Point>, usize) {
    let center = date_of(rows[pos]).0;
    let inside = |i: usize| (date_of(rows[i]).0 - center).abs() <= TOOLTIP_DAYS;
    let mut from = pos;
    while from > 0 && inside(from - 1) {
        from -= 1;
    }
    let mut to = pos;
    while to + 1 < rows.len() && inside(to + 1) {
        to += 1;
    }
    let point = |i: usize| Point {
        date: date_of(rows[i]),
        balance: balance_of(rows[i]),
    };
    if ascending {
        ((from..=to).map(point).collect(), pos - from)
    } else {
        ((from..=to).rev().map(point).collect(), to - pos)
    }
}
