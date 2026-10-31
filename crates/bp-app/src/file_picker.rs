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
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Un sélecteur refermé plus vite n'a pas pu s'ouvrir : rfd rend `None`
/// aussi bien pour une erreur que pour une annulation.
const FAILED: Duration = Duration::from_millis(300);

/// Un sélecteur n'a pas pu s'ouvrir (voir [`take_failure`]).
static FAILURE: AtomicBool = AtomicBool::new(false);

/// Fenêtre principale, propriétaire des sélecteurs sous Windows : sans
/// propriétaire, un sélecteur ouvert depuis un autre fil peut s'afficher
/// derrière elle, et l'application semble ne rien faire.
#[cfg(windows)]
static MAIN_WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Retient la fenêtre principale (Windows seulement).
pub fn set_main_window(frame: &eframe::Frame) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = frame.window_handle()
            && let RawWindowHandle::Win32(win32) = handle.as_raw()
        {
            MAIN_WINDOW.store(win32.hwnd.get(), Ordering::Relaxed);
        }
    }
    #[cfg(not(windows))]
    let _ = frame;
}

/// Fenêtre principale vue par rfd.
#[cfg(windows)]
struct MainWindow(std::num::NonZeroIsize);

#[cfg(windows)]
impl raw_window_handle::HasWindowHandle for MainWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let raw = raw_window_handle::RawWindowHandle::Win32(
            raw_window_handle::Win32WindowHandle::new(self.0),
        );
        // SAFETY: la fenêtre principale vit aussi longtemps que l'application.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

#[cfg(windows)]
impl raw_window_handle::HasDisplayHandle for MainWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::windows())
    }
}

/// `dialog` rattaché à la fenêtre principale (Windows seulement).
fn with_parent(dialog: rfd::FileDialog) -> rfd::FileDialog {
    #[cfg(windows)]
    if let Some(hwnd) = std::num::NonZeroIsize::new(MAIN_WINDOW.load(Ordering::Relaxed)) {
        return dialog.set_parent(&MainWindow(hwnd));
    }
    dialog
}

/// Un sélecteur a-t-il échoué à s'ouvrir depuis le dernier appel ?
pub fn take_failure() -> bool {
    FAILURE.swap(false, Ordering::Relaxed)
}

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
    let dialog = with_parent(dialog);
    std::thread::spawn(move || {
        let started = Instant::now();
        let path = match kind {
            Kind::File => dialog.pick_file(),
            Kind::Folder => dialog.pick_folder(),
            Kind::Save => dialog.save_file(),
        };
        if path.is_none() && started.elapsed() < FAILED {
            FAILURE.store(true, Ordering::Relaxed);
        }
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
    ctx.data_mut(|d| {
        d.remove::<Slot>(id);
        // `take()` a vidé le résultat : sans retirer aussi ce même slot
        // global, `is_open()` le prendrait pour un sélecteur encore ouvert.
        // Un ancien résultat ne doit pas effacer un sélecteur plus récent.
        if d.get_temp::<Slot>(current_id())
            .is_some_and(|current| Arc::ptr_eq(&current.0, &slot.0))
        {
            d.remove::<Slot>(current_id());
        }
    });
    (at.elapsed() < STALE).then_some(path)
}

/// Chemin choisi dans le sélecteur `id`, une seule fois, à sa fermeture.
pub fn chosen(ctx: &egui::Context, id: egui::Id) -> Option<PathBuf> {
    finished(ctx, id).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consuming_a_result_allows_another_picker() {
        for path in [Some(PathBuf::from("test.pem")), None] {
            let ctx = egui::Context::default();
            let id = egui::Id::new("test_picker");
            let slot = Slot::default();
            ctx.data_mut(|d| {
                d.insert_temp(id, slot.clone());
                d.insert_temp(current_id(), slot.clone());
            });
            assert!(is_open(&ctx));
            *slot.0.lock().unwrap() = Some((path.clone(), Instant::now()));
            assert_eq!(finished(&ctx, id), Some(path));
            assert!(!is_open(&ctx));
            assert_eq!(finished(&ctx, id), None);
        }
    }

    #[test]
    fn consuming_a_stale_result_releases_the_picker() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("test_picker");
        let slot = Slot::default();
        *slot.0.lock().unwrap() = Some((None, Instant::now() - STALE));
        ctx.data_mut(|d| {
            d.insert_temp(id, slot.clone());
            d.insert_temp(current_id(), slot);
        });
        assert_eq!(finished(&ctx, id), None);
        assert!(!is_open(&ctx));
    }

    #[test]
    fn consuming_an_old_result_preserves_a_new_picker() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("old_picker");
        let old = Slot::default();
        *old.0.lock().unwrap() = Some((None, Instant::now()));
        ctx.data_mut(|d| {
            d.insert_temp(id, old);
            d.insert_temp(current_id(), Slot::default());
        });
        assert_eq!(finished(&ctx, id), Some(None));
        assert!(is_open(&ctx));
    }
}
