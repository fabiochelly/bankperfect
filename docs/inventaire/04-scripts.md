# 04 — Inventaire des scripts Python (`Scripts/`)

> Périmètre : les scripts Python 2.3 livrés avec BankPerfect dans `Scripts/` (hors `gr322/`, qui est la bibliothèque Delphi Graphics32, et `icons2/`).
> Objectif : préparer la conversion en Python 3, avec la même API hôte (`import BP`, `BPEval`, `CreateComponent`…), exécutée par le futur hôte Rust.
> Méthode : lecture intégrale des 44 fichiers `.py` (convertis depuis CP1252), des 44 `.ini`, des fichiers annexes (`.slk`, `.dat`, `mapping.ini`), et vérification croisée avec `unitMain.pas`, `FormPython.pas`, `ObjectScriptsData.pas`, `manuel.html` et `plugins_bankperfect.htm`.
> Aucune donnée personnelle des fichiers `.bp`/`.bp0` ni des `.dat` n'est reproduite ici.

---

## 0. Vue d'ensemble

| Indicateur | Valeur |
|---|---|
| Sous-dossiers de `Scripts/` | 40 (38 dossiers de scripts + `gr322/` + `icons2/`) |
| Scripts déclarés (fichiers `.ini` `[BANKPERFECT_SCRIPT]`) | 44 |
| Fichiers `.py` actifs | 44 (+ `Notes/notes.py.bak`, une ancienne copie) |
| Lignes de Python | 10 798 (plus que les 8 000 estimés) |
| Encodage des sources | CP1252 / ISO-8859-1, presque toujours sans déclaration `coding` valide (`#coding = iso-8859-1` n'est pas reconnu par la PEP 263 à cause des espaces autour du `=`) |
| `gr322/` | Sources Pascal de Graphics32 (43 fichiers `.pas`/`.inc`/`.dfm`/`.lfm`/`.lrs`). Rien à voir avec les scripts, rangé là par erreur |
| `icons2/` | **Vide** (0 fichier) |

### 0.1 Clés du fichier `.ini` d'un script (lues dans `unitMain.pas`, `TFormMain.LoadScripts`)

| Clé | Défaut | Rôle |
|---|---|---|
| `Name` (obligatoire) | — | Libellé du menu, et identifiant utilisé par `DisableScripts` |
| `Path` (obligatoire) | — | Fichier `.py`, relatif au dossier du `.ini` |
| `Icon` | — | Index d'image dans la liste d'icônes de l'appli (ou nom de PNG 24×24 selon la doc) |
| `Menu` | — | Liste séparée par des virgules : `fichier`, `importer`, `exporter`, `edition`, `affichage`, `comptes`, `operations`, `outils`, `plugins`, `internet`, `context` (clic droit), `toolbar`, `?` (aide). Vide : le script n'apparaît dans aucun menu |
| `Index` | -1 | Position dans le menu |
| `Shortcut` | — | Raccourci clavier (`Ctrl+B`, `Maj+Ctrl+C`, `F9`…) |
| `NeedsFile` | **1** | Le script exige un fichier ouvert. S'il vaut 1, l'hôte crée aussi un point d'historique (annulation) avant de lancer le script (`AddHistory`) |
| `ReadOnly` | 0 | Lu et stocké dans `TScript.ReadOnly`, mais **jamais exploité pour les scripts** (seul `plugin.ini` des plugins DLL s'en sert pour éviter un rafraîchissement de la grille). Pour le portage, la clé est purement indicative |
| `DropDown` | 0 | Bouton de barre d'outils avec menu déroulant, que le script remplit lui-même (protocole `populate_popup_menu`, voir Filters) |
| `ExecuteWithFile` | 0 | Le script tourne automatiquement à l'ouverture d'un fichier |
| `ExecuteWithNewLine` | 0 | Le script tourne automatiquement après l'ajout de lignes (liste fournie par `get_lines_changed`) |
| `ExecuteWithModifiedLine` | 0 | Idem, après la modification de lignes |
| `DisableScripts` | — | Noms d'autres scripts à désactiver (BP ≥ 7.4). Aucun script livré ne l'utilise |

### 0.2 Environnement d'exécution vu par un script

- **Module `BP`** (`import BP`) : propriétés indexées et fonctions de l'objet `IBankPerfectPlugin`.
- **Fonctions injectées dans les globales** (sans `import`) : `CreateComponent`, `BPEval`, `Rect`, `CreateChart`, `DrawChart`, `DestroyChart`, `ExportChart`, ainsi que les constantes `MB_YESNO`, `MB_ICONINFORMATION`, `MB_ICONQUESTION`, `IDYES`.
- **`BPEval(cmd)` et `BP.GetURL(cmd)` sont strictement équivalents** : `BPEvalExecute` appelle `BP.GetURL(Cmd)` (`FormPython.pas`), et la doc des plugins le confirme (« raccourci de la méthode BP.GetURL() »). Les scripts emploient l'un ou l'autre indifféremment.
- **IHM** : les fenêtres sont construites dynamiquement avec des composants VCL (`TForm`, `TButton`, `TDrawGrid`…), via `CreateComponent("TClasse", parent)` et `obj.SetProps(Prop=valeur, …)`. Les gestionnaires d'événements sont des fonctions Python (`OnClick=fonction`).
- **Persistance « dans le fichier .bp »** : `BPEval("save_script_data:<clé>:<chaîne>")` et `BPEval("load_script_data:<clé>")`. L'hôte (`ObjectScriptsData.pas`) les range sous la forme `bp_script_data_<clé>=<valeur échappée _xx>` dans le fichier `.bp`. Les valeurs sont des **pickles Python 2 au protocole 0** (texte), produits par `cPickle.dumps(...)`. Seule exception : `fabiobpmailaddress`, qui est une chaîne brute.

---

## 1. Tableau récapitulatif

Légende de la colonne ReadOnly : la valeur de la clé `.ini`, ou « — » si elle est absente (défaut 0). Toutes les clés de persistance commencent par `fabiobp` et sont stockées dans le `.bp` sous `bp_script_data_<clé>`.

| Dossier | Nom (`Name`) | Menu (raccourci / déclencheur) | Fichier `.py` | Lignes | Rôle en une phrase | ReadOnly | Données persistantes | Fichiers annexes |
|---|---|---|---|---|---|---|---|---|
| AllAccountsView | relevé multi-comptes | comptes | all_accounts_view.py | 325 | Relevé consolidé des lignes filtrées de plusieurs comptes, avec totaux | — | `fabiobpviewall` | — |
| Auto Fields | Calcul automatique des champs | outils (Ctrl+F) ; auto `ExecuteWithNewLine`/`ExecuteWithModifiedLine` | auto_fields.py | 594 | Règles « si Tiers/Détails contient X alors appliquer mode/tiers/détails/catégorie/date » | — | `fabiobpautofields` | ancien `Scripts\Auto Fields\<fichier>.dat` (pickle, lu en secours) ; import/export `.txt` / `.bprules` |
| BackupMail | Sauvegarder par e-mail | internet (F9) | backup_mail.py | 168 | Envoie le `.bp` en pièce jointe, avec un résumé HTML des soldes | — | `fabiobpmailaddress` | — |
| Backups | Liste des sauvegardes | comptes ; NeedsFile=0 | backups.py | 117 | Liste et restaure les sauvegardes locales `.bp0` ou distantes (FTP) | — | — | lit le dossier `bp_backup_folder` |
| Budget | Budget | outils, toolbar (Ctrl+B) | budget.py | 664 | Budget mensuel par catégorie : prévu, réel, écart | — | `fabiobpbudget` (L/É), `fabiobpautofilters` (lecture) | `budget.slk` |
| BudgetYear | Budget annuel | outils | budget_year.py | 718 | Tableaux de budget annuels (12 mois × budget/réel/écart) | — | `fabiobpbudgetyear` | — |
| Calc | Calculatrice | outils (Maj+Ctrl+C) ; NeedsFile=0 | calc.py | 73 | Calculatrice scientifique (trigonométrie, degrés-minutes-secondes) | — | — | — |
| CategsTotals | Totaux par catégorie | outils, toolbar (Ctrl+T) | categs_totals.py | 369 | Tableau croisé catégories × mois/années, avec export tableur | — | — | `categs_totals.slk` |
| Charts | Graphiques | outils, toolbar | charts.py | 323 | Graphiques (soldes, catégories, mois, modes, tranches, jours du mois), export PDF | — | — | — |
| Currencies | Conversions de devises | outils (Ctrl+D) ; NeedsFile=0 | currencies.py | 267 | Convertisseur multidevise ; cours téléchargés sur xe.com | — | `bankperfect.ini` `[CurrenciesData]` (`symbols`, `values`) | — |
| Export CSV | Exporter vers un fichier CSV | exporter | export_csv.py | 185 | Export CSV paramétrable (champs, ordre, séparateur, symboles de pointage) | — | — | — |
| Export HTML | Export html | exporter | export_html.py | 127 | Export HTML du relevé visible, avec synthèse des soldes | — | — | — |
| Export QIF | Exporter vers un fichier QIF | exporter | export_qif.py | 56 | Export QIF `!type:Bank` des lignes visibles | — | — | — |
| Export XLS | Export tableur | exporter | export_xls.py | 73 | Export SYLK (nommé `.xls`) des lignes visibles | — | — | `export_xls.slk` |
| Export XML | Export xml | exporter | export_xml.py | 44 | Export XML de tous les comptes et de toutes les opérations | — | — | — |
| ExtraValues | Champs supplém. de la ligne | operations, context (F3) | extra_values_edit.py | 173 | Ajout, modification et suppression des champs supplémentaires d'une ligne | — | (dans le `.bp`, via `extra_set`) | — |
| Filters | Filtres automatiques | affichage, toolbar (F7) ; DropDown=1 | setfilters.py | 227 | Filtres prédéfinis à dates dynamiques (`1/m/a`, `dj/m-1/a`…) | — | `fabiobpautofilters` | `setfilters.dat` (ancien, **plus lu**) |
| Import CSV | Importer un fichier CSV | importer | import_csv.py | 661 | Import CSV/TSV avec correspondance des colonnes, débit/crédit, doublons | — | — | — |
| Import OFX | Importer un fichier OFX | importer | import_ofx.py | 568 | Import OFX multicompte, avec correspondance des comptes et gestion des doublons | — | — | `mapping.ini` |
| Import QIF | Importer un fichier qif | importer (Ctrl+Alt+Q) | import_qif.py | 363 | Import QIF avec correspondance des lettres préfixes et doublons | 0 | — | — |
| Loan | Simulateur de crédits | outils (Ctrl+L) ; NeedsFile=0 | loan.py | 704 | Calcul de crédit (capital, taux, durée, mensualité), tableau d'amortissement, graphique | — | `loan.dat` (texte, dans `appdata_dir`) | `loan.slk` |
| Luhn | Vérifier une carte bancaire | outils ; NeedsFile=0 | luhn.py | 30 | Contrôle de la clé de Luhn d'un numéro de carte | — | — | — |
| ManagePlugins | Installer/gérer les plugins | plugins ; NeedsFile=0 | manage_plugins.py | 198 | Catalogue en ligne, installation (setup `.exe`) et suppression de plugins | — | — | — |
| Manuel | Télécharger le manuel | ? (Ctrl+F1) ; NeedsFile=0 | manuel.py | 75 | Aspire le wiki chelly.net et l'assemble en un seul fichier HTML | — | — | — |
| ModifyGroup | Modifier le groupe d'opérations | operations, context (Ctrl+M) | modifygroup.py | 141 | Modifie en masse les lignes sélectionnées | — | — | — |
| MultipleInput | Saisie multiple | operations (F2) | multiple_input.py | 474 | Saisie rapide de plusieurs lignes en texte libre, avec abréviations | 0 | `fabiobpabbreviations` | — |
| Notes | Notes | outils (Ctrl+Maj+N) ; NeedsFile=0 | notes.py | 89 | Bloc-notes global (titre, texte, case « afficher à l'ouverture ») | — | `Scripts\Notes\notes.dat` (pickle) | `notes.py.bak` |
| Notes | Notes | aucun ; `ExecuteWithFile=1` | notes_auto.py | 29 | Affiche les notes marquées à l'ouverture d'un fichier | — | `notes.dat` (lecture) | — |
| Numerals | Conversions numériques | outils ; NeedsFile=0 | numerals.py | 125 | Conversions chiffres ↔ chiffres romains → toutes lettres | — | — | — |
| PeopleTotals | Totaux par tiers | outils (Ctrl+Alt+T) | people_totals.py | 337 | Tableau croisé tiers × mois/années, avec export tableur | — | — | `people_totals.slk` |
| Receipt | Reçu d'une liste de remboursements | outils | receipt.py | 192 | Génère un reçu PDF des remboursements d'un prêt entre particuliers | — | `Scripts\Receipt\receipt.dat` (pickle) | `recu.pdf` (sortie générée) |
| Replace | Remplacer | edition (Ctrl+R) | search_replace.py | 50 | Rechercher/remplacer dans Tiers et Détails des lignes visibles | — | — | — |
| Rib | Imprimer un rib | fichier | rib.py | 75 | Génère un PDF de 3 RIB (calcule la clé RIB) | — | — | — |
| Scheduler | Echéancier | comptes, toolbar (Ctrl+E) | scheduler.py | 840 | Gestion des échéances (ajout, modification, application, décalage, import/export) | 0 | `fabiobpscheduler` | `scheduler.dat` (ancien, pickle) ; export `.bpsched` |
| Scheduler | Ajouter à l'échéancier | operations, context | scheduler_add.py | 114 | Crée une échéance mensuelle à partir de la ligne sélectionnée | — | `fabiobpscheduler` | `scheduler.dat` (secours) |
| Scheduler | Insertion des échéances | aucun ; `ExecuteWithFile=1` | scheduler_auto.py | 195 | Insère automatiquement les échéances arrivées à terme, à l'ouverture | — | `fabiobpscheduler` | `scheduler.dat` (secours) |
| Scheduler | Solde prévisionnel | aucun ; lancé par le panneau « Prévisions » avec un paramètre date | previsions.py | 252 | Tableau du solde prévisionnel (échéances + lignes à venir), export HTML | — | `fabiobpscheduler` (lecture) | `scheduler.dat` (secours) |
| test | Test | internet ; NeedsFile=0 | test.py | 59 | Script de démonstration (son, image, graphique commenté) | — | — | `test.png` |
| Totals | Total des comptes | comptes | totals.py | 159 | Soldes pointé / à venir / final de chaque compte, avec export tableur | 0 | — | `totals.slk` |
| Transfer | Convertir la ligne en transfert | operations, context | convert_line_to_transfer.py | 117 | Transforme une ligne existante en transfert lié entre deux comptes | 0 | — | — |
| Transfer | Transfert de compte à compte | operations, toolbar (Ctrl+Maj+T) | transfer.py | 291 | Saisit un transfert : deux lignes liées dans deux comptes | 0 | `fabiobptransfer` | — |
| UpdateStock | Télécharger les cotations | internet | update_stock.py | 43 | Met à jour la valeur des lignes « Titre » (code ISIN) depuis MSN Money | — | — | — |
| Ventilate | Ventiler l'opération | operations, context (F6) | ventilate.py | 114 | Remplace une opération par N lignes réparties sur plusieurs catégories | — | — | — |

**Clés de persistance partagées entre plusieurs scripts**

- `fabiobpscheduler` : liste de dicts d'échéances, utilisée par scheduler, scheduler_add, scheduler_auto et previsions.
- `fabiobpautofilters` : dict des filtres prédéfinis, écrit par setfilters et lu par budget.

---

## 2. Fiches détaillées par script

Pour chaque fiche, la rubrique « Pièges Py2→3 » ne liste que les points propres au script. Les pièges communs à tous (encodage des sources, `cPickle`, divisions dans les calculs de mise en page, etc.) sont détaillés en section 4.

Conventions de l'API observées :

- `BP.OperationXxx[compte][ligne]` est lisible et inscriptible.
- `BP.OperationXxx[compte]` renvoie un **proxy de liste** : itérable, avec `len`, et `proxy[i] = v` écrit dans le document.
- Les dates sont lues au format `JJ-MM-AAAA`. `LineAdd` accepte `JJ/MM/AAAA` (et `JJ-MM-AAAA`).
- `BP.CategName[i]` vaut `"id=nom"`, avec un espace en tête du nom pour une sous-catégorie. `BP.CategParent[i]` est la **position** du parent (égale à `i` pour une catégorie principale).

### 2.1 Imports

#### Import QIF — `Import QIF/import_qif.py` (363 l.)

- **Ce que fait l'utilisateur**
  1. Choisit un `.qif` (boîte `OpenDialog`).
  2. Le script ne garde que les lignes qui commencent par une majuscule ou par `^`, et détecte les lettres préfixes. S'il en trouve moins de 3, il affiche « fichier QIF incorrect ».
  3. Fenêtre « Paramètres de l'import » :
     - ordre de la date (JMA, MJA ou AMJ) ;
     - doublons (Tout accepter, Tout refuser ou Demander) ;
     - aperçu des 50 premières lignes ;
     - une liste déroulante par lettre préfixe (correspondance par défaut `D`→Date, `N`→Mode, `P`→Tiers, `M`→Détails, `L`→Catégorie, `T`→Montant) ;
     - case « Créer les catégories manquantes ».
  4. Le découpage se fait aux `^`. Le mode est deviné à partir du texte et du signe (`chq`, `vir`, `cb`, `prl`…). Les numéros de chèque deviennent « Chèque émis » + `n°XXX` dans les détails.
  5. La catégorie est retrouvée par motif normalisé (minuscules, sans accent ni ponctuation), sinon créée.
  6. Doublons : même (date, mode simplifié, montant) qu'une ligne existante. La fenêtre propose Oui, Non, Toutes, Aucune.
  7. Un rapport final classe les lignes : importées, dates incorrectes, montants incorrects, refusées, doublons.
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `CategName`, `CategAdd(nom, 0)`, `CategCount`, `LineAdd(acc, date, mode, tiers, détails, categ, montant, 0)`, `MsgBox`, `OpenDialog(titre, "", ".qif", "Quicken Interchange Format (*.QIF)|*.qif")`, `OperationAmount`, `OperationCount` (comparé avant/après `LineAdd` pour détecter un refus), `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`. Composants : `TForm`, `TLabel`, `TComboBox`, `TMemo`, `TCheckBox`, `TButton` (ModalResult 6, 7, 9, 10).
- **Formats** : lecture QIF. Seuls les enregistrements simples sont gérés ; les lignes `!Type:` sont ignorées et les ventilations QIF (`S`, `$`, `E`) ne sont pas prises en charge.
- **Dépendances** : aucune hors `BP`.
- **Pièges Py2→3**
  - `v.translate(PatternTable, " -.,;:/!?'_=")` : `translate` à 2 arguments avec une table de 256 caractères. À remplacer par `str.maketrans(src, dst, delete)`.
  - `r.has_key("Date")`.
  - `r = d.keys(); r.sort()` ; `for m in fmodes.keys()` (passe).
  - `open(QIFFile).readlines()` : préciser l'encodage (les QIF bancaires français sont généralement en CP1252).
  - `TestDup` utilise la variable globale `line` (fonctionne, mais fragile).
- **Effort** : moyen.

#### Import CSV — `Import CSV/import_csv.py` (661 l.)

- **Ce que fait l'utilisateur**
  1. Choisit un `.csv` ou un `.tsv` (le `.tsv` « Vidéoposte » a des réglages prédéfinis : séparateur TAB, première ligne 10).
  2. Fenêtre « Paramètres de l'import » :
     - format de date et séparateur (`;`, `,`, `TAB`) ;
     - doublons ;
     - première et dernière ligne ;
     - « Créer une colonne si retour à la ligne » (découpe une cellule multiligne) ;
     - « Créer les catégories manquantes » ;
     - pointage : Non pointé, Pointé, Rapproché, ou « Valeur importée » (`P`/`R`, et `O`→P, `N`→R) ;
     - aperçu en grille (50, 100, 500 lignes ou tout).
  3. Correspondance des 10 champs (Date, Mode, Tiers, Détails, Catégorie, Sous-catégorie, Montant, Débit, Crédit, Pointage) avec les colonnes. Elle est devinée à partir des titres de la ligne d'en-tête (`guess`). Il faut soit Montant, soit Débit et Crédit.
  4. Les dates acceptent les noms de mois en français et en anglais, et les années sur 2 chiffres. Le mode est deviné ; les numéros de chèque sont extraits vers les détails.
  5. Les doublons sont traités comme pour le QIF, puis un rapport s'affiche.
  - Le script contient son propre analyseur CSV (classe `SCSV`, qui gère les guillemets et les champs multilignes).
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `CategAdd`, `CategCount`, `CategName`, `CategParent`, `LineAdd(..., mark)`, `OpenDialog(..., ".csv", "Comma Separated Values (*.CSV)|*.csv|Vidéoposte (*.TSV)|*.tsv")`, `OperationAmount`, `OperationCount`, `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`. Composants : `TDrawGrid` (`OnDrawCell`), `TComboBox`, `TEdit` (`OnExit`), `TCheckBox`, `TMemo`, `TForm`.
- **Formats** : lecture CSV/TSV.
- **Pièges Py2→3**
  - `raise SCSVError, "..."` (2 occurrences) : **erreur de syntaxe** en Py3.
  - `file.xreadlines()` et `self.lineiterator.next()`.
  - `translate` à 2 arguments.
  - `Century = CurrentYear / 100` devient un flottant, donc des années flottantes.
  - `has_key`, `cols = cols.keys(); cols.sort()`.
  - Encodage du fichier lu.
  - `close()` référence `self.file`, qui vaut toujours `None` (code mort).
  - Conseil : remplacer `SCSV` par le module `csv` de la bibliothèque standard.
- **Bogue existant** : dans `GetCtg`, une branche renvoie la position `i` au lieu de `CIndexes[i]`, et contient l'expression `c in t > -1`.
- **Effort** : élevé (gros script au code ancien ; une réécriture partielle est conseillée).

#### Import OFX — `Import OFX/import_ofx.py` (568 l.) + `mapping.ini`

- **Ce que fait l'utilisateur**
  1. Clique sur « Parcourir… » et choisit un `.ofx`.
  2. Le fichier est analysé de façon naïve, balise par balise (SGML OFX 1.x) :
     - `<STMTTRNRS>`, ou `<CCSTMTTRNRS>` renommé en `<STMTTRNRS>`, délimite un compte ;
     - `<BANKID>`, `<BRANCHID>`, `<ACCTID>` et `<TRNUID>` identifient ce compte ;
     - chaque `<STMTTRN>` fournit `<DTPOSTED>`, `<TRNTYPE>`, `<NAME>`, `<MEMO>`, `<CHECKNUM>` et `<TRNAMT>`.
  3. Un aperçu en grille s'affiche. Sur « Importer », une fenêtre « Correspondance des comptes » associe chaque compte OFX à un compte BP. Elle est présélectionnée par comparaison avec `n° banque-guichet-compte` ; l'option « Ne pas insérer » existe.
  4. Doublons : même clé (compte, date, mode, montant à 2 décimales). Une grille permet de cocher ou décocher chaque doublon (clic ou ESPACE, « Tout cocher »).
  5. Les lignes sont insérées sans catégorie, et les erreurs sont listées.
  - Le mode est déduit de `mapping.ini` (`TRNTYPE=mode débit,mode crédit`, par exemple `CHECK=Chèque émis,Dépôt de chèque`), corrigé par des heuristiques sur le tiers (`CHQ`, `DAB`, `CB`, `PRLV`, `VIR`…).
- **API BP** : `AccountBranchNumber` (format `"banque guichet"`), `AccountCurrent`, `AccountName`, `AccountNumber`, `AccountRefreshScreen`, `BankPerfectExePath` (pour trouver `Scripts\Import OFX\mapping.ini`), `LineAdd` (retour 0 = échec), `MsgBox`, `OpenDialog`, `OperationAmount`, `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`. Composants : `TDrawGrid` (`OnMouseUp`, `OnKeyUp` avec `Key.Value`), `TBevel`, `TLabel` (`OnClick`, `Cursor=-21`), `Rect(...)`.
- **Formats** : lecture OFX 1.x (SGML) ; l'OFX 2.x (XML) fonctionne par chance. Lecture de `mapping.ini` (format `clé=a,b`, sans section).
- **Pièges Py2→3**
  - `accounts = {}.fromkeys(...).keys(); accounts.sort()` et `l = d.keys(); l.sort()`.
  - Encodage : un OFX déclare `CHARSET:1252` ou UTF-8, il faut décoder en conséquence.
  - Divisions dans la mise en page.
  - Chemin construit avec `"Scripts\\Import OFX\\mapping.ini"`.
- **Bogue existant** : `elif cnt_lines == 1: "Fichier à importer (une ligne):"` (il manque l'affectation à `l1.Caption`).
- **Effort** : moyen. Conseil : migrer vers un vrai analyseur OFX, ou garder l'heuristique mais la tester.

### 2.2 Exports

Les exports fonctionnent tous sur `BPEval("bp_export_folder")` (le dossier d'export, avec un séparateur final). Après l'écriture, ils proposent d'ouvrir le fichier via `BP.ShellExecute("open", chemin, "", 1)` (`MsgBox(..., MB_YESNO + MB_ICONINFORMATION) == 6`).

#### Export CSV — `Export CSV/export_csv.py` (185 l.)

- **Ce que fait l'utilisateur** : fenêtre « Paramètres de l'export ».
  - Il coche les champs (Date, Mode, Tiers, Détails, Catégorie, Sous-catégorie, Montant, Pointage) et choisit leur ordre (1 à 8).
  - Il choisit le séparateur (`;`, `,`, TAB).
  - Options :
    - exporter tous les comptes ;
    - ligne d'en-tête ;
    - remplacer « Chq XXX » par « Chèque émis » ;
    - grouper catégorie et sous-catégorie avec un séparateur (« > » par défaut) ;
    - symboles de pointage (`-`, `P`, `R`).
  - Il obtient le fichier `<export>/<fichier .bp sans extension> - <compte>.csv`.
  - Seules les lignes visibles sont exportées, sauf avec « tous les comptes ». Il n'y a pas de guillemets : le séparateur est remplacé par un espace dans le tiers et les détails.
- **API BP** : `AccountCount`, `AccountCurrent`, `AccountName`, `BankPerfectFileName`, `CategName`, `CategParent`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails`, `OperationMark`, `OperationMode`, `Operationthirdparty`, `VisibleLines`. Commande `BPEval("bp_export_folder")`.
- **Formats** : écriture CSV.
- **Pièges Py2→3**
  - `Ctg.has_key(index)`.
  - `str(v).replace(".", ",")` : en Py2.3, `str(float)` donne 12 chiffres significatifs ; en Py3 on obtient le `repr` le plus court, d'où des différences possibles (`0.30000000000000004`). Utiliser `"%.2f"`.
  - Encodage du fichier de sortie (CP1252 attendu par Excel FR, ou UTF-8 avec BOM).
  - La variable `all` masque la fonction native (sans conséquence).
- **Effort** : faible.

#### Export HTML — `Export HTML/export_html.py` (127 l.)

- **Ce que fait l'utilisateur** : le script crée `export.html`, avec :
  - le titre « Compte "nom" (n° …) » ;
  - un encadré de synthèse : nombre de lignes, opérations pointées, rapprochées, à venir, solde (totaux par valeur de pointage) ;
  - un tableau à lignes alternées : Date, Mode, Tiers, Détails, Catégorie, Sous-catégorie, Montant (séparateur de milliers `&nbsp;`, virgule décimale), Pointage.
- **API BP** : `AccountBranchNumber`, `AccountCurrent`, `AccountName`, `AccountNumber`, `CategName`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails`, `OperationMark`, `OperationMode`, `Operationthirdparty`, `ShellExecute`, `VisibleLines`. Commande `bp_export_folder`.
- **Formats** : HTML 4.01 avec `<meta charset=iso-8859-1>` ; le `<title>` vaut « chelly.net ».
- **Pièges Py2→3** : écrire avec `encoding="cp1252"`, ou passer la balise meta en UTF-8 (sinon les accents seront faux) ; aucun échappement HTML des champs (`<`, `&`).
- **Effort** : faible.

#### Export QIF — `Export QIF/export_qif.py` (56 l.)

- **Ce que fait l'utilisateur** : le script crée `<export>/<fichier> - <compte>.qif`.
  - En-tête `!type:Bank`.
  - Pour chaque ligne visible : `D` (date `JJ/MM/AAAA`), `N` (mode), `P` (tiers), `M` (détails), `L` (« Parent:Sous-catégorie »), `T%.2f`, `^`.
- **API BP** : `AccountCurrent`, `AccountName`, `BankPerfectFileName`, `CategName`, `CategParent`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`, `VisibleLines`. Commande `bp_export_folder`.
- **Formats** : écriture QIF.
- **Pièges Py2→3** :
  - `Ctg.has_key` ;
  - `zip()` (utilisé une seule fois, passe) ;
  - encodage de sortie ;
  - test `i in visible_lines` sur une liste (complexité O(n²)).
- **Effort** : faible.

#### Export XLS (« Export tableur ») — `Export XLS/export_xls.py` (73 l.) + `export_xls.slk`

- **Ce que fait l'utilisateur** : le script crée `bankperfect.xls`, qui est en réalité un fichier **SYLK**. Il est construit à partir du modèle `export_xls.slk` (formats de nombre, largeurs de colonnes, polices) en remplaçant `{count}`, `{count-1}` et `{lines}`.
  - Colonnes : date en numéro de série Delphi (calculé par `to_delphi_date`), mode, tiers, détails, catégorie, sous-catégorie, montant `%.4f`, pointage (P ou R).
- **API BP** : `AccountCurrent`, `BankPerfectPluginPath()` (renvoie `"…|<chemin du .py courant>"`, sert à retrouver le `.slk` voisin), `CategName`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails`, `OperationMark`, `OperationMode`, `Operationthirdparty`, `ShellExecute`, `VisibleLines`. Commande `bp_export_folder`.
- **Formats** : écriture SYLK avec l'extension `.xls`. Excel récent affiche un avertissement « format et extension ne correspondent pas ».
- **Pièges Py2→3** :
  - **`to_delphi_date` : `i / 4 - i / 100 + i / 400` doit devenir `//`**, sinon les dates sont fausses ;
  - écrire le SYLK en CP1252.
- **Bogue existant** : `str(len(lines)).replace("{count-1}", ...)` (parenthèse mal placée), donc `{count-1}` n'est jamais remplacé.
- **Effort** : moyen. Conseil : produire un vrai `.xlsx` (voir section 5).

#### Export XML — `Export XML/export_xml.py` (44 l.)

- **Ce que fait l'utilisateur** : le script crée `export.xml` avec la structure `<comptes><compte …><operation …/></compte></comptes>`, pour tous les comptes et toutes les lignes (pas seulement les visibles).
  - Attributs du compte : nom, num, adresse_agence, nom_agence, num_agence, nom_banque, num_banque, bic, monnaie1/2, taux1/2, iban, virtuel.
  - Attributs de l'opération : date, mode, tiers, info, categ (« Parent:Sous »), montant, pointage.
- **API BP** : `AccountBIC`, `AccountBankName`, `AccountBranchAddress`, `AccountBranchName`, `AccountBranchNumber`, `AccountCount`, `AccountCurrency1`, `AccountCurrency2` (format `"symbole|taux"`), `AccountIBAN`, `AccountIsVirtual`, `AccountName`, `AccountNumber`, `CategName`, `CategParent`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationCount`, `OperationDate`, `OperationDetails`, `OperationMark`, `OperationMode`, `Operationthirdparty`, `ShellExecute`. Commande `bp_export_folder`.
- **Formats** : XML déclaré en UTF-8. La fonction `enc()` transforme les caractères non alphanumériques en entités `&#n;`.
- **Pièges Py2→3** :
  - En Py2, `ord(c)` portait sur l'octet CP1252, ce qui donnait des entités fausses pour 0x80–0x9F (sauf `€`, traité à part).
  - En Py3, `ord` donne le point de code Unicode (correct), mais `isalnum()` devient vrai pour les lettres accentuées, qui sont alors écrites telles quelles : il faut écrire le fichier en UTF-8.
  - Conseil : utiliser `xml.sax.saxutils.quoteattr`.
- **Effort** : faible.

### 2.3 Rapports et analyses

#### Total des comptes — `Totals/totals.py` (159 l.) + `totals.slk`

- **Ce que fait l'utilisateur** : une grille (`TDrawGrid` dessinée à la main) liste les comptes non virtuels, dans l'ordre d'affichage. Colonnes : Compte, Pointé (somme des lignes dont le pointage est > 0), A venir, Solde, plus une ligne Total ; les montants négatifs sont en rouge. Le bouton « Export tableur » crée `totals.xls` (SYLK, à partir de `{nb_lines}`, `{nb_lines+1}` et `{lines}`).
- **API BP** : `AccountIsVirtual`, `BankPerfectPluginPath`, `MsgBox`, `OperationAmount`, `OperationMark`, `ShellExecute`. Commandes `get_accounts`, `bp_export_folder`. Canevas : `Brush.Color`, `Font.Color`, `Font.Style`, `FillRect`, `TextRect`, `TextWidth`.
- **Pièges Py2→3** : divisions entières dans `SetColumnsWidth` et `ResizeForm`, qui produisent des largeurs flottantes ; propriété `onResize` en minuscules (l'hôte doit rester insensible à la casse).
- **Effort** : faible.

#### Totaux par catégorie — `CategsTotals/categs_totals.py` (369 l.) + `categs_totals.slk`

- **Ce que fait l'utilisateur** : une grille croisée.
  - Listes déroulantes : année (« Toutes années » ou une année), compte (« Tous les comptes », qui exclut les comptes virtuels, ou un compte), objectif (Totaux mensuels, Totaux annuels, Moyennes annuelles).
  - Cases à cocher : « Toutes les catégories » (sinon les sous-catégories sont agrégées dans leur parent) et « Totaux ligne à ligne » (débit et crédit calculés sur les opérations plutôt que sur les cellules).
  - Contenu : les lignes sont les catégories, les colonnes les mois ou les années, puis Débit, Crédit, Total, et une colonne Total finale. Une colonne « Solde reporté » apparaît si une année est choisie. La ligne « Sans catégorie » est supprimée quand elle est vide.
  - Le bouton « Export tableur » produit `categs_totals.xls` (SYLK).
- **API BP** : `AccountCurrent`, `AccountIsVirtual` (itéré), `AccountName` (`len()`), `BankPerfectPluginPath`, `CategName`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `ShellExecute`. Commandes `get_accounts`, `bp_export_folder`. Également `Rect(...)`.
- **Pièges Py2→3** : `data += zip(...)` (passe) ; divisions dans la mise en page ; `grid.Repaint` sans parenthèses (sans effet, déjà le cas en Py2).
- **Bogue existant** : `min(years)` échoue (ValueError) si un compte n'a aucune opération.
- **Effort** : moyen.

#### Totaux par tiers — `PeopleTotals/people_totals.py` (337 l.) + `people_totals.slk`

- **Ce que fait l'utilisateur** : même principe que les totaux par catégorie, mais par **tiers**.
  - Le tiers est normalisé : minuscules, sans accents. La case « Supprimer dates et nombres » retire les mots numériques ou entre parenthèses.
  - Listes déroulantes : années, comptes, objectif (mensuel, annuel, moyenne).
  - Le bouton « Export tableur » produit `people_totals.xls`.
- **API BP** : `AccountCurrent`, `AccountName`, `BankPerfectPluginPath`, `MsgBox`, `OperationAmount`, `OperationDate`, `Operationthirdparty`, `ShellExecute`. Commandes `get_accounts`, `bp_export_folder`.
- **Pièges Py2→3**
  - `s.translate(no_accents_table)` : table de 256 caractères, à remplacer par `str.maketrans` (ou `unicodedata`).
  - `global_people_names = global_peoples.keys(); .sort()`.
  - `SetProps(Checked="True")` passe une chaîne : l'hôte doit convertir.
- **Effort** : moyen.

#### Graphiques — `Charts/charts.py` (323 l.)

- **Ce que fait l'utilisateur** : fenêtre « Graphiques ».
  - Liste « Données » :
    - soldes des comptes ;
    - soldes détaillés (pointées, à venir, solde) ;
    - catégories (principales ou toutes) ;
    - répartition mensuelle ;
    - modes de paiement ;
    - niveaux des opérations (tranches 0–5–10–…–10000) ;
    - répartition au cours du mois.
  - Liste « Compte » : tous ou un seul. Liste « Modèle » : Histogramme, Histo. + lignes, Histo. empilé, Points, Ligne, Aire, Secteurs. Cases « 3D » et « Valeurs nulles ».
  - Les calculs portent sur les lignes **filtrées** (`simulate_set_filter:` avec les filtres courants).
  - Le bouton « Imprimer » exporte en PDF `bankperfect_chart.pdf`.
- **API BP** : `AccountCurrent`, `AccountIsVirtual`, `CategName`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationGetNameFromModeIndex(i)`, `OperationMark`, `OperationMode`, `ShellExecute`.
- **Commandes** : `BPEval("get_accounts")`, `BPEval("simulate_set_filter:")` (renvoie `"l1;l2;…|l1;…|…"`, une liste par compte réel), `BPEval("mode_count")`, `BPEval("bp_export_folder")`.
- **Graphiques** : `CreateChart("ChartZone")` dans `OnActivate`, puis `DrawChart("ChartZone", Names, Values, ShowNullValues, Bevel, Graphtype, 0)`. Ce dernier prend **7 arguments**, alors que la doc en donne 6 ; les noms `"/s=Titre"` démarrent une nouvelle série. Ensuite `ExportChart("ChartZone", chemin_pdf)`, qui renvoie `None` en cas d'échec, et `DestroyChart("ChartZone")` dans `OnDestroy`. Le panneau `TPanel` porte `Name="ChartZone"`, `Color` (fond du graphique) et `ShowHint=1` (affiche la légende).
- **Pièges Py2→3** : divisions dans la mise en page. Le reste est simple.
- **Effort** : moyen, conditionné par le moteur de graphiques de l'hôte.

#### Relevé multicompte — `AllAccountsView/all_accounts_view.py` (325 l.)

- **Ce que fait l'utilisateur** : une grille montre les lignes visibles (filtre courant) des comptes sélectionnés.
  - Colonnes : Compte, Date, Mode, Tiers/Détails (sur 2 lignes), Catégorie/Sous-catégorie (2 lignes), Montant, Pointage (« X » coloré).
  - Les couleurs suivent le thème de l'utilisateur.
  - Trois totaux en tête : « Pointé », « A venir », « Solde final ».
  - Le bouton « Comptes » ouvre une liste de cases à cocher ; la sélection est mémorisée.
- **API BP** : `CategName`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails`, `OperationMark`, `OperationMode`, `Operationthirdparty`, `GetURL`.
- **Commandes** :
  - `get_accounts`, `simulate_set_filter:` ;
  - `load_script_data:fabiobpviewall`, `save_script_data:fabiobpviewall:<pickle>` (dict `{"accounts": [index réels]}`) ;
  - `color:%d` (pour i de 0 à 13, couleurs du thème) ;
  - `format_date:%s` (formatage de date selon les réglages) ;
  - `draw_mark:%d;%d;%d;%d;%d;%d`, uniquement dans un commentaire.
- **Pièges Py2→3** : `cPickle` (section 4.2) ; divisions dans la mise en page (`c.Height / 3`, `(f.Width - w) / 2`).
- **Bogues existants** :
  - les totaux sont inversés : les lignes avec `mark == 0` sont additionnées dans « Pointé » ;
  - `get_real_data()` est du code mort ;
  - l'en-tête compte 7 colonnes alors que les données en ont 8.
- **Effort** : moyen.

#### Solde prévisionnel — `Scheduler/previsions.py` (252 l.)

- **Déclencheur** : le script n'est dans aucun menu. Il est lancé par l'hôte (`StartPrevisions`) depuis le panneau « Prévisions » de la fenêtre principale (champ date et bouton), avec `get_script_parameters` qui vaut `"AAAA-MM-JJ"`.
- **Ce que fait l'utilisateur** : il voit une grille Date, Tiers, Montant, Solde, Progression (barre colorée proportionnelle au solde maximal).
  - Lignes : le solde actuel (lignes pointées ou datées jusqu'à aujourd'hui), puis les échéances futures du compte courant jusqu'à la date choisie, et les lignes non pointées à venir, dans l'ordre chronologique.
  - La case « Une seule ligne par mois » regroupe les lignes par mois.
  - Le bouton « Export HTML » produit `previsions.htm`.
- **API BP** : `AccountCount`, `AccountCurrent`, `AccountName`, `BankPerfectExePath`, `BankPerfectFileName`, `GetURL`, `MsgBox`, `OperationAmount`, `OperationDate`, `OperationMark`, `Operationthirdparty`, `ShellExecute`. Commandes `get_script_parameters`, `load_script_data:fabiobpscheduler` (avec repli sur l'ancien `Scripts\Scheduler\scheduler.dat`), `bp_export_folder`. `save_script_data` est défini mais jamais appelé.
- **Pièges Py2→3** : `K = d.keys(); K.sort()` ; `bar()` calcule `abs(p) / 10` et compare avec `== 100` (en Py3 le résultat est un flottant) ; pickle ; l'ancien format extrait le nom de fichier en coupant sur `"\\"` uniquement.
- **Effort** : moyen.

#### Reçu — `Receipt/receipt.py` (192 l.)

- **Ce que fait l'utilisateur** (cas d'usage décrit dans le manuel : prêt entre particuliers sur un compte dédié)
  1. Il saisit le nom du payeur, la ville, le nom, l'adresse et le sexe du bénéficiaire ; ces valeurs sont mémorisées.
  2. Le script parcourt les lignes visibles : les crédits forment la somme prêtée, les débits les remboursements. Le libellé de chaque remboursement est déduit du mode (virement, espèces, chèque n°…).
  3. Il génère `Scripts\Receipt\recu.pdf` : titre « Reçu », texte « Je soussigné(e)… », liste des versements avec montants en lettres (« … euros et … cents »), dates en toutes lettres, solde restant dû. Puis il l'ouvre.
- **API BP** : `AccountCurrent`, `BankPerfectExePath`, `GetURL("pdf:<code>")`, `OperationAmount`, `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`, `ShellExecute`, `VisibleLines`.
- **Mini-langage PDF utilisé** (interprété par l'hôte) :
  - `Create Reçu;<chemin>;2100;2970` (A4, en dixièmes de mm) ;
  - `PenColor`, `FillRect x1;y1;x2;y2;couleur`, `DrawRectangle` ;
  - `FontName TimesBold|Helvetica`, `FontSize` ;
  - `TextOut x;y;texte`, `TextRect x;y;largeur;texte` (avec retour à la ligne ; y=0 continue à la suite) ;
  - `CurrY`.
- **Formats** : `receipt.dat` (pickle protocole 0 d'un dict `payer`, `payee`, `city`, `address`, `sex`) ; PDF en sortie.
- **Pièges Py2→3**
  - `long(Value)`.
  - **`N = N / 10` dans `DecToText` doit devenir `//`** : sinon la boucle travaille sur des flottants et produit un texte faux.
  - Pickle lu en mode texte (`open(...,"r")`), à remplacer par `"rb"` et `encoding`.
  - `"\xA0€"` (passe en Py3).
  - Le script **écrit dans le dossier du programme** (`Scripts\Receipt`), ce qui est interdit sous Linux et dans `Program Files`.
  - La ligne `CBSex.ItemIndex = Text=dic.get(...)` est une affectation chaînée accidentelle.
- **Bogue existant** : les données sont sauvegardées même si l'utilisateur annule.
- **Effort** : moyen (dépend du moteur PDF de l'hôte).

#### Imprimer un RIB — `Rib/rib.py` (75 l.)

- **Ce que fait l'utilisateur** : une fenêtre demande le nom du titulaire et propose la clé RIB calculée (modulo 97, lettres converties en chiffres). Le script génère ensuite `bankperfect_rib.pdf` (3 RIB par page, logo de la banque, IBAN, BIC, domiciliation) et l'ouvre.
- **API BP** : `AccountBIC`, `AccountBankLogo` (chemin d'un PNG), `AccountBankName`, `AccountBranchAddress`, `AccountBranchName`, `AccountBranchNumber`, `AccountCurrent`, `AccountIBAN`, `AccountNumber`, `ShellExecute`.
- **Commandes** :
  - `bp_export_folder` ;
  - `convert_to_bitmap:<png>|<bmp>` (convertit le logo en BMP pour le moteur PDF) ;
  - `pdf:<code>`, avec `Create`, `FontSize`, `FontName Helvetica(-Bold)`, `TextOut`, `DrawLine x1;y1;x2;y2`, `DrawImage x;y;l;h;chemin`.
- **Pièges Py2→3** : `long()` devient `int()`. `convert_to_bitmap` devient inutile si le nouveau moteur PDF accepte le PNG.
- **Effort** : moyen (moteur PDF).

### 2.4 Budget

#### Budget — `Budget/budget.py` (664 l.) + `budget.slk`

- **Ce que fait l'utilisateur** : fenêtre « Budget mensuel ».
  - Listes déroulantes :
    - « Mois de référence » : mois en cours, « Tout AAAA (moyenne) », « Toutes opérations (moyenne) », ou chacun des **filtres prédéfinis** du script Filters ;
    - « Mode de calcul » : mois inclus dans la période, ou mois calculés en jours / 30,4375 ;
    - « Compte ».
  - Cases : « Lignes sans budget », « Lignes à zéro », « Sous-catégories ».
  - Grille : Catégorie, Type (Recette ou Dépense), Budget, Réel, Différence (devise), Différence (%), puis les totaux des dépenses, des recettes et le total général.
  - Un double-clic ou le bouton « Budget de la ligne » ouvre une saisie du montant mensuel (boutons radio Dépense ou Recette).
  - Le bouton « Export tableur » produit `budget.xls`.
  - Un libellé décrit la période et le nombre de mois pris en compte.
- **API BP** : `AccountCurrency1` (symbole pris avant `|`), `AccountCurrent`, `BankPerfectPluginPath`, `CategName`, `GetURL`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `ShellExecute`.
- **Commandes** :
  - `get_accounts` ;
  - `load_script_data:fabiobpautofilters` (lecture) ;
  - `load_script_data:fabiobpbudget` et `save_script_data:fabiobpbudget:<pickle>` (dict `{id_categ: montant signé}`) ;
  - `simulate_set_filter:%s;%s;%s;%s;%s;%s;%d` (date de début, date de fin, mode, tiers, catégorie, montant, pointage) ;
  - `bp_export_folder`.
- **Composants** : `TRadioButton`, `TDrawGrid` (`OnDblClick`), `TLabel` (`AutoSize`, `WordWrap`).
- **Pièges Py2→3**
  - `rule_names = rules.keys(); rule_names.sort()`.
  - **`to_delphi_date` (`i / 4`…) doit utiliser `//`** : sinon `diff_date` donne un nombre de jours fractionnaire.
  - `eval` dans `fdate` (dates dynamiques) et dans `str_to_float`.
  - Pickle (section 4.2).
- **Effort** : moyen à élevé.

#### Budget annuel — `BudgetYear/budget_year.py` (718 l.)

- **Ce que fait l'utilisateur**
  1. Fenêtre d'accueil : liste des tableaux de budget « nom (année) », avec les boutons Nouveau, Renommer, Afficher, Supprimer, Dupliquer.
  2. Fenêtre d'un tableau : grille de 40 colonnes (Titre, 12 mois × [Budget, Réel, Écart], puis Total ×3) et une ligne « Totaux ».
  3. Un double-clic ou ENTRÉE modifie une cellule. La valeur s'applique au mois sélectionné, à tous les mois, à un mois sur deux ou à un mois sur trois.
  4. Ajouter ou modifier une ligne : titre, et réalisé saisi à la main ou calculé automatiquement à partir d'une **multisélection de catégories**.
  5. Le bouton « Comptes » choisit les comptes pris en compte (mémorisé par tableau).
  - Données : liste de `{"name", "year", "accounts"?, "lines": [{"title", "autocalc_real", "categs", "estimated_values"[12], "real_values"[12]}]}`.
- **API BP** : `AccountCurrent`, `CategName`, `GetURL`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`. Commandes `get_accounts`, `load_script_data:fabiobpbudgetyear`, `save_script_data:fabiobpbudgetyear:<pickle>`. Propriétés hôte particulières : `TListBox.Selected` (lecture et écriture d'une **liste d'index sélectionnés**, extension propre au wrapper), `Sender.ClassName`, `OnKeyPress` avec `Key.Value`.
- **Pièges Py2→3**
  - **`month = (ACol - 1) / 3` sert d'index de liste** : en Py3 cela déclenche une TypeError. Il faut `//`.
  - `deepcopy` maison via `cPickle`, à remplacer par `copy.deepcopy`.
  - `edit2.Text = t["year"]` affecte un entier à `Text` (l'hôte doit convertir).
  - Divisions dans la mise en page.
  - `sort_data` trie des tuples `(nom, dict)` : TypeError si deux noms sont égaux (les noms sont uniques en principe).
- **Effort** : élevé.

### 2.5 Échéancier

Structure d'une échéance (dict stocké dans `fabiobpscheduler`) :

| Clé | Contenu |
|---|---|
| `nextdate`, `enddate` | Tuples `(a, m, j)` ; `enddate` vaut `(0,0,0)` s'il n'y a pas de fin |
| `mode` | Index 0–8 dans `["Carte", "Retrait DAB", "Chèque émis", "Prélèvement", "Virement émis", "TIP", "Versement", "Dépôt de chèque", "Virement reçu"]` |
| `tiers`, `details` | Textes |
| `categ` | Id de catégorie |
| `montant` | Montant signé |
| `autoinsert` | 0 ou 1 |
| `account` | Index réel du compte |
| `transfer` | Index réel du compte lié, ou -1 |
| `delta`, `deltaunit` | Intervalle ; unité 0 = jours, 1 = mois, 2 = ans |
| `Postpone` | Combinaison de « S », « D » et « F » (samedi, dimanche, férié) |

#### Echéancier — `Scheduler/scheduler.py` (840 l.)

- **Ce que fait l'utilisateur**
  1. Grille Compte, Intervalle, Tiers, Montant, Prochaine échéance, triée par date ; la mention « Dernière échéance atteinte » apparaît en rouge.
  2. Boutons Ajouter, Modifier, Supprimer, « « en arrière » et « en avant » » (décale la prochaine date d'une période).
  3. « Appliquer… » ouvre une fenêtre : date limite ; lignes concernées (la ligne sélectionnée, les échéances non automatiques, ou toutes) ; case « simulation », qui insère les lignes sans avancer les dates.
  4. Les liens « Importer » et « Exporter » lisent et écrivent un fichier `.bpsched`. Si le fichier existe, une sauvegarde `.bak` est faite.
  5. Fenêtre d'édition : prochaine échéance, dernière échéance (optionnelle), mode et catégorie, tiers et détails, montant (son signe est imposé par le mode), intervalle et unité, compte, transfert vers un autre compte, insertion automatique à l'ouverture, report si le jour tombe un samedi, un dimanche ou un férié (fériés français, avec calcul de Pâques).
  - Une échéance de transfert insère deux lignes, « Transfert vers/depuis le compte X », avec le mode réciproque.
- **API BP** : `AccountCount`, `AccountName`, `AccountRefreshScreen`, `BankPerfectExePath`, `BankPerfectFileName`, `CategName`, `GetURL`, `LineAdd`, `MsgBox`, `OpenDialog` (filtre `"Fichier échéances BP|*.bpsched"`). Commandes `get_accounts`, `load_script_data:fabiobpscheduler`, `save_script_data:fabiobpscheduler:<pickle>`, `bp_export_folder`. Composants : `TDateTimePicker` (`.Date` en numéro de série Delphi, `.Format="dddd dd MMMM yyyy"`), `TLabel` (`OnMouseDown`, `OnMouseUp`, `Cursor`), `grid.ColWidths[i]`, `grid.Row`.
- **Formats** : `.bpsched` est une liste d'échéances au format pickle protocole 0, écrite en mode texte. L'ancien fichier `Scripts\Scheduler\scheduler.dat` est un pickle binaire (protocole 1) de la forme `{nom_fichier.bp: [échéances]}`.
- **Dépendances** : `nt` (`nt.lstat`, `nt.rename`), masque octal `0170000`/`0040000`.
- **Pièges Py2→3**
  - **Divisions entières dans `day_of_week` et `easter_date`** (`(23*m)/9`, `y/4`, `H/28`…) et dans `to_delphi_date` : en Py3, les jours fériés, les dates par défaut du sélecteur et les reports deviennent faux. Il faut `//`.
  - `CPositions.has_key(ctg)`.
  - `Recs.sort()` sur des tuples `(nextdate, dict)` : TypeError dès que deux échéances ont la même date. Utiliser `sort(key=...)`.
  - `nt` devient `os`, `0170000` devient `stat.S_ISDIR`.
  - `EValue.Text = abs(...)` et `EDelta.Text = 1` affectent des nombres à `Text`.
  - Divisions dans la mise en page.
- **Bogue existant** : le choix « Les lignes qui ne s'insèrent pas automatiquement » teste la clé `"auto_insert"` au lieu de `"autoinsert"`, donc il ne sélectionne rien.
- **Effort** : élevé.

#### Ajouter à l'échéancier — `Scheduler/scheduler_add.py` (114 l.)

- **Ce que fait l'utilisateur** : il clique sur une ligne puis lance le menu. Le script crée une échéance mensuelle (prochaine date = date de la ligne + 1 mois, insertion automatique). Le mode est retrouvé dans la liste ; à défaut, il vaut Virement reçu ou Prélèvement selon le signe. Un message confirme l'ajout.
- **API BP** : `AccountCurrent`, `BankPerfectExePath`, `BankPerfectFileName`, `GetURL`, `LineSelect(-1)` (ligne courante), `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`. Commandes `load_script_data:fabiobpscheduler` et `save_script_data:fabiobpscheduler:<pickle>`.
- **Pièges Py2→3** :
  - pickle ;
  - `extract_file_name` ne coupe que sur `"\\"` ;
  - `time.mktime` et `time.gmtime` pour ajouter des jours (fonctionne, mais `datetime` est préférable).
- **Effort** : faible à moyen.

#### Insertion des échéances — `Scheduler/scheduler_auto.py` (195 l.)

- **Déclencheur** : `ExecuteWithFile=1`, donc **à chaque ouverture de fichier**, sans intervention de l'utilisateur.
- **Ce que voit l'utilisateur** : pour chaque échéance avec `autoinsert` et un compte valide, le script insère toutes les occurrences jusqu'à aujourd'hui (ou jusqu'à `enddate`), en appliquant les reports S, D et F. Le tiers est préfixé par `[auto] ` ; un transfert produit deux lignes. Un message récapitule « Lignes insérées » et « Lignes refusées » (une ligne est refusée quand `OperationCount` n'a pas changé ou que le compte n'existe pas).
- **API BP** : `AccountCount`, `AccountName`, `AccountRefreshScreen`, `BankPerfectExePath`, `BankPerfectFileName`, `CategName`, `GetURL`, `LineAdd`, `MsgBox`, `OperationCount`. Commandes `load_script_data:fabiobpscheduler` et `save_script_data:fabiobpscheduler:<pickle>`.
- **Pièges Py2→3** :
  - **calcul de Pâques (`Easter`) en divisions entières**, à passer en `//` ;
  - pickle ;
  - `import sys` inutilisé.
- **Effort** : moyen. Ce script est **critique**, car il modifie les données automatiquement : il faut des tests de non-régression sur les dates.

### 2.6 Opérations

#### Transfert de compte à compte — `Transfer/transfer.py` (291 l.)

- **Ce que fait l'utilisateur** : fenêtre avec date (`TDateTimePicker`), mode (Virement, Chèque, Espèces, chacun associé à un couple de modes émis/reçu), détails, catégorie, montant, compte source et compte destination. Le script crée deux lignes, « Transfert vers le compte X » et « Transfert depuis le compte Y », et les **lie** par des champs supplémentaires. Les derniers paramètres saisis sont mémorisés.
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `CategName`, `GetURL`, `LineAdd` (renvoie l'index de la nouvelle ligne), `MsgBox`.
- **Commandes** :
  - `get_accounts` ;
  - `get_account_uid_from_id:%d` ;
  - `extra_set:%d;%d;%s;%s` avec les clés `#sisterAccUid` et `#sisterLineUid` ;
  - `extra_get:%d;%d;%s` avec la clé `UId` ;
  - `load_script_data:fabiobptransfer`, `save_script_data:fabiobptransfer:<pickle>` (dict `mode`, `categ`, `details`, `account1`, `account2`).
- **Pièges Py2→3**
  - `print(Uid1, Uid2)` : en Py2 cela affiche un tuple ; ce sont des traces de débogage à supprimer.
  - **`to_delphi_date` (`i / 4`…)** fausse la date par défaut du sélecteur ; il faut `//`.
  - Pickle ; divisions dans la mise en page.
- **Effort** : moyen.

#### Convertir la ligne en transfert — `Transfer/convert_line_to_transfer.py` (117 l.)

- **Ce que fait l'utilisateur** : il sélectionne **une seule** ligne visible dont le mode est convertible, puis choisit le compte lié. Le script crée la ligne miroir dans l'autre compte (mode réciproque, montant opposé), renomme les tiers en « Transfert vers/depuis le compte … » et lie les deux lignes (même mécanisme que transfer.py).
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `LineAdd`, `LineSelect(-1)` et `LineSelect(-2)` (bornes de la sélection), `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails` (écriture), `OperationMode`, `Operationthirdparty` (écriture), `VisibleLines`. Commandes `get_accounts`, `get_account_uid_from_id:`, `extra_set:`, `extra_get:`.
- **Pièges Py2→3** : `modes.has_key(...)`.
- **Bogue existant** : les `extra_set` sont exécutés même si `LineAdd` a échoué.
- **Effort** : faible.

#### Ventiler l'opération — `Ventilate/ventilate.py` (114 l.)

- **Ce que fait l'utilisateur** : il part de la ligne sélectionnée et ajoute des parts (détail, catégorie, montant) jusqu'à épuiser le montant. Le script supprime alors la ligne d'origine et crée N lignes, avec le tiers `"[i/N] tiers"` et les détails `"détail (Total: X)"`. Un mode « Chq NNN » devient « Chèque émis ».
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `CategName`, `LineAdd`, `LineDelete(acc, ligne)`, `LineSelect(-1)`, `MsgBox`, `OperationAmount`, `OperationCateg`, `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`.
- **Pièges Py2→3** : aucun de particulier.
- **Effort** : faible.

#### Modifier le groupe d'opérations — `ModifyGroup/modifygroup.py` (141 l.)

- **Ce que fait l'utilisateur** : il sélectionne une plage de lignes (seules les lignes visibles comptent). Dans la fenêtre, il coche les champs à modifier : Date (`TDateTimePicker`), Mode (appliqué seulement aux lignes de même signe), Tiers, Détails, Catégorie, Montant (valeur absolue multipliée par le signe du mode), Pointage.
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `CategName`, `LineSelect(-1/-2)`, `MsgBox`, `OperationGetNameFromModeIndex(0..9)`, `OperationModeSign`, `VisibleLines`. **Écriture** dans `OperationDate`, `OperationMode`, `Operationthirdparty`, `OperationDetails`, `OperationCateg`, `OperationAmount`, `OperationMark`.
- **Pièges Py2→3** : `from_delphi_date` (pas de division, passe) ; `Modalresult` mal casé (l'hôte doit rester insensible à la casse).
- **Effort** : faible.

#### Saisie multiple — `MultipleInput/multiple_input.py` (474 l.)

- **Ce que fait l'utilisateur** : il tape ou colle des lignes dans un mémo. Une évaluation s'affiche en direct dans une grille, avec les valeurs non reconnues en rouge et le total.
  - Format standard : `date;mode;tiers;détails;catégorie;montant`.
  - Mode rapide : séparateur espace, avec un double espace entre tiers, détails et catégorie.
  - Collage d'un tableau web : séparateur TAB.
  - Saisies partielles : date incomplète (`15` → mois et année courants ; `1503`) ; mode abrégé et signe `+`/`-` ; catégorie floue (`a+b`, `x++` pour la n-ième correspondance) ; **abréviations** personnelles (« raccourci=texte complet », éditées dans une fenêtre dédiée).
  - Boutons : « Insérer », « Insérer pointé », Aide, Abréviations.
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `CategName`, `LineAdd(..., pointage)`, `MsgBox`. Commandes `load_script_data:fabiobpabbreviations` et `save_script_data:fabiobpabbreviations:<pickle>` (dict).
- **Pièges Py2→3**
  - **`print line, sign`** (ligne 94) : **erreur de syntaxe** en Py3.
  - `translate` à 2 arguments ; `keys().sort()`.
  - `WordWrap="false"` passe une chaîne : l'hôte doit convertir.
  - Pickle ; divisions dans la mise en page.
- **Effort** : moyen.

#### Champs supplémentaires de la ligne — `ExtraValues/extra_values_edit.py` (173 l.)

- **Ce que fait l'utilisateur** : il voit la grille Nom/Valeur des champs supplémentaires de la ligne sélectionnée, avec les boutons Ajouter (fenêtre nom + valeur), Modifier (`InputText`) et Supprimer (valeur vide).
- **API BP** : `AccountCurrent`, `InputText(titre, invite, défaut)`, qui renvoie `(résultat, valeur)`, `LineSelect(-1)`, `MsgBox`.
- **Commandes** :
  - `extra_get:%d;%d` (sans clé) renvoie la liste des clés `k1;k2` ;
  - `extra_get:%d;%d;%s` renvoie une valeur ;
  - `extra_set:%d;%d;%s;%s` (une valeur vide supprime le champ).
- **Pièges Py2→3** : aucun de notable. La grille affiche aussi les clés internes (`UId`, `#sisterAccUid`…) : il faudra sans doute les masquer.
- **Effort** : faible.

#### Calcul automatique des champs — `Auto Fields/auto_fields.py` (594 l.)

- **Ce que fait l'utilisateur** : il gère une liste de règles. Chaque règle a un champ testé (Tiers, Détails, ou « Tiers ou Détails ») et une valeur cherchée :
  - `a+b` signifie ET, `!x` signifie NON ; avec l'option « + = OU », `a+b` signifie OU ;
  - une valeur commençant par `=` est une **formule Python évaluée** sur les variables `date`, `mode`, `tiers`, `details`, `montant`, `categ`/`categorie`/`category`, `categ_id`, `paymode`, `thirdparty`/`third`/`people`, `amount`/`value`, `detail`.
  - Si la règle s'applique, elle peut imposer : le mode (seulement si le signe correspond), le tiers, les détails, la date (valeurs fixes ou formule `=…`) et la catégorie (option : uniquement sur les lignes sans catégorie).
  - Boutons : Ajouter, Modifier, Supprimer, Appliquer (avec confirmation), « Appliquer actives », Importer et Exporter. Un double-clic active ou désactive une règle.
  - **Mode automatique** : quand une ligne est ajoutée ou modifiée, l'hôte relance le script. `get_lines_changed` renvoie alors `"compte-ligne;compte-ligne"`, et les règles actives marquées « Appliquer automatiquement à chaque nouvelle ligne » s'appliquent en silence.
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `BankPerfectExePath`, `BankPerfectFileName`, `CategName`, `CategParent`, `GetURL`, `MsgBox` (également appelée avec un seul argument), `OpenDialog`, `OperationGetNameFromModeIndex(0..9)`, `VisibleLines`. Écriture via proxies dans `OperationMode`, `Operationthirdparty`, `OperationDetails`, `OperationDate` et `OperationCateg`. Lecture de `OperationAmount`. Commandes `get_lines_changed`, `load_script_data:fabiobpautofields`, `save_script_data:fabiobpautofields:<pickle>`, `bp_export_folder`.
- **Formats** :
  - export `.txt` : une règle par ligne, écrite avec `repr(dict)` ;
  - import `.txt` : relu avec **`eval(ligne)`** ;
  - import `.bprules` : pickle ;
  - ancien fichier de secours `Scripts\Auto Fields\<nom du .bp>.dat` (pickle).
  - Clés d'une règle : `field`, `contains`, `do_mode`, `mode`, `do_third`, `third`, `do_info`, `info`, `do_categ`, `categ`, `active`, `withnewline`, `contains_or`, `emptycateg`, `do_date`, `date`.
- **Pièges Py2→3**
  - **Opérateur `<>`** (lignes 486, 493 et 500) : **erreur de syntaxe** en Py3.
  - `eval` des formules utilisateur et des fichiers `.txt` : à garder pour la compatibilité, mais c'est un risque de sécurité (bac à sable, ou au minimum `ast.literal_eval` pour l'import).
  - Les fichiers `.txt` exportés par Py2 contiennent des échappements `\xe9` : relus en Py3, ils donnent bien `é`, mais les caractères CP1252 de 0x80 à 0x9F (`€`, `œ`) seront faux.
  - `CPositions.keys()` (passe).
  - Pickle.
- **Effort** : élevé (règles, formules, déclenchement automatique, formats hérités).

#### Remplacer — `Replace/search_replace.py` (50 l.)

- **Ce que fait l'utilisateur** : il saisit le texte à chercher et le texte de remplacement (sensible à la casse ; un `;` est remplacé par un espace). Le remplacement porte sur Tiers et Détails des lignes visibles, puis le nombre de remplacements s'affiche.
- **API BP** : `AccountCurrent`, `AccountRefreshScreen`, `MsgBox`, `OperationDetails`, `Operationthirdparty` (écriture via proxy), `VisibleLines`.
- **Pièges Py2→3** : aucun.
- **Effort** : faible.

#### Filtres automatiques — `Filters/setfilters.py` (227 l.)

- **Ce que fait l'utilisateur** : fenêtre « Filtres prédéfinis » listant des règles (nom, date de début, date de fin, mode, tiers, catégorie, montant, pointage).
  - Les dates peuvent être dynamiques : `j` (jour), `m` (mois), `a` (année), `dj` (dernier jour du mois), combinés par arithmétique, par exemple `1/m-1/a` ou `j-50/m/a`.
  - Boutons « Ajouter », « Supprimer » et « Filtrer ».
  - Règles fournies par défaut : « Mois actuel » et « Mois précédent ».
  - **Bouton déroulant de la barre d'outils** : il liste les filtres et « Annuler tous les filtrages » (Maj+F7).
- **Protocole déroulant** :
  1. L'hôte lance le script avec `get_script_parameters == "populate_popup_menu"`.
  2. Le script répond avec `set_script_parameters:Annuler tous les filtrages|Maj+F7;règle1;règle2…` (format `libellé|raccourci` séparé par `;`).
  3. Quand l'utilisateur clique sur un élément, l'hôte relance le script avec `get_script_parameters` égal au nom de l'élément.
- **API BP** : `GetURL`, `MsgBox`. Commandes `load_script_data:fabiobpautofilters`, `save_script_data:fabiobpautofilters:<pickle>`, `get_script_parameters`, `set_script_parameters:<liste>`, `set_filter:%s;%s;%s;%s;%s;%s;%d` (`set_filter:;;;;;;4` annule tout). Pointage : 4 = toutes, 3 = pointées et rapprochées, 2 = rapprochées, 1 = pointées, 0 = non pointées.
- **Pièges Py2→3** : `t = rules.keys(); t.sort()` ; `List.Items.Assign(liste_python)` (l'hôte doit accepter une liste) ; `eval` des expressions de date ; pickle partagé avec Budget.
- **Effort** : moyen.

### 2.7 Outils autonomes (sans fichier ouvert)

#### Simulateur de crédits — `Loan/loan.py` (704 l.) + `loan.slk`

- **Ce que fait l'utilisateur**
  - Il saisit trois des quatre valeurs (capital, taux annuel, nombre de mensualités, mensualité). Un bouton « Calculer » en face de chaque champ calcule le quatrième : taux par Newton-Raphson, durée par logarithme.
  - Les boutons « + » et « − » font varier une valeur et relancent le dernier calcul.
  - Le script affiche le coût total et la part d'intérêts, ainsi qu'un graphique en aires du capital et des intérêts restants.
  - Tableau d'amortissement : mois de début, valeur résiduelle, « début de période ». Un double-clic sur une ligne en fait le nouveau point de départ.
  - Il gère des profils nommés (« Enregistrer » via `InputText`, « Supprimer ») et un export tableur `loan.xls`.
- **API BP** : `BankPerfectPluginPath`, `InputText`, `MsgBox`, `ShellExecute`. Commandes `appdata_dir` (dossier de `loan.dat`) et `bp_export_folder`. Graphiques : `CreateChart`, `DrawChart("loan_panel_chart", noms, valeurs, 1, 0, 5, 1)`, `DestroyChart`.
- **Formats** : `loan.dat` en texte, une ligne par profil : `nom;;capital;taux;n;mensualité;date_début_série_Delphi;résiduel;début_période`.
- **Pièges Py2→3**
  - `names = profiles.keys(); names.sort()`.
  - **`to_delphi_date` (`i / 4`…)** : les dates écrites dans `loan.dat` deviendraient fausses ou flottantes ; il faut `//`.
  - Divisions dans la mise en page.
- **Bogue existant** : `rates.append("")` au lieu de `Rates` (NameError, dans une branche rare).
- **Effort** : moyen.

#### Calculatrice — `Calc/calc.py` (73 l.)

- **Ce que fait l'utilisateur** : il dispose d'un clavier de 40 touches (trigonométrie directe, inverse et hyperbolique, `ln`, `Log`, `e`, π, √, `^`, `%`, `EE`, degrés-minutes-secondes, Deg/Rad). La saisie est évaluée par `eval` à l'appui sur « = » ou ENTRÉE ; le résultat devient rouge en cas d'erreur.
- **API BP** : aucune (seulement `CreateComponent`). Composants : `TBitBtn`, `TEdit` (`OnKeyPress` avec `Key.Value`).
- **Dépendances** : `math`, et la **police « Symbol »** pour afficher ←, √ et π, codés `¬`, `Ö` et `p`.
- **Pièges Py2→3** : `Top=60+40*(i/8)` donne un flottant ; la police Symbol n'existe pas hors Windows (utiliser les caractères Unicode `←√π`) ; `eval` est sans risque particulier ici.
- **Effort** : faible.

#### Conversions numériques — `Numerals/numerals.py` (125 l.)

- **Ce que fait l'utilisateur** : il saisit des chiffres et obtient les chiffres romains (et inversement) ainsi que le nombre en toutes lettres, en français, jusqu'aux trillions.
- **API BP** : aucune.
- **Pièges Py2→3** : `long()` ; **`N = N / 10` doit devenir `//`**, sinon le texte est faux.
- **Effort** : faible (mais ce piège est critique).

#### Vérifier une carte bancaire — `Luhn/luhn.py` (30 l.)

- **Ce que fait l'utilisateur** : il saisit un numéro et voit en direct « valide », « incorrect » ou « incomplet » (16 chiffres attendus).
- **API BP** : aucune.
- **Pièges Py2→3** : aucun.
- **Bogue existant** : `(int(c) * 2) % 9` donne 0 pour le chiffre 9 au lieu de 9, donc l'algorithme de Luhn est faux pour les chiffres 9 en position doublée.
- **Effort** : faible.

#### Conversions de devises — `Currencies/currencies.py` (267 l.)

- **Ce que fait l'utilisateur** : il voit une liste de devises, avec par défaut EUR et USD au taux 1,48479. La saisie d'un montant dans un champ recalcule tous les autres.
  - « Ajouter une devise » ouvre une recherche, insensible aux accents, parmi environ 150 devises.
  - Chaque devise a un bouton « Supprimer ».
  - « Télécharger les cotations » récupère les cours depuis xe.com.
- **API BP** : `MsgBox`. Commandes :
  - `ini_read:CurrenciesData;symbols`, `ini_read:CurrenciesData;values` ;
  - `ini_write:CurrenciesData;symbols;…`, `ini_write:CurrenciesData;values;…` ;
  - `file:http://www.xe.com/ucc/convert.cgi?Amount=1&From=EUR&To=%s&template=fr` (`file:` sans `@` renvoie le contenu de la page).
- **Pièges Py2→3**
  - `translate` avec une table de 256 caractères.
  - Détection du type de composant par `str(c.__class__)` contenant « Edit » ou « Button » : le wrapper hôte doit garder des noms de classes compatibles.
  - Itération de `f.Components` et `Box.Components`.
  - `eval` dans `str_to_float`.
- **Service** : la page xe.com scrapée n'existe plus. Il faut un autre fournisseur, par exemple le flux quotidien de la BCE (`eurofxref-daily.xml`).
- **Effort** : élevé (la fonction réseau est à refaire).

#### Notes — `Notes/notes.py` (89 l.) et `Notes/notes_auto.py` (29 l.)

- **Ce que fait l'utilisateur** : notes est un bloc-notes global, qui ne dépend pas du fichier `.bp`. Il affiche la liste des titres et, pour la note courante, son titre, son texte et la case « Afficher au chargement d'un fichier ». Le bouton « « » ajoute ou met à jour la note, « » » la supprime. La sauvegarde se fait sur OK. notes_auto (`ExecuteWithFile`) affiche, à l'ouverture d'un fichier, les notes marquées.
- **API BP** : `BankPerfectExePath`, `MsgBox` (`MB_YESNO+64`, `IDYES`).
- **Formats** : `Scripts\Notes\notes.dat` est un pickle protocole 0 de la forme `{titre: (afficher, texte)}`. `notes.py.bak` ne diffère que par `Multiselect=1`.
- **Pièges Py2→3** :
  - `has_key` ;
  - `k = Notes.keys(); k.sort()` ;
  - pickle en mode texte ;
  - le script **écrit dans le dossier du programme** ;
  - chemin `"%sScripts\\Notes\\notes.dat"`.
- **Effort** : faible.

### 2.8 Internet et maintenance

#### Sauvegarder par e-mail — `BackupMail/backup_mail.py` (168 l.)

- **Ce que fait l'utilisateur** : il saisit une ou plusieurs adresses (séparées par `;` ou `,`), qui sont validées et mémorisées. Le fichier `.bp` est envoyé en pièce jointe, avec un corps HTML qui résume chaque compte : nombre de lignes, dernière date, pointé, à venir, solde.
- **API BP** : `AccountIsVirtual`, `BankPerfectFileName`, `MsgBox`, `OperationAmount`, `OperationDate`, `OperationMark`. Commandes `get_accounts`, `load_script_data:fabiobpmailaddress`, `save_script_data:fabiobpmailaddress:<adresse>` (texte brut, pas de pickle), et `send_mail:Sauvegardes BankPerfect <webmaster@chelly.net>|<destinataire>|Sauvegarde BankPerfect : <fichier>|<corps HTML>|<chemin du .bp>`. Les champs sont séparés par `|`, si bien qu'un `|` dans le corps casserait la commande.
- **Dépendances** : l'envoi SMTP est fait par l'hôte, avec l'adresse d'expéditeur `webmaster@chelly.net` codée en dur.
- **Pièges Py2→3** : divisions dans la mise en page.
- **Effort** : moyen, car le côté hôte est à revoir (SMTP configurable ou client de messagerie du système).

#### Liste des sauvegardes — `Backups/backups.py` (117 l.)

- **Ce que fait l'utilisateur**
  - « Sauvegardes locales » liste les `*.bp0` de `bp_backup_folder`, par date décroissante.
  - « Sauvegardes FTP » liste les `*.bp` du serveur configuré.
  - « Restaurer » propose un nom (le nom sans suffixe `_N`, avec l'extension `.bp`). Si le fichier existe, il est renommé en `.bak`. Le script copie ensuite le fichier ou le télécharge, puis propose de l'ouvrir.
- **API BP** : `BankPerfectOpenFile(chemin, "")`, `GetURL`, `MsgBox`, `OpenDialog(..., ".bp", "Fichier BP|*.bp")`.
- **Commandes** :
  - `bp_backup_folder` ;
  - `copy_file:%s|%s` (la doc indique un `;` comme séparateur, mais le code utilise `|`) ;
  - `ftp_lst:default_bp_folder*.bp|default_bp_login|default_bp_password@default_bp_url` (renvoie des lignes `type|nom|date|taille`) ;
  - `ftp_get:<local>|default_bp_folder<nom>|default_bp_login|default_bp_password@default_bp_url`.
- **Dépendances** : `nt` (`lstat`, `listdir`, `rename`), masques octaux, `time.gmtime`.
- **Pièges Py2→3** : `nt` devient `os` ; `0170000` devient `0o170000`, ou mieux `os.path.isdir` ; divisions dans la mise en page.
- **Effort** : moyen. Le FTP en clair est à reconsidérer (SFTP, WebDAV, dossier synchronisé).

#### Télécharger les cotations — `UpdateStock/update_stock.py` (43 l.)

- **Ce que fait l'utilisateur** : pour chaque ligne du compte courant dont le mode est « Titre », le champ Détails contient le code ISIN et le tiers commence par la quantité. Le script télécharge le cours, multiplie par la quantité, convertit si nécessaire selon les taux du compte, puis écrit le montant.
- **API BP** : `AccountCurrency1`, `AccountCurrency2`, `AccountCurrent`, `AccountRefreshScreen`, `GetURL("http://fr.moneycentral.msn.com/investor/quotes/quotes.aspx?Symbol=FR%3a<ISIN>")`, `OperationAmount` (écriture via proxy), `OperationDate`, `OperationDetails`, `OperationMode`, `Operationthirdparty`.
- **Service** : MSN Money a disparu (le manuel cite Yahoo Finance, également fermé). Il faut un fournisseur de remplacement.
- **Effort** : élevé. Le code est trivial, mais la source de données est à trouver.

#### Installer/gérer les plugins — `ManagePlugins/manage_plugins.py` (198 l.)

- **Ce que fait l'utilisateur**
  - Il voit le catalogue en ligne, obtenu par `BPEval("http://www.chelly.net/plugins.php")`. La réponse est une liste séparée par `|`, à raison de 10 champs par plugin : id, name, author, description, downloads, filename, date_insertion, rating, picture, thumbnail. Le catalogue s'affiche dans une grille (Nom, Auteur, Date, Note, Téléch.) avec un mémo de description.
  - « Installer » ne fonctionne que pour les `.exe` : le script obtient l'URL via `BPEval("http://www.chelly.net/ajax/download_plugin.php?id=…")`, télécharge le fichier par `BPEval("file:<temp>temp_plugin.exe@http://www.chelly.net/<url>")`, l'**exécute** avec `nt.popen3`, puis le supprime.
  - La liste des plugins installés vient du parcours de `<exe>\Plugins\*\plugin.ini` (clé `name=`). « Supprimer » efface récursivement le dossier du plugin.
- **API BP** : `BankPerfectExePath`, `MsgBox`. Commandes `http://…`, `file:…@…`, `temp_dir`.
- **Dépendances** : `nt` (`lstat`, `listdir`, `unlink`, `rmdir`, `popen3`), installateurs Windows `.exe`.
- **Effort** : élevé, et la fonction est **à repenser** : nouveau dépôt, archives `.zip` de scripts Python, aucune exécution de binaire.

#### Télécharger le manuel — `Manuel/manuel.py` (75 l.)

- **Ce que fait l'utilisateur**
  1. Le script télécharge `http://www.chelly.net/wiki/wiki/manuel` et extrait la zone comprise entre `<div id="Content">` et `<div id="Menu">`.
  2. Il télécharge chaque page liée (`http://www.chelly.net/wiki/wiki.php?<url encodée>`) et corrige le mojibake UTF-8 vers Latin-1 avec une table de correspondance.
  3. Il assemble le tout en un seul HTML ISO-8859-1, avec des ancres internes, enregistré via `OpenDialog` (par défaut `desktop_dir\manuel_bankperfect.htm`).
  - Le fichier `manuel.html` à la racine du projet est très probablement un produit de ce script.
- **API BP** : `OpenDialog`. Commandes `http://…` et `desktop_dir`.
- **Effort** : script à **remplacer** par un manuel livré avec l'application, ou par un simple lien vers une documentation en ligne (le site est mort).

#### Test — `test/test.py` (59 l.) + `test.png`

- **Contenu** : démonstration. Le script liste les modes et leurs signes (`mode_count`, `mode_sign:%d`, `OperationGetNameFromModeIndex`) mais n'utilise pas le résultat. Il joue un son (`BPEval("play:C:\Windows\Media\Tada.wav|1")`, puis `play:` pour l'arrêter), contient un exemple de `DrawChart` en commentaire, et charge une image (`TImage.Picture.LoadFromFile(BankPerfectExePath() + "scripts\\test\\test.png")`).
- Il apparaît par erreur dans le menu « internet ».
- **Effort** : ne pas porter tel quel ; le garder éventuellement comme exemple pour les développeurs.

---

## 3. Synthèse transverse : API utilisée par l'ensemble des scripts

Le décompte porte sur les 44 fichiers `.py` actifs.

### 3.1 Module `BP` — par nombre de scripts utilisateurs

| Fonction / propriété `BP.*` | Nb scripts | Remarques d'usage |
|---|---:|---|
| `MsgBox(texte, flags)` | 32 | flags 0, 36, 64, 68, `MB_YESNO+MB_ICONINFORMATION`… ; renvoie 6 (IDYES). Parfois appelée avec un seul argument |
| `AccountCurrent()` | 25 | index réel du compte affiché |
| `OperationAmount[a][l]` | 24 | float ; écriture dans modifygroup et update_stock |
| `OperationDate[a][l]` | 23 | `"JJ-MM-AAAA"` ; écriture dans modifygroup et auto_fields |
| `CategName[i]` | 19 | `"id=nom"` (espace initial pour une sous-catégorie) ; itérable |
| `Operationthirdparty[a][l]` | 19 | écriture dans 4 scripts. Attention à la casse : `thirdparty` en minuscules |
| `OperationDetails[a][l]` | 17 | écriture dans 4 scripts |
| `OperationMode[a][l]` | 17 | nom du mode, ou `"Chq NNN"` |
| `OperationCateg[a][l]` | 15 | id de catégorie, -1 si aucune |
| `AccountRefreshScreen()` | 13 | |
| `GetURL(cmd)` | 13 | identique à `BPEval` |
| `ShellExecute(verbe, fichier, params, show)` | 12 | toujours `"open"` |
| `BankPerfectExePath()` | 11 | se termine par `\` ; sert à construire `Scripts\…` |
| `AccountName[i]` | 10 | utilisé aussi avec `len()` et en itération |
| `OperationMark[a][l]` | 10 | 0 non pointé, 1 pointé, 2 rapproché |
| `LineAdd(a, date, mode, tiers, détails, categ, montant, pointage)` | 9 | renvoie l'index de la nouvelle ligne (0 en cas d'échec) |
| `VisibleLines()` | 9 | liste des index de lignes visibles du compte courant |
| `BankPerfectFileName()` | 8 | chemin complet du `.bp` |
| `OpenDialog(titre, nom_défaut, ext, filtre)` | 7 | sert à l'ouverture **et** à l'enregistrement ; renvoie `""` si annulé |
| `BankPerfectPluginPath()` | 6 | `"…|<chemin du .py en cours>"` |
| `AccountCount()` | 5 | |
| `AccountIsVirtual[i]` | 5 | itérable |
| `CategParent[i]` | 5 | position du parent |
| `LineSelect(-1)` / `LineSelect(-2)` | 5 | ligne courante / autre borne de la sélection |
| `AccountBranchNumber[i]` | 4 | `"banque guichet"` |
| `AccountNumber[i]` | 4 | |
| `OperationCount[a]` | 4 | |
| `OperationGetNameFromModeIndex(i)` | 4 | i de 0 à `mode_count`-1 (ou 0–9) |
| `AccountCurrency1[i]` | 3 | `"symbole|taux"` |
| `AccountBIC`, `AccountBankName`, `AccountBranchAddress`, `AccountBranchName`, `AccountIBAN` | 2 chacun | |
| `AccountCurrency2[i]` | 2 | |
| `CategAdd(nom, 0)` | 2 | |
| `CategCount()` | 2 | |
| `InputText(titre, invite, défaut)` | 2 | renvoie `(résultat, valeur)` |
| `AccountBankLogo[i]` | 1 | chemin d'un PNG |
| `BankPerfectOpenFile(chemin, mot_de_passe)` | 1 | |
| `LineDelete(a, l)` | 1 | |
| `OperationModeSign[a][l]` | 1 | |

**Exposées par l'hôte mais inutilisées par les scripts livrés** (à garder pour d'éventuels scripts tiers) : `AccountChangeCurrent`, `BankPerfectSave`, `BankPerfectNewFile`, `BankPerfectCloseFile`, `CategDelete`, `CategRename`, `OperationMarkDate`, `OperationModeIndex`, `SingleLine`, `CustomForm`.

**Sémantique indispensable des proxies** :

- `x = BP.OperationDetails[a]` doit renvoyer un objet qui :
  - accepte `len(x)`, `x[i]`, l'itération et `zip(x, …)` ;
  - écrit dans le document lors d'un `x[i] = v` (search_replace, auto_fields et update_stock en dépendent).
- `BP.AccountName`, `BP.CategName` et `BP.AccountIsVirtual` doivent être itérables et accepter `len()`.

### 3.2 Fonctions globales injectées

| Fonction / constante | Nb scripts |
|---|---:|
| `CreateComponent(classe, parent)` + `.SetProps(**kw)` | 35 |
| `BPEval(cmd)` | 27 |
| `MB_YESNO` | 15 |
| `MB_ICONINFORMATION` | 11 |
| `Rect(l, t, r, b)` | 10 |
| `IDYES` | 9 |
| `CreateChart(panel)` / `DrawChart(panel, noms, valeurs, nulles, 3d, type[, x])` / `DestroyChart(panel)` | 3 |
| `MB_ICONQUESTION` | 3 |
| `ExportChart(panel, chemin_pdf)` | 1 |

### 3.3 Commandes `BPEval` / `BP.GetURL` (chaînes exactes)

| Commande (forme exacte) | Nb | Scripts |
|---|---:|---|
| `bp_export_folder` | 15 | auto_fields, budget, categs_totals, charts, export_csv, export_html, export_qif, export_xls, export_xml, loan, people_totals, previsions, rib, scheduler, totals |
| `load_script_data:<clé>` | 12 | all_accounts_view, auto_fields, backup_mail, budget, budget_year, multiple_input, previsions, scheduler, scheduler_add, scheduler_auto, setfilters, transfer |
| `save_script_data:<clé>:<valeur>` | 12 | les mêmes (previsions définit l'écriture mais ne l'appelle jamais) |
| `get_accounts` (renvoie `id=nom;id=nom…` dans l'ordre d'affichage) | 11 | all_accounts_view, backup_mail, budget, budget_year, categs_totals, charts, convert_line_to_transfer, people_totals, scheduler, totals, transfer |
| `extra_get:%d;%d` et `extra_get:%d;%d;%s` | 3 | convert_line_to_transfer, extra_values_edit, transfer |
| `extra_set:%d;%d;%s;%s` | 3 | convert_line_to_transfer, extra_values_edit, transfer |
| `http://…` (télécharge une page) | 3 | manage_plugins, manuel, update_stock (via `GetURL`) |
| `simulate_set_filter:` / `simulate_set_filter:%s;%s;%s;%s;%s;%s;%d` | 3 | all_accounts_view, charts, budget |
| `file:<url>` / `file:<chemin_local>@<url>` | 2 | currencies (contenu), manage_plugins (téléchargement) |
| `get_account_uid_from_id:%d` | 2 | transfer, convert_line_to_transfer |
| `get_script_parameters` | 2 | previsions, setfilters |
| `mode_count` | 2 | charts, test |
| `pdf:<code>` | 2 | receipt, rib |
| `appdata_dir` | 1 | loan |
| `bp_backup_folder` | 1 | backups |
| `color:%d` (0–13) | 1 | all_accounts_view |
| `convert_to_bitmap:%s|%s` | 1 | rib |
| `copy_file:%s|%s` | 1 | backups |
| `desktop_dir` | 1 | manuel |
| `format_date:%s` | 1 | all_accounts_view |
| `ftp_get:…|…|login|password@url` / `ftp_lst:…|login|password@url` (valeurs spéciales `default_bp_*`) | 1 | backups |
| `get_lines_changed` | 1 | auto_fields |
| `ini_read:section;clé` / `ini_write:section;clé;valeur` | 1 | currencies |
| `mode_sign:%d` | 1 | test |
| `play:<wav>|<boucle>` / `play:` | 1 | test |
| `send_mail:from|to|subject|body|attachment` | 1 | backup_mail |
| `set_filter:%s;%s;%s;%s;%s;%s;%d` | 1 | setfilters |
| `set_script_parameters:<items>` | 1 | setfilters |
| `temp_dir` | 1 | manage_plugins |
| `draw_mark:…` | 0 | uniquement dans un commentaire d'all_accounts_view |

Clés de `save_script_data` / `load_script_data` : `fabiobpviewall`, `fabiobpautofields`, `fabiobpmailaddress`, `fabiobpautofilters`, `fabiobpbudget`, `fabiobpbudgetyear`, `fabiobpabbreviations`, `fabiobpscheduler`, `fabiobptransfer`.

### 3.4 Couche IHM (VCL) utilisée — à fournir par l'hôte ou par une couche de compatibilité

- **Classes** (nombre de `CreateComponent`) : `TButton` (≈136), `TLabel` (≈133), `TEdit` (58), `TForm` (54), `TComboBox` (41), `TCheckBox` (32), `TDrawGrid` (17), `TMemo` (9), `TListBox` (7), `TGroupBox` (7), `TDateTimePicker` (5), `TRadioButton` (4), `TPanel` (3), `TImage` (1), `TBitBtn` (1), `TBevel` (1).
- **Événements** : `OnClick` (100), `OnResize` (35), `OnChange` (25), `OnDrawCell` (16), `OnExit` (7), `OnDblClick` (7), `OnActivate` (5), `OnMouseUp` (3), `OnDestroy` (3), `OnShow` (2), `OnMouseDown` (2), `OnKeyPress` (2, `Key.Value` modifiable), `OnKeyUp` (1), `OnEnter` (1). Les noms sont parfois en minuscules (`onResize`, `onchange`, `onclick`, `onDblClick`) : la résolution doit être **insensible à la casse**.
- **Canevas des grilles** : `Brush.Color`, `Brush.Style`, `Font.Color`, `Font.Style`, `FillRect`, `TextRect`, `TextWidth`, `TextHeight` ; `Rect` modifiable (`R.Left += 1`).
- **Propriétés notables** :
  - fenêtres et composants : `ModalResult` (valeurs 1, 2, 6, 7, 9, 10), `ShowModal()`, `Close()`, `Constraints.MinWidth/MinHeight`, `Position="poMainFormCenter"`, `BorderStyle`, `BorderIcons`, `Anchors`, `Font.Name/Size/Style`, `Cursor=-21`, `Hint`, `ShowHint` ;
  - grilles : `Selection.Top/Left`, `ColWidths[i]`, `Row`, `RowCount`, `ColCount`, `FixedRows`, `FixedCols`, `DefaultColWidth`, `DefaultRowHeight`, `Options=[...]`, `ScrollBars` ;
  - listes : `Items.Text`, `Items[i]`, `Items.Add`, `Items.Delete`, `Items.Assign(list)`, `len(Items)` ; `ItemIndex` ; `TListBox.Selected` (liste) ;
  - sélecteur de date : `TDateTimePicker.Date` (numéro de série Delphi) et `.Format` ;
  - divers : `Picture.LoadFromFile`, `Components` (itérable), `ClassName`, `Tag`, `Name`, `SetBounds`, `SetFocus`, `SelectAll`, `Repaint`.
- **Coercitions attendues de l'hôte** : `Text = int|float`, `Checked="True"`, `WordWrap="false"`, positions en flottant (voir 4.1).

---

## 4. Pièges de conversion Python 2.3 → 3 (transverses)

### 4.1 Tableau des constructions à traiter

| Construction | Gravité | Scripts concernés | Remède |
|---|---|---|---|
| Sources en CP1252 sans `coding` valide | Bloquant | tous ceux qui contiennent des accents (≈40) | Convertir les fichiers en UTF-8 (`iconv -f cp1252 -t utf-8`) |
| `print x, y` (instruction) | Erreur de syntaxe | multiple_input | `print(x, y)`, ou supprimer |
| `<>` | Erreur de syntaxe | auto_fields | `!=` |
| `raise E, "msg"` | Erreur de syntaxe | import_csv | `raise E("msg")` |
| Octal `0170000`, `0040000` | Erreur de syntaxe | backups, manage_plugins, scheduler | `stat.S_ISDIR(os.stat(p).st_mode)` / `os.path.isdir` |
| `dict.has_key(k)` | AttributeError | convert_line_to_transfer, scheduler, import_qif, import_csv, notes, export_qif, export_csv | `k in d` |
| `d.keys().sort()` (méthode de liste sur une vue) | AttributeError | setfilters, previsions, loan, import_csv, scheduler, import_qif, import_ofx, people_totals, notes, notes_auto, multiple_input, manage_plugins, budget | `sorted(d)` |
| `str.translate(table256)` et `translate(table, delete)` | TypeError ou résultat faux | import_qif, import_csv, people_totals, multiple_input, currencies | `str.maketrans(src, dst, delete)` |
| `long()` | NameError | receipt, numerals, rib | `int()` |
| `file.xreadlines()`, `it.next()` | AttributeError | import_csv | itération directe, `next(it)` ; mieux, le module `csv` |
| **Division entière `/` utilisée comme index ou dans un calcul de date** | **Silencieux, résultats faux** | budget_year (`(ACol-1)/3` comme index), scheduler (`day_of_week`, `easter_date`), scheduler_auto (`Easter`), numerals et receipt (`N/10`), `to_delphi_date` dans export_xls, transfer, budget, loan et scheduler, import_csv (`Century`), previsions (`abs(p)/10`) | `//` |
| Division `/` dans les calculs de mise en page (`(w - x) / 2`) | Flottants transmis à des propriétés entières | ≈25 scripts | `//`, **ou** l'hôte tronque et arrondit les flottants affectés à `Left/Top/Width/Height/ColWidths/DefaultColWidth` (recommandé, en complément) |
| `cPickle` | ImportError | 14 scripts | `pickle`, avec une couche de compatibilité (voir 4.2) |
| Tri de tuples contenant des dicts | TypeError en cas d'égalité | scheduler, budget_year | `sort(key=lambda r: r["nextdate"])` |
| `str(float)` (12 chiffres en Py2.3, `repr` le plus court en Py3) | Format différent | export_csv, export_xml, people_totals, receipt | Formatage explicite `"%.2f"` |
| `open(..., "r")` sur un pickle ; `open(..., "w")` d'exports | Erreur ou encodage incorrect | notes, receipt, scheduler, auto_fields ; tous les exports | `"rb"`/`"wb"` pour les pickles ; `encoding=` explicite pour les textes |
| `eval()` sur une saisie utilisateur ou un fichier | Sécurité | calc, setfilters, budget, auto_fields, loan, currencies, budget_year, all_accounts_view | Garder pour les formules, sinon `ast.literal_eval` ; restreindre `globals` |
| `zip`, `map`, `range` paresseux | Faible | ≈11 scripts | Vérifié : tous les usages sont itérés une seule fois ou convertis, sauf `{}.fromkeys().keys().sort()` (import_ofx) |
| Module `nt` | ImportError hors Windows | backups, scheduler, manage_plugins | `os`, `shutil`, `pathlib` |
| `nt.popen3` | Obsolète | manage_plugins | Supprimer (voir section 5) |
| Chemins `"Scripts\\X\\y.dat"` et découpe de nom sur `"\\"` | Faux sous Linux | notes, notes_auto, receipt, import_ofx, auto_fields, scheduler, scheduler_add, scheduler_auto, previsions, test | `os.path.join`, `os.path.basename` |
| `str(c.__class__)` contenant « Edit » | Dépend du wrapper | currencies | Fournir `ClassName`, ou garder des noms de classes compatibles |

### 4.2 Données héritées : les pickles Python 2

C'est le point le plus délicat, car il touche les **fichiers `.bp` des utilisateurs**.

- **Ce qui existe**
  - `bp_script_data_fabiobp*` dans le `.bp` : pickles protocole 0, en texte. Ils contiennent des `str` Py2 (octets CP1252), des tuples (dates `(a, m, j)`), des dicts, des listes, des entiers et des flottants.
  - Fichiers `.dat` hors du `.bp` : `notes.dat` et `receipt.dat` (protocole 0), `scheduler.dat` (protocole 1, binaire), `Auto Fields\*.dat` ; exports `.bpsched` et `.bprules`.
- **Lecture en Py3** : `pickle.loads(s.encode("latin-1"), encoding="cp1252")`.
  - La chaîne transmise par l'hôte doit être reconstituée **octet pour octet**, d'où l'aller-retour en latin-1.
  - `encoding="cp1252"` décode les `str` Py2 avec les bons accents et le `€`. En cas d'échec sur un octet non défini (0x81, 0x8D…), réessayer avec `encoding="latin-1"`.
  - L'hôte Rust doit donc restituer la valeur **décodée exactement** (`ObjectScriptsData.pas`) :
    - à l'écriture, `SimpleEncode` échappe en `_xx` hexadécimal les caractères de contrôle (dont les `\n` du pickle), `_`, `#`, `$`, `@` et d'autres, ligne par ligne ;
    - à la lecture, `SimpleDecode` fait l'inverse ;
    - la clé passe par `SimpleKey`, qui normalise la chaîne et supprime les espaces ;
    - la ligne stockée est `bp_script_data_<clé>=<valeur encodée>`.

    Ce codage est à reproduire à l'identique pour relire et réécrire les anciens `.bp`.
- **Écriture** : recommandation, passer à **JSON** pour les nouvelles sauvegardes, via un module utilitaire commun, par exemple `bpcompat.load_data(clé, défaut)` / `save_data(clé, valeur)`, qui :
  1. tente le JSON ;
  2. sinon tente le pickle Py2 (lecture seule) ;
  3. réécrit en JSON.
  - Attention : JSON transforme les **tuples en listes**. Or les scripts comparent `record["nextdate"] <= until` avec un tuple, ce qui lève une TypeError entre list et tuple en Py3. Il faut reconvertir les dates en tuples au chargement, ou les stocker en ISO `AAAA-MM-JJ`.
  - Les clés de dict entières (budget : `{id_categ: montant}`) deviennent des chaînes en JSON : il faut les reconvertir.
  - Si le format JSON est retenu, prévoir la compatibilité descendante : un `.bp` réécrit ne sera plus lisible par l'ancienne version 7.x. Une solution est d'écrire sous une nouvelle clé (`fabiobpscheduler_v2`).

### 4.3 Chaînes et encodage côté hôte

- En Py2, les chaînes échangées avec Delphi sont des `str` CP1252 (octets). En Py3, ce sont des `str` Unicode. Le module `BP` du nouvel hôte doit renvoyer et accepter des `str` Unicode, en convertissant depuis le format interne (UTF-8 côté Rust ; CP1252 pour les anciens `.bp`).
- Pour tous les fichiers produits (CSV, QIF, SLK, HTML, XML), choisir un encodage explicite et cohérent avec la déclaration du fichier : HTML `iso-8859-1` doit devenir UTF-8 avec meta UTF-8 ; le SYLK doit rester en CP1252.
- Les fichiers lus (QIF, CSV, OFX) sont surtout en CP1252 : proposer une détection (en-tête OFX `CHARSET`, BOM, ou `chardet`), avec un repli sur CP1252.

---

## 5. Dépendances propres à Windows et recommandations multiplateformes

| Dépendance actuelle | Où | Recommandation |
|---|---|---|
| `BP.ShellExecute("open", fichier, "", 1)` (ouvrir avec l'application par défaut) | 12 scripts (exports, PDF, graphiques…) | Implémenter `ShellExecute` côté Rust avec le crate `opener` (ou `open`), qui lance `xdg-open`, `open` ou `start` selon le système. Garder la signature ; ignorer `params` et `show` |
| Module `nt` (`lstat`, `listdir`, `rename`, `unlink`, `rmdir`) | backups, scheduler, manage_plugins | `os`, `shutil`, `pathlib` |
| `nt.popen3` qui exécute un setup `.exe` téléchargé | manage_plugins | Supprimer. Nouveau format de plugin : archive `.zip` (`.ini` + `.py`) décompressée dans le dossier de scripts de l'utilisateur, avec signature ou somme de contrôle |
| Chemins `BankPerfectExePath() + "Scripts\\…"` et **écriture dans le dossier du programme** (`notes.dat`, `receipt.dat`, `recu.pdf`, `Auto Fields\*.dat`) | notes, notes_auto, receipt, auto_fields, scheduler (secours), import_ofx, test | Distinguer le dossier des scripts (lecture seule) du dossier de données de l'utilisateur (`dirs::data_dir()/bankperfect`). Exposer une commande `BPEval("user_data_dir")`, ou faire pointer `appdata_dir` vers ce dossier. Utiliser `os.path.join` |
| Export « tableur » = SYLK nommé `.xls` (5 scripts, à partir de modèles `.slk`) | export_xls, totals, categs_totals, people_totals, budget, loan | **Option A (recommandée)** : générer du **`.xlsx`**, soit par une commande hôte (`BPEval("xlsx:…")`, ou une fonction `BP.WriteXlsx(chemin, lignes, formats)` adossée au crate `rust_xlsxwriter`), soit par `openpyxl` embarqué. **Option B (minimale)** : garder le SYLK, l'écrire en CP1252, et le **renommer en `.slk`** pour supprimer l'avertissement d'Excel ; LibreOffice et Excel l'ouvrent. Proposer aussi le CSV |
| Mini-langage PDF `pdf:` (`Create`, `FontName`, `FontSize`, `TextOut`, `TextRect`, `DrawLine`, `DrawImage`, `FillRect`, `DrawRectangle`, `PenColor`, `CurrY`) + `convert_to_bitmap:` | receipt, rib | Réimplémenter l'interpréteur côté Rust (`printpdf` ou `genpdf`, polices Helvetica et Times standard 14). `DrawImage` doit accepter le PNG directement ; `convert_to_bitmap` devient alors une copie sans effet. Autre voie : passer ces deux scripts à un rendu HTML imprimé, mais le PDF reste plus simple pour l'utilisateur |
| `ExportChart` (PDF), `CreateChart`/`DrawChart`/`DestroyChart` (TeeChart ou AACharts) | charts, loan, test | Composant de graphiques côté hôte (par exemple `plotters`), qui garde la convention des séries `"/s=…"`, les 7 types de graphiques, le 3D (peut devenir sans effet) et la légende |
| `BPEval("play:C:\Windows\Media\Tada.wav|1")` | test | Sans objet (script de test). Si la commande `play:` est conservée, utiliser `rodio` |
| Polices « Tahoma », « Arial », « Symbol » (48 occurrences de Tahoma) | presque tous | L'hôte remplace les polices absentes par la police système. Calc : remplacer les glyphes de la police Symbol par des caractères Unicode (`←`, `√`, `π`) |
| IHM VCL (`CreateComponent("TForm"…)`, `TDrawGrid` + `OnDrawCell`, `TDateTimePicker.Date` en numéro de série Delphi) | 35 scripts | Point le plus lourd côté hôte (documenté par l'agent API). Deux stratégies : (1) **une couche de compatibilité VCL** au-dessus du toolkit Rust (même API `CreateComponent`/`SetProps`), qui permet une conversion des scripts quasi mécanique ; (2) réécrire les fenêtres dans une API déclarative nouvelle. Recommandation : (1) pour la première version, au minimum pour `TForm`, `TButton`, `TLabel`, `TEdit`, `TComboBox`, `TCheckBox`, `TListBox`, `TMemo`, `TDrawGrid`, `TDateTimePicker`, `TGroupBox`, `TPanel`, `TRadioButton` ; avec des coercitions souples (entiers et flottants, chaînes « True »/« false », noms d'événements insensibles à la casse) |
| `MsgBox` avec les flags Win32 (0, 36, 64, 68…) et le retour `IDYES = 6` | 32 scripts | Garder les constantes Win32 dans le module (`MB_YESNO=4`, `MB_ICONQUESTION=32`, `MB_ICONINFORMATION=64`, `IDYES=6`, `IDNO=7`) et les traduire vers la boîte de dialogue native |
| Numéro de série de date Delphi (`TDateTimePicker.Date`, `to_delphi_date`, `from_delphi_date` recopiés dans 6 scripts ; dates SYLK) | modifygroup, transfer, budget, loan, scheduler, export_xls | Garder la sémantique (jours depuis le 30/12/1899) dans le wrapper, **ou** exposer `.Date` en `datetime.date` et fournir un utilitaire `bpcompat.delphi_to_date`. Factoriser ces fonctions recopiées dans un module commun |
| FTP en clair (`ftp_lst`, `ftp_get` ; `ftp_put` dans le Delphi) | backups | Déprécier. Proposer une sauvegarde vers un dossier (synchronisé par Nextcloud ou Dropbox), voire SFTP ou WebDAV |
| `send_mail:` avec un serveur et un expéditeur codés en dur (`webmaster@chelly.net`) | backup_mail | Rendre le serveur SMTP configurable (crate `lettre`), ou ouvrir le client de messagerie avec la pièce jointe (portable : `xdg-email --attach` sous Linux) |
| Services web disparus : xe.com, MSN Money, chelly.net (wiki, `plugins.php`) | currencies, update_stock, manuel, manage_plugins | Devises : flux BCE `https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml`. Cotations : fonction à repenser (API publique, ou désactivation). Manuel : manuel local (le `manuel.html` existe déjà) ou lien vers la documentation. Plugins : dépôt Git ou statique |
| `BankPerfectPluginPath().split("|")[1]` pour trouver le `.slk` voisin | 6 scripts | Garder la fonction, ou exposer `__file__` correctement dans les scripts (plus simple en Py3) |

---

## 6. Vérification croisée avec `manuel.html` et `plugins_bankperfect.htm`

### 6.1 Fonctions du manuel assurées par un script

| Section du manuel | Script |
|---|---|
| Imprimer un RIB | Rib ✔ |
| Effectuer un transfert de compte à compte | Transfer/transfer.py ✔. Le lien entre lignes (icône dans le champ tiers) repose sur `#sisterAccUid` et `#sisterLineUid` |
| Les filtres prédéfinis (dates dynamiques, méthode « petite flèche ») | Filters ✔, avec le protocole DropDown |
| Modification d'un groupe d'opérations | ModifyGroup ✔ |
| La fonction de remplacement de texte | Replace ✔. Le manuel indique « Opérations > Remplacer », alors que l'`.ini` place le script dans le menu `edition` |
| Restaurer une sauvegarde (locale ou en ligne) | Backups ✔ |
| Les graphiques (+ Imprimer) | Charts ✔ |
| Les totaux par catégories | CategsTotals ✔. **Écart** : le manuel décrit un export **HTML** (icône globe) en plus de l'export XLS ; le script actuel n'a **que** l'export tableur |
| L'échéancier (ajout, modification, suppression, application manuelle, insertion à l'ouverture) | Scheduler, scheduler_auto ✔ |
| Ajouter une opération existante à l'échéancier | scheduler_add ✔ |
| Gestion des champs supplémentaires (F3) | ExtraValues ✔ |
| Exporter un compte (HTML, QIF, XLS, CSV, XML) | les 5 scripts Export ✔ |
| Importer un fichier QIF | Import QIF ✔ |
| Importer un fichier texte (CSV) | Import CSV ✔ |
| Les calculs d'emprunt (calcul direct, profils `loan.dat`, décalage de la date de départ) | Loan ✔. **Écart** : le manuel décrit un « calcul par variations » avec un **curseur** ; le script utilise des boutons +/− |
| Conversion des autres monnaies (en ligne) | Currencies ✔ (le service est mort) |
| Impression d'un reçu (« Outils > Rapports > Reçu ») | Receipt ✔ (il est dans `outils`, sans sous-menu « Rapports ») |
| Ventiler une opération (F6) | Ventilate ✔. **Écart** : le manuel décrit un « mode de ventilation » intégré à l'interface principale (champs catégorie et montant seuls actifs) ; le script utilise une fenêtre séparée. La fonction est la même |
| Saisir un titre / Mettre à jour les cotations | UpdateStock ✔. Le manuel cite Yahoo Finance, le script MSN Money : les deux ont disparu |

### 6.2 Fonctions présentes dans le manuel mais assurées par l'application Delphi (hors scripts)

À inventorier côté hôte, et non ici : enregistrer sur un disque externe ; « Sauvegarder en ligne » par FTP (`unitMain.dfm`, élément de menu « Sauvegarder en ligne ») ; inverser les monnaies ; date de pointage (F11) ; gestion des catégories et des tiers ; copier/coller de lignes ; modes de paiement supplémentaires ; options et styles.

**Raccourci F9** : le manuel l'attribue à « Internet > Sauvegarder en ligne », mais `backup_mail.ini` déclare aussi `Shortcut=F9`. Le conflit est à trancher dans la nouvelle version.

### 6.3 Fonctions du manuel introuvables (ni script, ni Delphi)

- **« Conversion de monnaies de la zone €uro »** (taux fixes FRF, DEM, ITL…, Ctrl+D). Aucun script ni code Delphi ne l'implémente aujourd'hui, et Ctrl+D est attribué au convertisseur en ligne Currencies. La fonction semble avoir été retirée ou fusionnée. Si on veut la reprendre, les taux de conversion fixes de l'euro sont publics et peuvent remplacer les cours téléchargés pour ces devises.

### 6.4 Scripts sans section dans le manuel

relevé multicompte, Budget et Budget annuel (le manuel ne les mentionne que dans « les filtres sont utilisés dans le tableau de budget »), Calcul automatique des champs, Sauvegarde par e-mail, Calculatrice, Totaux par tiers, Total des comptes, Saisie multiple, Notes, Conversions numériques, Luhn, Convertir la ligne en transfert, Import OFX, Gestion des plugins, Télécharger le manuel, Solde prévisionnel. Il faudra les documenter dans le nouveau manuel.

### 6.5 Vérification avec `plugins_bankperfect.htm`

- La doc ne décrit, pour le `.ini` des scripts, que `Name`, `Path`, `Icon`, `Menu`, `NeedsFile`, `Index` et `DisableScripts`. Les clés `Shortcut`, `ReadOnly`, `DropDown`, `ExecuteWithFile`, `ExecuteWithNewLine` et `ExecuteWithModifiedLine` sont **utilisées mais non documentées** (voir 0.1). La liste des menus de la doc ne contient pas `toolbar` ni `?`, pourtant employés.
- Commandes `GetURL` documentées : page web, `file:`, `ftp_put`/`ftp_get`/`ftp_lst`, `bp_save_folder`, `bp_backup_folder`, `bp_export_folder`, `ini_read`/`ini_write`, `pdf:`, `set_filter`, `simulate_set_filter`, `get_accounts`, `copy_file`, `extra_get`/`extra_set`.
- Commandes **utilisées par les scripts mais absentes de la doc** : `load_script_data`, `save_script_data`, `get_script_parameters`, `set_script_parameters`, `get_lines_changed`, `get_account_uid_from_id`, `mode_count`, `mode_sign`, `color`, `format_date`, `convert_to_bitmap`, `send_mail`, `appdata_dir`, `desktop_dir`, `temp_dir`, `play`, `draw_mark`. Il faut les spécifier dans la documentation de l'API du nouvel hôte.
- `DrawChart` est documentée avec 6 paramètres, mais charts.py et loan.py en passent 7. `ExportChart` est documentée avec 1 paramètre, alors que charts.py passe le chemin du PDF et teste le retour (`None` en cas d'échec).
- `simulate_set_filter` est documentée comme renvoyant une liste séparée par `;`. En pratique, les scripts découpent d'abord sur `|` (une liste par compte), puis sur `;`.
- `copy_file` est documentée avec `;` comme séparateur, mais `backups.py` utilise `|`. Il faut vérifier l'implémentation de l'hôte et accepter les deux.

---

## 7. Bogues existants repérés pendant la lecture (à corriger lors du portage)

| Script | Bogue |
|---|---|
| luhn | `(int(c)*2) % 9` donne un résultat faux pour le chiffre 9 (doit valoir 9, pas 0) |
| export_xls | `{count-1}` jamais remplacé (parenthèse mal placée) |
| all_accounts_view | Totaux « Pointé » et « A venir » inversés ; nombre de colonnes incohérent |
| categs_totals | `min()` sur une liste vide si un compte n'a pas d'opération |
| import_ofx | Libellé « une ligne » jamais affecté |
| import_csv | `GetCtg` renvoie une position au lieu d'un id dans une branche |
| loan | `rates.append` au lieu de `Rates` |
| scheduler | Filtre « lignes non automatiques » sur une clé inexistante (`auto_insert`) |
| receipt | Les données sont sauvegardées même après une annulation |
| convert_line_to_transfer | `extra_set` exécuté même si `LineAdd` a échoué |
| transfer | `print` de débogage |
| test | Rangé dans le menu « internet » |

---

## 8. Recommandations de priorisation

1. **API minimale pour faire tourner 80 % des scripts**, dans cet ordre :
   - `MsgBox`, `AccountCurrent`, les propriétés `Operation*` en proxies lecture et écriture, `CategName`/`CategParent`, `LineAdd`, `VisibleLines`, `AccountRefreshScreen`, `AccountName`/`AccountCount`/`AccountIsVirtual` ;
   - `BPEval` avec `bp_export_folder`, `get_accounts`, `load_script_data`/`save_script_data` ;
   - `CreateComponent` avec la couche VCL de base (y compris `TDrawGrid` + `OnDrawCell`) ;
   - `ShellExecute` (opener) et `OpenDialog`.
2. **Ensuite** : `simulate_set_filter`, `set_filter`, `extra_get`/`extra_set`, `get_account_uid_from_id`, `get_script_parameters`/`set_script_parameters` (DropDown), `get_lines_changed` (déclencheurs automatiques), `LineSelect`, `LineDelete`, `CategAdd`, `InputText`, `TDateTimePicker`, les graphiques, `pdf:`.
3. **Module Python commun `bpcompat`** pour éviter 44 corrections isolées :
   - lecture et écriture des données (JSON, avec repli sur le pickle Py2) ;
   - dates Delphi, `fmt_float` (recopiée dans plus de 10 scripts) ;
   - normalisation sans accents (`Pattern`, recopiée dans 6 scripts) ;
   - `get_accounts` analysé, chargement des catégories (recopié partout) ;
   - devinette du mode de paiement (`getmode`, recopiée 3 fois).
4. **Ordre de conversion conseillé**
   - Faible effort, à faire d'abord : Replace, Ventilate, ModifyGroup, ExtraValues, convert_line_to_transfer, Export QIF, CSV, HTML et XML, Totals, Notes, Luhn, Calc, Numerals.
   - Effort moyen : Import QIF et OFX, Charts, CategsTotals, PeopleTotals, AllAccountsView, Filters, MultipleInput, Transfer, scheduler_add, scheduler_auto, previsions, Rib, Receipt, Loan, Export XLS, Backups, BackupMail.
   - Effort élevé : Scheduler, Budget, BudgetYear, Auto Fields, Import CSV.
   - Fonctions à repenser plutôt qu'à convertir : Currencies, UpdateStock, ManagePlugins, Manuel, test.
5. **Tests de non-régression indispensables** :
   - calcul des dates (Pâques, jours fériés, `add_months` en fin de mois, séries Delphi) ;
   - insertion automatique des échéances ;
   - relecture des pickles Py2 présents dans de vrais fichiers `.bp` (avec des copies anonymisées).
