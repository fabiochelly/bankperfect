//! Langue de l'interface.
//!
//! Les textes restent écrits en français dans le code : `tr!("Fichier")`
//! renvoie le texte tel quel en français, sa traduction dans les autres
//! langues (catalogues de `en/`). Un texte absent du catalogue reste en
//! français ; le test `every_text_is_translated` l'interdit.
//!
//! Les textes à compléter utilisent des repères `{}` (dans l'ordre),
//! `{0}` (par rang) ou `{nom}` (par nom), sans format (`{:.2}`) : les
//! valeurs arrivent déjà mises en forme.
//!
//! ```
//! use bp_core::{tr, trn};
//! let n = 3;
//! assert_eq!(tr!("Annuler"), "Annuler");
//! assert_eq!(tr!("{n} opérations importées", n = n), "3 opérations importées");
//! assert_eq!(trn!(n, "{} opération", "{} opérations"), "3 opérations");
//! ```

mod en;

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// Langue de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Fr, Lang::En];

    /// Code ISO 639-1 (`fr`, `en`), tel qu'enregistré dans les réglages.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == code)
    }

    /// Nom de la langue dans cette langue (pour le sélecteur).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Fr => "Français",
            Lang::En => "English",
        }
    }

    /// Langue d'après une étiquette de langue (`fr-FR`, `en_US.UTF-8`…) :
    /// le français pour le français, l'anglais pour toutes les autres.
    pub fn from_locale(locale: &str) -> Lang {
        let primary = locale
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if primary == "fr" { Lang::Fr } else { Lang::En }
    }

    fn from_u8(value: u8) -> Lang {
        match value {
            1 => Lang::En,
            _ => Lang::Fr,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Lang::Fr => 0,
            Lang::En => 1,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

thread_local! {
    /// Langue imposée au fil courant (tests), prioritaire sur `CURRENT`.
    static OVERRIDE: Cell<Option<Lang>> = const { Cell::new(None) };
}

/// Langue en cours.
pub fn lang() -> Lang {
    OVERRIDE
        .with(Cell::get)
        .unwrap_or_else(|| Lang::from_u8(CURRENT.load(Ordering::Relaxed)))
}

/// Change la langue de toute l'application.
pub fn set_lang(lang: Lang) {
    CURRENT.store(lang.as_u8(), Ordering::Relaxed);
}

/// Exécute `f` avec la langue `lang` sur ce fil seulement (les tests
/// tournent en parallèle et ne doivent pas se gêner).
pub fn with_lang<R>(lang: Lang, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Lang>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDE.with(|o| o.set(self.0));
        }
    }
    let _restore = Restore(OVERRIDE.with(|o| o.replace(Some(lang))));
    f()
}

fn catalog(lang: Lang) -> Option<&'static HashMap<&'static str, &'static str>> {
    static EN: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    match lang {
        Lang::Fr => None,
        Lang::En => Some(EN.get_or_init(|| en::entries().collect())),
    }
}

/// Traduction d'un texte français dans la langue en cours (le texte lui-même
/// s'il n'est pas au catalogue). Sert aussi aux textes repérés par
/// [`tr_mark!`](crate::tr_mark) et rangés dans des tables.
pub fn tr(fr: &str) -> &str {
    match catalog(lang()) {
        Some(map) => map.get(fr).copied().unwrap_or(fr),
        None => fr,
    }
}

/// Traduction de `key` (`contexte|texte`), sinon le texte français `fr`.
pub fn tr_in_context<'a>(key: &str, fr: &'a str) -> &'a str {
    match catalog(lang()) {
        Some(map) => map.get(key).copied().unwrap_or(fr),
        None => fr,
    }
}

/// Le singulier convient-il pour `n` ? (0 et 1 en français, 1 en anglais.)
pub fn singular(n: i64) -> bool {
    match lang() {
        Lang::Fr => n.abs() <= 1,
        Lang::En => n.abs() == 1,
    }
}

/// Séparateur décimal des montants affichés (`,` ou `.`).
pub fn decimal_separator() -> char {
    match lang() {
        Lang::Fr => ',',
        Lang::En => '.',
    }
}

/// Séparateur des milliers : espace fine insécable ou virgule.
pub fn thousands_separator() -> char {
    match lang() {
        Lang::Fr => '\u{202f}',
        Lang::En => ',',
    }
}

/// Retire d'un nombre tapé les séparateurs des milliers à l'anglaise : en
/// anglais, une virgule entre un chiffre et exactement trois chiffres
/// (« 1,234.56 », « -1,500 ») ; ailleurs elle reste décimale (« 12,50 »).
/// En français, le texte est rendu tel quel.
pub fn strip_thousands(text: &str) -> String {
    if lang() != Lang::En {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let digit = |i: usize| chars.get(i).is_some_and(char::is_ascii_digit);
    chars
        .iter()
        .enumerate()
        .filter(|&(i, &c)| {
            let thousands =
                c == ',' && i > 0 && digit(i - 1) && (1..=3).all(|k| digit(i + k)) && !digit(i + 4);
            !thousands
        })
        .map(|(_, &c)| c)
        .collect()
}

/// Nom du mois (`month0` de 0 à 11) : « janvier », « January ».
pub fn month_name(month0: usize) -> &'static str {
    const FR: [&str; 12] = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];
    pick(&FR, &EN_MONTHS, month0)
}

/// Nom du mois en début de phrase : « Janvier », « January ».
pub fn month_title(month0: usize) -> &'static str {
    const FR: [&str; 12] = [
        "Janvier",
        "Février",
        "Mars",
        "Avril",
        "Mai",
        "Juin",
        "Juillet",
        "Août",
        "Septembre",
        "Octobre",
        "Novembre",
        "Décembre",
    ];
    pick(&FR, &EN_MONTHS, month0)
}

const EN_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Mois abrégé pour les en-têtes de colonnes : « Janv », « Jan ».
pub fn month_abbr(month0: usize) -> &'static str {
    const FR: [&str; 12] = [
        "Janv", "Févr", "Mars", "Avr", "Mai", "Juin", "Juil", "Août", "Sept", "Oct", "Nov", "Déc",
    ];
    const EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    pick(&FR, &EN, month0)
}

/// Mois abrégé dans une date écrite : « janv. », « Jan ».
pub fn month_abbr_in_date(month0: usize) -> &'static str {
    const FR: [&str; 12] = [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ];
    const EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    pick(&FR, &EN, month0)
}

/// Jour abrégé (`from_monday` de 0 à 6) : « lun. », « Mon ».
pub fn weekday_abbr(from_monday: usize) -> &'static str {
    const FR: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
    const EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    pick(&FR, &EN, from_monday)
}

/// Nom du jour (`from_monday` de 0 à 6) : « lundi », « Monday ».
pub fn weekday_name(from_monday: usize) -> &'static str {
    const FR: [&str; 7] = [
        "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
    ];
    const EN: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    pick(&FR, &EN, from_monday)
}

/// Initiale du jour pour un calendrier : « L », « M ».
pub fn weekday_letter(from_monday: usize) -> &'static str {
    const FR: [&str; 7] = ["L", "M", "M", "J", "V", "S", "D"];
    const EN: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];
    pick(&FR, &EN, from_monday)
}

fn pick<const N: usize>(
    fr: &[&'static str; N],
    en: &[&'static str; N],
    index: usize,
) -> &'static str {
    match lang() {
        Lang::Fr => fr[index % N],
        Lang::En => en[index % N],
    }
}

/// Remplit les repères d'un texte traduit : `{}` prend la valeur suivante de
/// `positional`, `{2}` celle de rang 2, `{nom}` celle de `named` ; `{{` et
/// `}}` donnent une accolade.
pub fn format(
    template: &str,
    positional: &[&dyn Display],
    named: &[(&str, &dyn Display)],
) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        let Some(end) = tail.find('}').filter(|_| tail.starts_with('{')) else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
            continue;
        };
        let key = &tail[1..end];
        let value = if key.is_empty() {
            next += 1;
            positional.get(next - 1).copied()
        } else if let Ok(index) = key.parse::<usize>() {
            positional.get(index).copied()
        } else {
            named.iter().find(|(name, _)| *name == key).map(|(_, v)| *v)
        };
        match value {
            Some(value) => {
                let _ = write!(out, "{value}");
            }
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Texte traduit dans la langue en cours, complété par des valeurs :
/// `tr!("Fermer")`, `tr!("{} sur {}", a, b)`, `tr!("Compte {nom}", nom = n)`.
/// Sans valeur, renvoie un `&str` ; avec, une `String`.
#[macro_export]
macro_rules! tr {
    ($fr:literal $(,)?) => {
        $crate::i18n::tr($fr)
    };
    ($fr:literal $(, $name:ident = $value:expr)+ $(,)?) => {
        $crate::i18n::format(
            $crate::i18n::tr($fr),
            &[],
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+],
        )
    };
    ($fr:literal $(, $value:expr)+ $(,)?) => {
        $crate::i18n::format(
            $crate::i18n::tr($fr),
            &[$(&$value as &dyn ::std::fmt::Display),+],
            &[],
        )
    };
}

/// Texte à double sens, précisé par un contexte : `trc!("undo", "Annuler")`
/// cherche `undo|Annuler` au catalogue et affiche « Annuler » en français.
/// Des valeurs peuvent suivre, comme pour [`tr!`].
#[macro_export]
macro_rules! trc {
    ($ctx:literal, $fr:literal $(,)?) => {
        $crate::i18n::tr_in_context(concat!($ctx, "|", $fr), $fr)
    };
    ($ctx:literal, $fr:literal $(, $name:ident = $value:expr)+ $(,)?) => {
        $crate::i18n::format(
            $crate::i18n::tr_in_context(concat!($ctx, "|", $fr), $fr),
            &[],
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+],
        )
    };
    ($ctx:literal, $fr:literal $(, $value:expr)+ $(,)?) => {
        $crate::i18n::format(
            $crate::i18n::tr_in_context(concat!($ctx, "|", $fr), $fr),
            &[$(&$value as &dyn ::std::fmt::Display),+],
            &[],
        )
    };
}

/// Accord en nombre : `trn!(n, "{} ligne", "{} lignes")` choisit le texte
/// selon la règle de la langue et y place `n` (repère `{}` ou `{0}`) ; des
/// valeurs supplémentaires peuvent suivre (`{1}`, `{2}`…).
#[macro_export]
macro_rules! trn {
    ($n:expr, $one:literal, $many:literal $(, $value:expr)* $(,)?) => {{
        let n = $n;
        let template = if $crate::i18n::singular(n as i64) {
            $crate::i18n::tr($one)
        } else {
            $crate::i18n::tr($many)
        };
        $crate::i18n::format(
            template,
            &[&n as &dyn ::std::fmt::Display $(, &$value as &dyn ::std::fmt::Display)*],
            &[],
        )
    }};
}

/// Repère un texte à traduire plus tard, rangé dans une table ou une
/// constante : `tr_mark!("Ouvrir")` vaut le texte lui-même, à passer à
/// [`i18n::tr`](crate::i18n::tr) au moment de l'afficher.
#[macro_export]
macro_rules! tr_mark {
    ($fr:literal) => {
        $fr
    };
}

#[cfg(test)]
mod tests;
