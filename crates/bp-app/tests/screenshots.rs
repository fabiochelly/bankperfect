//! Captures d'écran rendues hors écran (wgpu), pour vérifier la mise en page.
//! Usage : BP_SCREENSHOT_FILE=<fichier.bp> cargo test -p bankperfect --test screenshots -- --ignored
//! Les images sont écrites dans target/screenshots/.

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn render(name: &str, theme: egui::Theme, setup: impl FnOnce(&mut Harness<'_, bankperfect::BankPerfectApp>)) {
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
    harness.run_steps(4);
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
    render("03-saisie", egui::Theme::Light, |h| h.get_by_label_contains("Nouvelle opération").click());
}
