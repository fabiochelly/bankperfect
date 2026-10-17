//! BankPerfect — interface graphique (egui).

mod app;
mod bank_sync_ui;
mod budget_ui;
mod calculator;
mod category_icons;
mod charts_ui;
mod command_palette;
mod commands;
mod currency_ui;
mod dashboard;
mod datepicker;
mod dialogs;
mod document;
mod editor;
mod ext_ui;
mod extensions;
mod file_picker;
mod files_ui;
mod filters_ui;
mod format;
mod grid;
mod import_ui;
mod islands;
mod license;
mod license_ui;
mod loan_ui;
pub mod logo;
mod manage_ui;
mod menu;
mod notes;
mod options_ui;
mod profiler;
mod protection_ui;
mod quotes_ui;
mod reports_ui;
mod river;
mod rules_ui;
mod scheduler_ui;
mod settings;
mod sidebar;
mod spreadsheet;
mod statement_pdf;
pub mod theme;
mod tools;
mod transfer_ui;
mod ui_kit;

pub use app::BankPerfectApp;

/// Le fond translucide est-il demandé dans les réglages (à lire avant de
/// créer la fenêtre, qui doit alors avoir une surface transparente) ?
pub fn translucent_window_setting() -> bool {
    settings::Settings::load().translucent()
}
