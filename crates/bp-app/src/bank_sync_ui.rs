//! Synchronisation bancaire par Enable Banking : configuration (une fois),
//! connexion aux banques (consentement sur le site de la banque) et import
//! des opérations par l'aperçu d'import habituel.
//!
//! Les identifiants bancaires ne passent jamais par BankPerfect.
//! L'utilisateur crée sa propre application Enable Banking (offre gratuite
//! pour un usage personnel) ; sa clé privée reste dans le dossier de
//! configuration, les connexions (sans secret) dans le fichier de comptes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bp_core::bank_sync::{self, Bank, BankAccountLink, BankConnection, BankSync};
use bp_core::import::{ImportedOperation, ParsedImport};
use bp_core::model::Date;
use egui::{Key, RichText};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::document::OpenDoc;
use crate::theme::{Palette, palette};
use crate::tools::{Outcome, Tool};
use crate::ui_kit;

const CONFIG_FILE: &str = "banque.json";
const KEY_FILE: &str = "banque-cle.pem";
/// Adresse de retour proposée : n'importe quelle page HTTPS convient,
/// l'utilisateur recopie ensuite l'adresse affichée (qui contient le code).
const DEFAULT_REDIRECT: &str = "https://github.com/fabiochelly/bankperfect";
const COUNTRIES: [(&str, &str); 16] = [
    ("FR", "France"),
    ("BE", "Belgique"),
    ("LU", "Luxembourg"),
    ("DE", "Allemagne"),
    ("ES", "Espagne"),
    ("IT", "Italie"),
    ("NL", "Pays-Bas"),
    ("PT", "Portugal"),
    ("AT", "Autriche"),
    ("IE", "Irlande"),
    ("FI", "Finlande"),
    ("SE", "Suède"),
    ("DK", "Danemark"),
    ("NO", "Norvège"),
    ("PL", "Pologne"),
    ("EE", "Estonie"),
];

/// Application Enable Banking de l'utilisateur.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Config {
    application_id: String,
    redirect_url: String,
}

fn load_config() -> Option<(Config, Vec<u8>)> {
    let config: Config = crate::settings::config_file(CONFIG_FILE)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())?;
    let key = crate::settings::config_file(KEY_FILE).and_then(|p| std::fs::read(p).ok())?;
    Some((config, key))
}

fn save_config(config: &Config, key: &[u8]) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    crate::settings::write_config_file(CONFIG_FILE, &json)?;
    crate::settings::write_config_file(KEY_FILE, key)?;
    // Clé privée lisible par l'utilisateur seul.
    #[cfg(unix)]
    if let Some(path) = crate::settings::config_file(KEY_FILE) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

// ─────────────────────────── Client ───────────────────────────

#[derive(Clone)]
struct Client {
    application_id: String,
    key: Arc<Vec<u8>>,
}

impl Client {
    fn token(&self) -> Result<String, String> {
        use jsonwebtoken::{Algorithm, EncodingKey, Header};
        let now = chrono::Utc::now().timestamp();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.application_id.clone());
        let claims = json!({
            "iss": "enablebanking.com",
            "aud": "api.enablebanking.com",
            "iat": now,
            "exp": now + 3600,
        });
        let key = EncodingKey::from_rsa_pem(&self.key)
            .map_err(|e| format!("clé privée illisible : {e}"))?;
        jsonwebtoken::encode(&header, &claims, &key)
            .map_err(|e| format!("signature impossible : {e}"))
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .into();
        let url = format!("{}{path}", bank_sync::API_URL);
        let auth = format!("Bearer {}", self.token()?);
        let response = match (method, body) {
            ("POST", Some(body)) => agent
                .post(&url)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .send(body.to_string().as_str()),
            ("DELETE", _) => agent.delete(&url).header("Authorization", &auth).call(),
            _ => agent.get(&url).header("Authorization", &auth).call(),
        };
        let mut response = response.map_err(|e| format!("connexion impossible : {e}"))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if status >= 400 {
            let detail = value
                .get("message")
                .or_else(|| value.get("detail"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| text.chars().take(200).collect());
            return Err(match status {
                401 | 403 => format!("accès refusé par Enable Banking ({status}) : {detail}"),
                _ => format!("erreur {status} : {detail}"),
            });
        }
        Ok(value)
    }
}

// ─────────────────────────── Tâches de fond ───────────────────────────

enum JobResult {
    Tested,
    Banks(Vec<Bank>),
    AuthUrl(String),
    Session(Value),
    Transactions(Vec<(BankAccountLink, usize, Vec<ImportedOperation>)>),
}

type Job = Arc<Mutex<Option<Result<JobResult, String>>>>;

fn spawn(
    ctx: &egui::Context,
    work: impl FnOnce() -> Result<JobResult, String> + Send + 'static,
) -> Job {
    let job: Job = Arc::new(Mutex::new(None));
    let slot = job.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = work();
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
        ctx.request_repaint();
    });
    job
}

/// Valeur difficile à deviner pour vérifier le retour de la banque.
fn random_state() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    hasher.write_u32(std::process::id());
    format!("bp{:016x}", hasher.finish())
}

// ─────────────────────────── Fenêtre ───────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Setup,
    Connections,
    AddBank,
}

/// Connexion en cours sur le site de la banque.
struct PendingAuth {
    bank: Bank,
    state: String,
    url: String,
}

pub struct BankSyncForm {
    page: Page,
    client: Option<Client>,
    // Configuration.
    app_id: String,
    redirect: String,
    key: Option<Vec<u8>>,
    key_name: String,
    // Connexions.
    sync: BankSync,
    // Ajout d'une banque.
    country: String,
    banks: Vec<Bank>,
    banks_country: String,
    search: String,
    selected: Option<usize>,
    pending: Option<PendingAuth>,
    pasted: String,
    // Tâche réseau en cours et son libellé.
    job: Option<(String, Job)>,
    error: Option<String>,
    message: Option<String>,
}

impl BankSyncForm {
    pub fn new(doc: &OpenDoc) -> Self {
        let loaded = load_config();
        let (config, key) = match &loaded {
            Some((c, k)) => (c.clone(), Some(k.clone())),
            None => (Config::default(), None),
        };
        let client = loaded.as_ref().map(|(c, k)| Client {
            application_id: c.application_id.clone(),
            key: Arc::new(k.clone()),
        });
        let sync = BankSync::load(&doc.document);
        let page = if client.is_none() {
            Page::Setup
        } else if sync.connections.is_empty() {
            Page::AddBank
        } else {
            Page::Connections
        };
        Self {
            page,
            client,
            app_id: config.application_id,
            redirect: if config.redirect_url.is_empty() {
                DEFAULT_REDIRECT.into()
            } else {
                config.redirect_url
            },
            key_name: if key.is_some() {
                "clé enregistrée".into()
            } else {
                String::new()
            },
            key,
            sync,
            country: "FR".into(),
            banks: Vec::new(),
            banks_country: String::new(),
            search: String::new(),
            selected: None,
            pending: None,
            pasted: String::new(),
            job: None,
            error: None,
            message: None,
        }
    }

    fn save_sync(&mut self, doc: &mut OpenDoc) {
        match self.sync.save(&mut doc.document) {
            Ok(()) => doc.dirty = true,
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn start(
        &mut self,
        ctx: &egui::Context,
        label: &str,
        work: impl FnOnce() -> Result<JobResult, String> + Send + 'static,
    ) {
        self.error = None;
        self.message = None;
        self.job = Some((label.to_string(), spawn(ctx, work)));
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &mut OpenDoc) -> Outcome {
        let mut outcome = self.poll(doc);
        if !matches!(outcome, Outcome::Keep) {
            return outcome;
        }
        if self.page == Page::AddBank
            && self.banks_country != self.country
            && self.job.is_none()
            && self.client.is_some()
        {
            self.load_banks(ctx);
        }
        let width = ui_kit::dialog_width(ctx, 0.6, 760.0, 1000.0);
        crate::dialogs::modal(ctx, "bank_sync").show(ctx, |ui| {
            ui.set_width(width);
            let pal = palette(ui);
            crate::dialogs::header(ui, icon::BANK, pal.accent, "Synchronisation bancaire");
            ui_kit::subtitle(
                ui,
                "Importe les opérations de vos comptes par Enable Banking, prestataire agréé de la directive européenne DSP2. Vous vous authentifiez sur le site de votre banque : vos identifiants ne passent jamais par BankPerfect.",
            );
            match self.page {
                Page::Setup => self.setup(ui, ctx, width, &pal),
                Page::Connections => self.connections(ui, ctx, doc, width, &pal),
                Page::AddBank => self.add_bank(ui, ctx, width, &pal),
            }
            if let Some((label, _)) = &self.job {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new(label).color(pal.muted));
                });
            } else if let Some(error) = &self.error {
                ui.label(RichText::new(format!("{}  {}", icon::WARNING, capitalize(error))).color(pal.negative));
            } else if let Some(message) = &self.message {
                ui.label(RichText::new(format!("{}  {message}", icon::CHECK_CIRCLE)).color(pal.positive));
            }
            crate::dialogs::footer(ui, |ui| {
                let busy = self.job.is_some();
                match self.page {
                    Page::Connections => {
                        let ready = self.importable(doc);
                        if crate::dialogs::primary(ui, &format!("{}  Importer les opérations", icon::DOWNLOAD_SIMPLE), ready && !busy).clicked() {
                            self.import(ctx, doc);
                        }
                        if crate::dialogs::button(ui, format!("{}  Ajouter une banque", icon::PLUS)).clicked() && !busy {
                            self.page = Page::AddBank;
                            self.pending = None;
                        }
                    }
                    Page::AddBank if !self.sync.connections.is_empty() => {
                        let back = crate::dialogs::button(ui, format!("{}  Connexions", icon::ARROW_LEFT)).clicked();
                        if back && !busy {
                            self.page = Page::Connections;
                            self.pending = None;
                        }
                    }
                    _ => {}
                }
                if crate::dialogs::button(ui, "Fermer").clicked() || (ui.input(|i| i.key_pressed(Key::Escape)) && !busy) {
                    outcome = Outcome::Close;
                }
            });
        });
        outcome
    }

    /// Résultat d'une tâche de fond.
    fn poll(&mut self, doc: &mut OpenDoc) -> Outcome {
        let Some((_, job)) = &self.job else {
            return Outcome::Keep;
        };
        let Some(result) = job.lock().ok().and_then(|mut g| g.take()) else {
            return Outcome::Keep;
        };
        self.job = None;
        match result {
            Err(e) => self.error = Some(e),
            Ok(JobResult::Tested) => {
                let config = Config {
                    application_id: self.app_id.trim().to_string(),
                    redirect_url: self.redirect.trim().to_string(),
                };
                match self
                    .key
                    .as_deref()
                    .ok_or("clé manquante".to_string())
                    .and_then(|k| save_config(&config, k))
                {
                    Ok(()) => {
                        self.message =
                            Some("Configuration enregistrée : Enable Banking répond.".into());
                        self.page = if self.sync.connections.is_empty() {
                            Page::AddBank
                        } else {
                            Page::Connections
                        };
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Ok(JobResult::Banks(banks)) => {
                self.banks = banks;
                self.selected = None;
                if self.banks.is_empty() {
                    self.error = Some("aucune banque disponible pour ce pays".into());
                }
            }
            Ok(JobResult::AuthUrl(url)) => {
                if let Some(pending) = &mut self.pending {
                    pending.url = url.clone();
                }
                if open::that(&url).is_err() {
                    self.message =
                        Some("Ouvrez l'adresse ci-dessous dans votre navigateur.".into());
                }
            }
            Ok(JobResult::Session(value)) => {
                let Some(pending) = self.pending.take() else {
                    return Outcome::Keep;
                };
                let session_id = value
                    .get("session_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let valid_until = value
                    .get("access")
                    .and_then(|a| a.get("valid_until"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let accounts = bank_sync::parse_session_accounts(&value, &doc.document);
                if accounts.is_empty() {
                    self.error = Some("la banque n'a donné accès à aucun compte (avez-vous relié vos comptes dans le panneau d'Enable Banking ?)".into());
                    return Outcome::Keep;
                }
                // Reconnexion : les comptes déjà associés le restent.
                let previous =
                    self.sync.connections.iter().position(|c| {
                        c.bank == pending.bank.name && c.country == pending.bank.country
                    });
                let mut connection = BankConnection {
                    session_id,
                    bank: pending.bank.name.clone(),
                    country: pending.bank.country.clone(),
                    valid_until,
                    accounts,
                };
                if let Some(k) = previous {
                    for link in &mut connection.accounts {
                        if let Some(old) = self.sync.connections[k].accounts.iter().find(|o| {
                            o.uid == link.uid || (!o.iban.is_empty() && o.iban == link.iban)
                        }) {
                            link.target = old.target.clone();
                        }
                    }
                    self.sync.connections[k] = connection;
                } else {
                    self.sync.connections.push(connection);
                }
                self.save_sync(doc);
                self.pasted.clear();
                self.page = Page::Connections;
                self.message = Some(format!(
                    "Banque « {} » connectée. Choisissez le compte BankPerfect de chaque compte.",
                    pending.bank.name
                ));
            }
            Ok(JobResult::Transactions(results)) => {
                let total: usize = results.iter().map(|(_, _, ops)| ops.len()).sum();
                if total == 0 {
                    self.message = Some("Aucune nouvelle opération.".into());
                    return Outcome::Keep;
                }
                let targets = results.iter().map(|(_, t, _)| Some(*t)).collect();
                let accounts = results
                    .into_iter()
                    .map(|(link, _, ops)| bank_sync::imported_account(&link, ops))
                    .collect();
                let parsed = ParsedImport {
                    accounts,
                    ..Default::default()
                };
                let form = crate::import_ui::ImportForm::from_parsed(
                    "Synchronisation bancaire".into(),
                    parsed,
                    targets,
                );
                return Outcome::Switch(Box::new(Tool::Import(Box::new(form))));
            }
        }
        Outcome::Keep
    }

    // ─────────────────────────── Configuration ───────────────────────────

    fn setup(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, width: f32, pal: &Palette) {
        let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
        ui_kit::section(
            ui,
            width,
            icon::LIST_NUMBERS,
            "Une seule fois : votre application Enable Banking",
            |ui| {
                let step = |ui: &mut egui::Ui, n: &str, text: &str| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(n)
                                .family(crate::theme::semibold())
                                .color(pal.accent),
                        );
                        ui.label(text);
                    });
                    ui.add_space(4.0);
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("1.")
                            .family(crate::theme::semibold())
                            .color(pal.accent),
                    );
                    ui.label("Créez un compte gratuit sur");
                    ui.hyperlink_to("enablebanking.com", "https://enablebanking.com/sign-in/");
                    ui.label("(offre gratuite réservée à l'usage personnel).");
                });
                ui.add_space(4.0);
                step(
                    ui,
                    "2.",
                    "Dans « API applications », enregistrez une application de type Production et laissez le navigateur générer la clé : un fichier .pem est téléchargé.",
                );
                step(
                    ui,
                    "3.",
                    "Comme adresse de redirection (« redirect URL »), indiquez celle ci-dessous, à l'identique.",
                );
                step(
                    ui,
                    "4.",
                    "Toujours sur Enable Banking, reliez vos comptes bancaires à l'application (« Link accounts »).",
                );
                step(
                    ui,
                    "5.",
                    "Ici, collez l'identifiant de l'application et choisissez le fichier .pem.",
                );
            },
        );
        ui_kit::section(ui, width, icon::KEY, "Application", |ui| {
            ui.add(
                crate::theme::field(&mut self.app_id)
                    .hint_text("Identifiant de l'application (aaaaaaaa-bbbb-…)")
                    .desired_width(inner - 20.0),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    crate::theme::field(&mut self.redirect)
                        .hint_text("Adresse de redirection HTTPS")
                        .desired_width(inner - 150.0),
                );
                if ui.button(format!("{}  Copier", icon::COPY)).clicked() {
                    ui.ctx().copy_text(self.redirect.trim().to_string());
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .button(format!("{}  Choisir le fichier de clé (.pem)", icon::FILE))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Clé privée", &["pem", "key"])
                        .pick_file()
                {
                    match std::fs::read(&path) {
                        Ok(bytes) if String::from_utf8_lossy(&bytes).contains("PRIVATE KEY") => {
                            self.key_name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            self.key = Some(bytes);
                            if self.app_id.trim().is_empty()
                                && let Some(stem) =
                                    path.file_stem().map(|s| s.to_string_lossy().into_owned())
                                && stem.len() == 36
                            {
                                // Le fichier porte le nom de l'application.
                                self.app_id = stem;
                            }
                        }
                        Ok(_) => {
                            self.error = Some("ce fichier ne contient pas de clé privée".into())
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if !self.key_name.is_empty() {
                    ui.label(
                        RichText::new(format!("{}  {}", icon::CHECK, self.key_name))
                            .color(pal.positive),
                    );
                }
            });
        });
        let ready = !self.app_id.trim().is_empty()
            && self.key.is_some()
            && self.redirect.trim().starts_with("https://");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    ready && self.job.is_none(),
                    crate::theme::primary_button(
                        ui,
                        format!("{}  Enregistrer et tester", icon::PLUGS_CONNECTED),
                    ),
                )
                .clicked()
            {
                let client = Client {
                    application_id: self.app_id.trim().to_string(),
                    key: Arc::new(self.key.clone().unwrap_or_default()),
                };
                self.client = Some(client.clone());
                self.start(
                    ctx,
                    "Test de la connexion à Enable Banking…",
                    move || {
                        client
                            .call("GET", "/aspsps?country=FR", None)
                            .map(|_| JobResult::Tested)
                    },
                );
            }
            if self.client.is_some() && ui.button("Annuler").clicked() {
                self.page = Page::Connections;
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new("La clé reste dans votre dossier de configuration, lisible par vous seul. Les connexions aux banques (sans identifiants) sont gardées dans le fichier de comptes.")
                .size(12.5)
                .color(pal.muted),
        );
    }

    // ─────────────────────────── Connexions ───────────────────────────

    fn importable(&self, doc: &OpenDoc) -> bool {
        let today = Date::today();
        self.sync
            .connections
            .iter()
            .filter(|c| !c.expired(today))
            .flat_map(|c| &c.accounts)
            .any(|l| bank_sync::target_index(&doc.document, l).is_some())
    }

    fn import(&mut self, ctx: &egui::Context, doc: &OpenDoc) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let today = Date::today();
        let mut work: Vec<(BankAccountLink, usize, Date)> = Vec::new();
        for c in self.sync.connections.iter().filter(|c| !c.expired(today)) {
            for link in &c.accounts {
                if let Some(target) = bank_sync::target_index(&doc.document, link) {
                    work.push((
                        link.clone(),
                        target,
                        bank_sync::sync_start(&doc.document, target, today),
                    ));
                }
            }
        }
        self.start(ctx, "Téléchargement des opérations…", move || {
            let mut results = Vec::new();
            for (link, target, from) in work {
                let mut operations = Vec::new();
                let mut continuation: Option<String> = None;
                loop {
                    let mut path = format!(
                        "/accounts/{}/transactions?date_from={}",
                        link.uid,
                        from.to_naive().format("%Y-%m-%d")
                    );
                    if let Some(key) = &continuation {
                        path.push_str(&format!("&continuation_key={key}"));
                    }
                    let page = client
                        .call("GET", &path, None)
                        .map_err(|e| format!("{} : {e}", link.name))?;
                    let (ops, next) = bank_sync::parse_transactions(&page);
                    operations.extend(ops);
                    match next {
                        Some(key) if operations.len() < 20_000 => continuation = Some(key),
                        _ => break,
                    }
                }
                results.push((link, target, operations));
            }
            Ok(JobResult::Transactions(results))
        });
    }

    fn connections(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        doc: &mut OpenDoc,
        width: f32,
        pal: &Palette,
    ) {
        let today = Date::today();
        let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
        let names: Vec<(String, String)> = doc
            .document
            .account_order
            .iter()
            .map(|&i| {
                (
                    doc.document.accounts[i].uid.clone(),
                    doc.document.accounts[i].name.clone(),
                )
            })
            .collect();
        let mut changed = false;
        let mut remove = None;
        let mut reconnect = None;
        egui::ScrollArea::vertical()
            .id_salt("bank_connections")
            .max_height(ui_kit::dialog_height(ctx, 0.5))
            .show(ui, |ui| {
                for (k, connection) in self.sync.connections.iter_mut().enumerate() {
                    let expired = connection.expired(today);
                    ui_kit::card(ui, width - 16.0, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}  {}", icon::BANK, connection.bank))
                                    .family(crate::theme::semibold()),
                            );
                            let until = bank_sync::valid_until_date(&connection.valid_until)
                                .map(|d| d.to_string())
                                .unwrap_or_default();
                            if expired {
                                ui_kit::pill(
                                    ui,
                                    &format!("consentement expiré le {until}"),
                                    pal.negative,
                                );
                            } else if !until.is_empty() {
                                ui_kit::pill(
                                    ui,
                                    &format!("autorisée jusqu'au {until}"),
                                    pal.positive,
                                );
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(icon::TRASH).color(pal.muted),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text("Oublier cette connexion")
                                        .clicked()
                                    {
                                        remove = Some(k);
                                    }
                                    if ui
                                        .button(format!("{}  Reconnecter", icon::ARROWS_CLOCKWISE))
                                        .on_hover_text(
                                            "Renouveler le consentement sur le site de la banque",
                                        )
                                        .clicked()
                                    {
                                        reconnect = Some(k);
                                    }
                                },
                            );
                        });
                        ui.add_space(6.0);
                        for link in &mut connection.accounts {
                            ui.horizontal(|ui| {
                                let label = if link.iban.is_empty() {
                                    link.name.clone()
                                } else {
                                    format!("{} · {}", link.name, mask_iban(&link.iban))
                                };
                                ui_kit::cell(
                                    ui,
                                    inner * 0.5,
                                    egui::Align::Min,
                                    RichText::new(label).color(pal.fg_dark),
                                );
                                ui.label(RichText::new(icon::ARROW_RIGHT).color(pal.muted));
                                let current = link
                                    .target
                                    .as_ref()
                                    .and_then(|uid| names.iter().find(|(u, _)| u == uid))
                                    .map(|(_, n)| n.clone());
                                egui::ComboBox::from_id_salt(("bank_target", &link.uid))
                                    .selected_text(
                                        current.unwrap_or_else(|| "Ne pas importer".into()),
                                    )
                                    .width(inner * 0.4)
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                link.target.is_none(),
                                                "Ne pas importer",
                                            )
                                            .clicked()
                                        {
                                            link.target = None;
                                            changed = true;
                                        }
                                        for (uid, name) in &names {
                                            if ui
                                                .selectable_label(
                                                    link.target.as_deref() == Some(uid.as_str()),
                                                    format!("{}  {name}", icon::BANK),
                                                )
                                                .clicked()
                                            {
                                                link.target = Some(uid.clone());
                                                changed = true;
                                            }
                                        }
                                    });
                            });
                        }
                    });
                    ui.add_space(8.0);
                }
            });
        ui.label(
            RichText::new("L'import reprend quelques jours avant la dernière opération de chaque compte (90 jours pour un compte vide) ; l'aperçu écarte les doublons avant tout ajout.")
                .size(12.5)
                .color(pal.muted),
        );
        if changed {
            self.save_sync(doc);
        }
        if let Some(k) = remove {
            let connection = self.sync.connections.remove(k);
            self.save_sync(doc);
            if let Some(client) = self.client.clone() {
                // Révocation chez Enable Banking, sans attendre la réponse.
                std::thread::spawn(move || {
                    let _ = client.call(
                        "DELETE",
                        &format!("/sessions/{}", connection.session_id),
                        None,
                    );
                });
            }
        }
        if let Some(k) = reconnect {
            let c = &self.sync.connections[k];
            let bank = Bank {
                name: c.bank.clone(),
                country: c.country.clone(),
                max_consent_seconds: None,
            };
            self.begin_auth(ctx, bank);
            self.page = Page::AddBank;
        }
        let _ = ctx;
    }

    // ─────────────────────────── Ajout d'une banque ───────────────────────────

    fn load_banks(&mut self, ctx: &egui::Context) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.banks_country = self.country.clone();
        self.banks.clear();
        let country = self.country.clone();
        self.start(ctx, "Liste des banques…", move || {
            client
                .call(
                    "GET",
                    &format!("/aspsps?country={country}&psu_type=personal"),
                    None,
                )
                .map(|v| JobResult::Banks(bank_sync::parse_banks(&v)))
        });
    }

    fn begin_auth(&mut self, ctx: &egui::Context, bank: Bank) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some((config, _)) = load_config() else {
            self.page = Page::Setup;
            return;
        };
        let state = random_state();
        // Consentement le plus long accepté par la banque (180 jours au plus).
        let seconds = bank
            .max_consent_seconds
            .unwrap_or(90 * 86_400)
            .clamp(86_400, 180 * 86_400);
        let valid_until =
            (chrono::Utc::now() + chrono::Duration::seconds(seconds - 60)).to_rfc3339();
        let body = json!({
            "access": { "valid_until": valid_until },
            "aspsp": { "name": bank.name, "country": bank.country },
            "state": state,
            "redirect_url": config.redirect_url,
            "psu_type": "personal",
        });
        self.pending = Some(PendingAuth {
            bank,
            state,
            url: String::new(),
        });
        self.pasted.clear();
        self.start(
            ctx,
            "Préparation de la connexion à la banque…",
            move || {
                let value = client.call("POST", "/auth", Some(&body))?;
                value
                    .get("url")
                    .and_then(Value::as_str)
                    .map(|u| JobResult::AuthUrl(u.to_string()))
                    .ok_or_else(|| "réponse inattendue d'Enable Banking".into())
            },
        );
    }

    fn add_bank(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, width: f32, _pal: &Palette) {
        let inner = width - 2.0 * ui_kit::CARD_PADDING - 2.0;
        if let Some(pending) = &self.pending {
            let bank = pending.bank.name.clone();
            let url = pending.url.clone();
            let state = pending.state.clone();
            let mut submit = false;
            ui_kit::section(
                ui,
                width,
                icon::GLOBE,
                &format!("Connexion à « {bank} »"),
                |ui| {
                    ui.label("1. Authentifiez-vous sur le site de votre banque, ouvert dans votre navigateur, et autorisez l'accès à vos comptes.");
                    if !url.is_empty() {
                        ui.horizontal(|ui| {
                            if ui
                                .link(format!(
                                    "{}  Rouvrir la page de la banque",
                                    icon::ARROW_SQUARE_OUT
                                ))
                                .clicked()
                            {
                                let _ = open::that(&url);
                            }
                            if ui
                                .small_button(format!("{}  Copier l'adresse", icon::COPY))
                                .clicked()
                            {
                                ui.ctx().copy_text(url.clone());
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.label("2. La banque vous renvoie ensuite vers la page de redirection : copiez toute l'adresse affichée par le navigateur (elle contient « code= ») et collez-la ici.");
                    ui.add_space(4.0);
                    let field = ui.add(
                        crate::theme::field(&mut self.pasted)
                            .hint_text("https://…?state=…&code=…")
                            .desired_width(inner - 20.0),
                    );
                    if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        submit = true;
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.pasted.trim().is_empty() && self.job.is_none(),
                                crate::theme::primary_button(
                                    ui,
                                    format!("{}  Valider", icon::CHECK),
                                ),
                            )
                            .clicked()
                        {
                            submit = true;
                        }
                        if ui.button("Annuler").clicked() {
                            self.pending = None;
                        }
                    });
                },
            );
            if submit && self.job.is_none() {
                match bank_sync::authorization_code(&self.pasted, &state) {
                    Ok(code) => {
                        if let Some(client) = self.client.clone() {
                            self.start(ctx, "Ouverture de la session…", move || {
                                client
                                    .call("POST", "/sessions", Some(&json!({ "code": code })))
                                    .map(JobResult::Session)
                            });
                        }
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            return;
        }
        ui_kit::section(
            ui,
            width,
            icon::MAGNIFYING_GLASS,
            "Choisir la banque",
            |ui| {
                ui.horizontal(|ui| {
                    let label = COUNTRIES
                        .iter()
                        .find(|(c, _)| *c == self.country)
                        .map_or(self.country.clone(), |(_, n)| n.to_string());
                    egui::ComboBox::from_id_salt("bank_country")
                        .selected_text(label)
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            for (code, name) in COUNTRIES {
                                ui.selectable_value(&mut self.country, code.to_string(), name);
                            }
                        });
                    ui.add(
                        crate::theme::field(&mut self.search)
                            .hint_text(format!("{}  Rechercher une banque", icon::MAGNIFYING_GLASS))
                            .desired_width(inner - 200.0),
                    );
                });
                ui.add_space(8.0);
                let key = bp_core::currencies::search_key(&self.search);
                let matches: Vec<usize> = (0..self.banks.len())
                    .filter(|&i| {
                        key.is_empty()
                            || bp_core::currencies::search_key(&self.banks[i].name).contains(&key)
                    })
                    .collect();
                egui::ScrollArea::vertical()
                    .id_salt("bank_list")
                    .max_height(260.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(inner - 16.0);
                        for i in matches {
                            if ui
                                .selectable_label(
                                    self.selected == Some(i),
                                    format!("{}  {}", icon::BANK, self.banks[i].name),
                                )
                                .clicked()
                            {
                                self.selected = Some(i);
                            }
                        }
                    });
                ui.add_space(8.0);
                let chosen = self.selected.and_then(|i| self.banks.get(i)).cloned();
                if ui
                    .add_enabled(
                        chosen.is_some() && self.job.is_none(),
                        crate::theme::primary_button(
                            ui,
                            format!("{}  Se connecter à cette banque", icon::ARROW_SQUARE_OUT),
                        ),
                    )
                    .clicked()
                    && let Some(bank) = chosen
                {
                    self.begin_auth(ctx, bank);
                }
            },
        );
        if ui
            .link(format!("{}  Configuration Enable Banking", icon::GEAR))
            .clicked()
        {
            self.page = Page::Setup;
        }
    }
}

/// `FR76 3000 6000 0112 3456 7890 189` → `FR76 •••• 0189`.
fn mask_iban(iban: &str) -> String {
    let compact: String = iban.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 8 {
        return compact;
    }
    format!("{} •••• {}", &compact[..4], &compact[compact.len() - 4..])
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clé RSA de test, sans valeur (générée pour ce test).
    const TEST_KEY: &str = include_str!("../tests/data/test-rsa.pem");

    #[test]
    fn signs_a_token() {
        let client = Client {
            application_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
            key: Arc::new(TEST_KEY.as_bytes().to_vec()),
        };
        let token = client.token().expect("jeton signé");
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header = jsonwebtoken::decode_header(&token).unwrap();
        assert_eq!(
            header.kid.as_deref(),
            Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        );
        assert_eq!(header.alg, jsonwebtoken::Algorithm::RS256);
        assert_eq!(
            mask_iban("FR76 3000 6000 0112 3456 7890 189"),
            "FR76 •••• 0189"
        );
        assert!(random_state().starts_with("bp"));
    }

    /// Appel réel (réseau) avec une application inconnue : refus attendu,
    /// avec le message d'Enable Banking. `cargo test -- --ignored enable_banking`.
    #[test]
    #[ignore]
    fn enable_banking_refuses_an_unknown_application() {
        let client = Client {
            application_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
            key: Arc::new(TEST_KEY.as_bytes().to_vec()),
        };
        let error = client
            .call("GET", "/aspsps?country=FR", None)
            .expect_err("application inconnue");
        println!("{error}");
        assert!(error.contains("40"), "{error}");
    }
}
