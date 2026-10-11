//! Sauvegardes `.bp0` : copie de la version précédente avant chaque
//! enregistrement, et liste pour la restauration.
//!
//! L'ancienne version utilisait 20 emplacements numérotés et un compteur
//! commun à tous les fichiers. Ici, chaque fichier a ses sauvegardes,
//! horodatées (`<nom>_AAAAMMJJ-HHMMSS.bp0`), dont on garde les plus récentes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const EXTENSION: &str = "bp0";
pub const DEFAULT_KEEP: usize = 20;

/// Une sauvegarde trouvée sur disque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub path: PathBuf,
    pub modified: SystemTime,
    pub size: u64,
}

fn stem_of(file: &Path) -> String {
    file.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Copie `file` (s'il existe) dans `dir` avant son remplacement, puis ne garde
/// que les `keep` sauvegardes les plus récentes de ce fichier. Renvoie le
/// chemin de la sauvegarde créée.
pub fn backup_before_save(file: &Path, dir: &Path, keep: usize) -> io::Result<Option<PathBuf>> {
    if !file.exists() {
        return Ok(None);
    }
    fs::create_dir_all(dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let stem = stem_of(file);
    let mut target = dir.join(format!("{stem}_{stamp}.{EXTENSION}"));
    let mut n = 1;
    while target.exists() {
        n += 1;
        target = dir.join(format!("{stem}_{stamp}-{n}.{EXTENSION}"));
    }
    fs::copy(file, &target)?;
    prune(file, dir, keep.max(1))?;
    Ok(Some(target))
}

/// Sauvegardes de `file` dans `dir`, la plus récente en premier.
pub fn list(file: &Path, dir: &Path) -> Vec<Backup> {
    let prefix = format!("{}_", stem_of(file));
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut backups: Vec<Backup> = entries
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(&prefix) && name.ends_with(&format!(".{EXTENSION}"))
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(Backup {
                path: e.path(),
                modified: meta.modified().ok()?,
                size: meta.len(),
            })
        })
        .collect();
    backups.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.path.cmp(&a.path))
    });
    backups
}

fn prune(file: &Path, dir: &Path, keep: usize) -> io::Result<()> {
    for old in list(file, dir).into_iter().skip(keep) {
        fs::remove_file(old.path)?;
    }
    Ok(())
}

/// Restaure une sauvegarde : la version actuelle est d'abord elle-même
/// sauvegardée, pour que la restauration soit réversible.
pub fn restore(backup: &Path, file: &Path, dir: &Path, keep: usize) -> io::Result<()> {
    backup_before_save(file, dir, keep + 1)?;
    let data = fs::read(backup)?;
    crate::format_v6::write_atomic(file, &data).map_err(|e| io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_most_recent_backups_per_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Comptes.bp");
        let other = dir.path().join("Autre.bp");
        let backups = dir.path().join("sauvegardes");
        fs::write(&other, b"autre").unwrap();
        backup_before_save(&other, &backups, 3).unwrap();
        for i in 0..5 {
            fs::write(&file, format!("version {i}")).unwrap();
            backup_before_save(&file, &backups, 3).unwrap();
        }
        let list = list(&file, &backups);
        assert_eq!(list.len(), 3, "seules les 3 plus récentes sont gardées");
        assert_eq!(fs::read_to_string(&list[0].path).unwrap(), "version 4");
        assert_eq!(
            super::list(&other, &backups).len(),
            1,
            "les sauvegardes des autres fichiers sont intactes"
        );
    }

    #[test]
    fn missing_file_needs_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            backup_before_save(&dir.path().join("absent.bp"), dir.path(), 3).unwrap(),
            None
        );
    }

    #[test]
    fn restore_is_reversible() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Comptes.bp");
        let backups = dir.path().join("sauvegardes");
        fs::write(&file, b"ancienne").unwrap();
        let saved = backup_before_save(&file, &backups, 5).unwrap().unwrap();
        fs::write(&file, b"nouvelle").unwrap();
        restore(&saved, &file, &backups, 5).unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"ancienne");
        assert!(
            list(&file, &backups)
                .iter()
                .any(|b| fs::read(&b.path).unwrap() == b"nouvelle")
        );
    }
}
