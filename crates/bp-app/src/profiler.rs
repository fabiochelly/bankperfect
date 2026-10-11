//! Mesure de la réactivité : temps de chaque image et événements reçus.
//!
//! Activé par `BP_PROFILE=1` : un compteur s'affiche en bas à droite
//! (Ctrl+Alt+P pour le masquer) et chaque image est enregistrée dans
//! `bankperfect-profile.csv` (dossier temporaire du système).

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

use egui::{Align2, Event, RichText};

#[derive(Default, Clone, Copy)]
struct Events {
    moves: u32,
    presses: u32,
    releases: u32,
    wheels: u32,
    keys: u32,
}

#[derive(Clone, Copy)]
struct Frame {
    at: f64,
    interval_ms: f32,
    ui_ms: f32,
}

pub struct Profiler {
    enabled: bool,
    /// `BP_PROFILE=2` : redessine en continu (banc d'essai sans interaction).
    continuous: bool,
    overlay: bool,
    start: Instant,
    last_frame: Option<Instant>,
    events: Events,
    recent: VecDeque<Frame>,
    out: Option<BufWriter<File>>,
}

impl Profiler {
    pub fn from_env() -> Self {
        let level = std::env::var("BP_PROFILE").unwrap_or_default();
        let enabled = !level.is_empty() && level != "0";
        let continuous = level == "2";
        let path = enabled.then(|| std::env::temp_dir().join("bankperfect-profile.csv"));
        let out = path.as_ref().and_then(|p| File::create(p).ok()).map(BufWriter::new).map(|mut w| {
            let _ = writeln!(w, "t_ms;intervalle_ms;ui_ms;cpu_egui_ms_image_prec;deplacements;appuis;relachements;molette;touches");
            let _ = w.flush();
            w
        });
        if let Some(path) = &path {
            eprintln!("BankPerfect : mesures enregistrées dans {}", path.display());
        }
        Self {
            enabled,
            continuous,
            overlay: enabled,
            start: Instant::now(),
            last_frame: None,
            events: Events::default(),
            recent: VecDeque::new(),
            out,
        }
    }

    /// Compte les événements reçus pour l'image à venir.
    pub fn note_events(&mut self, events: &[Event]) {
        if !self.enabled {
            return;
        }
        for event in events {
            match event {
                Event::PointerMoved(_) => self.events.moves += 1,
                Event::PointerButton { pressed: true, .. } => self.events.presses += 1,
                Event::PointerButton { pressed: false, .. } => self.events.releases += 1,
                Event::MouseWheel { .. } => self.events.wheels += 1,
                Event::Key { pressed: true, .. } => self.events.keys += 1,
                _ => {}
            }
        }
    }

    /// Enregistre l'image qui vient d'être construite.
    pub fn end_frame(&mut self, started: Instant, cpu_previous: Option<f32>) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        let ui_ms = (now - started).as_secs_f32() * 1000.0;
        let interval_ms = self.last_frame.map_or(0.0, |last| (started - last).as_secs_f32() * 1000.0);
        self.last_frame = Some(started);
        let at = (started - self.start).as_secs_f64();
        let e = std::mem::take(&mut self.events);
        if let Some(out) = &mut self.out {
            let _ = writeln!(
                out,
                "{:.2};{interval_ms:.2};{ui_ms:.3};{:.3};{};{};{};{};{}",
                at * 1000.0,
                cpu_previous.unwrap_or(0.0) * 1000.0,
                e.moves,
                e.presses,
                e.releases,
                e.wheels,
                e.keys
            );
        }
        // Écrit immédiatement : le fichier reste exploitable si l'application est tuée.
        if let Some(out) = &mut self.out {
            let _ = out.flush();
        }
        self.recent.push_back(Frame { at, interval_ms, ui_ms });
        while self.recent.front().is_some_and(|f| at - f.at > 1.0) {
            self.recent.pop_front();
        }
    }

    /// Compteur à l'écran : images par seconde, temps d'interface, pire intervalle.
    pub fn overlay(&mut self, ctx: &egui::Context, cpu_previous: Option<f32>) {
        if !self.enabled {
            return;
        }
        if self.continuous {
            ctx.request_repaint();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::ALT, egui::Key::P)) {
            self.overlay = !self.overlay;
        }
        if !self.overlay {
            return;
        }
        let frames = self.recent.len();
        let worst_ui = self.recent.iter().map(|f| f.ui_ms).fold(0.0, f32::max);
        let worst_interval = self.recent.iter().skip(1).map(|f| f.interval_ms).fold(0.0, f32::max);
        let last_ui = self.recent.back().map_or(0.0, |f| f.ui_ms);
        let cpu = cpu_previous.unwrap_or(0.0) * 1000.0;
        let text = format!(
            "{frames} img/s · interface {last_ui:.2} ms (pire {worst_ui:.2}) · egui+rendu {:.2} ms · pire intervalle {worst_interval:.1} ms",
            (cpu - last_ui).max(0.0)
        );
        egui::Area::new(egui::Id::new("profiler"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -40.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(200))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| ui.label(RichText::new(text).monospace().color(egui::Color32::from_rgb(0x9e, 0xce, 0x6a))));
            });
    }
}
