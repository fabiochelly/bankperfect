//! Analyse du contenu texte d'un fichier v5 (une fois décodé par `codec_v5`).
//!
//! Suit `docs/inventaire/02-format-fichier-bp.md`, en corrigeant les bugs de
//! lecture de l'ancienne version (voir `docs/corrections.md`).

use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::model::*;

const FILE_SEPARATOR: &str = "TNSLPPBNTSOGPTQBC280673";
const SCRIPT_DATA_PREFIX: &str = "bp_script_data_";
const DEFAULT_RATE1: f64 = 1.0;
const DEFAULT_RATE2: f64 = 6.55957;

/// Résultat de l'analyse d'un fichier v5.
pub struct ParsedV5 {
    pub document: Document,
    /// Mot de passe historique (stocké en clair, en minuscules), vide si aucun.
    pub password: String,
    /// Anomalies rencontrées et corrigées, à signaler à l'utilisateur.
    pub warnings: Vec<String>,
}

pub fn parse(text: &str) -> Result<ParsedV5> {
    let Some(sep) = text.find(FILE_SEPARATOR) else {
        return Err(Error::InvalidV5);
    };
    let header = Header::parse(&text[..sep]);
    let blocks: Vec<&str> = text[sep + FILE_SEPARATOR.len()..]
        .split(FILE_SEPARATOR)
        .collect();

    let mut warnings = Vec::new();
    let custom_modes = parse_modes(header.get("modes"));
    let categories = parse_categories(header.get("categ"));
    let mut accounts = parse_accounts(&header);
    let mut parser = OperationParser {
        custom_modes,
        category_ids: categories.iter().map(|c| c.id).collect(),
        warnings: &mut warnings,
    };

    if blocks.len() > accounts.len() {
        parser.warnings.push(format!(
            "{} bloc(s) d'opérations sans compte correspondant ont été ignorés",
            blocks.len() - accounts.len()
        ));
    }
    // Un compte sans bloc (dernier compte vide) garde une liste vide : l'ancienne
    // version le perdait au rechargement.
    for (account, block) in accounts.iter_mut().zip(&blocks) {
        account.operations = lines(block)
            .filter_map(|line| parser.parse_line(line))
            .collect();
    }

    let payees = parse_payees(&header, &parser.custom_modes, &parser.category_ids);
    let custom_modes = parser.custom_modes;
    assign_missing_uids(&mut accounts, &mut warnings);

    let account_order = parse_account_order(header.get("accounts_order"), accounts.len());
    let default_account = header
        .get("compte_defaut")
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&i| i < accounts.len())
        .unwrap_or(0);

    let guid = match header.get("guid") {
        "" => new_file_guid(),
        guid => guid.to_string(),
    };

    let document = Document {
        group_name: header.get("utilisateur").to_string(),
        guid,
        user_name: header.get("UserName").to_string(),
        custom_modes,
        accounts,
        account_order,
        default_account,
        categories,
        payees,
        feature_data: header.feature_data(),
        unknown_header: header.unknown(),
    };
    Ok(ParsedV5 {
        document,
        password: header.get("pass").to_string(),
        warnings,
    })
}

/// Découpe sur CR, LF ou CRLF en ignorant les lignes vides (`TStrings.Text`).
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split(['\r', '\n']).filter(|l| !l.is_empty())
}

/// En-tête `clé=valeur` avec la sémantique de `TStringList.Values` : nom avant
/// le premier `=`, comparaison insensible à la casse, la première ligne gagne.
struct Header<'a> {
    entries: Vec<(&'a str, &'a str)>,
}

impl<'a> Header<'a> {
    fn parse(text: &'a str) -> Self {
        let entries = lines(text)
            .filter_map(|line| line.split_once('='))
            .collect();
        Self { entries }
    }

    fn find(&self, name: &str) -> Option<&'a str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }

    fn get(&self, name: &str) -> &'a str {
        self.find(name).unwrap_or("")
    }

    fn feature_data(&self) -> Vec<FeatureData> {
        self.entries
            .iter()
            .filter_map(|(k, v)| {
                let key = k.strip_prefix(SCRIPT_DATA_PREFIX)?;
                Some(FeatureData {
                    key: key.to_string(),
                    value: simple_decode(v),
                })
            })
            .collect()
    }

    fn unknown(&self) -> Vec<(String, String)> {
        const KNOWN: [&str; 8] = [
            "utilisateur",
            "guid",
            "username",
            "modes",
            "categ",
            "accounts_order",
            "compte_defaut",
            "pass",
        ];
        let numbered = |k: &str, prefix: &str| {
            k.len() > prefix.len()
                && k[..prefix.len()].eq_ignore_ascii_case(prefix)
                && k[prefix.len()..].bytes().all(|b| b.is_ascii_digit())
        };
        self.entries
            .iter()
            .filter(|(k, _)| {
                !KNOWN.iter().any(|known| k.eq_ignore_ascii_case(known))
                    && !numbered(k, "compte")
                    && !numbered(k, "who")
                    && !k.starts_with(SCRIPT_DATA_PREFIX)
            })
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

/// Décodage `SimpleDecode` : `_hh` → octet.
fn simple_decode(value: &str) -> Vec<u8> {
    let bytes = encoding_rs::WINDOWS_1252.encode(value).0;
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'_'
            && i + 2 < bytes.len()
            && let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn parse_modes(value: &str) -> Vec<CustomMode> {
    let mut modes: Vec<CustomMode> = Vec::new();
    for item in value.split(';').filter(|s| !s.trim().is_empty()) {
        if modes
            .iter()
            .any(|m| m.raw().to_lowercase() == item.to_lowercase())
        {
            continue;
        }
        modes.push(match item.strip_prefix('-') {
            Some(name) => CustomMode {
                name: name.to_string(),
                debit: true,
            },
            None => CustomMode {
                name: item.to_string(),
                debit: false,
            },
        });
    }
    modes
}

impl CustomMode {
    fn raw(&self) -> String {
        if self.debit {
            format!("-{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

fn parse_categories(value: &str) -> Vec<Category> {
    let mut categories: Vec<Category> = Vec::new();
    if value.is_empty() {
        return categories;
    }
    for (position, item) in value.split(';').enumerate() {
        let (id, name) = match item.split_once('=') {
            Some((id, name)) => {
                let id = id.trim().parse::<u32>().unwrap_or(position as u32);
                (id, name)
            }
            None => (position as u32, item),
        };
        if name.trim().is_empty() {
            continue;
        }
        // Le premier élément ne peut pas être une sous-catégorie (`UpdateCategs`).
        let is_child = name.starts_with(' ') && !categories.is_empty();
        categories.push(Category {
            id,
            name: name.trim_start().to_string(),
            is_child,
        });
    }
    categories
}

fn parse_accounts(header: &Header<'_>) -> Vec<Account> {
    let mut accounts = Vec::new();
    while let Some(value) = header.find(&format!("compte{}", accounts.len())) {
        let fields: Vec<&str> = value.split(';').collect();
        let field = |i: usize| fields.get(i).copied().unwrap_or("").to_string();
        let rate = |i: usize, default: f64| {
            fields
                .get(i)
                .and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok())
                .unwrap_or(default)
        };
        let index = accounts.len();
        accounts.push(Account {
            uid: match field(15) {
                uid if uid.is_empty() => new_account_uid(index),
                uid => uid,
            },
            name: field(3),
            bank_name: field(0),
            agency_name: field(1),
            agency_address: field(2),
            number: field(4),
            bank_code: field(9),
            agency_code: field(10),
            bic: field(11),
            iban: field(13),
            logo: field(12),
            currency: field(6),
            currency_rate: rate(5, DEFAULT_RATE1),
            secondary_currency: field(8),
            secondary_rate: rate(7, DEFAULT_RATE2),
            is_virtual: field(14) == "virtual",
            disabled: field(16) == "disabled",
            operations: Vec::new(),
        });
    }
    accounts
}

fn parse_payees(
    header: &Header<'_>,
    custom_modes: &[CustomMode],
    category_ids: &HashSet<u32>,
) -> Vec<Payee> {
    let mut payees = Vec::new();
    loop {
        let value = header.get(&format!("who{}", payees.len()));
        if value.is_empty() {
            return payees;
        }
        // L'ancienne version déclarait le fichier corrompu s'il manquait un champ ;
        // les champs manquants prennent ici leur valeur par défaut.
        let fields: Vec<&str> = value.split(';').collect();
        let field = |i: usize| fields.get(i).copied().unwrap_or("").to_string();
        let mode_index = fields
            .get(1)
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let mode = match StandardMode::ALL.get(mode_index) {
            Some(&m) => Mode::Standard(m),
            None => custom_modes
                .get(mode_index - StandardMode::ALL.len())
                .map(|m| Mode::Custom(m.name.clone()))
                .unwrap_or_default(),
        };
        let category = fields
            .get(2)
            .and_then(|s| s.trim().parse::<u32>().ok())
            .filter(|id| category_ids.contains(id));
        payees.push(Payee {
            caption: field(0),
            mode,
            category,
            payee: field(3),
            details: field(4),
            amount: field(5),
        });
    }
}

fn parse_account_order(value: &str, count: usize) -> Vec<usize> {
    let mut order = Vec::with_capacity(count);
    for index in value
        .split(';')
        .filter_map(|s| s.trim().parse::<usize>().ok())
    {
        if index < count && !order.contains(&index) {
            order.push(index);
        }
    }
    order.extend(
        (0..count)
            .filter(|i| !order.contains(i))
            .collect::<Vec<_>>(),
    );
    order
}

struct OperationParser<'w> {
    custom_modes: Vec<CustomMode>,
    category_ids: HashSet<u32>,
    warnings: &'w mut Vec<String>,
}

impl OperationParser<'_> {
    fn parse_line(&mut self, line: &str) -> Option<Operation> {
        let fields: Vec<&str> = line.split(';').collect();
        if !(7..=9).contains(&fields.len()) {
            self.warnings.push(format!(
                "ligne d'opération illisible ignorée : {} champs",
                fields.len()
            ));
            return None;
        }

        let date = match fields[0].trim().parse::<i32>() {
            Ok(n) => Date(n),
            Err(_) => {
                self.warnings.push(format!(
                    "date invalide « {} » remplacée par la date du jour",
                    fields[0]
                ));
                Date::today()
            }
        };
        let value_date = fields
            .get(7)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .map(Date)
            .unwrap_or(date);
        let tack = match fields[1] {
            "P" | "1" => Tack::Pointed,
            "R" => Tack::Reconciled,
            _ => Tack::None,
        };
        let amount = match fields[6] {
            "" => Amount::ZERO,
            text => Amount::parse(text).unwrap_or_else(|| {
                self.warnings
                    .push(format!("montant invalide « {text} » remplacé par 0"));
                Amount::ZERO
            }),
        };
        let category = fields[5]
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|id| self.category_ids.contains(id));
        let mode = self.parse_mode(fields[2], amount);

        let mut uid = 0;
        let mut extra = Vec::new();
        if let Some(raw) = fields.get(8).filter(|s| !s.is_empty()) {
            for entry in raw
                .replace("<||>", ";")
                .split(';')
                .filter(|e| !e.is_empty())
            {
                let (key, value) = entry.split_once('=').unwrap_or((entry, ""));
                if key == "UId" {
                    uid = value.trim().parse().unwrap_or(0);
                } else {
                    extra.push((key.to_string(), unescape_extra(value)));
                }
            }
        }

        Some(Operation {
            uid,
            date,
            value_date,
            tack,
            mode,
            payee: fields[3].to_string(),
            details: fields[4].to_string(),
            category,
            amount,
            extra,
        })
    }

    /// `mode_paiement` : code numérique ou libellé en clair.
    fn parse_mode(&mut self, field: &str, amount: Amount) -> Mode {
        let Ok(code) = field.trim().parse::<i32>() else {
            if field.starts_with("Chq") {
                return Mode::Standard(StandardMode::ChequeEmis);
            }
            if let Some(standard) = StandardMode::from_label(field) {
                return Mode::Standard(standard);
            }
            // Mode personnalisé supprimé depuis : l'ancienne version le
            // transformait en « Carte » ; on le recrée pour ne rien perdre.
            if !self
                .custom_modes
                .iter()
                .any(|m| m.name.to_lowercase() == field.to_lowercase())
            {
                self.warnings.push(format!(
                    "mode de paiement « {field} » absent de la liste : recréé"
                ));
                self.custom_modes.push(CustomMode {
                    name: field.to_string(),
                    debit: amount.is_negative(),
                });
            }
            return Mode::Custom(field.to_string());
        };
        let standard = |index: usize| Mode::Standard(StandardMode::ALL[index]);
        match code {
            21..=29 => standard((code - 20) as usize),
            40.. => {
                let index = (code - 34) as usize;
                if index < StandardMode::ALL.len() {
                    standard(index)
                } else {
                    // Format ancien : mode personnalisé désigné par sa position.
                    self.custom_modes
                        .get(index - StandardMode::ALL.len())
                        .map(|m| Mode::Custom(m.name.clone()))
                        .unwrap_or_default()
                }
            }
            _ => Mode::default(),
        }
    }
}

/// Décodage des valeurs de champs supplémentaires (`GetExtraValue`).
fn unescape_extra(value: &str) -> String {
    value.replace("<fab_pv>", ";").replace("<fab_cr>", "\n")
}

/// Attribue un UId aux opérations qui n'en ont pas (ou en double).
///
/// L'ancienne version renumérotait alors TOUTES les lignes et effaçait les
/// liens de transfert (`#sisterLineUid`) ; ici les UId existants sont gardés.
fn assign_missing_uids(accounts: &mut [Account], warnings: &mut Vec<String>) {
    let mut next = accounts
        .iter()
        .flat_map(|a| &a.operations)
        .map(|op| op.uid)
        .max()
        .unwrap_or(0)
        + 1;
    let mut seen = HashSet::new();
    let mut assigned = 0;
    for op in accounts.iter_mut().flat_map(|a| &mut a.operations) {
        if op.uid == 0 || !seen.insert(op.uid) {
            op.uid = next;
            seen.insert(next);
            next += 1;
            assigned += 1;
        }
    }
    if assigned > 0 {
        warnings.push(format!(
            "{assigned} opération(s) sans identifiant ont reçu un nouvel identifiant"
        ));
    }
}

const UID_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// `CreateAccountUID` : `aaaammjjhhmmss-<index>-<5 caractères aléatoires>`.
pub fn new_account_uid(index: usize) -> String {
    let mut random = [0u8; 5];
    let _ = getrandom::fill(&mut random);
    let suffix: String = random
        .iter()
        .map(|b| UID_CHARS[*b as usize % UID_CHARS.len()] as char)
        .collect();
    format!(
        "{}-{index}-{suffix}",
        chrono::Local::now().format("%Y%m%d%H%M%S")
    )
}

/// Identifiant de fichier au format GUID Windows (`{XXXXXXXX-XXXX-…}`).
pub fn new_file_guid() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02X}")).collect();
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "utilisateur=Fabio\n\
guid={35F83405-0CDC-4DC8-8F71-6530905AE342}\n\
UserName=Fabio\n\
modes=-Carte FNAC;Espèces;;-carte fnac\n\
compte0=Test;Agence;Adresse;Courant;1234567890A;1;€;6,5596;F;12345;67890;BIC;CIC;FR78 1234 ;real;20140910070720-0-VkqQU\n\
compte1=SG;;;Livret;0002;1;€;6,55957;F;;;;LCL;FR76 ;virtual;uid-1;disabled\n\
compte2=;;;Vide;;1;€;6,55957;F;;;;;;real;uid-2;\n\
categ=0=Alimentation;1=     Epicerie;48=     Restaurant;3=Décaissement;5=\n\
accounts_order=0;2;1\n\
who0=Coin;7;5;Coin;;500\n\
who1=Fnac;10;1;Totor;test\n\
compte_defaut=1\n\
pass=secret\n\
bp_script_data_fabiobpbudget=(dp1_0aI24_0aF-12.16_0as.\n\
cle_inconnue=valeur\n\
TNSLPPBNTSOGPTQBC28067339444;;22;hyper u ;n°3104512;0;-1,78;39445;UId=1\n\
39445;P;Carte FNAC;Fnac;;1;-10,09;39445;UId=2<||>#sisterAccUid=uid-1<||>note=a<fab_pv>b\n\
39446;R;Mode disparu;X;;999;12;39446;UId=2\n\
39447;;43\n\
TNSLPPBNTSOGPTQBC28067340119;R;42;GF;;-1;600,00\n";

    #[test]
    fn parses_sample() {
        let parsed = parse(SAMPLE).unwrap();
        let doc = &parsed.document;
        assert_eq!(doc.group_name, "Fabio");
        assert_eq!(parsed.password, "secret");
        assert_eq!(doc.accounts.len(), 3);
        assert_eq!(doc.account_order, [0, 2, 1]);
        assert_eq!(doc.default_account, 1);

        let a0 = &doc.accounts[0];
        assert_eq!(
            (
                a0.name.as_str(),
                a0.secondary_rate,
                a0.is_virtual,
                a0.disabled
            ),
            ("Courant", 6.5596, false, false)
        );
        let a1 = &doc.accounts[1];
        assert!(a1.is_virtual && a1.disabled);

        // Modes : vide et doublon (casse ignorée) supprimés, mode disparu recréé.
        let names: Vec<_> = doc
            .custom_modes
            .iter()
            .map(|m| (m.name.as_str(), m.debit))
            .collect();
        assert_eq!(
            names,
            [
                ("Carte FNAC", true),
                ("Espèces", false),
                ("Mode disparu", false)
            ]
        );

        // Catégories : nom vide ignoré, hiérarchie.
        assert_eq!(doc.categories.len(), 4);
        assert_eq!(
            doc.category_label(48).as_deref(),
            Some("Alimentation > Restaurant")
        );

        let ops = &a0.operations;
        assert_eq!(ops.len(), 3, "la ligne à 3 champs est ignorée");
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::ChequeEmis));
        assert_eq!(ops[0].cheque_number(), Some("3104512"));
        assert_eq!(ops[0].amount, Amount(-17_800));
        assert_eq!(ops[0].value_date, Date(39445));
        assert_eq!(ops[1].mode, Mode::Custom("Carte FNAC".into()));
        assert_eq!(ops[1].tack, Tack::Pointed);
        assert_eq!(ops[1].extra("#sisterAccUid"), Some("uid-1"));
        assert_eq!(ops[1].extra("note"), Some("a;b"));
        assert_eq!(ops[2].category, None, "catégorie inexistante");
        assert_eq!(ops[2].tack, Tack::Reconciled);

        // UId en double et UId manquant : nouveaux identifiants, les autres gardés.
        assert_eq!((ops[0].uid, ops[1].uid, ops[2].uid), (1, 2, 3));
        let a1_op = &doc.accounts[1].operations[0];
        assert_eq!((a1_op.uid, a1_op.value_date), (4, Date(40119)));
        assert!(
            doc.accounts[2].operations.is_empty(),
            "dernier compte vide conservé"
        );

        assert_eq!(
            doc.payees[0].mode,
            Mode::Standard(StandardMode::DepotCheque)
        );
        assert_eq!(doc.payees[1].mode, Mode::Custom("Carte FNAC".into()));
        assert_eq!(doc.payees[1].amount, "", "champ manquant toléré");

        assert_eq!(doc.feature_data[0].key, "fabiobpbudget");
        assert_eq!(doc.feature_data[0].value, b"(dp1\nI24\nF-12.16\ns.");
        assert_eq!(
            doc.unknown_header,
            [("cle_inconnue".to_string(), "valeur".to_string())]
        );
    }

    #[test]
    fn missing_separator_is_invalid() {
        assert!(matches!(parse("utilisateur=x\n"), Err(Error::InvalidV5)));
    }

    #[test]
    fn generated_identifiers() {
        let guid = new_file_guid();
        assert_eq!(guid.len(), 38);
        assert!(guid.starts_with('{') && guid.ends_with('}'));
        assert_eq!(new_account_uid(3).split('-').nth(1), Some("3"));
    }
}
