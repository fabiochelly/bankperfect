# Inventaire exhaustif des fonctionnalités du cœur Delphi de BankPerfect

> Périmètre : exécutable Delphi `bankperfect.exe` (build 8.0.0.370, `history.txt`), **hors** scripts Python
> (`Scripts/`) et **hors** API Python/plugins (`UnitPlugins.pas`, `dmPython.pas`, `FormPython.pas`,
> `pyAccounts.pas`, `pyRun.pas`, `IBankPerfect.pas`, `UnitCustomForms.pas`), qui font l'objet d'autres inventaires.
> Les points de contact cœur ↔ scripts (chargement des scripts, menus, déclencheurs automatiques) sont
> néanmoins listés au §26 pour ne rien perdre.
>
> Le format binaire/texte du fichier `.bp` est spécifié dans `02-format-fichier-bp.md` ; ce document n'en
> reprend que ce qui est nécessaire à la compréhension des comportements.
>
> Toutes les références `fichier:ligne` pointent vers les sources de `/home/fabio/Work/bankperfect`
> (lignes des fichiers convertis CP1252 → UTF-8, identiques en numérotation).

## Légende

- `- [ ]` : fonctionnalité à reproduire (case à cocher pour le suivi de parité).
- **[Q]** : comportement surprenant / bug d'origine. À reproduire tel quel *ou* à arbitrer explicitement
  (décision à tracer) — la parité « iso » exige au minimum de ne perdre aucune donnée.
- **[W]** : dépendance spécifique Windows ; un équivalent multiplateforme est proposé au §32.
- **[S]** : la fonctionnalité visible par l'utilisateur est en réalité réalisée par un script Python ;
  seul le point d'accroche du cœur est décrit ici.
- Identifiants `XXX-nn` : référence stable pour le suivi.
- Raccourcis : notation Delphi `ShortCut` décodée (`$4000` = Ctrl, `$2000` = Maj, `$8000` = Alt).

---

## 1. Démarrage, chemins et environnement

- [ ] **START-01 — Répertoire de l'exécutable** : `PathExe := ExtractFilePath(ParamStr(0))` ; `Today := Trunc(Date)` figé au démarrage (utilisé pour la coloration « date future » et la date de pointage) — `unitMain.pas:1186-1187`. **[Q]** `Today` n'est jamais rafraîchi si l'application reste ouverte après minuit.
- [ ] **START-02 — Mode portable** : si un fichier nommé `portable` (sans extension, contenu indifférent) existe dans `PathExe`, alors `PathDocuments = PathAppData = PathExe` — `unitMain.pas:1193-1196`.
- [ ] **START-03 — Mode installé** : `PathDocuments` = dossier « Mes documents » (`CSIDL_PERSONAL`), `PathAppData` = `%APPDATA%\BankPerfect\` (créé si absent) — `unitMain.pas:1197-1201`, `GetSpecialFolder` `unitMain.pas:1126`. **[W]**
- [ ] **START-04 — Migration des anciens emplacements** (`MovePathExeFiles`, mode installé uniquement) — `unitMain.pas:1140-1177` :
  - tous les `*.bp` de `PathExe` déplacés vers `PathDocuments` ;
  - tous les `*.bp0` de `PathExe` déplacés vers `PathAppData` ;
  - `bankperfect.ini` copié vers `PathAppData` s'il n'y existe pas, puis réécriture de `[SAVE] PathSave` (si égal à `PathExe` → `PathDocuments`) et `[SAVE] PathBackup` (si égal à `PathExe` → `PathAppData`), puis suppression de l'original ;
  - `localcolors.ini` et `loan.dat` déplacés vers `PathAppData` s'ils n'y existent pas.
- [ ] **START-05 — Fichier INI principal** : `PathAppData + 'bankperfect.ini'`, chargé en mémoire (`TMemIniFile`) ; **toutes les écritures restent en mémoire** et ne sont persistées que par `Ini.UpdateFile` à la fermeture (`FormCloseQuery`) ou à la saisie d'une licence — `unitMain.pas:1224`, `1789-1793`, `5408`. Clés insensibles à la casse. **[Q]** un plantage perd les réglages de la session.
- [ ] **START-06 — Conversion de l'ancien `bp.ini`** (`ConvertIniFile`) : si `PathExe\bp.ini` existe, chaque `clé=valeur` est recopiée dans la bonne section de `bankperfect.ini` (true/false → 1/0), puis `bp.ini` est supprimé — `unitMain.pas:1016-1064`. Table de routage :
  - `WINDOW` : col0…col7, top, left, height, width, maximized, scaled ;
  - `COLORS` : toute clé commençant par `clr` ;
  - `FILTER` : clés commençant par `filter` + ignorefilter, hideoldlines, rememberfilters, sortcol, sortascending, remembersortcol ;
  - `LAYOUT` : disabler, disableautocomplete, dateformat, maskhints, maskvaluehints, printfont, masktotalhints, singlevaluecol, showdetails, defaultfont, separateinput ;
  - `APPLICATION` : precision, maxundo, licence, nolog, noregistry ;
  - `LOAN` : clés commençant par `loan` (préfixe `loan` retiré) ;
  - `HTTP` : smtpserver ; `FTP` : ftpsave, ftppassword, ftpurl, ftplogin, ftpfolders, lastonlinebackupnumber ;
  - `SAVE` : autosave, lastbackupnumber, pathsave, pathbackup ; autres clés ignorées.
  - **[Q]** le test est `Pos(clé, 'liste…') > 0` : toute clé qui est une sous-chaîne de la liste est routée (ex. `col`).
- [ ] **START-07 — Séparateur décimal** : si le séparateur décimal système n'est ni `.` ni `,`, il est forcé à `.` ; `Sep1`/`Sep2` déterminent la conversion à la frappe (touche `Sep2` remplacée par `Sep1`) — `unitMain.pas:1228-1231`. **[Q]** avec séparateur système `.`, `Sep1 := ','` et `Sep2` reste `#0` (non initialisé) : la conversion ne fonctionne qu'en locale française (`,`).
- [ ] **START-08 — Paramètres régionaux forcés** (`SetParameters`) : `DateSeparator='/'`, `ShortDateFormat='dd/mm/yy'`, `LongDateFormat='dddd, dd mmm yyyy'`, `ThousandSeparator=' '` (espace) — `unitMain.pas:4556-4562`. Les noms de jours/mois (`ddd`, `mmmm`) restent ceux de la locale Windows (français attendu). **[W]**
- [ ] **START-09 — Valeurs initiales** : `MarkFilter=4` (aucun filtre de pointage), largeurs de colonnes par défaut `ColsW = [111,20,35,18,25,25]`, `DefaultDate = 28/06/1973`, couleurs lues depuis `[COLORS]` — `unitMain.pas:1204-1273`.
- [ ] **START-10 — Fenêtre** : taille mini 640×480, centrée (`poScreenCenter`) ; restauration `[WINDOW] maximized` ou `Left/Top/Width/Height` (valeurs ≤ 0 ignorées) — `unitMain.dfm:11-20`, `unitMain.pas:1302-1314`.
- [ ] **START-11 — Ligne de commande** :
  - `ParamStr(1)` : si le fichier existe, il est ouvert au démarrage — `unitMain.pas:1321`, `1329` ;
  - `ParamStr(2)` : si extension `.py`, le script est exécuté au premier affichage de la fenêtre (`FormShow`) — `unitMain.pas:7088-7093`, `6457-6476`. **[S]**
- [ ] **START-12 — Licence lue au démarrage** depuis `[APPLICATION] Licence` (cf. §18) — `unitMain.pas:1324-1326`.
- [ ] **START-13 — Fichier d'identité obligatoire** : si `PathExe\identity.dat` est absent → message « Le fichier identity.dat est introuvable. Ce fichier est nécessaire… » et l'application **ne démarre pas** ; si `BPDefaultCaption` est vide après décodage → « Le fichier identity.dat est corrompu… » et arrêt — `bankperfect.dpr` (bloc principal), `UnitRStrings.pas:156-158`. Cf. §18.
- [ ] **START-14 — Chargements au démarrage** : plugins DLL (§26), scripts (§26), icônes libres `PathExe\Icons\**\*.png` (§25), raccourci secondaire `Ctrl+Pavé numérique 0` pour « Vider la zone de saisie » ajouté par code — `unitMain.pas:1245-1247`, `1330`.
- [ ] **START-15 — Champ « solde prévisionnel »** initialisé à aujourd'hui + 12 mois — `unitMain.pas:1338`.
- [ ] **START-16 — Libération** : `FormDestroy` libère tout ; toute exception pendant la libération → `Halt(0)` — `unitMain.pas:7373-7406`.

---

## 2. Fenêtre principale — structure générale

- [ ] **UI-01 — Titre de fenêtre** (`UpdateFormCaption`) — `unitMain.pas:986-1007` :
  - non enregistré ou identité sans `BPRegisteredCaption` → `BPDefaultCaption` (« BankPerfect (NON ENREGISTRE) ») ;
  - enregistré → `BPRegisteredCaption + Registered` (« BankPerfect - Licence n°1234 - Nom ») ; licence Gold → « Licence n° » remplacé par « Licence Gold n° » ;
  - si un fichier est ouvert : ` [chemin complet du .bp]` ajouté ;
  - visibilité : bandeau licence (§UI-03) si pas de fichier ouvert et non enregistré ; menu « S'enregistrer » si non enregistré ; menus « Entrer son n° de licence » et « Mettre à jour BankPerfect » cachés si `BPRegisteredCaption` vide.
  - `gridClick` rappelle `UpdateFormCaption` si le titre ne commence plus par « BankPerfect » (cas où un script l'a modifié) — `unitMain.pas:1634`.
- [ ] **UI-02 — Écran d'accueil (aucun fichier ouvert)** : panneau défilant centré (`BaseFiles`, ≥ 265×240, 1/3 de la fenêtre) contenant des libellés cliquables (rouge au survol) avec icône — `unitMain.pas:840-872`, `4216-4230`, `6707-6716` :
  1. « Créer un nouveau fichier comptes » → `ANewFile` ;
  2. « Ouvrir un fichier » → `AOpenFile` ;
  3. ligne vide ;
  4. un libellé par fichier `*.bp` trouvé dans `[SAVE] PathSave` (ordre `FindFirst`, i.e. alphabétique NTFS) → ouvre `PathSave + nom`.
  - Il ne s'agit **pas** d'une liste de fichiers récents (aucune MRU dans le cœur).
- [ ] **UI-03 — Bandeau licence** (`PanelLicence`) : « Si vous appréciez ce logiciel et souhaitez m'aider, vous pouvez vous enregistrer : cliquez ici pour en savoir plus. » ; clic → URL d'enregistrement ; texte rouge foncé au survol — `unitMain.dfm:54-90`, `unitMain.pas:5414-5422`.
- [ ] **UI-04 — Activation/désactivation de l'interface** (`EnableUI`) selon qu'un fichier est ouvert : grille, filtres, panneau de saisie, panneau des soldes, zone sensible, bouton devise, bouton comptes, et les actions Enregistrer sous, Mot de passe, Pointer, Supprimer, Imprimer, Catégories, Saisir, Copier, Inverser monnaies, FTP, Gestion comptes, Fermer, Disque externe, Importer, Exporter, Tiers, Modes, Rapprocher tout, Ajouter aux tiers, Couper, Vider saisie, Cacher lignes, 2 lignes, Charger tiers, Date de pointage, Afficher dates de pointage, + tous les éléments de script marqués `NeedsFile` — `unitMain.pas:1661-1731`. Sans fichier, le bouton comptes affiche « Compte ».
- [ ] **UI-05 — Mise en page dynamique** (`AdjustForm`) — `unitMain.pas:6672-6828` :
  - marges : haut 10 (+ hauteur barre d'outils), bas 25, gauche 11 ; panneau des soldes 160 px de large ; séparateur 7 px ; zone de saisie intégrée 41 px de haut ;
  - panneau des soldes visible → grille à droite du panneau ; sinon grille pleine largeur ;
  - zone de saisie intégrée sous la grille, contrôles alignés sur les colonnes ; en mode fenêtre séparée, panneau flottant 450×360 centré sur la grille ;
  - recalcul des largeurs de colonnes (§GRID-20), position des filtres dans l'en-tête, du panneau de filtre de dates et du panneau de filtre de pointage.
- [ ] **UI-06 — Barre d'outils** (`ToolBar1`, boutons 51×42 avec libellés, police 7 pt) — `unitMain.dfm:896-1013` :
  1. **Comptes** (liste déroulante) : clic principal = compte suivant (comme Ctrl+Tab), flèche = menu des comptes actifs dans l'ordre d'affichage ; libellé = nom du compte courant tronqué avec « … » à la largeur du plus large bouton − 5 px ; infobulle = nom complet — `unitMain.pas:3162-3164`, `7034-7062`, `7064-7086`, `7166-7189` ;
  2. **Saisir** (bouton bascule lié à `AInput`, visible seulement en mode saisie séparée) ;
  3. **Devise** (bouton bascule, libellé = symbole de la monnaie affichée, infobulle « Changer de monnaie ») — §15 ;
  4. séparateur ; **Comptes** (gestion des comptes, `AAccounts`) ; **Ouvrir** ; **Enregistrer** ; séparateur ; **Annuler** ; **Refaire** ; **Verrou** (`ALockLines`, icône 34/35 selon état) ; séparateur ;
  5. puis les boutons de scripts déclarés `toolbar` (§26).
- [ ] **UI-07 — Barre d'état** (`VersionNumber`, libellé en bas à droite) — `unitMain.pas:5746-5763` :
  - sans fichier : numéro de version de l'exécutable ;
  - avec fichier et panneau des soldes visible : « N lignes » + « - Compte rapproché au jj/mm/aa » (date max des lignes `R`) ; message temporaire « Fichier X enregistré. » (4 s) après sauvegarde ;
  - avec panneau des soldes masqué : « Pointé: X      A venir: Y      Solde: Z » ; l'infobulle de la barre d'état contient le total de la sélection (§BAL-07).
- [ ] **UI-08 — Zone sensible** (image 8×49 px au bord gauche, centrée verticalement) : clic = afficher/masquer le panneau des soldes ; l'image change selon l'état (`PngImageCollection1` items 15/16) — `unitMain.pas:7217-7228`, `1694-1696`.
- [ ] **UI-09 — Clic n'importe où** ferme l'état « menu du bouton Ajouter ouvert » — `unitMain.pas:7146-7159`.

---

## 3. Menus, actions et raccourcis clavier

Source : `unitMain.dfm:1104-1463` (ActionList), `1533-1768` (MainMenu), `1769-1797` (menu contextuel), `5026-5038` (menu du bouton Ajouter).

### 3.1 Menu « Fichier »

- [ ] **MENU-F01 — Ouvrir** (`AOpenFile`, **Ctrl+O**) : boîte « Sélectionnez un fichier BankPerfect », filtres « Comptes bankperfect (.bp)|*.bp|Sauvegardes (.bp0)|*.bp0 » ; ferme le fichier courant (avec proposition de sauvegarde) puis charge — `unitMain.pas:2494-2506`.
- [ ] **MENU-F02 — Fermer** (`ACloseGroup`) — `unitMain.pas:4269-4272`, §16.
- [ ] **MENU-F03 — Enregistrer** (`ASave`, **Ctrl+S**) — §16. Libellé temporaire « En cours... » pendant l'écriture.
- [ ] **MENU-F04 — Enregistrer sous...** (`ASaveAs`, **F12**) : « Enregistrer le compte... », filtre « Comptes bancaires (.bp)|*.bp », ajoute `.bp` si absent, devient le fichier courant — `unitMain.pas:2636-2651`.
- [ ] **MENU-F05 — Enreg. sur disque externe** (`SaveOnFloppy`) — §16.9.
- [ ] **MENU-F06 — Importer ▸** (sous-menu vide dans le cœur, rempli par les scripts `importer`) **[S]**.
- [ ] **MENU-F07 — Exporter ▸** (idem, scripts `exporter`) **[S]**.
- [ ] **MENU-F08 — Imprimer...** (`APrint`, **Ctrl+P**) — §22.
- [ ] **MENU-F09 — Quitter** (`AQuit`) — ferme la fenêtre (déclenche §16.4).
- [ ] Scripts `fichier` insérés à leur `Index` (ex. « Imprimer un rib ») **[S]**.

### 3.2 Menu « Edition »

- [ ] **MENU-E01 — Annuler [action]** (`AUndo`, **Ctrl+Z**) ; libellé dynamique « Annuler <libellé> » — §7.1.
- [ ] **MENU-E02 — Refaire [action]** (`ARedo`, **Ctrl+Y**) — §7.1.
- [ ] **MENU-E03 — Saisir** (`AInput`, **F8**) : visible uniquement en mode saisie séparée ; en mode intégré la touche F8 reste active et donne le focus au champ date — `unitMain.pas:5730-5738`, `5726`.
- [ ] **MENU-E04 — Vider la zone de saisie** (`AResetAllFields`, **Ctrl+0** et **Ctrl+Pavé 0**) : date du jour, mode « Carte », tiers/détails/catégorie/montant vidés, focus sur le mode — `unitMain.pas:5384-5394`, `1330`.
- [ ] **MENU-E05 — Supprimer** (`ADeleteLine`, **Ctrl+Suppr**) — §7.2.
- [ ] **MENU-E06 — Couper** (`ACutLines`) ; **Copier** (`ACopyLines`) ; **Coller** (`APasteLines`, désactivé tant qu'aucune copie) — §7.3. Ctrl+X/C/V gérés sur la grille (§GRID-17).
- [ ] Éléments de scripts `edition` (ex. « Remplacer » Ctrl+R) + éléments invisibles portant les raccourcis des menus déroulants de scripts **[S]**.

### 3.3 Menu « Affichage »

- [ ] **MENU-A01 — Affichage sur 2 lignes** (`AShowDetails`, **F4** porté par l'élément de menu) : bascule, persiste `[LAYOUT] ShowDetails` — `unitMain.pas:5973-5979`, §GRID-05.
- [ ] **MENU-A02 — Afficher les dates de pointage** (**F11**) : bascule de session (non persistée) — `unitMain.pas:6001-6006`, §6.6.
- [ ] **MENU-A03 — Afficher le panneau des soldes** : bascule, persiste `[LAYOUT] ShowTotalPanels` — `unitMain.pas:6830-6840`.
- [ ] **MENU-A04 — Afficher le solde prévisionnel** : bascule, persiste `[LAYOUT] ShowPrevisions` — `unitMain.pas:7419-7433`.
- [ ] **MENU-A05 — Cacher les lignes inutiles** (`AHideOldLines`, **Ctrl+H**) : bascule, persiste `[FILTER] HideOldLines` — `unitMain.pas:5788-5799`, §9.8.
- [ ] Scripts `affichage` (ex. « Filtres automatiques » F7) **[S]**.

### 3.4 Menu « Comptes »

- [ ] **MENU-C01 — Créer un nouveau fichier de comptes** (`ANewFile`, **Ctrl+N**, infobulle « Crée un fichier BP qui pourra contenir plusieurs comptes (un groupe de comptes) ») — §16.1.
- [ ] **MENU-C02 — Supprimer un fichier de comptes** (`ADeleteFile`) — §16.10.
- [ ] **MENU-C03 — Gestion des comptes** (`AAccounts`, **Ctrl+G**) — §11 ; au retour, la grille est recalculée — `unitMain.pas:2728-2732`.
- [ ] **MENU-C04 — Mot de passe** (`ASetPassword`) — §17.
- [ ] **MENU-C05 — Modes de paiement** (`AModes`) — §14.
- [ ] **MENU-C06 — Gestion des catégories** (`ACategsForm`) — §12.
- [ ] **MENU-C07 — Gestion des tiers** (`AManagePeople`, **F5**) — §13.
- [ ] Scripts `comptes` (Échéancier Ctrl+E, Liste des sauvegardes, Total des comptes, relevé multi-comptes…) **[S]**.
- [ ] **Raccourcis sans menu** : **Ctrl+Tab** = compte suivant (`ANextAccount`), **Ctrl+Maj+Tab** = compte précédent (`APreviousAccount`) — `unitMain.dfm:1452-1463`, §11.10.

### 3.5 Menu « Opérations »

- [ ] **MENU-O01 — Pointer** (`ATack`) — §6.1.
- [ ] **MENU-O02 — Date de pointage** (`ASetMarkDate`, **Ctrl+Maj+P**) — §6.5.
- [ ] **MENU-O03 — Charger un tiers** (`ALoadPeople`, **F10**) — §13.6.
- [ ] **MENU-O04 — Ajouter aux tiers** (`APeopleAdd`) — §13.7 ; désactivé si plusieurs lignes sélectionnées — `unitMain.pas:5252-5255`.
- [ ] **MENU-O05 — Verrouiller les lignes rapprochées** (`ALockLines`, coché selon état) — §6.4.
- [ ] **MENU-O06 — Rapprocher toutes les lignes pointées** (`ATackRedInBlack`) — §6.3.
- [ ] Scripts `operations` (Transfert de compte à compte Ctrl+Maj+T, Ventiler F6, Champs supplém. F3, Modifier le groupe Ctrl+M, Saisie multiple F2, Ajouter à l'échéancier, Convertir en transfert…) **[S]**.
- [ ] **Raccourcis sans menu visible** : **Ctrl+Entrée** = ajouter l'opération (`AAddOperation`), **Maj+Entrée** = modifier l'opération sélectionnée (`AModifyOperation`) ; `APressOnEnter` (Entrée) déclaré sans gestionnaire — `unitMain.dfm:1351-1367`, `unitMain.pas:4978-4986`.

### 3.6 Menu « Outils »

- [ ] **MENU-T01 — Inverser les monnaies** (`AReverseMoney`) — §15.3.
- [ ] **MENU-T02 — Options** (`AOptionsForm`) — §19.
- [ ] Scripts `outils` (Budget Ctrl+B, Graphiques, Totaux par catégorie Ctrl+T, Simulateur de crédits Ctrl+L, Conversions de devises Ctrl+D, Calculatrice Maj+Ctrl+C…) **[S]**.

### 3.7 Menu « Plugins »

- [ ] **MENU-P01** — rempli dynamiquement par les DLL de `PathExe\Plugins\` et les scripts `plugins` — §26.

### 3.8 Menu « Internet »

- [ ] **MENU-I01 — Sauvegarder par FTP** (`ASaveToFTP` ; **F9** sur l'action, **Maj+F9** sur l'élément de menu) — §16.11. **[Q]** le script « Sauvegarder par e-mail » déclare aussi F9.
- [ ] **MENU-I02 — Site Internet** (`AWebSite`) : ouvre `BPWebsite` (défaut `http://www.bankperfect.net`) — `unitMain.pas:2988-2991`.
- [ ] **MENU-I03 — Mettre à jour BankPerfect** (`AUpdate`) — §30 (abandonné).
- [ ] **MENU-I04 — Contacter le support** (`ASendMail`) : ouvre `BPSupport` (URL ou mailto) — `unitMain.pas:6622-6625`.

### 3.9 Menu « ? »

- [ ] **MENU-H01 — Aide** (`AHelp`, **F1**) : ouvre `PathExe + [APPLICATION] helpfile` (défaut `bp.htm`) s'il existe, sinon `http://www.chelly.net/wiki/wiki/manuel` — `unitMain.pas:2993-3000`.
- [ ] **MENU-H02 — Consulter la FAQ (en ligne)** (`AFAQ`) : visible seulement si identité `BPShowOnlineFAQ=1` — `unitMain.pas:1331`, `6607-6610`.
- [ ] **MENU-H03 — A propos...** (`AAbout`) — §18.4.
- [ ] **MENU-H04 — S'enregistrer** (`ARegisterBankPerfect`) : ouvre `http://www.chelly.net/index.php?q=aaf` — `unitMain.pas:4279-4282`.
- [ ] **MENU-H05 — Entrer son n° de licence** — §18.2.
- [ ] `AOnlineHelp` (« Consulter l'aide en ligne ») : action déclarée sans gestionnaire ni menu (code mort) — `unitMain.dfm:1441-1444`.

### 3.10 Menu contextuel de la grille (clic droit)

- [ ] **MENU-X01** — Supprimer, Pointer, —, Couper, Copier, Coller, —, Ajouter aux tiers, + scripts `context`/`right` — `unitMain.dfm:1769-1797`. N'est attaché à la grille que si elle contient au moins une ligne (`buildGrid`) — `unitMain.pas:1956-1969`. Le clic droit hors sélection resélectionne la ligne cliquée — `unitMain.pas:1914-1930`.

### 3.11 Menu du bouton Ajouter (zone de saisie intégrée)

- [ ] **MENU-B01** — « Ajouter l'opération (Ctrl+Entrée) » / « Modifier l'opération (Maj+Entrée) » ; s'ouvre en cliquant sur la partie droite (flèche) du bouton ; repositionné si débordement écran — `unitMain.pas:7101-7134`, `7191-7215`.

### 3.12 Récapitulatif des raccourcis du cœur

| Raccourci | Action | Réf. |
|---|---|---|
| Ctrl+O | Ouvrir | dfm:1113 |
| Ctrl+S | Enregistrer | dfm:1121 |
| F12 | Enregistrer sous | dfm:1129 |
| Ctrl+P | Imprimer | dfm:1137 |
| Ctrl+N | Nouveau fichier | dfm:1154 |
| Ctrl+G | Gestion des comptes | dfm:1162 |
| Ctrl+Z / Ctrl+Y | Annuler / Refaire | dfm:1177, 1185 |
| Ctrl+Suppr | Supprimer lignes | dfm:1200 |
| F1 | Aide | dfm:1244 |
| F9 (action) / Maj+F9 (menu) | Sauvegarde FTP | dfm:1320, 1730 |
| F5 | Gestion des tiers | dfm:1343 |
| Ctrl+Entrée / Maj+Entrée | Ajouter / Modifier opération | dfm:1354, 1360 |
| Ctrl+0, Ctrl+Pavé0 | Vider la saisie | dfm:1388, pas:1330 |
| F8 | Saisir | dfm:1397 |
| F10 | Charger un tiers | dfm:1414 |
| Ctrl+Maj+P | Date de pointage | dfm:1433 |
| Ctrl+Tab / Ctrl+Maj+Tab | Compte suivant / précédent | dfm:1455, 1461 |
| F4 | Affichage sur 2 lignes | dfm:1622 |
| F11 | Afficher dates de pointage | dfm:1626 |
| Ctrl+H | Cacher lignes inutiles | dfm:1642 |
| Espace, P/p (grille) | Pointer | pas:5071-5076 |
| + ou A majuscule (grille) | Saisir | pas:5075 |
| Ctrl+X / Ctrl+C / Ctrl+V (grille) | Couper / Copier / Coller | pas:5496-5514 |
| Entrée (champs de saisie) | Champ suivant / valider | pas:1985-2013 |
| Échap (champs de saisie) | Sortir catégorie / fermer panneau séparé | pas:2001-2010 |
| PgPréc/PgSuiv, ↑/↓ (date) | ±1 jour | pas:4996-5020 |
| Ctrl+PgPréc/PgSuiv | ±1 mois | pas:5016 |
| Maj+PgPréc/PgSuiv | ligne précédente/suivante de la grille | pas:5009-5015 |
| + (champ date) | +1 jour | pas:5922-5929 |

---

## 4. Grille des opérations

Composant `TDrawGrid` 6 colonnes, sans colonne fixe, 1 ligne d'en-tête, options `goRowSelect, goRangeSelect, goColSizing, goThumbTracking`, défilement vertical seul — `unitMain.dfm:106-133`.

- [ ] **GRID-01 — Pas d'édition en place** : la grille n'a pas `goEditing` ; toute modification passe par la zone de saisie (§5). Sélection de lignes entières, plage contiguë (glisser ou Maj+clic).
- [ ] **GRID-02 — Colonnes et en-têtes** (`intitules_colonnes`) — `unitMain.pas:655`, `3222-3251` :
  | Col | En-tête | Contenu |
  |---|---|---|
  | 0 | « Date » (« Ptge » si dates de pointage affichées) | date d'opération (ou date de pointage) formatée `DateFormat` |
  | 1 | « Mode » | libellé du mode (ex. « Carte », « Chq 1234 ») |
  | 2 | « Tiers » | tiers (+ détail en 2ᵉ ligne si mode 2 lignes) |
  | 3 | « Categ » | « Catégorie » ou « Parent > Enfant » |
  | 4 | « € » | montant (et solde selon `ValueMode`) |
  | 5 | (case de filtre) | coche de pointage |
  **[Q]** l'en-tête « € » est fixe quelle que soit la devise (l'appel `SetCell(5, 0, …)` n'a aucun effet) — `unitMain.pas:3325`.
- [ ] **GRID-03 — Données affichées** : calculées à chaque `AFillGrid` dans des listes `GDates/GModes/GPeoples/GCategs/GValues` ; `LineNumbers[ligne-1]` = index réel de l'opération dans le compte — `unitMain.pas:3275-3547`.
- [ ] **GRID-04 — Hauteur de ligne** : 32 px en mode 2 lignes, 17 px sinon ; en-tête = hauteur d'un champ de saisie + 4 — `unitMain.pas:1009-1014`.
- [ ] **GRID-05 — Mode 2 lignes** (`ShowDetails`) — `unitMain.pas:1545-1561` :
  - colonne Tiers : tiers en haut, détail (`Info`) en bas dans la couleur `clr_details_text` (sauf ligne sélectionnée) ;
  - colonne Catégorie : partie avant « > » en haut, sous-catégorie en bas ;
  - sinon texte centré verticalement sur une ligne.
- [ ] **GRID-06 — Couleurs de fond** : lignes impaires (1, 3, …) `BPColors[0]` (`clr_line1`), paires `BPColors[1]` (`clr_line2`), sélection `BPColors[4]` ; texte respectivement `[2]`, `[3]`, `[5]` ; en-tête `clBtnFace` avec séparateur vertical ombré entre colonnes — `unitMain.pas:1434-1451`, `1531-1534`.
- [ ] **GRID-07 — Alignements** : dates et montants alignés à droite, autres colonnes à gauche ; marge interne 2 px ; texte sur une seule ligne (`DT_SINGLELINE`) ; `&` doublé pour l'affichage — `unitMain.pas:1541-1563`, `UnitStrings.pas:280-288`.
- [ ] **GRID-08 — Montants négatifs** en `BPColors[9]` (rouge par défaut) sauf ligne sélectionnée — `unitMain.pas:1520-1530`.
- [ ] **GRID-09 — Modes d'affichage des montants** (`ValueMode`, option §19) — `unitMain.pas:1514-1591`, `3328-3330` :
  - **1 — Une colonne** : montant signé aligné à droite ;
  - **2 — 2 colonnes débit/crédit** : débit (sans signe) dans la moitié gauche, crédit dans la moitié droite, trait vertical gris `$C0C0C0` au milieu ;
  - **3 — 2 colonnes montant/solde** : montant signé dans la moitié gauche, solde progressif en **gras** dans la moitié droite (rouge si négatif, sinon couleur du texte de la ligne) ;
  - **4 — 3 colonnes débit/crédit/solde** : débit (sans signe) dans le 1ᵉʳ tiers, crédit dans le 2ᵉ tiers, solde en gras dans le 3ᵉ ; deux traits verticaux.
  - Correctif « couleurs inversées » (8.0 beta 3) : en modes 3/4 le crédit d'une ligne sélectionnée est dessiné en `BPColors[5]` pour rester lisible — `unitMain.pas:1514-1519`, `history.txt:10`.
- [ ] **GRID-10 — Solde progressif** (`Cumuls`) — `unitMain.pas:3427-3515` :
  - calculé sur **toutes** les opérations du compte dans l'ordre d'affichage courant (y compris les lignes masquées par les filtres ou « lignes inutiles »), puis seules les valeurs des lignes affichées sont conservées ;
  - tri descendant : cumul de bas en haut ; tri ascendant : de haut en bas ;
  - **[Q]** si le tri n'est pas chronologique (tri par tiers, montant…), le solde progressif suit cet ordre arbitraire ;
  - converti dans la monnaie secondaire si le bouton devise est enfoncé — `unitMain.pas:3246-3248`.
- [ ] **GRID-11 — Coches de pointage (col. 5)** : pointé (`P`, tack=1) = coche couleur `clr_tack1`, rapproché (`R`, tack=2) = coche couleur `clr_tack2` ; si sélectionnée, la coche est entourée d'un cadre noir sur fond de ligne ; si « rapprochement désactivé », une ligne `P` est dessinée comme `R` — `unitMain.pas:1341-1368`, `1593-1604`. Forme exacte de la coche : polyligne de 10 points (`DrawTack`).
- [ ] **GRID-12 — Case de filtre de pointage dans l'en-tête col. 5** : carré blanc bordé `$885522` de 14 px ; 0 = vide, 1 = coche gris clair, 2 = coche noire, 3 = coche rouge, 4 = carré gris foncé plein — `unitMain.pas:1452-1469`, §9.6.
- [ ] **GRID-13 — Flèche de tri** : triangle ombré (couleur d'ombre de `clBtnFace` −50) à droite de l'en-tête de la colonne triée, pointe en haut si ascendant — `unitMain.pas:1370-1378`, `1606-1608`.
- [ ] **GRID-14 — Opérations futures** : trait vertical rouge (`$0000CC`, 1 px) au bord gauche de la cellule date si `Date > Today` — `unitMain.pas:1610-1618`.
- [ ] **GRID-15 — Séparateur rouge de changement de mois** (8.0 beta 4) : trait horizontal rouge `$0000CC` de 1 px en haut de chaque cellule d'une ligne dont le mois/année diffère de la ligne au-dessus ; **uniquement** quand le tri est sur la colonne Date (`SortCol=0`), à partir de la 2ᵉ ligne ; basé sur la date d'opération même si les dates de pointage sont affichées — `unitMain.pas:1387-1403`, `1620-1623`.
- [ ] **GRID-16 — Icônes de la colonne Tiers** (14×14 à gauche, texte décalé de 18 px), par priorité — `unitMain.pas:1475-1501` :
  1. champ supplémentaire `freeicon=<nom>` → icône `Icons\<nom>.png` (§25.4) ;
  2. transfert « Transfert vers le compte … » → flèche débit, « Transfert depuis le compte … » → flèche crédit (§8) ;
  3. tiers préfixé `[auto] ` (échéance insérée par l'échéancier) → horloge, préfixe masqué ;
  4. tiers préfixé `working_guid` (`34B21B70-D9D6-4226-8F28-500FE55FCF51`, ligne en cours de création par un script) → cadenas, préfixe masqué.
- [ ] **GRID-17 — Clavier sur la grille** — `unitMain.pas:5071-5076`, `5496-5514` : Espace ou `P`/`p` = pointer ; `+` ou `A` (majuscule seulement) = Saisir ; Ctrl+X/C/V = couper/copier/coller ; relâchement de touche → recalcul du total de sélection.
- [ ] **GRID-18 — Souris** — `unitMain.pas:2172-2219`, `4382-4431`, `5765-5773` :
  - clic sur en-tête col. 0-4 : tri (§GRID-19) ; clic sur en-tête col. 5 : ouvre/ferme le panneau de filtre de pointage (anti-rebond 200 ms) ;
  - clic (relâchement) dans la colonne 5 d'une ligne : pointe la sélection (§6.1), **sauf** si la grille a défilé d'elle-même (ligne partiellement visible) et que la souris n'a pas bougé de plus de 5 px ;
  - clic gauche dans les 13 premiers px de la cellule Tiers d'une ligne de transfert : saute au compte et à la ligne liés (§8.3) ; curseur main au survol de cette zone ;
  - Ctrl+clic (ou toujours si option « Sélectionner le champ de saisie lors d'un clic ») en mode intégré : focus sur le champ de saisie correspondant à la colonne (0 → date, 1 → mode, 2 → tiers si moitié haute / détails si moitié basse en mode 2 lignes, 4 → montant ; col. 3 → rien) — `unitMain.pas:4425-4446` ;
  - double-clic sur une ligne : mode séparé → ouvre la saisie et sélectionne le champ cliqué ; mode intégré → **pointe** la ligne ;
  - clic simple : charge l'opération dans la zone de saisie (§5.2) et recalcule le total de sélection.
- [ ] **GRID-19 — Tri** — `unitMain.pas:2211-2218`, `2221-2296` :
  - défaut : colonne Date, **descendant** (plus récent en haut) ;
  - clic sur la colonne déjà triée inverse le sens ; clic sur une autre colonne change de colonne en gardant le sens ;
  - `SortCol`/`SortAscending` toujours écrits dans l'INI, mais relus seulement si `RememberSortCol` ;
  - comparateurs en cascade : **Date** → date, puis mode, puis tiers, puis montant, puis détail ; **Mode** → mode, tiers, montant, détail ; **Tiers** → tiers (sans préfixe de ventilation), montant, détail ; **Catégorie** → libellé formaté uniquement ; **Montant** → montant (tolérance 0,001), puis détail ; avec dates de pointage affichées, la colonne Date trie sur la date de pointage puis mode… ;
  - préfixe de ventilation ignoré dans le tri par tiers : chaîne commençant par `[` dont le `]` est en position ≤ 7 et n'est pas le dernier caractère, longueur ≥ 7 — `unitMain.pas:2227-2237` ;
  - comparaisons de chaînes **ordinales sensibles à la casse** (`CompareStr`, octets CP1252) ;
  - **[Q]** tri par `TList.Sort` (QuickSort **instable**) : l'ordre des égalités n'est pas garanti identique ;
  - le tri **réordonne physiquement** la liste des opérations du compte (donc l'ordre d'écriture dans le fichier) — `SortAccount` appelé par `AFillGrid` sauf après un pointage.
- [ ] **GRID-20 — Largeurs de colonnes** — `unitMain.pas:5269-5327`, `6781-6804` :
  - col. 0 en largeur absolue (min 30), col. 5 fixée à 25 px, col. 1-4 réparties proportionnellement à leurs largeurs mémorisées dans l'espace restant (largeur grille − col0 − 25 − largeur barre de défilement − 15) ;
  - redimensionner une col. 0-3 ajuste la colonne suivante (minimum 30 px pour les deux) ; col. 4 et 5 non redimensionnables (rétablies) ;
  - `[WINDOW] Col0..Col4` réécrits à chaque changement.
- [ ] **GRID-21 — Infobulles de la grille** (fenêtre d'infobulle « flat » blanche, largeur max 200) — `unitMain.pas:2342-2422` :
  - col. Tiers : détail de l'opération, sauf si option « masquer (tiers) » ou mode 2 lignes ;
  - col. Montant : « Dépense : x Curr » / « Recette : … » exprimé dans **l'autre** monnaie que celle affichée ; si la souris est dans la partie droite (au-delà de la moitié en mode 3, des 2/3 sinon) : « Solde : … » (solde progressif, autre monnaie) ; masqué par l'option « masquer (montant) » ; **[Q]** la zone « solde » existe aussi en modes 1 et 2 (dernier tiers de la cellule) ;
  - col. P : « Solde : … » (solde progressif dans la monnaie affichée), masqué si option « masquer (pointage) » ou `ValueMode > 2` ;
  - format `%.<Precision>n` ; cachée pendant un clic ; cachée en quittant la fenêtre.
- [ ] **GRID-22 — Sélection d'une ligne nouvellement ajoutée/modifiée** : marquage interne `Tack += 3` puis retrait au remplissage, la ligne marquée devient la ligne courante et est mémorisée pour les scripts (`ScriptLinesChanged = "<compte>-<index>"`) — `unitMain.pas:3443-3448`, `3521-3524`, `3894`.
- [ ] **GRID-23 — Grille vide** : si aucune ligne ne passe les filtres, la grille n'a que l'en-tête, pas de menu contextuel, et les actions de ligne sont sans effet — `unitMain.pas:1956-1969`.
- [ ] **GRID-24 — Tri ascendant** : au chargement d'un compte, la dernière ligne est sélectionnée — `unitMain.pas:3198`.

---

## 5. Zone de saisie d'opération

### 5.1 Champs et disposition

- [ ] **IN-01 — Champs** : Date (`dt`) + bouton calendrier ; Mode (liste déroulante non éditable, hauteur = nombre de modes) ; Tiers (`EPeople`) et Détails (`EDetails`) côte à côte sous la colonne Tiers ; Catégorie (`CategZone`, zone cliquable) ; Montant (`EValue`) ; bouton Ajouter/Modifier — `unitMain.dfm:640-887`, `unitMain.pas:6814-6827`.
- [ ] **IN-02 — Mode intégré** (défaut) : bandeau de 41 px sous la grille, couleur `clr_input_panel` ; bouton Ajouter combiné : zone gauche (crayon) clic gauche = Ajouter, clic droit = Modifier ; zone droite (flèche) = menu Ajouter/Modifier ; infobulle « Clic gauche = Ajouter / Clic droit = Modifier » — `unitMain.pas:4970-4976`, `6823-6826`, `7101-7122`, `UnitRStrings.pas:110`.
- [ ] **IN-03 — Mode fenêtre séparée** (option `SeparateInput`) — `unitMain.pas:5667-5744`, `7316-7346` :
  - panneau flottant 450×360 dans la fenêtre, libellés « Date : », « Mode de paiement : », « Tiers : », « Détails : », « Catégorie : », « Montant : », boutons « Fermer », « Modifier », « Ajouter » ;
  - masqué par défaut ; F8 / bouton « Saisir » bascule ; double-clic sur la grille l'ouvre ; Échap ou « Fermer » le masque ;
  - déplaçable à la souris (glisser le fond) ; position conservée pendant la session et bornée à la fenêtre.
- [ ] **IN-04 — Aide contextuelle** (bulle sous le champ, 5 s) — `unitMain.pas:4349-4363`, `5094-5126` :
  - Date : « Passez de champ en champ avec Entrée » ;
  - Mode : « Choisissez le mode de paiement » ;
  - Tiers : « Saisissez le nom du bénéficiaire » (modes 0-5), « … du payeur » (6-9 et perso), « Saisissez la qté et le nom de l'action » (Titre) ;
  - Détails : « Saisissez le détail de l'opération », « Saisissez le n° de chq sous la forme n°XXXX » (Chèque), « Saisissez le code du titre dans le champ détails » (Titre) ;
  - Montant : « Montant (sans le signe) » (affichée sous la catégorie si `ValueMode > 1`).

### 5.2 Chargement d'une ligne dans la saisie (clic sur la grille)

- [ ] **IN-05** — `gridClick` `unitMain.pas:1627-1659` :
  - date formatée ; mode : si commence par « Chq » → « Chèque émis », sinon libellé trouvé dans la liste ; introuvable → « Prélèvement » si montant < 0, « Virement reçu » sinon ;
  - tiers, détails, catégorie (libellé sans indentation) ;
  - montant = valeur absolue ; pour le mode « Titre », divisé par la quantité (prix unitaire) ;
  - non exécuté automatiquement après un remplissage si l'option « Vider les champs après l'ajout » est active — `unitMain.pas:3526`.

### 5.3 Navigation et contrôle de saisie

- [ ] **IN-06 — Entrée** : Date → Mode → Tiers → Détails → Montant (en arrivant sur Montant, le menu des catégories s'ouvre automatiquement) → sur Montant, valide l'ajout (sauf si un menu vient de se fermer < 200 ms) — `unitMain.pas:1985-2013`.
- [ ] **IN-07 — Échap** : depuis la zone catégorie → focus montant ; en mode séparé → ferme le panneau — `unitMain.pas:2001-2010`.
- [ ] **IN-08 — Caractères interdits** : `;` et Entrée bloqués dans Tiers, Détails, champs de filtre et dates (le `;` est le séparateur du format de fichier) — `unitMain.pas:1971-1982`, `4525-4528`.
- [ ] **IN-09 — Date** : saisie libre interprétée par `ParseDate` à la sortie du champ puis reformatée ; `+` = jour suivant ; ↑/↓ et PgPréc/PgSuiv = ±1 jour ; Ctrl+PgPréc/PgSuiv = ±1 mois ; Maj+PgPréc/PgSuiv = déplace la sélection de la grille sans quitter la saisie (valable depuis date, mode, tiers, détails, montant) — `unitMain.pas:4996-5020`, `5922-5929`, `5968-5971`.
- [ ] **IN-10 — Interprétation des dates** (`ParseDate`) — `unitMain.pas:1071-1124`, `5931-5966` :
  - normalisation (`SimpleString2` : ponctuation → espace, accents retirés, minuscules), découpe sur les espaces ;
  - noms de mois FR/EN reconnus (janvier/january/janv/jan, fev/feb/febr/fevr/february/fevrier, march/mars, avril/april/apr/avr, may/mai, jun/june/juin, jul/juil/juillet/july, aou/aout/aug/august, sep/sept/september/septembre, oct/octobre/october, nov/november/novembre, dec/december/decembre) ;
  - seuls les nombres sont conservés ; 1 nombre = jour du mois courant ; 2 nombres = jour+mois selon l'ordre du format d'affichage (`dm` ou `md`) dans l'année courante ; 3 nombres = selon l'ordre `d/m/y` déduit du format ; année < 100 → siècle courant ; bornée 1900..9999 ;
  - toute autre situation ou erreur → **date du jour** (champ vidé + Entrée = aujourd'hui) ;
  - **[Q]** suppression d'éléments vides suivie d'un accès à l'index supprimé → exception → date du jour.
- [ ] **IN-11 — Montant = calculatrice** — `unitMain.pas:1932-1954`, `2298-2306`, `3908-3918`, `4343-4347` :
  - pendant la frappe, le résultat formaté (« 1 234,50 € ») s'affiche dans une bulle sous le champ ;
  - à la sortie du champ (et avant validation), l'expression est évaluée et remplacée par le résultat (`CurrToStr`) ;
  - substitutions avant évaluation (insensibles à la casse) : symbole de la monnaie secondaire → `*taux1/taux2`, symbole de la monnaie principale → `*taux2/taux1`, `/` → `*1.0/` (division réelle), `op` → solde pointé, `av` → solde à venir, `solde` → solde total, `,` → `.` ;
  - évaluation par l'interpréteur **Python** (`PyEval`) ; erreur → 0 ;
  - priorité des opérateurs standard (2+3*4 = 14).
- [ ] **IN-12 — Saisie en centimes** (option `EvalCents`) : à l'entrée dans le champ, la valeur existante est multipliée par 100 (entier) ; un entier saisi est divisé par 100 à l'évaluation — `unitMain.pas:1935-1938`, `5114-5120`.
- [ ] **IN-13 — Clic dans le montant** : si exactement 2 caractères sont sélectionnés, la sélection est annulée — `unitMain.pas:7353-7356`.
- [ ] **IN-14 — Auto-complétion** (tiers et détails ; désactivable) — `unitMain.pas:1796-1841` : quand on tape **un** caractère de plus, recherche de la dernière opération du compte (de la fin vers le début de la liste) dont le tiers (resp. le détail) **commence** par le texte (insensible à la casse) ; le reste est ajouté et sélectionné. Réinitialisée à l'entrée dans le champ tiers.
- [ ] **IN-15 — Double-clic dans le champ tiers** : ouvre la gestion des tiers — `unitMain.dfm:881`.

### 5.4 Mode de paiement

- [ ] **IN-16 — Liste** : 10 modes standard puis les modes personnalisés (sans le préfixe `-`) — `unitMain.pas:4791-4803`, `5780-5786` :
  | Index | Libellé | Code fichier | Signe |
  |---|---|---|---|
  | 0 | Carte | 20 | − |
  | 1 | Retrait DAB | 21 | − |
  | 2 | Chèque émis | 22 | − |
  | 3 | Prélèvement | 23 | − |
  | 4 | Virement émis | 24 | − |
  | 5 | TIP | 25 | − |
  | 6 | Versement | 40 | + |
  | 7 | Dépôt de chèque | 41 | + |
  | 8 | Virement reçu | 42 | + |
  | 9 | Titre | 43 | + |
  | ≥10 | modes perso | libellé en clair | − si préfixe `-`, sinon + |
  Réf. : `UnitRStrings.pas:19-28`, `unitMain.pas:4729-4746`.
- [ ] **IN-17 — Chèque** : choisir « Chèque émis » remplit Détails avec le numéro suivant `n°XXXX` (§5.6) — `unitMain.pas:3745-3749`.

### 5.5 Validation (Ajouter / Modifier) — `ASendOperationExecute` `unitMain.pas:3756-3906`

- [ ] **IN-18** — montant vide → focus sur montant, rien d'autre.
- [ ] **IN-19** — évaluation de l'expression du montant ; valeur = **valeur absolue** ; signe appliqué selon le mode.
- [ ] **IN-20 — Modifier** exige une ligne sélectionnée (« Veuillez sélectionner auparavant l'opération à modifier »).
- [ ] **IN-21 — Contrôle du n° de chèque** (mode Chèque émis, ajout seulement) : numéro = chiffres suivant `°` dans Détails jusqu'au premier espace ; s'il existe déjà dans une opération du compte dont le mode commence par « Ch » → « Le numéro de chèque X est déjà attribué. Continuer tout de même ? ».
- [ ] **IN-22 — Modifier une ligne rapprochée** (ou pointée si rapprochement désactivé) avec verrou actif → refus « Impossible de modifier une opération rapprochée car elles sont actuellement verrouillées ».
- [ ] **IN-23 — Modifier une ligne de transfert liée** (tiers saisi commençant par « Transfert ») : « La ligne à modifier est liée à une autre. Quelles lignes souhaitez-vous modifier ? » [Les 2 lignes | Uniquement cette ligne | Annuler] ; « Les 2 lignes » recopie date, détail, catégorie et montant (signe de la ligne sœur conservé, ou montant positif si elle valait 0) dans la ligne sœur (§8).
- [ ] **IN-24 — Modifier une ligne normale** : « Remplacer la ligne sélectionnée ? » (avec « (les 2 montants diffèrent) » si |ancien| ≠ nouveau à 0,00001 près).
- [ ] **IN-25 — Construction de l'opération** :
  - catégorie = celle de la zone catégorie (aucune → −1) ; date = `ParseDate` ; détail = Détails ; `MarkDate := Date` ; pointage : 0 à l'ajout, conservé à la modification ;
  - **UId** : à l'ajout, `Extra := 'UId=' + (last_line_id+1)` ; à la modification, l'`Extra` d'origine est conservé intégralement (UId et champs supplémentaires) ;
  - mode standard : libellé recalculé par `mode_paiement(code, détail)` → un détail contenant `n°1234` sur un Chèque émis donne le mode « Chq 1234 » ; **[Q]** un détail contenant `m=` impose comme mode les 15 caractères qui suivent (§5.7) ;
  - mode perso : libellé = texte du mode, signe selon `-` ;
  - **Titre** : quantité = premier mot du tiers s'il est numérique ; si le tiers ne contient pas « acheté », ajout de ` acheté <montant saisi>` ; montant = prix × quantité ; refus avec bulle « Saisissez le nombre de titres dans le champ tiers » (quantité 0, focus tiers) ou « Saisissez le code du titre dans le champ détails » (détail vide) ;
  - modification = suppression de l'ancienne ligne + ajout de la nouvelle en fin de liste (puis re-tri).
- [ ] **IN-26 — Après validation** : nouvelle ligne sélectionnée, focus sur Tiers ; si option « Vider les champs » → réinitialisation (§MENU-E04) ; exécution des scripts `ExecuteWithNewLine` (ajout) ou `ExecuteWithModifiedLine` (modification) ; ré-affichage ; historique d'annulation « ajout opération » / « modif opération ».
- [ ] **IN-27 — Incrément du chèque** : après validation d'un chèque, Détails est préparé avec `n°<numéro+1>` (**[Q]** sans les zéros de tête, contrairement à §5.6) — `unitMain.pas:3854`.

### 5.6 Numéro de chèque suivant — `GetNextChqNumber` `unitMain.pas:3724-3743`

- [ ] **IN-28** — parcours des opérations triées par date (de la plus récente) ; première dont le mode commence par « Chq » ; numéro = texte après « Chq » (à partir du 5ᵉ caractère) + 1, complété de zéros à la même longueur ; résultat `n°0124` ; aucun chèque → `n°`. Effet de bord : re-tri du compte.

### 5.7 Calcul du libellé de mode depuis un code — `mode_paiement` `unitMain.pas:2016-2060`

- [ ] **IN-29** — si le détail contient `m=`, le mode = 15 caractères après `m=` (héritage) ; sinon code ≤ 20 → « Carte » ; 21-39 → `tabMode[code-20]` (21-29 valides, 30-39 → exception → « Carte ») ; ≥ 40 → index `code-34` (40-43 = Versement, Dépôt de chèque, Virement reçu, Titre ; ≥ 44 → mode perso par **position** dans la liste) ; si le code (après ajustement) vaut 22 et que le détail contient `°` suivi d'un nombre → « Chq <nombre> ». Utilisé au chargement de chaque ligne.

---

## 6. Pointage, rapprochement, verrou

- [ ] **MARK-01 — Pointer** (`MarkLines`) — `unitMain.pas:2767-2815` :
  - sans effet si la grille est vide ; historique « pointage » ;
  - **[Q]** si la ligne sélectionnée (haut de la sélection) est de mode « Titre » : pas de pointage, affiche « Progression: x% » = (montant / (quantité × prix d'achat)) × 100 − 100 ;
  - ligne de référence = ligne cliquée dans la colonne P si elle est en haut ou en bas de la sélection, sinon haut de la sélection ; état de référence = son pointage ;
  - si l'état de référence est « rapproché » (ou « pointé » en mode sans rapprochement) : refus si verrou actif ; sinon confirmation « Vous avez demandé à supprimer le pointage d'une ligne qui a été rapprochée… » (singulier/pluriel) [Supprimer le pointage | Annuler la demande] ;
  - cycle normal : non pointé → pointé → rapproché → non pointé ; sans rapprochement : non pointé ↔ **rapproché** (la valeur stockée est `R`) ;
  - seules les lignes de la sélection ayant le même état que la référence changent (sans rapprochement : toutes les lignes marquées passent à 0) ; `MarkDate := Today` pour chaque ligne modifiée ;
  - pas de re-tri après pointage.
- [ ] **MARK-02 — Déclencheurs du pointage** : menu Pointer, menu contextuel, Espace/P sur la grille, clic dans la colonne P, double-clic (mode intégré).
- [ ] **MARK-03 — Rapprocher toutes les lignes pointées** : toutes les lignes **affichées** à `P` passent à `R` (date de pointage inchangée) ; inactif si rapprochement désactivé ; historique « Tout rapprocher » — `unitMain.pas:5078-5092`.
- [ ] **MARK-04 — Verrou des lignes rapprochées** (`LockLines`, persistant `[LAYOUT] LockLines`) : interdit de dépointer, supprimer ou modifier une ligne rapprochée (ou pointée si rapprochement désactivé) — `unitMain.pas:6942-6949`, `2792-2795`, `2860-2863`, `3812-3816`.
- [ ] **MARK-05 — Date de pointage manuelle** (Ctrl+Maj+P) : saisie libre préremplie avec `jj/mm/aa` ; interprétée par `StrToDate` (format `dd/mm/yy`, séparateur `/`) ; invalide → « La date saisie est invalide » ; historique enregistré même en cas d'annulation ; **[Q]** la grille n'est pas rafraîchie — `unitMain.pas:6008-6026`.
- [ ] **MARK-06 — Affichage des dates de pointage** (F11) : la colonne Date affiche `MarkDate`, en-tête « Ptge », tri sur cette date — `unitMain.pas:3489-3490`, `3235`, `2289`.
- [ ] **MARK-07 — Sémantique de `MarkDate`** : = date d'opération à la création/modification ; = date du jour à chaque changement de pointage par `MarkLines` (y compris dépointage) ; absente du fichier → = date d'opération.
- [ ] **MARK-08 — Rapprochement désactivé** (option `DisableR`) : `P` affiché comme `R`, boutons radio de filtre 1 et 2 grisés, filtre 1/2 converti en 3, pas de ligne « Rapproché : » dans les infobulles des soldes, `ATackRedInBlack` inactif — `unitMain.pas:918-926`, `unitOptions.pas:252-259`.

---

## 7. Édition : annuler/refaire, suppression, presse-papiers interne

### 7.1 Annuler / Refaire — `unitMain.pas:2424-2486`, `2739-2765`

- [ ] **EDIT-01** — historique de **clichés complets** de toutes les opérations de tous les comptes (pas des métadonnées de comptes, catégories, tiers, modes) ; chaque action enregistre l'état **avant** modification avec un libellé : « ajout opération », « modif opération », « pointage », « supprimer lignes », « coller lignes », « Tout rapprocher », « Date de pointage », nom du script exécuté…
- [ ] **EDIT-02** — profondeur `[APPLICATION] MaxUndo` (défaut 30) ; les plus anciens supprimés ; une nouvelle action efface la branche « refaire ».
- [ ] **EDIT-03** — libellés de menu « Annuler <libellé> » / « Refaire <libellé> » ; boutons grisés si rien à annuler/refaire.
- [ ] **EDIT-04** — l'historique est vidé à la fermeture, à toute modification dans la gestion des comptes, à l'inversion des monnaies ; il est verrouillé pendant l'exécution d'un script (une seule entrée au nom du script si celui-ci `NeedsFile`).
- [ ] **EDIT-05 — Indicateur « modifié »** : toute mise à jour du niveau d'historique active « Enregistrer » (y compris le vidage de l'historique) **[Q]**.

### 7.2 Supprimer — `ADeleteLineExecute` `unitMain.pas:2837-2903`

- [ ] **EDIT-06** — refus si cela laisserait le compte vide : « Vous devez garder au moins une ligne ».
- [ ] **EDIT-07** — lignes rapprochées dans la sélection + verrou → refus ; sinon le message de confirmation est préfixé « Il y a des lignes rapprochées dans la sélection. ».
- [ ] **EDIT-08** — une seule ligne liée par transfert à une ligne sœur (dont le compte a plus d'une ligne) : « La ligne sélectionnée est liée à une autre. Souhaitez-vous supprimer les 2 lignes ? » [Oui | Non | Annuler] ; Non → dans la ligne sœur, « Transfert » est remplacé par « Virement » (toutes occurrences, insensible à la casse), ce qui la délie.
- [ ] **EDIT-09** — sinon « Supprimer la ligne sélectionnée ? » / « Supprimer les lignes sélectionnées ? » ; puis **seconde** confirmation si lignes rapprochées.
- [ ] **EDIT-10** — **[Q]** implémentation : les lignes à supprimer reçoivent la date 28/06/1973 (série 26843) puis **toutes** les lignes du compte portant cette date sont supprimées : une opération réelle datée du 28/06/1973 serait supprimée aussi.

### 7.3 Couper / Copier / Coller — `unitMain.pas:2905-2937`, `5257-5267`

- [ ] **EDIT-11 — Copier** : copie profonde des lignes sélectionnées dans un tampon **interne** (pas le presse-papiers système), survit au changement de compte et de fichier ; active « Coller ».
- [ ] **EDIT-12 — Couper** : « Couper la ligne séléctionnée ? » / « … lignes … », puis copie + suppression (sans les questions de transfert).
- [ ] **EDIT-13 — Coller** : historique, puis « Coller N ligne(s) ? » ; ajoute dans le **compte courant** ; catégorie hors bornes → aucune ; **nouvel UId** attribué ; les autres champs supplémentaires (y compris `#sister…`) sont recopiés tels quels.

---

## 8. Transferts de compte à compte et lignes liées

La création d'un transfert est un script **[S]** (`transfer.py`, « Convertir la ligne en transfert ») ; le cœur gère la reconnaissance, l'affichage, la navigation, la suppression et la modification.

- [ ] **XFER-01 — Reconnaissance** (`TransferType`) : tiers commençant par « Transfert vers le compte » → type 1 (débit, flèche sortante) ; « Transfert depuis le compte » → type 2 (crédit) — `unitMain.pas:5245-5250`, `UnitRStrings.pas:17-18`.
- [ ] **XFER-02 — Recherche de la ligne sœur** (`GetTransferSister`) — `unitMain.pas:5192-5243` :
  1. compte sœur par champ supplémentaire `#sisterAccUid` (UID de compte, §11.3) ; sinon par **nom** = texte suivant « compte » dans le tiers ;
  2. ligne sœur par `#sisterLineUid` (UId de ligne) ;
  3. sinon recherche d'une ligne de même date, dont le tiers commence par le préfixe opposé et de montant opposé exact.
- [ ] **XFER-03 — Navigation** : clic sur l'icône de transfert → bascule sur le compte sœur et sélectionne la ligne sœur — `unitMain.pas:4413-4421`.
- [ ] **XFER-04 — Suppression liée** (§EDIT-08) et **modification liée** (§IN-23).
- [ ] **XFER-05** — **[Q]** renommer un compte ne met pas à jour les tiers « Transfert … compte <ancien nom> » ; seuls les liens par UID restent valides.

---

## 9. Filtres et recherche

Champs de filtre intégrés à l'en-tête de la grille, à droite du titre de chaque colonne — `unitMain.pas:1856-1893`. Tout changement de texte relance le remplissage — `unitMain.dfm:440-487`.

- [ ] **FILT-01 — Mode** (`editFilterMode`), **Tiers** (`editFilterTiers` : porte sur tiers + détail + **champs supplémentaires bruts** concaténés), **Catégorie** (`editFilterCateg` : sur le libellé « Parent > Enfant »), **Montant** (`editFilterMontant`) — `unitMain.pas:3349-3471`.
  - Bulles d'aide : montant « Filtrez par montant. Ex: 45.99 ou <0 ou >-100 ou -50>50 » ; mode « Filtrez par mode. Ex: 'ch' (contient 'ch') ou '=carte' (strictement égal à 'carte') » ; tiers « … 'montexte' … ou '/test' (tout sauf…) » — `UnitRStrings.pas:143-145`.
  - **[Q]** la syntaxe `=carte` annoncée n'existe pas : `=` est transformé en espace par la normalisation → aucune ligne ne correspond.
  - **[Q]** le filtre tiers inclut le texte brut des champs supplémentaires (`UId=…`) : chercher « uid » trouve tout ; `//` (champ vide) ne correspond jamais depuis l'introduction des UId.
- [ ] **FILT-02 — Normalisation** (`SimpleString`) : minuscules, lettres accentuées CP1252 repliées (à→a, ç→c, ñ→n, ÿ→y…), chiffres conservés, `,` conservée, tout autre caractère → espace — `UnitStrings.pas:664-690`.
- [ ] **FILT-03 — Filtre simple** (aucun des caractères espace, `|`, `(`, `)`, `"`, `/`) : sous-chaîne normalisée — `UnitFilterString.pas:17-33`.
- [ ] **FILT-04 — Filtre avancé** — `UnitFilterString.pas:35-136` :
  - mots séparés par des espaces = ET ; `"…"` = expression avec espaces ;
  - `|` = OU avec le terme précédent (évaluation par chaîne de bits : `1|0`→`1`, `0|1`→`1` répétés, puis tous les termes restants doivent valoir 1) ;
  - `/` devant un mot = NON ; `(` devant = le mot doit **commencer un mot** ; `)` après = le mot doit **finir un mot** (la valeur est encadrée d'espaces) — **[Q]** le manuel parle de début/fin de chaîne ;
  - `//` seul (tiers, catégorie) = valeur vide.
- [ ] **FILT-05 — Montant** : caractères autorisés `0-9 , . - < > =` ; `<x` → montant ≤ x ; `>x` → ≥ x ; `a>b` → a ≤ montant ≤ b ; sinon sous-chaîne de `Format('%.<Precision>f')` (sans séparateur de milliers) ; `=` accepté sans effet — `unitMain.pas:1733-1738`, `3396-3413`, `1976-1981`.
- [ ] **FILT-06 — Pointage** (panneau ouvert par clic sur l'en-tête de la colonne P) : 0 « Afficher les lignes non pointées », 1 « … pointées », 2 « … rapprochées », 3 « … pointées et rapprochées » (≠ 0), 4 « Tout afficher (aucun filtrage) » (gras) ; fermeture par choix, Échap ou perte de focus — `unitMain.dfm:1014-1090`, `unitMain.pas:6951-6971`.
- [ ] **FILT-07 — Dates** : case à cocher dans l'en-tête Date ; clic → panneau déroulant « Du: » [case][date][calendrier] / « Au: » [case][date][calendrier] / « OK » ; à l'ouverture, les bornes non cochées sont préremplies avec le 1ᵉʳ et le dernier jour du mois courant ; OK ou nouveau clic sur la case → application ; la case d'en-tête est cochée si au moins une borne est active ; bornes inclusives — `unitMain.pas:5343-5382`, `3467`.
- [ ] **FILT-08 — Lignes inutiles** (`HideOldLines`) : masque les lignes **rapprochées** dont la date est **antérieure au 1ᵉʳ du mois courant** — `unitMain.pas:3481`. **[Q]** le manuel décrit une autre règle (frontière de la plus ancienne ligne non pointée). Les lignes masquées restent comptées dans les soldes.
- [ ] **FILT-09 — Effet sur les soldes** : par défaut les soldes du panneau ne portent que sur les lignes filtrées ; option « Ignorer le filtrage lors du calcul du solde » → toutes les lignes — `unitMain.pas:3450-3479`.
- [ ] **FILT-10 — Réinitialisation** au changement de compte si des filtres sont actifs et que « Sauvegarder les filtrages » est désactivé (filtre de pointage → 4, textes vidés, dates décochées) — `unitMain.pas:3080-3091`, `3183`. « Lignes inutiles » non concerné.
- [ ] **FILT-11 — Mémorisation** (option « Sauvegarder les filtrages ») : à la fermeture, écriture de `FilterDate1/2` (série ou 0), `FilterMode/Tiers/Categ/Montant`, `FilterMark` ; relecture à l'ouverture d'un fichier — `unitMain.pas:962-984`, `1753-1766`.
- [ ] **FILT-12 — `IsFilter`** : vrai si case de date cochée, un texte non vide ou filtre de pointage ≠ 4 — `unitMain.pas:6612-6620`.
- [ ] **FILT-13 — Simulation de filtres** (`SimulateFilters`, utilisé par l'API) : même logique sur tous les comptes, renvoie `idx;idx|idx;…` ; **[Q]** le filtre tiers n'y inclut pas les champs supplémentaires — `unitMain.pas:3549-3705`.
- [ ] **FILT-14 — Impression** : les filtres actifs sont rappelés en tête de relevé (§22).

---

## 10. Panneau des soldes et calculs

Panneau gauche 160 px, couleur `clr_total_panel` — `unitMain.dfm:134-427`, `unitMain.pas:6722-6767`.

- [ ] **BAL-01 — Logo** (zone 1..158 × 120 px, centré) : `PathExe\logos\<logo en minuscules, espaces→_>.png` du compte, sinon logo BankPerfect — `unitMain.pas:3200-3220`.
- [ ] **BAL-02 — Titre** : nom du compte en gras, tronqué avec « ... » à la largeur du filet ; « Totaux » sans fichier — `unitMain.pas:3165-3181`, `4248`.
- [ ] **BAL-03 — Soldes** « Pointé : » (lignes P+R), « A venir : » (non pointées), « Solde : » (somme) ; alignés à droite ; négatif en `clr2_total_panel_text`, sinon `clr_total_panel_text` ; format `%.<Precision>n <devise>` dans la monnaie affichée — `unitMain.pas:4691-4722`.
- [ ] **BAL-04 — Infobulles des soldes** : montant dans l'autre monnaie + (si rapprochement actif) « Rapproché : <somme des lignes R> » — `unitMain.pas:4710-4715`.
- [ ] **BAL-05 — Barre d'état** si panneau masqué (§UI-07).
- [ ] **BAL-06 — Date de dernier rapprochement** : date max des lignes `R` (non filtrée par les filtres) affichée « - Compte rapproché au … » — `unitMain.pas:3456`, `3531-3538`.
- [ ] **BAL-07 — Sélection** (« Sélection : ») — `unitMain.pas:6842-6940` :
  - 1 ligne : « Ligne <UId> : <montant> » ; N lignes : « N lignes : <total> » ;
  - si plusieurs lignes avec débits **et** crédits : « +<total crédits> » et « <total débits> » en dessous ;
  - total négatif en couleur « solde négatif » ; montants **dans la monnaie principale** (pas de conversion) ;
  - si panneau masqué : ces informations deviennent l'infobulle de la barre d'état.
- [ ] **BAL-08 — Champs supplémentaires** (« Champs supplément. : ») : pour une seule ligne sélectionnée, liste `clé : valeur` des champs supplémentaires, **sauf** `UId`, `freeicon` et les clés commençant par `#` ; `<fab_pv>` réaffiché `;` ; titre masqué si vide — `unitMain.pas:6869-6899`, §25.
- [ ] **BAL-09 — Solde prévisionnel** (« Solde prévisionnel : ») : champ date (défaut aujourd'hui + 12 mois) + bouton calendrier + bouton « démarrer » qui lance le script `previsions.py` avec le paramètre `yyyy-mm-dd` (rien si le script est absent) ; masquable (§MENU-A04) — `unitMain.pas:7408-7433`, `6748-6767`. **[S]**
- [ ] **BAL-10 — Polices** du panneau = police par défaut et taille (§19) — `unitMain.pas:5866-5920`.

---

## 11. Comptes

### 11.1 Modèle

- [ ] **ACC-01 — Champs d'un compte** (`TAccount`) : BankName, AgencyName, AgencyAddress, AccName, AccNumber, Rate1, Curr1, Rate2, Curr2, BankNumber, AgencyNumber, BIC, picture (logo), IBAN, IsVirtual, AccUID, IsDisabled — `ObjectAccountList.pas:10-36` (ordre positionnel dans le fichier : `02-format-fichier-bp.md`).
- [ ] **ACC-02 — Compte « virtuel »** : `IsVirtual` **ou** nom vide **ou** nom commençant par `_` → exclu des totaux multi-comptes (API `getsoldes`/`GetTotals`) — `ObjectAccountList.pas:120-126`, `unitMain.pas:2079-2133`.
- [ ] **ACC-03 — UID de compte** : s'il est absent au chargement, généré `yyyymmddhhnnss-<index>-<5 car.>` (alphabet `a-zA-Z0-9` mais tirage `RandomRange(1,62)` → le caractère `9` n'est jamais produit) — `ObjectAccountList.pas:58-71`, `167-168`.
- [ ] **ACC-04 — Compte désactivé** (`IsDisabled`) : absent du menu des comptes et du cycle Ctrl+Tab, exclu des totaux multi-comptes ; reste dans le fichier et dans la gestion des comptes — `unitMain.pas:7180`, `7052`, `2090`.
- [ ] **ACC-05 — Ordre d'affichage** (`accounts_order`) indépendant de l'ordre dans le fichier ; par défaut identité — `ObjectAccountsOrder.pas`.
- [ ] **ACC-06 — Compte par défaut** : index (ordre fichier) du compte courant au moment de la sauvegarde ; rouvert sur ce compte — `unitMain.pas:2601`, `4113-4123`.

### 11.2 Fenêtre « Gestion des comptes » (`UnitAccounts.pas`, `UnitAccounts.dfm`)

- [ ] **ACC-07 — Liste** « Comptes: » dans l'ordre d'affichage (couleurs alternées), sélection → chargement des champs — `UnitAccounts.pas:109-186`.
- [ ] **ACC-08 — Champs** (enregistrés à la sortie de chaque champ, sans bouton « Appliquer ») :
  - « Nom du compte: » (25 car. max ; vide → « Compte<n> » + bulle « Le champ nom ne peut pas rester vide ») ;
  - « Banque/guichet: » nom de banque (30) et d'agence (30) ; « Adresse complète: » (150, infobulle « Adresse de l'agence ») ;
  - « N° de compte (code banque / code guichet / n° de compte): » : code banque (5 chiffres), code guichet (5 chiffres, bulle « Comptes belges: laissez vide »), numéro (11 alphanumériques majuscules) et **clé RIB calculée** affichée à droite ;
  - « BIC: » (11 alphanumériques, majuscules) ; « IBAN: » (80, alphanumériques et espaces, majuscules) + bouton calculatrice (§ACC-12) ;
  - groupe « Monnaies » : « Principale: » taux1 (défaut 1, 15 car., chiffres `, .`) + symbole (3 car. majuscules, défaut €) ; « = » ; « Secondaire: » taux2 (défaut 6.55957) + symbole (défaut F) ;
  - groupe « Logo bancaire » : liste « Aucun » + fichiers `PathExe\logos\*.png` affichés en MAJUSCULES avec `_`→espace, aperçu ;
  - « Ne pas inclure ce compte lors des calculs de solde ou statistiques » (virtuel) ;
  - « Compte inactif (ne pas l'afficher dans la liste des comptes) ».
  - Réf. : `UnitAccounts.dfm`, `UnitAccounts.pas:188-240`, `682-730`.
- [ ] **ACC-09 — Assainissement** : `;` remplacé par espace dans tous les champs texte ; taux invalides → 1 / 6.55957 ; symboles vides → € / F ; toute modification **vide l'historique d'annulation** et marque le fichier modifié — `UnitAccounts.pas:196-202`, `684`.
- [ ] **ACC-10 — Ajouter un compte** (« Ajouter un compte ») : si le champ nom est vide ou déjà utilisé → « Entrez le nom du compte à créer » ; puis « Entrez le solde initial (positif ou négatif) » (annuler = 0) ; création d'un compte vierge (€/F, 1/6,55957, nouvel UID) avec une première ligne : date du jour, **rapprochée**, mode « Virement reçu », tiers « Solde initial », montant saisi, catégorie aucune ; ajouté en fin d'ordre d'affichage et devient courant — `UnitAccounts.pas:248-261`, `307-357`, `ObjectAccountList.pas:75-79`.
- [ ] **ACC-11 — Supprimer** (« Supprimer le compte ») : « Supprimer le compte X ? » ; refus s'il n'en reste qu'un (« Vous devez garder au moins un compte dans le groupe ») ; supprime le compte et ses lignes, réindexe l'ordre ; le premier compte de l'ordre devient courant — `UnitAccounts.pas:264-291`.
- [ ] **ACC-12 — Monter/Descendre** : réordonne l'ordre d'affichage — `UnitAccounts.pas:647-661`.
- [ ] **ACC-13 — Clé RIB** (`clef_rib`) — `UnitAccounts.pas:407-440` : vides → `00000`/`00000`/`0000000` ; lettres du numéro converties (A-I→1-9, J-R→1-9, S-Z→2-9) ; France : clé = (8·B mod 97 + 194 − 15·G mod 97 − 3·C mod 97) mod 97, 0 → 97, sur 2 chiffres (équivaut à 97 − (89B+15G+3C) mod 97) ; **Belgique** si code guichet `00000` : numéro ramené à 7 chiffres préfixé du code banque, clé = 10 derniers chiffres mod 97 (0→97) sur 2 chiffres.
- [ ] **ACC-14 — Calcul d'IBAN** (bouton) — `UnitAccounts.pas:359-395` : France `FR` + clé + banque+guichet+compte+clé RIB ; Belgique (guichet vide ou `00000`) `BE` + chiffres 3-5 de la banque + chiffres 5-11 du compte + clé ; lettres → nombres (A=10…), mod 97 par blocs, clé = 98 − mod ; affiché par groupes de 4 séparés par des espaces (espace final).
- [ ] **ACC-15 — OK** : rafraîchit le menu des comptes et recharge le compte courant — `UnitAccounts.pas:663-667`.

### 11.3 Navigation entre comptes

- [ ] **ACC-16** — menu déroulant du bouton Comptes ; Ctrl+Tab / Ctrl+Maj+Tab / clic principal = suivant/précédent dans l'ordre d'affichage en sautant les comptes désactivés, bouclage, arrêt après 2 cycles — `unitMain.pas:7028-7062`.
- [ ] **ACC-17 — Chargement d'un compte** (`ALoadAccount`) : titre, filtres (§FILT-10), taux et devises, bouton devise, mode de saisie remis à « Carte », remplissage, logo — `unitMain.pas:3153-3207`.

---

## 12. Catégories

### 12.1 Modèle — `ObjectCategs.pas`

- [ ] **CAT-01 — Liste ordonnée** de libellés ; chaque libellé a un **identifiant numérique stable** (référencé par les opérations et les tiers) distinct de sa position ; sous-catégorie = libellé commençant par un espace (stocké avec **5 espaces** de préfixe) ; une seule profondeur — `ObjectCategs.pas:280-283`, `77-78`.
- [ ] **CAT-02 — Libellé formaté** : catégorie mère → son nom ; sous-catégorie → « Mère > Enfant » ; identifiants sans libellé → vide ; `LastIndex` = plus grand identifiant — `ObjectCategs.pas:394-419`.
- [ ] **CAT-03 — Contrôles** : la première ligne ne peut pas être une sous-catégorie (désindentée automatiquement) ; catégories à nom vide ignorées au chargement ; **[Q]** nom tronqué au chargement à `p+51` caractères (`p` = position du `=`) — `ObjectCategs.pas:298-324`.
- [ ] **CAT-04 — Nouvel identifiant** = `LastIndex + 1` — `ObjectCategs.pas:439-447`.
- [ ] **CAT-05 — Catégories par défaut** d'un nouveau fichier (identifiant = position) : Alimentation (Epicerie, Restaurant) ; Loisirs (Sorties Spectacles, Sports, Hifi Vidéo, Téléphonie) ; Habillement ; Habitation (EDF, Téléphone, Loyer, Equipement, Rangement/Déco, Abonnements, Assurance) ; Santé (Médecine, Pharmacie) ; Transport (Essence, Assurance, Equipement/Réparations, Public, Parking, Péage) ; Remboursements (Rbt santé, Autres) ; Salaire (Paye, Revenus annexes) — `UnitRStrings.pas:62-63`.
- [ ] **CAT-06 — Validation des références** : au chargement, catégorie invalide → aucune ; au remplissage, identifiant > `LastIndex` → remis à −1 (**modification des données**) — `unitMain.pas:3964-3965`, `3439`.

### 12.2 Fenêtre « Gestion des catégories » — `unitCategsManage.pas`

- [ ] **CAT-07 — Liste arborescente dessinée** : trait vertical continu, case pour les mères (pleine blanche si sans enfant), trait horizontal pour les enfants, couleurs alternées — `unitCategsManage.pas:185-241`.
- [ ] **CAT-08 — Insérer une catégorie / Insérer une sous-catégorie** : nom demandé (saisie restreinte, §27.2) ; insérée **avant** la ligne sélectionnée, sinon en fin — `unitCategsManage.pas:147-168`.
- [ ] **CAT-09 — Supprimer la catégorie** : « Supprimer la catégorie X ? » ; supprime la seule ligne (**[Q]** les sous-catégories d'une mère supprimée sont rattachées à la mère précédente ; les opérations gardent l'identifiant orphelin → affichées sans catégorie puis remises à −1 au prochain chargement) — `unitCategsManage.pas:67-83`.
- [ ] **CAT-10 — Renommer la catégorie** (indentation conservée) — `unitCategsManage.pas:85-104`.
- [ ] **CAT-11 — Indenter** : bouton libellé « Changer en sous-catégorie » / « Changer en catégorie » selon la ligne ; impossible sur la 1ʳᵉ ligne (« Cette ligne ne peut être indentée ») — `unitCategsManage.pas:133-145`, `243-257`.
- [ ] **CAT-12 — Monter/Descendre** : échange avec la voisine ; case « Déplacer la catégorie avec ses sous-catégories » → déplace le groupe entier par-dessus le groupe voisin ; « Déplacement impossible » sinon — `unitCategsManage.pas:106-131`, `ObjectCategs.pas:116-196`.
- [ ] **CAT-13 — Trier les catégories** : mères triées alphabétiquement (ordinal, sensible à la casse), enfants triés au sein de leur mère, identifiants conservés — `ObjectCategs.pas:214-278`.
- [ ] **CAT-14 — Exporter** : fichier `.txt` (nom proposé = fichier .bp en `.txt`), une ligne `id=libellé` par catégorie — `unitCategsManage.pas:265-276`.
- [ ] **CAT-15 — Importer** : fichier `.txt`, lignes jointes par `;` puis rechargées (remplace toute la liste, identifiants du fichier) ; **[Q]** aucune vérification du nombre de lignes malgré le manuel — `unitCategsManage.pas:278-301`.
- [ ] **CAT-16** — chaque action marque le fichier modifié et rafraîchit la grille ; taille de fenêtre mémorisée (`[FormCategsManage]`) — `unitCategsManage.pas:303-306`, `361-364`.

### 12.3 Choix d'une catégorie

- [ ] **CAT-17 — Menu contextuel des catégories** (zone catégorie de la saisie et de la fiche tiers) : une mère avec enfants devient un sous-menu dont le premier élément est la mère elle-même, puis un séparateur, puis les enfants ; à la fin, séparateur + « Aucune » — `unitMain.pas:3094-3150`, `2067-2077`.

---

## 13. Tiers (opérations types)

### 13.1 Modèle — `ObjectPeople.pas`

- [ ] **PEO-01** — champs : Caption (nom), Mode (**index** dans la liste des modes), Category (identifiant), Who (tiers), Details, Value (chaîne libre) — `ObjectPeople.pas:19-28`. **[Q]** l'index de mode se décale si les modes perso sont modifiés.
- [ ] **PEO-02** — liste triée par nom (ordinal) à l'affichage ; recherche « contient » insensible à la casse sur nom+tiers+détails — `ObjectPeople.pas:97-140`.

### 13.2 Fenêtre « Liste des tiers » — `UnitPeople.pas`

- [ ] **PEO-03 — Nouveau tiers** : crée « Nouveau tiers » (mode −1, catégorie −1) et le sélectionne pour édition — `UnitPeople.pas:114-125`.
- [ ] **PEO-04 — Supprimer le tiers** (sans confirmation ; bulle si aucun sélectionné) — `UnitPeople.pas:99-112`.
- [ ] **PEO-05 — Rechercher:** filtre la liste sur le nom ; ↑/↓ naviguent ; Entrée/Échap ferment ; texte d'aide « Entrez quelques caractères contenus dans le nom du tiers à rechercher puis validez » — `UnitPeople.pas:91-97`, `240-274`.
- [ ] **PEO-06 — Fiche** « Tiers sélectionné » : Nom, Mode, Tiers, Détails, Montant, Catégorie (menu §CAT-17) ; modification immédiate ; **[Q]** l'édition d'un champ ne marque pas le fichier modifié (seuls ajout/suppression le font) — `UnitPeople.pas:127-148`.
- [ ] **PEO-07 — Sélection = chargement en direct** dans la zone de saisie principale (tiers, mode, détails, catégorie, montant) ; pour un chèque, le numéro suivant est préfixé aux détails — `UnitPeople.pas:191-238`.
- [ ] **PEO-08** — double-clic sur un tiers : charge et ferme ; taille de fenêtre mémorisée `[FormPeople]` — `UnitPeople.pas:170-189`.
- [ ] **PEO-09** — `ECaptionExit` (copier le nom dans « Tiers » si vide) existe mais n'est pas branché (code mort) — `UnitPeople.pas:164-168`.

### 13.3 Depuis la fenêtre principale

- [ ] **PEO-10 — Charger un tiers (F10)** — `unitMain.pas:5801-5856` : nécessite la zone de saisie visible ; texte du champ tiers avant la partie auto-complétée ; vide → « Saisissez du texte dans le champ tiers » ; recherche d'abord dans la liste des tiers (chargement des champs, focus sur le premier champ vide parmi tiers/détails/montant), sinon dans les opérations du compte de la plus récente (de la fin de liste) dont tiers+détail contient le texte → recopie tiers, mode, détail, catégorie, montant ; sinon « Aucun tiers ne contient ce texte ». **[Q]** depuis une opération : un chèque donne l'index 1 (« Retrait DAB »), la catégorie est posée par identifiant au lieu de position, le montant garde son signe.
- [ ] **PEO-11 — Ajouter aux tiers** : « Souhaitez-vous créer un nouveau tiers "X" ? » [Créer | Annuler] → tiers créé depuis la saisie courante (nom = tiers saisi) puis ouverture de la liste des tiers — `unitMain.pas:5058-5069`.

---

## 14. Modes de paiement personnalisés — `UnitModes.pas`

- [ ] **MODE-01 — Fenêtre** « Modes de paiement supplémentaires » : liste « Nom (dépense) » / « Nom (recette) » ; « Nouveau mode » ajoute « -NOUVEAU MODE » ; champ « Modifier le mode de paiement sélectionné : » (`;` supprimés, vide → « NOUVEAU NOM ») ; case « Mode de paiement créditeur » (absence de `-`) ; « Supprimer le mode » ; « Fermer » applique — `UnitModes.pas:42-134`.
- [ ] **MODE-02** — **[Q]** fermer par la croix de la fenêtre annule les changements ; « Fermer » ne marque pas le fichier modifié.
- [ ] **MODE-03** — au chargement, modes vides et doublons ignorés — `unitMain.pas:4846-4852`.
- [ ] **MODE-04** — **[Q]** perte de données : une opération dont le libellé de mode n'est plus dans la liste (mode perso supprimé/renommé) est écrite avec le code `19` et redevient « Carte » au rechargement (`02-format-fichier-bp.md`) — `unitMain.pas:4001-4005`.

---

## 15. Devises

- [ ] **CUR-01 — Deux monnaies par compte** : principale (Curr1/Rate1) et secondaire (Curr2/Rate2) ; conversion principale → secondaire = `montant × Rate2 / Rate1` — `unitMain.pas:4679-4689`.
- [ ] **CUR-02 — Bouton devise** (bascule) : affiche montants, soldes et solde progressif dans la monnaie secondaire ; libellé = symbole affiché ; les calculs restent en monnaie principale — `unitMain.pas:3321-3325`, `3485-3486`.
- [ ] **CUR-03 — Inverser les monnaies** — `unitMain.pas:2939-2976` : « Attention ! Toutes les opérations vont être converties de X en Y. Souhaitez-vous continuer ? » ; historique vidé ; chaque ligne du compte courant : `Round(valeur × 100 × Rate2 / Rate1) / 100` (**arrondi bancaire** Delphi) ; ajout d'une ligne d'ajustement : date du jour, rapprochée, mode « Virement reçu » si écart > 0 sinon « Prélèvement », tiers « Ajustement des arrondis de conversion », détail « Solde: <ancien solde, 2 déc.> <ancienne devise> », montant = ancien solde converti − somme des lignes converties ; échange devises/taux ; message « Votre monnaie principale est désormais le X ».
- [ ] **CUR-04 — Symboles dans la calculatrice** (§IN-11).

---

## 16. Fichiers : création, ouverture, sauvegarde

### 16.1 Nouveau fichier — `unitMain.pas:2676-2726`

- [ ] **FILE-01** — nom demandé « Entrez le nom du fichier comptes » (saisie restreinte §27.2) ; existe déjà dans le dossier → « Ce fichier comptes existe déjà. Veuillez en choisir un autre. » et nouvelle demande ; extension retirée.
- [ ] **FILE-02** — « Entrez le solde initial (positif ou négatif) » (flottant ; annuler = 0 ; invalide = 0).
- [ ] **FILE-03** — fermeture du fichier courant (abandon si l'utilisateur annule) ; création dans `[SAVE] PathSave` (ou Documents si le dossier n'existe pas) d'un fichier minimal : `utilisateur=<nom>`, un compte « Courant » (€/F, 1/6.55957, réel), catégories par défaut, une opération datée du jour, rapprochée, code mode 23 (Prélèvement) si solde négatif sinon 40 (Versement), tiers « Création du compte courant », catégorie aucune, montant saisi ; puis ouverture. **[Q]** solde saisi vide puis OK → accès à `Solde[1]` sur chaîne vide.

### 16.2 Ouverture — `LoadBankPerfectFile` `unitMain.pas:4023-4134`

- [ ] **FILE-04** — extension `.bp` ou `.bp0` uniquement, sinon « Ce fichier n'est pas un fichier BankPerfect ».
- [ ] **FILE-05** — fichier sur `A:` → copié d'abord dans `PathExe` avec message (« Par sécurité, le fichier va être copié depuis le lecteur de disquettes… ») **[W]**.
- [ ] **FILE-06 — Restauration d'une sauvegarde `.bp0`** : nom cible = chemin coupé au **dernier** `_` + `.bp` (dans le **dossier de la sauvegarde**) ; s'il existe : « Attention ! Si vous restaurez le fichier de sauvegarde X, il écrasera le fichier Y… Continuer ? » ; copie puis ouverture du `.bp`. **[Q]** le `.bp` restauré atterrit dans le dossier des sauvegardes, pas dans `PathSave`.
- [ ] **FILE-07** — fichier en lecture seule → « Attention ! Ce fichier est en lecture seule… » (ouverture quand même).
- [ ] **FILE-08 — Décodage** : V5, puis UCL/UPX, puis Shuffle (cf. doc 02) ; échec → fichier corrompu (§16.8).
- [ ] **FILE-09 — Mot de passe** (§17).
- [ ] **FILE-10** — lecture de `UserName` (**[Q]** si la clé est vide, la valeur du fichier précédemment ouvert est conservée — `unitMain.pas:4099-4100`), des données de scripts, des comptes, de l'ordre, du compte par défaut ; `guid` absent → généré (§16.12) ; attribution des UId manquants (§25.2) ; filtres mémorisés ; titre ; scripts `ExecuteWithFile` (ex. insertion automatique des échéances) et menus déroulants de scripts. **[S]**
- [ ] **FILE-11** — après ouverture, le fichier n'est pas marqué modifié (sauf attribution d'UId).

### 16.3 Enregistrer — `ASaveExecute` `unitMain.pas:2561-2634`

- [ ] **FILE-12** — sans effet si aucun fichier ou sauvegarde déjà en cours ; fenêtre désactivée pendant l'opération.
- [ ] **FILE-13 — Sauvegarde tournante** : **avant** l'écriture, copie du `.bp` actuel sur disque vers `[SAVE] PathBackup` (défaut `PathAppData`) + `<nom sans ext>_NN.bp0`, où `NN` = compteur **global** `[SAVE] LastBackupNumber` incrémenté à chaque sauvegarde, sur 2 chiffres, de `01` à `20` puis retour à `01` — `unitMain.pas:726-735`, `2580-2587`. **[Q]** le manuel annonce 30 versions paramétrables ; le code en garde 20, compteur partagé entre tous les fichiers.
- [ ] **FILE-14 — Écriture sécurisée** (`SecuritySave`) — `unitMain.pas:2526-2559` : écrit `file1_bp.tmp` dans le dossier du `.bp`, le copie en `file2_bp.tmp`, vérifie la relecture de `file2` ; remplace le `.bp` par `file1` (suppression puis déplacement, attente jusqu'à 1 s) ; vérifie le `.bp` ; en cas d'échec le remplace par `file2` ; jusqu'à 9 tentatives espacées de 200 ms ; en cas de succès, `<nom>_current.bp0` (dans le dossier des sauvegardes) est remplacé par une copie du fichier enregistré ; échec définitif → « Impossible de sauvegarder le fichier : il y a peut-être un problème de droits d'écriture. Essayez d'utiliser la fonction "Enregistrer sous"… » ; fichiers temporaires supprimés. **[Q]** une boucle `Continue` sans incrément peut boucler indéfiniment ; de même `SyncMoveFile` (attente du fichier déplacé) n'incrémente jamais son compteur et boucle sans fin si le fichier n'apparaît pas — `unitMain.pas:2508-2524`.
- [ ] **FILE-15** — puis « Enregistrer » grisé, message de barre d'état 4 s.
- [ ] **FILE-16** — l'ordre d'écriture des opérations est l'ordre courant en mémoire (dépend du dernier tri, §GRID-19).
- [ ] **FILE-16b — Effets de bord de la sauvegarde** (détail : doc 02) : les montants sont écrits avec `Precision` décimales (**[Q]** précision 0 = centimes perdus) ; les clés d'en-tête inconnues sont perdues ; **[Q]** un **dernier** compte sans aucune opération n'est pas relu au rechargement (décalage comptes/opérations, fichier inutilisable à la sauvegarde suivante) — `unitMain.pas:2615-2619`, `4855-4865`, `4019`.

### 16.4 Fermeture / quitter — `unitMain.pas:1769-1794`, `4232-4267`

- [ ] **FILE-17** — si modifié : option « Sauvegarder automatiquement à la fermeture » → sauvegarde silencieuse ; sinon « Vos comptes ont été modifiés. Souhaitez-vous les enregistrer avant de quitter ? » [Oui | Non | Annuler].
- [ ] **FILE-18** — fermeture : retour à l'écran d'accueil (liste rafraîchie), données libérées, données de scripts effacées, historique vidé, mot de passe oublié.
- [ ] **FILE-19** — en quittant : écriture de la géométrie et des filtres (§19), puis persistance de l'INI ; échec → « Impossible de mettre à jour le fichier INI ».

### 16.5 Enregistrer sous (§MENU-F04).

### 16.6 Fichier corrompu — `unitMain.pas:5652-5665`

- [ ] **FILE-20** — « Votre fichier est illisible : il semble avoir été corrompu… Souhaitez-vous récupérer une des sauvegardes les plus récentes ? » → lance le script `backups.py` s'il existe. **[S]**

### 16.7 Sauvegarde sur disque externe — `unitMain.pas:4606-4640`

- [ ] **FILE-21** — lettre `[SAVE] FloppyDrive` (défaut `A`, une lettre) ; attente d'un disque (« Veuillez d'abord insérer une disquette… » [Réessayer | Annuler]) ; enregistre puis copie vers `X:\<nom>_NN.bp` (compteur `LastFloppyBackupNumber`, 01-20) ; messages « Le fichier de comptes a été sauvegardé sur le disque X » / « Impossible de sauvegarder… ». **[W]**

### 16.8 Sauvegarde FTP — `unitMain.pas:4284-4336`

- [ ] **FILE-22** — si `[FTP] ftpSave` faux → « Veuillez activer la sauvegarde FTP » et ouverture des options ; sinon envoi du `.bp` **tel qu'il est sur disque** (modifications non enregistrées exclues) vers `ftpFolders` + `<nom>_NN.bp` (compteur `LastOnlineBackupNumber`, 01-20) ; mot de passe déchiffré (§28.6) ; erreurs « Transfert du fichier via FTP impossible : … » ; fenêtre de progression (`UnitDownload`). Probablement à abandonner/remplacer.

### 16.9 Supprimer un fichier de comptes — `unitMain.pas:5981-5999`

- [ ] **FILE-23** — sélection d'un `.bp` (« Séléctionnez le fichier à supprimer », dossier `PathSave`) ; « Supprimer le fichier BankPerfect X ? » ; refus si c'est le fichier ouvert (« Le fichier X est actuellement ouvert… ») ; suppression ; liste d'accueil rafraîchie si aucun fichier ouvert.

### 16.10 GUID de fichier

- [ ] **FILE-24** — clé `guid` ; absente → `{GUID}-ddmmyyyyhhnnsszzz-<chemin du .bp>` (`CreateBPGUID`) — `unitMain.pas:4838-4839`, `6637-6644`.

---

## 17. Mot de passe

- [ ] **PASS-01 — Stockage** en clair dans la clé `pass` du fichier — `unitMain.pas:2602`.
- [ ] **PASS-02 — Vérification à l'ouverture** : comparaison **insensible à la casse** ; boucle « Entrez le mot de passe » (champ masqué) jusqu'à succès ou annulation ; **mot de passe maître** `ch280673` accepté pour tout fichier ; après ouverture, le mot de passe conservé est la version **en minuscules** (donc réécrit en minuscules) ; mode silencieux (API) → code retour 2 — `unitMain.pas:4083-4097`.
- [ ] **PASS-03 — Définir/modifier** (« Entrez un mot de passe » / « Modifiez ou supprimez le mot de passe ») : vide → suppression (« Le mot de passe a été supprimé ») ; sinon confirmation « Veuillez confirmer le mot de passe » ; différents → « Les deux mots de passe ne correspondent pas » ; succès → « … pris en compte » / « … modifié » ; marque le fichier modifié — `unitMain.pas:4642-4671`. (Le manuel dit à tort qu'il n'y a pas de confirmation.)
- [ ] **PASS-04 — Masquage** : police Wingdings avec caractère `l` (puce) — `UnitInput.pas:43-46`. **[W]**

---

## 18. Licence, identité, à propos

- [x] **LIC-01 — Vérification de la clé** : remplacée. L'ancienne vérification locale de la clé n'est plus dans l'application ; la clé reçue à l'achat est vérifiée une seule fois par le serveur de licences de l'auteur, qui renvoie un certificat signé (Ed25519) vérifié hors ligne par l'application (`crates/bp-app/src/license.rs`). Le format de l'ancienne clé n'est volontairement pas documenté ici.
- [ ] **LIC-02 — Entrer son n° de licence** : boucle « Entrez ici votre code de licence » (saisie libre) ; invalide → « Le code de licence saisi est incorrect. N'essayez pas de le saisir manuellement… » ; valide → `[APPLICATION] Licence`, titre mis à jour, INI persisté immédiatement — `unitMain.pas:5396-5412`.
- [ ] **LIC-03 — Effets de l'enregistrement** : titre « Licence n° … » / « Licence Gold n° … », bandeau et menu « S'enregistrer » masqués, fonction d'envoi d'e-mail (API) autorisée (§30).
- [ ] **LIC-04 — À propos** : `BPDefaultCaption` (non enregistré) ou « BankPerfect », puis « <Description> / Version <FileVersion> du <date du fichier> / Emplacement: <PathExe> / Copyright: <BPCopyright> / Organisation: <BPCompany> », puis « Programme non enregistré » ou « Licence n° … » ; si non enregistré : « Souhaitez-vous acquérir une licence ? » [Oui | Non] → page d'enregistrement — `unitMain.pas:3040-3078`, `UnitRStrings.pas:74`.
- [ ] **LIC-05 — Fichier `identity.dat`** (chiffré codec v5, lignes `clé=valeur` séparées par LF) lu à chaque appel de `GetIdentity` — `unitMain.pas:6646-6670`. Clés : `BPDefaultCaption`, `BPRegisteredCaption` (vide = enregistrement désactivé), `BPWebsite`, `BPSupport`, `BPCopyright`, `BPCompany`, `BPShowOnlineHelp` (lu nulle part dans le cœur), `BPShowOnlineFAQ`. Variantes fournies : `identity-default.dat` (= `identity.dat`, BankPerfect / chelly.net) et `identity-office-one.dat` (marque blanche « OFFICE One BankPerfect », Issendis, enregistrement désactivé, aide/FAQ en ligne masquées). Le contenu en clair figure en commentaire `unitMain.pas:1210-1221`.

---

## 19. Options (`unitOptions.pas`) et fichier `bankperfect.ini`

### 19.1 Fenêtre « Options du programme BankPerfect »

Onglets (liste à gauche) : Général, Sauvegarde, Affichage, Couleurs et police ; boutons « Annuler » et « OK ». « OK » applique et enregistre toutes les options (sauf couleurs/police, appliquées immédiatement) ; « Annuler » ne restaure **pas** les couleurs/police déjà modifiées — `unitOptions.pas:123-313`, `unitOptions.dfm`.

| ID | Onglet | Libellé | Clé INI | Type | Défaut | Réf. |
|---|---|---|---|---|---|---|
| OPT-01 | Général | Niveaux d'annulation | `[APPLICATION] MaxUndo` | entier 1..100 | 30 | opt.pas:131, 275 |
| OPT-02 | Général | Précision (décimales) — affecte aussi l'**écriture** des montants dans le .bp | `[APPLICATION] Precision` | entier 0..4 | 2 | opt.pas:137, 276 ; main:929, 4019 |
| OPT-03 | Général | Désactiver le rapprochement | `[LAYOUT] DisableR` (écrit `disableR`) | bool | 0 | opt.pas:289 ; main:918 |
| OPT-04 | Général | Ignorer le filtrage lors du calcul du solde | `[FILTER] IgnoreFilter` | bool | 0 | opt.pas:283 ; main:916 |
| OPT-05 | Général | Saisir les données dans une fenêtre séparée | `[LAYOUT] SeparateInput` | bool | 0 | opt.pas:286 ; main:952 |
| OPT-06 | Général | Désactiver la fonction 'autocomplete' | `[LAYOUT] DisableAutocomplete` | bool | 0 | opt.pas:290 ; main:928 |
| OPT-07 | Général | Saisie des montants saisis en centimes | `[LAYOUT] EvalCents` | bool | 0 | opt.pas:294 ; main:943 |
| OPT-08 | Général | Vider les champs après l'ajout d'une ligne | `[LAYOUT] EmptyFields` | bool | 0 | opt.pas:287 ; main:953 |
| OPT-09 | Général | Sélectionner le champ de saisie lors d'un clic sur le tableau | `[APPLICATION] FocusAuto` | bool | 0 | opt.pas:279 ; main:931 |
| OPT-10 | Général | Sauvegarder la colonne de tri | `[FILTER] RememberSortCol` | bool | 0 | opt.pas:281 ; main:933 |
| OPT-11 | Général | Sauvegarder les filtrages | `[FILTER] RememberFilters` | bool | 0 | opt.pas:282 ; main:966 |
| OPT-12 | Sauvegarde | Sauvegarder automatiquement le fichier à la fermeture | `[SAVE] Autosave` | bool | 0 | opt.pas:273 ; main:1775 |
| OPT-13 | Sauvegarde | Dossier de stockage par défaut des fichiers comptes (*.bp) + parcourir | `[SAVE] PathSave` | chemin (avec `\` final) | Documents | opt.pas:128, 270 ; main:888 |
| OPT-14 | Sauvegarde | Dossier de stockage des sauvegardes (*.bp0) + parcourir | `[SAVE] PathBackup` | chemin | `PathAppData` | opt.pas:129, 271 ; main:2581 |
| OPT-15 | Sauvegarde | Dossier d'export (pour les rapports PDF, tableaux XLS etc.) + parcourir | `[SAVE] PathExport` | chemin | Documents | opt.pas:130, 272 (utilisé par les scripts) |
| OPT-16 | Sauvegarde | Sauvegarde en ligne (case) | `[FTP] ftpSave` | bool | 0 | opt.pas:148, 296 |
| OPT-17 | Sauvegarde | Adresse FTP | `[FTP] ftpURL` | chaîne (préfixe `ftp://` et `/` final retirés) | '' | opt.pas:132, 297, 323-333 |
| OPT-18 | Sauvegarde | Login | `[FTP] ftpLogin` | chaîne | '' | opt.pas:133, 298 |
| OPT-19 | Sauvegarde | Mot de passe | `[FTP] ftpPassword` | chiffré `28O673`+hex (§28.6) | '' | opt.pas:134, 299 |
| OPT-20 | Sauvegarde | Sous-dossiers | `[FTP] ftpFolders` | chaîne normalisée `/x/` | '' (options) / '/' (envoi) | opt.pas:135, 300, 336-346 ; main:4301 |
| OPT-21 | Sauvegarde | Lettre du lecteur pour la sauvegarde sur disque externe | `[SAVE] FloppyDrive` | 1 car. | A | opt.pas:127, 269 |
| OPT-22 | Affichage | Format des dates (+ aperçu en direct + aide) | `[LAYOUT] DateFormat` | format Delphi | `ddd dd-mm-yy` | opt.pas:149, 288, 348-379 ; main:654, 915 |
| OPT-23 | Affichage | Affichage des montants : Une colonne (montant) / 2 colonnes (débit / crédit) / 2 colonnes (montant / solde) / 3 colonnes (débit / crédit / solde) | `[LAYOUT] ValueMode` | 1..4 | 2 (1 si `SingleValueCol=1`) | opt.pas:145, 285 ; main:948-950 |
| OPT-24 | Affichage | Masquer les infobulles : Colonne "tiers" (détails) | `[LAYOUT] MaskHints` | bool | 0 | opt.pas:291 |
| OPT-25 | Affichage | … Colonne "montant" (conversion) | `[LAYOUT] MaskValueHints` | bool | 0 | opt.pas:292 |
| OPT-26 | Affichage | … Colonne "pointage" (soldes) | `[LAYOUT] MaskTotalHints` | bool | 0 | opt.pas:293 |
| OPT-27 | Couleurs et police | 14 couleurs (§20) | `[COLORS] clr_*` | entier `TColor` (BGR) | cf. §20 | opt.pas:417-433 |
| OPT-28 | Couleurs et police | Police | `[LAYOUT] DefaultFont` | nom | Tahoma | opt.pas:634-639 ; main:954 |
| OPT-29 | Couleurs et police | Styles prédéfinis (charger/sauver/supprimer) | `globalcolors.ini` / `localcolors.ini` | — | — | §20 |

- [ ] **OPT-30 — Aide sur le format de date** : « dd : jour / mm : mois en chiffres / mmm : mois en lettres (abrév.) / mmmm : mois en lettres / yy : année (2 chiffres) / yyyy année / ddd : jour de la semaine (abrév.) / dddd : jour de la semaine » — `unitOptions.pas:372-379`. Les `m` du format sont convertis en `M` au chargement (sans effet sur `FormatDateTime`) — `unitMain.pas:915`, `unitOptions.pas:311`.
- [ ] **OPT-31 — Choix d'un dossier** via la boîte Windows « Choisissez un dossier » ; changer le dossier `.bp` rafraîchit la liste d'accueil — `unitOptions.pas:172-241`, `380-384`. **[W]**
- [ ] **OPT-32 — Application** : redimensionnement, remplissage, mise à jour de la zone de saisie, filtres de pointage ajustés si rapprochement désactivé — `unitOptions.pas:302-312`.

### 19.2 Clés INI non modifiables dans l'interface (lues ou écrites par le cœur)

| Clé | Rôle | Défaut | Réf. |
|---|---|---|---|
| `[APPLICATION] Licence` | clé de licence | '' | main:1325, 5406 |
| `[APPLICATION] helpfile` | fichier d'aide local | `bp.htm` | main:2997 |
| `[LAYOUT] ShowTotalPanels` | panneau des soldes (menu) | 1 | main:897 |
| `[LAYOUT] LockLines` | verrou (menu/barre d'outils) | 0 | main:901 |
| `[LAYOUT] ShowPrevisions` | solde prévisionnel (menu) | 1 | main:909 |
| `[LAYOUT] ShowDetails` | affichage 2 lignes (menu F4) | 1 | main:912 |
| `[LAYOUT] SingleValueCol` | héritage : défaut de `ValueMode` | 0 | main:948 |
| `[LAYOUT] ForceFontSize` | taille de police forcée (−1 = 8 pt) | −1 | main:5879 |
| `[LAYOUT] PrintFont` | police d'impression | Verdana | UnitPrint.pas:187 |
| `[FILTER] SortCol`, `SortAscending` | tri (écrits à chaque clic d'en-tête) | 0 / 0 | main:934-935, 2214-2215 |
| `[FILTER] HideOldLines` | lignes inutiles (menu Ctrl+H) | 0 | main:942, 5798 |
| `[FILTER] FilterDate1`, `FilterDate2` | bornes de date mémorisées (série TDateTime, 0 = inactif) | 0 | main:968-975, 1754-1759 |
| `[FILTER] FilterMode`, `FilterTiers`, `FilterCateg`, `FilterMontant` | textes de filtre mémorisés | '' | main:978-981 |
| `[FILTER] FilterMark` | filtre de pointage mémorisé | 4 | main:982 |
| `[SAVE] LastBackupNumber` | compteur des `.bp0` (1..20) | 0 | main:730-734 |
| `[SAVE] LastOnlineBackupNumber` | compteur FTP | 0 | main:4286 |
| `[SAVE] LastFloppyBackupNumber` | compteur disque externe | 0 | main:4618 |
| `[WINDOW] maximized`, `Left`, `Top`, `Width`, `Height` | géométrie (écrite à la fermeture) | 0 / 0 | main:1302-1314, 1744-1751 |
| `[WINDOW] Col0`..`Col4` | largeurs de colonnes | 111,20,35,18,25 | main:882-886, 5324, 6803 |
| `[WINDOW] Scaled` | mise à l'échelle VCL des fenêtres | 0 | main:4599, 5024 |
| `[COLORS] …` | 14 couleurs | §20 | main:1272-1273 |
| `[FormCategsManage]`, `[FormPeople]` : `Maximized`, `Width`, `Height`, `Left`, `Top` | taille des fenêtres (position écrite mais non relue) | — | UnitStrings.pas:514-543 |

- [ ] **INI-01 — Clés présentes dans les INI fournis mais non lues par le cœur** (scripts, héritage) : `[MARKET] url/start_position/left_bound/right_bound`, `[CURRENCY] url/currency_name1/currency_name2`, `[FTP] PassiveFTP/ReadTimeOut/Proxy*`, `[HTTP] BasicAuthentication/Proxy*`, `[LAYOUT] AutoCalc`, `[FILTER] SingleValueCol/FilterTack`, `[LOAN] …`, `[CurrenciesData] …`, sections `form_*`, `[formCategsTotals]`, `[formLoan]`, `[formGraph]` — `bankperfect_setup.ini`, `bankperfect.ini`. À préserver lors de la réécriture de l'INI (les scripts les utilisent).
- [ ] **INI-02 — `bankperfect_setup.ini`** (INI initial installé) : `[APPLICATION] helpfile=bp.htm`, `[MARKET]`, `[FTP]` proxy, `[HTTP]` proxy, `[LAYOUT] ForceFontSize=-1` (avec commentaire `;`).
- [ ] **INI-03 — Écriture** : toutes les écritures passent par l'objet mémoire et sont persistées à la fermeture (§START-05).

---

## 20. Couleurs, styles et police

- [ ] **COL-01 — 14 couleurs** (`ColorNames`, `ColorDescr`, `ColorDefau`) — `unitOptions.pas:110-113`. Valeurs `TColor` = `$00BBGGRR` :
  | # | Clé | Libellé | Défaut (TColor) | RVB | Usage |
  |---|---|---|---|---|---|
  | 0 | clr_line1 | Lignes paires | $00FFFFFF | #FFFFFF | fond lignes impaires (1, 3…) de la grille et listes |
  | 1 | clr_line2 | Lignes impaires | $00EFE8DC | #DCE8EF | fond lignes paires |
  | 2 | clr_line1_text | Texte des lignes paires | 0 | #000000 | texte lignes impaires |
  | 3 | clr_line2_text | Texte des lignes impaires | 0 | #000000 | texte lignes paires |
  | 4 | clr_line_sel | Lignes sélectionnées | $00AA8F77 | #778FAA | fond sélection |
  | 5 | clr_line_sel_text | Texte des lignes sélectionnées | 0 | #000000 | texte sélection |
  | 6 | clr_input_panel | Panneau de saisie | $00EBDCCA | #CADCEB | fond zone de saisie |
  | 7 | clr_total_panel | Panneau des soldes | $00EBDCCA | #CADCEB | fond panneau des soldes et sélection |
  | 8 | clr_total_panel_text | Texte du panneau des soldes | 0 | #000000 | libellés et soldes positifs |
  | 9 | clr_negative_values | Montants négatifs | $000000FF | #FF0000 | montants/soldes négatifs de la grille |
  | 10 | clr_tack1 | Coches de pointage | $00808040 | #408080 | coche P |
  | 11 | clr_tack2 | Coches de rapprochement | 0 | #000000 | coche R |
  | 12 | clr2_total_panel_text | Solde négatif | $000000C8 | #C80000 | soldes négatifs du panneau |
  | 13 | clr_details_text | Texte du détail des opérations | $00808080 | #808080 | détail en mode 2 lignes |
  (Une ancienne palette est conservée en commentaire `unitOptions.pas:112`.)
- [ ] **COL-02 — Modification** : clic sur une couleur → sélecteur de couleur → appliqué et écrit immédiatement — `unitOptions.pas:417-433`, `unitMain.pas:5451-5474`.
- [ ] **COL-03 — Styles** : `globalcolors.ini` (dans `PathExe`, fourni, lecture seule) + `localcolors.ini` (dans `PathAppData`, utilisateur) ; format `Nom=Police;c0;c1;…;c13` (la police est optionnelle : reconnue si le 1ᵉʳ élément n'est pas un nombre ; couleurs manquantes → défauts ; liste vide → Tahoma + défauts) — `unitOptions.pas:486-627`.
  - « >> » (Sauvegarder comme nouveau style) : nom demandé (saisie restreinte), écrit police courante + 14 couleurs dans `localcolors.ini` ;
  - « << » (Charger le style sélectionné) : applique police et couleurs, écrit dans `[COLORS]` ;
  - « Supprimer le style » : uniquement dans `localcolors.ini`, sinon « Cet élément ne peut pas être supprimé ».
  - Styles fournis : Classique (vide = défauts), Classique Trebuchet, Classique Old v6, Chantal, Sista Sly, Pastel jaune, Pastel vert, Pastel rouge, Console, Inversé, Fraise pistache. **[Q]** dans `globalcolors.ini`, la ligne « Pastel rouge » est collée à « Bleu royal » (fin de ligne manquante) : « Bleu royal » n'apparaît pas comme style et « Pastel rouge » a des valeurs parasites ; « Inversé » contient une couleur −1.
- [ ] **COL-04 — Police** : liste des polices système ; clic = appliquée et écrite immédiatement ; repli si absente : Segoe UI → Tahoma → Arial → Verdana → MS Sans Serif ; taille 8 pt sauf `ForceFontSize` ; appliquée à la fenêtre, au panneau des soldes et à l'écran d'accueil ; les fenêtres modales héritent de la police principale — `unitMain.pas:5866-5920`, `4593-4604`.

---

## 21. Calendrier (`UnitCalendar.pas`)

- [ ] **CAL-01 — Ouverture** par l'icône à droite de chaque champ date (saisie, filtre Du/Au, prévisionnel) ; positionné sous le champ (au-dessus s'il déborde de la zone de travail) ; cliquer à nouveau sur l'icône le ferme ; icône « enfoncée » de 1 px au clic — `unitMain.pas:6217-6248`, `UnitCalendar.pas:298-324`.
- [ ] **CAL-02 — Grille** 6 semaines × 7 jours, **lundi en premier** (en-têtes Lu Ma Me Je Ve Sa Di), jours sur 2 chiffres ; jours des mois voisins en gris, cliquables (sélectionnent le mois voisin) ; jour sélectionné en rouge `$CC` si dans le mois de la date d'origine ; aujourd'hui en gras ; février non bissextif commençant un lundi → une ligne entière du mois précédent en tête — `UnitCalendar.pas:238-296`.
- [ ] **CAL-03 — Navigation** : flèches gauche/droite = mois précédent/suivant (jour borné à la fin du mois) ; titre « Mois aaaa » (initiale majuscule) cliquable = **aujourd'hui** et fermeture ; Échap ferme ; perte de focus ferme sans changement — `UnitCalendar.pas:68-104`, `369-372`.
- [ ] **CAL-04 — Cellules** façon bouton (relief inversé au survol), fond `$00F8EFE9` — `UnitCalendar.pas:164-186`, `328-342`.
- [ ] **CAL-05 — Bornes** années 1900..9999.

---

## 22. Impression et aperçu (`UnitPrint.pas`, `Preview.pas`)

- [ ] **PRN-01 — Fenêtre « Impression »** : aperçu (unités twips), « Zoom: » 50 % / 100 % / 150 % / 200 % / 500 % / Largeur de la page / Hauteur de la page / Page entière ; onglets « Page N » ; case « Noir et blanc » (régénère) ; « Imprimer... » (boîte d'impression avec plage de pages) ; « Fermer » — `UnitPrint.dfm`, `UnitPrint.pas:60-127`, `420-456`.
- [ ] **PRN-02 — Contenu** (compte courant, lignes **affichées** dans l'ordre affiché) — `UnitPrint.pas:155-342` :
  - cadre arrondi d'en-tête ; logo bancaire à droite (converti en BMP temporaire `%TEMP%\bp_logo.bmp`, hauteur 700 twips) ;
  - « BankPerfect - Compte <nom> (n° <numéro>) » (gras, 200) ; « Fichier <chemin> » (150) ;
  - ligne des totaux « Pointé : X   A venir : Y   Solde : Z » ;
  - bloc « Filtrages: » : « Mode de paiement: … », « Tiers: … », « Catégorie: … », « Montant: … », « Opérations non pointées » (dès que le filtre de pointage ≠ 4, quel qu'il soit), « du X au Y » / « depuis le X » / « avant le Y » ;
  - tableau : colonnes (twips) Date 1000-2700, Mode 2700-4000, Tiers 4000-6500, Catégorie 6500-8100, Dépense 8100-9300, Recette 9300-10500, P 10500-10900 ; en-têtes centrés « Date » (ou « Ptge »), « Mode », « Tiers », « Categ », « Dépense », « Recette », « P » ; débits sans signe en rouge (sauf N&B), crédits ; P = « P », « R » ou vide ; fonds alternés (couleurs de l'interface ou gris `$F0F0F0`/blanc en N&B) ; en mode 2 lignes, détail sous le tiers et sous-catégorie sous la catégorie (ligne double) ; indépendant de `ValueMode` ;
  - police `[LAYOUT] PrintFont` (Verdana), taille auto (plus petite taille ≥ 6 pour laquelle « VEN 17-09-2002 » mesure ≥ 1000 twips) ;
  - saut de page quand y > hauteur − 1500 ; chaque page : cadre arrondi inséré de 700, « Page N » en bas à droite ; à partir de la page 2, ligne des totaux en haut ;
  - titre du travail d'impression « BankPerfect - relevé de comptes ».
- [ ] **PRN-03** — **[W]** dépend de l'imprimante par défaut Windows pour le format de page.

---

## 23. Génération PDF (`UnitPDF.pas` + `tnpdf.pas`)

Utilisé par l'API (scripts, ex. RIB, reçus) ; le cœur ne l'appelle pas directement.

- [ ] **PDF-01 — Mini-langage** `CreatePDF(code)` : une commande par ligne `commande param1;param2;…` (échappements `\\`, `\s`=`;`, `\r`, `\n`) — `UnitPDF.pas:13-94` :
  - `create titre;fichier;largeur;hauteur` (police Helvetica 20) ; `newpage` ; `curry y` ; `pencolor c` ; `linewidth w` ; `drawline x1;y1;x2;y2` ; `drawrectangle x1;y1;x2;y2` ; `fillrect x1;y1;x2;y2;couleur` ; `fontname` parmi Helvetica, Helvetica-Bold, Helvetica-Oblique, Helvetica-BoldOblique, Courier(-Bold/-Oblique/-BoldOblique), Times-Roman, Times-Bold, Times-Italic (défaut Helvetica) ; `fontsize n` ; `textout x;y;texte` ; `textrect x;y;largeur;texte` ; `drawimage x;y;l;h;chemin`.
- [ ] **PDF-02** — polices standard PDF Type 1, fin de document à la fin du code.

---

## 24. Graphiques (`AACharts.pas`)

Utilisé par l'API (scripts « Graphiques », simulateur de crédit, budget…) ; aucun appel direct dans l'interface du cœur.

- [ ] **CHART-01 — Types** : barres, barres + ligne, empilé, points, ligne, aires, secteurs (camembert) — `AACharts.pas:22`, `1269-1286`.
- [ ] **CHART-02 — Données** : liste de libellés + valeurs ; un libellé préfixé `/s=` démarre une **série** (multi-séries → une couleur par série) — `AACharts.pas:120-132`, `1226-1259`.
- [ ] **CHART-03 — Options** : afficher les valeurs nulles, relief (« bevel », 20 passes pour les secteurs), couleur de fond, format de légende (défaut `%0:s (%1:.2n)`), masquer la légende, bordures, anticrénelage 0..4, valeur minimale de l'axe Y — `AACharts.pas:61-66`, `1211-1292`.
- [ ] **CHART-04 — Légende** à droite, numérotée si non-secteurs, police Tahoma 8 — `AACharts.pas:362-430`.
- [ ] **CHART-05 — Palette** de 50 couleurs semi-transparentes (alpha $7F) dans un ordre fixe (rouge, bleu, rose, bleu ciel, jaune pâle, mauve, vert monnaie, vert, marine, …) — `AACharts.pas:196-248`.
- [ ] **CHART-06 — Infobulles** au survol des zones (barres, points, secteurs) ; redessin au redimensionnement — `AACharts.pas:272-294`.

---

## 25. Champs supplémentaires, UId, icônes libres, préfixes spéciaux

- [ ] **EXTRA-01 — Stockage** : chaîne `clé=valeur;clé=valeur` par opération ; valeur : `;` → `<fab_pv>`, retours ligne → `<fab_cr>` ; dans le fichier `;` → `<||>` (doc 02) — `unitMain.pas:6028-6101`.
- [ ] **EXTRA-02 — Lecture** `GetExtraValue(op, clé)` (décode `<fab_pv>`/`<fab_cr>`) ; clé vide → liste des clés séparées par `;` ; écriture `SetExtraValue` (valeur vide = suppression) ; la clé `UId` est **protégée** (non modifiable), seule la clé interne `__internal_BP_UId` permet de l'écrire.
- [ ] **EXTRA-03 — UId de ligne** : entier unique dans le fichier ; à l'ouverture, si **une seule** ligne n'a pas d'UId valide (> 0), **toutes** les lignes de tous les comptes sont renumérotées 1..N et `#sisterLineUid` est vidé partout ; le fichier est alors marqué modifié ; `last_line_id` = max ; nouvelle ligne / collage → `last_line_id + 1` — `unitMain.pas:4873-4874`, `4890-4938`, `3848-3852`, `2931-2932`. Affiché dans « Sélection : Ligne <UId> ».
- [ ] **EXTRA-04 — Icône libre** : clé `freeicon=<nom>` → `PathExe\Icons\<nom>.png` (nom insensible à la casse, recherche récursive au démarrage, extension `.png` exacte) dessinée dans la colonne Tiers — `unitMain.pas:5138-5182`.
- [ ] **EXTRA-05 — Clés réservées** : `UId`, `freeicon`, `#sisterAccUid`, `#sisterLineUid` ; toute clé commençant par `#` est cachée du panneau.
- [ ] **EXTRA-06 — Gestion manuelle** (ajout/modif/suppression, F3) : script `extra_values_edit.py` **[S]**.
- [ ] **EXTRA-07 — Préfixes de tiers** : `[auto] ` (échéance automatique, icône horloge), `working_guid` (ligne en cours d'ajout par un script, retiré après exécution et déclenchant les scripts `ExecuteWithNewLine`), `[xxxxx]` en tête (ventilation, ignoré au tri) — §GRID-16, §GRID-19, `unitMain.pas:6503-6520`.

---

## 26. Intégration des scripts et plugins (côté cœur) **[S]**

- [ ] **SCR-01 — Découverte des scripts** : tous les `*.ini` sous `PathExe\Scripts\` (récursif), section `[BANKPERFECT_SCRIPT]` : `Path` (relatif au dossier de l'ini, obligatoire, fichier existant), `Name` (obligatoire), `Icon` (n° d'image ou fichier .png), `Menu`, `DropDown`, `NeedsFile` (défaut **1**), `Index` (position dans le menu, −1 = fin), `ExecuteWithFile`, `ExecuteWithNewLine`, `ExecuteWithModifiedLine`, `Shortcut` (texte Delphi, ex. `Ctrl+Maj+T`), `ReadOnly`, `DisableScripts` (liste de noms de scripts à ignorer, 1ʳᵉ passe) — `unitMain.pas:6285-6431`.
- [ ] **SCR-02 — Placement** selon les mots contenus dans `Menu` : `toolbar` (bouton avec 1ᵉʳ mot du nom, infobulle = nom ; style déroulant si `DropDown`), `fichier`, `importer`, `exporter`, `édition`/`edition`, `affichage`, `comptes`, `operations`/`opérations`, `outils`, `plugins`, `internet`, `context`/`right` (menu contextuel), `?`/`help` ; `NeedsFile` → grisé sans fichier ouvert.
- [ ] **SCR-03 — Exécution manuelle** : entrée d'historique au nom du script (si `NeedsFile`), historique verrouillé pendant l'exécution, gestion des lignes ajoutées (`working_guid`), rafraîchissement ; erreur → « Le script X a provoqué une exception : … » — `unitMain.pas:6433-6525`.
- [ ] **SCR-04 — Déclencheurs automatiques** : à l'ouverture d'un fichier (`ExecuteWithFile`), après ajout (`ExecuteWithNewLine`) ou modification (`ExecuteWithModifiedLine`) d'une ligne — `unitMain.pas:4184-4214`.
- [ ] **SCR-05 — Menus déroulants de scripts** : le script est appelé avec le paramètre `populate_popup_menu` et renvoie une liste `libellé|raccourci;…` ; les raccourcis sont rendus actifs via des éléments invisibles du menu Edition — `unitMain.pas:4136-4182`, `7095-7099`.
- [ ] **SCR-06 — Scripts appelés nommément par le cœur** : `previsions.py` (solde prévisionnel), `backups.py` (récupération après corruption) — `unitMain.pas:7408-7417`, `5662`, `7435-7446`.
- [ ] **SCR-07 — Données de scripts persistées dans le .bp** (`bp_script_data_<clé>`) : clé normalisée (`SimpleString` sans espaces), valeur encodée (caractères hors liste → `_hh`) ; `SetData` marque le fichier modifié — `ObjectScriptsData.pas` (format : doc 02).
- [ ] **SCR-07b** — **[Q]** (API, pour mémoire) `BankPerfectNewFile` appelle `CreateBankPerfectFile` avec le chemin et le nom d'utilisateur inversés — `UnitPlugins.pas:454-456`.
- [ ] **SCR-08 — Plugins DLL** : `PathExe\Plugins\**\*.dll` (hors `borlndmm.dll`) accompagnés d'un `plugin.ini` (`Name`, `Path` = chemin de sous-menu séparé par `>`, `Hidden=1`, `ReadOnly=1`) et d'une icône `<dll>.png` ; la DLL doit exporter `Execute` ; clic → appel de `Execute(interface BankPerfect)`, puis rafraîchissement sauf `ReadOnly` ; erreur → « Le plugin X a provoqué une exception : … » — `unitMain.pas:6106-6159`, `6527-6599`. **[W]** (probablement abandonné).

---

## 27. Boîtes de dialogue et retours visuels génériques

- [ ] **DLG-01 — `MsgBox`** : fenêtre modale maison (titre = « BankPerfect », icône de l'application, texte avec retour à la ligne limité à la moitié de l'écran en largeur et 3/4 en hauteur, boutons issus d'une liste séparée par des virgules, largeur ≥ 70 px, renvoie l'index du bouton ; Échap/croix → index « annulation » fourni) — `Diags.pas:26-120`.
- [ ] **DLG-02 — `EnterText`** : boîte de saisie modale (titre = question, OK/Annuler, Entrée = OK, texte présélectionné) ; masques : `''` → caractères autorisés lettres, chiffres, espace, `à`..`ü`, `-`, `_`, `'` uniquement ; `MFloat` → chiffres, `-`, `,`, `.` (point converti selon `Sep1`) ; `MInteger` → chiffres ; `MALL` → libre ; `MPass` → libre masqué ; autre → masque de saisie Delphi — `UnitInput.pas:37-68`.
- [ ] **DLG-03 — Bulles d'alerte** (`ShowAlert`) : libellé Verdana 7 gras avec icône, sous le contrôle visé, 3 s (5 s pour les aides), sauf sous le montant (reste jusqu'à la sortie) ; une seule à la fois — `unitMain.pas:4748-4789`.
- [ ] **DLG-04 — Infobulles « flat »** (`TFlatHint`) : fond blanc, bordure noire, flèche sur fond `$E0D0CC`, largeur max 200 — `ObjectFlatHint.pas`, `unitMain.pas:1236-1237`.
- [ ] **DLG-05 — Fenêtres modales** : police héritée de la principale, mise à l'échelle selon `[WINDOW] Scaled` — `unitMain.pas:4593-4604`.

---

## 28. Fonctions utilitaires et formats (`UnitStrings.pas`)

- [ ] **UTIL-01 — Montants** : type `Currency` Delphi (virgule fixe 4 décimales, entier 64 bits ×10 000) ; affichage `%.<Precision>n` (séparateur de milliers espace, séparateur décimal système), avec devise `"%.<P>n <devise>"` — `unitMain.pas:4274-4277`.
- [ ] **UTIL-02 — `Str2Currency`** : ne garde que `- . , 0-9`, essaie `,` puis `.` comme séparateur décimal, valeur par défaut ou exception — `UnitStrings.pas:420-458`. **[Q]** la suppression du séparateur de milliers quand `,` et `.` coexistent est inopérante (résultat de `FastReplace` ignoré) → « 1.234,56 » est invalide ; `Str2Extended` le fait correctement — `UnitStrings.pas:460-498`.
- [ ] **UTIL-03 — `Split` / `Merge`** sur `;` (pas d'échappement) — `UnitStrings.pas:374-401`.
- [ ] **UTIL-04 — Normalisations** : `SimpleString` (garde `,`), `SimpleString2` (`,` → espace), `KeepOnlyDigits`, `KeepOnlyAlphaNum`, `KeepOnlyAlphaNumSpace` (tables de 256 entrées CP1252) — `UnitStrings.pas:545-690`.
- [ ] **UTIL-05 — Codec v5** (`EncryptCodec5`/`DecryptCodec5`, `LoadFileV5`/`SaveFileV5`) : utilisé pour les `.bp` et `identity.dat` — `UnitStrings.pas:135-227` (détail : doc 02).
- [ ] **UTIL-06 — Chiffrement du mot de passe FTP** : sans objet (sauvegarde FTP abandonnée) ; le procédé n'est pas documenté ici.
- [ ] **UTIL-07 — Divers** : `Color2HTML` (BGR → `#RRGGBB`), `Slice`, `FPos` (position de la n-ième ou dernière occurrence), `Str2Date`, `IsInteger`, `bp_guid` (identifiant aléatoire a-z0-9), `OpenTextFile`/`SaveTextFile` (octets bruts), `AppendToFile`, `Log` (`bp.log`), `GetCRLF`, `checkUTF8` (non utilisé) — `UnitStrings.pas`.
- [ ] **UTIL-08 — Dates** : `TDateTime` (jours depuis le 30/12/1899) ; affichage `FormatDateTime(DateFormat)` ; `DateToStr` → `dd/mm/yy`.
- [ ] **UTIL-09 — Code mort** à ne pas reproduire mais à connaître : `op_auto_suivante` (intervalles d'échéances 1-14 j, 30/60/90/180/360 → mois), `correctDate`, `Webdate`, `ValidateOperation`, `AEvalExpression`, `ReadFirstLine`, `GetWinVersion`, `code.pas` (liste d'affectations de traduction d'une ancienne version, mentionnant des fonctions passées en scripts) — `unitMain.pas:1905-1912`, `5329-5341`, `4673-4677`, `4988-4994`, `4547-4554`, `5516-5534`, `7283-7314`, `code.pas`.

---

## 29. Fonctions cachées (débogage)

- [ ] **DBG-01 — Décodage d'un fichier** : avec la date de saisie **28/06/1973**, Ctrl+Maj+clic sur le solde « Pointé » → choix d'un fichier (.bp/.bp0 en V5, UCL ou Shuffle) → écrit `<fichier>.txt` en clair — `unitMain.pas:2153-2170`.
- [ ] **DBG-02 — Encodage d'un fichier** : même date, Ctrl+Maj+clic sur le solde total, sur les champs supplémentaires ou sur les totaux de sélection → choix d'un `.txt` → écrit `<fichier>.txt.bp` au format V5 — `unitMain.pas:4564-4582`.

---

## 30. Mise à jour en ligne, téléchargements, e-mail (abandonnés)

- [ ] **NET-01 — Mettre à jour BankPerfect** : ferme le fichier, télécharge `http://www.chelly.net/history.txt`, lit la version (caractères 9-12), compare à la version de l'exécutable, propose « La version X est disponible ! » ou « Votre version (X) est la plus récente… installer tout de même ? » ; télécharge `bp.exe` dans `%TEMP%`, le lance avec `/S` et quitte — `unitMain.pas:4448-4453`, `4499-4523`. **Abandonné** (à ne pas reproduire, noter seulement).
- [ ] **NET-02 — `UnitDownload`** : fenêtre de progression pour FTP (get/put/list avec analyse des listings Unix/Windows) et téléchargement HTTP ; utilisée par FTP, mise à jour et API — `UnitDownload.pas`.
- [ ] **NET-03 — Envoi d'e-mail** (`SendByMail`, API) : réservé aux licenciés (sinon message anti-spam) ; pièce jointe envoyée par FTP sur le serveur de l'auteur sous un nom horodaté aléatoire, puis requête HTTP POST vers `send_mail.php` avec contrôle `guid`+`crc` ; ajoute « Message envoyé depuis BankPerfect <version> / Licence n°… » — `unitMain.pas:5593-5650`, `7230-7281`. Identifiants de serveur codés en dur dans la source (ne pas réutiliser). Dépend d'un service tiers : **abandonné**.
- [ ] **NET-04 — `LoadURL`/`PostData`** (HTTP GET/POST, API) — `unitMain.pas:4455-4497`.

---

## 31. Fonctionnalités du manuel / de l'historique réalisées par des scripts (renvoi)

Pour mémoire (à couvrir par l'inventaire des scripts) : import CSV/QIF/OFX, exports HTML/QIF/XLS/CSV/XML, échéancier (et insertion automatique, `[auto] `), transfert de compte à compte, conversion d'une ligne en transfert, ventilation (F6), champs supplémentaires (F3), modification d'un groupe d'opérations (Ctrl+M), remplacer (Ctrl+R), saisie multiple (F2), filtres prédéfinis (F7, valeurs dynamiques `j m a dj`), graphiques, totaux par catégorie (Ctrl+T), totaux par tiers, total des comptes, relevé multi-comptes, budget / budget annuel, solde prévisionnel, simulateur de crédit (`loan.dat`), conversion de devises (zone euro / en ligne), téléchargement des cotations boursières, reçu de remboursements, impression de RIB (PDF), liste/restauration des sauvegardes, sauvegarde par e-mail, calculatrice, conversions numériques, vérification de carte bancaire (Luhn), notes, calcul automatique des champs, gestion des plugins, téléchargement du manuel.

---

## 32. Dépendances Windows et équivalents multiplateformes

| Élément Delphi/Windows | Usage | Équivalent proposé (Rust/egui) |
|---|---|---|
| `CSIDL_PERSONAL`, `CSIDL_APPDATA` | dossiers par défaut | crate `directories`/`dirs` (`document_dir`, `config_dir()/BankPerfect`) ; conserver le mode `portable` |
| `TMemIniFile` | `bankperfect.ini`, `globalcolors.ini` | parseur INI préservant ordre, commentaires et clés inconnues, insensible à la casse, encodage CP1252 |
| `ShellExecute` | URL, fichiers, mailto | crate `open` / `webbrowser` |
| Lecteur `A:`, `DiskInDrive`, `SetErrorMode` | disque externe | « copie vers un dossier de sauvegarde externe » choisi par l'utilisateur ; supprimer la détection de disquette |
| `CopyFile`/`MoveFile`/attribut lecture seule | sauvegardes | `std::fs` (rename atomique, `metadata().permissions()`) |
| `GetFileVersionInfo` | version, à propos | `env!("CARGO_PKG_VERSION")` |
| `ExtractShortPathName` | comparaison de chemins | `std::fs::canonicalize` |
| `GetTempPath` | logo d'impression, mise à jour | `std::env::temp_dir` |
| Locale Windows (séparateur décimal, noms de jours/mois) | affichage et saisie | paramètres explicites (fr-FR par défaut : `,`, espace, noms français) |
| Encodage ANSI CP1252 | fichiers .bp, INI, export | `encoding_rs::WINDOWS_1252` |
| VCL `TDrawGrid`, menus, `TActionList`, raccourcis | interface | egui (`egui_extras::TableBuilder`, menus, gestion clavier) ; vérifier Ctrl+Tab (capturé par egui pour le focus) |
| Wingdings pour masquer le mot de passe | saisie mot de passe | `TextEdit::password(true)` |
| `TPrintPreview`, `Printers`, twips | impression | génération PDF (ex. `printpdf`/`genpdf`) + aperçu + impression système (`lpr`/`ShellExecute print`) |
| `tnpdf` | PDF des scripts | même crate PDF, conserver le mini-langage |
| Graphics32 (`AACharts`) | graphiques | `egui_plot` ou rendu `tiny-skia` |
| Python4Delphi (`PyEval`) | calculatrice du montant | évaluateur d'expressions Rust (`+ - * / ()` , priorités, décimaux) |
| ICS (FTP/HTTP) | FTP, mise à jour, e-mail | abandonner ou `suppaftp`/`ureq` |
| DLL plugins (`LoadLibrary`) | menu Plugins | abandonner (ou système de plugins dynamique séparé) |
| `WM_SYSCOMMAND $F012` | déplacer le panneau de saisie séparé | `egui::Window` déplaçable |
| `GetSystemMetrics(SM_CXVSCROLL)` | calcul des colonnes | largeur de barre de défilement egui |
| Boîte « Parcourir les dossiers » | options | crate `rfd` |
| `SetErrorMode`, `XPManifest`, `PopupListEx` (messages de fermeture de menu) | confort Windows | sans objet |

---

## 33. Récapitulatif des points d'incertitude et questions ouvertes

1. **Bugs d'origine à arbitrer** (reproduire ou corriger ?) : dernier compte vide perdu au rechargement et précision 0 perdant les centimes (§FILE-16b) ; `UserName` hérité du fichier précédent (§FILE-10) ; suppression via la date sentinelle 28/06/1973 (§EDIT-10) ; perte du mode perso supprimé → « Carte » (§MODE-04) ; `m=` dans le détail imposant le mode (§IN-29) ; `=carte` inopérant et `//` inopérant sur le tiers (§FILT-01) ; F10 depuis une opération chèque → « Retrait DAB » et catégorie par identifiant (§PEO-10) ; mot de passe maître `ch280673` et comparaison insensible à la casse (§PASS-02) ; `Str2Currency` (§UTIL-02) ; boucle potentiellement infinie de `SecuritySave` (§FILE-14) ; `Sep2` non initialisé hors locale `,` (§START-07) ; restauration `.bp0` dans le dossier des sauvegardes (§FILE-06).
2. **Écarts manuel ↔ code** : nombre de sauvegardes (20 fixes vs « 30 paramétrables »), règle des lignes inutiles, confirmation du mot de passe, vérification à l'import de catégories, sens de `(`/`)` dans les filtres. La parité vise le **code**.
3. **Locale** : le rendu `ddd` et `%n` dépend de la locale Windows de l'utilisateur ; supposé fr-FR — à confirmer.
4. **Tri instable** (QuickSort) : l'ordre des ex-æquo, donc l'ordre d'écriture des lignes dans le `.bp`, peut différer ; impact sur la parité octet pour octet à évaluer avec le doc 02.
5. **Raccourcis en conflit** : F9 (FTP) et script « Sauvegarder par e-mail » ; décider lequel l'emporte.
6. **Fonctions abandonnées** (mise à jour en ligne, e-mail, FTP, disquette, plugins DLL) : confirmer la liste de ce qui est volontairement non porté.
7. **Fenêtres `[FormCategsManage]`/`[FormPeople]`** : seules taille et état maximisé sont restaurés.
