//! Saisie : nouvelle opération, modification, suppression, pointage, et ce
//! qui aide à saisir vite (tiers déjà utilisés, catégorie proposée).

use bp_core::autofields::AutoFieldRules;
use bp_core::categorize::{AUTO_THRESHOLD, Categorizer, Query};
use bp_core::filter::normalize;
use bp_core::model::{Amount, Date, Document, Mode, Operation, StandardMode, Tack};

use crate::view::TackState;
use crate::{BpError, Inner, Session};

/// Opération en cours de saisie, telle que dans le formulaire.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Draft {
    /// `jj/mm/aaaa`.
    pub date: String,
    /// Position dans [`Session::modes`].
    pub mode: u32,
    pub payee: String,
    pub details: String,
    pub category: Option<u32>,
    /// Montant sans signe, tel que tapé (`12,50`).
    pub amount: String,
    /// Recette (sinon dépense).
    pub credit: bool,
    pub tack: TackState,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ModeItem {
    pub index: u32,
    pub label: String,
    /// Mode de dépense : choisir ce mode bascule le montant en dépense.
    pub debit: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CategoryItem {
    pub id: u32,
    pub name: String,
    /// `Parent > Enfant` pour une sous-catégorie.
    pub label: String,
    pub child: bool,
}

/// Proposition pour le champ tiers.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PayeeChoice {
    pub caption: String,
    /// Tiers mémorisé : remplit aussi mode, catégorie, détail et montant.
    pub memorized: bool,
}

/// Catégorie proposée pour la saisie en cours.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CategoryGuess {
    pub id: u32,
    pub label: String,
    /// Assez sûre pour être choisie d'office.
    pub confident: bool,
}

fn mode_index(document: &Document, mode: &Mode) -> u32 {
    document
        .mode_list()
        .iter()
        .position(|m| m == mode)
        .unwrap_or(0) as u32
}

fn mode_at(document: &Document, index: u32) -> Mode {
    document
        .mode_list()
        .into_iter()
        .nth(index as usize)
        .unwrap_or_default()
}

/// Montant signé d'un brouillon (zéro s'il est vide ou illisible).
fn signed_amount(draft: &Draft) -> Option<Amount> {
    let text = draft.amount.trim();
    let amount = if text.is_empty() {
        Amount::ZERO
    } else {
        Amount::parse(text)?.abs()
    };
    Some(if draft.credit { amount } else { -amount })
}

impl Inner {
    fn categorizer(&mut self) -> &Categorizer {
        if self
            .categorizer
            .as_ref()
            .is_none_or(|(r, _)| *r != self.revision)
        {
            self.categorizer = Some((self.revision, Categorizer::new(&self.document)));
        }
        &self.categorizer.as_ref().expect("construit").1
    }

    /// Règles automatiques actives sur une ligne nouvelle.
    fn apply_rules(&mut self, account: usize, line: usize) {
        let Ok(rules) = AutoFieldRules::load(&self.document, &bp_core::legacy_pickle::parse_py)
        else {
            return;
        };
        if rules.rules.iter().any(|r| r.active && r.auto_apply) {
            rules.apply_to_new_lines_in_document(&mut self.document, &[(account, line)]);
        }
    }
}

#[uniffi::export]
impl Session {
    /// Modes de paiement : les standard puis ceux de l'utilisateur.
    pub fn modes(&self) -> Vec<ModeItem> {
        let inner = self.lock();
        inner
            .document
            .mode_list()
            .iter()
            .enumerate()
            .map(|(i, mode)| ModeItem {
                index: i as u32,
                label: mode.label().to_string(),
                debit: bp_core::edit::mode_is_debit(&inner.document, mode),
            })
            .collect()
    }

    /// Catégories dans l'ordre du fichier.
    pub fn categories(&self) -> Vec<CategoryItem> {
        let inner = self.lock();
        let document = &inner.document;
        document
            .categories
            .iter()
            .map(|c| CategoryItem {
                id: c.id,
                name: c.name.clone(),
                label: document.category_label(c.id).unwrap_or_default(),
                child: c.is_child,
            })
            .collect()
    }

    /// Formulaire vierge : aujourd'hui, carte, dépense.
    pub fn new_draft(&self) -> Draft {
        let inner = self.lock();
        Draft {
            date: Date::today().to_string(),
            mode: mode_index(&inner.document, &Mode::Standard(StandardMode::Carte)),
            payee: String::new(),
            details: String::new(),
            category: None,
            amount: String::new(),
            credit: false,
            tack: TackState::None,
        }
    }

    /// Formulaire d'une opération existante.
    pub fn draft_of(&self, uid: u64) -> Result<Draft, BpError> {
        let inner = self.lock();
        let (a, i) = inner.find(uid)?;
        let op = &inner.document.accounts[a].operations[i];
        Ok(Draft {
            date: op.date.to_string(),
            mode: mode_index(&inner.document, &op.mode),
            payee: op.payee.clone(),
            details: op.details.clone(),
            category: op.category,
            amount: op.amount.abs().format(2, ','),
            credit: !op.amount.is_negative(),
            tack: op.tack.into(),
        })
    }

    /// Propositions pour le tiers tapé : tiers mémorisés, puis tiers déjà
    /// utilisés dans le compte (les plus récents d'abord).
    pub fn payee_choices(&self, account: u32, text: String) -> Vec<PayeeChoice> {
        let inner = self.lock();
        let needle = normalize(&text);
        let needle = needle.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        let document = &inner.document;
        let mut choices: Vec<PayeeChoice> = document
            .payees
            .iter()
            .filter(|p| normalize(&p.caption).contains(needle))
            .take(4)
            .map(|p| PayeeChoice {
                caption: p.caption.clone(),
                memorized: true,
            })
            .collect();
        let mut seen = std::collections::HashSet::new();
        if let Some(acc) = document.accounts.get(account as usize) {
            let mut ops: Vec<&Operation> = acc.operations.iter().collect();
            ops.sort_by_key(|op| std::cmp::Reverse(op.date));
            for op in ops {
                if choices.len() >= 8 {
                    break;
                }
                if op.payee.is_empty()
                    || op.payee == text
                    || !seen.insert(op.payee.as_str())
                    || !normalize(&op.payee).contains(needle)
                {
                    continue;
                }
                choices.push(PayeeChoice {
                    caption: op.payee.clone(),
                    memorized: false,
                });
            }
        }
        choices
    }

    /// Reporte le tiers choisi dans le formulaire : un tiers mémorisé remplit
    /// mode, catégorie, détail et montant ; un tiers déjà utilisé reprend le
    /// mode et la catégorie de sa dernière opération.
    pub fn fill_from_payee(&self, account: u32, draft: Draft, choice: PayeeChoice) -> Draft {
        let inner = self.lock();
        let document = &inner.document;
        let mut draft = draft;
        if choice.memorized {
            if let Some(p) = document.payees.iter().find(|p| p.caption == choice.caption) {
                draft.payee = if p.payee.is_empty() {
                    p.caption.clone()
                } else {
                    p.payee.clone()
                };
                draft.mode = mode_index(document, &p.mode);
                draft.credit = !bp_core::edit::mode_is_debit(document, &p.mode);
                if p.category.is_some() {
                    draft.category = p.category;
                }
                if !p.details.is_empty() {
                    draft.details = p.details.clone();
                }
                if let Some(a) = Amount::parse(&p.amount).filter(|&a| a != Amount::ZERO) {
                    draft.amount = a.abs().format(2, ',');
                }
            }
            return draft;
        }
        draft.payee = choice.caption.clone();
        let last = document.accounts.get(account as usize).and_then(|acc| {
            acc.operations
                .iter()
                .filter(|op| op.payee == choice.caption)
                .max_by_key(|op| (op.date, op.uid))
        });
        if let Some(op) = last {
            draft.mode = mode_index(document, &op.mode);
            draft.credit = !op.amount.is_negative();
            if op.category.is_some() {
                draft.category = op.category;
            }
        }
        draft
    }

    /// Catégorie proposée d'après le tiers, le détail, le montant et le mode
    /// (historique du fichier, dictionnaire de commerçants).
    pub fn guess_category(&self, draft: Draft) -> Option<CategoryGuess> {
        if draft.payee.trim().is_empty() && draft.details.trim().is_empty() {
            return None;
        }
        let mut inner = self.lock();
        let mode = mode_at(&inner.document, draft.mode);
        let amount = signed_amount(&draft).unwrap_or(Amount::ZERO);
        let suggestion = inner.categorizer().suggest(&Query {
            payee: &draft.payee,
            details: &draft.details,
            amount,
            mode: Some(&mode),
            merchant_code: None,
        });
        let best = suggestion.best()?;
        Some(CategoryGuess {
            id: best.category,
            label: inner
                .document
                .category_label(best.category)
                .unwrap_or_default(),
            confident: best.confidence >= AUTO_THRESHOLD,
        })
    }

    /// Avertissement quand le sens du montant contredit le mode (chèque émis
    /// en recette…). Rien ne bloque l'enregistrement.
    pub fn mode_warning(&self, draft: Draft) -> Option<String> {
        let inner = self.lock();
        let mode = mode_at(&inner.document, draft.mode);
        let amount = signed_amount(&draft)?;
        if !bp_core::edit::amount_contradicts_mode(&inner.document, &mode, amount) {
            return None;
        }
        Some(if amount.0 > 0 {
            format!(
                "Recette avec le mode « {} », d'ordinaire une dépense : vérifiez le sens",
                mode.label()
            )
        } else {
            format!(
                "Dépense avec le mode « {} », d'ordinaire une recette : vérifiez le sens",
                mode.label()
            )
        })
    }

    /// Enregistre le formulaire : nouvelle opération dans `account`
    /// (`uid` absent) ou modification. Renvoie l'identifiant de l'opération.
    pub fn save_operation(
        &self,
        account: u32,
        uid: Option<u64>,
        draft: Draft,
    ) -> Result<u64, BpError> {
        let mut inner = self.lock();
        let today = Date::today();
        let date = Date::parse_fr(&draft.date, today)
            .ok_or_else(|| BpError::invalid("Date invalide (jj/mm/aaaa)"))?;
        if draft.amount.trim().is_empty() {
            return Err(BpError::invalid("Saisissez un montant"));
        }
        let amount = signed_amount(&draft).ok_or_else(|| BpError::invalid("Montant invalide"))?;
        let mode = mode_at(&inner.document, draft.mode);
        // Le point-virgule sépare les champs de l'ancien format.
        let payee = draft.payee.trim().replace(';', " ");
        let details = draft.details.trim().replace(';', " ");
        let tack: Tack = draft.tack.into();
        let saved = match uid {
            Some(uid) => {
                let (a, i) = inner.find(uid)?;
                let op = &mut inner.document.accounts[a].operations[i];
                if op.tack != tack {
                    op.mark_date = today;
                }
                op.date = date;
                op.tack = tack;
                op.mode = mode;
                op.payee = payee;
                op.details = details;
                op.category = draft.category;
                op.amount = amount;
                uid
            }
            None => {
                let a = inner.account(account)?;
                let uid = inner.document.next_operation_uid();
                let operations = &mut inner.document.accounts[a].operations;
                operations.push(Operation {
                    uid,
                    date,
                    mark_date: if tack == Tack::None { date } else { today },
                    tack,
                    mode,
                    payee,
                    details,
                    category: draft.category,
                    amount,
                    extra: Vec::new(),
                });
                let line = operations.len() - 1;
                inner.apply_rules(a, line);
                uid
            }
        };
        inner.changed();
        Ok(saved)
    }

    pub fn delete_operation(&self, uid: u64) -> Result<(), BpError> {
        let mut inner = self.lock();
        let (a, i) = inner.find(uid)?;
        inner.document.accounts[a].operations.remove(i);
        inner.changed();
        Ok(())
    }

    /// Pointe ou dépointe d'un geste. Une ligne rapprochée ne se dépointe
    /// qu'en la modifiant (elle a été vérifiée sur un relevé).
    pub fn toggle_tack(&self, uid: u64) -> Result<TackState, BpError> {
        let mut inner = self.lock();
        let (a, i) = inner.find(uid)?;
        let op = &mut inner.document.accounts[a].operations[i];
        op.tack = match op.tack {
            Tack::None => Tack::Pointed,
            Tack::Pointed => Tack::None,
            Tack::Reconciled => {
                return Err(BpError::invalid(
                    "Ligne rapprochée : ouvrez-la pour changer son pointage",
                ));
            }
        };
        op.mark_date = Date::today();
        let tack = op.tack.into();
        inner.changed();
        Ok(tack)
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::{add, file, sample};
    use crate::*;

    fn session() -> Arc<Session> {
        let (mut document, protection) = sample();
        add(&mut document, "Boulangerie Dupont", -350);
        Session::open(file(&document, &protection), None).unwrap()
    }

    #[test]
    fn new_operation_and_edit() {
        let session = session();
        let mut draft = session.new_draft();
        draft.payee = "Pharmacie".into();
        draft.amount = "12,90".into();
        let uid = session.save_operation(0, None, draft).unwrap();
        assert!(session.is_dirty());
        let view = session
            .operations(0, "pharmacie".into(), TackView::All)
            .unwrap();
        assert_eq!(view.rows[0].amount, "-12,90\u{A0}€");
        assert_eq!(view.balance, "83,60\u{A0}€");

        let mut draft = session.draft_of(uid).unwrap();
        assert_eq!((draft.amount.as_str(), draft.credit), ("12,90", false));
        draft.credit = true;
        session.save_operation(0, Some(uid), draft).unwrap();
        let view = session.operations(0, String::new(), TackView::All).unwrap();
        assert_eq!(view.balance, "109,40\u{A0}€");
    }

    #[test]
    fn invalid_entries_are_refused() {
        let session = session();
        let mut draft = session.new_draft();
        draft.amount = "abc".into();
        assert!(matches!(
            session.save_operation(0, None, draft.clone()),
            Err(BpError::Invalid { .. })
        ));
        draft.amount = "5".into();
        draft.date = "31/02/2026".into();
        assert!(matches!(
            session.save_operation(0, None, draft),
            Err(BpError::Invalid { .. })
        ));
    }

    #[test]
    fn tack_toggle_and_delete() {
        let session = session();
        let uid = session
            .operations(0, "boulangerie".into(), TackView::All)
            .unwrap()
            .rows[0]
            .uid;
        assert_eq!(session.toggle_tack(uid).unwrap(), TackState::Pointed);
        assert_eq!(session.toggle_tack(uid).unwrap(), TackState::None);
        session.delete_operation(uid).unwrap();
        assert_eq!(
            session
                .operations(0, String::new(), TackView::All)
                .unwrap()
                .total,
            1
        );
    }

    #[test]
    fn payee_suggestions_fill_the_form() {
        let session = session();
        let choices = session.payee_choices(0, "boul".into());
        assert_eq!(
            choices,
            [PayeeChoice {
                caption: "Boulangerie Dupont".into(),
                memorized: false
            }]
        );
        let draft = session.fill_from_payee(0, session.new_draft(), choices[0].clone());
        assert_eq!(draft.payee, "Boulangerie Dupont");
        assert!(!draft.credit);
    }

    #[test]
    fn category_is_guessed_from_history() {
        let session = session();
        let courses = session
            .categories()
            .into_iter()
            .find(|c| c.label.contains("Alimentation") || c.name.contains("Courses"))
            .expect("catégorie standard")
            .id;
        // Trois achats déjà classés chez ce commerçant.
        for _ in 0..3 {
            let mut draft = session.new_draft();
            draft.payee = "Biocoop Lyon".into();
            draft.amount = "20".into();
            draft.category = Some(courses);
            session.save_operation(0, None, draft).unwrap();
        }
        let mut draft = session.new_draft();
        draft.payee = "Biocoop Lyon".into();
        let guess = session.guess_category(draft).expect("proposition");
        assert_eq!(guess.id, courses);
    }

    #[test]
    fn mode_warning_on_contradiction() {
        let session = session();
        let cheque = session
            .modes()
            .into_iter()
            .find(|m| m.label.contains("Chèque émis") || m.label.contains("Chq"))
            .map(|m| m.index);
        let Some(cheque) = cheque else { return };
        let mut draft = session.new_draft();
        draft.mode = cheque;
        draft.amount = "10".into();
        draft.credit = true;
        assert!(session.mode_warning(draft.clone()).is_some());
        draft.credit = false;
        assert!(session.mode_warning(draft).is_none());
    }
}
