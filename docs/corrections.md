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
| C20 | Échéancier : le choix « Les lignes qui ne s'insèrent pas automatiquement » testait la clé inexistante `auto_insert` et ne sélectionnait jamais rien. | Sélectionne les échéances sans insertion automatique. | 04 §2.5, §7 |
| C21 | Échéancier : une échéance mensuelle tombant un dernier jour de mois passait définitivement en fin de mois (le 30/04 donnait 31/05, 30/06, 31/07… ; le 28/02 donnait 31/03). « Ajouter à l'échéancier » sur une opération du 28/02 créait ainsi une échéance au 31/03. | Le jour du mois voulu est mémorisé (`anchor_day`) : le 30 reste le 30, le 31 reste en fin de mois. Une ancienne échéance déjà en fin de mois garde ce comportement. | 04 §2.5 |
| C22 | Échéance annuelle au 29/02 : date invalide 29/02 les années non bissextiles, ligne refusée. | Insérée le 28/02, et de nouveau le 29/02 les années bissextiles. | 04 §2.5 |
| C23 | Intervalle nul ou négatif : boucle sans fin (application manuelle, prévisionnel), échéance ignorée sans message à l'ouverture. | Échéance refusée avec le motif « intervalle nul ». | 04 §2.5 |
| C24 | Les échéances désignaient leur compte par sa position : supprimer ou insérer un compte reportait les échéances sur un autre compte. | Le compte est désigné par son identifiant (`uid`) ; une échéance dont le compte a disparu est refusée. | 04 §2.5 |
| C25 | À l'ouverture, une échéance dont le compte dépassait le dernier compte était ignorée sans message (seuls les index négatifs étaient signalés). | Refusée et signalée (« compte inexistant »). | 04 §2.5 |
| C26 | Les deux lignes d'un transfert inséré par l'échéancier n'étaient pas liées. | Liées par `#sisterAccUid` / `#sisterLineUid`, comme le script Transfert. | 04 §2.5, §2.6 |
| C27 | Prévisionnel : les transferts reçus d'un autre compte (échéance portée par le compte lié) étaient ignorés. | Comptés avec le montant de la ligne réciproque. | 04 §2.3 |
| C28 | Prévisionnel : les opérations non pointées postérieures à la date choisie étaient comptées quand même. | Seules les opérations jusqu'à la date choisie sont comptées. | 04 §2.3 |
| C29 | Prévisionnel : dates des échéances sans le report week-end/férié ; plantage (division par zéro) si tous les soldes étaient nuls. | Report appliqué comme à l'insertion ; barre de progression à 0. | 04 §2.3 |

## À faire (fonctionnalités pas encore portées)

- Un détail contenant `m=` imposait le mode de paiement.
- Compteur des sauvegardes `.bp0` commun à tous les fichiers, 20 emplacements fixes (le manuel annonce 30, paramétrables).
- Bugs des scripts relevés dans `docs/inventaire/04-scripts.md` (Luhn faux pour le chiffre 9, `{count-1}` non remplacé…).
