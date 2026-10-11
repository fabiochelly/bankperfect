//! Vérifie le décodage v5 sur les fichiers .bp de l'ancien dossier BankPerfect,
//! s'ils sont présents à côté du projet. Aucun contenu n'est affiché.

use std::path::PathBuf;

fn legacy_files() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("bp" | "bp0")))
        .collect()
}

#[test]
fn decodes_every_legacy_file() {
    for path in legacy_files() {
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(
            bp_core::detect_format(&raw).unwrap(),
            bp_core::FileFormat::V5,
            "{path:?}"
        );
        let text = bp_core::codec_v5::decode(&raw).unwrap();
        assert!(text.starts_with("utilisateur="), "{path:?}");
    }
}

#[test]
fn legacy_content_survives_v6_roundtrip() {
    for path in legacy_files() {
        let text = bp_core::codec_v5::decode(&std::fs::read(&path).unwrap()).unwrap();
        let protection = bp_core::format_v6::Protection::none().unwrap();
        let file = protection.seal(text.as_bytes()).unwrap();
        let (content, _) =
            bp_core::format_v6::Protection::open(&file, bp_core::format_v6::Unlock::None).unwrap();
        assert_eq!(content, text.as_bytes(), "{path:?}");
    }
}

#[test]
fn legacy_files_open_and_survive_v6_save() {
    use bp_core::file;
    use bp_core::format_v6::Protection;

    let dir = tempfile::tempdir().unwrap();
    for path in legacy_files() {
        let raw = std::fs::read(&path).unwrap();
        if file::requires_password(&raw).unwrap() {
            // Fichier protégé : on vérifie seulement que l'ouverture est refusée sans mot de passe.
            assert!(matches!(
                file::open(&path, None),
                Err(bp_core::Error::PasswordRequired)
            ));
            continue;
        }
        let opened = file::open(&path, None).unwrap();
        let doc = &opened.document;
        assert!(!doc.accounts.is_empty(), "{path:?}");
        assert_eq!(doc.account_order.len(), doc.accounts.len());
        let uids: std::collections::HashSet<u64> = doc
            .accounts
            .iter()
            .flat_map(|a| &a.operations)
            .map(|o| o.uid)
            .collect();
        let count: usize = doc.accounts.iter().map(|a| a.operations.len()).sum();
        assert_eq!(uids.len(), count, "UId uniques : {path:?}");

        let out = dir.path().join("copie.bp");
        file::save(&out, doc, &Protection::none().unwrap()).unwrap();
        let reopened = file::open(&out, None).unwrap();
        assert_eq!(&reopened.document, doc, "{path:?}");
    }
}
