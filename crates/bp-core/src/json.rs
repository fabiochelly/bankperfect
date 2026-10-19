//! Export et import d'un fichier de comptes complet en JSON.
//!
//! Format ouvert et lisible, indépendant du format interne v6 : il contient
//! tout ce que contient un `.bp` (comptes, opérations, référentiels, données
//! des fonctionnalités) et se réimporte sans perte.
//!
//! ```json
//! {
//!   "format": "bankperfect",
//!   "version": 1,
//!   "generator": "BankPerfect 9.8.2",
//!   "exported_at": "2026-10-12T04:50:00+02:00",
//!   "group_name": "Mes comptes",
//!   "accounts": [{ "name": "Courant", "operations": [{
//!     "uid": 42, "date": "2026-10-12", "mark_date": "2026-10-12",
//!     "tack": "pointed", "mode": "carte", "payee": "Boulangerie",
//!     "details": "", "category": 7, "amount": "-12.50"
//!   }] }],
//!   "features": [{ "key": "budget.v6", "data": { … } }]
//! }
//! ```
//!
//! - dates : `AAAA-MM-JJ` (un entier, nombre de jours depuis le 30/12/1899,
//!   est aussi accepté) ;
//! - montants : chaîne décimale exacte avec un point (`"-12.50"`, jusqu'à
//!   4 décimales) ; un nombre JSON est aussi accepté ;
//! - modes : `carte`, `retrait_dab`, `cheque_emis`, `prelevement`,
//!   `virement_emis`, `tip`, `versement`, `depot_cheque`, `virement_recu`,
//!   `titre`, ou `{"custom": "<nom>"}` pour un mode personnalisé ;
//! - pointage : `none`, `pointed` (P) ou `reconciled` (R) ;
//! - données des fonctionnalités : converties en JSON (`data`) quand la
//!   conversion est sans perte, sinon gardées telles quelles en base64
//!   (`base64`), par exemple les données d'anciens scripts d'un fichier v5.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use chrono::{Days, NaiveDate};
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};
use crate::model::{
    Account, Amount, Category, CustomMode, Date, Document, FeatureData, Mode, Operation, Payee,
    StandardMode, Tack,
};
use crate::tr;

pub const FORMAT: &str = "bankperfect";
/// Version du format JSON (indépendante de celle du format v6).
pub const VERSION: u32 = 1;

/// Sérialise un document complet en JSON indenté.
pub fn to_json(document: &Document) -> Result<Vec<u8>> {
    let file = JsonFile {
        format: FORMAT.to_string(),
        version: VERSION,
        generator: Some(format!("BankPerfect {}", env!("CARGO_PKG_VERSION"))),
        exported_at: Some(chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)),
        group_name: document.group_name.clone(),
        guid: document.guid.clone(),
        user_name: document.user_name.clone(),
        default_account: document.default_account,
        account_order: document.account_order.clone(),
        custom_modes: document
            .custom_modes
            .iter()
            .map(JsonCustomMode::from)
            .collect(),
        categories: document.categories.iter().map(JsonCategory::from).collect(),
        payees: document.payees.iter().map(JsonPayee::from).collect(),
        accounts: document.accounts.iter().map(JsonAccount::from).collect(),
        features: document
            .feature_data
            .iter()
            .map(JsonFeature::from)
            .collect(),
        unknown_header: document.unknown_header.clone(),
    };
    serde_json::to_vec_pretty(&file).map_err(|e| Error::Json(e.to_string()))
}

/// Relit un export JSON et vérifie sa cohérence.
pub fn from_json(data: &[u8]) -> Result<Document> {
    let probe: Probe = serde_json::from_slice(data).map_err(|e| {
        Error::Json(if e.is_syntax() || e.is_eof() {
            tr!("JSON invalide : {e}", e = e)
        } else {
            tr!("ce fichier n'est pas un export JSON de BankPerfect").into()
        })
    })?;
    if probe.format.as_deref() != Some(FORMAT) {
        return Err(Error::Json(
            tr!("ce fichier n'est pas un export JSON de BankPerfect").into(),
        ));
    }
    if probe.version.is_none_or(|v| v > VERSION) {
        return Err(Error::Json(tr!(
            "version {version} du format JSON non prise en charge (version {max} au plus)",
            version = probe
                .version
                .map_or(tr!("inconnue").to_string(), |v| v.to_string()),
            max = VERSION
        )));
    }
    let file: JsonFile = serde_json::from_slice(data).map_err(|e| Error::Json(e.to_string()))?;
    let document = file.into_document()?;
    validate(&document)?;
    Ok(document)
}

/// Contrôles d'un document importé : ce que le reste de l'application
/// suppose sans le vérifier.
fn validate(document: &Document) -> Result<()> {
    let count = document.accounts.len();
    let mut order = document.account_order.clone();
    order.sort_unstable();
    if order != (0..count).collect::<Vec<_>>() {
        return Err(Error::Json(tr!(
            "« account_order » doit contenir chaque compte une fois (0 à {})",
            count.saturating_sub(1)
        )));
    }
    if count > 0 && document.default_account >= count {
        return Err(Error::Json(tr!(
            "« default_account » : compte {} inexistant",
            document.default_account
        )));
    }
    let mut ids = std::collections::HashSet::new();
    for category in &document.categories {
        if !ids.insert(category.id) {
            return Err(Error::Json(tr!(
                "catégorie {} définie deux fois",
                category.id
            )));
        }
    }
    let mut uids = std::collections::HashSet::new();
    for (account, op) in document
        .accounts
        .iter()
        .flat_map(|a| a.operations.iter().map(move |op| (a, op)))
    {
        if op.uid == 0 || !uids.insert(op.uid) {
            return Err(Error::Json(if op.uid == 0 {
                tr!(
                    "compte « {} » : identifiant d'opération {} invalide",
                    account.name,
                    op.uid
                )
            } else {
                tr!(
                    "compte « {} » : identifiant d'opération {} en double",
                    account.name,
                    op.uid
                )
            }));
        }
    }
    let mut keys = std::collections::HashSet::new();
    for feature in &document.feature_data {
        if !keys.insert(feature.key.as_str()) {
            return Err(Error::Json(tr!(
                "donnée « {} » présente deux fois",
                feature.key
            )));
        }
    }
    Ok(())
}

// ─────────────────────────────── Structure ───────────────────────────────

#[derive(Deserialize)]
struct Probe {
    format: Option<String>,
    version: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct JsonFile {
    format: String,
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    generator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exported_at: Option<String>,
    #[serde(default)]
    group_name: String,
    #[serde(default)]
    guid: String,
    #[serde(default)]
    user_name: String,
    #[serde(default)]
    default_account: usize,
    /// Ordre d'affichage (indices dans `accounts`) ; l'ordre de `accounts`
    /// si absent.
    #[serde(default)]
    account_order: Vec<usize>,
    #[serde(default)]
    custom_modes: Vec<JsonCustomMode>,
    #[serde(default)]
    categories: Vec<JsonCategory>,
    #[serde(default)]
    payees: Vec<JsonPayee>,
    #[serde(default)]
    accounts: Vec<JsonAccount>,
    #[serde(default)]
    features: Vec<JsonFeature>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unknown_header: Vec<(String, String)>,
}

impl JsonFile {
    fn into_document(self) -> Result<Document> {
        let accounts = self
            .accounts
            .into_iter()
            .map(JsonAccount::into_account)
            .collect::<Result<Vec<_>>>()?;
        let account_order = if self.account_order.is_empty() {
            (0..accounts.len()).collect()
        } else {
            self.account_order
        };
        Ok(Document {
            group_name: self.group_name,
            guid: self.guid,
            user_name: self.user_name,
            custom_modes: self.custom_modes.into_iter().map(Into::into).collect(),
            accounts,
            account_order,
            default_account: self.default_account,
            categories: self.categories.into_iter().map(Into::into).collect(),
            payees: self
                .payees
                .into_iter()
                .map(JsonPayee::into_payee)
                .collect::<Result<_>>()?,
            feature_data: self
                .features
                .into_iter()
                .map(JsonFeature::into_feature)
                .collect::<Result<_>>()?,
            unknown_header: self.unknown_header,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct JsonCustomMode {
    name: String,
    /// Mode de dépense (montant négatif par défaut) ; sinon mode de recette.
    #[serde(default)]
    debit: bool,
}

impl From<&CustomMode> for JsonCustomMode {
    fn from(m: &CustomMode) -> Self {
        Self {
            name: m.name.clone(),
            debit: m.debit,
        }
    }
}

impl From<JsonCustomMode> for CustomMode {
    fn from(m: JsonCustomMode) -> Self {
        Self {
            name: m.name,
            debit: m.debit,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct JsonCategory {
    id: u32,
    name: String,
    /// Sous-catégorie de la dernière catégorie non enfant qui la précède.
    #[serde(default)]
    is_child: bool,
}

impl From<&Category> for JsonCategory {
    fn from(c: &Category) -> Self {
        Self {
            id: c.id,
            name: c.name.clone(),
            is_child: c.is_child,
        }
    }
}

impl From<JsonCategory> for Category {
    fn from(c: JsonCategory) -> Self {
        Self {
            id: c.id,
            name: c.name,
            is_child: c.is_child,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct JsonPayee {
    caption: String,
    #[serde(default)]
    mode: JsonMode,
    #[serde(default)]
    category: Option<u32>,
    #[serde(default)]
    payee: String,
    #[serde(default)]
    details: String,
    /// Montant par défaut, tel que saisi par l'utilisateur.
    #[serde(default)]
    amount: String,
}

impl From<&Payee> for JsonPayee {
    fn from(p: &Payee) -> Self {
        Self {
            caption: p.caption.clone(),
            mode: JsonMode::from(&p.mode),
            category: p.category,
            payee: p.payee.clone(),
            details: p.details.clone(),
            amount: p.amount.clone(),
        }
    }
}

impl JsonPayee {
    fn into_payee(self) -> Result<Payee> {
        Ok(Payee {
            mode: self.mode.into_mode()?,
            caption: self.caption,
            category: self.category,
            payee: self.payee,
            details: self.details,
            amount: self.amount,
        })
    }
}

fn default_rate() -> f64 {
    1.0
}

fn default_logo() -> String {
    "Aucun".into()
}

#[derive(Serialize, Deserialize)]
struct JsonAccount {
    #[serde(default)]
    uid: String,
    name: String,
    #[serde(default)]
    bank_name: String,
    #[serde(default)]
    agency_name: String,
    #[serde(default)]
    agency_address: String,
    #[serde(default)]
    number: String,
    #[serde(default)]
    bank_code: String,
    #[serde(default)]
    agency_code: String,
    #[serde(default)]
    bic: String,
    #[serde(default)]
    iban: String,
    #[serde(default = "default_logo")]
    logo: String,
    #[serde(default)]
    currency: String,
    #[serde(default = "default_rate")]
    currency_rate: f64,
    #[serde(default)]
    secondary_currency: String,
    #[serde(default = "default_rate")]
    secondary_rate: f64,
    #[serde(default)]
    is_virtual: bool,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    operations: Vec<JsonOperation>,
}

impl From<&Account> for JsonAccount {
    fn from(a: &Account) -> Self {
        Self {
            uid: a.uid.clone(),
            name: a.name.clone(),
            bank_name: a.bank_name.clone(),
            agency_name: a.agency_name.clone(),
            agency_address: a.agency_address.clone(),
            number: a.number.clone(),
            bank_code: a.bank_code.clone(),
            agency_code: a.agency_code.clone(),
            bic: a.bic.clone(),
            iban: a.iban.clone(),
            logo: a.logo.clone(),
            currency: a.currency.clone(),
            currency_rate: a.currency_rate,
            secondary_currency: a.secondary_currency.clone(),
            secondary_rate: a.secondary_rate,
            is_virtual: a.is_virtual,
            disabled: a.disabled,
            operations: a.operations.iter().map(JsonOperation::from).collect(),
        }
    }
}

impl JsonAccount {
    fn into_account(self) -> Result<Account> {
        let name = self.name.clone();
        let operations = self
            .operations
            .into_iter()
            .map(|op| {
                let uid = op.uid;
                op.into_operation().map_err(|e| match e {
                    Error::Json(message) => Error::Json(tr!(
                        "compte « {name} », opération {uid} : {message}",
                        name = name,
                        uid = uid,
                        message = message
                    )),
                    e => e,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Account {
            uid: self.uid,
            name: self.name,
            bank_name: self.bank_name,
            agency_name: self.agency_name,
            agency_address: self.agency_address,
            number: self.number,
            bank_code: self.bank_code,
            agency_code: self.agency_code,
            bic: self.bic,
            iban: self.iban,
            logo: self.logo,
            currency: self.currency,
            currency_rate: self.currency_rate,
            secondary_currency: self.secondary_currency,
            secondary_rate: self.secondary_rate,
            is_virtual: self.is_virtual,
            disabled: self.disabled,
            operations,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct JsonOperation {
    uid: u64,
    date: JsonDate,
    /// Date de pointage ; la date de l'opération si absente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mark_date: Option<JsonDate>,
    #[serde(default)]
    tack: JsonTack,
    #[serde(default)]
    mode: JsonMode,
    #[serde(default)]
    payee: String,
    #[serde(default)]
    details: String,
    #[serde(default)]
    category: Option<u32>,
    amount: JsonAmount,
    /// Champs supplémentaires : paires `[nom, valeur]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    extra: Vec<(String, String)>,
}

impl From<&Operation> for JsonOperation {
    fn from(op: &Operation) -> Self {
        Self {
            uid: op.uid,
            date: JsonDate(op.date),
            mark_date: Some(JsonDate(op.mark_date)),
            tack: JsonTack::from(op.tack),
            mode: JsonMode::from(&op.mode),
            payee: op.payee.clone(),
            details: op.details.clone(),
            category: op.category,
            amount: JsonAmount(op.amount),
            extra: op.extra.clone(),
        }
    }
}

impl JsonOperation {
    fn into_operation(self) -> Result<Operation> {
        Ok(Operation {
            uid: self.uid,
            date: self.date.0,
            mark_date: self.mark_date.map_or(self.date.0, |d| d.0),
            tack: self.tack.into(),
            mode: self.mode.into_mode()?,
            payee: self.payee,
            details: self.details,
            category: self.category,
            amount: self.amount.0,
            extra: self.extra,
        })
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JsonTack {
    #[default]
    None,
    Pointed,
    Reconciled,
}

impl From<Tack> for JsonTack {
    fn from(t: Tack) -> Self {
        match t {
            Tack::None => Self::None,
            Tack::Pointed => Self::Pointed,
            Tack::Reconciled => Self::Reconciled,
        }
    }
}

impl From<JsonTack> for Tack {
    fn from(t: JsonTack) -> Self {
        match t {
            JsonTack::None => Self::None,
            JsonTack::Pointed => Self::Pointed,
            JsonTack::Reconciled => Self::Reconciled,
        }
    }
}

/// Mode de paiement : identifiant d'un mode standard ou `{"custom": nom}`.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum JsonMode {
    Standard(String),
    Custom { custom: String },
}

impl Default for JsonMode {
    fn default() -> Self {
        JsonMode::from(&Mode::default())
    }
}

const STANDARD_IDS: [(StandardMode, &str); 10] = [
    (StandardMode::Carte, "carte"),
    (StandardMode::RetraitDab, "retrait_dab"),
    (StandardMode::ChequeEmis, "cheque_emis"),
    (StandardMode::Prelevement, "prelevement"),
    (StandardMode::VirementEmis, "virement_emis"),
    (StandardMode::Tip, "tip"),
    (StandardMode::Versement, "versement"),
    (StandardMode::DepotCheque, "depot_cheque"),
    (StandardMode::VirementRecu, "virement_recu"),
    (StandardMode::Titre, "titre"),
];

impl From<&Mode> for JsonMode {
    fn from(mode: &Mode) -> Self {
        match mode {
            Mode::Standard(m) => JsonMode::Standard(
                STANDARD_IDS
                    .iter()
                    .find(|(s, _)| s == m)
                    .map(|(_, id)| id.to_string())
                    .expect("mode standard listé"),
            ),
            Mode::Custom(name) => JsonMode::Custom {
                custom: name.clone(),
            },
        }
    }
}

impl JsonMode {
    fn into_mode(self) -> Result<Mode> {
        match self {
            JsonMode::Standard(id) => STANDARD_IDS
                .iter()
                .find(|(_, s)| *s == id)
                .map(|(m, _)| Mode::Standard(*m))
                .ok_or_else(|| {
                    Error::Json(tr!(
                        "mode « {id} » inconnu (modes personnalisés : {{\"custom\": \"{id}\"}})",
                        id = id
                    ))
                }),
            JsonMode::Custom { custom } => Ok(Mode::Custom(custom)),
        }
    }
}

/// Date `AAAA-MM-JJ`, ou entier (jours depuis le 30/12/1899) si elle sort du
/// calendrier représentable.
struct JsonDate(Date);

const EPOCH: NaiveDate = NaiveDate::from_ymd_opt(1899, 12, 30).expect("date valide");

impl Serialize for JsonDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let days = self.0.0;
        let date = if days >= 0 {
            EPOCH.checked_add_days(Days::new(days as u64))
        } else {
            EPOCH.checked_sub_days(Days::new(days.unsigned_abs() as u64))
        };
        match date {
            Some(date) => serializer.collect_str(&date.format("%Y-%m-%d")),
            None => serializer.serialize_i32(days),
        }
    }
}

impl<'de> Deserialize<'de> for JsonDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Serial(i32),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Serial(days) => Ok(JsonDate(Date(days))),
            Raw::Text(text) => NaiveDate::parse_from_str(&text, "%Y-%m-%d")
                .map(|d| JsonDate(Date::from_naive(d)))
                .map_err(|_| {
                    D::Error::custom(tr!(
                        "date « {text} » invalide (AAAA-MM-JJ attendu)",
                        text = text
                    ))
                }),
        }
    }
}

/// Montant exact : chaîne décimale avec un point, de 2 à 4 décimales.
struct JsonAmount(Amount);

impl Serialize for JsonAmount {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&format_amount(self.0))
    }
}

impl<'de> Deserialize<'de> for JsonAmount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Number(f64),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Text(text) => parse_amount(&text).map(JsonAmount).ok_or_else(|| {
                D::Error::custom(tr!(
                    "montant « {text} » invalide (ex. \"-12.50\")",
                    text = text
                ))
            }),
            Raw::Number(n) => {
                let scaled = (n * Amount::SCALE as f64).round();
                if scaled.is_finite() && scaled.abs() < i64::MAX as f64 {
                    Ok(JsonAmount(Amount(scaled as i64)))
                } else {
                    Err(D::Error::custom(tr!("montant {n} invalide", n = n)))
                }
            }
        }
    }
}

/// `-12.50`, `0.1234`, `1000.00` : au moins deux décimales, zéros de fin
/// retirés au-delà.
fn format_amount(amount: Amount) -> String {
    let text = amount.format(4, '.');
    let trimmed = text.trim_end_matches('0');
    let decimals = trimmed.len() - trimmed.find('.').expect("séparateur décimal") - 1;
    text[..text.len() - (4 - decimals.max(2))].to_string()
}

/// Analyse stricte : `-?chiffres(.1 à 4 chiffres)?`.
fn parse_amount(text: &str) -> Option<Amount> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));
    if int_part.is_empty()
        || !int_part.bytes().all(|b| b.is_ascii_digit())
        || frac_part.len() > 4
        || !frac_part.bytes().all(|b| b.is_ascii_digit())
        || (body.contains('.') && frac_part.is_empty())
    {
        return None;
    }
    let int: i64 = int_part.parse().ok()?;
    let frac: i64 = format!("{frac_part:0<4}").parse().ok()?;
    let value = int.checked_mul(Amount::SCALE)?.checked_add(frac)?;
    Some(Amount(if negative { -value } else { value }))
}

/// Données d'une fonctionnalité : `data` (JSON) ou `base64` (octets bruts).
#[derive(Serialize, Deserialize)]
struct JsonFeature {
    key: String,
    /// Valeur MessagePack, écrite et relue en JSON.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "msgpack_as_json",
        deserialize_with = "json_to_msgpack"
    )]
    data: Option<Vec<u8>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "bytes_as_base64",
        deserialize_with = "base64_to_bytes"
    )]
    base64: Option<Vec<u8>>,
}

impl From<&FeatureData> for JsonFeature {
    fn from(f: &FeatureData) -> Self {
        let lossless = msgpack_json_roundtrip(&f.value);
        Self {
            key: f.key.clone(),
            data: lossless.then(|| f.value.clone()),
            base64: (!lossless).then(|| f.value.clone()),
        }
    }
}

impl JsonFeature {
    fn into_feature(self) -> Result<FeatureData> {
        let value = match (self.data, self.base64) {
            (Some(value), None) | (None, Some(value)) => value,
            _ => {
                return Err(Error::Json(tr!(
                    "donnée « {} » : il faut soit « data », soit « base64 »",
                    self.key
                )));
            }
        };
        Ok(FeatureData {
            key: self.key,
            value,
        })
    }
}

/// Vrai si la valeur MessagePack passe en JSON et revient à l'identique
/// (pas d'octets bruts, de clés non textuelles, de flottants 32 bits ou NaN…).
fn msgpack_json_roundtrip(bytes: &[u8]) -> bool {
    let mut json = Vec::new();
    let to_json = serde_transcode::transcode(
        &mut rmp_serde::Deserializer::new(bytes),
        &mut serde_json::Serializer::new(&mut json),
    );
    if to_json.is_err() {
        return false;
    }
    let mut back = Vec::with_capacity(bytes.len());
    serde_transcode::transcode(
        &mut serde_json::Deserializer::from_slice(&json),
        &mut rmp_serde::Serializer::new(&mut back),
    )
    .is_ok()
        && back == bytes
}

fn msgpack_as_json<S: Serializer>(
    bytes: &Option<Vec<u8>>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    let bytes = bytes.as_deref().unwrap_or_default();
    serde_transcode::Transcoder::new(&mut rmp_serde::Deserializer::new(bytes))
        .serialize(serializer)
        .map_err(|e| S::Error::custom(e.to_string()))
}

fn json_to_msgpack<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Vec<u8>>, D::Error> {
    let mut bytes = Vec::new();
    serde_transcode::transcode(deserializer, &mut rmp_serde::Serializer::new(&mut bytes))
        .map_err(|e| D::Error::custom(e.to_string()))?;
    Ok(Some(bytes))
}

fn bytes_as_base64<S: Serializer>(
    bytes: &Option<Vec<u8>>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&BASE64.encode(bytes.as_deref().unwrap_or_default()))
}

fn base64_to_bytes<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Vec<u8>>, D::Error> {
    let text = String::deserialize(deserializer)?;
    BASE64
        .decode(text.trim())
        .map(Some)
        .map_err(|_| D::Error::custom(tr!("base64 invalide")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature_store;

    fn sample() -> Document {
        let mut doc = crate::manage::new_document("Mes comptes", Amount(1_234_500));
        doc.custom_modes.push(CustomMode {
            name: "Paypal".into(),
            debit: true,
        });
        let account = &mut doc.accounts[0];
        account.iban = "FR76 3000 6000 0112 3456 7890 189".into();
        account.currency_rate = 6.55957;
        let next = account.operations.iter().map(|o| o.uid).max().unwrap_or(0) + 1;
        account.operations.push(Operation {
            uid: next,
            date: Date::from_ymd(2026, 10, 12).unwrap(),
            mark_date: Date::from_ymd(2026, 10, 14).unwrap(),
            tack: Tack::Reconciled,
            mode: Mode::Custom("Paypal".into()),
            payee: "Boulangerie « Au pain d'or »".into(),
            details: "n°1234\nligne 2".into(),
            category: doc.categories.first().map(|c| c.id),
            amount: Amount(-125_001),
            extra: vec![
                ("Note".into(), "à rembourser".into()),
                ("Note".into(), "doublon".into()),
            ],
        });
        account.operations.push(Operation {
            uid: next + 1,
            date: Date(-700_000_000),
            mark_date: Date(0),
            tack: Tack::Pointed,
            mode: Mode::Standard(StandardMode::VirementRecu),
            payee: String::new(),
            details: String::new(),
            category: None,
            amount: Amount(i64::MIN + 1),
            extra: Vec::new(),
        });
        doc.payees.push(Payee {
            caption: "EDF".into(),
            mode: Mode::Standard(StandardMode::Prelevement),
            category: Some(3),
            payee: "EDF".into(),
            details: "Électricité".into(),
            amount: "-45,00".into(),
        });
        doc.unknown_header
            .push(("cle_inconnue".into(), "valeur".into()));
        doc
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Feature {
        name: String,
        values: Vec<i64>,
        rate: f64,
        nested: std::collections::BTreeMap<String, Option<u32>>,
    }

    #[derive(Serialize)]
    struct Opaque {
        ratio: f32,
        #[serde(with = "serde_bytes")]
        raw: Vec<u8>,
        by_id: std::collections::BTreeMap<u32, String>,
    }

    #[test]
    fn roundtrip_is_lossless() {
        let mut doc = sample();
        let feature = Feature {
            name: "budget".into(),
            values: vec![0, -1, 127, 128, 255, 256, -33, i64::MIN, i64::MAX],
            rate: 0.1,
            nested: [("a".to_string(), Some(5)), ("b".to_string(), None)].into(),
        };
        feature_store::set(&mut doc, "budget.v6", &feature).unwrap();
        feature_store::set(
            &mut doc,
            "opaque.v6",
            &Opaque {
                ratio: 0.5,
                raw: vec![0, 255],
                by_id: [(1, "x".to_string())].into(),
            },
        )
        .unwrap();
        doc.feature_data.push(FeatureData {
            key: "fabiobpbudget".into(),
            value: b"(dp0\nS'pickle'\n".to_vec(),
        });

        let json = to_json(&doc).unwrap();
        let back = from_json(&json).unwrap();
        assert_eq!(back, doc);

        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        let features = value["features"].as_array().unwrap();
        assert_eq!(features[0]["data"]["name"], "budget");
        assert!(features[1].get("data").is_none() && features[1].get("base64").is_some());
        assert!(features[2].get("base64").is_some());
        let op = &value["accounts"][0]["operations"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .nth(1)
            .unwrap();
        assert_eq!(op["date"], "2026-10-12");
        assert_eq!(op["amount"], "-12.5001");
        assert_eq!(op["mode"]["custom"], "Paypal");
        assert_eq!(op["tack"], "reconciled");
    }

    #[test]
    fn amounts() {
        for (amount, text) in [
            (0, "0.00"),
            (-125_000, "-12.50"),
            (1234, "0.1234"),
            (10_000_000, "1000.00"),
            (-5, "-0.0005"),
        ] {
            assert_eq!(format_amount(Amount(amount)), text);
            assert_eq!(parse_amount(text), Some(Amount(amount)));
        }
        for bad in ["", "-", "1,5", "1.", ".5", "1.23456", "1e3", "+1", " 1"] {
            assert_eq!(parse_amount(bad), None, "{bad}");
        }
    }

    #[test]
    fn minimal_hand_written_file() {
        let json = br#"{
            "format": "bankperfect", "version": 1,
            "categories": [{"id": 1, "name": "Courses"}],
            "accounts": [{"name": "Courant", "operations": [
                {"uid": 1, "date": "2026-01-02", "amount": -3.5, "category": 1},
                {"uid": 2, "date": 46000, "amount": "10", "mode": "virement_recu"}
            ]}]
        }"#;
        let doc = from_json(json).unwrap();
        let ops = &doc.accounts[0].operations;
        assert_eq!(ops[0].amount, Amount(-35_000));
        assert_eq!(ops[0].mark_date, ops[0].date);
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::Carte));
        assert_eq!(ops[1].date, Date(46000));
        assert_eq!(doc.account_order, vec![0]);
        assert_eq!(doc.accounts[0].logo, "Aucun");
    }

    #[test]
    fn rejects_invalid_files() {
        let error = |json: &str| from_json(json.as_bytes()).unwrap_err().to_string();
        assert!(error("[]").contains("pas un export"));
        assert!(error(r#"{"format": "autre", "version": 1}"#).contains("pas un export"));
        assert!(error(r#"{"format": "bankperfect", "version": 2}"#).contains("version 2"));
        let account = |ops: &str| {
            format!(
                r#"{{"format": "bankperfect", "version": 1, "accounts": [{{"name": "C", "operations": [{ops}]}}]}}"#
            )
        };
        assert!(
            error(&account(
                r#"{"uid": 1, "date": "2026-02-30", "amount": "1"}"#
            ))
            .contains("2026-02-30")
        );
        assert!(
            error(&account(
                r#"{"uid": 1, "date": "2026-02-03", "amount": "1,5"}"#
            ))
            .contains("1,5")
        );
        assert!(
            error(&account(
                r#"{"uid": 1, "date": "2026-02-03", "amount": "1", "mode": "cb"}"#
            ))
            .contains("cb")
        );
        assert!(error(&account(
            r#"{"uid": 1, "date": "2026-02-03", "amount": "1"}, {"uid": 1, "date": "2026-02-03", "amount": "1"}"#
        ))
        .contains("en double"));
        assert!(error(r#"{"format": "bankperfect", "version": 1, "accounts": [{"name": "C"}], "account_order": [1]}"#)
            .contains("account_order"));
        assert!(
            error(r#"{"format": "bankperfect", "version": 1, "features": [{"key": "x"}]}"#)
                .contains("« data »")
        );
    }
}
