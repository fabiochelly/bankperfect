//! Extensions de BankPerfect, écrites en Luau.
//!
//! Chaque extension est un fichier `nom.luau` ou un dossier `nom/` contenant
//! `init.luau`, dans le dossier des extensions. Elle tourne dans sa propre
//! machine virtuelle, en bac à sable (pas d'accès au disque ni au réseau
//! hors de l'API), avec une limite de temps et de mémoire.
//!
//! L'application prête le document le temps d'un appel ([`DocContext`]) et
//! reçoit en retour les [`Effects`] : modifications à inscrire dans
//! l'historique d'annulation, messages, sélection. Les boîtes de dialogue et
//! fenêtres ouvertes par une extension forment une pile ([`Engine::frames`])
//! que l'application affiche avec ses propres composants ; chaque réponse de
//! l'utilisateur relance la coroutine de l'extension.

mod api;
mod date;
mod features;
mod import;
mod objects;
mod session;
pub mod ui;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::thread::ThreadStatus;
use mlua::{Lua, MultiValue, Table, Thread, UserData, UserDataMethods, Value, VmState};

pub use session::{Changes, DocContext};
pub use ui::{UiEvent, Widget, Window};

use api::{Outbox, Registry};
use session::Session;

/// Temps maximal d'un appel (entre deux interactions de l'utilisateur).
const TIME_LIMIT: Duration = Duration::from_secs(20);
const MEMORY_LIMIT: usize = 512 << 20;
const PRELUDE: &str = include_str!("prelude.luau");

/// Services de l'application utilisés par les extensions.
pub trait Services {
    /// Fenêtre « Ouvrir » du système.
    fn pick_open(&self, title: &str, filters: &[(String, Vec<String>)]) -> Option<PathBuf>;
    /// Fenêtre « Enregistrer sous » du système.
    fn pick_save(
        &self,
        title: &str,
        name: &str,
        filters: &[(String, Vec<String>)],
    ) -> Option<PathBuf>;
    fn open_url(&self, url: &str);
}

/// Ce dont une commande a besoin pour être proposée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// Un fichier de comptes ouvert (par défaut).
    File,
    /// Des lignes sélectionnées.
    Selection,
    /// Rien : la commande marche sans fichier.
    Nothing,
}

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub id: String,
    pub title: String,
    /// Nom d'icône Phosphor (`"magnifying-glass"`).
    pub icon: Option<String>,
    /// Raccourci (`"Ctrl+Alt+D"`).
    pub shortcut: Option<String>,
    pub description: Option<String>,
    pub needs: Needs,
    /// Menus où la commande apparaît ([`MENUS`]) ; jamais vide
    /// (`"extensions"` par défaut).
    pub menus: Vec<String>,
}

/// Emplacements possibles d'une commande (`menu = …` dans `bp.command`) :
/// identifiant et libellé.
pub const MENUS: &[(&str, &str)] = &[
    ("file", "Fichier"),
    ("export", "Fichier › Exporter"),
    ("edit", "Édition"),
    ("view", "Affichage"),
    ("accounts", "Comptes"),
    ("operations", "Opérations"),
    ("reports", "Rapports"),
    ("tools", "Outils"),
    ("extensions", "Extensions"),
    ("grid", "clic droit sur les opérations"),
    ("account", "clic droit sur un compte"),
];

/// Évènements auxquels une extension peut réagir (`bp.on`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    FileOpened,
    /// Opération ajoutée par l'utilisateur : saisie, import, collage,
    /// ventilation… (reçoit l'opération).
    OperationAdded,
    /// Opération modifiée par l'utilisateur (reçoit l'opération et une copie
    /// de son état d'avant).
    OperationChanged,
    /// Opération supprimée par l'utilisateur (reçoit une copie).
    OperationDeleted,
    BeforeSave,
    AfterSave,
    /// Un autre compte est affiché (reçoit le compte).
    AccountSelected,
    /// La sélection de la grille a changé.
    SelectionChanged,
}

impl Event {
    /// Nom donné à `bp.on`.
    pub const NAMES: &[(&str, Event)] = &[
        ("file_opened", Event::FileOpened),
        ("operation_added", Event::OperationAdded),
        ("operation_changed", Event::OperationChanged),
        ("operation_deleted", Event::OperationDeleted),
        ("before_save", Event::BeforeSave),
        ("after_save", Event::AfterSave),
        ("account_selected", Event::AccountSelected),
        ("selection_changed", Event::SelectionChanged),
    ];
}

/// Sujet d'un évènement : opération (compte, identifiant) et, pour une
/// modification ou une suppression, son état d'avant ; compte affiché pour
/// `AccountSelected`.
#[derive(Debug, Clone)]
pub struct EventItem {
    pub account: usize,
    pub uid: u64,
    pub before: Option<bp_core::model::Operation>,
}

impl EventItem {
    pub fn line(account: usize, uid: u64) -> Self {
        Self {
            account,
            uid,
            before: None,
        }
    }
}

/// Boîte de dialogue demandée par une extension.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    Alert {
        title: String,
        message: String,
    },
    Confirm {
        title: String,
        message: String,
        ok: String,
        danger: bool,
    },
    Prompt {
        title: String,
        label: String,
        text: String,
        placeholder: Option<String>,
        ok: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Ok,
    Cancel,
}

/// Ce que l'application doit faire après un appel.
#[derive(Debug, Default)]
pub struct Effects {
    /// Modifications à inscrire dans l'historique, dans l'ordre.
    pub changes: Vec<Changes>,
    /// Données rangées dans le fichier modifiées (`bp.store`).
    pub feature_changed: bool,
    /// Compte à afficher et lignes à sélectionner (identifiants).
    pub select: Option<(usize, Vec<u64>)>,
    /// Filtres à appliquer à la grille (`bp.filter`).
    pub filters: Option<bp_core::filter::Filters>,
    pub toasts: Vec<String>,
    /// Erreurs à montrer (extension, message).
    pub errors: Vec<String>,
}

impl Effects {
    pub fn modified(&self) -> bool {
        !self.changes.is_empty() || self.feature_changed
    }
}

/// Coroutine d'une extension en cours (commande, gestionnaire, crochet).
struct Run {
    thread: Thread,
    label: String,
    tag: u64,
}

/// Élément de la pile des fenêtres d'extension.
enum Frame {
    Dialog {
        ext: usize,
        run: Option<Run>,
        dialog: Dialog,
    },
    Window {
        ext: usize,
        owner: Option<Run>,
        window: Box<Window>,
    },
}

/// Accès de l'application à une fenêtre de la pile.
pub enum FrameMut<'a> {
    Dialog(&'a mut Dialog),
    Window(&'a Window),
}

struct Vm {
    lua: Lua,
    registry: Rc<RefCell<Registry>>,
}

pub struct Extension {
    /// Nom du fichier ou du dossier, sans extension.
    pub id: String,
    /// Fichier principal.
    pub path: PathBuf,
    pub enabled: bool,
    /// Erreur au chargement.
    pub error: Option<String>,
    vm: Option<Vm>,
}

impl Extension {
    fn registry(&self) -> Option<std::cell::Ref<'_, Registry>> {
        self.vm.as_ref().map(|vm| vm.registry.borrow())
    }

    pub fn name(&self) -> String {
        self.registry()
            .and_then(|r| r.name.clone())
            .unwrap_or_else(|| self.id.clone())
    }

    pub fn description(&self) -> String {
        self.registry()
            .map(|r| r.description.clone())
            .unwrap_or_default()
    }

    pub fn version(&self) -> String {
        self.registry()
            .map(|r| r.version.clone())
            .unwrap_or_default()
    }

    pub fn author(&self) -> String {
        self.registry()
            .map(|r| r.author.clone())
            .unwrap_or_default()
    }

    pub fn permissions(&self) -> Vec<String> {
        self.registry()
            .map(|r| r.permissions.clone())
            .unwrap_or_default()
    }

    pub fn commands(&self) -> Vec<CommandInfo> {
        self.registry()
            .map(|r| r.commands.iter().map(|(c, _)| c.clone()).collect())
            .unwrap_or_default()
    }

    /// Évènements suivis.
    pub fn events(&self) -> Vec<Event> {
        let mut events: Vec<Event> = self
            .registry()
            .map(|r| r.hooks.iter().map(|(e, _)| *e).collect())
            .unwrap_or_default();
        events.dedup();
        events
    }

    /// Formats d'import : (nom, extensions de fichier).
    pub fn importers(&self) -> Vec<(String, Vec<String>)> {
        self.registry()
            .map(|r| {
                r.importers
                    .iter()
                    .map(|i| (i.name.clone(), i.extensions.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Dernières lignes écrites par `print`.
    pub fn log(&self) -> Vec<String> {
        self.registry().map(|r| r.log.clone()).unwrap_or_default()
    }

    pub fn is_loaded(&self) -> bool {
        self.vm.is_some()
    }
}

/// Format d'import déclaré par une extension (`bp.importer`).
#[derive(Debug, Clone)]
pub struct ImporterInfo {
    pub name: String,
    /// Nom de l'extension.
    pub extension: String,
    /// Extensions de fichier, sans le point.
    pub extensions: Vec<String>,
}

/// `detect` puis `parse` d'un format d'import ; `None` si `detect` refuse.
fn run_importer(
    lua: &Lua,
    spec: &import::ImporterSpec,
    path: &Path,
    data: &[u8],
) -> mlua::Result<Option<bp_core::import::ParsedImport>> {
    let file = import::file_table(lua, path, data)?;
    if let Some(detect) = &spec.detect
        && !detect.call::<bool>(file.clone())?
    {
        return Ok(None);
    }
    let value: Value = spec.parse.call(file)?;
    let parsed = objects::with_session(lua, |s| import::parsed_import(lua, &s.document, value))?;
    Ok(Some(parsed))
}

/// Arguments d'une réaction à un évènement.
fn event_args(lua: &Lua, event: Event, file: u64, item: &EventItem) -> mlua::Result<MultiValue> {
    let op = || -> mlua::Result<Value> {
        Ok(Value::UserData(lua.create_userdata(
            objects::OpRef::new(file, item.account, item.uid, 0),
        )?))
    };
    let before = || -> mlua::Result<Value> {
        let before = item
            .before
            .as_ref()
            .ok_or_else(|| objects::runtime("état d'avant manquant"))?;
        objects::operation_copy(lua, file, item.account, before).map(Value::Table)
    };
    Ok(match event {
        Event::OperationAdded => MultiValue::from_iter([op()?]),
        Event::OperationChanged => MultiValue::from_iter([op()?, before()?]),
        Event::OperationDeleted => MultiValue::from_iter([before()?]),
        Event::AccountSelected => MultiValue::from_iter([Value::UserData(lua.create_userdata(
            objects::AccountRef {
                file,
                index: item.account,
            },
        )?)]),
        _ => MultiValue::new(),
    })
}

/// Poignée `win` passée aux gestionnaires d'une fenêtre.
struct WinHandle(u64);

impl UserData for WinHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("close", |lua, w, value: Value| {
            if let Some(mut outbox) = lua.app_data_mut::<Outbox>() {
                outbox.closes.push((w.0, value));
            }
            Ok(())
        });
    }
}

struct Deadline(Instant);

pub struct Engine {
    pub extensions: Vec<Extension>,
    frames: Vec<Frame>,
    dir: PathBuf,
    data_dir: PathBuf,
    services: Rc<dyn Services>,
    next_tag: u64,
    next_window: u64,
}

/// Fichier principal d'une entrée du dossier des extensions.
fn main_file(path: &Path) -> Option<(String, PathBuf)> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    if name.starts_with('.') {
        return None;
    }
    if path.is_dir() {
        let init = path.join("init.luau");
        return init.is_file().then_some((name, init));
    }
    let id = name.strip_suffix(".luau")?;
    Some((id.to_string(), path.to_path_buf()))
}

fn error_text(e: &mlua::Error) -> String {
    let text = match e {
        mlua::Error::CallbackError { traceback, cause } => {
            let cause = error_text(cause);
            let lines: Vec<&str> = traceback
                .lines()
                .filter(|l| l.contains(".luau:"))
                .take(4)
                .collect();
            if lines.is_empty() {
                cause
            } else {
                format!("{cause}\n{}", lines.join("\n"))
            }
        }
        mlua::Error::RuntimeError(message) | mlua::Error::SyntaxError { message, .. } => {
            message.clone()
        }
        mlua::Error::MemoryError(_) => {
            "mémoire insuffisante (limite des extensions atteinte)".into()
        }
        other => other.to_string(),
    };
    text.replace("runtime error: ", "")
}

impl Engine {
    /// Moteur sur le dossier `dir` ; les préférences des extensions vont dans
    /// `data_dir`. Rien n'est chargé avant [`Engine::load_all`].
    pub fn new(dir: PathBuf, data_dir: PathBuf, services: Rc<dyn Services>) -> Self {
        Self {
            extensions: Vec::new(),
            frames: Vec::new(),
            dir,
            data_dir,
            services,
            next_tag: 1 << 40,
            next_window: 1,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// (Re)charge toutes les extensions du dossier ; `disabled` : identifiants
    /// désactivés par l'utilisateur.
    pub fn load_all(&mut self, disabled: &[String]) {
        self.frames.clear();
        self.extensions.clear();
        let mut entries: Vec<(String, PathBuf)> = std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| main_file(&e.path()))
                    .collect()
            })
            .unwrap_or_default();
        entries.sort_by_key(|(id, _)| id.to_lowercase());
        for (id, path) in entries {
            let enabled = !disabled.contains(&id);
            let mut ext = Extension {
                id,
                path,
                enabled,
                error: None,
                vm: None,
            };
            if enabled {
                self.load(&mut ext);
            }
            self.extensions.push(ext);
        }
    }

    /// Active ou désactive une extension (chargée à l'activation).
    pub fn set_enabled(&mut self, index: usize, enabled: bool) {
        let Some(mut ext) = self.extensions.get_mut(index).map(|e| {
            std::mem::replace(
                e,
                Extension {
                    id: String::new(),
                    path: PathBuf::new(),
                    enabled: false,
                    error: None,
                    vm: None,
                },
            )
        }) else {
            return;
        };
        ext.enabled = enabled;
        ext.error = None;
        ext.vm = None;
        if enabled {
            self.load(&mut ext);
        }
        self.extensions[index] = ext;
    }

    fn load(&self, ext: &mut Extension) {
        match self.create_vm(ext) {
            Ok(vm) => ext.vm = Some(vm),
            Err(e) => ext.error = Some(error_text(&e)),
        }
    }

    fn create_vm(&self, ext: &Extension) -> mlua::Result<Vm> {
        let lua = Lua::new();
        lua.set_memory_limit(MEMORY_LIMIT)?;
        let registry = Rc::new(RefCell::new(Registry {
            loading: true,
            ..Default::default()
        }));
        let dir = (ext.path.file_name().is_some_and(|n| n == "init.luau"))
            .then(|| ext.path.parent().map(Path::to_path_buf))
            .flatten();
        api::install(
            &lua,
            api::ApiContext {
                id: ext.id.clone(),
                dir,
                data_dir: self.data_dir.clone(),
                services: self.services.clone(),
                registry: registry.clone(),
            },
        )?;
        lua.load(PRELUDE).set_name("=prelude").exec()?;
        lua.set_interrupt(|lua| {
            if lua
                .app_data_ref::<Deadline>()
                .is_some_and(|d| Instant::now() > d.0)
            {
                return Err(mlua::Error::runtime(format!(
                    "l'extension a dépassé le temps maximal ({} s)",
                    TIME_LIMIT.as_secs()
                )));
            }
            Ok(VmState::Continue)
        });
        lua.sandbox(true)?;
        let source = std::fs::read_to_string(&ext.path)
            .map_err(|e| mlua::Error::runtime(format!("lecture impossible : {e}")))?;
        lua.set_app_data(Deadline(Instant::now() + TIME_LIMIT));
        lua.set_app_data(Outbox::default());
        let name = if ext.path.file_name().is_some_and(|n| n == "init.luau") {
            "@init.luau".to_string()
        } else {
            format!("@{}.luau", ext.id)
        };
        let result = lua.load(source).set_name(name).exec();
        lua.remove_app_data::<Deadline>();
        lua.remove_app_data::<Outbox>();
        registry.borrow_mut().loading = false;
        result?;
        Ok(Vm { lua, registry })
    }

    /// Commandes de toutes les extensions actives : (extension, commande).
    pub fn commands(&self) -> Vec<(usize, usize, CommandInfo, String)> {
        let mut out = Vec::new();
        for (e, ext) in self.extensions.iter().enumerate() {
            if !ext.enabled {
                continue;
            }
            let name = ext.name();
            for (c, info) in ext.commands().into_iter().enumerate() {
                out.push((e, c, info, name.clone()));
            }
        }
        out
    }

    /// Formats d'import des extensions actives.
    pub fn importers(&self) -> Vec<ImporterInfo> {
        let mut out = Vec::new();
        for ext in self.extensions.iter().filter(|e| e.enabled) {
            if let Some(r) = ext.registry() {
                for i in &r.importers {
                    out.push(ImporterInfo {
                        name: i.name.clone(),
                        extension: ext.name(),
                        extensions: i.extensions.clone(),
                    });
                }
            }
        }
        out
    }

    /// Lit un fichier avec le premier format d'import d'extension qui le
    /// reconnaît (extension du fichier, puis `detect`). `None` : aucun ;
    /// sinon le nom du format et les opérations lues, ou l'erreur.
    pub fn import_file(
        &mut self,
        path: &Path,
        data: &[u8],
        ctx: &mut DocContext<'_>,
    ) -> Option<(String, Result<bp_core::import::ParsedImport, String>)> {
        let suffix = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        for ext in 0..self.extensions.len() {
            let specs: Vec<import::ImporterSpec> = match self.extensions[ext].registry() {
                Some(r) if self.extensions[ext].enabled => r
                    .importers
                    .iter()
                    .filter(|i| i.extensions.contains(&suffix))
                    .cloned()
                    .collect(),
                _ => continue,
            };
            for spec in specs {
                let mut outcome: Option<Result<bp_core::import::ParsedImport, String>> = None;
                let name = self.extensions[ext].name();
                self.with_doc(ext, Some(ctx), |_, lua, _| {
                    outcome = run_importer(lua, &spec, path, data)
                        .map_err(|e| format!("{name} : {}", error_text(&e)))
                        .transpose();
                    if let Some(mut s) = lua.app_data_mut::<Session>()
                        && s.has_changes()
                    {
                        let before = s.accounts_before();
                        s.rollback(before);
                        outcome = Some(Err(format!(
                            "{name} : un format d'import lit le fichier sans modifier les comptes"
                        )));
                    }
                });
                if let Some(result) = outcome {
                    return Some((spec.name, result));
                }
            }
        }
        None
    }

    pub fn has_hooks(&self, event: Event) -> bool {
        self.extensions.iter().any(|e| {
            e.enabled
                && e.registry()
                    .is_some_and(|r| r.hooks.iter().any(|(h, _)| *h == event))
        })
    }

    pub fn has_frames(&self) -> bool {
        !self.frames.is_empty()
    }

    /// Fenêtres et boîtes de dialogue ouvertes, de la plus ancienne à la plus
    /// récente (seule la dernière reçoit les interactions).
    pub fn frames_mut(&mut self) -> impl Iterator<Item = FrameMut<'_>> {
        self.frames.iter_mut().map(|f| match f {
            Frame::Dialog { dialog, .. } => FrameMut::Dialog(dialog),
            Frame::Window { window, .. } => FrameMut::Window(window),
        })
    }

    /// Ferme tout (fichier fermé ou changé) ; les coroutines sont abandonnées.
    pub fn abandon(&mut self) {
        self.frames.clear();
    }

    fn new_tag(&mut self) -> u64 {
        self.next_tag += 1;
        self.next_tag
    }

    /// Exécute `f` avec le document prêté à l'extension `ext`.
    fn with_doc(
        &mut self,
        ext: usize,
        ctx: Option<&mut DocContext<'_>>,
        f: impl FnOnce(&mut Self, &Lua, &mut Effects),
    ) -> Effects {
        let mut fx = Effects::default();
        let Some(lua) = self
            .extensions
            .get(ext)
            .and_then(|e| e.vm.as_ref())
            .map(|vm| vm.lua.clone())
        else {
            return fx;
        };
        let mut ctx = ctx;
        if let Some(ctx) = ctx.as_deref_mut() {
            lua.set_app_data(Session::new(ctx));
        }
        lua.set_app_data(Outbox::default());
        lua.set_app_data(Deadline(Instant::now() + TIME_LIMIT));
        f(self, &lua, &mut fx);
        lua.remove_app_data::<Deadline>();
        if let Some(outbox) = lua.remove_app_data::<Outbox>() {
            fx.toasts.extend(outbox.toasts);
        }
        if let Some(session) = lua.remove_app_data::<Session>() {
            let mut session = session;
            fx.feature_changed |= session.feature_changed();
            fx.select = session.select.take().or(fx.select.take());
            if session.filters_changed {
                fx.filters = Some(session.filters.clone());
            }
            if let Some(ctx) = ctx {
                session.finish(ctx);
            }
        }
        fx
    }

    fn report(&self, ext: usize, error: &mlua::Error, fx: &mut Effects) {
        let name = self
            .extensions
            .get(ext)
            .map(|e| e.name())
            .unwrap_or_default();
        fx.errors.push(format!("{name} : {}", error_text(error)));
    }

    /// Reprend une coroutine, inscrit ses modifications et traite ce qu'elle
    /// demande (boîte de dialogue, fenêtre).
    fn resume(&mut self, ext: usize, lua: &Lua, run: Run, args: MultiValue, fx: &mut Effects) {
        let before = lua.app_data_ref::<Session>().map(|s| s.accounts_before());
        let result = run.thread.resume::<MultiValue>(args);
        match result {
            Err(e) => {
                if let (Some(before), Some(mut s)) = (before, lua.app_data_mut::<Session>()) {
                    s.rollback(before);
                }
                self.report(ext, &e, fx);
            }
            Ok(values) => {
                if let Some(mut s) = lua.app_data_mut::<Session>()
                    && let Some(changes) = s.take_changes(&run.label, run.tag)
                {
                    fx.changes.push(changes);
                }
                if run.thread.status() == ThreadStatus::Resumable {
                    match values.into_iter().next() {
                        Some(Value::Table(request)) => self.request(ext, lua, run, &request, fx),
                        _ => self.report(
                            ext,
                            &mlua::Error::runtime(
                                "coroutine.yield n'est pas utilisable dans une commande",
                            ),
                            fx,
                        ),
                    }
                }
            }
        }
    }

    fn request(&mut self, ext: usize, lua: &Lua, run: Run, request: &Table, fx: &mut Effects) {
        let parsed = (|| -> mlua::Result<Frame> {
            let kind: String = request.get("__bp")?;
            let title = request
                .get::<Option<String>>("title")?
                .unwrap_or_else(|| self.extensions[ext].name());
            let text = |key: &str| -> mlua::Result<String> {
                Ok(request.get::<Option<String>>(key)?.unwrap_or_default())
            };
            Ok(match kind.as_str() {
                "alert" => Frame::Dialog {
                    ext,
                    run: Some(run),
                    dialog: Dialog::Alert {
                        title,
                        message: text("message")?,
                    },
                },
                "confirm" => Frame::Dialog {
                    ext,
                    run: Some(run),
                    dialog: Dialog::Confirm {
                        title,
                        message: text("message")?,
                        ok: request
                            .get::<Option<String>>("ok")?
                            .unwrap_or_else(|| "Confirmer".into()),
                        danger: request.get::<Option<bool>>("danger")?.unwrap_or(false),
                    },
                },
                "prompt" => Frame::Dialog {
                    ext,
                    run: Some(run),
                    dialog: Dialog::Prompt {
                        title,
                        label: text("label")?,
                        text: crate::ui::display(&request.get::<Value>("default")?)?,
                        placeholder: request.get("placeholder")?,
                        ok: request
                            .get::<Option<String>>("ok")?
                            .unwrap_or_else(|| "Valider".into()),
                    },
                },
                "window" => {
                    self.next_window += 1;
                    let window = Box::new(Window::open(lua, self.next_window, request)?);
                    Frame::Window {
                        ext,
                        owner: Some(run),
                        window,
                    }
                }
                other => return Err(mlua::Error::runtime(format!("demande inconnue : {other}"))),
            })
        })();
        match parsed {
            Ok(frame) => self.frames.push(frame),
            Err(e) => self.report(ext, &e, fx),
        }
    }

    /// Ferme les fenêtres qui l'ont demandé (et reprend leur propriétaire),
    /// reconstruit celle du dessus si besoin.
    fn settle(&mut self, ext: usize, lua: &Lua, fx: &mut Effects) {
        loop {
            let closes = lua
                .app_data_mut::<Outbox>()
                .map(|mut o| std::mem::take(&mut o.closes))
                .unwrap_or_default();
            for (id, value) in closes {
                for frame in &mut self.frames {
                    if let Frame::Window { window, .. } = frame
                        && window.id == id
                        && !window.closed
                    {
                        window.closed = true;
                        window.result = Some(value.clone());
                    }
                }
            }
            match self.frames.last_mut() {
                Some(Frame::Window { ext: e, window, .. }) if *e == ext && window.closed => {
                    let Some(Frame::Window { owner, window, .. }) = self.frames.pop() else {
                        unreachable!()
                    };
                    if let Some(owner) = owner {
                        let result = window.result.unwrap_or(Value::Nil);
                        self.resume(ext, lua, owner, MultiValue::from_iter([result]), fx);
                    }
                }
                Some(Frame::Window { ext: e, window, .. }) if *e == ext && window.dirty => {
                    if let Err(e) = window.rebuild(lua) {
                        window.dirty = false;
                        self.report(ext, &e, fx);
                    }
                    break;
                }
                _ => break,
            }
        }
    }

    /// Lance une commande d'extension.
    pub fn run_command(
        &mut self,
        ext: usize,
        command: usize,
        ctx: Option<&mut DocContext<'_>>,
    ) -> Effects {
        let Some((info, function)) = self
            .extensions
            .get(ext)
            .filter(|e| e.enabled)
            .and_then(|e| e.registry().and_then(|r| r.commands.get(command).cloned()))
        else {
            return Effects::default();
        };
        if info.needs != Needs::Nothing && ctx.is_none() {
            return Effects {
                errors: vec!["Ouvrez d'abord un fichier de comptes.".into()],
                ..Default::default()
            };
        }
        let tag = self.new_tag();
        self.with_doc(ext, ctx, |me, lua, fx| {
            match lua.create_thread(function) {
                Ok(thread) => {
                    let run = Run {
                        thread,
                        label: info.title.trim_end_matches('…').to_string(),
                        tag,
                    };
                    me.resume(ext, lua, run, MultiValue::new(), fx);
                }
                Err(e) => me.report(ext, &e, fx),
            }
            me.settle(ext, lua, fx);
        })
    }

    /// Prévient les extensions d'un évènement. `items` : opérations
    /// concernées pour les évènements d'opération (une réaction par
    /// opération), compte pour `AccountSelected`.
    pub fn fire(&mut self, event: Event, items: &[EventItem], mut ctx: DocContext<'_>) -> Effects {
        let mut all = Effects::default();
        let tag = ctx.merge_tag.unwrap_or_else(|| self.new_tag());
        for ext in 0..self.extensions.len() {
            let hooks: Vec<mlua::Function> = match self.extensions[ext].registry() {
                Some(r) if self.extensions[ext].enabled => r
                    .hooks
                    .iter()
                    .filter(|(e, _)| *e == event)
                    .map(|(_, f)| f.clone())
                    .collect(),
                _ => continue,
            };
            if hooks.is_empty() {
                continue;
            }
            let label = self.extensions[ext].name();
            let file_id = ctx.file_id;
            let fx = self.with_doc(ext, Some(&mut ctx), |me, lua, fx| {
                for hook in hooks {
                    let calls: Vec<MultiValue> = match event {
                        Event::OperationAdded
                        | Event::OperationChanged
                        | Event::OperationDeleted
                        | Event::AccountSelected => items
                            .iter()
                            .filter_map(|item| event_args(lua, event, file_id, item).ok())
                            .collect(),
                        Event::FileOpened
                        | Event::BeforeSave
                        | Event::AfterSave
                        | Event::SelectionChanged => vec![MultiValue::new()],
                    };
                    for args in calls {
                        match lua.create_thread(hook.clone()) {
                            Ok(thread) => {
                                let run = Run {
                                    thread,
                                    label: label.clone(),
                                    tag,
                                };
                                me.resume(ext, lua, run, args, fx);
                            }
                            Err(e) => me.report(ext, &e, fx),
                        }
                    }
                }
                me.settle(ext, lua, fx);
            });
            all.changes.extend(fx.changes);
            all.feature_changed |= fx.feature_changed;
            all.select = fx.select.or(all.select);
            all.filters = fx.filters.or(all.filters);
            all.toasts.extend(fx.toasts);
            all.errors.extend(fx.errors);
        }
        all
    }

    /// Réponse à la boîte de dialogue du dessus.
    pub fn answer(&mut self, answer: Answer, ctx: Option<&mut DocContext<'_>>) -> Effects {
        let Some(Frame::Dialog { .. }) = self.frames.last() else {
            return Effects::default();
        };
        let Some(Frame::Dialog { ext, run, dialog }) = self.frames.pop() else {
            unreachable!()
        };
        self.with_doc(ext, ctx, |me, lua, fx| {
            let value = match (&dialog, answer) {
                (Dialog::Alert { .. }, _) => Value::Nil,
                (Dialog::Confirm { .. }, a) => Value::Boolean(a == Answer::Ok),
                (Dialog::Prompt { text, .. }, Answer::Ok) => match lua.create_string(text) {
                    Ok(s) => Value::String(s),
                    Err(_) => Value::Nil,
                },
                (Dialog::Prompt { .. }, Answer::Cancel) => Value::Nil,
            };
            if let Some(run) = run {
                me.resume(ext, lua, run, MultiValue::from_iter([value]), fx);
            }
            me.settle(ext, lua, fx);
        })
    }

    /// Interaction dans la fenêtre du dessus.
    pub fn window_event(&mut self, event: UiEvent, ctx: Option<&mut DocContext<'_>>) -> Effects {
        let Some(Frame::Window { ext, .. }) = self.frames.last() else {
            return Effects::default();
        };
        let ext = *ext;
        let tag = self.new_tag();
        self.with_doc(ext, ctx, |me, lua, fx| {
            let Some(Frame::Window { window, .. }) = me.frames.last_mut() else {
                return;
            };
            let (id, title, state) = (window.id, window.title.clone(), window.state.clone());
            match window.apply(lua, event) {
                Ok(Some((handler, extra))) => {
                    let args = (|| -> mlua::Result<MultiValue> {
                        let mut args = vec![Value::Table(state)];
                        args.extend(extra);
                        args.push(Value::UserData(lua.create_userdata(WinHandle(id))?));
                        Ok(MultiValue::from_iter(args))
                    })();
                    match args.and_then(|a| Ok((lua.create_thread(handler)?, a))) {
                        Ok((thread, args)) => {
                            let run = Run {
                                thread,
                                label: title,
                                tag,
                            };
                            me.resume(ext, lua, run, args, fx);
                        }
                        Err(e) => me.report(ext, &e, fx),
                    }
                }
                Ok(None) => {}
                Err(e) => me.report(ext, &e, fx),
            }
            me.settle(ext, lua, fx);
        })
    }
}

#[cfg(test)]
mod tests;
