//! Fichier modifié ailleurs pendant qu'il est ouvert : autre ordinateur ou
//! téléphone, via Dropbox, Google Drive, OneDrive…
//!
//! On garde la version lue ou écrite en dernier (l'ancêtre commun) et
//! l'empreinte du fichier correspondant. Si le fichier sur disque a changé,
//! il est relu avec la clé en mémoire et fusionné avec la version affichée
//! ([`bp_core::merge`]) : avant chaque enregistrement, et en cours de route
//! quand rien n'est en cours de saisie.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bp_core::file;
use bp_core::format_v6::Protection;
use bp_core::merge::{self, Conflict};
use bp_core::model::Document;
use bp_core::tr;

use crate::document::OpenDoc;

/// Version commune avec le fichier sur disque.
pub struct DiskState {
    path: PathBuf,
    /// Contenu du fichier tel que lu ou écrit en dernier.
    base: Document,
    hash: [u8; 32],
    /// Clé avec laquelle relire le fichier : celle du dernier enregistrement,
    /// même si le mot de passe vient d'être changé en mémoire.
    protection: Protection,
    /// Date et taille du fichier à la dernière vérification (surveillance peu
    /// coûteuse : le contenu n'est relu que si elles changent).
    seen: Option<(SystemTime, u64)>,
}

impl DiskState {
    pub fn new(path: &Path, base: Document, bytes: &[u8], protection: &Protection) -> Self {
        Self {
            path: path.to_path_buf(),
            base,
            hash: file::fingerprint(bytes),
            protection: protection.clone(),
            seen: stat(path),
        }
    }
}

fn stat(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Modifications venues d'ailleurs, intégrées au document.
pub struct Incoming {
    pub conflicts: Vec<String>,
}

impl Incoming {
    /// Message pour l'utilisateur (`None` si tout s'est fusionné sans conflit).
    pub fn conflict_report(&self) -> Option<String> {
        if self.conflicts.is_empty() {
            return None;
        }
        let mut text = tr!(
            "Ce fichier a été modifié sur un autre appareil. Ses modifications ont été intégrées ; ces points ont été modifiés des deux côtés :"
        )
        .to_string();
        for line in &self.conflicts {
            text.push_str("\n• ");
            text.push_str(line);
        }
        Some(text)
    }
}

/// Le fichier semble avoir changé sur disque (date ou taille) depuis la
/// dernière vérification.
pub fn looks_changed(doc: &OpenDoc) -> bool {
    doc.disk
        .as_ref()
        .is_some_and(|s| s.path == doc.path && stat(&s.path) != s.seen)
}

/// Retient la date et la taille actuelles du fichier : la surveillance ne
/// signalera plus ce changement (il sera fusionné à l'enregistrement).
pub fn mark_seen(doc: &mut OpenDoc) {
    if let Some(state) = &mut doc.disk {
        state.seen = stat(&state.path);
    }
}

/// Relit le fichier s'il a changé sur disque et fusionne ses modifications
/// dans le document ouvert. `Ok(None)` s'il n'a pas changé.
pub fn integrate(doc: &mut OpenDoc) -> Result<Option<Incoming>, String> {
    let Some(mut state) = doc.disk.take() else {
        return Ok(None);
    };
    let result = integrate_with(doc, &mut state);
    doc.disk = Some(state);
    result
}

fn integrate_with(doc: &mut OpenDoc, state: &mut DiskState) -> Result<Option<Incoming>, String> {
    // Enregistré sous un autre nom : rien à fusionner.
    if state.path != doc.path {
        return Ok(None);
    }
    let bytes = match std::fs::read(&state.path) {
        Ok(bytes) => bytes,
        // Fichier déplacé ou supprimé ailleurs : l'enregistrement le recrée.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(tr!("Impossible de relire {} : {}", state.path.display(), e));
        }
    };
    // Vu, même si la relecture échoue : la surveillance ne réessaie pas en
    // boucle (l'enregistrement, lui, compare toujours le contenu).
    state.seen = stat(&state.path);
    let hash = file::fingerprint(&bytes);
    if hash == state.hash {
        return Ok(None);
    }
    let theirs = match file::reopen_bytes(&bytes, &state.protection) {
        Ok(Some(document)) => document,
        Ok(None) => {
            return Err(tr!(
                "Ce fichier a été enregistré sur un autre appareil avec un autre mot de passe. Pour ne rien écraser, il n'a pas été enregistré : fermez-le et rouvrez-le, ou gardez vos modifications avec Fichier > Enregistrer sous."
            )
            .into());
        }
        Err(e) => {
            return Err(tr!(
                "Ce fichier a été modifié sur un autre appareil mais cette version est illisible ({}). Il n'a pas été enregistré : gardez vos modifications avec Fichier > Enregistrer sous.",
                e
            ));
        }
    };
    let merged = merge::merge(&state.base, &doc.document, &theirs).map_err(|e| {
        tr!(
            "Impossible d'intégrer les modifications faites ailleurs : {}. Gardez vos modifications avec Fichier > Enregistrer sous.",
            e
        )
    })?;
    let conflicts = merged.conflicts.iter().map(Conflict::describe).collect();
    // Tout ce qui est sur disque fait maintenant partie du document : c'est
    // le nouvel ancêtre commun.
    let unsaved = merged.document != theirs;
    state.base = theirs;
    state.hash = hash;
    if merged.document != doc.document {
        doc.replace_document(merged.document);
    }
    doc.dirty = unsaved;
    Ok(Some(Incoming { conflicts }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bp_core::model::{Amount, Tack};

    /// Fichier ouvert ici, puis enregistré « ailleurs » (autre appareil).
    fn setup() -> (tempfile::TempDir, OpenDoc, Protection) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Comptes.bp");
        let mut document = bp_core::manage::new_document("Comptes", Amount(10_000));
        document.guid = "{TEST}".into();
        let protection = Protection::none().unwrap();
        let bytes = file::to_bytes(&document, &protection).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let opened = file::open(&path, None).unwrap();
        let protection = opened.protection.clone().unwrap();
        let mut doc = OpenDoc::new(path.clone(), opened);
        doc.disk = Some(DiskState::new(
            &path,
            doc.document.clone(),
            &bytes,
            &protection,
        ));
        (dir, doc, protection)
    }

    fn save_elsewhere(doc: &OpenDoc, protection: &Protection, change: impl FnOnce(&mut Document)) {
        let mut other = file::open(&doc.path, None).unwrap().document;
        change(&mut other);
        let bytes = file::to_bytes(&other, protection).unwrap();
        bp_core::format_v6::write_atomic(&doc.path, &bytes).unwrap();
    }

    #[test]
    fn unchanged_file_needs_nothing() {
        let (_dir, mut doc, _) = setup();
        assert!(integrate(&mut doc).unwrap().is_none());
    }

    #[test]
    fn changes_from_both_sides_are_merged() {
        let (_dir, mut doc, protection) = setup();
        let first = doc.document.accounts[0].operations[0].uid;
        save_elsewhere(&doc, &protection, |other| {
            let mut op = other.accounts[0].operations[0].clone();
            op.uid = other.next_operation_uid();
            op.payee = "Pharmacie".into();
            op.amount = Amount(-1290);
            other.accounts[0].operations.push(op);
        });
        doc.document.accounts[0].operations[0].tack = Tack::Pointed;
        doc.dirty = true;

        let incoming = integrate(&mut doc).unwrap().expect("changement vu");
        assert!(incoming.conflict_report().is_none());
        let ops = &doc.document.accounts[0].operations;
        assert_eq!(ops.len(), 2);
        assert_eq!(
            ops.iter().find(|o| o.uid == first).unwrap().tack,
            Tack::Pointed
        );
        assert!(ops.iter().any(|o| o.payee == "Pharmacie"));
        assert!(doc.dirty, "le pointage local reste à enregistrer");
        // Le disque est désormais la base : pas de seconde fusion.
        assert!(integrate(&mut doc).unwrap().is_none());
    }

    #[test]
    fn clean_document_takes_the_disk_version() {
        let (_dir, mut doc, protection) = setup();
        save_elsewhere(&doc, &protection, |other| {
            other.accounts[0].operations[0].payee = "Solde initial corrigé".into();
        });
        integrate(&mut doc).unwrap().expect("changement vu");
        assert_eq!(
            doc.document.accounts[0].operations[0].payee,
            "Solde initial corrigé"
        );
        assert!(!doc.dirty);
    }

    #[test]
    fn password_changed_elsewhere_blocks_the_save() {
        let (_dir, mut doc, _) = setup();
        let other = bp_core::format_v6::Protection::with_password("autre mot de passe")
            .unwrap()
            .0;
        save_elsewhere(&doc, &other, |_| {});
        assert!(integrate(&mut doc).is_err());
    }
}
