//! Curseur de texte animé : il glisse vers sa nouvelle position au lieu de
//! sauter, comme dans Word.
//!
//! egui transmet à chaque image la position du curseur du champ qui a le
//! focus (pour la méthode de saisie) : on s'en sert pour dessiner notre
//! propre curseur, celui d'egui étant rendu transparent par le thème. Les
//! images supplémentaires se limitent à la durée du glissement.

use egui::{Context, Id, Rect, Stroke};

/// Durée du glissement.
const SLIDE_SECS: f64 = 0.09;
/// Épaisseur du curseur (celle du thème).
const WIDTH: f32 = 2.0;

pub struct SmoothCaret {
    /// Champ suivi : changer de champ place le curseur sans glissement.
    field: Option<Id>,
    from: Rect,
    to: Rect,
    start: f64,
    /// Début du cycle de clignotement (dernier mouvement ou frappe).
    blink_from: f64,
}

impl Default for SmoothCaret {
    fn default() -> Self {
        Self {
            field: None,
            from: Rect::NOTHING,
            to: Rect::NOTHING,
            start: 0.0,
            blink_from: 0.0,
        }
    }
}

impl SmoothCaret {
    /// À appeler en fin d'image, une fois tous les champs dessinés.
    /// `composing` : une composition est en cours dans la méthode de saisie,
    /// egui dessine alors la pré-saisie lui-même.
    pub fn paint(&mut self, ctx: &Context, composing: bool) {
        if !crate::settings::shared::smooth_caret() {
            self.field = None;
            return;
        }
        let ime = ctx.output(|o| o.ime);
        let focused = ctx.memory(|m| m.focused());
        let (Some(ime), Some(id)) = (ime, focused) else {
            self.field = None;
            return;
        };
        let Some(response) = ctx.read_response(id) else {
            self.field = None;
            return;
        };

        let now = ctx.input(|i| i.time);
        let target = ime.cursor_rect;
        if self.field != Some(id) {
            *self = Self {
                field: Some(id),
                from: target,
                to: target,
                start: now - SLIDE_SECS,
                blink_from: now,
            };
        } else if target != self.to {
            self.from = self.at(now);
            self.to = target;
            self.start = now;
            self.blink_from = now;
        }
        let typed = ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. }
                        | egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::PointerButton { pressed: true, .. }
                )
            })
        });
        if typed {
            self.blink_from = now;
        }
        // Comme egui : pas de curseur quand la fenêtre n'a pas le focus.
        if composing || !ctx.input(|i| i.focused) {
            return;
        }

        let sliding = now - self.start < SLIDE_SECS;
        if sliding {
            ctx.request_repaint();
        } else {
            let cursor = ctx.style_of(ctx.theme()).visuals.text_cursor.clone();
            if cursor.blink {
                let total = cursor.on_duration + cursor.off_duration;
                let phase = ((now - self.blink_from) % total as f64) as f32;
                if phase >= cursor.on_duration {
                    ctx.request_repaint_after_secs(total - phase);
                    return;
                }
                ctx.request_repaint_after_secs(cursor.on_duration - phase);
            }
        }

        // Dessiné dans la couche du champ, limité à sa zone de texte, pour
        // rester sous les fenêtres qui le recouvrent et ne pas déborder
        // quand le texte défile.
        let layer = response.layer_id;
        let from_global = ctx.layer_transform_from_global(layer).unwrap_or_default();
        let clip = (from_global * ime.rect)
            .expand(WIDTH)
            .intersect(response.interact_rect.expand(WIDTH));
        let rect = from_global * self.at(now);
        let color = crate::theme::palette_of(ctx).accent;
        ctx.layer_painter(layer).with_clip_rect(clip).line_segment(
            [rect.center_top(), rect.center_bottom()],
            Stroke::new(WIDTH, color),
        );
    }

    /// Position du curseur à l'instant `now` (ralenti en fin de course).
    fn at(&self, now: f64) -> Rect {
        let t = ((now - self.start) / SLIDE_SECS).clamp(0.0, 1.0) as f32;
        let eased = 1.0 - (1.0 - t).powi(3);
        Rect::from_min_max(
            self.from.min.lerp(self.to.min, eased),
            self.from.max.lerp(self.to.max, eased),
        )
    }
}
