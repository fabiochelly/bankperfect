# Corrections de bugs de l'ancienne version

Décision : la nouvelle version reproduit le comportement **voulu** de BankPerfect, pas ses bugs.
Chaque écart volontaire avec l'ancienne version est listé ici. Les références renvoient aux
inventaires de `docs/inventaire/`.

## Fait

| # | Ancien comportement | Nouveau comportement | Réf. |
|---|---|---|---|
| C1 | Un dernier compte sans opération disparaissait au rechargement (puis erreur à la sauvegarde). | Le compte est conservé avec une liste d'opérations vide. | 02 §5.3 |
| C2 | Une opération dont le mode personnalisé avait été supprimé était réécrite avec le code 19 et redevenait « Carte ». | Le mode est recréé dans la liste des modes personnalisés ; l'opération le garde. | 02 §6.4 |
| C3 | Un tiers (`whoN`) avec moins de 6 champs faisait déclarer tout le fichier corrompu. | Les champs manquants prennent leur valeur par défaut. | 02 §4.8 |
| C4 | Une seule opération sans UId provoquait la renumérotation de toutes les lignes et l'effacement des liens de transfert (`#sisterLineUid`). | Seules les opérations sans UId (ou en double) reçoivent un nouvel identifiant ; les liens sont préservés. | 02 §7 |
| C5 | Un montant contenant à la fois `,` et `.` (ex. `1.234,56`) était lu comme 0. | Le dernier séparateur est décimal, les autres sont des séparateurs de milliers. | 02 §6.7 |
| C6 | Les noms de catégories pouvaient être tronqués (`Copy(s, p+1, p+51)`). | Aucune troncature. | 02 §4.9 |
| C7 | `accounts_order` non numérique : exception non gérée ; doublons ou index manquants acceptés. | Lecture tolérante, l'ordre est réparé en permutation valide. | 02 §4.10 |
| C8 | `compte_defaut` hors bornes accepté. | Ramené au premier compte. | 02 §4.11 |
| C9 | Mot de passe maître codé en dur acceptant n'importe quel fichier. | Supprimé. | 02 §10.2 |
| C10 | Clés d'en-tête inconnues supprimées à la sauvegarde. | Conservées. | 02 §4.1 |
| C11 | `UserName` absent : la valeur du fichier ouvert précédemment était héritée et écrite. | La valeur du fichier lui-même est utilisée. | 02 §4.5 |
| C12 | Avec l'option Précision à 0, les centimes étaient perdus à l'écriture. | Les montants sont toujours stockés au dix-millième ; la précision ne concerne que l'affichage. | 02 §6.7 |
| C13 | `SecuritySave` / `SyncMoveFile` pouvaient boucler sans fin. | Écriture atomique (fichier temporaire, fsync, renommage). | 02 §8.2 |
| C14 | Mot de passe stocké en clair dans le fichier, protégé par une simple obfuscation. | Format v6 : chiffrement XChaCha20-Poly1305, clé dérivée par Argon2id, clé de secours. | 02 §10 |
| C15 | Le filtre `=carte` annoncé dans l'aide ne trouvait rien (`=` était effacé par la normalisation). | `=texte` est une égalité stricte. | 01 FILT-01 |
| C16 | Le filtre tiers cherchait aussi dans le texte brut `UId=…` : « uid » trouvait tout et `//` (tiers vide) ne trouvait jamais rien. | L'UId est exclu de la recherche ; `//` trouve les champs vides. | 01 FILT-01 |
| C17 | Tri instable (QuickSort) : l'ordre des opérations égales variait d'un affichage à l'autre, et le tri réordonnait le fichier. | Tri stable, sur une vue : l'ordre stocké n'est pas modifié. | 01 GRID-19 |
| C18 | Impossible de supprimer la dernière opération d'un compte (« Vous devez garder au moins une ligne »), contrainte du format. | Un compte peut être vide (C1 garantit qu'il est conservé). | 01 EDIT-06 |
| C19 | La suppression marquait les lignes avec la date 28/06/1973 puis supprimait toutes les lignes à cette date : une vraie opération du 28/06/1973 disparaissait aussi. | Suppression par position. | 01 EDIT-10 |
| C20 | Calcul automatique des champs : une formule de condition en erreur affichait un message puis la règle cherchait le texte de la formule comme des mots ordinaires (elle pouvait s'appliquer par accident). | La ligne n'est pas retenue et l'erreur est signalée (`RuleOutcome::errors`). | 04 §2.6 Auto Fields |
| C21 | Calcul automatique des champs : une formule d'action (tiers, détail, date) en erreur écrivait son propre texte (`=details[:10]`) dans le champ ; hors `tiers`/`details`/`date`/`montant`/`mode`, les variables étaient indéfinies ou gardaient la valeur de la dernière ligne testée. | Le champ est laissé inchangé et l'erreur signalée ; toutes les variables valent celles de l'opération traitée (avant les actions de la règle). | 04 §2.6 Auto Fields |
| C22 | « Modifier uniquement les opérations sans catégorie » ne protégeait que la catégorie : mode, tiers, détail et date étaient quand même modifiés sur les lignes déjà catégorisées (qui étaient retirées du compteur). | La règle ne s'applique qu'aux opérations sans catégorie. | 04 §2.6 Auto Fields |
| C23 | Catégorie à appliquer inexistante : règle refusée en mode interactif, mais identifiant écrit tel quel en mode automatique (opération rattachée à une catégorie inexistante). | Règle refusée dans tous les cas (`RuleError::UnknownCategory`). | 04 §2.6 Auto Fields |
| C24 | Formules des règles, dates dynamiques et import `.txt` des règles évalués par `eval()` Python : exécution de code arbitraire possible depuis une règle ou un fichier importé ; un mode hors liste ou une valeur cherchée vide faisaient planter le script. | Interpréteur restreint (`formula.rs` : littéraux, opérateurs, `in`, tranches, fonctions et méthodes de chaîne listées) et relecture des littéraux sans `eval` (`pyvalue.rs`) ; règles mal formées refusées avant toute modification, import refusé en entier avec le numéro de ligne fautive. | 04 §4.1 |
| C25 | Comparaisons « sans casse » limitées à l'ASCII (chaînes Python 2 en octets CP1252) : `édf` ne trouvait pas `ÉDF`. | Minuscules Unicode. | 04 §2.6 Auto Fields |
| C26 | Nombre de lignes modifiées annoncé : lignes trouvées même inchangées, additionnées règle par règle (une ligne touchée par deux règles comptait deux fois). | Lignes réellement modifiées, sans doublon (`BatchOutcome::changed`). | 04 §2.6 Auto Fields |
| C27 | Date imposée par une règle : la formule `=date` ne pouvait jamais marcher (la variable vaut `jj-mm-aaaa`, seul `jj/mm/aaaa` était accepté) ; une date invalide était ignorée sans message. | `jj/mm/aaaa` et `jj-mm-aaaa` acceptés ; une date invalide est signalée. | 04 §2.6 Auto Fields |
| C28 | Budget annuel : la colonne « Total budget » acceptait une saisie (mois n° 12 : plantage, ou les 12 mois remplacés avec « A tous les mois »). | Seules les colonnes Budget et Réel d'un mois sont modifiables (`GridColumn::editable`). | 04 §2.4 BudgetYear |
| C29 | Budget annuel : dupliquer deux fois un tableau créait deux tableaux nommés « Copie de X » (noms censés être uniques). | Suffixe « (2) », « (3) »… | 04 §2.4 BudgetYear |
| C30 | Budget annuel : comptes retenus mémorisés par indice ; après suppression ou ajout d'un compte, d'autres comptes étaient pris en compte. | Comptes mémorisés par identifiant (`Account::uid`). | 04 §2.4 BudgetYear |
| C31 | Budget mensuel, mode « jours / 30,4375 » : le dernier jour n'était pas compté ; avec une seule journée d'opérations (0 jour), aucune moyenne n'était faite (totaux bruts) ; sans opération, la date du jour servait de borne et le libellé annonçait des milliers de mois. | Jours comptés bornes incluses ; durée nulle sans opération. | 04 §2.4 Budget |
| C32 | Budget mensuel, « Lignes à zéro » décochée et sous-catégories agrégées : une catégorie parente sans opération propre était masquée même si ses sous-catégories en avaient. | Le test porte sur le montant agrégé affiché. | 04 §2.4 Budget |
| C33 | Filtres prédéfinis : après suppression de tous les filtres, « Mois actuel » et « Mois précédent » réapparaissaient au chargement suivant. | Une liste vidée reste vide (un ancien fichier dont la liste est vide reçoit les filtres par défaut, comme avant). | 04 §2.6 Filters |
| C34 | Filtres prédéfinis : une date dynamique invalide était ignorée sans message, et les constantes étaient remplacées dans le texte (`aa` devenait `20262026`). | Dates vérifiées à l'enregistrement (`AutoFilter::validate`) ; constantes reconnues comme des noms entiers. | 04 §2.6 Filters |
| C35 | Saisie d'un montant de budget : `100/3` donnait 33 (division entière Python 2). | Vraie division (33,3333). | 04 §4.1 |

## À faire (fonctionnalités pas encore portées)

- Un détail contenant `m=` imposait le mode de paiement.
- Compteur des sauvegardes `.bp0` commun à tous les fichiers, 20 emplacements fixes (le manuel annonce 30, paramétrables).
- Bugs des scripts relevés dans `docs/inventaire/04-scripts.md` (Luhn faux pour le chiffre 9, `{count-1}` non remplacé, filtre `auto_insert` de l'échéancier…).
