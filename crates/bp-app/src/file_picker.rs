//! Sélecteurs de fichiers natifs ouverts dans un fil à part : pendant que
//! l'utilisateur choisit, l'interface continue de tourner. Ouvert depuis le
//! fil de l'interface, le sélecteur la bloquait : sous Linux, le compositeur
//! la croyait figée au bout de quelques secondes et proposait de la terminer.
//!
//! Deux usages :
//! - dans une fenêtre redessinée à chaque image : [`open`] au clic, puis
//!   [`chosen`] à chaque image, au même endroit du code ;
//! - pour une commande ponctuelle : `BankPerfectApp::ask`, avec la suite à
//!   exécuter une fois le fichier choisi.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Ce que demande le sélecteur.
#[derive(Clone, Copy)]
pub enum Kind {
    File,
    Folder,
    Save,
}

/// Chemin choisi (`None` si annulé) et instant de fermeture du sélecteur.
type Outcome = (Option<PathBuf>, Instant);

/// Résultat d'un sélecteur : `None` tant qu'il est ouvert.
#[derive(Clone, Default)]
struct Slot(Arc<Mutex<Option<Outcome>>>);

/// Un résultat plus ancien n'est plus attendu (fenêtre fermée entre-temps).
const STALE: Duration = Duration::from_secs(3);

/// Le sélecteur ouvert, s'il y en a un.
fn current_id() -> egui::Id {
    egui::Id::new("file_picker_current")
}

/// Un sélecteur est-il ouvert ?
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Slot>(current_id()))
        .is_some_and(|slot| slot.0.lock().is_ok_and(|result| result.is_none()))
}

/// Ouvre le sélecteur `dialog` ; le résultat sera rendu par [`finished`] (ou
/// [`chosen`]) avec le même `id`. Sans effet si un sélecteur est déjà ouvert.
pub fn open(ctx: &egui::Context, id: egui::Id, kind: Kind, dialog: rfd::FileDialog) {
    if is_open(ctx) {
        return;
    }
    let slot = Slot::default();
    ctx.data_mut(|d| {
        d.insert_temp(id, slot.clone());
        d.insert_temp(current_id(), slot.clone());
    });
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let path = match kind {
            Kind::File => dialog.pick_file(),
            Kind::Folder => dialog.pick_folder(),
            Kind::Save => dialog.save_file(),
        };
        if let Ok(mut result) = slot.0.lock() {
            *result = Some((path, Instant::now()));
        }
        ctx.request_repaint();
    });
}

/// `Some(choix)` une seule fois, quand le sélecteur `id` vient de se fermer
/// (`Some(None)` s'il a été annulé) ; `None` tant qu'il est ouvert.
pub fn finished(ctx: &egui::Context, id: egui::Id) -> Option<Option<PathBuf>> {
    let slot = ctx.data(|d| d.get_temp::<Slot>(id))?;
    let (path, at) = slot.0.lock().ok()?.take()?;
    ctx.data_mut(|d| d.remove::<Slot>(id));
    (at.elapsed() < STALE).then_some(path)
}

/// Chemin choisi dans le sélecteur `id`, une seule fois, à sa fermeture.
pub fn chosen(ctx: &egui::Context, id: egui::Id) -> Option<PathBuf> {
    finished(ctx, id).flatten()
}
