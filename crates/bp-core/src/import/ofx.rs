//! Import OFX (Open Financial Exchange), ex-script `import_ofx.py` et son
//! fichier `mapping.ini`.
//!
//! L'analyseur est volontairement tolérant et sans dépendance : il découpe le
//! texte en balises et lit les valeurs, ce qui convient à l'OFX 1.x (SGML, où
//! les éléments simples n'ont pas de balise fermante) comme à l'OFX 2.x (XML).
//!
//! - Chaque relevé (`<STMTTRNRS>`/`<STMTRS>`, ou `<CCSTMTTRNRS>`/`<CCSTMTRS>`
//!   pour une carte) donne un [`ImportedAccount`] identifié par `BANKID`,
//!   `BRANCHID`, `ACCTID` (et `TRNUID`, `CURDEF`).
//! - Chaque `<STMTTRN>` donne une opération : `DTPOSTED` (date), `TRNAMT`
//!   (montant), `NAME` (tiers), `MEMO` (détail), `CHECKNUM`, `FITID`,
//!   `TRNTYPE` (mode, via la table de `mapping.ini` et des heuristiques sur
//!   le tiers), `DTAVAIL` (date de valeur).
//!
//! Écarts avec l'original : entités XML/SGML décodées (`&amp;`), opérations
//! sans date ou sans montant signalées au lieu de faire échouer le script,
//! tiers vide remplacé par le mémo, mémo identique au tiers supprimé.

use super::{
    ImportError, ImportedAccount, ImportedOperation, ParsedImport, RejectReason, RejectedLine,
    StatementBalance, clean_text, decode_text, guess_mode_from_label, parse_amount,
};
use crate::model::{Amount, Date, Mode, StandardMode};

/// Paramètres de l'import OFX.
#[derive(Debug, Clone, PartialEq)]
pub struct OfxOptions {
    /// Mode selon le type de transaction OFX (`TRNTYPE`) : (dépense, recette).
    /// Contenu par défaut : celui de `mapping.ini`.
    pub trntype_modes: Vec<(String, StandardMode, StandardMode)>,
}

impl OfxOptions {
    /// Table de `mapping.ini`.
    pub fn default_trntype_modes() -> Vec<(String, StandardMode, StandardMode)> {
        use StandardMode::*;
        [
            ("POS", Carte, VirementRecu),
            ("ATM", RetraitDab, Versement),
            ("PAYMENT", Prelevement, VirementRecu),
            ("CHECK", ChequeEmis, DepotCheque),
            ("DIRECTDEP", Prelevement, VirementRecu),
            ("CASH", RetraitDab, Versement),
            ("CREDIT", VirementEmis, VirementRecu),
            ("DEBIT", Prelevement, VirementRecu),
            ("DIV", Prelevement, VirementRecu),
            ("INT", Prelevement, VirementRecu),
            ("SRVCHG", Prelevement, VirementRecu),
            ("XFER", VirementEmis, VirementRecu),
            ("DIRECTDEBIT", Prelevement, VirementRecu),
            ("REPEATPMT", Prelevement, VirementRecu),
            ("OTHER", Prelevement, VirementRecu),
        ]
        .into_iter()
        .map(|(t, d, c)| (t.to_string(), d, c))
        .collect()
    }

    /// Lit une table au format de `mapping.ini` (`TRNTYPE=mode dépense,mode
    /// recette`, libellés des modes standard). Les lignes illisibles sont
    /// ignorées.
    pub fn parse_mapping(text: &str) -> Vec<(String, StandardMode, StandardMode)> {
        text.lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                let (debit, credit) = value.split_once(',')?;
                Some((
                    key.trim().to_uppercase(),
                    StandardMode::from_label(debit.trim())?,
                    StandardMode::from_label(credit.trim())?,
                ))
            })
            .collect()
    }
}

impl Default for OfxOptions {
    fn default() -> Self {
        Self {
            trntype_modes: Self::default_trntype_modes(),
        }
    }
}

/// Élément lexical d'un fichier OFX.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token<'a> {
    /// Balise ouvrante (nom en majuscules) et position dans le texte.
    Open(String, usize),
    Close(String),
    Text(&'a str),
}

/// Découpe le texte en balises et en textes. Commentaires, instructions de
/// traitement (`<?xml …?>`) et déclarations (`<!DOCTYPE …>`) sont ignorés ;
/// les sections `CDATA` sont rendues comme du texte. Les attributs sont
/// ignorés.
fn tokenize(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut rest = text;
    let mut offset = 0;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            tokens.push(Token::Text(rest));
            break;
        };
        if lt > 0 {
            tokens.push(Token::Text(&rest[..lt]));
        }
        let tag_start = offset + lt;
        let after = &rest[lt..];
        let (consumed, token) = if let Some(body) = after.strip_prefix("<![CDATA[") {
            let end = body.find("]]>").unwrap_or(body.len());
            (9 + end + 3, Some(Token::Text(&body[..end])))
        } else if after.starts_with("<!--") {
            (after.find("-->").map_or(after.len(), |p| p + 3), None)
        } else if after.starts_with("<?") {
            (after.find("?>").map_or(after.len(), |p| p + 2), None)
        } else {
            match after.find('>') {
                Some(gt) => {
                    let inner = after[1..gt].trim();
                    let token = if let Some(name) = inner.strip_prefix('/') {
                        Some(Token::Close(name.trim().to_uppercase()))
                    } else if inner.starts_with('!') || inner.is_empty() {
                        None
                    } else {
                        let self_closing = inner.ends_with('/');
                        let name = inner
                            .trim_end_matches('/')
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_uppercase();
                        if self_closing {
                            tokens.push(Token::Open(name.clone(), tag_start));
                            Some(Token::Close(name))
                        } else {
                            Some(Token::Open(name, tag_start))
                        }
                    };
                    (gt + 1, token)
                }
                // Balise non terminée : fin du fichier.
                None => (after.len(), None),
            }
        };
        if let Some(token) = token {
            tokens.push(token);
        }
        let consumed = lt + consumed.min(after.len());
        offset += consumed;
        rest = &rest[consumed..];
    }
    tokens
}

/// Décode les entités `&amp; &lt; &gt; &quot; &apos; &nbsp; &#NNN; &#xHH;`.
/// Une esperluette isolée (fréquente dans l'OFX SGML) est conservée.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let decoded = after.find(';').filter(|&end| end <= 10).and_then(|end| {
            let name = &after[1..end];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => {
                    let code =
                        if let Some(hex) = name.strip_prefix("#x").or(name.strip_prefix("#X")) {
                            u32::from_str_radix(hex, 16).ok()
                        } else {
                            name.strip_prefix('#').and_then(|d| d.parse().ok())
                        };
                    code.and_then(char::from_u32)
                }
            };
            c.map(|c| (c, end + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &after[len..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Date OFX `AAAAMMJJ[HHMMSS[.XXX]][[-5:EST]]` : seuls les 8 premiers
/// chiffres comptent.
fn parse_ofx_date(text: &str) -> Option<Date> {
    let digits = text.trim().get(..8)?;
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Date::from_ymd(
        digits[..4].parse().ok()?,
        digits[4..6].parse().ok()?,
        digits[6..].parse().ok()?,
    )
}

/// Champs bruts d'une transaction `<STMTTRN>`.
#[derive(Debug, Default)]
struct RawTransaction {
    line: usize,
    trntype: Option<String>,
    posted: Option<String>,
    available: Option<String>,
    amount: Option<String>,
    fitid: Option<String>,
    checknum: Option<String>,
    name: Option<String>,
    memo: Option<String>,
    sic: Option<String>,
}

impl RawTransaction {
    fn set(&mut self, tag: &str, value: String) {
        let slot = match tag {
            "TRNTYPE" => &mut self.trntype,
            "DTPOSTED" => &mut self.posted,
            "DTAVAIL" => &mut self.available,
            "TRNAMT" => &mut self.amount,
            "FITID" => &mut self.fitid,
            "CHECKNUM" => &mut self.checknum,
            "NAME" => &mut self.name,
            "MEMO" => &mut self.memo,
            "SIC" => &mut self.sic,
            _ => return,
        };
        if slot.is_none() {
            *slot = Some(value);
        }
    }

    fn summary(&self) -> String {
        [
            ("DTPOSTED", &self.posted),
            ("TRNAMT", &self.amount),
            ("NAME", &self.name),
            ("MEMO", &self.memo),
        ]
        .iter()
        .filter_map(|(k, v)| {
            v.as_deref()
                .filter(|v| !v.is_empty())
                .map(|v| format!("{k}={v}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
    }
}

/// Mode d'une transaction (`calculate_paymode`) : pour les types peu précis,
/// les mots du tiers (puis du mémo) priment ; sinon la table
/// [`OfxOptions::trntype_modes`] ; à défaut Prélèvement / Virement reçu.
fn transaction_mode(
    options: &OfxOptions,
    trntype: Option<&str>,
    name: &str,
    memo: &str,
    negative: bool,
) -> StandardMode {
    let trntype = trntype.map(|t| t.trim().to_uppercase());
    let vague = match trntype.as_deref() {
        None | Some("OTHER") => true,
        Some("ATM" | "PAYMENT" | "DEBIT") => negative,
        Some("CREDIT" | "DEP") => !negative,
        _ => false,
    };
    if vague
        && let Some(mode) =
            guess_mode_from_label(name, negative).or_else(|| guess_mode_from_label(memo, negative))
    {
        return mode;
    }
    let (debit, credit) = trntype
        .and_then(|t| {
            options
                .trntype_modes
                .iter()
                .find(|(key, _, _)| key.eq_ignore_ascii_case(&t))
                .map(|(_, d, c)| (*d, *c))
        })
        .unwrap_or((StandardMode::Prelevement, StandardMode::VirementRecu));
    if negative { debit } else { credit }
}

struct Builder<'a> {
    options: &'a OfxOptions,
    result: ParsedImport,
    current: Option<ImportedAccount>,
    has_statement: bool,
    transaction: Option<RawTransaction>,
    /// Dans `LEDGERBAL` (`Some(true)`) ou `AVAILBAL` (`Some(false)`).
    in_balance: Option<bool>,
}

impl Builder<'_> {
    fn start_account(&mut self, credit_card: bool) {
        self.finish_transaction();
        if let Some(account) = self.current.take() {
            self.result.accounts.push(account);
        }
        self.current = Some(ImportedAccount {
            credit_card,
            ..Default::default()
        });
        self.has_statement = false;
    }

    fn account(&mut self) -> &mut ImportedAccount {
        self.current.get_or_insert_with(ImportedAccount::default)
    }

    fn finish_transaction(&mut self) {
        let Some(raw) = self.transaction.take() else {
            return;
        };
        match self.convert(&raw) {
            Ok(op) => self.account().operations.push(op),
            Err(reason) => self.result.rejected.push(RejectedLine {
                line: raw.line,
                text: raw.summary(),
                reason,
            }),
        }
    }

    fn convert(&self, raw: &RawTransaction) -> Result<ImportedOperation, RejectReason> {
        let posted = raw
            .posted
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let date_text = posted.ok_or(RejectReason::MissingDate)?;
        let date = parse_ofx_date(date_text)
            .ok_or_else(|| RejectReason::InvalidDate(date_text.to_string()))?;
        let amount_text = raw
            .amount
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or(RejectReason::MissingAmount)?;
        let amount = parse_amount(amount_text, None)
            .ok_or_else(|| RejectReason::InvalidAmount(amount_text.to_string()))?;
        if amount == Amount::ZERO {
            return Err(RejectReason::ZeroAmount);
        }
        let negative = amount.is_negative();
        let mut payee = clean_text(raw.name.as_deref().unwrap_or(""));
        let mut details = clean_text(raw.memo.as_deref().unwrap_or(""));
        if details == "." || details == payee {
            details.clear();
        }
        if payee.is_empty() {
            payee = std::mem::take(&mut details);
        }
        let mode = transaction_mode(
            self.options,
            raw.trntype.as_deref(),
            raw.name.as_deref().unwrap_or(""),
            raw.memo.as_deref().unwrap_or(""),
            negative,
        );
        let cheque_number = raw
            .checknum
            .as_deref()
            .map(str::trim)
            .filter(|n| negative && !n.is_empty())
            .map(str::to_string);
        Ok(ImportedOperation {
            line: raw.line,
            date,
            value_date: raw.available.as_deref().and_then(parse_ofx_date),
            amount,
            mode: Mode::Standard(mode),
            cheque_number,
            payee,
            details,
            category: None,
            subcategory: None,
            tack: None,
            fitid: raw
                .fitid
                .as_deref()
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .map(str::to_string),
            merchant_code: raw.sic.as_deref().and_then(|s| s.trim().parse().ok()),
        })
    }

    fn open(&mut self, tag: &str, line: usize) {
        match tag {
            "LEDGERBAL" => self.in_balance = Some(true),
            "AVAILBAL" => self.in_balance = Some(false),
            "BALAMT" | "DTASOF" => {}
            _ => self.in_balance = None,
        }
        match tag {
            "STMTTRNRS" | "CCSTMTTRNRS" => self.start_account(tag.starts_with("CC")),
            "STMTRS" | "CCSTMTRS" => {
                if self.current.is_none() || self.has_statement {
                    self.start_account(tag.starts_with("CC"));
                }
                self.has_statement = true;
                if tag.starts_with("CC") {
                    self.account().credit_card = true;
                }
            }
            "STMTTRN" => {
                self.finish_transaction();
                self.transaction = Some(RawTransaction {
                    line,
                    ..Default::default()
                });
            }
            "LEDGERBAL" | "AVAILBAL" | "BANKTRANLIST" => self.finish_transaction(),
            _ => {}
        }
    }

    fn close(&mut self, tag: &str) {
        if matches!(tag, "STMTTRN" | "BANKTRANLIST" | "STMTRS" | "CCSTMTRS") {
            self.finish_transaction();
        }
        if matches!(tag, "LEDGERBAL" | "AVAILBAL") {
            self.in_balance = None;
        }
    }

    /// Solde du relevé : `LEDGERBAL` (comptable), à défaut `AVAILBAL`.
    fn balance(&mut self, tag: &str, value: &str, booked: bool) {
        let account = self.account();
        match tag {
            "BALAMT" => {
                if let Some(amount) = parse_amount(value, None)
                    && account.balance.is_none_or(|b| booked && !b.booked)
                {
                    account.balance = Some(StatementBalance {
                        amount,
                        date: None,
                        booked,
                    });
                }
            }
            "DTASOF" => {
                if let Some(b) = account.balance.as_mut().filter(|b| b.booked == booked) {
                    b.date = parse_ofx_date(value);
                }
            }
            _ => {}
        }
    }

    fn value(&mut self, tag: &str, value: String) {
        if let Some(transaction) = self.transaction.as_mut() {
            transaction.set(tag, value);
            return;
        }
        if value.is_empty() {
            return;
        }
        if let Some(booked) = self.in_balance {
            self.balance(tag, &value, booked);
            return;
        }
        let account = match tag {
            "BANKID" | "BRANCHID" | "ACCTID" | "TRNUID" | "CURDEF" => self.account(),
            _ => return,
        };
        let slot = match tag {
            "BANKID" => &mut account.bank_id,
            "BRANCHID" => &mut account.branch_id,
            "ACCTID" => &mut account.account_id,
            "TRNUID" => &mut account.transaction_uid,
            _ => &mut account.currency,
        };
        if slot.is_none() {
            *slot = Some(value);
        }
    }
}

/// Analyse un fichier OFX 1.x (SGML) ou 2.x (XML).
///
/// Erreur [`ImportError::NotOfx`] si le fichier ne contient ni balise `<OFX>`
/// ni transaction `<STMTTRN>`. Les transactions sans date ou sans montant
/// lisible sont signalées dans [`ParsedImport::rejected`].
pub fn parse(data: &[u8], options: &OfxOptions) -> Result<ParsedImport, ImportError> {
    let text = decode_text(data);
    let tokens = tokenize(&text);
    let is_ofx = tokens
        .iter()
        .any(|t| matches!(t, Token::Open(name, _) if name == "OFX" || name == "STMTTRN"));
    if !is_ofx {
        return Err(ImportError::NotOfx);
    }

    let mut builder = Builder {
        options,
        result: ParsedImport::default(),
        current: None,
        has_statement: false,
        transaction: None,
        in_balance: None,
    };
    let mut last_open: Option<&str> = None;
    // Numéro de ligne : calculé au fil des positions croissantes.
    let mut line = 1;
    let mut line_offset = 0;
    for token in &tokens {
        match token {
            Token::Open(name, offset) => {
                line += text[line_offset..*offset].matches('\n').count();
                line_offset = *offset;
                builder.open(name, line);
                last_open = Some(name);
            }
            Token::Close(name) => {
                builder.close(name);
                last_open = None;
            }
            Token::Text(raw) => {
                if let Some(tag) = last_open.take() {
                    builder.value(tag, decode_entities(raw.trim()));
                }
            }
        }
    }
    builder.finish_transaction();
    if let Some(account) = builder.current.take() {
        builder.result.accounts.push(account);
    }
    builder.result.accounts.retain(|a| !a.operations.is_empty());
    Ok(builder.result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::tests::ymd;
    use crate::import::{ConvertOptions, to_operation};

    const SGML: &str = "OFXHEADER:100\r\n\
DATA:OFXSGML\r\n\
VERSION:102\r\n\
ENCODING:USASCII\r\n\
CHARSET:1252\r\n\
\r\n\
<OFX>\r\n\
<SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS><DTSERVER>20260311</SONRS></SIGNONMSGSRSV1>\r\n\
<BANKMSGSRSV1>\r\n\
<STMTTRNRS>\r\n\
<TRNUID>20260311\r\n\
<STATUS><CODE>0<SEVERITY>INFO</STATUS>\r\n\
<STMTRS>\r\n\
<CURDEF>EUR\r\n\
<BANKACCTFROM>\r\n\
<BANKID>30004\r\n\
<BRANCHID>00123\r\n\
<ACCTID>00012345678\r\n\
<ACCTTYPE>CHECKING\r\n\
</BANKACCTFROM>\r\n\
<BANKTRANLIST>\r\n\
<DTSTART>20260301\r\n\
<DTEND>20260311\r\n\
<STMTTRN>\r\n\
<TRNTYPE>DEBIT\r\n\
<DTPOSTED>20260305\r\n\
<DTAVAIL>20260306\r\n\
<TRNAMT>-12.50\r\n\
<FITID>A001\r\n\
<NAME>CB FNAC  PARIS\r\n\
<MEMO>.\r\n\
</STMTTRN>\r\n\
<STMTTRN>\r\n\
<TRNTYPE>CHECK\r\n\
<DTPOSTED>20260306120000.000[+1:CET]\r\n\
<TRNAMT>-50,00\r\n\
<FITID>A002\r\n\
<CHECKNUM>0001234\r\n\
<NAME>CHEQUE\r\n\
</STMTTRN>\r\n\
<STMTTRN>\r\n\
<TRNTYPE>CREDIT\r\n\
<DTPOSTED>20260307\r\n\
<TRNAMT>1500.00\r\n\
<FITID>A003\r\n\
<NAME>VIR SEPA EMPLOYEUR\r\n\
<MEMO>SALAIRE MARS\r\n\
</STMTTRN>\r\n\
<STMTTRN>\r\n\
<TRNTYPE>DEBIT\r\n\
<DTPOSTED>\r\n\
<TRNAMT>-1.00\r\n\
<NAME>SANS DATE\r\n\
</STMTTRN>\r\n\
<STMTTRN>\r\n\
<TRNTYPE>POS\r\n\
<DTPOSTED>20260308\r\n\
<TRNAMT>-3.20\r\n\
<NAME>Café &amp; Cie\r\n\
</STMTTRN>\r\n\
</BANKTRANLIST>\r\n\
<LEDGERBAL><BALAMT>1000.00<DTASOF>20260311</LEDGERBAL>\r\n\
</STMTRS>\r\n\
</STMTTRNRS>\r\n\
</BANKMSGSRSV1>\r\n\
<CREDITCARDMSGSRSV1>\r\n\
<CCSTMTTRNRS>\r\n\
<TRNUID>2\r\n\
<CCSTMTRS>\r\n\
<CURDEF>EUR\r\n\
<CCACCTFROM><ACCTID>4970XXXXXXXX1234</CCACCTFROM>\r\n\
<BANKTRANLIST>\r\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260309<TRNAMT>-20.00<NAME>RETRAIT DAB<MEMO>RETRAIT DAB</STMTTRN>\r\n\
</BANKTRANLIST>\r\n\
</CCSTMTRS>\r\n\
</CCSTMTTRNRS>\r\n\
</CREDITCARDMSGSRSV1>\r\n\
</OFX>\r\n";

    /// Le texte SGML ci-dessus est en Windows-1252 (« é » = 0xE9).
    fn sgml_bytes() -> Vec<u8> {
        SGML.chars()
            .map(|c| if c == '\u{e9}' { 0xE9 } else { c as u8 })
            .collect()
    }

    #[test]
    fn parse_sgml() {
        let parsed = parse(&sgml_bytes(), &OfxOptions::default()).unwrap();
        assert_eq!(parsed.accounts.len(), 2);

        let bank = &parsed.accounts[0];
        assert_eq!(bank.bank_id.as_deref(), Some("30004"));
        assert_eq!(bank.branch_id.as_deref(), Some("00123"));
        assert_eq!(bank.account_id.as_deref(), Some("00012345678"));
        assert_eq!(bank.transaction_uid.as_deref(), Some("20260311"));
        assert_eq!(bank.currency.as_deref(), Some("EUR"));
        assert!(!bank.credit_card);
        assert_eq!(bank.description(), "n° 30004-00123-00012345678");
        assert_eq!(bank.operations.len(), 4);
        assert_eq!(
            bank.balance,
            Some(StatementBalance {
                amount: Amount::from_cents(100_000),
                date: Some(ymd(2026, 3, 11)),
                booked: true,
            })
        );
        assert_eq!(parsed.accounts[1].balance, None);

        let ops = &bank.operations;
        assert_eq!(ops[0].date, ymd(2026, 3, 5));
        assert_eq!(ops[0].value_date, Some(ymd(2026, 3, 6)));
        assert_eq!(ops[0].amount, Amount::from_cents(-1250));
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::Carte));
        assert_eq!(ops[0].payee, "CB FNAC PARIS");
        assert_eq!(ops[0].details, "");
        assert_eq!(ops[0].fitid.as_deref(), Some("A001"));
        assert_eq!(ops[0].line, 24);

        assert_eq!(ops[1].date, ymd(2026, 3, 6));
        assert_eq!(ops[1].mode, Mode::Standard(StandardMode::ChequeEmis));
        assert_eq!(ops[1].cheque_number.as_deref(), Some("0001234"));
        assert_eq!(ops[1].amount, Amount::from_cents(-5000));

        assert_eq!(ops[2].mode, Mode::Standard(StandardMode::VirementRecu));
        assert_eq!(ops[2].details, "SALAIRE MARS");

        assert_eq!(ops[3].payee, "Café & Cie");
        assert_eq!(ops[3].mode, Mode::Standard(StandardMode::Carte));

        assert_eq!(parsed.rejected.len(), 1);
        assert_eq!(parsed.rejected[0].reason, RejectReason::MissingDate);
        assert_eq!(parsed.rejected[0].text, "TRNAMT=-1.00, NAME=SANS DATE");

        let card = &parsed.accounts[1];
        assert!(card.credit_card);
        assert_eq!(card.account_id.as_deref(), Some("4970XXXXXXXX1234"));
        assert_eq!(card.transaction_uid.as_deref(), Some("2"));
        assert_eq!(card.operations.len(), 1);
        assert_eq!(
            card.operations[0].mode,
            Mode::Standard(StandardMode::RetraitDab)
        );
        // Mémo identique au tiers : supprimé.
        assert_eq!(card.operations[0].details, "");

        // Conversion : le numéro de chèque passe dans le détail.
        let doc = crate::import::tests::document(&[]);
        let op = to_operation(&doc, &ops[1], &ConvertOptions::default());
        assert_eq!(op.details, "n°0001234");
        assert_eq!(op.mode_label(), "Chq 0001234");
    }

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<?OFX OFXHEADER="200" VERSION="220" SECURITY="NONE" OLDFILEUID="NONE" NEWFILEUID="NONE"?>
<OFX>
  <!-- Relevé de test -->
  <BANKMSGSRSV1>
    <STMTTRNRS>
      <TRNUID>1001</TRNUID>
      <STMTRS>
        <CURDEF>EUR</CURDEF>
        <BANKACCTFROM>
          <BANKID>10278</BANKID>
          <BRANCHID>06000</BRANCHID>
          <ACCTID>00020412501</ACCTID>
          <ACCTTYPE>CHECKING</ACCTTYPE>
        </BANKACCTFROM>
        <BANKTRANLIST>
          <DTSTART>20260301</DTSTART>
          <DTEND>20260331</DTEND>
          <STMTTRN>
            <TRNTYPE>PAYMENT</TRNTYPE>
            <DTPOSTED>20260310000000</DTPOSTED>
            <TRNAMT>-45.99</TRNAMT>
            <FITID>X1</FITID>
            <NAME>PRLV SEPA FREE MOBILE</NAME>
            <MEMO>Échéance &lt;mars&gt;</MEMO>
            <BANKACCTTO><BANKID>1</BANKID><ACCTID>99999</ACCTID></BANKACCTTO>
          </STMTTRN>
          <STMTTRN>
            <TRNTYPE>XFER</TRNTYPE>
            <DTPOSTED>20260312</DTPOSTED>
            <TRNAMT>-200.00</TRNAMT>
            <FITID>X2</FITID>
            <PAYEE><NAME>Livret A</NAME></PAYEE>
          </STMTTRN>
          <STMTTRN>
            <TRNTYPE>DEP</TRNTYPE>
            <DTPOSTED>20260313</DTPOSTED>
            <TRNAMT>abc</TRNAMT>
            <NAME>Erreur</NAME>
          </STMTTRN>
          <STMTTRN>
            <TRNTYPE>INT</TRNTYPE>
            <DTPOSTED>20260314</DTPOSTED>
            <TRNAMT>0.00</TRNAMT>
          </STMTTRN>
          <STMTTRN>
            <TRNTYPE>OTHER</TRNTYPE>
            <DTPOSTED>20260315</DTPOSTED>
            <TRNAMT>-8.00</TRNAMT>
            <MEMO><![CDATA[FRAIS CB <INTERNATIONAL>]]></MEMO>
          </STMTTRN>
        </BANKTRANLIST>
      </STMTRS>
    </STMTTRNRS>
  </BANKMSGSRSV1>
</OFX>
"#;

    #[test]
    fn parse_xml() {
        let parsed = parse(XML.as_bytes(), &OfxOptions::default()).unwrap();
        assert_eq!(parsed.accounts.len(), 1);
        let account = &parsed.accounts[0];
        // L'ACCTID du compte destinataire d'une transaction est ignoré.
        assert_eq!(account.account_id.as_deref(), Some("00020412501"));
        assert_eq!(account.bank_id.as_deref(), Some("10278"));
        assert_eq!(account.transaction_uid.as_deref(), Some("1001"));
        let ops = &account.operations;
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].date, ymd(2026, 3, 10));
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::Prelevement));
        assert_eq!(ops[0].payee, "PRLV SEPA FREE MOBILE");
        assert_eq!(ops[0].details, "Échéance <mars>");
        assert_eq!(ops[1].payee, "Livret A");
        assert_eq!(ops[1].mode, Mode::Standard(StandardMode::VirementEmis));
        // Tiers vide : le mémo le remplace ; type OTHER : heuristique « CB ».
        assert_eq!(ops[2].payee, "FRAIS CB <INTERNATIONAL>");
        assert_eq!(ops[2].details, "");
        assert_eq!(ops[2].mode, Mode::Standard(StandardMode::Carte));

        let reasons: Vec<&RejectReason> = parsed.rejected.iter().map(|r| &r.reason).collect();
        assert_eq!(
            reasons,
            vec![
                &RejectReason::InvalidAmount("abc".into()),
                &RejectReason::ZeroAmount
            ]
        );
    }

    #[test]
    fn minimal_and_invalid_files() {
        // Ni STMTTRNRS ni STMTRS : un seul compte, comme l'original.
        let text = "<STMTTRN><DTPOSTED>20260301<TRNAMT>5<NAME>X</STMTTRN>";
        let parsed = parse(text.as_bytes(), &OfxOptions::default()).unwrap();
        assert_eq!(parsed.operation_count(), 1);
        assert_eq!(parsed.accounts[0].description(), "");
        assert_eq!(
            parsed.accounts[0].operations[0].mode,
            Mode::Standard(StandardMode::VirementRecu)
        );

        // Transactions SGML sans balise fermante.
        let text = "<OFX><STMTRS><BANKTRANLIST>\
<STMTTRN><TRNTYPE>ATM<DTPOSTED>20260301<TRNAMT>-40\
<STMTTRN><TRNTYPE>ATM<DTPOSTED>20260302<TRNAMT>40\
</BANKTRANLIST><LEDGERBAL><BALAMT>0</LEDGERBAL></STMTRS></OFX>";
        let parsed = parse(text.as_bytes(), &OfxOptions::default()).unwrap();
        let ops = &parsed.accounts[0].operations;
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].mode, Mode::Standard(StandardMode::RetraitDab));
        assert_eq!(ops[1].mode, Mode::Standard(StandardMode::Versement));

        assert_eq!(
            parse(b"Date;Montant\n", &OfxOptions::default()),
            Err(ImportError::NotOfx)
        );
        assert_eq!(parse(b"", &OfxOptions::default()), Err(ImportError::NotOfx));
        // Fichier tronqué : pas de panique.
        let parsed = parse(b"<OFX><STMTTRN><DTPOSTED>2026", &OfxOptions::default()).unwrap();
        assert_eq!(
            parsed.rejected[0].reason,
            RejectReason::InvalidDate("2026".into())
        );
    }

    #[test]
    fn mapping_file() {
        let table = OfxOptions::parse_mapping(
            "POS=Carte,Virement reçu\nCHECK=Chèque émis,Dépôt de chèque\nfaux\nX=Inconnu,Carte\n",
        );
        assert_eq!(
            table,
            vec![
                (
                    "POS".to_string(),
                    StandardMode::Carte,
                    StandardMode::VirementRecu
                ),
                (
                    "CHECK".to_string(),
                    StandardMode::ChequeEmis,
                    StandardMode::DepotCheque
                ),
            ]
        );
        let options = OfxOptions {
            trntype_modes: vec![("POS".into(), StandardMode::Tip, StandardMode::Titre)],
        };
        assert_eq!(
            transaction_mode(&options, Some("pos"), "", "", true),
            StandardMode::Tip
        );
        assert_eq!(
            transaction_mode(&options, Some("POS"), "", "", false),
            StandardMode::Titre
        );
        assert_eq!(
            transaction_mode(&options, Some("CHECK"), "", "", true),
            StandardMode::Prelevement
        );
    }

    #[test]
    fn entities_and_dates() {
        assert_eq!(
            decode_entities("A &amp; B &#233;&#xE9; &lt;x&gt;"),
            "A & B éé <x>"
        );
        assert_eq!(
            decode_entities("Dupont & fils &foo;"),
            "Dupont & fils &foo;"
        );
        assert_eq!(parse_ofx_date("20260305"), Some(ymd(2026, 3, 5)));
        assert_eq!(
            parse_ofx_date("20260305235959.000[-5:EST]"),
            Some(ymd(2026, 3, 5))
        );
        assert_eq!(parse_ofx_date("20261305"), None);
        assert_eq!(parse_ofx_date("2026"), None);
        assert_eq!(parse_ofx_date("2026-03-05"), None);
    }
}
