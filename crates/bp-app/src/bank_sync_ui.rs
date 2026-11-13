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
use bp_core::import::{ImportedOperation, ParsedImport, StatementBalance};
use bp_core::model::Date;
use bp_core::{tr, tr_mark};
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
/// Pages demandées à l'enregistrement de l'application.
const PRIVACY_URL: &str = "https://github.com/fabiochelly/bankperfect/blob/main/PRIVACY.md";
const TERMS_URL: &str = "https://github.com/fabiochelly/bankperfect/blob/main/LICENSE";
const COUNTRIES: [(&str, &str); 16] = [
    ("FR", tr_mark!("France")),
    ("BE", tr_mark!("Belgique")),
    ("LU", tr_mark!("Luxembourg")),
    ("DE", tr_mark!("Allemagne")),
    ("ES", tr_mark!("Espagne")),
    ("IT", tr_mark!("Italie")),
    ("NL", tr_mark!("Pays-Bas")),
    ("PT", tr_mark!("Portugal")),
    ("AT", tr_mark!("Autriche")),
    ("IE", tr_mark!("Irlande")),
    ("FI", tr_mark!("Finlande")),
    ("SE", tr_mark!("Suède")),
    ("DK", tr_mark!("Danemark")),
    ("NO", tr_mark!("Norvège")),
    ("PL", tr_mark!("Pologne")),
    ("EE", tr_mark!("Estonie")),
];

/// Une application Enable Banking de l'utilisateur.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Config {
    application_id: String,
    redirect_url: String,
    /// Nom donné par l'utilisateur (facultatif).
    #[serde(default)]
    name: String,
}

impl Config {
    /// Nom affiché : celui de l'utilisateur, sinon le début de l'identifiant.
    fn label(&self) -> String {
        if self.name.trim().is_empty() {
            let short: String = self.application_id.chars().take(8).collect();
            format!("{short}…")
        } else {
            self.name.trim().to_string()
        }
    }
}

/// Contenu de `banque.json` : les applications et celle en cours. Avant la
/// gestion de plusieurs applications, le fichier ne décrivait qu'une
/// application, à la racine, avec sa clé dans `banque-cle.pem`.
#[derive(Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    applications: Vec<Config>,
    /// Identifiant de l'application en cours.
    #[serde(default)]
    current: String,
    #[serde(default, skip_serializing)]
    application_id: String,
    #[serde(default, skip_serializing)]
    redirect_url: String,
}

/// Applications décrites par `banque.json`, la position de celle en cours,
/// et vrai si le fichier est à l'ancien format (une seule application).
fn parse_store(json: &[u8]) -> (Vec<Config>, usize, bool) {
    let store: Store = serde_json::from_slice(json).unwrap_or_default();
    if store.applications.is_empty() && !store.application_id.trim().is_empty() {
        let legacy = Config {
            application_id: store.application_id,
            redirect_url: store.redirect_url,
            name: String::new(),
        };
        return (vec![legacy], 0, true);
    }
    let current = store
        .applications
        .iter()
        .position(|c| c.application_id == store.current)
        .unwrap_or(0);
    (store.applications, current, false)
}

/// Fichier de la clé privée d'une application.
fn key_file(application_id: &str) -> String {
    let id: String = application_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    format!("banque-cle-{id}.pem")
}

/// Applications configurées sur cet ordinateur (avec leur clé) et position
/// de celle en cours.
fn load_apps() -> (Vec<(Config, Vec<u8>)>, usize) {
    let read = |name: &str| crate::settings::config_file(name).and_then(|p| std::fs::read(p).ok());
    let Some(json) = read(CONFIG_FILE) else {
        return (Vec::new(), 0);
    };
    let (configs, current, legacy) = parse_store(&json);
    let current_id = configs.get(current).map(|c| c.application_id.clone());
    let apps: Vec<(Config, Vec<u8>)> = configs
        .into_iter()
        .filter_map(|c| {
            let key = read(&key_file(&c.application_id))
                .or_else(|| legacy.then(|| read(KEY_FILE)).flatten())?;
            Some((c, key))
        })
        .collect();
    let current = apps
        .iter()
        .position(|(c, _)| Some(&c.application_id) == current_id.as_ref())
        .unwrap_or(0);
    (apps, current)
}

fn save_apps(apps: &[(Config, Vec<u8>)], current: usize) -> Result<(), String> {
    let store = Store {
        applications: apps.iter().map(|(c, _)| c.clone()).collect(),
        current: apps
            .get(current)
            .map(|(c, _)| c.application_id.clone())
            .unwrap_or_default(),
        ..Default::default()
    };
    let json = serde_json::to_vec_pretty(&store).map_err(|e| e.to_string())?;
    for (config, key) in apps {
        let name = key_file(&config.application_id);
        crate::settings::write_config_file(&name, key)?;
        // Clé privée lisible par l'utilisateur seul.
        #[cfg(unix)]
        if let Some(path) = crate::settings::config_file(&name) {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
    crate::settings::write_config_file(CONFIG_FILE, &json)
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
            .map_err(|e| tr!("clé privée illisible : {}", e))?;
        jsonwebtoken::encode(&header, &claims, &key)
            .map_err(|e| tr!("signature impossible : {}", e))
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
        let mut response = response.map_err(|e| tr!("connexion impossible : {}", e))?;
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
                401 | 403 => tr!(
                    "accès refusé par Enable Banking ({status}) : {detail}",
                    status = status,
                    detail = detail
                ),
                _ => tr!(
                    "erreur {status} : {detail}",
                    status = status,
                    detail = detail
                ),
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
    Transactions(
        Vec<(
            BankAccountLink,
            usize,
            Vec<ImportedOperation>,
            Option<StatementBalance>,
        )>,
    ),
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
    /// Client de l'application en cours.
    client: Option<Client>,
    /// Applications configurées sur cet ordinateur, et celle en cours.
    apps: Vec<(Config, Vec<u8>)>,
    current: usize,
    // Configuration (application en cours, ou nouvelle application).
    app_name: String,
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
        let (apps, current) = load_apps();
        let loaded = apps.get(current).cloned();
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
            apps,
            current,
            app_name: config.name,
            app_id: config.application_id,
            redirect: if config.redirect_url.is_empty() {
                DEFAULT_REDIRECT.into()
            } else {
                config.redirect_url
            },
            key_name: if key.is_some() {
                tr!("clé enregistrée").into()
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

    /// Passe à l'application `k` : elle sert aux nouvelles connexions.
    fn select_app(&mut self, k: usize) {
        let Some((config, key)) = self.apps.get(k).cloned() else {
            return;
        };
        self.current = k;
        self.client = Some(Client {
            application_id: config.application_id.clone(),
            key: Arc::new(key.clone()),
        });
        self.app_name = config.name;
        self.app_id = config.application_id;
        self.redirect = config.redirect_url;
        self.key = Some(key);
        self.key_name = tr!("clé enregistrée").into();
    }

    /// Client de l'application `application` d'une connexion (vide :
    /// l'application en cours).
    fn client_for(&self, application: &str) -> Option<Client> {
        if application.is_empty() {
            return self.client.clone();
        }
        self.apps
            .iter()
            .find(|(c, _)| c.application_id == application)
            .map(|(c, k)| Client {
                application_id: c.application_id.clone(),
                key: Arc::new(k.clone()),
            })
    }

    /// Choix de l'application (s'il y en a plusieurs) et ajout d'une autre.
    fn app_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .link(format!(
                    "{}  {}",
                    icon::GEAR,
                    tr!("Configuration Enable Banking")
                ))
                .clicked()
            {
                self.select_app(self.current);
                self.page = Page::Setup;
            }
            if self.apps.len() > 1 {
                let mut chosen = self.current;
                let label = self.apps[self.current].0.label();
                ui_kit::Combo::new("bank_sync_app")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (k, (config, _)) in self.apps.iter().enumerate() {
                            ui.selectable_value(&mut chosen, k, config.label());
                        }
                    });
                if chosen != self.current {
                    self.select_app(chosen);
                    let _ = save_apps(&self.apps, self.current);
                }
            }
            if ui
                .link(format!(
                    "{}  {}",
                    icon::PLUS,
                    tr!("Ajouter une application")
                ))
                .on_hover_text(tr!(
                    "Une autre application Enable Banking, par exemple pour les comptes d'un proche."
                ))
                .clicked()
            {
                self.app_name.clear();
                self.app_id.clear();
                self.key = None;
                self.key_name.clear();
                self.page = Page::Setup;
            }
        });
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
            crate::dialogs::header(ui, icon::BANK, pal.accent, tr!("Synchronisation bancaire"));
            ui_kit::subtitle(
                ui,
                tr!("Importe les opérations de vos comptes par Enable Banking, prestataire agréé de la directive européenne DSP2. Vous vous authentifiez sur le site de votre banque : vos identifiants ne passent jamais par BankPerfect."),
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
                        if crate::dialogs::primary(ui, &format!("{}  {}", icon::DOWNLOAD_SIMPLE, tr!("Importer les opérations")), ready && !busy).clicked() {
                            self.import(ctx, doc);
                        }
                        if crate::dialogs::button(ui, format!("{}  {}", icon::PLUS, tr!("Ajouter une banque"))).clicked() && !busy {
                            self.page = Page::AddBank;
                            self.pending = None;
                        }
                    }
                    Page::AddBank if !self.sync.connections.is_empty() => {
                        let back = crate::dialogs::button(ui, format!("{}  {}", icon::ARROW_LEFT, tr!("Connexions"))).clicked();
                        if back && !busy {
                            self.page = Page::Connections;
                            self.pending = None;
                        }
                    }
                    _ => {}
                }
                if crate::dialogs::button(ui, tr!("Fermer")).clicked() || (ui.input(|i| i.key_pressed(Key::Escape)) && !busy) {
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
                    name: self.app_name.trim().to_string(),
                };
                let saved = self
                    .key
                    .clone()
                    .ok_or(tr!("clé manquante").to_string())
                    .and_then(|key| {
                        // Application déjà connue : mise à jour ; sinon ajout.
                        let known = self
                            .apps
                            .iter()
                            .position(|(c, _)| c.application_id == config.application_id);
                        match known {
                            Some(k) => {
                                self.apps[k] = (config, key);
                                self.current = k;
                            }
                            None => {
                                self.apps.push((config, key));
                                self.current = self.apps.len() - 1;
                            }
                        }
                        save_apps(&self.apps, self.current)
                    });
                match saved {
                    Ok(()) => {
                        self.message =
                            Some(tr!("Configuration enregistrée : Enable Banking répond.").into());
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
                    self.error = Some(tr!("aucune banque disponible pour ce pays").into());
                }
            }
            Ok(JobResult::AuthUrl(url)) => {
                if let Some(pending) = &mut self.pending {
                    pending.url = url.clone();
                }
                if open::that_detached(&url).is_err() {
                    self.message =
                        Some(tr!("Ouvrez l'adresse ci-dessous dans votre navigateur.").into());
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
                    self.error = Some(tr!("la banque n'a donné accès à aucun compte (avez-vous relié vos comptes dans le panneau d'Enable Banking ?)").into());
                    return Outcome::Keep;
                }
                let application = self
                    .apps
                    .get(self.current)
                    .map(|(c, _)| c.application_id.clone())
                    .unwrap_or_default();
                // Reconnexion : les comptes déjà associés le restent. La même
                // banque peut être reliée par deux applications.
                let previous = self.sync.connections.iter().position(|c| {
                    c.bank == pending.bank.name
                        && c.country == pending.bank.country
                        && (c.application.is_empty() || c.application == application)
                });
                let mut connection = BankConnection {
                    session_id,
                    bank: pending.bank.name.clone(),
                    country: pending.bank.country.clone(),
                    valid_until,
                    accounts,
                    application: application.clone(),
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
                self.message = Some(tr!(
                    "Banque « {} » connectée. Choisissez le compte BankPerfect de chaque compte.",
                    pending.bank.name
                ));
            }
            Ok(JobResult::Transactions(results)) => {
                let total: usize = results.iter().map(|(_, _, ops, _)| ops.len()).sum();
                if total == 0 {
                    self.message = Some(tr!("Aucune nouvelle opération.").into());
                    return Outcome::Keep;
                }
                let targets = results.iter().map(|(_, t, _, _)| Some(*t)).collect();
                let accounts = results
                    .into_iter()
                    .map(|(link, _, ops, balance)| bank_sync::imported_account(&link, ops, balance))
                    .collect();
                let parsed = ParsedImport {
                    accounts,
                    ..Default::default()
                };
                let form = crate::import_ui::ImportForm::from_parsed(
                    tr!("Synchronisation bancaire").into(),
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
        let redirect = self.redirect.trim().to_string();
        ui_kit::section(
            ui,
            width,
            icon::LIST_NUMBERS,
            tr!("Une seule fois : votre application Enable Banking"),
            |ui| {
                let number = |ui: &mut egui::Ui, n: &str| {
                    ui.label(
                        RichText::new(n)
                            .family(crate::theme::semibold())
                            .color(pal.accent),
                    );
                };
                let step = |ui: &mut egui::Ui, n: &str, text: &str| {
                    ui.horizontal_wrapped(|ui| {
                        number(ui, n);
                        ui.label(text);
                    });
                    ui.add_space(4.0);
                };
                ui.horizontal_wrapped(|ui| {
                    number(ui, "1.");
                    ui.label(tr!("Créez un compte sur"));
                    ui.hyperlink_to("enablebanking.com", "https://enablebanking.com/sign-in/");
                    ui.label(tr!(
                        "(gratuit pour un usage personnel) : il suffit de votre adresse e-mail, un lien de connexion vous est envoyé."
                    ));
                });
                ui.add_space(4.0);
                step(
                    ui,
                    "2.",
                    tr!(
                        "Dans « API applications », remplissez le formulaire « Add a new application » ainsi, en gardant l'option proposée pour la clé privée :"
                    ),
                );
                // Champs du formulaire : libellé, valeur, à copier.
                let fields: [(&str, &str, bool); 7] = [
                    ("Environment", "Production", false),
                    ("Application name", "BankPerfect", false),
                    ("Redirect URLs", &redirect, true),
                    ("Description", tr!("Usage personnel"), false),
                    ("Data protection email", tr!("votre adresse e-mail"), false),
                    ("Privacy policy URL", PRIVACY_URL, true),
                    ("Terms of service URL", TERMS_URL, true),
                ];
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    egui::Grid::new("bank_sync_form")
                        .num_columns(2)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            for (label, value, copy) in fields {
                                ui.label(RichText::new(label).color(pal.fg_dark));
                                ui.horizontal(|ui| {
                                    if copy {
                                        ui.label(RichText::new(value).monospace());
                                        if ui
                                            .small_button(icon::COPY)
                                            .on_hover_text(tr!("Copier"))
                                            .clicked()
                                        {
                                            ui.ctx().copy_text(value.to_string());
                                        }
                                    } else {
                                        ui.label(value);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(8.0);
                step(
                    ui,
                    "3.",
                    tr!(
                        "Cliquez sur « Register » : le navigateur génère la clé privée et la télécharge dans un fichier .pem nommé d'après l'identifiant de l'application. Gardez ce fichier, c'est la seule copie de la clé."
                    ),
                );
                step(
                    ui,
                    "4.",
                    tr!(
                        "Sur la fiche de l'application, cliquez sur « Link accounts » et autorisez, sur le site de votre banque, l'accès à vos comptes. Seuls les comptes reliés seront accessibles."
                    ),
                );
                step(
                    ui,
                    "5.",
                    tr!(
                        "Ici, choisissez le fichier .pem (l'identifiant de l'application est rempli d'après son nom), puis « Enregistrer et tester »."
                    ),
                );
            },
        );
        ui_kit::section(ui, width, icon::KEY, tr!("Application"), |ui| {
            ui.add(
                crate::theme::field(&mut self.app_name)
                    .hint_text(tr!(
                        "Nom de l'application (facultatif : « Personnel », « Parents »…)"
                    ))
                    .desired_width(inner - 20.0),
            );
            ui.add_space(8.0);
            ui.add(
                crate::theme::field(&mut self.app_id)
                    .hint_text(tr!("Identifiant de l'application (aaaaaaaa-bbbb-…)"))
                    .desired_width(inner - 20.0),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    crate::theme::field(&mut self.redirect)
                        .hint_text(tr!("Adresse de redirection HTTPS"))
                        .desired_width(inner - 150.0),
                );
                if ui
                    .button(format!("{}  {}", icon::COPY, tr!("Copier")))
                    .clicked()
                {
                    ui.ctx().copy_text(self.redirect.trim().to_string());
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let pick = egui::Id::new("bank_sync_key");
                if ui
                    .button(format!(
                        "{}  {}",
                        icon::FILE,
                        tr!("Choisir le fichier de clé (.pem)")
                    ))
                    .clicked()
                {
                    let dialog =
                        rfd::FileDialog::new().add_filter(tr!("Clé privée"), &["pem", "key"]);
                    crate::file_picker::open(
                        ui.ctx(),
                        pick,
                        crate::file_picker::Kind::File,
                        dialog,
                    );
                }
                if let Some(path) = crate::file_picker::chosen(ui.ctx(), pick) {
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
                            self.error =
                                Some(tr!("ce fichier ne contient pas de clé privée").into())
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
                        format!(
                            "{}  {}",
                            icon::PLUGS_CONNECTED,
                            tr!("Enregistrer et tester")
                        ),
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
                    tr!("Test de la connexion à Enable Banking…"),
                    move || {
                        client
                            .call("GET", "/aspsps?country=FR", None)
                            .map(|_| JobResult::Tested)
                    },
                );
            }
            if !self.apps.is_empty() && ui.button(tr!("Annuler")).clicked() {
                // Les champs reviennent à l'application en cours.
                self.select_app(self.current);
                self.page = Page::Connections;
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr!("La clé reste dans votre dossier de configuration, lisible par vous seul. Les connexions aux banques (sans identifiants) sont gardées dans le fichier de comptes."))
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
        let today = Date::today();
        let mut work: Vec<(Client, BankAccountLink, usize, Date)> = Vec::new();
        for c in self.sync.connections.iter().filter(|c| !c.expired(today)) {
            let client = self.client_for(&c.application);
            for link in &c.accounts {
                if let Some(target) = bank_sync::target_index(&doc.document, link) {
                    let Some(client) = client.clone() else {
                        self.error = Some(tr!(
                            "l'application Enable Banking de la connexion « {} » n'est pas configurée sur cet ordinateur",
                            c.bank
                        ));
                        return;
                    };
                    work.push((
                        client,
                        link.clone(),
                        target,
                        bank_sync::sync_start(&doc.document, target, today),
                    ));
                }
            }
        }
        self.start(ctx, tr!("Téléchargement des opérations…"), move || {
            let mut results = Vec::new();
            for (client, link, target, from) in work {
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
                        .map_err(|e| tr!("{} : {}", link.name, e))?;
                    let (ops, next) = bank_sync::parse_transactions(&page);
                    operations.extend(ops);
                    match next {
                        Some(key) if operations.len() < 20_000 => continuation = Some(key),
                        _ => break,
                    }
                }
                // Solde facultatif : une banque qui ne le donne pas n'empêche
                // pas l'import.
                let balance = client
                    .call("GET", &format!("/accounts/{}/balances", link.uid), None)
                    .ok()
                    .and_then(|json| bank_sync::parse_balances(&json));
                results.push((link, target, operations, balance));
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
                let app_labels: Vec<(String, String)> = if self.apps.len() > 1 {
                    self.apps
                        .iter()
                        .map(|(c, _)| (c.application_id.clone(), c.label()))
                        .collect()
                } else {
                    Vec::new()
                };
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
                                    &tr!("consentement expiré le {}", until),
                                    pal.negative,
                                );
                            } else if !until.is_empty() {
                                ui_kit::pill(
                                    ui,
                                    &tr!("autorisée jusqu'au {}", until),
                                    pal.positive,
                                );
                            }
                            if let Some((_, label)) = app_labels
                                .iter()
                                .find(|(id, _)| *id == connection.application)
                            {
                                ui_kit::pill(ui, label, pal.accent);
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
                                        .on_hover_text(tr!("Oublier cette connexion"))
                                        .clicked()
                                    {
                                        remove = Some(k);
                                    }
                                    if ui
                                        .button(format!(
                                            "{}  {}",
                                            icon::ARROWS_CLOCKWISE,
                                            tr!("Reconnecter")
                                        ))
                                        .on_hover_text(tr!(
                                            "Renouveler le consentement sur le site de la banque"
                                        ))
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
                                crate::ui_kit::Combo::new(("bank_target", &link.uid))
                                    .selected_text(
                                        current.unwrap_or_else(|| tr!("Ne pas importer").into()),
                                    )
                                    .width(inner * 0.4)
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                link.target.is_none(),
                                                tr!("Ne pas importer"),
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
            RichText::new(tr!("L'import reprend quelques jours avant la dernière opération de chaque compte (90 jours pour un compte vide) ; l'aperçu écarte les doublons avant tout ajout."))
                .size(12.5)
                .color(pal.muted),
        );
        if changed {
            self.save_sync(doc);
        }
        if let Some(k) = remove {
            let connection = self.sync.connections.remove(k);
            self.save_sync(doc);
            if let Some(client) = self.client_for(&connection.application) {
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
            // Avec l'application qui avait ouvert la connexion.
            let application = self.sync.connections[k].application.clone();
            if let Some(app) = self
                .apps
                .iter()
                .position(|(c, _)| c.application_id == application)
            {
                self.select_app(app);
            }
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
        self.start(ctx, tr!("Liste des banques…"), move || {
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
        let Some((config, _)) = self.apps.get(self.current).cloned() else {
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
            tr!("Préparation de la connexion à la banque…"),
            move || {
                let value = client.call("POST", "/auth", Some(&body))?;
                value
                    .get("url")
                    .and_then(Value::as_str)
                    .map(|u| JobResult::AuthUrl(u.to_string()))
                    .ok_or_else(|| tr!("réponse inattendue d'Enable Banking").into())
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
                &tr!("Connexion à « {} »", bank),
                |ui| {
                    ui.label(tr!("1. Authentifiez-vous sur le site de votre banque, ouvert dans votre navigateur, et autorisez l'accès à vos comptes."));
                    if !url.is_empty() {
                        ui.horizontal(|ui| {
                            if ui
                                .link(format!(
                                    "{}  {}",
                                    icon::ARROW_SQUARE_OUT,
                                    tr!("Rouvrir la page de la banque")
                                ))
                                .clicked()
                            {
                                let _ = open::that_detached(&url);
                            }
                            if ui
                                .small_button(format!(
                                    "{}  {}",
                                    icon::COPY,
                                    tr!("Copier l'adresse")
                                ))
                                .clicked()
                            {
                                ui.ctx().copy_text(url.clone());
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.label(tr!("2. La banque vous renvoie ensuite vers la page de redirection : copiez toute l'adresse affichée par le navigateur (elle contient « code= ») et collez-la ici."));
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
                                    format!("{}  {}", icon::CHECK, tr!("Valider")),
                                ),
                            )
                            .clicked()
                        {
                            submit = true;
                        }
                        if ui.button(tr!("Annuler")).clicked() {
                            self.pending = None;
                        }
                    });
                },
            );
            if submit && self.job.is_none() {
                match bank_sync::authorization_code(&self.pasted, &state) {
                    Ok(code) => {
                        if let Some(client) = self.client.clone() {
                            self.start(ctx, tr!("Ouverture de la session…"), move || {
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
            tr!("Choisir la banque"),
            |ui| {
                ui.horizontal(|ui| {
                    let label = COUNTRIES
                        .iter()
                        .find(|(c, _)| *c == self.country)
                        .map_or(self.country.clone(), |(_, n)| {
                            bp_core::i18n::tr(n).to_string()
                        });
                    crate::ui_kit::Combo::new("bank_country")
                        .selected_text(label)
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            for (code, name) in COUNTRIES {
                                ui.selectable_value(
                                    &mut self.country,
                                    code.to_string(),
                                    bp_core::i18n::tr(name),
                                );
                            }
                        });
                    ui.add(
                        crate::theme::field(&mut self.search)
                            .hint_text(format!(
                                "{}  {}",
                                icon::MAGNIFYING_GLASS,
                                tr!("Rechercher une banque")
                            ))
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
                            format!(
                                "{}  {}",
                                icon::ARROW_SQUARE_OUT,
                                tr!("Se connecter à cette banque")
                            ),
                        ),
                    )
                    .clicked()
                    && let Some(bank) = chosen
                {
                    self.begin_auth(ctx, bank);
                }
            },
        );
        self.app_bar(ui);
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
    fn one_or_several_applications() {
        // Ancien fichier : une application à la racine.
        let (apps, current, legacy) =
            parse_store(br#"{"application_id": "aaa", "redirect_url": "https://x"}"#);
        assert_eq!((apps.len(), current, legacy), (1, 0, true));
        assert_eq!(apps[0].application_id, "aaa");
        assert_eq!(apps[0].label(), "aaa…");

        let store = Store {
            applications: vec![
                apps[0].clone(),
                Config {
                    application_id: "bbb".into(),
                    redirect_url: "https://y".into(),
                    name: "Parents".into(),
                },
            ],
            current: "bbb".into(),
            ..Default::default()
        };
        let json = serde_json::to_vec(&store).unwrap();
        let (apps, current, legacy) = parse_store(&json);
        assert_eq!((apps.len(), current, legacy), (2, 1, false));
        assert_eq!(apps[1].label(), "Parents");
        assert_eq!(key_file("ab-12/../x"), "banque-cle-ab-12x.pem");
    }

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
