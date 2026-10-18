//! Synchronisation bancaire par Enable Banking (agrégateur agréé DSP2) :
//! modèle des connexions, lecture des réponses de l'API et conversion des
//! opérations au format de l'import (aperçu, doublons, règles).
//!
//! Le réseau et la signature des requêtes sont dans l'application ; ce module
//! ne fait que des conversions, testables sans connexion.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::import::{ImportedAccount, ImportedOperation, StatementBalance, guess_mode_from_field};
use crate::model::{Amount, Date, Document, Mode, StandardMode};

/// Adresse de l'API.
pub const API_URL: &str = "https://api.enablebanking.com";
/// Clé des connexions dans le fichier de comptes.
pub const FEATURE_KEY: &str = "bank_sync.v6";
/// Historique téléchargé lors de la première synchronisation d'un compte.
pub const FIRST_SYNC_DAYS: i32 = 90;
/// Recouvrement avec la dernière opération du compte (les doublons sont
/// écartés par l'aperçu d'import).
pub const OVERLAP_DAYS: i32 = 10;

/// Compte bancaire d'une connexion, et son compte BankPerfect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankAccountLink {
    /// Identifiant du compte chez Enable Banking.
    pub uid: String,
    pub name: String,
    #[serde(default)]
    pub iban: String,
    #[serde(default)]
    pub currency: String,
    /// `Account::uid` du compte BankPerfect, `None` : non importé.
    #[serde(default)]
    pub target: Option<String>,
}

/// Connexion à une banque (consentement de l'utilisateur, limité dans le
/// temps).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankConnection {
    pub session_id: String,
    pub bank: String,
    pub country: String,
    /// Fin du consentement (RFC 3339).
    pub valid_until: String,
    pub accounts: Vec<BankAccountLink>,
}

/// Connexions enregistrées dans le fichier de comptes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankSync {
    pub connections: Vec<BankConnection>,
}

impl BankSync {
    pub fn load(document: &Document) -> Self {
        crate::feature_store::get(document, FEATURE_KEY)
            .and_then(Result::ok)
            .unwrap_or_default()
    }

    pub fn save(&self, document: &mut Document) -> Result<(), crate::feature_store::FeatureError> {
        crate::feature_store::set(document, FEATURE_KEY, self)
    }
}

impl BankConnection {
    /// Le consentement est-il expiré à la date `today` ?
    pub fn expired(&self, today: Date) -> bool {
        valid_until_date(&self.valid_until).is_some_and(|d| d < today)
    }
}

/// Date (jour) d'un horodatage RFC 3339.
pub fn valid_until_date(text: &str) -> Option<Date> {
    parse_iso_date(text.get(..10)?)
}

/// `AAAA-MM-JJ` → date.
pub fn parse_iso_date(text: &str) -> Option<Date> {
    let d = chrono::NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()?;
    Some(Date::from_naive(d))
}

/// Date de début du téléchargement pour un compte BankPerfect : quelques
/// jours avant sa dernière opération, ou `FIRST_SYNC_DAYS` jours en arrière
/// pour un compte vide.
pub fn sync_start(document: &Document, account: usize, today: Date) -> Date {
    let latest = document.accounts.get(account).and_then(|a| {
        a.operations
            .iter()
            .map(|o| o.date)
            .filter(|d| *d <= today)
            .max()
    });
    match latest {
        Some(d) => Date(d.0 - OVERLAP_DAYS),
        None => Date(today.0 - FIRST_SYNC_DAYS),
    }
}

/// Index du compte BankPerfect d'un lien.
pub fn target_index(document: &Document, link: &BankAccountLink) -> Option<usize> {
    let uid = link.target.as_deref()?;
    document.accounts.iter().position(|a| a.uid == uid)
}

/// Compte BankPerfect proposé pour un compte bancaire : même IBAN (sans
/// espaces), sinon aucun.
pub fn suggest_target(document: &Document, iban: &str) -> Option<String> {
    let wanted: String = iban
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    if wanted.is_empty() {
        return None;
    }
    document
        .accounts
        .iter()
        .find(|a| {
            let own: String = a
                .iban
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_uppercase();
            !own.is_empty() && own == wanted
        })
        .map(|a| a.uid.clone())
}

/// Code d'autorisation dans l'adresse de retour collée par l'utilisateur
/// (ou le code lui-même). Vérifie `state` quand il est présent.
pub fn authorization_code(pasted: &str, expected_state: &str) -> Result<String, String> {
    let text = pasted.trim();
    if text.is_empty() {
        return Err("collez l'adresse de la page de retour".into());
    }
    let Some(query) = text.split_once('?').map(|(_, q)| q) else {
        // Le code seul.
        return if text.contains(char::is_whitespace) || text.contains('/') {
            Err("adresse ou code non reconnu".into())
        } else {
            Ok(text.to_string())
        };
    };
    let query = query.split('#').next().unwrap_or("");
    let param = |name: &str| {
        query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == name)
            .map(|(_, v)| percent_decode(v))
    };
    if let Some(error) = param("error") {
        let description = param("error_description").unwrap_or_default();
        return Err(
            format!("la banque a refusé l'accès ({error}) {description}")
                .trim()
                .to_string(),
        );
    }
    if let Some(state) = param("state")
        && !expected_state.is_empty()
        && state != expected_state
    {
        return Err("cette adresse ne correspond pas à la connexion en cours".into());
    }
    param("code")
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "aucun code dans l'adresse collée".into())
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                match (
                    bytes.get(i + 1).copied().and_then(hex),
                    bytes.get(i + 2).copied().and_then(hex),
                ) {
                    (Some(h), Some(l)) => {
                        out.push(h * 16 + l);
                        i += 3;
                        continue;
                    }
                    _ => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Banque proposée par l'agrégateur.
#[derive(Debug, Clone, PartialEq)]
pub struct Bank {
    pub name: String,
    pub country: String,
    /// Durée maximale du consentement, en secondes.
    pub max_consent_seconds: Option<i64>,
}

/// Liste des banques (`GET /aspsps`).
pub fn parse_banks(json: &Value) -> Vec<Bank> {
    let mut banks: Vec<Bank> = json
        .get("aspsps")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|b| {
                    Some(Bank {
                        name: b.get("name")?.as_str()?.to_string(),
                        country: b.get("country")?.as_str()?.to_string(),
                        max_consent_seconds: b
                            .get("maximum_consent_validity")
                            .and_then(Value::as_i64),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    banks.sort_by_key(|b| b.name.to_lowercase());
    banks
}

/// Comptes d'une session (`POST /sessions`).
pub fn parse_session_accounts(json: &Value, document: &Document) -> Vec<BankAccountLink> {
    json.get("accounts")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let uid = a.get("uid")?.as_str()?.to_string();
                    let iban = a
                        .get("account_id")
                        .and_then(|id| id.get("iban"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let name = ["name", "product", "details"]
                        .iter()
                        .find_map(|k| {
                            a.get(*k)
                                .and_then(Value::as_str)
                                .filter(|s| !s.trim().is_empty())
                        })
                        .unwrap_or("Compte")
                        .to_string();
                    Some(BankAccountLink {
                        target: suggest_target(document, &iban),
                        uid,
                        name,
                        iban,
                        currency: a
                            .get("currency")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Opérations comptabilisées d'une page de `GET /accounts/{uid}/transactions`
/// (les opérations en attente sont ignorées) et clé de la page suivante.
pub fn parse_transactions(json: &Value) -> (Vec<ImportedOperation>, Option<String>) {
    let operations = json
        .get("transactions")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(transaction).collect())
        .unwrap_or_default();
    let next = json
        .get("continuation_key")
        .and_then(Value::as_str)
        .filter(|k| !k.is_empty())
        .map(str::to_string);
    (operations, next)
}

fn text_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn transaction(t: &Value) -> Option<ImportedOperation> {
    if text_at(t, &["status"]).is_some_and(|s| s != "BOOK") {
        return None;
    }
    let date = ["booking_date", "value_date", "transaction_date"]
        .iter()
        .find_map(|k| text_at(t, &[k]).and_then(parse_iso_date))?;
    let value_date = text_at(t, &["value_date"]).and_then(parse_iso_date);
    let raw = text_at(t, &["transaction_amount", "amount"])?;
    let mut amount = Amount::parse(&raw.replace('.', ","))?.abs();
    let debit = text_at(t, &["credit_debit_indicator"]) == Some("DBIT") || raw.starts_with('-');
    if debit {
        amount = Amount(-amount.0);
    }
    let remittance: Vec<String> = t
        .get("remittance_information")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let counterpart = if debit { "creditor" } else { "debtor" };
    let payee = text_at(t, &[counterpart, "name"])
        .map(str::to_string)
        .or_else(|| remittance.first().cloned())
        .unwrap_or_default();
    let details: Vec<&str> = remittance
        .iter()
        .map(String::as_str)
        .filter(|s| *s != payee)
        .collect();
    let code = text_at(t, &["bank_transaction_code", "description"]).unwrap_or_default();
    let (mode, cheque_number) = guess_mode_from_field(code, debit)
        .map(|(m, n)| (Mode::Standard(m), n))
        .unwrap_or_else(|| {
            let fallback = if debit {
                StandardMode::Carte
            } else {
                StandardMode::VirementRecu
            };
            (Mode::Standard(fallback), None)
        });
    let fitid = ["entry_reference", "transaction_id"]
        .iter()
        .find_map(|k| text_at(t, &[k]))
        .map(str::to_string);
    Some(ImportedOperation {
        line: 0,
        date,
        value_date,
        amount,
        mode,
        cheque_number,
        payee,
        details: details.join(" "),
        category: None,
        subcategory: None,
        tack: None,
        fitid,
        merchant_code: text_at(t, &["merchant_category_code"]).and_then(|c| c.trim().parse().ok()),
    })
}

/// Solde le plus utile de `GET /accounts/{uid}/balances` : de préférence
/// comptable (il correspond aux opérations comptabilisées, les seules
/// importées), le plus récent ; à défaut, un solde disponible.
pub fn parse_balances(json: &Value) -> Option<StatementBalance> {
    // Types ISO 20022 : comptables, puis disponibles et prévisionnels.
    const BOOKED: [&str; 4] = ["ITBD", "CLBD", "PRCD", "OPBD"];
    const OTHERS: [&str; 5] = ["XPCD", "ITAV", "CLAV", "OPAV", "FWAV"];
    json.get("balances")?
        .as_array()?
        .iter()
        .filter_map(|b| {
            let kind = text_at(b, &["balance_type"]).unwrap_or_default();
            let rank = BOOKED
                .iter()
                .chain(&OTHERS)
                .position(|k| *k == kind)
                .unwrap_or(BOOKED.len() + OTHERS.len());
            let raw = text_at(b, &["balance_amount", "amount"])?;
            let mut amount = Amount::parse(&raw.replace('.', ","))?;
            if text_at(b, &["credit_debit_indicator"]) == Some("DBIT") && !amount.is_negative() {
                amount = Amount(-amount.0);
            }
            let mut date = text_at(b, &["reference_date"])
                .and_then(parse_iso_date)
                .or_else(|| text_at(b, &["last_change_date_time"]).and_then(valid_until_date));
            // Solde d'ouverture : sans les opérations du jour.
            if kind == "OPBD" || kind == "OPAV" {
                date = date.map(|d| Date(d.0 - 1));
            }
            let balance = StatementBalance {
                amount,
                date,
                booked: rank < BOOKED.len(),
            };
            Some((rank, balance))
        })
        .min_by_key(|(rank, b)| (*rank, std::cmp::Reverse(b.date)))
        .map(|(_, b)| b)
}

/// Compte importé à présenter à l'aperçu d'import.
pub fn imported_account(
    link: &BankAccountLink,
    mut operations: Vec<ImportedOperation>,
    balance: Option<StatementBalance>,
) -> ImportedAccount {
    operations.sort_by_key(|o| o.date);
    for (k, op) in operations.iter_mut().enumerate() {
        op.line = k + 1;
    }
    ImportedAccount {
        name: Some(link.name.clone()),
        account_id: (!link.iban.is_empty()).then(|| link.iban.clone()),
        currency: (!link.currency.is_empty()).then(|| link.currency.clone()),
        balance,
        operations,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_authorization_code() {
        let url = "https://github.com/fabiochelly/bankperfect?state=abc&code=1234-abcd";
        assert_eq!(authorization_code(url, "abc").as_deref(), Ok("1234-abcd"));
        assert_eq!(
            authorization_code("  1234-abcd ", "abc").as_deref(),
            Ok("1234-abcd")
        );
        assert!(
            authorization_code("https://x.fr/?state=zzz&code=1", "abc").is_err(),
            "state différent"
        );
        let refused = authorization_code(
            "https://x.fr/?error=access_denied&error_description=Annul%C3%A9",
            "",
        );
        assert_eq!(
            refused,
            Err("la banque a refusé l'accès (access_denied) Annulé".into())
        );
        assert!(authorization_code("https://x.fr/page", "").is_err());
    }

    #[test]
    fn converts_transactions() {
        let json: Value = serde_json::from_str(
            r#"{"transactions":[
                {"entry_reference":"E1","transaction_amount":{"currency":"EUR","amount":"42.50"},
                 "credit_debit_indicator":"DBIT","status":"BOOK","booking_date":"2026-10-02",
                 "value_date":"2026-10-03","creditor":{"name":"CARREFOUR"},"merchant_category_code":"5411",
                 "remittance_information":["CARTE 01/10 CARREFOUR","PARIS"]},
                {"transaction_id":"T2","transaction_amount":{"currency":"EUR","amount":"1500"},
                 "credit_debit_indicator":"CRDT","status":"BOOK","booking_date":"2026-10-05",
                 "debtor":{"name":"EMPLOYEUR SA"},"remittance_information":["SALAIRE OCTOBRE"]},
                {"entry_reference":"E3","transaction_amount":{"currency":"EUR","amount":"9.99"},
                 "credit_debit_indicator":"DBIT","status":"PDNG","booking_date":"2026-10-06"}
              ],"continuation_key":"page2"}"#,
        )
        .unwrap();
        let (ops, next) = parse_transactions(&json);
        assert_eq!(next.as_deref(), Some("page2"));
        assert_eq!(ops.len(), 2, "opération en attente ignorée");
        assert_eq!(ops[0].amount, Amount::from_cents(-4250));
        assert_eq!(ops[0].payee, "CARREFOUR");
        assert_eq!(ops[0].details, "CARTE 01/10 CARREFOUR PARIS");
        assert_eq!(ops[0].fitid.as_deref(), Some("E1"));
        assert_eq!(ops[0].merchant_code, Some(5411));
        assert_eq!(ops[1].merchant_code, None);
        assert_eq!(ops[0].value_date, parse_iso_date("2026-10-03"));
        assert_eq!(ops[1].amount, Amount::from_cents(150_000));
        assert_eq!(ops[1].payee, "EMPLOYEUR SA");
        assert_eq!(ops[1].mode, Mode::Standard(StandardMode::VirementRecu));
        assert_eq!(ops[1].fitid.as_deref(), Some("T2"));
    }

    #[test]
    fn picks_the_booked_balance() {
        let json: Value = serde_json::from_str(
            r#"{"balances":[
                {"balance_amount":{"currency":"EUR","amount":"1180.40"},"balance_type":"ITAV","reference_date":"2026-10-16"},
                {"balance_amount":{"currency":"EUR","amount":"1200.00"},"balance_type":"CLBD","reference_date":"2026-10-14"},
                {"balance_amount":{"currency":"EUR","amount":"-35.10"},"balance_type":"CLBD","reference_date":"2026-10-15"}
              ]}"#,
        )
        .unwrap();
        let b = parse_balances(&json).unwrap();
        assert_eq!(b.amount, Amount::from_cents(-3510), "le plus récent");
        assert_eq!(b.date, parse_iso_date("2026-10-15"));
        assert!(b.booked);

        let json: Value = serde_json::from_str(
            r#"{"balances":[{"balance_amount":{"currency":"EUR","amount":"10"},"balance_type":"OPAV","reference_date":"2026-10-16"}]}"#,
        )
        .unwrap();
        let b = parse_balances(&json).unwrap();
        assert!(!b.booked);
        assert_eq!(b.date, parse_iso_date("2026-10-15"), "solde d'ouverture");
        assert_eq!(parse_balances(&serde_json::json!({"balances": []})), None);
    }

    #[test]
    fn banks_and_accounts() {
        let banks: Value = serde_json::from_str(
            r#"{"aspsps":[{"name":"Société Générale","country":"FR","maximum_consent_validity":15552000},{"name":"BNP Paribas","country":"FR"}]}"#,
        )
        .unwrap();
        let list = parse_banks(&banks);
        assert_eq!(list[0].name, "BNP Paribas");
        assert_eq!(list[1].max_consent_seconds, Some(15_552_000));

        let mut doc = crate::manage::new_document("Test", Amount::ZERO);
        doc.accounts[0].iban = "FR76 3000 6000 0112 3456 7890 189".into();
        let session: Value = serde_json::from_str(
            r#"{"session_id":"s","accounts":[{"uid":"u1","account_id":{"iban":"FR7630006000011234567890189"},"name":"Compte courant","currency":"EUR"},{"uid":"u2","product":"Livret A"}]}"#,
        )
        .unwrap();
        let accounts = parse_session_accounts(&session, &doc);
        assert_eq!(accounts.len(), 2);
        assert_eq!(
            accounts[0].target.as_deref(),
            Some(doc.accounts[0].uid.as_str()),
            "rapproché par l'IBAN"
        );
        assert_eq!(accounts[1].name, "Livret A");
        assert_eq!(accounts[1].target, None);
    }

    #[test]
    fn sync_window_and_expiry() {
        let mut doc = crate::manage::new_document("Test", Amount::ZERO);
        let today = parse_iso_date("2026-10-11").unwrap();
        doc.accounts[0].operations.clear();
        assert_eq!(sync_start(&doc, 0, today), Date(today.0 - FIRST_SYNC_DAYS));
        let connection = BankConnection {
            session_id: "s".into(),
            bank: "B".into(),
            country: "FR".into(),
            valid_until: "2026-10-10T12:00:00+00:00".into(),
            accounts: Vec::new(),
        };
        assert!(connection.expired(today));
    }
}
