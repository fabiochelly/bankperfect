//! Catalogue anglais, découpé par partie du code (un fichier chacune).

mod app_data;
mod app_main;
mod app_shell;
mod app_tools;
mod app_views;
mod categorize;
mod engine;
mod ext;
mod settings;

/// Tous les couples (français, anglais).
pub fn entries() -> impl Iterator<Item = (&'static str, &'static str)> {
    [
        settings::ENTRIES,
        app_shell::ENTRIES,
        app_main::ENTRIES,
        app_tools::ENTRIES,
        app_data::ENTRIES,
        app_views::ENTRIES,
        categorize::ENTRIES,
        engine::ENTRIES,
        ext::ENTRIES,
    ]
    .into_iter()
    .flatten()
    .copied()
}

/// Les couples, catalogue par catalogue (pour les vérifications).
#[cfg(test)]
pub fn by_file() -> Vec<(&'static str, &'static [(&'static str, &'static str)])> {
    vec![
        ("settings", settings::ENTRIES),
        ("app_shell", app_shell::ENTRIES),
        ("app_main", app_main::ENTRIES),
        ("app_tools", app_tools::ENTRIES),
        ("app_data", app_data::ENTRIES),
        ("app_views", app_views::ENTRIES),
        ("categorize", categorize::ENTRIES),
        ("engine", engine::ENTRIES),
        ("ext", ext::ENTRIES),
    ]
}
