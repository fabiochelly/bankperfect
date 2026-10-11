//! Vérifie le décodage v5 sur les fichiers .bp de l'ancien dossier BankPerfect,
//! s'ils sont présents à côté du projet. Aucun contenu n'est affiché.

use std::path::PathBuf;

fn legacy_files() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("bp" | "bp0")))
        .collect()
}

#[test]
fn decodes_every_legacy_file() {
    for path in legacy_files() {
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(bp_core::detect_format(&raw).unwrap(), bp_core::FileFormat::V5, "{path:?}");
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
