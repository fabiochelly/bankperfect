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
| C20 | Clé de Luhn : `(2 × chiffre) % 9` comptait 0 pour un 9 en position doublée (des cartes valides étaient refusées) ; seuls les numéros de 16 chiffres étaient acceptés. | Algorithme de Luhn exact (doublé au-delà de 9 : on retire 9), doublement depuis la droite ; 13 à 19 chiffres (American Express : 15). | 04 §7 luhn |
| C21 | Relevé multicompte : les totaux « Pointé » et « A venir » étaient inversés ; les lignes étaient juxtaposées compte par compte. | « Pointé » = lignes pointées ou rapprochées ; lignes fusionnées et triées par date (puis compte, puis ordre du compte). | 04 §7 all_accounts_view |
| C22 | Totaux par catégorie : plantage (`min()` sur une liste vide) dès qu'un compte n'avait aucune opération ; années calculées sur tous les comptes, même hors sélection. | Aucun plantage (tableau sans colonne d'année si rien à afficher) ; années des seuls comptes sélectionnés ; une opération de catégorie supprimée compte dans « Sans catégorie ». | 04 §7 categs_totals |
| C23 | Totaux par tiers : en totaux mensuels « Toutes années », toutes les cellules valaient 0 ; les moyennes annuelles étaient divisées par le nombre d'années au lieu de 12 ; « Tous les comptes » incluait les comptes virtuels. | Mois cumulés sur toutes les années ; moyenne = total de l'année / 12 (comme les catégories) ; comptes virtuels exclus de « Tous les comptes ». Regroupement des tiers sans préfixe de ventilation `[i/N]` ni ponctuation. | 04 §2.3 people_totals |
| C24 | Simulateur de crédit : `rates.append` (NameError) dans une branche d'erreur ; la valeur résiduelle et le « début de période » n'étaient pris en compte que dans le tableau, pas dans le calcul de la mensualité, de la durée, du taux ni du coût total. | Plus d'exception ; les quatre calculs et le coût total intègrent résiduel et début de période ; taux par dichotomie (convergence garantie) ; dernière échéance ajustée pour une durée fractionnaire. | 04 §7 loan |
| C25 | Nombres en lettres (`numerals.py`, `receipt.py`) : « dix sept » sans trait d'union, « milliers-de-billions » pour 10^15, montants du reçu en « … euros et … cents » (« zéro euros », « un euros »). Piège Python 3 : `N / 10` rendait le texte faux. | Orthographe traditionnelle (vingt et un, soixante et onze, quatre-vingts, deux cents / deux cent un, mille invariable, millions accordés, billiard) ; « euro(s) et centime(s) », « un euro », « un million d'euros », arrondi au centime. | 04 §2.7 numerals, §2.3 receipt |
| C26 | Chiffres romains : les écritures non canoniques (« IIII », « IC ») étaient additionnées sans contrôle. | Seules les écritures canoniques de 1 à 3999 sont acceptées. | 04 §2.7 numerals |
| C27 | Clé RIB : un numéro de compte contenant un caractère autre qu'une lettre ou un chiffre était lu comme 0 (clé fausse affichée). | Pas de clé dans ce cas ; ajout de la validation et du calcul de l'IBAN. | `UnitAccounts.pas` `clef_rib` |
| C28 | Convertir la ligne en transfert : les champs de liaison étaient écrits même si l'ajout de la ligne miroir avait échoué (sur la ligne 0 de l'autre compte) ; une ligne déjà transfert pouvait être reconvertie ; détail « tiers, » avec une virgule finale si le détail était vide. | Tout est vérifié avant la moindre modification ; une ligne déjà transfert est refusée ; pas de virgule superflue. | 04 §7 convert_line_to_transfer |
| C29 | Transfert de compte à compte : traces de débogage (`print`) ; date par défaut faussée par `to_delphi_date` en Python 3. | Supprimées ; la date est une `Date`. | 04 §7 transfer |
| C30 | Ventilation : les parts étaient recréées non pointées, en fin de compte, sans les champs supplémentaires ; reventiler une part cumulait les préfixes (« [1/2] [1/3] … ») ; montants en flottants arrondis à 2 décimales. | Les parts gardent le pointage, la place de la ligne d'origine et ses champs supplémentaires (sauf les liens de transfert) ; préfixe non cumulé ; somme vérifiée exactement en dix-millièmes. | 04 §2.6 ventilate |
| C31 | Saisie multiple : total affiché sans le signe imposé par le mode ; « Virement reçu » tapé en entier devenait « Virement émis » ; modes personnalisés ignorés ; montant absent en mode rapide pris pour 0 ; date illisible remplacée par la date du jour sans prévenir ; abréviation dont le texte contient `=` tronquée ; numéro de chèque dupliqué quand le détail finissait par `n°NNN`. | Total signé ; un libellé complet de mode (standard ou personnalisé) est reconnu tel quel ; montant absent ou date illisible = ligne invalide (en rouge) ; texte d'abréviation conservé entier ; numéro de chèque ajouté seulement s'il manque (`ch123`/`chq 123` reconnus). | 04 §2.6 multiple_input |

## À faire (fonctionnalités pas encore portées)

- Un détail contenant `m=` imposait le mode de paiement.
- Compteur des sauvegardes `.bp0` commun à tous les fichiers, 20 emplacements fixes (le manuel annonce 30, paramétrables).
- Bugs des scripts relevés dans `docs/inventaire/04-scripts.md` restant à porter (`{count-1}` non remplacé, filtre `auto_insert` de l'échéancier…).
