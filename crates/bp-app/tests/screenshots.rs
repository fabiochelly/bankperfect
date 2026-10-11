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
    render_sized(name, theme, egui::vec2(1440.0, 900.0), setup);
}

fn render_sized(
    name: &str,
    theme: egui::Theme,
    size: egui::Vec2,
    setup: impl FnOnce(&mut Harness<'_, bankperfect::BankPerfectApp>),
) {
    let file = PathBuf::from(std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE"));
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .wgpu()
        .build_eframe(|cc| {
            bankperfect::theme::install(&cc.egui_ctx);
            bankperfect::BankPerfectApp::with_settings(Some(file), Default::default())
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
        h.event(egui::Event::PointerMoved(egui::pos2(1275.0, 160.0)));
        h.event(press(egui::pos2(1275.0, 160.0), true));
        h.run_steps(2);
        for x in (1285..=1385).step_by(10) {
            h.event(egui::Event::PointerMoved(egui::pos2(x as f32, 160.0)));
            h.run_steps(1);
        }
        h.event(press(egui::pos2(1385.0, 160.0), false));
        h.run_steps(3);
        // Survol du badge de filtre : la croix passe au rouge.
        h.event(egui::Event::PointerMoved(egui::pos2(440.0, 881.0)));
    });
    render("08-remplacer", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::R);
        h.run_steps(2);
        h.event(egui::Event::Text("test".into()));
    });
    render("09-export-csv", egui::Theme::Dark, |h| {
        h.get_by_label("Fichier").click();
        h.run_steps(3);
        h.get_by_label("Exporter en CSV…").click();
        h.run_steps(2);
    });
    render("10-groupe", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::PageDown);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::M);
        h.run_steps(2);
        h.get_by_label(&format!("{}  Tiers", egui_phosphor::regular::USER))
            .click();
    });
    render("11-champs", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press(egui::Key::F3);
    });
    render("12-notes", egui::Theme::Dark, |h| {
        h.key_press_modifiers(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::N,
        );
    });
    render("13-calculatrice", egui::Theme::Dark, |h| {
        h.key_press_modifiers(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::C,
        );
        h.run_steps(2);
        h.get_by_label("7").click();
        h.get_by_label("*").click();
        h.get_by_label("cos").click();
        h.get_by_label("0").click();
        h.get_by_label(")").click();
    });
    render("14-gestion-comptes", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::G);
    });
    render("15-gestion-categories", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::G);
        h.run_steps(2);
        h.get_by_label("Catégories").click();
        h.run_steps(2);
        h.query_all_by_label_contains("Alimentation")
            .last()
            .unwrap()
            .click();
    });
    render("16-gestion-tiers", egui::Theme::Light, |h| {
        h.key_press(egui::Key::F5);
        h.run_steps(2);
        h.query_all_by_label("EDF").last().unwrap().click();
    });
    render("17-protection", egui::Theme::Dark, |h| {
        h.get_by_label("Fichier").click();
        h.run_steps(3);
        h.get_by_label("Protection du fichier…").click();
        h.run_steps(2);
        h.get_by_label("Protéger").click();
    });
    render("18-palette", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    });
    render("19-palette-recherche", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("categ".into()));
    });
    render("20-palette-clair", egui::Theme::Light, |h| {
        h.key_press(egui::Key::F1);
        h.run_steps(2);
        h.event(egui::Event::Text("compte".into()));
        h.run_steps(2);
        h.key_press(egui::Key::ArrowDown);
    });
    render("21-non-enregistre", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.key_press(egui::Key::Space);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::O);
    });
    render("22-sauvegardes", egui::Theme::Dark, |h| {
        h.get_by_label("Fichier").click();
        h.run_steps(3);
        h.get_by_label("Sauvegardes…").click();
    });
    render_sized(
        "23-etroit-1000",
        egui::Theme::Dark,
        egui::vec2(1000.0, 700.0),
        |_| {},
    );
    render_sized(
        "24-etroit-820",
        egui::Theme::Dark,
        egui::vec2(820.0, 600.0),
        |_| {},
    );
    render_sized(
        "25-tuile-700",
        egui::Theme::Dark,
        egui::vec2(700.0, 850.0),
        |_| {},
    );
    for (name, theme) in [
        ("26-vitre-sombre", egui::Theme::Dark),
        ("27-vitre-clair", egui::Theme::Light),
    ] {
        render(name, theme, |h| {
            bankperfect::theme::set_surface_transparent(true);
            bankperfect::theme::set_translucent(&h.ctx, true);
        });
    }
    bankperfect::theme::set_surface_transparent(false);
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
            bankperfect::BankPerfectApp::with_settings(Some(file), Default::default())
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

/// Robustesse : tailles de fenêtre, défilement, tiroir de saisie, navigation
/// clavier. Ne doit jamais paniquer (à lancer aussi en mode debug).
#[test]
#[ignore]
fn robustness() {
    let file = PathBuf::from(std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE"));
    for size in [
        egui::vec2(1440.0, 900.0),
        egui::vec2(800.0, 500.0),
        egui::vec2(1024.0, 640.0),
    ] {
        let mut harness = Harness::builder().with_size(size).build_eframe(|cc| {
            bankperfect::theme::install(&cc.egui_ctx);
            bankperfect::BankPerfectApp::with_settings(Some(file.clone()), Default::default())
        });
        harness.run_steps(3);
        for i in 0..60 {
            harness.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, if i < 40 { -3.0 } else { 5.0 }),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
        for key in [
            egui::Key::ArrowDown,
            egui::Key::End,
            egui::Key::PageDown,
            egui::Key::Home,
        ] {
            harness.key_press(key);
            harness.run_steps(2);
        }
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::End);
        harness.run_steps(2);
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::PageUp);
        harness.run_steps(2);
        harness.key_press(egui::Key::Insert);
        harness.run_steps(20);
        harness.key_press(egui::Key::Escape);
        harness.run_steps(3);
    }
}
