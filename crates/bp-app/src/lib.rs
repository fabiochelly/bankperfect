//! BankPerfect — interface graphique (egui).

mod app;
mod calculator;
mod category_icons;
mod command_palette;
mod commands;
mod dashboard;
mod datepicker;
mod dialogs;
mod document;
mod editor;
mod files_ui;
mod format;
mod grid;
mod import_ui;
pub mod logo;
mod manage_ui;
mod menu;
mod notes;
mod options_ui;
mod profiler;
mod protection_ui;
mod settings;
mod sidebar;
mod statement_pdf;
pub mod theme;
mod tools;
mod ui_kit;

pub use app::BankPerfectApp;

/// Le fond translucide est-il demandé dans les réglages (à lire avant de
/// créer la fenêtre, qui doit alors avoir une surface transparente) ?
pub fn translucent_window_setting() -> bool {
    settings::Settings::load().translucent()
}
