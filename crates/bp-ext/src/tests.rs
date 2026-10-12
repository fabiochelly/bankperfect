use std::path::PathBuf;
use std::rc::Rc;

use bp_core::model::{Amount, Date, Document, Tack};

use super::*;

struct NoServices;

impl Services for NoServices {
    fn pick_open(&self, _: &str, _: &[(String, Vec<String>)]) -> Option<PathBuf> {
        None
    }
    fn pick_save(&self, _: &str, _: &str, _: &[(String, Vec<String>)]) -> Option<PathBuf> {
        None
    }
    fn open_url(&self, _: &str) {}
}

struct Fixture {
    _dir: tempfile::TempDir,
    engine: Engine,
    document: Document,
}

impl Fixture {
    fn new(source: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("essai.luau"), source).unwrap();
        let mut engine = Engine::new(
            dir.path().to_path_buf(),
            dir.path().join("donnees"),
            Rc::new(NoServices),
        );
        engine.load_all(&[]);
        let mut document = bp_core::manage::new_document("Test", Amount::from_cents(100_000));
        bp_core::manage::add_account(&mut document, "Livret", Amount::from_cents(50_000));
        Self {
            _dir: dir,
            engine,
            document,
        }
    }

    fn ctx(&mut self) -> DocContext<'_> {
        let n = self.document.accounts[0].operations.len();
        DocContext {
            document: &mut self.document,
            file_id: 7,
            path: PathBuf::from("/tmp/test.bp"),
            account: 0,
            selection: Vec::new(),
            visible: (0..n).collect(),
            locked: true,
            merge_tag: None,
        }
    }

    fn run(&mut self, command: usize) -> Effects {
        let mut engine = std::mem::replace(
            &mut self.engine,
            Engine::new(PathBuf::new(), PathBuf::new(), Rc::new(NoServices)),
        );
        let fx = engine.run_command(0, command, Some(&mut self.ctx()));
        self.engine = engine;
        fx
    }

    fn event(&mut self, event: UiEvent) -> Effects {
        let mut engine = std::mem::replace(
            &mut self.engine,
            Engine::new(PathBuf::new(), PathBuf::new(), Rc::new(NoServices)),
        );
        let fx = engine.window_event(event, Some(&mut self.ctx()));
        self.engine = engine;
        fx
    }

    fn answer(&mut self, answer: Answer) -> Effects {
        let mut engine = std::mem::replace(
            &mut self.engine,
            Engine::new(PathBuf::new(), PathBuf::new(), Rc::new(NoServices)),
        );
        let fx = engine.answer(answer, Some(&mut self.ctx()));
        self.engine = engine;
        fx
    }

    fn window(&mut self) -> Option<&Window> {
        self.engine.frames.iter().rev().find_map(|f| match f {
            Frame::Window { window, .. } => Some(&**window),
            _ => None,
        })
    }
}

#[test]
fn loads_declarations() {
    let f = Fixture::new(
        r#"
        bp.extension{ name = "Essai", version = "1.2", author = "Moi", permissions = { "network" } }
        bp.command{ title = "Bonjour…", icon = "hand-waving", shortcut = "Ctrl+Alt+B", run = function() end }
        bp.command{ title = "Sans fichier", needs = "nothing", run = function() end }
        bp.on("operation_added", function(op) end)
        "#,
    );
    let ext = &f.engine.extensions[0];
    assert_eq!(ext.error, None);
    assert_eq!(ext.name(), "Essai");
    assert_eq!(ext.version(), "1.2");
    assert_eq!(ext.permissions(), vec!["network".to_string()]);
    let commands = ext.commands();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].shortcut.as_deref(), Some("Ctrl+Alt+B"));
    assert_eq!(commands[1].needs, Needs::Nothing);
    assert!(f.engine.has_hooks(Event::OperationAdded));
}

#[test]
fn command_menus() {
    let f = Fixture::new(
        r#"
        bp.command{ title = "Par défaut", run = function() end }
        bp.command{ title = "Opérations", menu = "operations", run = function() end }
        bp.command{ title = "Deux", menu = { "grid", "account" }, run = function() end }
        "#,
    );
    let commands = f.engine.extensions[0].commands();
    assert_eq!(commands[0].menus, vec!["extensions"]);
    assert_eq!(commands[1].menus, vec!["operations"]);
    assert_eq!(commands[2].menus, vec!["grid", "account"]);

    let f = Fixture::new(r#"bp.command{ title = "X", menu = "nulle-part", run = function() end }"#);
    let error = f.engine.extensions[0].error.clone().unwrap_or_default();
    assert!(error.contains("nulle-part"), "{error}");
}

#[test]
fn reports_load_errors() {
    let f = Fixture::new("bp.command{ title = 'x' }");
    let error = f.engine.extensions[0].error.clone().unwrap();
    assert!(error.contains("run"), "{error}");
    let f = Fixture::new("local x = ");
    assert!(f.engine.extensions[0].error.is_some());
}

#[test]
fn sandbox_blocks_system_access() {
    let f = Fixture::new(
        r#"
        assert(io == nil, "io")
        assert(os.execute == nil and os.remove == nil and os.getenv == nil, "os")
        assert(debug == nil or debug.sethook == nil, "debug")
        bp.command{ title = "x", run = function() end }
        "#,
    );
    assert_eq!(f.engine.extensions[0].error, None);
    let f = Fixture::new("bp.accounts = nil");
    assert!(
        f.engine.extensions[0].error.is_some(),
        "bp est en lecture seule"
    );
}

#[test]
fn adds_and_edits_operations_with_one_undo_step() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "Ajouter…", run = function()
            local acc = bp.current_account()
            local op = acc:add{ date = "05/03/2026", payee = "Boulangerie", amount = -4.2,
                                category = "Alimentation > Epicerie", fields = { ticket = 12 } }
            assert(op.mode == "Carte")
            assert(op.date == bp.date(2026, 3, 5))
            assert(op.category == "Alimentation > Epicerie")
            assert(op:field("ticket") == "12")
            op.details = "pain; croissants"
            local salaire = acc:add{ payee = "Salaire", amount = "1 800,50", mode = "Virement reçu" }
            assert(salaire.amount == 1800.5)
            local livret = bp.account("Livret")
            salaire:move_to(livret)
            assert(salaire.account == livret)
            bp.select({ op })
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert_eq!(fx.changes.len(), 1);
    let changes = &fx.changes[0];
    assert_eq!(changes.label, "Ajouter");
    assert_eq!(changes.accounts.len(), 2);
    assert_eq!(changes.accounts[0].1.len(), 1, "état d'avant gardé");
    let ops = &f.document.accounts[0].operations;
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[1].payee, "Boulangerie");
    assert_eq!(ops[1].details, "pain  croissants");
    assert_eq!(ops[1].amount, Amount(-42_000));
    assert_eq!(
        f.document.accounts[1].operations[1].amount,
        Amount(18_005_000)
    );
    let uid = ops[1].uid;
    assert_ne!(uid, f.document.accounts[1].operations[1].uid);
    assert_eq!(fx.select, Some((0, vec![uid])));
}

#[test]
fn error_rolls_back_changes() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "Échec", run = function()
            local acc = bp.current_account()
            acc:add{ payee = "A", amount = -1 }
            bp.add_category("Nouvelle")
            error("raté")
        end }
        "#,
    );
    let categories = f.document.categories.len();
    let fx = f.run(0);
    assert!(fx.changes.is_empty());
    assert_eq!(fx.errors.len(), 1);
    assert!(fx.errors[0].contains("raté"), "{:?}", fx.errors);
    assert_eq!(f.document.accounts[0].operations.len(), 1);
    assert_eq!(f.document.categories.len(), categories);
}

#[test]
fn locked_reconciled_lines_are_protected() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "x", run = function()
            local first = bp.current_account():operations()[1]
            assert(first.tack == "reconciled")
            first.amount = 0
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors[0].contains("verrouillées"), "{:?}", fx.errors);
    assert_eq!(
        f.document.accounts[0].operations[0].amount,
        Amount(10_000_000)
    );
}

#[test]
fn filters_and_dates() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "x", run = function()
            local acc = bp.current_account()
            for i = 1, 12 do
                acc:add{ date = bp.date(2025, i, 10), payee = if i % 2 == 0 then "EDF" else "Loyer",
                         amount = -i, category = if i % 2 == 0 then "EDF" else "Loyer" }
            end
            local edf = acc:operations{ payee = "edf" }
            assert(#edf == 6, #edf)
            local h1 = acc:operations{ from = "01/01/2025", to = bp.date(2025, 6, 30) }
            assert(#h1 == 6)
            local habitation = acc:operations{ category = "Habitation" }
            assert(#habitation == 12, "la catégorie mère inclut ses sous-catégories")
            local big = acc:operations{ max = -10 }
            assert(#big == 3)
            local d = bp.date(2024, 1, 31)
            assert(tostring(d:add_months(1)) == "29/02/2024")
            assert((d + 1).month == 2)
            assert(bp.date(2024, 3, 1) - d == 30)
            assert(d:end_of_month() == d and d:start_of_month().day == 1)
            assert(d.weekday_name == "mercredi")
            assert(d:format("aaaa-mm-jj") == "2024-01-31")
            assert(acc:balance_at("31/12/2024") == 0)
            assert(acc:balance_at("31/12/2025") == -78)
            assert(bp.money(-1234.5) == "-1\u{202f}234,50\u{a0}€")
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
}

#[test]
fn categories_and_modes() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "x", run = function()
            local c = bp.add_category("Vacances")
            assert(c.label == "Vacances")
            local child = bp.add_category("Hôtel", "Vacances")
            assert(child.label == "Vacances > Hôtel" and child.parent == "Vacances")
            assert(bp.add_category("Hôtel", "Vacances").id == child.id, "pas de doublon")
            local ok, err = pcall(function() return bp.category("Assurance").id end)
            assert(bp.category("Transport > Assurance") ~= nil)
            assert(bp.add_mode("Crypto", false) == "Crypto")
            local op = bp.current_account():add{ amount = 5, mode = "crypto", category = child }
            assert(op.mode == "Crypto")
            assert(op.category_id == child.id)
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert!(fx.changes[0].referentials.is_some());
    let vacances = f
        .document
        .categories
        .iter()
        .position(|c| c.name == "Vacances")
        .unwrap();
    assert!(f.document.categories[vacances + 1].is_child);
}

#[test]
fn dialogs_suspend_and_resume() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "Question", run = function()
            bp.current_account():add{ payee = "avant", amount = -1 }
            local name = bp.ui.prompt("Nom du tiers", { default = "Boulanger" })
            if name and bp.ui.confirm("Ajouter " .. name .. " ?") then
                bp.current_account():add{ payee = name, amount = -2 }
            end
            bp.ui.toast("fini")
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert_eq!(fx.changes.len(), 1);
    let tag = fx.changes[0].tag;
    match f.engine.frames_mut().last() {
        Some(FrameMut::Dialog(Dialog::Prompt { text, .. })) => {
            assert_eq!(text, "Boulanger");
            *text = "Pâtissier".into();
        }
        _ => panic!("prompt attendu"),
    }
    let fx = f.answer(Answer::Ok);
    assert!(fx.changes.is_empty());
    assert!(matches!(
        f.engine.frames_mut().last(),
        Some(FrameMut::Dialog(Dialog::Confirm { .. }))
    ));
    let fx = f.answer(Answer::Ok);
    assert!(!f.engine.has_frames());
    assert_eq!(fx.toasts, vec!["fini".to_string()]);
    assert_eq!(fx.changes.len(), 1);
    assert_eq!(
        fx.changes[0].tag, tag,
        "même lancement : même étape d'annulation"
    );
    assert_eq!(f.document.accounts[0].operations[2].payee, "Pâtissier");
}

#[test]
fn windows_bind_state_and_run_handlers() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "Fenêtre", run = function()
            local result = bp.ui.window{
                title = "Doublons",
                state = { days = 3, amount = 10, label = "", flag = false },
                view = function(s)
                    local rows = {}
                    for _, op in bp.current_account():operations() do
                        table.insert(rows, { op.date, op.payee, bp.ui.money(op.amount), id = op })
                    end
                    return {
                        bp.ui.section{ title = "Critères",
                            bp.ui.number{ "days", label = "Écart" },
                            bp.ui.amount{ "amount", label = "Montant" },
                            bp.ui.checkbox{ "flag", label = "Option" },
                            bp.ui.choice{ "mode", options = { "a", { "b", "Bé" } } },
                        },
                        bp.ui.text("Jours : " .. s.days),
                        bp.ui.table{ columns = { "Date", "Tiers", { "Montant", align = "right" } },
                                     rows = rows, select = "picked", multiple = true },
                        s.flag and bp.ui.text("option active"),
                    }
                end,
                buttons = {
                    bp.ui.button{ "Annuler", cancel = true },
                    bp.ui.button{ "Ajouter", on_click = function(s)
                        bp.current_account():add{ payee = "ajout", amount = -s.amount }
                    end },
                    bp.ui.button{ "Valider", primary = true, close = true },
                },
            }
            assert(result.days == 7)
            assert(#result.picked == 1 and result.picked[1].payee == "Solde initial")
            bp.ui.toast("résultat " .. result.mode)
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    let w = f.window().unwrap();
    assert_eq!(w.title, "Doublons");
    assert_eq!(w.body.len(), 3, "l'élément nil est ignoré");
    let Widget::Section { children, .. } = &w.body[0] else {
        panic!()
    };
    let Widget::Input(days) = &children[0] else {
        panic!()
    };
    assert_eq!(days.text, "3");
    assert_eq!(w.footer.len(), 3);

    // Saisie incomplète : l'état ne change pas, le texte est gardé.
    f.event(UiEvent::Text {
        key: "days".into(),
        text: "7,".into(),
    });
    f.event(UiEvent::Text {
        key: "days".into(),
        text: "7".into(),
    });
    f.event(UiEvent::Text {
        key: "amount".into(),
        text: "12,5".into(),
    });
    f.event(UiEvent::Bool {
        key: "flag".into(),
        value: true,
    });
    f.event(UiEvent::Pick {
        key: "mode".into(),
        index: Some(1),
    });
    let w = f.window().unwrap();
    assert_eq!(w.body.len(), 4);
    let Widget::Text { text, .. } = &w.body[1] else {
        panic!()
    };
    assert_eq!(text, "Jours : 7");
    let Widget::Table(table) = &w.body[2] else {
        panic!()
    };
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0][2].amount, Some(1000.0));

    // Bouton « Ajouter » : une étape d'annulation, la fenêtre reste ouverte.
    let Widget::Button { id, .. } = &w.footer[1] else {
        panic!()
    };
    let add = *id;
    let fx = f.event(UiEvent::Click { button: add });
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert_eq!(fx.changes.len(), 1);
    assert_eq!(
        f.document.accounts[0].operations[1].amount,
        Amount(-125_000)
    );
    let Widget::Table(table) = &f.window().unwrap().body[2] else {
        panic!()
    };
    assert_eq!(table.rows.len(), 2, "la fenêtre est reconstruite");
    let table_id = table.id;

    f.event(UiEvent::TableSelect {
        table: table_id,
        rows: vec![0],
    });
    let Widget::Table(table) = &f.window().unwrap().body[2] else {
        panic!()
    };
    assert_eq!(table.selected, vec![0]);

    let Widget::Button { id, .. } = f.window().unwrap().footer[2].clone() else {
        panic!()
    };
    let fx = f.event(UiEvent::Click { button: id });
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert!(!f.engine.has_frames());
    assert_eq!(fx.toasts, vec!["résultat b".to_string()]);
}

#[test]
fn form_returns_values_or_nil() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "Form", run = function()
            local v = bp.ui.form{ title = "Réglage",
                bp.ui.input{ "name", label = "Nom", value = "x" },
                bp.ui.date{ "when", label = "Date", value = bp.date(2026, 1, 2) },
                validate = function(s) if s.name == "" then return "Nom obligatoire" end end,
            }
            bp.ui.toast(if v then v.name .. " " .. tostring(v.when) else "annulé")
        end }
        "#,
    );
    f.run(0);
    let Widget::Section { children, .. } = &f.window().unwrap().body[0] else {
        panic!()
    };
    let Widget::Input(when) = &children[1] else {
        panic!()
    };
    assert_eq!(when.text, "02/01/2026");
    f.event(UiEvent::Text {
        key: "name".into(),
        text: String::new(),
    });
    let ok = |f: &mut Fixture| {
        let Some(Widget::Button { id, .. }) = f.window().unwrap().footer.last().cloned() else {
            panic!()
        };
        f.event(UiEvent::Click { button: id })
    };
    let fx = ok(&mut f);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert!(f.engine.has_frames(), "validation refusée");
    assert!(matches!(f.window().unwrap().body[1], Widget::Notice { .. }));
    f.event(UiEvent::Text {
        key: "name".into(),
        text: "Paul".into(),
    });
    f.event(UiEvent::Text {
        key: "when".into(),
        text: "3/2/26".into(),
    });
    let fx = ok(&mut f);
    assert_eq!(fx.toasts, vec!["Paul 03/02/2026".to_string()]);

    f.run(0);
    let fx = f.event(UiEvent::Dismiss);
    assert_eq!(fx.toasts, vec!["annulé".to_string()]);
}

#[test]
fn hooks_merge_into_the_triggering_step() {
    let mut f = Fixture::new(
        r#"
        bp.on("operation_added", function(op)
            if op.payee:lower():find("carrefour") then
                op.category = "Alimentation > Epicerie"
            end
        end)
        "#,
    );
    let mut op = f.document.accounts[0].operations[0].clone();
    op.uid = 99;
    op.payee = "CARREFOUR MARKET".into();
    op.tack = Tack::None;
    f.document.accounts[0].operations.push(op);
    let mut engine = std::mem::replace(
        &mut f.engine,
        Engine::new(PathBuf::new(), PathBuf::new(), Rc::new(NoServices)),
    );
    let mut ctx = f.ctx();
    ctx.merge_tag = Some(5);
    let fx = engine.fire(Event::OperationAdded, &[EventItem::line(0, 99)], ctx);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert_eq!(fx.changes.len(), 1);
    assert_eq!(fx.changes[0].tag, 5);
    let label = f
        .document
        .category_label(f.document.accounts[0].operations[1].category.unwrap());
    assert_eq!(label.as_deref(), Some("Alimentation > Epicerie"));
}

#[test]
fn event_arguments() {
    let mut f = Fixture::new(
        r#"
        bp.on("operation_changed", function(op, before)
            bp.ui.toast(`changé {before.amount} -> {op.amount} {before.date}`)
        end)
        bp.on("operation_deleted", function(old)
            bp.ui.toast(`supprimé {old.payee} {old.account.name} {old.tack}`)
        end)
        bp.on("account_selected", function(account)
            bp.ui.toast(`compte {account.name}`)
        end)
        bp.on("selection_changed", function()
            bp.ui.toast(`sélection {#bp.selection()}`)
        end)
        "#,
    );
    let mut engine = std::mem::replace(
        &mut f.engine,
        Engine::new(PathBuf::new(), PathBuf::new(), Rc::new(NoServices)),
    );
    let op = f.document.accounts[0].operations[0].clone();
    let mut before = op.clone();
    before.amount = Amount::from_cents(-500);
    let item = EventItem {
        account: 0,
        uid: op.uid,
        before: Some(before.clone()),
    };
    let fx = engine.fire(Event::OperationChanged, std::slice::from_ref(&item), f.ctx());
    assert_eq!(fx.errors, Vec::<String>::new());
    assert_eq!(
        fx.toasts,
        vec![format!("changé -5 -> 1000 {}", before.date)]
    );
    let fx = engine.fire(Event::OperationDeleted, &[item], f.ctx());
    assert_eq!(fx.errors, Vec::<String>::new());
    assert_eq!(fx.toasts, vec![format!("supprimé {} Courant reconciled", op.payee)]);
    let fx = engine.fire(Event::AccountSelected, &[EventItem::line(1, 0)], f.ctx());
    assert_eq!(fx.toasts, vec!["compte Livret".to_string()]);
    let fx = engine.fire(Event::SelectionChanged, &[], f.ctx());
    assert_eq!(fx.toasts, vec!["sélection 0".to_string()]);
}

#[test]
fn split_operations() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "x", run = function()
            local op = bp.current_account():add{ payee = "Hyper", amount = -120 }
            assert(op.split_info == nil)
            local parts = op:split({
                { amount = 80, category = "Courses", details = "nourriture" },
                { details = "maison" },
            })
            assert(#parts == 2)
            assert(parts[1].amount == -80 and parts[2].amount == -40)
            local info = parts[2].split_info
            assert(info.index == 2 and info.count == 2 and info.total == -120 and info.payee == "Hyper")
            assert(parts[1].category == "Courses")
            assert(not pcall(function() parts[1]:split({ { amount = 1 } }) end))
        end }
        bp.command{ title = "y", run = function()
            bp.add_category("Courses")
        end }
        "#,
    );
    let fx = f.run(1);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert_eq!(fx.changes.len(), 1);
    assert_eq!(f.document.accounts[0].operations.len(), 3);
}

#[test]
fn importers_read_files() {
    let mut f = Fixture::new(
        r#"
        bp.importer{
            name = "Banque Test",
            extensions = { "csv" },
            detect = function(file) return file.text:sub(1, 5) == "date;" end,
            parse = function(file)
                local ops = {}
                for line in file.text:gmatch("[^\n]+") do
                    local date, label, amount = line:match("^(.-);(.-);(.-)$")
                    if date ~= "date" then
                        table.insert(ops, { date = date, payee = label, amount = amount, category = "Loisirs > Cinéma", id = "x" .. #ops })
                    end
                end
                return { number = "123", operations = ops }
            end,
        }
        bp.importer{
            name = "Fautif",
            extensions = "bad",
            parse = function() bp.current_account():add{ amount = 1 } return {} end,
        }
        "#,
    );
    assert_eq!(f.engine.extensions[0].error, None);
    let mut engine = std::mem::replace(
        &mut f.engine,
        Engine::new(PathBuf::new(), PathBuf::new(), Rc::new(NoServices)),
    );
    let data = b"date;libelle;montant
01/02/2026;Cinema;-12,50
;Sans date;3
02/02/2026;Salaire;1500";
    let (name, parsed) = engine
        .import_file(std::path::Path::new("/tmp/releve.CSV"), data, &mut f.ctx())
        .expect("format reconnu");
    assert_eq!(name, "Banque Test");
    let parsed = parsed.expect("lu");
    assert_eq!(parsed.accounts.len(), 1);
    assert_eq!(parsed.accounts[0].account_id.as_deref(), Some("123"));
    let ops = &parsed.accounts[0].operations;
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].amount, Amount::from_cents(-1250));
    assert_eq!(ops[0].category.as_deref(), Some("Loisirs"));
    assert_eq!(ops[0].subcategory.as_deref(), Some("Cinéma"));
    assert_eq!(ops[1].fitid.as_deref(), Some("x2"));
    assert_eq!(parsed.rejected.len(), 1);

    // Pas reconnu : l'import de BankPerfect prend le relais.
    assert!(engine
        .import_file(std::path::Path::new("a.csv"), b"autre", &mut f.ctx())
        .is_none());
    assert!(engine
        .import_file(std::path::Path::new("a.qif"), data, &mut f.ctx())
        .is_none());

    // Un format qui modifie les comptes est refusé et défait.
    let n = f.document.accounts[0].operations.len();
    let (_, result) = engine
        .import_file(std::path::Path::new("a.bad"), b"", &mut f.ctx())
        .unwrap();
    assert!(result.is_err());
    assert_eq!(f.document.accounts[0].operations.len(), n);
}

#[test]
fn store_is_restored_on_error() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "ok", run = function()
            bp.store.set("n", 1)
        end }
        bp.command{ title = "échec", run = function()
            bp.store.set("n", 2)
            bp.store.set("autre", "x")
            error("raté")
        end }
        bp.command{ title = "question", run = function()
            bp.store.set("n", 3)
            bp.ui.confirm("Continuer ?")
            bp.store.set("n", 4)
            error("raté après la question")
        end }
        bp.command{ title = "lire", run = function()
            bp.ui.toast(tostring(bp.store.get("n")) .. " " .. tostring(bp.store.get("autre")))
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.feature_changed);
    let saved = f.document.feature_data.clone();
    let fx = f.run(1);
    assert_eq!(fx.errors.len(), 1);
    assert!(!fx.feature_changed, "rien de gardé");
    assert_eq!(f.document.feature_data, saved, "valeur d'avant remise");
    assert_eq!(f.run(3).toasts, vec!["1 nil".to_string()]);

    // Une première clé absente disparaît aussi.
    f.document.feature_data.clear();
    f.run(1);
    assert!(f.document.feature_data.is_empty());

    // Ce qui précède une boîte de dialogue est gardé, la suite est défaite.
    f.run(0);
    f.run(2);
    let fx = f.answer(Answer::Ok);
    assert_eq!(fx.errors.len(), 1);
    assert_eq!(f.run(3).toasts, vec!["3 nil".to_string()]);
}

#[test]
fn store_and_json() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "x", run = function()
            assert(bp.store.get("n") == nil)
            bp.store.set("n", { compte = 3, liste = { 1, 2 } })
            local back = bp.store.get("n")
            assert(back.compte == 3 and back.liste[2] == 2)
            local text = bp.json.encode({ a = 1 })
            assert(bp.json.decode(text).a == 1)
            bp.prefs.set("theme", "vert")
            assert(bp.prefs.get("theme") == "vert")
            local ok = pcall(bp.http.get, "https://example.com")
            assert(not ok, "réseau non déclaré")
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert!(fx.feature_changed);
    assert!(
        f.document
            .feature_data
            .iter()
            .any(|d| d.key == "extension.essai")
    );
}

#[test]
fn infinite_loops_are_stopped() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "x", run = function()
            while true do end
        end }
        "#,
    );
    // Délai raccourci pour le test.
    let lua = f.engine.extensions[0].vm.as_ref().unwrap().lua.clone();
    let start = Instant::now();
    let mut engine = std::mem::replace(
        &mut f.engine,
        Engine::new(PathBuf::new(), PathBuf::new(), Rc::new(NoServices)),
    );
    let function = engine.extensions[0].registry().unwrap().commands[0]
        .1
        .clone();
    lua.set_app_data(Deadline(Instant::now() + Duration::from_millis(200)));
    let result = lua.create_thread(function).unwrap().resume::<()>(());
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_secs(5));
    let _ = &mut engine;
}

#[test]
fn transfers_and_require_guard() {
    let mut f = Fixture::new(
        r#"
        bp.command{ title = "x", run = function()
            local a, b = bp.transfer{ from = bp.current_account(), to = "Livret", amount = 100, date = "01/02/2026" }
            assert(a.amount == -100 and b.amount == 100)
            assert(a:sister() == b)
            local c = bp.current_account():add{ amount = 1 }
            assert(c.uid ~= a.uid and c.uid ~= b.uid)
            assert(not pcall(require, "../secret"))
        end }
        "#,
    );
    let fx = f.run(0);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert_eq!(f.document.accounts[1].operations.len(), 2);
    let _ = Date::today();
}

#[test]
fn bundled_examples_load_and_open() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let data = tempfile::tempdir().unwrap();
    let mut engine = Engine::new(dir, data.path().to_path_buf(), Rc::new(NoServices));
    engine.load_all(&[]);
    assert_eq!(engine.extensions.len(), 6);
    let mut document = bp_core::manage::new_document("Test", Amount::from_cents(100_000));
    bp_core::manage::add_account(&mut document, "Livret", Amount::ZERO);
    for ext in 0..engine.extensions.len() {
        let e = &engine.extensions[ext];
        assert_eq!(e.error, None, "{}", e.id);
        for command in 0..e.commands().len() {
            let n = document.accounts[0].operations.len();
            let mut ctx = DocContext {
                document: &mut document,
                file_id: 1,
                path: PathBuf::from("t.bp"),
                account: 0,
                selection: vec![0],
                visible: (0..n).collect(),
                locked: false,
                merge_tag: None,
            };
            let fx = engine.run_command(ext, command, Some(&mut ctx));
            assert!(
                fx.errors.is_empty(),
                "{} : {:?}",
                engine.extensions[ext].id,
                fx.errors
            );
            assert!(
                engine.has_frames(),
                "{} ouvre une fenêtre",
                engine.extensions[ext].id
            );
            let fx = engine.window_event(UiEvent::Dismiss, Some(&mut ctx));
            assert!(fx.errors.is_empty(), "{:?}", fx.errors);
            assert!(!engine.has_frames());
        }
    }
    // Arrondis : réglage puis paiement par carte.
    let arrondis = engine
        .extensions
        .iter()
        .position(|e| e.id == "arrondis")
        .unwrap();
    let mut ctx = DocContext {
        document: &mut document,
        file_id: 1,
        path: PathBuf::from("t.bp"),
        account: 0,
        selection: vec![],
        visible: vec![],
        locked: false,
        merge_tag: None,
    };
    engine.run_command(arrondis, 0, Some(&mut ctx));
    engine.window_event(
        UiEvent::Bool {
            key: "active".into(),
            value: true,
        },
        Some(&mut ctx),
    );
    engine.window_event(
        UiEvent::Pick {
            key: "account".into(),
            index: Some(1),
        },
        Some(&mut ctx),
    );
    let ok = match engine.frames_mut().last() {
        Some(FrameMut::Window(w)) => match w.footer.last() {
            Some(Widget::Button { id, .. }) => *id,
            _ => panic!(),
        },
        _ => panic!(),
    };
    let fx = engine.window_event(UiEvent::Click { button: ok }, Some(&mut ctx));
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    assert!(!engine.has_frames());
    drop(ctx);
    let mut card = document.accounts[0].operations[0].clone();
    card.uid = 500;
    card.amount = Amount(-123_400);
    card.mode = bp_core::model::Mode::Standard(bp_core::model::StandardMode::Carte);
    document.accounts[0].operations.push(card);
    let ctx = DocContext {
        document: &mut document,
        file_id: 1,
        path: PathBuf::from("t.bp"),
        account: 0,
        selection: vec![],
        visible: vec![],
        locked: false,
        merge_tag: Some(3),
    };
    let fx = engine.fire(Event::OperationAdded, &[EventItem::line(0, 500)], ctx);
    assert!(fx.errors.is_empty(), "{:?}", fx.errors);
    let livret = &document.accounts[1].operations;
    assert_eq!(
        livret.last().unwrap().amount,
        Amount(6_600),
        "12,34 € arrondis à 13 €"
    );
}
