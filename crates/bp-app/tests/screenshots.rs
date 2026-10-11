//! Captures d'écran rendues hors écran (wgpu), pour vérifier la mise en page.
//! Usage : BP_SCREENSHOT_FILE=<fichier.bp> cargo test -p bankperfect --test screenshots -- --ignored
//! Les images sont écrites dans target/screenshots/.

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// Icône Phosphor du bouton calendrier.
const CALENDAR_DOTS: &str = "\u{E7B4}";

fn render(
    name: &str,
    theme: egui::Theme,
    setup: impl FnOnce(&mut Harness<'_, bankperfect::BankPerfectApp>),
) {
    let file = PathBuf::from(std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE"));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .wgpu()
        .build_eframe(|cc| {
            bankperfect::theme::install(&cc.egui_ctx);
            bankperfect::BankPerfectApp::new(Some(file))
        });
    harness.run_steps(4);
    setup(&mut harness);
    harness.run_steps(20);
    let image = harness.render().expect("rendu");
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    image.save(dir.join(format!("{name}.png"))).unwrap();
}

#[test]
#[ignore]
fn screenshots() {
    render("01-clair", egui::Theme::Light, |_| {});
    render("02-sombre", egui::Theme::Dark, |_| {});
    render("03-saisie", egui::Theme::Dark, |h| {
        h.get_by_label_contains("Nouvelle opération").click()
    });
    render("04-menu", egui::Theme::Dark, |h| {
        h.get_by_label("Édition").click()
    });
    render("06-calendrier", egui::Theme::Dark, |h| {
        h.get_by_label_contains("Nouvelle opération").click();
        h.run_steps(20);
        h.get_by_label(CALENDAR_DOTS).click();
    });
    render("07-plage", egui::Theme::Dark, |h| {
        // Glisser sur la courbe du solde : sélection d'une période.
        let press = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        h.event(egui::Event::PointerMoved(egui::pos2(360.0, 250.0)));
        h.event(press(egui::pos2(360.0, 250.0), true));
        h.run_steps(2);
        for x in (370..=560).step_by(10) {
            h.event(egui::Event::PointerMoved(egui::pos2(x as f32, 250.0)));
            h.run_steps(1);
        }
        h.event(press(egui::pos2(560.0, 250.0), false));
    });
    render("05-menu-survol", egui::Theme::Dark, |h| {
        h.get_by_label("Édition").click();
        h.run_steps(3);
        h.get_by_label("Affichage").hover();
    });
}

/// Temps CPU d'une image (passe egui complète, sans rendu GPU), en faisant
/// défiler la grille, sur le fichier indiqué.
#[test]
#[ignore]
fn frame_time() {
    let file = PathBuf::from(std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE"));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .build_eframe(|cc| {
            bankperfect::theme::install(&cc.egui_ctx);
            bankperfect::BankPerfectApp::new(Some(file))
        });
    harness.run_steps(5);
    let frames = 300;
    let start = std::time::Instant::now();
    for i in 0..frames {
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, if i % 100 < 50 { -1.0 } else { 1.0 }),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    let per_frame = start.elapsed() / frames;
    println!("temps CPU par image (défilement) : {per_frame:.2?}");
}
