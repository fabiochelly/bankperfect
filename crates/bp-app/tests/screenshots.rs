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
    render_full(name, theme, size, None, setup);
}

fn render_full(
    name: &str,
    theme: egui::Theme,
    size: egui::Vec2,
    extensions: Option<PathBuf>,
    setup: impl FnOnce(&mut Harness<'_, bankperfect::BankPerfectApp>),
) {
    render_with(name, theme, size, extensions, Default::default(), setup);
}

fn render_with(
    name: &str,
    theme: egui::Theme,
    size: egui::Vec2,
    extensions: Option<PathBuf>,
    settings: bankperfect::Settings,
    setup: impl FnOnce(&mut Harness<'_, bankperfect::BankPerfectApp>),
) {
    let file = std::env::var("BP_SCREENSHOT_OVERRIDE")
        .or_else(|_| std::env::var("BP_SCREENSHOT_FILE"))
        .map(PathBuf::from)
        .expect("BP_SCREENSHOT_FILE");
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .wgpu()
        .build_eframe(|cc| {
            bankperfect::theme::install(&cc.egui_ctx);
            // Fond translucide (BP_TRANSLUCENT) : l'image garde la
            // transparence, à poser sur un fond d'écran pour la juger.
            if std::env::var_os("BP_TRANSLUCENT").is_some() {
                bankperfect::theme::set_surface_transparent(true);
                bankperfect::theme::set_translucent(&cc.egui_ctx, true);
            }
            let mut app = bankperfect::BankPerfectApp::with_settings(None, settings);
            if let Some(dir) = extensions {
                app.load_extensions(dir);
            }
            app.open_file(file);
            app
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
    render("28-menu-fichier", egui::Theme::Dark, |h| {
        h.get_by_label("Fichier").click();
    });
    render("29-nouveau-fichier", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::N);
    });
    // Fichier illisible.
    let dir = std::env::temp_dir().join("bp-screenshot-corrompu");
    std::fs::create_dir_all(&dir).unwrap();
    let corrupt = dir.join("Comptes abîmés.bp");
    std::fs::write(&corrupt, b"BPFILE6\0\x01\x00garbage").unwrap();
    unsafe { std::env::set_var("BP_SCREENSHOT_OVERRIDE", &corrupt) };
    render("30-fichier-illisible", egui::Theme::Dark, |_| {});
    unsafe { std::env::remove_var("BP_SCREENSHOT_OVERRIDE") };
    render("31-options", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
    });
    render("32-options-fichiers", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
        h.run_steps(2);
        h.query_all_by_label("Fichiers").last().unwrap().click();
    });
    render("33-options-affichage", egui::Theme::Light, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
        h.run_steps(2);
        h.query_all_by_label("Affichage").last().unwrap().click();
    });
    render("34-a-propos", egui::Theme::Dark, |h| {
        h.get_by_label("Aide").click();
        h.run_steps(3);
        h.get_by_label("À propos de BankPerfect").click();
    });
    let samples = std::env::var("BP_IMPORT_SAMPLES").ok();
    if let Some(dir) = samples {
        for (name, file, theme) in [
            ("35-import-csv", "releve.csv", egui::Theme::Dark),
            ("36-import-ofx", "releve.ofx", egui::Theme::Dark),
            ("37-import-qif", "releve.qif", egui::Theme::Light),
        ] {
            unsafe { std::env::set_var("BP_IMPORT_FILE", format!("{dir}/{file}")) };
            render(name, theme, |h| {
                h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::I);
            });
        }
        unsafe { std::env::remove_var("BP_IMPORT_FILE") };
    }
    render("38-echeancier-vide", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::E);
    });
    render("39-echeance-depuis-operation", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("ajouter à l'échéancier".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
    });
    render("40-previsionnel", egui::Theme::Light, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("ajouter à l'échéancier".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        h.get_by_label_contains("Solde prévisionnel").click();
    });
    render("41-dates-pointage", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::F11);
    });
    render("42-filtres-predefinis", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::F7);
    });
    render("43-menu-filtres", egui::Theme::Dark, |h| {
        h.get_by_label_contains("Filtres").click();
    });
    render("44-regles", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    });
    render("05-menu-survol", egui::Theme::Dark, |h| {
        h.get_by_label("Édition").click();
        h.run_steps(3);
        h.get_by_label("Affichage").hover();
    });
}

/// Outils autonomes : simulateur de crédit, convertisseur de devises.
#[test]
#[ignore]
fn protection_screenshots() {
    render("17b-protection-protegee", egui::Theme::Dark, |h| {
        let open = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
            h.get_by_label("Fichier").click();
            h.run_steps(3);
            h.get_by_label("Protection du fichier…").click();
            h.run_steps(2);
        };
        open(h);
        for i in 0..2 {
            h.query_all_by_role(egui::accesskit::Role::PasswordInput)
                .chain(h.query_all_by_role(egui::accesskit::Role::TextInput))
                .nth(i)
                .unwrap()
                .click();
            h.run_steps(2);
            h.event(egui::Event::Text("motdepasse".into()));
            h.run_steps(2);
        }
        h.run_steps(2);
        h.get_by_label("Protéger").click();
        h.run_steps(2);
        h.get_by_label("J'ai noté ma clé").click();
        h.run_steps(2);
        open(h);
    });
}

#[test]
#[ignore]
fn theme_screenshots() {
    for theme in bankperfect::theme::THEMES {
        let mode = if theme.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        render(&format!("theme-{}", theme.id), mode, |h| {
            bankperfect::theme::select(&h.ctx, theme.id, theme.id);
        });
    }
    for (name, mode) in [
        ("theme-galerie-sombre", egui::Theme::Dark),
        ("theme-galerie-clair", egui::Theme::Light),
    ] {
        render_sized(name, mode, egui::vec2(1440.0, 1000.0), |h| {
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
            h.run_steps(2);
            h.query_all_by_label("Affichage").last().unwrap().click();
        });
    }
    bankperfect::theme::select(
        &egui::Context::default(),
        bankperfect::theme::DEFAULT_DARK,
        bankperfect::theme::DEFAULT_LIGHT,
    );
}

#[test]
#[ignore]
fn tool_screenshots() {
    render("61-credit", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::L);
    });
    render("62-credit-clair", egui::Theme::Light, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::L);
    });
    render("65-cotations", egui::Theme::Dark, |h| {
        h.get_all_by_label_contains("Titres")
            .next()
            .unwrap()
            .click();
        h.run_steps(3);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("cotations".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
    });
    render("63-devises", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    });
    render("64-devises-ajout", egui::Theme::Light, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
        h.run_steps(3);
        h.get_by_label_contains("Ajouter une devise").click();
        h.run_steps(3);
        h.event(egui::Event::Text("dol".into()));
    });
}

/// Transferts et ventilation.
#[test]
#[ignore]
fn transfer_screenshots() {
    render("66-transfert", egui::Theme::Dark, |h| {
        h.key_press_modifiers(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::T,
        );
    });
    render("67-ventilation", egui::Theme::Light, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press(egui::Key::F6);
    });
    render("68-supprimer-transfert", egui::Theme::Dark, |h| {
        h.get_by_label("Transfert vers le compte Livret").click();
        h.run_steps(2);
        h.key_press(egui::Key::Delete);
    });
    render("70-menu-exporter", egui::Theme::Dark, |h| {
        h.get_by_label("Fichier").click();
        h.run_steps(3);
        h.get_by_label("Exporter").hover();
    });
    let bottom_entry = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
        h.get_by_label("Affichage").click();
        h.run_steps(3);
        h.get_by_label("Saisie en bas de la fenêtre").click();
        h.run_steps(3);
    };
    render("71-saisie-bas", egui::Theme::Dark, |h| bottom_entry(h));
    render_sized(
        "72-saisie-bas-large",
        egui::Theme::Light,
        egui::vec2(1920.0, 1000.0),
        |h| {
            bottom_entry(h);
            h.key_press(egui::Key::ArrowDown);
            h.key_press(egui::Key::ArrowDown);
            h.run_steps(2);
            h.key_press(egui::Key::Enter);
        },
    );
    render("73-f8", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::F8);
    });
    render("74-maj-entree", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    });
    render("75-maj-entree-enregistre", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
        h.run_steps(4);
        h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    });
    render("76-synchro-config", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("synchroniser avec la banque".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
    });
    render("69-convertir", egui::Theme::Dark, |h| {
        h.key_press(egui::Key::ArrowDown);
        h.key_press(egui::Key::ArrowDown);
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("convertir en transfert".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
    });
}

/// Encadrés de solde à plusieurs largeurs de fenêtre.
#[test]
#[ignore]
fn dashboard_widths() {
    for width in [1280.0, 1100.0, 1000.0, 820.0] {
        render_sized(
            &format!("60-largeur-{width}"),
            egui::Theme::Dark,
            egui::vec2(width, 700.0),
            |_| {},
        );
    }
    // Soldes séparés en trois cartes (palette de commandes).
    for (name, theme) in [
        ("61-cartes", egui::Theme::Dark),
        ("61-cartes-clair", egui::Theme::Light),
    ] {
        render_sized(name, theme, egui::vec2(1280.0, 700.0), |h| {
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
            h.run_steps(2);
            h.event(egui::Event::Text("trois cartes".into()));
            h.run_steps(2);
            h.key_press(egui::Key::Enter);
        });
    }
    // Filtre actif (période choisie sur la courbe) : mention dans la barre de
    // menus, en toutes lettres ou en icône selon la place.
    for width in [1280.0, 900.0] {
        render_sized(
            &format!("62-filtre-{width}"),
            egui::Theme::Dark,
            egui::vec2(width, 700.0),
            |h| {
                let press = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                };
                let (from, to) = (width - 110.0, width - 40.0);
                h.event(egui::Event::PointerMoved(egui::pos2(from, 130.0)));
                h.event(press(egui::pos2(from, 130.0), true));
                h.run_steps(2);
                for x in (from as i32..=to as i32).step_by(10) {
                    h.event(egui::Event::PointerMoved(egui::pos2(x as f32, 130.0)));
                    h.run_steps(1);
                }
                h.event(press(egui::pos2(to, 130.0), false));
                h.run_steps(3);
            },
        );
    }
    // Ligne des filtres de colonnes ouverte par la loupe.
    for (name, theme) in [
        ("63-filtres-colonnes", egui::Theme::Dark),
        ("63-filtres-colonnes-clair", egui::Theme::Light),
    ] {
        render_sized(name, theme, egui::vec2(1280.0, 700.0), |h| {
            h.get_by_label("Filtres de colonnes").click();
        });
    }
    render_sized(
        "60-largeur-1280-clair",
        egui::Theme::Light,
        egui::vec2(1280.0, 700.0),
        |_| {},
    );
}

/// Rapports, graphiques et budgets (lancés seuls pendant la mise au point).
#[test]
#[ignore]
fn report_screenshots() {
    render("45-totaux-categories", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
    });
    render("46-total-comptes", egui::Theme::Light, |h| {
        h.get_by_label("Rapports").click();
        h.run_steps(3);
        h.get_by_label_contains("Total des comptes").click();
    });
    render("47-totaux-tiers", egui::Theme::Light, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
        h.run_steps(3);
        h.get_by_label("Par tiers").click();
    });
    render("48-releve-multicompte", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
        h.run_steps(3);
        h.get_by_label("Relevé multicompte").click();
    });
    let charts = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
        h.key_press_modifiers(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::G,
        );
        h.run_steps(3);
    };
    render("49-graphique-anneau", egui::Theme::Dark, |h| charts(h));
    render("50-graphique-barres", egui::Theme::Light, |h| {
        charts(h);
        h.get_by_label("Barres").click();
    });
    render("51-graphique-soldes", egui::Theme::Dark, |h| {
        charts(h);
        h.get_by_label_contains("Soldes des comptes").click();
    });
    render("52-graphique-evolution", egui::Theme::Light, |h| {
        charts(h);
        h.get_by_label_contains("Évolution du solde").click();
    });
    render("53-graphique-jours", egui::Theme::Dark, |h| {
        charts(h);
        h.get_by_label_contains("Par jour du mois").click();
    });
    render("54-budget-mensuel", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h.run_steps(3);
        h.get_by_label("Tout").click();
    });
    render("56-budget-saisie", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h.run_steps(3);
        h.get_by_label("Tout").click();
        h.run_steps(3);
        h.get_all_by_label("—").nth(1).unwrap().click();
        h.run_steps(3);
        h.event(egui::Event::Text("-120".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        h.get_all_by_label("—").nth(3).unwrap().click();
        h.run_steps(3);
        h.event(egui::Event::Text("+15".into()));
        h.run_steps(2);
    });
    render("55-budget-annuel", egui::Theme::Light, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h.run_steps(3);
        h.get_by_label("Budgets annuels").click();
    });
    render("57-budget-annuel-ligne", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h.run_steps(3);
        h.get_by_label("Budgets annuels").click();
        h.run_steps(3);
        h.get_by_label_contains("  Alimentation").click();
        h.run_steps(3);
        h.get_all_by_label("·").next().unwrap().click();
        h.run_steps(3);
        h.event(egui::Event::Text("-300".into()));
        h.run_steps(2);
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

fn render_extension(
    name: &str,
    theme: egui::Theme,
    setup: impl FnOnce(&mut Harness<'_, bankperfect::BankPerfectApp>),
) {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    render_full(
        name,
        theme,
        egui::vec2(1440.0, 900.0),
        Some(examples),
        setup,
    );
}

#[test]
#[ignore]
fn extension_screenshots() {
    let open_in = |h: &mut Harness<'_, bankperfect::BankPerfectApp>, menu: &str, item: &str| {
        h.get_by_label(menu).click();
        h.run_steps(3);
        h.get_by_label_contains(item).click();
        h.run_steps(3);
    };
    let open_menu = |h: &mut Harness<'_, bankperfect::BankPerfectApp>, item: &str| {
        open_in(h, "Extensions", item);
    };
    render_extension("80-ext-menu", egui::Theme::Dark, |h| {
        h.get_by_label("Extensions").click();
    });
    render_extension("81-ext-doublons", egui::Theme::Dark, |h| {
        open_in(h, "Opérations", "Rechercher les doublons");
    });
    render_extension("82-ext-synthese", egui::Theme::Dark, |h| {
        open_in(h, "Rapports", "Synthèse du mois");
    });
    render_extension("83-ext-arrondis", egui::Theme::Dark, |h| {
        open_menu(h, "Épargne par arrondis");
    });
    render_extension("84-ext-export", egui::Theme::Light, |h| {
        h.get_by_label("Fichier").click();
        h.run_steps(3);
        h.get_by_label("Exporter").hover();
        h.run_steps(3);
        h.get_by_label_contains("Exporter des opérations en JSON")
            .click();
        h.run_steps(3);
    });
    render_extension("85-ext-gestion", egui::Theme::Dark, |h| {
        open_menu(h, "Gérer les extensions");
        // Dernière de la liste : « Synthèse du mois » (placée dans des menus).
        let pos = egui::pos2(346.0, 659.0);
        h.event(egui::Event::PointerMoved(pos));
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.step();
        }
    });
    render_extension("86-ext-menu-operations", egui::Theme::Dark, |h| {
        h.get_by_label("Opérations").click();
    });
    render_extension("87-ext-clic-droit-grille", egui::Theme::Dark, |h| {
        right_click(h, egui::pos2(760.0, 330.0));
    });
    render_extension("88-ext-clic-droit-compte", egui::Theme::Dark, |h| {
        right_click(h, egui::pos2(120.0, 245.0));
    });
    render_extension("89-ext-total-selection", egui::Theme::Dark, |h| {
        // Plusieurs lignes : clic, Maj+clic, puis la commande du clic droit.
        h.event(egui::Event::PointerMoved(egui::pos2(760.0, 330.0)));
        h.run_steps(2);
        for (pos, modifiers) in [
            (egui::pos2(760.0, 330.0), egui::Modifiers::NONE),
            (egui::pos2(760.0, 640.0), egui::Modifiers::SHIFT),
        ] {
            for pressed in [true, false] {
                h.event(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                });
                h.step();
            }
        }
        right_click(h, egui::pos2(760.0, 640.0));
        h.run_steps(3);
        h.get_by_label_contains("Total de la sélection").click();
    });
    // BP_IMPORT_FILE : relevé lu par le format d'import d'une extension.
    if std::env::var_os("BP_IMPORT_FILE").is_some() {
        render_extension("90-ext-import", egui::Theme::Dark, |h| {
            h.get_by_label("Fichier").click();
            h.run_steps(3);
            h.get_by_label_contains("Importer des opérations").click();
        });
    }
}

/// Clic droit à une position (menus contextuels).
fn right_click(h: &mut Harness<'_, bankperfect::BankPerfectApp>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
}

/// Licence : activation de bout en bout contre un serveur de test
/// (BP_LICENSE_URL et BP_LICENSE_KEY), puis licence affichée.
#[test]
#[ignore]
fn license_screens() {
    let key = std::env::var("BP_LICENSE_KEY").expect("BP_LICENSE_KEY");
    let open_dialog = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("Licence".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
    };
    render_sized(
        "65-licence",
        egui::Theme::Dark,
        egui::vec2(1280.0, 700.0),
        |h| {
            open_dialog(h);
            h.event(egui::Event::Text(key.clone()));
        },
    );
    render_sized(
        "66-licence-activee",
        egui::Theme::Dark,
        egui::vec2(1280.0, 700.0),
        |h| {
            open_dialog(h);
            h.event(egui::Event::Text(key.clone()));
            h.run_steps(2);
            h.get_by_label("Activer").click();
            for _ in 0..60 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                h.run_steps(1);
            }
        },
    );
}

/// Relevé OFX fictif pour l'aperçu d'import.
const SAMPLE_OFX: &str = "OFXHEADER:100\nDATA:OFXSGML\nVERSION:102\n\n<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><CURDEF>EUR\n<BANKACCTFROM><BANKID>30001<BRANCHID>00001<ACCTID>99999999999</BANKACCTFROM>\n<BANKTRANLIST>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261017<TRNAMT>-4.20<FITID>X1<NAME>CB BOULANGERIE DU MARCHE</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261017<TRNAMT>-63.18<FITID>X2<NAME>CB LIDL 4521<SIC>5411</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261016<TRNAMT>-54.90<FITID>X3<NAME>CB TOTAL ACCESS VILLEJUIF</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261016<TRNAMT>-15.99<FITID>X4<NAME>PRLV SEPA ORANGE SA-ORANGE<MEMO>Votre abonnement mobile</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261015<TRNAMT>-12.90<FITID>X5<NAME>CB PHARMACIE DU CENTRE</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261015<TRNAMT>-30.00<FITID>X6<NAME>PRLV SEPA LC FITNESS KREMLIN</STMTTRN>\n\
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20261014<TRNAMT>24.46<FITID>X7<NAME>VIR SEPA CPAM DES YVELINES</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261014<TRNAMT>-60.00<FITID>X8<NAME>VIR INST MARTIN DUPONT<MEMO>Kine</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261013<TRNAMT>-23.00<FITID>X9<NAME>VIR Abonnement Claude Pro</STMTTRN>\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261013<TRNAMT>-250.00<FITID>X10<NAME>VIR DEPENSES APPART</STMTTRN>\n\
</BANKTRANLIST><LEDGERBAL><BALAMT>1000.00<DTASOF>20261017</LEDGERBAL></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>\n";

/// Catégorisation automatique : fenêtre « Catégoriser », baguette magique de
/// la saisie, colonne des catégories prévues à l'import.
#[test]
#[ignore]
fn categorize_screenshots() {
    render("70-categoriser", egui::Theme::Dark, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::J);
    });
    render("71-categoriser-clair", egui::Theme::Light, |h| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::J);
    });
    render("72-saisie-devinee", egui::Theme::Dark, |h| {
        h.get_by_label_contains("Nouvelle opération").click();
        h.run_steps(10);
        click_at(h, egui::pos2(1210.0, 433.0));
        h.event(egui::Event::Text(
            "PRLV SEPA EDF clients particuliers".into(),
        ));
        h.run_steps(4);
    });
    render("73-saisie-suggestions", egui::Theme::Dark, |h| {
        h.get_by_label_contains("Nouvelle opération").click();
        h.run_steps(10);
        click_at(h, egui::pos2(1210.0, 433.0));
        h.event(egui::Event::Text("VIR INST MARTIN".into()));
        click_at(h, egui::pos2(1210.0, 487.0));
        h.event(egui::Event::Text("Kine".into()));
        h.run_steps(4);
    });
    render("74-baguette", egui::Theme::Dark, |h| {
        h.get_by_label_contains("Nouvelle opération").click();
        h.run_steps(10);
        click_at(h, egui::pos2(1210.0, 433.0));
        h.event(egui::Event::Text("VIR INST MARTIN".into()));
        click_at(h, egui::pos2(1210.0, 487.0));
        h.event(egui::Event::Text("Kine".into()));
        h.run_steps(2);
        h.key_press(egui::Key::F9);
        h.run_steps(4);
    });
    let dir = tempfile_dir();
    let ofx = dir.join("releve.ofx");
    std::fs::write(&ofx, SAMPLE_OFX).unwrap();
    render("75-import-categories", egui::Theme::Dark, move |h| {
        h.input_mut()
            .dropped_files
            .push(std::sync::Arc::new(DroppedPath(ofx)));
        h.run_steps(4);
    });
}

/// Clic gauche à une position (points logiques).
fn click_at(h: &mut Harness<'_, bankperfect::BankPerfectApp>, pos: egui::Pos2) {
    let press = |pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.event(egui::Event::PointerMoved(pos));
    h.event(press(true));
    h.run_steps(1);
    h.event(press(false));
    h.run_steps(2);
}

/// Fichier déposé sur la fenêtre.
#[derive(Debug)]
struct DroppedPath(PathBuf);

impl egui::DroppedFile for DroppedPath {
    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn tempfile_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots/fixtures");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Pointage intelligent : solde simulé d'après le fichier (lignes non
/// pointées passées depuis plus de trois jours, sauf les chèques récents).
#[test]
#[ignore]
fn smart_tack_screenshots() {
    let file = std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE");
    let document = bp_core::file::open(file.as_ref(), None).unwrap().document;
    let account = &document.accounts[document.account_order[0]];
    let today = bp_core::model::Date::today();
    let cleared: bp_core::model::Amount = account
        .operations
        .iter()
        .filter(|o| o.tack == bp_core::model::Tack::None && o.date.0 <= today.0 - 3)
        .map(|o| o.amount)
        .sum();
    let balance = bp_core::smart_tack::pointed_balance(account, Some(today)) + cleared;
    let open = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("pointage intelligent".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(4);
    };
    render("80-pointage-vide", egui::Theme::Dark, |h| open(h));
    let text = balance.format(2, ',');
    render("81-pointage", egui::Theme::Dark, move |h| {
        open(h);
        click_at(h, egui::pos2(SMART_TACK_FIELD.0, SMART_TACK_FIELD.1));
        h.event(egui::Event::Text(text));
        h.key_press(egui::Key::Enter);
        h.run_steps(30);
    });
}

/// Champ du solde dans la fenêtre de pointage intelligent (points logiques).
const SMART_TACK_FIELD: (f32, f32) = (453.0, 242.0);

/// Relevé OFX fictif (banque américaine) pour l'aperçu d'import en anglais :
/// opérations des derniers jours, solde annoncé `balance`.
fn sample_ofx_en(today: bp_core::model::Date, balance: bp_core::model::Amount) -> String {
    let lines: [(i32, &str, &str, &str); 10] = [
        (2, "POS", "-5.85", "STARBUCKS STORE 10432 CHICAGO IL"),
        (2, "POS", "-84.12", "TRADER JOE'S #706<SIC>5411"),
        (3, "POS", "-48.90", "SHELL OIL 57444 CHICAGO"),
        (
            3,
            "PAYMENT",
            "-70.00",
            "VERIZON WIRELESS PAYMENT<MEMO>Autopay",
        ),
        (4, "POS", "-12.99", "CVS/PHARMACY #08821"),
        (4, "PAYMENT", "-24.99", "PLANET FITNESS CLUB FEES"),
        (5, "CREDIT", "28.00", "BCBS CLAIM REIMBURSEMENT"),
        (5, "XFER", "-60.00", "ZELLE TO SAM CARTER<MEMO>Dinner"),
        (6, "POS", "-23.00", "CLAUDE.AI SUBSCRIPTION"),
        (6, "XFER", "-250.00", "ONLINE TRANSFER TO SAV ...6603"),
    ];
    let day = |back: i32| {
        bp_core::model::Date(today.0 - back)
            .to_naive()
            .format("%Y%m%d")
            .to_string()
    };
    let mut ofx = String::from(
        "OFXHEADER:100\nDATA:OFXSGML\nVERSION:102\n\n<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><CURDEF>USD\n<BANKACCTFROM><BANKID>123456789<ACCTID>440123456601</BANKACCTFROM>\n<BANKTRANLIST>\n",
    );
    let mut total = bp_core::model::Amount::ZERO;
    for (i, (back, kind, amount, name)) in lines.iter().enumerate() {
        total = total
            + bp_core::model::Amount::from_cents(
                (amount.parse::<f64>().unwrap() * 100.0).round() as i64
            );
        ofx.push_str(&format!(
            "<STMTTRN><TRNTYPE>{kind}<DTPOSTED>{}<TRNAMT>{amount}<FITID>E{i}<NAME>{name}</STMTTRN>\n",
            day(*back)
        ));
    }
    let balance = balance + total;
    ofx.push_str(&format!(
        "</BANKTRANLIST><LEDGERBAL><BALAMT>{}<DTASOF>{}</LEDGERBAL></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>\n",
        balance.format(2, '.'),
        day(2)
    ));
    ofx
}

/// Une capture en anglais (réglage `language = "en"`), exemples d'extensions
/// chargés. `BP_SCREENSHOT_ONLY` (mots séparés par des virgules) limite les
/// captures rendues à celles dont le nom contient l'un des mots.
fn render_en(
    name: &str,
    theme: egui::Theme,
    size: egui::Vec2,
    setup: impl FnOnce(&mut Harness<'_, bankperfect::BankPerfectApp>),
) {
    if let Ok(only) = std::env::var("BP_SCREENSHOT_ONLY")
        && !only.split(',').any(|word| name.contains(word.trim()))
    {
        return;
    }
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    // Réglages par défaut (jamais enregistrés), langue anglaise.
    let mut settings = bankperfect::Settings::default();
    settings.language = Some("en".into());
    render_with(name, theme, size, Some(examples), settings, setup);
}

/// Taille habituelle des captures.
const FULL: egui::Vec2 = egui::vec2(1440.0, 900.0);

/// Ouvre une commande par la palette (texte cherché, puis Entrée).
fn run_command(h: &mut Harness<'_, bankperfect::BankPerfectApp>, text: &str) {
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    h.run_steps(2);
    h.event(egui::Event::Text(text.into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
}

/// Écrans principaux en anglais, sur la démo anglaise
/// (`gen_demo --lang en`) : `BP_SCREENSHOT_FILE=demo-en.bp`.
#[test]
#[ignore]
fn english_screenshots() {
    use egui::{Key, Modifiers, Theme};
    let dark = Theme::Dark;
    let light = Theme::Light;
    render_en("en-01-dashboard-light", light, FULL, |_| {});
    render_en("en-02-dashboard-dark", dark, FULL, |_| {});
    render_en("en-03-entry", dark, FULL, |h| {
        h.get_by_label_contains("New transaction").click()
    });
    render_en("en-03b-calendar", dark, FULL, |h| {
        h.get_by_label_contains("New transaction").click();
        h.run_steps(20);
        h.get_by_label(CALENDAR_DOTS).click();
    });
    for menu in [
        "File",
        "Edit",
        "View",
        "Accounts",
        "Transactions",
        "Reports",
        "Tools",
        "Extensions",
        "Help",
    ] {
        render_en(
            &format!("en-04-menu-{}", menu.to_lowercase()),
            dark,
            FULL,
            |h| h.get_by_label(menu).click(),
        );
    }
    render_en("en-04-menu-export", dark, FULL, |h| {
        h.get_by_label("File").click();
        h.run_steps(3);
        h.get_by_label("Export").hover();
    });
    render_en("en-05-palette", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    });
    render_en("en-05-palette-search", light, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::K);
        h.run_steps(2);
        h.event(egui::Event::Text("categ".into()));
    });
    for (tab, theme) in [
        ("Entry and calculations", dark),
        ("Files", dark),
        ("View", light),
    ] {
        let name = format!(
            "en-06-options-{}",
            tab.split(' ').next().unwrap().to_lowercase()
        );
        render_en(&name, theme, egui::vec2(1440.0, 1000.0), |h| {
            h.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
            h.run_steps(2);
            h.query_all_by_label(tab).last().unwrap().click();
        });
    }
    render_en("en-07-accounts", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::G);
    });
    render_en("en-07-categories", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::G);
        h.run_steps(2);
        h.get_by_label("Categories").click();
        h.run_steps(2);
        h.query_all_by_label_contains("Food")
            .last()
            .unwrap()
            .click();
    });
    render_en("en-07-payees", light, FULL, |h| {
        h.key_press(Key::F5);
        h.run_steps(2);
        h.query_all_by_label("ComEd").last().unwrap().click();
    });
    render_en("en-08-scheduler", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::E);
    });
    render_en("en-08-schedule-from-transaction", dark, FULL, |h| {
        h.key_press(Key::ArrowDown);
        h.key_press(Key::ArrowDown);
        h.run_steps(2);
        run_command(h, "add to scheduler");
    });
    render_en("en-08-forecast", light, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::E);
        h.run_steps(3);
        h.get_by_label_contains("Forecast balance").click();
    });
    render_en("en-09-budget-monthly", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
    });
    render_en("en-09-budget-all", light, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
        h.run_steps(3);
        h.get_by_label("All").click();
    });
    render_en("en-09-budget-annual", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
        h.run_steps(3);
        h.get_by_label("Annual budgets").click();
    });
    render_en("en-10-report-categories", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
    });
    render_en("en-10-report-payees", light, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
        h.run_steps(3);
        h.get_by_label("By payee").click();
    });
    render_en("en-10-report-statement", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
        h.run_steps(3);
        h.get_by_label("Multi-account statement").click();
    });
    render_en("en-10-report-accounts", light, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
        h.run_steps(3);
        h.get_by_label_contains("Account totals").click();
    });
    let charts = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::G);
        h.run_steps(3);
    };
    render_en("en-11-chart-donut", dark, FULL, |h| charts(h));
    render_en("en-11-chart-bars", light, FULL, |h| {
        charts(h);
        h.get_by_label("Bars").click();
    });
    for (name, label, theme) in [
        ("balances", "Account balances", dark),
        ("evolution", "Balance over time", light),
        ("days", "By day of the month", dark),
    ] {
        render_en(&format!("en-11-chart-{name}"), theme, FULL, |h| {
            charts(h);
            h.get_by_label_contains(label).click();
        });
    }
    render_en("en-12-loan", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::L);
    });
    render_en("en-12-loan-light", light, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::L);
    });
    render_en("en-13-currencies", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::D);
    });
    // Solde annoncé par la banque : le solde pointé du compte, plus le relevé.
    let file = std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE");
    let document = bp_core::file::open(file.as_ref(), None).unwrap().document;
    let account = &document.accounts[document.account_order[0]];
    let today = bp_core::model::Date::today();
    let in_bank = bp_core::smart_tack::pointed_balance(account, Some(today));
    let dir = tempfile_dir();
    let ofx = dir.join("statement.ofx");
    std::fs::write(&ofx, sample_ofx_en(today, in_bank)).unwrap();
    render_en("en-14-import", dark, FULL, |h| {
        h.input_mut()
            .dropped_files
            .push(std::sync::Arc::new(DroppedPath(ofx.clone())));
        h.run_steps(4);
    });
    render_en("en-15-categorize", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::J);
    });
    render_en("en-15-entry-guess", dark, FULL, |h| {
        h.get_by_label_contains("New transaction").click();
        h.run_steps(10);
        click_at(h, egui::pos2(1210.0, 433.0));
        h.event(egui::Event::Text("COMED ELECTRIC BILL PAYMENT".into()));
        h.run_steps(4);
    });
    render_en("en-16-smart-clearing", dark, FULL, |h| {
        run_command(h, "smart clearing");
    });
    // Solde de la banque : lignes non pointées depuis plus de trois jours.
    let cleared: bp_core::model::Amount = account
        .operations
        .iter()
        .filter(|o| o.tack == bp_core::model::Tack::None && o.date.0 <= today.0 - 3)
        .map(|o| o.amount)
        .sum();
    let bank = (in_bank + cleared).format(2, '.');
    render_en("en-16-smart-clearing-result", dark, FULL, move |h| {
        run_command(h, "smart clearing");
        click_at(h, egui::pos2(SMART_TACK_FIELD.0, SMART_TACK_FIELD.1));
        h.event(egui::Event::Text(bank));
        h.key_press(Key::Enter);
        h.run_steps(30);
    });
    render_en("en-17-split", light, FULL, |h| {
        h.key_press(Key::ArrowDown);
        h.key_press(Key::ArrowDown);
        h.run_steps(2);
        h.key_press(Key::F6);
    });
    render_en("en-17-transfer", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::T);
    });
    render_en("en-18-protection", dark, FULL, |h| {
        h.get_by_label("File").click();
        h.run_steps(3);
        h.get_by_label("File protection…").click();
        h.run_steps(2);
        h.get_by_label("Protect").click();
    });
    // Fenêtre de licence seulement : aucune activation (pas de requête).
    render_en("en-19-license", dark, egui::vec2(1280.0, 700.0), |h| {
        h.get_by_label("Help").click();
        h.run_steps(3);
        h.get_by_label("License…").click();
    });
    render_en("en-19-about", dark, FULL, |h| {
        h.get_by_label("Help").click();
        h.run_steps(3);
        h.get_by_label("About BankPerfect").click();
    });
    render_en("en-20-extensions", dark, FULL, |h| {
        h.get_by_label("Extensions").click();
        h.run_steps(3);
        h.get_by_label_contains("Manage extensions").click();
    });
    render_en("en-20-ext-duplicates", dark, FULL, |h| {
        h.get_by_label("Transactions").click();
        h.run_steps(3);
        h.get_by_label_contains("Find duplicates").click();
    });
    render_en("en-20-ext-summary", dark, FULL, |h| {
        h.get_by_label("Reports").click();
        h.run_steps(3);
        h.get_by_label_contains("Monthly summary").click();
    });
    render_en("en-20-ext-roundup", dark, FULL, |h| {
        h.get_by_label("Extensions").click();
        h.run_steps(3);
        h.get_by_label_contains("Round-up savings").click();
    });
    render_en("en-21-rules", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::F);
    });
    render_en("en-21-filters", dark, FULL, |h| {
        h.key_press(Key::F7);
    });
    render_en(
        "en-21-column-filters",
        light,
        egui::vec2(1280.0, 700.0),
        |h| {
            h.get_by_label("Column filters").click();
        },
    );
    render_en("en-22-new-file", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    });
    render_en("en-22-backups", dark, FULL, |h| {
        h.get_by_label("File").click();
        h.run_steps(3);
        h.get_by_label("Backups…").click();
    });
    render_en("en-22-replace", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::R);
    });
    render_en("en-22-notes", dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::N);
    });
    render_en("en-22-fields", dark, FULL, |h| {
        h.key_press(Key::ArrowDown);
        h.run_steps(2);
        h.key_press(Key::F3);
    });
    render_en("en-22-print", dark, FULL, |h| {
        run_command(h, "print statement");
    });
    for width in [1280.0, 1100.0] {
        render_en(
            &format!("en-23-width-{width}"),
            dark,
            egui::vec2(width, 700.0),
            |_| {},
        );
    }
}

/// Relevé CSV « Date;Description;Debit;Credit » pour l'import par extension.
const SAMPLE_CSV_EN: &str = "Date;Description;Debit;Credit
05/01/2026;WHOLE FOODS MARKET #10233;42.15;
06/01/2026;PAYROLL EXAMPLE CORP;;5233.69
08/01/2026;COMED ELECTRIC BILL;99.00;
09/01/2026;CHECK 1052;120.00;
";

/// Captures anglaises de la documentation et du site, sur la démo anglaise
/// (`gen_demo --lang en`), dans target/screenshots/captures-en/ : mêmes
/// scénarios que les captures françaises de docs/captures et du site.
#[test]
#[ignore]
fn english_captures() {
    use egui::{Key, Modifiers, Theme};
    let dark = Theme::Dark;
    let light = Theme::Light;
    let out =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots/captures-en");
    std::fs::create_dir_all(&out).unwrap();
    let name = |n: &str| format!("captures-en/{n}");
    let entry = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
        h.get_by_label_contains("New transaction").click()
    };
    let charts = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::G);
        h.run_steps(3);
    };
    render_en(&name("tableau-de-bord"), dark, FULL, |_| {});
    render_en(&name("tableau-de-bord-clair"), light, FULL, |_| {});
    render_en(&name("saisie"), dark, FULL, entry);
    render_en(&name("graphiques"), dark, FULL, charts);
    render_en(&name("graphique-solde"), light, FULL, |h| {
        charts(h);
        h.get_by_label_contains("Balance over time").click();
    });
    render_en(&name("budget"), dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
        h.run_steps(3);
        h.get_by_label("All").click();
    });
    render_en(&name("simulateur-credit"), dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::L);
    });
    render_en(&name("rapport-categories"), dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
    });
    render_en(&name("releve-multicompte"), dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
        h.run_steps(3);
        h.get_by_label("Multi-account statement").click();
    });
    render_en(&name("transfert"), dark, FULL, |h| {
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::T);
    });
    render_en(&name("ventilation"), light, FULL, |h| {
        h.key_press(Key::ArrowDown);
        h.key_press(Key::ArrowDown);
        h.run_steps(2);
        h.key_press(Key::F6);
    });
    render_en(&name("themes"), dark, egui::vec2(1440.0, 1000.0), |h| {
        h.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
        h.run_steps(2);
        h.query_all_by_label("View").last().unwrap().click();
        h.run_steps(3);
        // Défile jusqu'aux thèmes (la langue est en tête de l'onglet).
        h.event(egui::Event::PointerMoved(egui::pos2(660.0, 600.0)));
        for _ in 0..4 {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -40.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            });
            h.step();
        }
    });
    // Catégorisation : relevé OFX américain, saisie devinée.
    let file = std::env::var("BP_SCREENSHOT_FILE").expect("BP_SCREENSHOT_FILE");
    let document = bp_core::file::open(file.as_ref(), None).unwrap().document;
    let account = &document.accounts[document.account_order[0]];
    let today = bp_core::model::Date::today();
    let in_bank = bp_core::smart_tack::pointed_balance(account, Some(today));
    let dir = tempfile_dir();
    let ofx = dir.join("statement.ofx");
    std::fs::write(&ofx, sample_ofx_en(today, in_bank)).unwrap();
    render_en(&name("categorisation-import"), dark, FULL, |h| {
        h.input_mut()
            .dropped_files
            .push(std::sync::Arc::new(DroppedPath(ofx.clone())));
        h.run_steps(4);
    });
    render_en(&name("categorisation-saisie"), dark, FULL, |h| {
        entry(h);
        h.run_steps(10);
        click_at(h, egui::pos2(1210.0, 433.0));
        h.event(egui::Event::Text("COMED ELECTRIC BILL PAYMENT".into()));
        h.run_steps(4);
    });
    // Extensions.
    render_en(&name("extensions-clic-droit"), dark, FULL, |h| {
        right_click(h, egui::pos2(760.0, 330.0));
    });
    render_en(&name("extensions-doublons"), dark, FULL, |h| {
        h.get_by_label("Transactions").click();
        h.run_steps(3);
        h.get_by_label_contains("Find duplicates").click();
    });
    render_en(&name("extensions-formulaire"), light, FULL, |h| {
        h.get_by_label("File").click();
        h.run_steps(3);
        h.get_by_label("Export").hover();
        h.run_steps(3);
        h.get_by_label_contains("Export transactions as JSON")
            .click();
        h.run_steps(3);
    });
    render_en(&name("extensions-gestion"), dark, FULL, |h| {
        h.get_by_label("Extensions").click();
        h.run_steps(3);
        h.get_by_label_contains("Manage extensions").click();
        h.run_steps(3);
        // Dernière de la liste : « Monthly summary » (placée dans des menus).
        click_at(h, egui::pos2(346.0, 659.0));
    });
    render_en(&name("extensions-synthese"), dark, FULL, |h| {
        h.get_by_label("Reports").click();
        h.run_steps(3);
        h.get_by_label_contains("Monthly summary").click();
    });
    render_en(&name("ext-arrondis"), dark, FULL, |h| {
        h.get_by_label("Extensions").click();
        h.run_steps(3);
        h.get_by_label_contains("Round-up savings").click();
    });
    render_en(&name("ext-selection"), dark, FULL, |h| {
        click_at(h, egui::pos2(760.0, 790.0));
        right_click(h, egui::pos2(760.0, 790.0));
        h.run_steps(3);
        h.get_by_label_contains("Selection total").click();
    });
    let csv = dir.join("statement.csv");
    std::fs::write(&csv, SAMPLE_CSV_EN).unwrap();
    render_en(&name("extensions-import"), dark, FULL, |h| {
        h.input_mut()
            .dropped_files
            .push(std::sync::Arc::new(DroppedPath(csv.clone())));
        h.run_steps(4);
    });
    // Fichier protégé : sur une copie de la démo (la protection l'enregistre).
    let copy = dir.join("protected-demo.bp");
    std::fs::copy(&file, &copy).unwrap();
    unsafe { std::env::set_var("BP_SCREENSHOT_OVERRIDE", &copy) };
    render_en(&name("protection"), dark, FULL, |h| {
        let open = |h: &mut Harness<'_, bankperfect::BankPerfectApp>| {
            h.get_by_label("File").click();
            h.run_steps(3);
            h.get_by_label("File protection…").click();
            h.run_steps(2);
        };
        open(h);
        for i in 0..2 {
            h.query_all_by_role(egui::accesskit::Role::PasswordInput)
                .chain(h.query_all_by_role(egui::accesskit::Role::TextInput))
                .nth(i)
                .unwrap()
                .click();
            h.run_steps(2);
            h.event(egui::Event::Text("password1".into()));
            h.run_steps(2);
        }
        h.run_steps(2);
        h.get_by_label("Protect").click();
        h.run_steps(2);
        h.get_by_label("I have written it down").click();
        h.run_steps(2);
        open(h);
    });
    unsafe { std::env::remove_var("BP_SCREENSHOT_OVERRIDE") };
}
