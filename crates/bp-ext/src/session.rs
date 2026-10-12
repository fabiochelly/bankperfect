//! Session : le document prêté à une extension le temps d'un appel, avec ce
//! qu'il faut pour annuler ou défaire ses modifications.

use std::path::PathBuf;

use bp_core::model::{Document, Operation, Referentials};

/// Ce que l'application prête à une extension pour un appel.
pub struct DocContext<'a> {
    pub document: &'a mut Document,
    /// Identifiant du fichier ouvert (change à chaque ouverture) : les objets
    /// gardés par une extension d'un fichier à l'autre sont refusés.
    pub file_id: u64,
    pub path: PathBuf,
    /// Compte affiché (index réel).
    pub account: usize,
    /// Lignes sélectionnées et affichées du compte affiché, dans l'ordre
    /// d'affichage.
    pub selection: Vec<usize>,
    /// Lignes affichées du compte affiché, dans l'ordre d'affichage.
    pub visible: Vec<usize>,
    /// « Verrouiller les lignes rapprochées » est actif.
    pub locked: bool,
    /// Étape d'annulation à compléter (crochets lancés après une action de
    /// l'utilisateur) : les modifications s'y ajoutent au lieu d'en créer une.
    pub merge_tag: Option<u64>,
}

/// Modifications d'un appel, à inscrire dans l'historique d'annulation.
#[derive(Debug, Clone)]
pub struct Changes {
    /// Libellé de l'étape (« Annuler <libellé> »).
    pub label: String,
    /// Les modifications d'un même lancement (avant et après une fenêtre)
    /// portent la même étiquette et rejoignent la même étape si rien ne s'est
    /// intercalé.
    pub tag: u64,
    /// Opérations des comptes touchés, avant la modification.
    pub accounts: Vec<(usize, Vec<Operation>)>,
    /// Référentiels avant la modification, s'ils ont changé.
    pub referentials: Option<Referentials>,
    /// Des comptes ont été créés.
    pub accounts_added: bool,
}

/// Document et état d'un appel en cours (donnée d'application de la VM).
pub(crate) struct Session {
    pub document: Document,
    pub file_id: u64,
    pub path: PathBuf,
    pub account: usize,
    pub selection: Vec<usize>,
    pub visible: Vec<usize>,
    pub locked: bool,
    /// Prochain identifiant d'opération (calculé une fois).
    next_uid: Option<u64>,
    /// Opérations d'origine des comptes modifiés pendant le segment.
    originals: Vec<(usize, Vec<Operation>)>,
    referentials: Option<Referentials>,
    accounts_before: usize,
    /// Données propres aux extensions rangées dans le fichier modifiées.
    pub feature_changed: bool,
    /// Compte à afficher et lignes à sélectionner (identifiants).
    pub select: Option<(usize, Vec<u64>)>,
}

impl Session {
    pub fn new(ctx: &mut DocContext<'_>) -> Self {
        let document = std::mem::take(ctx.document);
        let accounts_before = document.accounts.len();
        Self {
            document,
            file_id: ctx.file_id,
            path: ctx.path.clone(),
            account: ctx.account,
            selection: ctx.selection.clone(),
            visible: ctx.visible.clone(),
            locked: ctx.locked,
            next_uid: None,
            originals: Vec::new(),
            referentials: None,
            accounts_before,
            feature_changed: false,
            select: None,
        }
    }

    /// Rend le document à l'application.
    pub fn finish(self, ctx: &mut DocContext<'_>) {
        *ctx.document = self.document;
    }

    /// Opérations d'un compte, mémorisées avant leur première modification.
    pub fn operations_mut(&mut self, account: usize) -> &mut Vec<Operation> {
        if !self.originals.iter().any(|(a, _)| *a == account) && account < self.accounts_before {
            let saved = self.document.accounts[account].operations.clone();
            self.originals.push((account, saved));
        }
        &mut self.document.accounts[account].operations
    }

    /// Document dont les référentiels vont changer (mémorisés avant).
    pub fn referentials_mut(&mut self) -> &mut Document {
        if self.referentials.is_none() {
            self.referentials = Some(self.document.referentials());
        }
        &mut self.document
    }

    /// Toutes les opérations vont changer (suppression d'une catégorie…).
    pub fn touch_all_accounts(&mut self) {
        for a in 0..self.document.accounts.len() {
            self.operations_mut(a);
        }
    }

    /// Des identifiants ont été attribués hors de la session (transferts).
    pub fn reset_uid(&mut self) {
        self.next_uid = None;
    }

    pub fn next_uid(&mut self) -> u64 {
        let uid = self
            .next_uid
            .unwrap_or_else(|| self.document.next_operation_uid());
        self.next_uid = Some(uid + 1);
        uid
    }

    /// Clôt un segment : renvoie ses modifications (ou rien).
    pub fn take_changes(&mut self, label: &str, tag: u64) -> Option<Changes> {
        let accounts_added = self.document.accounts.len() > self.accounts_before;
        self.accounts_before = self.document.accounts.len();
        if self.originals.is_empty() && self.referentials.is_none() && !accounts_added {
            return None;
        }
        Some(Changes {
            label: label.to_string(),
            tag,
            accounts: std::mem::take(&mut self.originals),
            referentials: self.referentials.take(),
            accounts_added,
        })
    }

    /// Défait les modifications du segment (erreur dans l'extension).
    pub fn rollback(&mut self, before_accounts: usize) {
        for (a, operations) in std::mem::take(&mut self.originals) {
            if let Some(account) = self.document.accounts.get_mut(a) {
                account.operations = operations;
            }
        }
        if let Some(saved) = self.referentials.take() {
            self.document.swap_referentials(saved);
        }
        self.document.accounts.truncate(before_accounts);
        self.document.account_order.retain(|&i| i < before_accounts);
        self.accounts_before = before_accounts;
        self.next_uid = None;
        self.select = None;
    }

    pub fn accounts_before(&self) -> usize {
        self.accounts_before
    }
}
