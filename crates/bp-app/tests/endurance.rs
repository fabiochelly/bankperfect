//! Banc d'endurance : des minutes simulées de survols et de saisies (une
//! opération toutes les 20 s), temps de chaque image mesuré par tranche de
//! 30 s. Une dérive trahirait ce qui s'accumule au fil de l'utilisation.
//! Le fichier est copié dans le dossier temporaire avant d'être modifié.
//! Usage : BP_SCREENSHOT_FILE=<fichier.bp> [BP_MINUTES=10] cargo test --release -p bankperfect --test endurance -- --ignored --nocapture

use std::path::PathBuf;
use std::time::Instant;

use egui_kittest::Harness;
use egui_kittest::kittest::{Queryable, by};

#[test]
#[ignore]
fn endurance() {
    let source = PathBuf::from(std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE"));
    let file = std::env::temp_dir().join("bp-endurance.bp");
    std::fs::copy(&source, &file).unwrap();
    let count = |path: &std::path::Path| -> usize {
        let opened = bp_core::file::open(path, None).unwrap();
        opened
            .document
            .accounts
            .iter()
            .map(|a| a.operations.len())
            .sum()
    };
    let before = count(&file);
    let reopen = file.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_pixels_per_point(1.0)
        .with_step_dt(1.0 / 60.0)
        .build_eframe(|cc| {
            bankperfect::theme::install(&cc.egui_ctx);
            let mut app = bankperfect::BankPerfectApp::with_settings(None, Default::default());
            app.open_file(file);
            app
        });
    harness.run_steps(10);
    let minutes: usize = std::env::var("BP_MINUTES")
        .ok()
        .and_then(|m| m.parse().ok())
        .unwrap_or(10);
    let frames_per_minute = 60 * 60;
    let mut bucket = Vec::new();
    for frame in 0..minutes * frames_per_minute {
        // Survol en va-et-vient sur la grille.
        let x = 300.0 + (frame % 400) as f32 * 2.0;
        let y = 250.0 + ((frame / 7) % 500) as f32;
        harness.hover_at(egui::pos2(x, y));
        // Une opération saisie toutes les 20 s.
        if frame % 1200 == 600 {
            harness.get_by_label("Nouvelle opération").click();
            harness.run_steps(15);
            let placeholder =
                |hint: &'static str| by().predicate(move |n| n.placeholder() == Some(hint));
            harness
                .get(placeholder("Commerçant, organisme, personne…"))
                .focus();
            harness.run_steps(2);
            harness
                .get(placeholder("Commerçant, organisme, personne…"))
                .type_text("Boulangerie");
            harness.run_steps(3);
            harness.get(placeholder("0,00")).focus();
            harness.run_steps(2);
            harness.get(placeholder("0,00")).type_text("12,50");
            harness.run_steps(3);
            harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
            harness.run_steps(15);
            harness.key_press(egui::Key::Escape);
            harness.run_steps(15);
        }
        let started = Instant::now();
        harness.step();
        bucket.push(started.elapsed().as_secs_f64() * 1000.0);
        if bucket.len() == frames_per_minute / 2 {
            bucket.sort_by(f64::total_cmp);
            let mean = bucket.iter().sum::<f64>() / bucket.len() as f64;
            let p95 = bucket[bucket.len() * 95 / 100];
            let data = harness.ctx.data(|d| d.len());
            println!(
                "{:>5.1} min  moyenne {mean:6.2} ms  p95 {p95:6.2} ms  max {:6.2} ms  données egui {data}",
                (frame + 1) as f64 / frames_per_minute as f64,
                bucket[bucket.len() - 1]
            );
            bucket.clear();
        }
    }
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run_steps(10);
    println!("opérations : {before} avant, {} après", count(&reopen));
}
