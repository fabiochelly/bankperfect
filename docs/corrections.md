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
| C20 | Import CSV : `GetCtg` renvoyait, dans la branche « catégorie ou sous-catégorie seule », la position de la catégorie dans la liste au lieu de son identifiant (et contenait l'expression `c in t > -1`) : l'opération recevait une autre catégorie. | La recherche renvoie toujours un identifiant (`import::resolve_category`). | 04 §2.1, §7 |
| C21 | Import OFX : le libellé « Fichier à importer (une ligne) » n'était jamais affiché (affectation manquante). | Le nombre d'opérations lisibles est fourni par l'analyse, avec son libellé (`operation_count_label` : « aucune opération », « une opération », « N opérations »). | 04 §2.1, §7 |
| C22 | Imports QIF et CSV : une année sur 2 chiffres recevait 2000 (`98` → 2098) ; en Python 3 le calcul du siècle de l'import CSV donnait une année décimale. | Fenêtre glissante, comme la saisie (`Date::parse_fr`) : une année plus de 20 ans dans le futur est ramenée au siècle précédent (`98` → 1998, `02` → 2002). | 04 §2.1, §4.1 |
| C23 | Import CSV avec colonnes Débit et Crédit : seule la dernière colonne lue comptait ; un débit accompagné d'un crédit `0,00` donnait un montant nul et la ligne était refusée. Un montant illisible faisait classer la ligne en « enregistrement incorrect ». | Montant = crédit − débit (valeurs absolues) ; une seule des deux colonnes suffit ; un montant illisible est signalé comme tel. | 04 §2.1 |
| C24 | Doublons (QIF, CSV, OFX) : une seule opération existante suffisait à faire refuser toutes les lignes identiques du fichier (deux cafés au même prix le même jour : le second était perdu). | Chaque opération existante ne justifie qu'un doublon (`import::detect_duplicates`) ; tolérance de date et prise en compte du mode réglables ; un même `FITID` OFX est un doublon certain. | 04 §2.1 |
| C25 | Import OFX : une transaction sans `DTPOSTED` (ou avec une date illisible) faisait échouer tout le script ; un `TRNAMT` illisible n'était signalé qu'à l'insertion. | La transaction est écartée et signalée (date absente ou incorrecte, montant absent ou incorrect), les autres sont importées. | 04 §2.1 |
| C26 | Import OFX : analyse par découpage de chaînes ; les entités (`&amp;`) restaient telles quelles, les sections `CDATA` et commentaires XML n'étaient pas gérés, l'OFX 2.x ne fonctionnait « que par chance », et l'`ACCTID` d'un compte destinataire (`BANKACCTTO`) pouvait être pris pour celui du relevé. | Analyse par balises tolérante (SGML 1.x et XML 2.x), entités décodées, identifiants du compte lus hors des transactions seulement. | 04 §2.1 |
| C27 | Imports : fichiers lus en octets bruts (CP1252 implicite) ; un fichier UTF-8 donnait des « Ã© ». | Détection : BOM UTF-8/UTF-16, sinon UTF-8 s'il est valide, sinon Windows-1252 (`import::decode_text`). | 04 §4.3 |
| C28 | Import QIF : le dernier enregistrement sans `^` final était perdu ; les sections `!Type:Cat`, `!Type:Class`, `!Type:Memorized` et `!Account` étaient lues comme des opérations (et rejetées) ; `L[Compte]` (virement QIF) créait une catégorie « [Compte] ». | Dernier enregistrement lu ; seules les sections bancaires (`Bank`, `Cash`, `CCard`, `Oth A`, `Oth L`) sont lues, chaque `!Account` donne un compte distinct ; `L[…]` ne donne pas de catégorie. | 04 §2.1 |
| C29 | Import QIF : les catégories étaient comparées aux motifs des chaînes « id=nom », si bien que la correspondance exacte n'aboutissait jamais et que `Catégorie:Sous-catégorie` n'était jamais retrouvée (création d'un doublon de catégorie). | `L` est découpé en catégorie et sous-catégorie, retrouvées par nom (égalité puis inclusion), la sous-catégorie départagée par son parent. | 04 §2.1 |
| C30 | Imports QIF et CSV : une catégorie dont le nom ne contient ni lettre ni chiffre (« --- ») avait un motif vide qui correspondait à toutes les recherches. | Ces catégories sont ignorées par la recherche. | 04 §2.1 |
| C31 | « Créer les catégories manquantes » (CSV) créait une catégorie principale nommée « Catégorie Sous-catégorie ». | La sous-catégorie est créée sous son parent (lui-même créé au besoin), y compris quand seul le parent existe (`import::ensure_category`). | 04 §2.1 |
| C32 | Imports QIF et CSV : sans champ « mode », toute dépense devenait « Prélèvement » et toute recette « Virement reçu » ; l'import OFX n'examinait que `NAME`. | Le mode est aussi déduit des mots du tiers puis du détail (`CB`, `PRLV`, `VIR`, `RETRAIT`, `CHEQUE`…), mots entiers seulement (« CARREFOUR » n'est pas une carte) ; l'OFX examine `NAME` puis `MEMO`, y compris pour les types `OTHER` ou absents. | 04 §2.1 |
| C33 | Import OFX : les transactions de montant nul étaient insérées, alors que QIF et CSV les refusaient. | Refusées et signalées (« montant nul ») dans les trois formats. | 04 §2.1 |
| C34 | Import OFX : banques qui ne remplissent que `MEMO` → tiers vide ; banques qui répètent le tiers dans `MEMO` → détail en double. | Tiers vide : le mémo devient le tiers ; mémo identique au tiers : détail vide. | 04 §2.1 |

## À faire (fonctionnalités pas encore portées)

- Un détail contenant `m=` imposait le mode de paiement.
- Compteur des sauvegardes `.bp0` commun à tous les fichiers, 20 emplacements fixes (le manuel annonce 30, paramétrables).
- Bugs des scripts relevés dans `docs/inventaire/04-scripts.md` (Luhn faux pour le chiffre 9, `{count-1}` non remplacé, filtre `auto_insert` de l'échéancier…).
