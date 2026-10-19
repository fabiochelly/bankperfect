use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use super::*;

#[test]
fn formats_placeholders() {
    let a: &dyn Display = &"a";
    let b: &dyn Display = &2;
    assert_eq!(format("{} et {}", &[a, b], &[]), "a et 2");
    assert_eq!(format("{1} puis {0}", &[a, b], &[]), "2 puis a");
    assert_eq!(format("{x} : {{ok}}", &[], &[("x", b)]), "2 : {ok}");
    assert_eq!(format("{inconnu} }", &[], &[]), "{inconnu} }");
}

#[test]
fn macros_fill_values() {
    with_lang(Lang::Fr, || {
        let n = 2;
        assert_eq!(
            tr!("{n} opérations importées", n = n),
            "2 opérations importées"
        );
        assert_eq!(trn!(1, "{} opération", "{} opérations"), "1 opération");
        assert_eq!(trn!(0, "{} opération", "{} opérations"), "0 opération");
        assert_eq!(
            trn!(3usize, "{} opération", "{} opérations"),
            "3 opérations"
        );
    });
    with_lang(Lang::En, || {
        assert_eq!(trc!("test", "Inconnu"), "Inconnu");
        assert!(!singular(0));
        assert!(singular(1));
        assert_eq!(
            super::tr("Texte absent du catalogue"),
            "Texte absent du catalogue"
        );
    });
}

#[test]
fn strips_english_thousands_only() {
    assert_eq!(with_lang(Lang::Fr, || strip_thousands("1,500")), "1,500");
    with_lang(Lang::En, || {
        assert_eq!(strip_thousands("-1,234,567.89"), "-1234567.89");
        assert_eq!(strip_thousands("12,50"), "12,50");
        assert_eq!(strip_thousands("1,2345"), "1,2345");
        assert_eq!(strip_thousands("-€1,500"), "-€1500");
    });
}

#[test]
fn language_from_locale() {
    assert_eq!(Lang::from_locale("fr_FR.UTF-8"), Lang::Fr);
    assert_eq!(Lang::from_locale("fr-CA"), Lang::Fr);
    assert_eq!(Lang::from_locale("en-GB"), Lang::En);
    assert_eq!(Lang::from_locale("de_DE"), Lang::En);
    assert_eq!(Lang::from_locale("C"), Lang::En);
}

/// Texte trouvé dans le code.
struct Use {
    text: String,
    /// Suivi de valeurs (`tr!("…", x)`) ; toujours vrai pour `trn!`.
    with_values: bool,
    /// Repéré par `tr_mark!` (complété plus tard par `i18n::format`).
    marked: bool,
    place: String,
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                rust_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Lit une chaîne littérale à partir de `"` ; renvoie le texte et la
/// position après le guillemet fermant.
fn string_literal(src: &[char], start: usize) -> (String, usize) {
    let mut out = String::new();
    let mut i = start + 1;
    while i < src.len() {
        match src[i] {
            '"' => return (out, i + 1),
            '\\' => {
                i += 1;
                match src[i] {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '0' => out.push('\0'),
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    '\'' => out.push('\''),
                    'x' => {
                        let hex: String = src[i + 1..i + 3].iter().collect();
                        out.push(u8::from_str_radix(&hex, 16).unwrap() as char);
                        i += 2;
                    }
                    'u' => {
                        let end = (i..src.len()).find(|&j| src[j] == '}').unwrap();
                        let hex: String = src[i + 2..end].iter().collect();
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
                        i = end;
                    }
                    '\n' | '\r' => {
                        while i + 1 < src.len() && src[i + 1].is_whitespace() {
                            i += 1;
                        }
                    }
                    other => panic!("échappement inconnu \\{other}"),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    panic!("chaîne non terminée");
}

/// Les `tr!`, `trn!` et `tr_mark!` d'un fichier (hors commentaires).
fn uses_in(path: &Path, out: &mut Vec<Use>) {
    let src: Vec<char> = std::fs::read_to_string(path).unwrap().chars().collect();
    let line_of = |pos: usize| src[..pos].iter().filter(|&&c| c == '\n').count() + 1;
    let skip_ws = |mut i: usize| {
        while i < src.len() && src[i].is_whitespace() {
            i += 1;
        }
        i
    };
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        if c == '/' && src.get(i + 1) == Some(&'/') {
            while i < src.len() && src[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && src.get(i + 1) == Some(&'*') {
            while i + 1 < src.len() && !(src[i] == '*' && src[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '"' {
            i = string_literal(&src, i).1;
        } else if c == 'r'
            && matches!(src.get(i + 1), Some('"' | '#'))
            && (!src[i - 1].is_alphanumeric() || src[i - 1] == 'b' && !src[i - 2].is_alphanumeric())
        {
            let hashes = (i + 1..).take_while(|&j| src[j] == '#').count();
            let open = i + 1 + hashes;
            if src.get(open) == Some(&'"') {
                let close: String = std::iter::once('"')
                    .chain("#".repeat(hashes).chars())
                    .collect();
                let rest: String = src[open + 1..].iter().collect();
                i = open + 1 + rest.find(&close).unwrap() + close.len();
            } else {
                i += 1;
            }
        } else if c == '\'' {
            // Caractère ('x', '\n', '\'') ou durée de vie ('a).
            if src.get(i + 1) == Some(&'\\') {
                i = (i + 2..).find(|&j| src[j] == '\'').unwrap() + 1;
            } else if src.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < src.len() && (src[i].is_alphanumeric() || src[i] == '_') {
                i += 1;
            }
            let word: String = src[start..i].iter().collect();
            let preceded = start > 0 && (src[start - 1] == ':' || src[start - 1] == '.');
            let is_macro = matches!(word.as_str(), "tr" | "trn" | "trc" | "tr_mark");
            if !is_macro || src.get(i) != Some(&'!') || (preceded && src[start - 1] == '.') {
                continue;
            }
            let open = skip_ws(i + 1);
            if src.get(open) != Some(&'(') {
                continue;
            }
            let place = format!("{}:{}", path.display(), line_of(start));
            let mut j = skip_ws(open + 1);
            let mut context = String::new();
            if word == "trc" {
                assert_eq!(
                    src.get(j),
                    Some(&'"'),
                    "{place} : trc! attend un contexte littéral"
                );
                let (text, after) = string_literal(&src, j);
                context = format!("{text}|");
                j = skip_ws(skip_ws(after) + 1);
            }
            let wanted = if word == "trn" {
                // Sauter le nombre jusqu'à la virgule de premier niveau.
                let mut depth = 0;
                while !(depth == 0 && src[j] == ',') {
                    match src[j] {
                        '(' | '[' | '{' => depth += 1,
                        ')' | ']' | '}' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                j = skip_ws(j + 1);
                2
            } else {
                1
            };
            for k in 0..wanted {
                assert_eq!(
                    src.get(j),
                    Some(&'"'),
                    "{place} : {word}! attend un texte littéral"
                );
                let (text, after) = string_literal(&src, j);
                j = skip_ws(after);
                let with_values = word == "trn"
                    || src.get(j) == Some(&',') && {
                        let next = skip_ws(j + 1);
                        src.get(next) != Some(&')')
                    };
                out.push(Use {
                    text: format!("{context}{text}"),
                    with_values,
                    marked: word == "tr_mark",
                    place: place.clone(),
                });
                if k + 1 < wanted {
                    j = skip_ws(j + 1);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
}

fn all_uses() -> Vec<Use> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    rust_files(crates, &mut files);
    files.sort();
    let mut uses = Vec::new();
    for file in files {
        if file.components().any(|c| c.as_os_str() == "i18n") {
            continue;
        }
        uses_in(&file, &mut uses);
    }
    uses
}

/// Repères d'un texte : les noms et rangs, et le nombre de `{}`.
fn placeholders(text: &str) -> (BTreeSet<String>, usize) {
    let clean = text.replace("{{", "").replace("}}", "");
    let mut named = BTreeSet::new();
    let mut anonymous = 0;
    let mut rest = clean.as_str();
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            break;
        };
        let key = &rest[start + 1..start + len];
        if key.is_empty() {
            anonymous += 1;
        } else {
            named.insert(key.to_string());
        }
        rest = &rest[start + len + 1..];
    }
    (named, anonymous)
}

#[test]
fn every_text_is_translated() {
    let uses = all_uses();
    let mut catalog: HashMap<&str, (&str, &str)> = HashMap::new();
    let mut problems = Vec::new();
    for (file, entries) in en::by_file() {
        for &(fr, en) in entries {
            if let Some((other_file, other)) = catalog.insert(fr, (file, en))
                && other != en
            {
                problems.push(format!(
                    "« {fr} » traduit « {other} » ({other_file}) et « {en} » ({file})"
                ));
            }
            if placeholders(fr) != placeholders(en) {
                problems.push(format!("repères différents : « {fr} » / « {en} » ({file})"));
            }
            if fr.contains("{:") || en.contains("{:") {
                problems.push(format!("format dans un repère : « {fr} » ({file})"));
            }
        }
    }
    let mut used = BTreeSet::new();
    for u in &uses {
        used.insert(u.text.as_str());
        if !catalog.contains_key(u.text.as_str()) {
            problems.push(format!(
                "{} : « {} » sans traduction anglaise",
                u.place, u.text
            ));
        }
        let (named, anonymous) = placeholders(&u.text);
        if !u.with_values && !u.marked && (anonymous > 0 || !named.is_empty()) {
            problems.push(format!(
                "{} : « {} » a des repères mais aucune valeur",
                u.place, u.text
            ));
        }
    }
    for (file, entries) in en::by_file() {
        for &(fr, _) in entries {
            if !used.contains(fr) {
                problems.push(format!("« {fr} » ({file}) n'est plus utilisé"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{} problèmes :\n{}",
        problems.len(),
        problems.join("\n")
    );
}
