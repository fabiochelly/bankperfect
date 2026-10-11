# 03 — API Python hôte (scripts BankPerfect) : spécification contractuelle

> Périmètre : tout ce qu'un script Python de `Scripts/` peut voir ou faire, et tout ce que l'hôte Delphi fait autour (découverte, menus, hooks, persistance, formulaires). Objectif : que les 43 scripts existants tournent **tels quels** (ou avec une conversion automatique 2→3 transparente) sous l'hôte Rust + Python 3 (PyO3).
>
> Sources analysées (CP1252, lues via `iconv`) : `dmPython.pas/.dfm`, `pyAccounts.pas`, `pyRun.pas`, `UnitPlugins.pas` (implémentation réelle de l'API !), `IBankPerfect.pas`, `UnitCustomForms.pas`, `ObjectScriptsData.pas`, `UnitPDF.pas`, `tnpdf.pas`, `AACharts.pas`, `UnitStrings.pas`, `ObjectCategs.pas`, `unitMain.pas`, `UnitDownload.pas`, `UnitBrowseFiles.pas`, `FormPython.pas` (variante morte), `plugins_bankperfect.htm`, `history.txt`, les 43 `.py`/`.ini` de `Scripts/`.
>
> Conventions : `fichier.pas:ligne` renvoie aux sources Delphi (numéros de ligne du fichier d'origine). « Doc » = `plugins_bankperfect.htm`. Lorsque le code et la doc divergent, **le code fait foi** (c'est lui que les scripts ont éprouvé) ; les divergences sont signalées par ⚠.

---

## 0. Vue d'ensemble en une page

```
 Menu / barre d'outils / hook ──► TFormMain.LoadScript / DirectLoadScript   (unitMain.pas:6433-6524)
                                        │  LoadedScript := chemin .py ; LoadedScriptName := Name (.ini)
                                        ▼
                                  pyRun.ExecutePythonScript(Plugins)         (pyRun.pas:20-54)
                                        │  crée un TPythonDM NEUF (= nouvel interpréteur python23.dll)
                                        │  Initialize : module BP + module __main__ (+ wrapper VCL)
                                        │  lit le .py (octets CP1252) et l'exécute dans __main__
                                        ▼
                         Script Python 2.3 ── import BP ──► dmPython (TPythonModule 'BP')
                                │                              └─► IBankPerfectPlugin (UnitPlugins.pas : TBankPerfectPlugin)
                                ├── BPEval("cmd:...") ─────────────► TBankPerfectPlugin.GetURL  (UnitPlugins.pas:674-934)
                                ├── CreateComponent("TForm", None) ► WrapDelphi/WrapDelphiVCL (VCL réelle)
                                └── CreateChart/DrawChart/...      ► AACharts (GR32) dans un TPanel
```

Points structurants :

1. **Un interpréteur neuf par exécution** : `TPythonDM.Create` → `Py_Initialize`, `PythonDM.Free` → `Py_Finalize` (`pyRun.pas:39,51`). Aucun état Python ne survit entre deux exécutions ; la persistance passe par `save_script_data` (dans le fichier .bp), `ini_write` (bankperfect.ini) ou des fichiers.
2. **Pas de réentrance** : si un script tourne déjà, tout autre lancement est silencieusement ignoré (`if PythonUsed then Exit`, `pyRun.pas:25`).
3. **L'API « BP » est une façade mince** sur l'interface `IBankPerfectPlugin` (la même que celle des plugins DLL abandonnés). Son implémentation `TBankPerfectPlugin` (`UnitPlugins.pas`) **n'est pas abandonnée** : c'est le cœur de l'API script à réimplémenter.
4. **Les formulaires des scripts sont de la vraie VCL** pilotée par Python via `CreateComponent` (Python4Delphi `TPyDelphiWrapper` attaché au module `__main__`, `dmPython.dfm`). C'est de loin la plus grosse surface à émuler (cf. §4). L'ancien protocole texte `BP.CustomForm` existe aussi (§4.1).
5. **Les chaînes sont des octets CP1252** côté Python 2.3 (`str`), les sources `.py` aussi.

---

## 1. Architecture d'exécution

### 1.1 Moteur

| Élément | Valeur | Réf. |
|---|---|---|
| DLL | `python23.dll` chargée depuis le répertoire de l'exe (`PE.DllPath := ExtractFilePath(ParamStr(0))`) | `dmPython.dfm` (PE), `dmPython.pas:611-614` |
| Version API | `APIVersion = 1012`, `RegVersion = '2.3'`, `UseLastKnownVersion = False` | `dmPython.dfm` |
| Flags | `pfIgnoreEnvironmentFlag` (PYTHONPATH etc. ignorés) | `dmPython.dfm` |
| Console | `PE.UseWindowsConsole := ExecPythonOnly` (vrai seulement en mode ligne de commande) | `pyRun.pas:42` |
| Bibliothèque standard | **aucune Lib/** livrée : seuls les modules *built-in* de python23.dll (`sys`, `time`, `math`, `cPickle`, `nt`, `string`?, …) + `copy_reg.pyc` et `types.pyc` posés à côté de l'exe (nécessaires à `cPickle`) | racine du dépôt |
| Unité utilisée | `dmPython` (TDataModule). `FormPython.pas` est une **copie morte** (non référencée par `bankperfect.dpr`) qui n'expose pas `AccountIsDisabled` et renvoie des 12-uplets dans `Accounts()` | `bankperfect.dpr` |

### 1.2 `sys.path`

`PESysPathInit` (`dmPython.pas:599-609`) : supprime **tous les éléments sauf le dernier** de la liste par défaut, puis ajoute `ParamStr(0)` (chemin complet de **l'exe**, pas du dossier — sans effet utile).
Résultat : `sys.path == [<dernier élément par défaut de Python 2.3 embarqué (vraisemblablement le dossier de l'exe)>, "C:\...\bankperfect.exe"]`. Ni `''` ni le dossier du script ne sont dans le chemin : **un script ne peut pas importer un module voisin** sans modifier `sys.path` lui-même.

> Nouvel hôte : `sys.path` = stdlib Python 3 embarquée + dossier des shims de compatibilité (§5) ; ajouter le dossier du script en tête est un sur-ensemble sans risque.

### 1.3 Namespace d'exécution

* Le source est lu par `TStringList.LoadFromFile` (octets bruts, ANSI) et exécuté par `GetPythonEngine.ExecStrings(prog)` (`pyRun.pas:44-48`), i.e. `exec` en mode *file_input* dans le dictionnaire de `__main__`.
* `__name__ == "__main__"` ; **`__file__` n'est pas défini** (exécution d'une chaîne). Les scripts retrouvent leur chemin via `BP.BankPerfectPluginPath().split("|")[1]` (ex. `Budget/budget.py:79`).
* `sys.argv` : non garanti (dépend de Python4Delphi, non initialisé explicitement) — aucun script ne l'utilise.
* Noms injectés dans `__main__` par l'hôte **avant** l'exécution :
  * fonctions du `FormsModule` (module nommé `__main__`, `dmPython.dfm`) : `SingleLine`, `BPEval`, `CreateChart`, `DestroyChart`, `DrawChart`, `ExportChart` ;
  * tout ce que `TPyDelphiWrapper` (WrapDelphi + WrapDelphiVCL) enregistre dans `__main__` ; parmi cela, les scripts utilisent **uniquement** `CreateComponent`, `Rect`, `MB_YESNO`, `MB_ICONINFORMATION`, `MB_ICONQUESTION`, `IDYES` (§4.2.1).
* Le module `BP` doit être importé (`import BP`) — 26 scripts le font explicitement.

### 1.4 Répertoire courant

`pyRun.pas:37-38,52` fait `SetCurrentDir(BPPPath)` puis restaure l'ancien. Or `BPPPath` = `CurrentPluginPath` = chemin de la **dernière DLL de plugin exécutée** (vide au démarrage) : l'appel échoue (chemin vide ou fichier) et **le répertoire courant reste celui du processus**, c'est-à-dire le dossier de sauvegarde `[SAVE] PathSave` (fixé par `ChDir` au démarrage, `unitMain.pas:888-895`) ou celui où la dernière boîte de dialogue Windows l'a déplacé. ⇒ Le répertoire courant n'est **pas** fiable pour les scripts ; aucun script correct ne doit en dépendre.

> Nouvel hôte : ne pas changer le cwd du processus (état global) ; documenter que les chemins relatifs sont résolus par rapport au dossier de sauvegarde. Optionnel : shim `open()` résolvant les chemins relatifs par rapport au dossier du script.

### 1.5 Encodage

| Flux | Python 2.3 / Delphi | Conséquence Py3 |
|---|---|---|
| Source `.py` | octets CP1252 bruts exécutés sans décodage ; 5 scripts ont un cookie `# coding=iso-8859-1` (`Auto Fields/auto_fields.py:2`, `Loan/loan.py:2`, `MultipleInput/multiple_input.py:2`, `Totals/totals.py:2`) ; **aucun fichier n'est en UTF-8** | décoder en cp1252 par défaut ; traiter `iso-8859-1`/`latin-1` comme cp1252 (sur-ensemble pratique : `€`=0x80) ; respecter un BOM/cookie UTF-8 pour les futurs scripts |
| Chaînes Python → hôte | `str` (octets) → `AnsiString` sans conversion ; `WideString` (paramètres `IBankPerfectPlugin` sont `WideString`) via `CP_ACP` (= CP1252) | tout est `str` Unicode : l'hôte travaille en UTF-8/Unicode, plus de conversion |
| Hôte → Python | `VariantAsPyObject` sur `string`/`WideString` → `str` 8 bits CP1252 (les scripts concatènent librement avec des littéraux accentués, ce qui prouve que ce ne sont pas des `unicode`) | renvoyer `str` |
| Caractère NUL | tronque (conversion `PChar`) | — |

### 1.6 Gestion des erreurs

| Cas | Comportement observé | Réf. |
|---|---|---|
| Exception Python non rattrapée | P4D lève `EPythonError` → `MsgBox("Le script %s a provoqué une exception : %s")` avec le *Name* du script | `unitMain.pas:6441-6447`, `UnitRStrings.pas:224` |
| Exception Delphi dans `BPEval` ou une fonction `BP.*` (hors `GetURL`/`CustomForm`) | non interceptée par le wrapper → remonte à travers l'interpréteur → même boîte de message, script interrompu | `dmPython.pas:623-631` |
| Exception Delphi dans `BP.GetURL` | `BPMod.RaiseError('BPError', msg)` puis `PyErr_Clear` et retour `NULL` ⇒ côté Python : `SystemError` (« error return without exception set ») | `dmPython.pas:473-493` |
| Mauvais nombre d'arguments | `varArgs[i]` hors bornes → exception Delphi (abandon du script) ; `CustomForm` seul lève `IndexError("Not enough arguments")` | `dmPython.pas:450-471` |
| Arguments nommés | non supportés (METH_VARARGS) | — |
| `print` | en mode GUI la sortie standard n'est reliée à rien (pas de composant IO) : perdue ; en mode console (`ExecPythonOnly`) elle va dans la console | `pyRun.pas:42` |

> Nouvel hôte : lever une exception Python propre `BP.BPError` (sous-classe d'`Exception`) pour toute erreur hôte, et afficher la même boîte « Le script X a provoqué une exception : … » avec la trace. Rediriger `sys.stdout/stderr` vers un journal consultable.

### 1.7 Découverte des scripts (`TFormMain.LoadScripts`, `unitMain.pas:6285-6431`)

1. Appelée une fois au démarrage (`FormCreate`, `unitMain.pas:1246`), jamais rechargée ensuite.
2. `BrowseFiles(PathExe + 'Scripts\', sl, '.ini')` : parcours **récursif** de `Scripts\` (ordre FindFirst/FindNext, alphabétique sous NTFS), extension comparée **en respectant la casse** (`'.ini'`) (`UnitBrowseFiles.pas`).
3. **Première passe** : union de toutes les valeurs `DisableScripts` (séparées par `,`, sans suppression des espaces autour des virgules ; comparaison ultérieure par `TStringList.IndexOf`, **insensible à la casse**).
4. **Seconde passe**, pour chaque `.ini` (section `[BANKPERFECT_SCRIPT]`) :
   * ignoré si `Path` ou `Name` vide (c'est le cas de `Import OFX/mapping.ini`, simple fichier de données) ;
   * ignoré si `Name` figure dans la liste désactivée ;
   * `Path` est relatif au dossier du `.ini` ; ignoré si le fichier n'existe pas ;
   * enregistrement d'un `TScript` dans `ScriptsList` (index = ordre de découverte, utilisé comme `Tag` des menus) ;
   * création des entrées de menu / bouton de barre d'outils (§2.3).

### 1.8 Quand un script est-il exécuté ? (déclencheurs)

| Déclencheur | Code | Particularités |
|---|---|---|
| Clic menu / raccourci / bouton barre d'outils | `LoadScript` (`unitMain.pas:6478-6524`) | `ScriptParameters := ''` ; `ScriptExecutedManually := True` ; `ScriptLinesChanged := ''` ; si `NeedsFile` : instantané d'annulation nommé d'après le script (`AddHistory(Script.Name)`), puis historique verrouillé pendant l'exécution ⇒ **tout le script = une seule étape d'annulation** ; après exécution : si `DropDown`, re-peuplement du menu déroulant ; si des lignes ont été ajoutées par `LineAdd` (`LinesAdded`), post-traitement GUID (§1.9) puis hooks `ExecuteWithNewLine` et rafraîchissement de la grille |
| Ouverture d'un fichier .bp | `Load_ExecuteWithFile_Scripts` (`unitMain.pas:4184-4194`), appelé en fin de `LoadBankPerfectFile` (`unitMain.pas:4133`) | pour chaque script dans l'ordre de découverte : si `ExecuteWithFile=1` → exécution ; si `DropDown=1` → peuplement du menu déroulant |
| Saisie manuelle d'une nouvelle opération | `Load_ExecuteWithNewLine_Scripts` (`unitMain.pas:3903`, `4196-4204`) | `get_lines_changed` = `"compte-ligne"` de la ligne saisie ; ne s'exécute que si `ScriptLinesChanged <> ''` ; vidé ensuite |
| Modification manuelle d'une opération | `Load_ExecuteWithModifiedLines_Scripts` (`unitMain.pas:3902`, `4206-4214`) | idem ; ⚠ la ligne modifiée est supprimée puis **ré-ajoutée en fin de liste** : son index change |
| Après un script manuel ayant appelé `LineAdd` | `LoadScript` → `Load_ExecuteWithNewLine_Scripts` (`unitMain.pas:6503-6519`) | `get_lines_changed` = `"c-l;c-l;..."` de toutes les lignes ajoutées (cas des imports CSV/QIF/OFX) |
| Clic sur un élément d'un menu déroulant de script | `ScriptDropDownMenuClick` (`unitMain.pas:7095-7099`) | `ScriptParameters := <libellé de l'élément>` puis `DirectLoadScript` (non « manuel » : pas d'annulation, pas de GUID) |
| Bouton « solde prévisionnel » de l'écran principal | `StartPrevisions` (`unitMain.pas:7408-7417`) | couplage par **nom de fichier** : exécute `previsions.py` avec `ScriptParameters := 'yyyy-mm-dd'` |
| Fichier .bp illisible | `CorruptedFile` (`unitMain.pas:5652-5665`) | propose puis exécute `backups.py` (couplage par nom de fichier) |
| Ligne de commande | `FormShow` → `ExecScriptFromCommandLine(ParamStr(2))` si `ParamStr(2)` finit par `.py` (`unitMain.pas:7088-7093`, `6457-6476`) | `ExecPythonOnly := True` (console, et `Application.Run` n'est pas lancé : l'appli se termine après le script, `bankperfect.dpr`) ; `LoadedScriptName` = nom du fichier sans extension. ⚠ history.txt annonce `bankperfect.exe "fichier.py"` mais le code teste le **2ᵉ** paramètre |
| Évaluation d'expressions (hors scripts) | `TFormMain.PyEval` (`unitMain.pas:6627-6635`) via `mathparser` (`unitMain.pas:1932-1954`) | les champs montant évaluent les formules avec Python (`/` réécrit en `*1.0/`, `op`/`av`/`solde` remplacés par les soldes) ; **interpréteur dédié**, aucune API BP. À remplacer par un évaluateur d'expressions natif |

Hooks par nom de fichier : `ScriptIndexByFilename` (`unitMain.pas:7435-7446`) compare le nom de fichier en minuscules. Le nouvel hôte doit conserver `previsions.py` et `backups.py` comme points d'entrée (ou les déclarer dans l'.ini via une nouvelle clé, en gardant le repli par nom).

### 1.9 Marquage GUID des lignes ajoutées

Lorsqu'un script est lancé **manuellement**, `LineAdd` préfixe le tiers par la constante `working_guid = '34B21B70-D9D6-4226-8F28-500FE55FCF51'` (`UnitPlugins.pas:321-323`, `UnitRStrings.pas:6`). Pendant l'exécution du script, `BP.Operationthirdparty[a][l]` d'une ligne fraîchement ajoutée **contient ce préfixe** (la grille l'affiche avec une icône « en cours », `unitMain.pas:1482-1503`). À la fin, `LoadScript` retire le préfixe de toutes les lignes et construit `ScriptLinesChanged` (`unitMain.pas:6503-6519`).

> Compatibilité : reproduire ce comportement observable (préfixe visible pendant l'exécution) ou, mieux, le masquer à la lecture tout en conservant la mécanique `get_lines_changed`. Aucun script ne teste le préfixe.

### 1.10 Variables d'état partagées hôte ↔ scripts

| Variable Delphi | Exposée par | Durée de vie |
|---|---|---|
| `ScriptParameters` (globale) | `get_script_parameters` / `set_script_parameters:` | persiste **entre** exécutions (canal inter-scripts) ; remis à `''` au début de `LoadScript` et après le peuplement d'un menu déroulant |
| `ScriptLinesChanged` | `get_lines_changed` | vidé au début de `LoadScript` et après chaque passe de hooks |
| `ScriptSingleLine` | `SingleLine()` | -1 au démarrage ; mis à jour lors du remplissage de la grille quand une ligne marquée « nouvelle » (`Tack > 2`) est rencontrée (`unitMain.pas:3443-3448`) ; jamais remis à -1 |
| `LoadedScriptName` | titre des `MsgBox`, nom des `CustomForm` | durée de l'exécution |
| `LinesAdded` | — | mis à vrai par `LineAdd`, à faux par `LineDelete` (les index ne sont plus fiables) |

### 1.11 Menus déroulants (`DropDown=1`)

Protocole (`LoadDropDownScript`, `unitMain.pas:4136-4182`) :

1. L'hôte vide le `TPopupMenu` du bouton, met `ScriptParameters := 'populate_popup_menu'` et exécute le script.
2. Le script lit `BPEval("get_script_parameters")`, voit `populate_popup_menu` et répond par `BPEval("set_script_parameters:" + "Libellé1;Libellé2|Ctrl+1;...")`.
3. Si `ScriptParameters` vaut encore `populate_popup_menu` ou `''`, rien n'est fait. Sinon split sur `;` ; pour chaque élément, `Libellé|Raccourci` optionnel (raccourci au format `TextToShortCut`, cf. §2.4) ; un `TMenuItem` invisible est en plus inséré dans le menu Édition pour que le raccourci fonctionne (⚠ fuite : jamais supprimé, s'accumule à chaque re-peuplement).
4. Clic sur un élément : `ScriptParameters := Libellé` (via `Hint`), exécution du script.
5. Clic sur le bouton lui-même : exécution normale (`ScriptParameters = ''`), puis re-peuplement.

Seul `Filters/setfilters.ini` utilise `DropDown=1`.

### 1.12 Persistance des données de scripts (`bp_script_data_*`)

Implémentation : `ObjectScriptsData.pas` (objet `FormMain.ScriptsData`).

**API** : `save_script_data:<clé>:<valeur>`, `load_script_data:<clé>`, `keys_script_data` (§3.3).

**Normalisation de la clé** (`SimpleKey`, `ObjectScriptsData.pas:31-35` + `SimpleString`, `UnitStrings.pas:664-690`) — appliquée à la sauvegarde **et** au chargement :

* `A-Z` → `a-z` ; lettres accentuées Latin-1 → lettre de base (`ÀÁÂÃÄÅÆàáâãäåæ`→`a`, `Çç`→`c`, `Ð`→`d`, `ÈÉÊËèéêë`→`e`, `ÌÍÎÏìíîï`→`i`, `Ññ`→`n`, `ÒÓÔÕÖòóôõö`→`o`, `ÙÚÛÜùúûü`→`u`, `Ýýÿ`→`y`) ;
* `0-9`, `a-z` et **`,`** conservés ;
* tout autre octet → espace, puis **tous les espaces supprimés**.
* ⇒ `"fabio_bp-budget"` et `"fabiobpbudget"` désignent la même clé ; `_` est supprimé.

**Encodage de la valeur** (`SimpleEncode`, `ObjectScriptsData.pas:37-65`) — octet par octet (sur la chaîne ANSI CP1252) :

* conservés tels quels : l'espace, `! " % & ' ( ) * + , - . /`, `0-9`, `: ; < = > ?`, `A-Z`, `[`, `]`, `a-z`, `{`, `}` ;
* **tous les autres octets** deviennent `_` + 2 chiffres hexadécimaux **minuscules** : `0x00-0x1F` (dont `\n`→`_0a`, `\r`→`_0d`, `\t`→`_09`), `#`→`_23`, `$`→`_24`, `@`→`_40`, `\`→`_5c`, `^`→`_5e`, `_`→`_5f`, `` ` ``→`_60`, `|`→`_7c`, `~`→`_7e`, `0x7F-0xFF`→`_7f`…`_ff`.
* Note : `=` et `;` ne sont pas échappés (sans danger : la ligne est `clé=valeur` et seule la première `=` compte).

**Décodage** (`SimpleDecode`, `ObjectScriptsData.pas:67-97`) : chaque `_` est suivi de **exactement 2** caractères hex interprétés par une formule ASCII (`ord<60` → chiffre, sinon `ord-87`) : seules les minuscules `a-f` sont correctement décodées ; un `_` en fin de chaîne lit au-delà (comportement indéfini). Tout autre caractère est recopié.

**Stockage** : lors de la sauvegarde (`ASaveExecute`, `unitMain.pas:2604-2612`), après les lignes d'en-tête du .bp (`utilisateur=`, `guid=`, `UserName=`, `modes=`, `compteN=`, `categ=`, `accounts_order=`, `whoN=`, `compte_defaut=`, `pass=`), une ligne par clé, dans l'ordre d'insertion :

```
bp_script_data_<clé normalisée>=<valeur encodée>
```

Au chargement (`LoadFromLines`, `ObjectScriptsData.pas:141-160`, appelé `unitMain.pas:4103`) toute ligne d'en-tête commençant par `bp_script_data_` est reprise (clé = reste avant le premier `=`, valeur = après). Les données sont vidées à la fermeture du fichier (`unitMain.pas:4260`). `SetData` active le bouton Enregistrer (`ASave.Enabled := True`) : écrire des données de script **marque le fichier comme modifié**.

**Contenu effectif** : 13 scripts stockent un `cPickle.dumps(obj)` (protocole 0, texte ASCII multi-lignes) — clés `fabiobpbudget`, `fabiobpbudgetyear`, `fabiobpautofilters`, `fabiobpautofields`, `fabiobpviewall`, `fabiobpabbreviations`, `fabiobpscheduler`, `fabiobptransfer` ; `backup_mail.py` stocke une adresse e-mail en clair (`fabiobpmailaddress`). Exemple : `cPickle.dumps({'a': 1})` = `"(dp1\nS'a'\nI1\ns."` est stocké `(dp1_0aS'a'_0aI1_0as.`.

> Nouvel hôte : garder **exactement** ce format dans le .bp (lecture/écriture), exposer la valeur aux scripts comme `str` (décodage cp1252 « à la Windows » : octets non définis 0x81/0x8D/0x8F/0x90/0x9D → U+0081… pour être sans perte), et fournir le shim `cPickle` du §5.3 pour que les pickles Python 2 existants se rechargent.

### 1.13 Mode « ReadOnly »

* `ReadOnly` est lu dans l'.ini (`unitMain.pas:6350`) et stocké dans `TScript.ReadOnly` mais **jamais utilisé** pour les scripts (aucune référence).
* Pour les plugins DLL (abandonnés), `plugin.ini: ReadOnly=1` évitait le rafraîchissement de la grille après exécution (`unitMain.pas:6155-6158`).
* Après un script, l'hôte ne rafraîchit la grille que si `LineAdd` a été appelé ; les scripts appellent eux-mêmes `BP.AccountRefreshScreen()`.

> Nouvel hôte : peut utiliser `ReadOnly=1` comme indication (pas d'instantané d'annulation, pas de rafraîchissement), sans changer le comportement des scripts existants (aucun n'a `ReadOnly=1`).

### 1.14 Icônes personnalisées par ligne (`freeicon`)

* Au démarrage, `LoadFreeIcons` charge tous les `*.png` de `PathExe\Icons\` (récursif) ; nom de l'icône = nom de fichier en minuscules sans extension (`unitMain.pas:5155-5171`). Livrées : `alarm, art, attach, cd, connect, doc, find, game, health, help, home, member, ok, plane, rgs_echeance, star, web, work`.
* Une opération affiche l'icône si son champ supplémentaire `freeicon` vaut un de ces noms (insensible à la casse) : dessin 14×14 px en tête de la colonne Tiers, prioritaire sur les icônes transfert / échéance / « en cours » (`unitMain.pas:1480-1503`).
* Côté script : `BPEval("extra_set:%d;%d;freeicon;star" % (compte, ligne))`, effacement avec une valeur vide.

### 1.15 Sons

`BPEval("play:<chemin.wav>|<flags>")` → `sndPlaySound(chemin, flags)` (`UnitPlugins.pas:690-694`). Sans exactement 2 paramètres (`play:`), arrête le son en cours. Flags Win32 `SND_*` : 0 = synchrone, 1 = asynchrone, 2 = pas de son par défaut, 8 = boucle, 16 = ne pas interrompre. ⚠ La doc indique « 4 = pas de son par défaut » : 4 est `SND_MEMORY` (le chemin serait interprété comme un pointeur mémoire) — le nouvel hôte doit ignorer le bit 4.

---

## 2. Format des fichiers `.ini` de script

### 2.1 Section et clés

Fichier INI Windows (`TIniFile`, noms de clés insensibles à la casse), section **`[BANKPERFECT_SCRIPT]`**, encodage CP1252.

| Clé | Type / défaut | Obligatoire | Sémantique exacte | Réf. |
|---|---|---|---|---|
| `Name` | texte | **oui** | Libellé du menu, titre des `MsgBox`, préfixe du nom des `CustomForm`, libellé de l'étape d'annulation, cible de `DisableScripts`. Le premier mot (jusqu'au premier espace) sert de légende au bouton de barre d'outils | `unitMain.pas:6337,6353-6355` |
| `Path` | chemin relatif au dossier de l'.ini | **oui** | fichier `.py` à exécuter ; script ignoré s'il n'existe pas | `6336,6351-6352` |
| `Icon` | entier **ou** nom de fichier `.png` | non (défaut : pas d'icône) | entier ≥ 0 → index dans l'image-list interne `PNGImageList1` (icônes de l'appli, ~58) ; sinon fichier relatif au dossier de l'.ini, extension `.png` en **minuscules** exigée (PNG 24 bits, 24×24 recommandé) ; échec de chargement → icône par défaut n° 14 ; vide/introuvable → aucune icône (-1) | `LoadPngIcon`, `unitMain.pas:6261-6283` |
| `Menu` | liste libre | non | voir §2.2. Vide = script sans menu (hooks uniquement) | `6342`, `6400-6424` |
| `Index` | entier, défaut -1 | non | position d'insertion dans le menu (0 = en haut) ; -1 ou position invalide → ajout en fin | `InsertMenuItem`, `unitMain.pas:6250-6259` |
| `NeedsFile` | booléen INI (`1`/`0`), **défaut 1** | non | 1 : entrées de menu/bouton désactivées tant qu'aucun fichier n'est ouvert, et instantané d'annulation avant exécution manuelle | `6344`, `6201-6205`, `6495` |
| `Shortcut` | texte `TextToShortCut` | non | raccourci clavier du menu (cf. §2.4) ; invalide → ignoré | `6349`, `6208-6212` |
| `DropDown` | booléen, défaut 0 | non | bouton de barre d'outils de style menu déroulant (protocole §1.11) — n'a d'effet que si `Menu` contient `toolbar` et qu'une icône existe | `6343`, `6390-6394` |
| `ExecuteWithFile` | booléen, défaut 0 | non | exécution automatique après chaque ouverture de fichier | `6346`, `4190` |
| `ExecuteWithNewLine` | booléen, défaut 0 | non | exécution après ajout de ligne(s) (saisie manuelle ou `LineAdd` d'un script manuel) | `6347`, `4196-4204` |
| `ExecuteWithModifiedLine` | booléen, défaut 0 | non | exécution après modification manuelle d'une ligne | `6348`, `4206-4214` |
| `ReadOnly` | booléen, défaut 0 | non | **lu mais sans effet** (§1.13) | `6350` |
| `DisableScripts` | liste séparée par `,` | non | `Name` d'autres scripts à ne pas charger (depuis 7.4) | `6319-6330` |

Booléens : `TIniFile.ReadBool` → « `0` = faux, toute autre valeur entière = vrai » ; une valeur non numérique donne le défaut.

Exemples réels : `Scheduler/scheduler_auto.ini` (pas de `Menu`, `ExecuteWithFile=1`), `Auto Fields/auto_fields.ini` (`ExecuteWithNewLine=1`, `ExecuteWithModifiedLine=1`), `Filters/setfilters.ini` (`Menu=affichage,toolbar`, `DropDown=1`), `Scheduler/previsions.ini` (`Menu=` vide : appelé uniquement par le hook `previsions.py`).

### 2.2 Valeurs de `Menu`

Test par **sous-chaîne** (`Pos(motclé, menu) > 0`), **sensible à la casse** ; séparateur conventionnel `,` mais non requis ; un même script peut apparaître dans plusieurs menus (une entrée créée par menu, chacune avec le même raccourci).

| Mot-clé (n'importe lequel) | Emplacement | Composant Delphi |
|---|---|---|
| `fichier` | menu Fichier | `MenuFile` |
| `importer` | Fichier > Importer | `Importer1` |
| `exporter` | Fichier > Exporter | `Exporter1` |
| `édition` (CP1252) ou `edition` | menu Édition | `MenuEdit` |
| `affichage` | menu Affichage | `MenuView` |
| `comptes` | menu Comptes | `MenuAccounts` |
| `operations` ou `opérations` | menu Opérations | `MenuLines` |
| `outils` | menu Outils | `MenuTools` |
| `plugins` | menu Plugins | `MenuPlugins` |
| `internet` | menu Internet | `MenuWeb` |
| `context` ou `right` | menu contextuel (clic droit) de la grille | `GridPopupMenu` |
| `?` ou `help` | menu Aide | `MenuHelp` |
| `toolbar` | bouton dans la barre d'outils `ToolBar1` (**seulement si une icône existe**), ajouté à droite, légende = premier mot du `Name`, info-bulle = `Name` | `6378-6398` |

⚠ La doc écrit « fichiern » (coquille) et ne mentionne ni `toolbar`, ni `right`, ni `?`. Effets de bord du test par sous-chaîne à reproduire : `Menu=comptes` ne déclenche pas `toolbar` ; mais un hypothétique `Menu=outilsplugins` placerait le script dans deux menus.

### 2.3 Construction des entrées

`CreateMenuItem` (`unitMain.pas:6199-6214`) : légende = `Name`, `Tag` = index dans `ScriptsList`, `OnClick = LoadScript`, image = icône, désactivée si `NeedsFile` jusqu'à l'ouverture d'un fichier (liste `EnableMenuItems`, `unitMain.pas:1720-1722`).

### 2.4 Format des raccourcis

`TextToShortCut` de la VCL **française** : modificateurs `Ctrl+`, `Maj+` (Shift), `Alt+` combinables dans n'importe quel ordre, touche finale : lettre, chiffre, `F1`…`F12`, noms VCL (`Suppr`, `Inser`, `Entrée`, `Echap`, `Haut`… en français). Valeurs réelles : `Ctrl+F`, `F9`, `Ctrl+B`, `Maj+Ctrl+C`, `Ctrl+T`, `Ctrl+D`, `Ctrl+F1`, `Ctrl+M`, `F2`, `Ctrl+Maj+N`, `Ctrl+Alt+T`, `F3`, `F7`, `Ctrl+Alt+Q`, `Ctrl+L`, `F6`, `Ctrl+R`, `Ctrl+E`, `Ctrl+Maj+T`.

> Nouvel hôte : accepter les formes françaises **et** anglaises (`Shift+`, `Del`…).

### 2.5 Icônes internes

`Icon=<n>` désigne une icône de `PNGImageList1` (ressource de `unitMain.dfm`, ~58 images, n° 14 = icône par défaut des plugins). Indices utilisés par les scripts livrés : 4, 5, 7, 9, 17, 20, 26, 27, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 53, 54, 55, 56, 57. Il faudra extraire cette image-list (code commenté `FormDblClick` en fin de `unitMain.pas` qui l'exportait en `png_<i>.png`) et la livrer avec la même numérotation.

---

## 3. Référence de l'API

### 3.0 Tableau récapitulatif

**Module `BP` — fonctions (30)** (`dmPython.dfm`, implémentations `dmPython.pas`, logique `UnitPlugins.pas`)

| # | Fonction | Signature (args positionnels) | Retour | Usages scripts |
|---|---|---|---|---|
| 1 | `BankPerfectFileName` | `()` | `str` chemin du .bp ouvert, `""` si aucun | 10 |
| 2 | `BankPerfectExePath` | `()` | `str` dossier de l'exe **avec** `\` final | 12 |
| 3 | `BankPerfectPluginPath` | `()` | `str` `"<plugin>\|<script.py>"` | 6 |
| 4 | `BankPerfectOpenFile` | `(path, password)` | `None` | 1 |
| 5 | `BankPerfectCloseFile` | `()` | `None` (⚠ cf. détail) | 0 |
| 6 | `BankPerfectNewFile` | `(path, username, solde)` | `None` | 0 |
| 7 | `BankPerfectSave` | `(path)` | `None` | 0 |
| 8 | `Accounts` | `()` | `list` de 13-uplets | 0 |
| 9 | `AccountFields` | `()` | 13-uplet de noms | 0 |
| 10 | `AccountCount` | `()` | `int` | 6 |
| 11 | `AccountCurrent` | `()` | `int` | 27 |
| 12 | `AccountChangeCurrent` | `(account_index)` | `None` | 0 |
| 13 | `AccountRefreshScreen` | `()` | `None` | 16 |
| 14 | `Categs` | `()` | `list` de 12-uplets (⚠ bogués) | 0 |
| 15 | `CategFields` | `()` | `("CategName","CategParent")` | 0 |
| 16 | `CategCount` | `()` | `int` | 2 |
| 17 | `CategAdd` | `(name, is_sub)` | `None` | 2 |
| 18 | `CategRename` | `(position, new_name)` | `None` | 0 |
| 19 | `CategDelete` | `(position)` | `None` | 0 |
| 20 | `LineAdd` | `(acc, date, mode, tiers, details, categ, amount, mark)` | `int` index de la ligne ou 0 | 15 |
| 21 | `LineDelete` | `(acc, line)` | `None` | 1 |
| 22 | `LineSelect` | `(n)` | `int` | 7 |
| 23 | `VisibleLines` | `()` | `list[int]` | 12 |
| 24 | `OperationGetNameFromModeIndex` | `(mode_index)` | `str` | 4 |
| 25 | `MsgBox` | `(msg, utype)` | `int` (IDOK…) | 71 |
| 26 | `InputText` | `(caption, prompt, value)` | `(int, str)` | 2 |
| 27 | `OpenDialog` | `(title, filename, default_ext, filters)` | `str` | 9 |
| 28 | `CustomForm` | `(caption, code, width, height)` | `str` | 0 |
| 29 | `GetURL` | `(cmd)` | `str` | 25 |
| 30 | `ShellExecute` | `(operation, filename, parameters, showcmd)` | `None` | 12 |

**Module `BP` — attributs séquences (27)** (`dmPython.pas:560-586`, classes `pyAccounts.pas`) — index = `AccountIndex` (et `LineIndex` pour les `Operation*`), lecture `BP.X[i]` / `BP.X[i][j]`.

| Attribut | Niveau | Type élément | Écriture | Usages |
|---|---|---|---|---|
| `AccountName` | 1 | `str` | oui | 12 |
| `AccountNumber` | 1 | `str` | oui | 4 |
| `AccountIBAN` | 1 | `str` | oui | 2 |
| `AccountBIC` | 1 | `str` | oui | 2 |
| `AccountIsVirtual` | 1 | `int` 0/1 | oui | 10 |
| `AccountIsDisabled` | 1 | `int` 0/1 | oui | 0 |
| `AccountBranchName` | 1 | `str` | oui | 2 |
| `AccountBranchAddress` | 1 | `str` | oui | 2 |
| `AccountBranchNumber` | 1 | `str` `"banque guichet"` | oui | 4 |
| `AccountBankName` | 1 | `str` | oui | 2 |
| `AccountCurrency1` | 1 | `str` `"SYM\|taux"` | oui | 3 |
| `AccountCurrency2` | 1 | `str` `"SYM\|taux"` | oui | 2 |
| `AccountBankLogo` | 1 | `str` chemin `.png` ou `""` | ignorée | 1 |
| `CategName` | 1 | `str` `"<id>=<ligne brute>"` | **TypeError** | 26 |
| `CategParent` | 1 | `int` position | **TypeError** | 7 |
| `OperationCount` | 1 | `int` | **TypeError** | 7 |
| `OperationDate` | 2 | `str` `DD-MM-YYYY` | oui | 28 |
| `OperationMarkDate` | 2 | `str` `DD-MM-YYYY` | oui | 0 |
| `OperationMode` | 2 | `str` | oui | 19 |
| `OperationModeIndex` | 2 | `int` | ignorée | 0 |
| `OperationModeSign` | 2 | `int` ±1 | ignorée | 2 |
| `Operationthirdparty` | 2 | `str` | oui | 21 |
| `OperationDetails` | 2 | `str` | oui | 19 |
| `OperationAmount` | 2 | `float` **signé** | oui | 32 |
| `OperationCateg` | 2 | `int` id unique de catégorie, -1 | oui | 19 |
| `OperationMark` | 2 | `int` 0/1/2 | oui | 10 |
| `OperationExtra` | 2 | toujours `None` (désactivé) | ignorée | 0 |

**Exception** : `BP.BPError` (déclarée dynamiquement, `dmPython.pas:593-596` ; en pratique jamais levée correctement, cf. §1.6).

**Fonctions de `__main__` (6)** (`dmPython.dfm` FormsModule) : `SingleLine()`, `BPEval(cmd)`, `CreateChart(panel)`, `DrawChart(panel, names, values, show_null, bevel, type)`, `DestroyChart(panel)`, `ExportChart(panel, path)`.

**Wrapper VCL** : `CreateComponent(class_name, owner)` + objets VCL (§4.2).

**Commandes `BPEval`/`GetURL` (38 + repli HTTP)** (`UnitPlugins.pas:674-934`) — l'ordre des tests compte (préfixes testés dans l'ordre ci-dessous ; premier qui correspond gagne) :

| # | Commande | Paramètres (séparateur) | Retour | Réf. |
|---|---|---|---|---|
| 1 | `play:` | `chemin\|flags` (`\|`) | `""` | 690 |
| 2 | `keys_script_data` | — | clés séparées par CRLF (avec CRLF final) | 695 |
| 3 | `save_script_data:` | `clé:valeur` (1er `:`) | `"1"` / `"0"` | 700 |
| 4 | `add_account:` | `nom;solde` (`;`) | `"OK"` / `"KO"` | 710 |
| 5 | `get_account_uid_from_id:` | `id` | UID ou `""` | 718 |
| 6 | `get_account_id_from_uid:` | `uid` | id ou `"-1"` | 727 |
| 7 | `get_accounts` | — | `"id=nom;id=nom;..."` ordre d'affichage | 731 |
| 8 | `load_script_data:` | `clé` | valeur ou `""` | 740 |
| 9 | `get_lines_changed` | — | `"c-l;c-l"` | 741 |
| 10 | `get_script_parameters` | — | texte | 742 |
| 11 | `set_script_parameters:` | texte | `""` | 743 |
| 12 | `convert_to_bitmap:` | `src\|dst` (`\|`) | `"1"` / `"0"` | 747 |
| 13 | `send_mail:` | `de\|à\|sujet\|corps\|fichier` (`\|`, 5 champs) | `""` ou message d'erreur | 752 |
| 14 | `temp_dir` | — | dossier temp **avec** `\` | 757 |
| 15 | `desktop_dir` | — | Bureau **sans** `\` | 758 |
| 16 | `appdata_dir` | — | dossier config **avec** `\` | 759 |
| 17 | `mode_count` | — | nb de modes (10 + perso) | 760 |
| 18 | `mode_sign:` | `index` | `"-1"` / `"1"` | 761 |
| 19 | `last_line_id` | — | dernier UId attribué | 765 |
| 20 | `bp_file_guid` | — | GUID du fichier | 766 |
| 21 | `bp_save_folder` | — | `[SAVE] PathSave` + `\` | 767 |
| 22 | `bp_backup_folder` | — | `[SAVE] PathBackup` + `\` | 768 |
| 23 | `bp_export_folder` | — | `[SAVE] PathExport` + `\` | 769 |
| 24 | `ini_read:` | `section;clé` | valeur ou `""` | 770 |
| 25 | `ini_write:` | `section;clé;valeur` (exactement 3) | `""` | 779 |
| 26 | `extra_get:` | `acc;ligne[;clé]` | valeur, ou liste des clés `k1;k2` | 785 |
| 27 | `extra_set:` | `acc;ligne;clé;valeur` (valeur = reste) | `""` | 792 |
| 28 | `file:` | `local@url` → téléchargement ; sinon `url` → GET | `""` / contenu | 812 |
| 29 | `pdf:` | programme PDF (§3.5) | `""` | 821 |
| 30 | `set_filter:` | `d1;d2;mode;tiers;catég;montant;pointage` | `""` | 825 |
| 31 | `simulate_set_filter:` | idem ou rien | `"l;l\|l;l\|..."` | 848 |
| 32 | `copy_file:` | `src\|dst` (`\|`) | `"ok"` / `"ko"` / `""` | 864 |
| 33 | `ftp_put:` | `local\|distant\|login\|pass@hôte` | `""` ou erreur | 872 |
| 34 | `ftp_get:` | `local\|distant\|login\|pass@hôte` | `""` ou erreur | 872 |
| 35 | `ftp_lst:` | `distant\|login\|pass@hôte` | lignes `type\|nom\|date\|taille` | 872 |
| 36 | `color:` | nom `cl*` ou n° palette | entier TColor (décimal) | 898 |
| 37 | `draw_mark:` | `canvas;l;t;r;b;mark` (`;`) | `""` | 902 |
| 38 | `format_date:` | `DD-MM-YYYY` | date au format utilisateur | 918 |
| — | *(autre)* | URL | corps de la réponse HTTP GET, `""` + boîte d'erreur si échec | 930 |

Total : 30 fonctions + 27 séquences + 1 exception dans `BP`, 6 fonctions `__main__`, 38 commandes `BPEval` (+repli), 13 instructions PDF, 12 types de contrôles `CustomForm`, et le sous-ensemble VCL du §4.2.

---

### 3.1 Module `BP` — fonctions (détail)

Règles communes (wrappers `dmPython.pas`) :
* Arguments **positionnels** convertis via `PyObjectAsVariant(args)` puis affectés à des variables Delphi typées : un `int` passé là où une chaîne est attendue est converti (`"12"`), une chaîne passée là où un entier est attendu est convertie par le Variant (`"12"` → 12, `"abc"` → exception). Les flottants ↔ chaînes passent par le **séparateur décimal système** (`DecimalSeparator`, `.` ou `,`, `unitMain.pas:1228`).
* Arguments en trop : ignorés. Manquants : exception (abandon).
* Les index ne sont **pas** contrôlés : un index hors bornes produit une exception Delphi (abandon du script) ou, pour `AccountList.el`, un pointeur nil.

#### `BankPerfectFileName() -> str` — `dmPython.pas:118`, `UnitPlugins.pas:513-516`
Chemin complet du .bp ouvert (`FormMain.CurrentBP`), `""` si aucun fichier (sert de test « fichier ouvert ? »).

#### `BankPerfectExePath() -> str` — `dmPython.pas:295`, `UnitPlugins.pas:508-511`
`FormMain.PathExe` = **dossier** de l'exécutable avec `\` final (⚠ la doc dit « chemin de l'exécutable »). Usage : `BP.BankPerfectExePath() + "Scripts\\..."`.

#### `BankPerfectPluginPath() -> str` — `dmPython.pas:300`, `UnitPlugins.pas:502-506`
`CurrentPluginPath + '|' + LoadedScript` si un script est chargé, sinon `CurrentPluginPath`. En pratique (plugins abandonnés) : **`"|C:\...\Scripts\Dossier\script.py"`** (champ plugin vide). Les scripts font `.split("|")[1]`. Le nouvel hôte doit renvoyer `"|" + chemin_absolu_du_script` (séparateurs natifs ; les scripts font ensuite `.replace(".py", ".slk")`, `os.path`/`nt` etc.).

#### `BankPerfectOpenFile(path, password) -> None` — `dmPython.pas:305-315`, `UnitPlugins.pas:446-451`
Ferme le fichier courant s'il y en a un (`CloseCurrentFile(True)` = avec sauvegarde/confirmation), puis `LoadBankPerfectFile(path, password, Silent=False)`. Le code de retour Delphi (0 = erreur, 1 = OK, 2 = mot de passe incorrect) est **jeté** par le wrapper : Python reçoit `None` (⚠ doc : « renvoie un Integer »). Avec `Silent=False`, un mauvais mot de passe ouvre la boîte de saisie de mot de passe. Déclenche les scripts `ExecuteWithFile` (attention : impossible pendant qu'un script tourne, cf. non-réentrance → ils sont silencieusement sautés).

#### `BankPerfectCloseFile() -> None` — `dmPython.pas:317-320`, `UnitPlugins.pas:441-444`
`CloseCurrentFile(True)` si un fichier est ouvert. ⚠ Le wrapper n'affecte pas `Result` (`NULL` sans exception → `SystemError` en Python 2.3 selon P4D). Le nouvel hôte renvoie `None`.

#### `BankPerfectNewFile(path, username, solde) -> None` — `dmPython.pas:322-334`, `UnitPlugins.pas:453-457`
`CreateBankPerfectFile(path, username, CurrToStr(solde))` : crée un .bp avec un compte « Courant » de solde initial `solde` (float).

#### `BankPerfectSave(path) -> None` — `dmPython.pas:336-345`, `UnitPlugins.pas:489-493`
`path == ""` → action Enregistrer ; sinon `SaveFileTo(path)` (enregistrer sous).

#### `Accounts() -> list[tuple]` — `dmPython.pas:123-180`
Liste (ordre des index internes, pas d'affichage) de 13-uplets :
`(AccountName, AccountIBAN, AccountBIC, AccountIsVirtual:int, AccountNumber, AccountBranchName, AccountBranchAddress, AccountBranchNumber, AccountBankName, AccountCurrency1, AccountCurrency2, AccountBankLogo, AccountIsDisabled:int)` — mêmes valeurs que les séquences homonymes.

#### `AccountFields() -> tuple[str]` — `dmPython.pas:182-200`
`('AccountName','AccountIBAN','AccountBIC','AccountIsVirtual','AccountNumber','AccountBranchName','AccountBranchAddress','AccountBranchNumber','AccountBankName','AccountCurrency1','AccountCurrency2','AccountBankLogo','AccountIsDisabled')`.

#### `AccountCount() -> int` — `dmPython.pas:202`, `UnitPlugins.pas:102-105`
Nombre de comptes (`AccountList.Count`).

#### `AccountCurrent() -> int` — `dmPython.pas:218`, `UnitPlugins.pas:107-110`
Index interne du compte affiché (`RawAccountsListIndex`).

#### `AccountChangeCurrent(account_index) -> None` — `dmPython.pas:207-216`, `UnitPlugins.pas:478-482`
Affiche le compte (`RawAccountsListIndex := i; ALoadAccount.Execute`).

#### `AccountRefreshScreen() -> None` — `dmPython.pas:223-227`, `UnitPlugins.pas:484-487`
Re-remplit la grille (`AFillGrid.Execute`) : recalcul des soldes, filtres, tri, `VisibleLines`.

#### `Categs() -> list[tuple]` — `dmPython.pas:229-251`
⚠ Crée des `PyTuple_New(12)` dont seuls les éléments 0 (`CategName[i]`) et 1 (`CategParent[i]`) sont remplis : les éléments 2-11 sont `NULL` (tout accès ou `repr` plante). Aucun script ne l'utilise. Nouvel hôte : renvoyer des 2-uplets `(CategName[i], CategParent[i])`.

#### `CategFields() -> tuple` — `dmPython.pas:347-354` : `('CategName', 'CategParent')`.

#### `CategCount() -> int` — `UnitPlugins.pas:362-365` : nombre de lignes de la liste des catégories (catégories + sous-catégories).

#### `CategAdd(name, is_sub) -> None` — `dmPython.pas:282-293`, `UnitPlugins.pas:459-466`
Ajoute `Trim(name)` en fin de liste avec un nouvel id unique (`LastIndex+1`) ; si `is_sub == 1` la ligne est indentée (préfixe de 5 espaces = sous-catégorie du maître qui la précède).

#### `CategRename(position, new_name) -> None` — `UnitPlugins.pas:473-476`, `ObjectCategs.pas:107-114`
Renomme la ligne à la **position** donnée (conserve l'indentation) ; nom vide ignoré. L'id unique ne change pas, donc les opérations suivent.

#### `CategDelete(position) -> None` — `UnitPlugins.pas:468-471`, `unitCategsManage.pas:62-65`
Supprime la ligne à la position donnée, sans confirmation ni mise à jour des opérations (leur id devient orphelin).

#### `LineAdd(acc, date, mode, tiers, details, categ, amount, mark) -> int` — `dmPython.pas:356-373`, `UnitPlugins.pas:289-336`
1. `date` : `"DD-MM-YYYY"` lu par positions fixes (`[1..2]`, `[4..5]`, `[7..10]`, séparateur quelconque) ; invalide → retour **0**, rien n'est ajouté.
2. `mode` doit être **exactement** un libellé de la liste des modes (`PayMode.Items`, ex. `"Carte"`, `"Chèque émis"`, modes perso) ; sinon retour 0.
3. Montant stocké = `signe(mode) × |amount|` (le signe passé est ignoré ; signe : modes 0-5 débit, 6-9 crédit, modes perso selon `-` initial).
4. `tiers` et `details` : `;` remplacés par des espaces. Si script manuel, tiers préfixé par le GUID (§1.9).
5. `categ` = **id unique** de catégorie (-1 = aucune), `mark` 0/1/2.
6. Date de valeur = date ; `Extra = "UId=<last_line_id+1>"` (nouvel id unique de ligne).
7. Le libellé de mode est recalculé (`mode_paiement`) : pour « Chèque émis », si `details` contient `°<nombre>` le mode devient `"Chq <nombre>"` ; si `details` contient `m=`, le mode devient les 15 caractères qui suivent (`unitMain.pas:2016-2059`).
8. Instantané d'annulation (sauf historique verrouillé), ajout **en fin** de liste, `LinesAdded := True`.
9. Retour : **index de la ligne ajoutée** (`Count-1`) — ⚠ la doc dit « 1 si OK ». Un index 0 est ambigu avec l'échec (impossible en pratique : un compte a toujours ≥ 1 ligne).

#### `LineDelete(acc, line) -> None` — `UnitPlugins.pas:338-350`
Refusé (silencieusement) s'il ne reste qu'une ligne. Sinon instantané d'annulation, suppression, `LinesAdded := False`.

#### `LineSelect(n) -> int` — `dmPython.pas:387-395`, `UnitPlugins.pas:599-609`
* `n >= 0` : sélectionne la **ligne de grille** `n+1` (position d'affichage, pas index d'opération !) ; ⚠ le résultat n'est pas affecté (valeur indéterminée) — le nouvel hôte renverra `n`.
* `n == -1` : renvoie l'index d'opération de la **première** ligne sélectionnée (`GetLineNumber(Selection.Top)` ; si rien n'est sélectionné, sélectionne la ligne 1).
* `n < -1` (les scripts utilisent `-2`) : index d'opération de la **dernière** ligne sélectionnée.
* Grille vide : -1.

#### `VisibleLines() -> list[int]` — `dmPython.pas:495-520`, `UnitPlugins.pas:936-949`
Index d'opération des lignes affichées dans la grille, **dans l'ordre d'affichage** (tri et filtres appliqués) — c'est la liste `LineNumbers` construite par le remplissage de grille. Liste vide si aucune.

#### `OperationGetNameFromModeIndex(i) -> str` — `UnitPlugins.pas:495-500`
`-1` → `"<< Sans mode >>"` ; `i >= ModeList.Count` → `""` ; sinon libellé (`0 Carte, 1 Retrait DAB, 2 Chèque émis, 3 Prélèvement, 4 Virement émis, 5 TIP, 6 Versement, 7 Dépôt de chèque, 8 Virement reçu, 9 Titre`, puis modes perso).

#### `MsgBox(msg, utype) -> int` — `dmPython.pas:397-407`, `UnitPlugins.pas:280-287`
`MessageBox(0, msg, titre, utype)` modale, sans fenêtre propriétaire ; titre = `Name` du script (ou nom du plugin). `utype` = combinaison Win32 : boutons `MB_OK 0, MB_OKCANCEL 1, MB_ABORTRETRYIGNORE 2, MB_YESNOCANCEL 3, MB_YESNO 4, MB_RETRYCANCEL 5` ; icônes `MB_ICONSTOP/ERROR 16, MB_ICONQUESTION 32, MB_ICONWARNING 48, MB_ICONINFORMATION 64` ; bouton par défaut `MB_DEFBUTTON2 256`, `MB_DEFBUTTON3 512`. Retour : `IDOK 1, IDCANCEL 2, IDABORT 3, IDRETRY 4, IDIGNORE 5, IDYES 6, IDNO 7`. Constantes : l'hôte fournit `MB_YESNO`, `MB_ICONINFORMATION`, `MB_ICONQUESTION`, `IDYES` (§4.2.1) ; certains scripts définissent en plus leurs propres `ID_YES = 6`. Le nouvel hôte doit honorer **tous** les bits (boutons, icône, bouton par défaut) et renvoyer les mêmes codes ; fermeture par la croix = `IDCANCEL` (ou `IDOK` pour `MB_OK`).

#### `InputText(caption, prompt, value) -> (int, str)` — `dmPython.pas:422-438`, `UnitPlugins.pas:611-627`
* `prompt != ""` : boîte `InputQuery` (une ligne) ;
* `prompt == ""` : boîte multi-lignes BankPerfect `EnterText(s, caption, 'MALL')`.
Retour `(1, texte)` si validé, `(0, texte_initial_ou_modifié)` sinon.

#### `OpenDialog(title, filename, default_ext, filters) -> str` — `dmPython.pas:409-420`, `UnitPlugins.pas:367-379`
* `filename == ""` → boîte **Ouvrir** ; sinon boîte **Enregistrer sous** pré-remplie avec `filename`.
* `filters` au format VCL `"Libellé|*.ext|Libellé2|*.a;*.b"` ; `default_ext` sans point de préférence (VCL).
* Retour : chemin choisi ou `""`. Effet de bord : le répertoire courant peut changer.

#### `CustomForm(caption, code, width, height) -> str` — §4.1.

#### `GetURL(cmd) -> str` — `dmPython.pas:473-493`
Identique à `BPEval(cmd)` (§3.3), mais une exception hôte produit une erreur Python au lieu d'abandonner le script.

#### `ShellExecute(operation, filename, parameters, showcmd) -> None` — `dmPython.pas:522-535`, `unitMain.pas:796-799`
`ShellExecute(hwnd, operation, filename, parameters, ExtractFilePath(filename), showcmd)` : répertoire de travail = dossier de `filename`. Opérations utilisées : `"open"` ; `showcmd` : 1 (`SW_SHOWNORMAL`). Nouvel hôte : `open`/`xdg-open`/`ShellExecuteW`, `print` si supporté.

---

### 3.2 Module `BP` — séquences (détail)

Deux types Python4Delphi (`dmPython.dfm`) : `PT` (services séquence : `len`, `[i]`, `[i:j]`, `[i] = v`, `in`, `repr`) et `PTRO` (idem sans affectation → `TypeError` à l'affectation).

**Sémantique commune** (`pyAccounts.pas:245-368`) :

| Opération | Comportement |
|---|---|
| `len(BP.X)` | `AccountCount` (comptes), `CategCount` (catégories) ; pour `BP.X[a]` (niveau 2) : `OperationCount[a]` |
| `BP.X[i]` | `i < 0` ou `i >= len` → `IndexError("Index %d out of range (0,%d)")` (Python 2.3 ajoute `len` aux index négatifs **avant** l'appel : `BP.X[-1]` fonctionne) ; séquence vide → `IndexError("No accounts")` (même pour les catégories et opérations — message figé) |
| itération `for x in BP.X` / `enumerate` | protocole de repli `__getitem__` jusqu'à `IndexError` (utilisé : `enumerate(BP.CategName)`) |
| `BP.X[i:j]` | liste Python ; ⚠ pas de bornage : `j > len` ou tranche ouverte (`[i:]`, Python 2 passe `INT_MAX`) lit hors bornes ⇒ erreur ; le nouvel hôte appliquera la sémantique standard des tranches (sur-ensemble) |
| `v in BP.X` | comparaison **insensible à la casse** de `str(v)` avec la représentation chaîne de chaque élément |
| `repr(BP.X)` / `str(BP.X)` | `"[v0, v1, ...]"` sans guillemets (valeurs converties en chaîne) ; pour un niveau 1 d'opérations : `"[[...], [...]]"` |
| `BP.X[i] = v` | `v` converti par `str(v)` (P4D `PyObjectAsString`) puis passé au setter Delphi qui reconvertit (Variant) : `int`/`float`/`str` acceptés ; une valeur non convertible lève une exception Python |

**Niveau 2** (`TPyBPFirstLevelSequence`, `pyAccounts.pas:585-597`) : `BP.OperationDate[a]` crée à chaque accès un nouvel objet séquence lié au compte `a` (type `PT` même pour les attributs en lecture seule : l'affectation y est **silencieusement ignorée**). `BP.OperationDate[a][l]` lit l'opération `l` (index interne, ordre de stockage = ordre chronologique d'insertion, pas d'affichage).

Les modifications d'opérations passent par `FormMain.AddHistory(TXT_MODIFY_OP)` (instantané d'annulation, sans effet quand l'historique est verrouillé pendant un script manuel). **Aucune ne rafraîchit l'écran** : appeler `AccountRefreshScreen()`.

| Attribut | Lecture | Écriture | Réf. |
|---|---|---|---|
| `AccountName[i]` | `AccName` | met aussi à jour la liste des comptes affichée | `UnitPlugins.pas:112-135` |
| `AccountNumber[i]` | `AccNumber` | idem | `157-160`, `179-182` |
| `AccountBranchNumber[i]` | `BankNumber + ' ' + AgencyNumber` | split au **premier espace** : sans espace → seul `AgencyNumber` modifié | `152-155`, `167-177` |
| `AccountBranchName[i]`, `AccountBranchAddress[i]`, `AccountBankName[i]`, `AccountIBAN[i]`, `AccountBIC[i]` | champ brut | champ brut | `117-150`, `540-558` |
| `AccountIsVirtual[i]`, `AccountIsDisabled[i]` | 1/0 | `v != 0` → 1 (côté Python) puis `Value = 1` | `pyAccounts.pas:432-456`, `UnitPlugins.pas:518-538` |
| `AccountCurrency1[i]`, `AccountCurrency2[i]` | `Curr + '\|' + FloatToStr(Rate)` — ⚠ taux formaté avec le **séparateur décimal système** (`"EUR\|1"`, `"USD\|1,3"` sur un Windows français) | sans `\|` → ignoré ; taux lu par `Str2Extended` | `UnitPlugins.pas:381-410` |
| `AccountBankLogo[i]` | `PathExe + 'logos\' + nom_du_logo(espaces→_) + '.png'` s'il existe, sinon `""` (« aucun » → `""`) — ⚠ doc : « BMP » | ignorée | `412-422` |
| `CategName[p]` | ⚠ **`"<id_unique>=<ligne brute>"`** où la ligne brute d'une sous-catégorie commence par 5 espaces (ex. `"3=Alimentation"`, `"7=     Restaurant"`) ; `""` si hors bornes | TypeError | `ObjectCategs.pas:417-428` |
| `CategParent[p]` | **position** de la catégorie maître (elle-même si maître) | TypeError | `ObjectCategs.pas:50-54` |
| `OperationCount[a]` | nombre d'opérations du compte | TypeError | `UnitPlugins.pas:197-200` |
| `OperationDate[a][l]`, `OperationMarkDate[a][l]` | `FormatDateTime('DD-MM-YYYY')` | positions fixes 1-2/4-5/7-10 ; invalide → ignoré silencieusement | `202-222`, `560-580` |
| `OperationMode[a][l]` | libellé stocké (ex. `"Chq 1234"`) | affecté **sans validation** (⚠ doc : « doit être valide ») | `224-234` |
| `OperationModeIndex[a][l]` | `2` si le mode commence par `"Chq"`, sinon index dans la liste des modes (-1 si inconnu) | ignorée | `424-431` |
| `OperationModeSign[a][l]` | signe du mode (-1/1) | ignorée | `433-439` |
| `Operationthirdparty[a][l]` | `Who` (avec préfixe GUID éventuel, §1.9) | `;` → espace | `236-245` |
| `OperationDetails[a][l]` | `Info` | `;` → espace | `247-256` |
| `OperationAmount[a][l]` | `Value` **signé** (débit < 0) — ⚠ doc : « sans le signe » | `str(v)`, `,`/`.` → séparateur système, `StrToFloat` ; stocké **tel quel, signé** (cf. `ModifyGroup/modifygroup.py:73` qui multiplie par `OperationModeSign`) | `pyAccounts.pas:721-735`, `UnitPlugins.pas:184-195` |
| `OperationCateg[a][l]` | id unique de catégorie (-1 aucune) | entier | `258-272` |
| `OperationMark[a][l]` | 0 non pointé, 1 pointé, 2 rapproché | entier | `263-278` |
| `OperationExtra[a][l]` | `None` (code commenté) | ignorée | `pyAccounts.pas:664-672` |

Conversion des flottants : en lecture `OperationAmount` est un `float` Python (Double). En écriture, la conversion intermédiaire `str(float)` de Python 2.3 utilise 12 chiffres significatifs (`%.12g`) : `0.1+0.2` est écrit `0.3`. Le nouvel hôte doit accepter directement `float`/`int`/`str` et arrondir à la précision monétaire (2 décimales) comme le fait le stockage `Currency`.

---

### 3.3 Commandes `BPEval(cmd)` / `BP.GetURL(cmd)` (détail)

`BPEval(cmd) -> str` (`dmPython.pas:623-631`) = `IBankPerfectPlugin.GetURL(cmd)` (`UnitPlugins.pas:674-934`). Tests **par préfixe** dans l'ordre du tableau §3.0, sensibles à la casse. Valeur de retour non affectée → `""`. Un `Split(sl, s, sep)` (`UnitStrings.pas:374-387`) découpe sur un séparateur d'**un caractère** ; une chaîne vide donne 0 élément, `"a;"` donne `["a", ""]`. Accès à un paramètre manquant → exception (abandon du script).

**`play:<wav>|<flags>`** — §1.15.

**`keys_script_data`** — clés normalisées (§1.12) séparées par `\r\n`, avec `\r\n` final ; `""` si aucune.

**`save_script_data:<clé>:<valeur>`** — clé = jusqu'au premier `:` après le préfixe, valeur = tout le reste (peut contenir `:`, retours à la ligne, etc.). Sans `:` → `"0"`. Sinon normalise la clé, encode et remplace/ajoute, marque le fichier modifié → `"1"`. Ne nécessite pas de fichier ouvert mais les données sont perdues sans sauvegarde d'un .bp.

**`load_script_data:<clé>`** — valeur décodée, `""` si absente.

**`add_account:<nom>;<solde>`** — exactement 2 champs, nom non vide, solde numérique (`isNumber`) → `FormAccounts.CreateAccount(nom, solde)` → `"OK"` ; sinon `"KO"`.

**`get_account_uid_from_id:<id>`** — UID (chaîne) du compte d'index `id` ; `""` si `id` non numérique ou hors bornes.
**`get_account_id_from_uid:<uid>`** — index du compte, `"-1"` si inconnu. ⚠ La doc écrit deux fois `get_account_uid_from_id`.

**`get_accounts`** — comptes dans l'**ordre d'affichage** choisi par l'utilisateur : `"3=Livret;0=Courant;1=Epargne"` (index interne `=` nom, séparés par `;`).

**`get_lines_changed`** — `"compte-ligne"` séparés par `;` (§1.8). Vide si aucun.

**`get_script_parameters`** / **`set_script_parameters:<texte>`** — §1.10/§1.11 ; `set_` renvoie `""`.

**`convert_to_bitmap:<src>|<dst>`** — charge une image (PNG/JPG/BMP), l'enregistre en BMP → `"1"` si `dst` existe ensuite, `"0"` sinon (exception si pas de `|`). Utilisé par `Rib/rib.py` pour insérer un logo dans un PDF (`DrawImage` n'accepte que ce que `TImage` sait lire puis convertit en bitmap) — dans le nouvel hôte, `DrawImage` acceptera directement le PNG ; la commande reste à fournir.

**`send_mail:<de>|<à>|<sujet>|<corps>|<fichier>`** — exactement **5** champs (⚠ doc : 6). Sinon `"Nombre de paramètres incorrect"`. Refusé sans licence (`Registered = ''`) avec un message. Sinon : téléversement FTP du fichier joint puis POST HTTP vers un service du site de l'auteur ; ajoute un pied « Message envoyé depuis BankPerfect … Licence n°… » (`<br>` si le corps commence par `<html>`). Retour `""`. (`unitMain.pas:5593-5650` — contient des identifiants en dur, ne pas reprendre.) Nouvel hôte : à redéfinir (SMTP configurable ou client mail par `mailto:`), en conservant la signature et `""` = succès.

**`temp_dir`** — `GetTempPath` (avec `\` final). **`desktop_dir`** — Bureau (sans séparateur final). **`appdata_dir`** — `PathAppData` (avec séparateur final ; = dossier de l'exe en mode portable si un fichier `portable` existe à côté de l'exe, sinon `%APPDATA%\BankPerfect\`).

**`mode_count`** — `ModeList.Count` (10 modes intégrés + modes personnalisés).
**`mode_sign:<index>`** — `index > 9` : `-1` si le mode perso commence par `-`, sinon `1` ; `6..9` : `1` ; `<= 5` : `-1` (`unitMain.pas:4735-4746`). Index non numérique → exception.

**`last_line_id`** — compteur du dernier `UId` attribué aux opérations. **`bp_file_guid`** — GUID du fichier ouvert.

**`bp_save_folder`**, **`bp_backup_folder`**, **`bp_export_folder`** — valeurs `[SAVE] PathSave` (défaut Documents), `PathBackup` (défaut `appdata_dir`), `PathExport` (défaut Documents) de `bankperfect.ini`, avec séparateur final.

**`ini_read:<section>;<clé>`** — lecture dans `bankperfect.ini` (`TMemIniFile` de l'appli), `""` si absent ou paramètres manquants.
**`ini_write:<section>;<clé>;<valeur>`** — écrit seulement s'il y a **exactement 3** champs (une valeur contenant `;` est donc ignorée) ; renvoie `""`. Les scripts y stockent leurs préférences globales.

**`extra_get:<acc>;<ligne>[;<clé>]`** — champs supplémentaires de l'opération (`GetExtraValue`, `unitMain.pas:6074-6100`) :
* clé absente ou vide → noms de toutes les clés séparés par `;` (ex. `"UId;freeicon;#note"`) ;
* sinon valeur, avec décodage `<fab_pv>` → `;` et `<fab_cr>` → `\r\n` ; `""` si absente.
* Stockage interne : `Extra = "k1=v1;k2=v2"` ; `UId=<n>` est toujours présent.

**`extra_set:<acc>;<ligne>;<clé>;<valeur>`** — la valeur est **tout le reste** (peut contenir `;`). Moins de 3 `;` → ne fait rien. (`SetExtraValue`, `unitMain.pas:6028-6058`) :
* clé `UId` : refusée ; clé `__internal_BP_UId` : modifie `UId` ;
* valeur vide : supprime la clé ;
* sinon `;` → `<fab_pv>`, CRLF/CR/LF → `<fab_cr>` ; instantané d'annulation `Valeur supp.`.
* Clés à sémantique hôte : `freeicon` (§1.14), `UId` ; les clés commençant par `#` ne sont pas affichées dans le panneau des soldes (history 7.51) ; d'autres clés servent aux transferts liés (version 8.0).

**`file:<local>@<url>`** — téléchargement HTTP vers `local` avec une fenêtre modale de progression (`FormDownload.HTTPDownload`) → `""`. **`file:<url>`** (sans `@`) — équivaut à un GET de `<url>`.

**`pdf:<programme>`** — §3.5. Renvoie `""`.

**`set_filter:<d1>;<d2>;<mode>;<tiers>;<catég>;<montant>;<pointage>`** — sans fichier ouvert : rien. Remplit les champs de filtre de l'écran principal et rafraîchit la grille : dates `DD-MM-YYYY` ou vide (case décochée), textes de filtre (syntaxe des filtres BankPerfect : `AND`/`OR`, `//` = vide…), pointage entier (`0` non pointées, `1` pointées, `2` rapprochées, `3` pointées et rapprochées, `4` = toutes). Exactement 7 champs requis.

**`simulate_set_filter:`** — sans paramètre : simule les filtres courants ; avec les 7 champs : simule ces filtres. Renvoie, pour **chaque compte** dans l'ordre interne, la liste des index d'opérations qui passent le filtre, séparés par `;`, les comptes séparés par `|` : `"0;1;5||2;3"` (compte 1 vide). ⚠ La doc parle d'« ID des lignes » : ce sont des index. Ne modifie pas l'écran. Sans fichier : `""`.

**`copy_file:<src>|<dst>`** — `CopyFile` avec écrasement → `"ok"`/`"ko"` ; sans `|` → `""`. ⚠ La doc indique `;` comme séparateur : c'est `|`.

**`ftp_put:<local>|<distant>|<login>|<pass>@<hôte>`**, **`ftp_get:`** (mêmes champs), **`ftp_lst:<distant_avec_masque>|<login>|<pass>@<hôte>`** — `TransferFTP` (`unitMain.pas:4290-4321`) : valeurs spéciales `default_bp_url`, `default_bp_login`, `default_bp_password` (lus/décryptés dans `[FTP]` de l'ini) et `default_bp_folder` (remplacé dans le chemin distant par `[FTP] ftpFolders`). Le mot de passe s'arrête au premier `@`. Mode passif, binaire, fenêtre modale de progression. Retour `""` si OK, sinon `"Transfert du fichier via FTP impossible : <message>"` ; pour `ftp_lst` : lignes `nature|nom|date|taille` séparées par CRLF. Champ manquant → `""` sans rien faire.

**`color:<nom>`** — valeur entière décimale d'une couleur Delphi : noms système `clScrollBar … clMenuBar` (insensibles à la casse ; valeurs négatives du type `0xFF0000xx` en entier signé, ex. `clbtnface` → `-16777201`) ; un nombre `n` → `BPColors[n]` (palette utilisateur : 0 fond lignes impaires, 1 fond lignes paires, 2/3 texte, 4 fond sélection, 5 texte sélection, 6 panneau de saisie, 7 panneau des totaux, 8 texte totaux, 9 montants négatifs, 10/11 pointage, 12 texte totaux 2, 13 texte détails — `unitOptions.pas:111-113`) ; inconnu → `"0"`. Le nouvel hôte doit renvoyer des valeurs compatibles avec ce qu'il accepte en propriétés `Color` (§4.2).

**`draw_mark:<canvas>;<left>;<top>;<right>;<bottom>;<mark>`** — dessine la coche de pointage BankPerfect (0 vide, 1 pointé, 2 rapproché, fond blanc) dans le rectangle ; `<canvas>` est l'**adresse mémoire** d'un objet `TCanvas` obtenue côté Python (cf. §4.2) — exactement 6 champs entiers sinon rien.

**`format_date:<DD-MM-YYYY>`** — formate selon le format de date de l'utilisateur (`FormMain.DateFormat`, défaut `'ddd dd-mm-yy'`, ex. `"mer. 13-04-11"`). Composants invalides → `""`.

**Repli** — toute autre chaîne est traitée comme une URL HTTP : GET via ICS (HTTP sans TLS), corps renvoyé tel quel (octets) ; en cas d'échec, `ShowMessage(erreur)` puis `""`. Utilisé avec `http://…` (cotations, devises, plugins, manuel). ⚠ Les sites visés n'existent plus / sont en HTTPS : le nouvel hôte doit supporter HTTPS et ne pas afficher de boîte modale (journaliser).

Commandes historiques **absentes** du code actuel : `delete_file` (annoncée en history.txt l. 477). Ne pas l'implémenter sauf besoin.

---

### 3.4 Fonctions de `__main__`

#### `SingleLine() -> int` — `dmPython.pas:617-620`
`FormMain.ScriptSingleLine` : index d'opération de la dernière ligne saisie manuellement (§1.10), -1 au démarrage.

#### `BPEval(cmd) -> str` — §3.3.

#### Graphiques (AACharts / Graphics32) — `dmPython.pas:633-786`

Le panneau est cherché par **nom de composant** (`FindComponent(Name)`) dans **toutes** les fenêtres ouvertes (`Screen.Forms`), donc typiquement un `TPanel` créé par le script avec `Name = "..."`.

| Fonction | Arguments | Effet |
|---|---|---|
| `CreateChart(panel_name)` | nom | crée une surface de dessin alignée sur tout le panneau (fond = `Panel.Color`), efface le `Hint` du panneau. ⚠ panneau introuvable → plantage (accès nil) |
| `DrawChart(panel_name, names, values, show_null, bevel, graph_type)` | `names`: liste de `str` ; `values`: **liste** de nombres (même longueur) ; `show_null`, `bevel` (« 3D ») : 0/1 ; `graph_type` : `0` barres, `1` barres + ligne, `2` barres empilées, `3` points, `4` ligne, `5` aires, `6` secteurs | dessine sur la surface créée (sinon rien) ; légende `"%s (%.2n)"` (nom + valeur avec séparateur de milliers) affichée **si `Panel.ShowHint` est faux** (⚠ la doc dit l'inverse : « ShowHint doit être égale à 1 ») ; anti-crénelage ×4 |
| `DestroyChart(panel_name)` | nom | supprime la surface si présente |
| `ExportChart(panel_name, path)` | nom, chemin (⚠ 2ᵉ argument obligatoire, la doc n'en cite qu'un) | PDF A4 (unités 2100×2970) titré « Graphique BankPerfect », image du graphique 1500 px de large en (300, 500) ; `path == ""` → bogue (dossier non initialisé) ; renvoie le chemin, ou `None` si pas de graphique |

Utilisé par `Charts/charts.py`, `Loan/loan.py`, `Budget*`… (cf. §4.2 pour le panneau hôte).

---

### 3.5 Mini-langage PDF (`BPEval("pdf:...")`) — `UnitPDF.pas`, `tnpdf.pas`

Programme = lignes séparées par CR/LF ; lignes vides ignorées. Chaque ligne : `Commande<espace>p1;p2;...` (commande insensible à la casse). Dans les paramètres : `\\` → `\`, `\s` → `;`, `\r` → CR, `\n` → LF. Les nombres sont des entiers (`StrToInt`, exception sinon). Système de coordonnées : unités PDF (points), origine **en haut à gauche** (y converti en `PageHeight - y`), page `MediaBox [0 0 W H]`. Marges figées : `TopMargin 200`, `BottomMargin 2770` (saut de page automatique dans `TextOut` quand y ≥ 2770), interligne `50`.

| Commande | Paramètres | Effet |
|---|---|---|
| `Create` | `titre;fichier;largeur;hauteur` | **doit être la première** : métadonnée titre, fichier de sortie, taille de page (scripts : `2100;2970` = A4 au dixième de mm), police Helvetica 20 |
| `CurrY` | `y` | position verticale courante |
| `PenColor` | `couleur` (entier TColor BGR) | couleur du texte (et des traits suivants) |
| `LineWidth` | `w` | épaisseur de trait |
| `DrawLine` | `x1;y1;x2;y2` | trait noir |
| `DrawRectangle` | `x1;y1;x2;y2` | rectangle (contour) |
| `FillRect` | `x1;y1;x2;y2;couleur` | rectangle plein (implémenté comme un trait épais) |
| `FontName` | `Helvetica-Bold`, `Helvetica-Oblique`, `Helvetica-BoldOblique`, `Courier`, `Courier-Bold`, `Courier-Oblique`, `Courier-BoldOblique`, `Times-Roman`, `Times-Bold`, `Times-Italic` ; autre → `Helvetica` | police standard PDF (pas de Times-BoldItalic) |
| `FontSize` | `n` | taille |
| `NewPage` | — | nouvelle page |
| `TextOut` | `x;y;texte` | `y = 0` → `CurrY` ; `y < 0` → `CurrY - y` ; avance `CurrY` de 50 ; `(`/`)` échappés ; texte CP1252 (WinAnsi) |
| `TextRect` | `x;y;limite_droite;texte` | texte multi-lignes avec retour à la ligne automatique avant `limite_droite` (`y > 0` fixe `CurrY`) |
| `DrawImage` | `largeur;hauteur;x;y;chemin` | image (formats `TImage` : BMP, et selon les codecs liés JPG/PNG — les scripts convertissent d'abord en BMP) mise à l'échelle dans la boîte en conservant le ratio |

Fin de programme : le document est fermé et écrit (`EndDoc`). Exemples : `Rib/rib.py:42-80`, `Receipt/receipt.py`.

---

## 4. Formulaires personnalisés

Deux mécanismes coexistent :

* **§4.1 `BP.CustomForm`** : formulaire modal décrit par un mini-langage texte (`UnitCustomForms.pas`). Hérité des plugins DLL ; **aucun script livré ne l'utilise** mais il fait partie de l'API documentée.
* **§4.2 VCL via `CreateComponent`** : les scripts construisent de vraies fenêtres VCL avec Python4Delphi (WrapDelphi). **C'est ce qu'utilisent tous les scripts à interface** (≈ 538 appels `CreateComponent`).

### 4.1 `BP.CustomForm(caption, code, width, height) -> str`

`dmPython.pas:450-471` → `UnitPlugins.pas:629-632` → `CreateCustomForm` (`UnitCustomForms.pas:661-745`).

#### Fenêtre

* `TForm` créé sans propriétaire, `Caption = caption`, police **Tahoma** (taille VCL par défaut 8 pt), `Position = poMainFormCenter`, `FormStyle = fsStayOnTop`, `AutoScroll = False`.
* `width >= 0 et height >= 0` : bordure fixe (`bsSingle`), seul le bouton Fermer (`biSystemMenu`), taille `width × height` (taille **extérieure** de la fenêtre, bordures comprises).
* `width < 0 ou height < 0` : fenêtre **redimensionnable** (`bsSizeable`, boutons système/agrandir/réduire), taille `|width| × |height|`, taille restaurée depuis l'ini (`LoadFormSize(Ini, f, True)`) ; taille minimale = taille initiale (`Constraints.MinWidth/MinHeight`).
* `Name = 'form_' + TextToName(NomDuScript + '_' + caption)` (minuscules, accents retirés, espaces → `_`, autres caractères supprimés) : clé de section de `bankperfect.ini` où taille/position sont **sauvegardées à la fermeture** (`SaveFormSize`) — pour toutes les fenêtres, fixes ou non.
* Affichage **modal** (`ShowModal`) ; tout bouton ferme la fenêtre (`ModalResult = mrOK`).

#### Langage `code`

* Lignes séparées par CR/LF (`TStringList.Text`). Une ligne sans `;` ou dont le premier segment n'a pas de `=` est ignorée.
* Ligne : `type=nom;prop1=val1;prop2=val2...` — premier segment = type (insensible à la casse) et nom du composant (doit être un identifiant Delphi valide et **unique** dans la fenêtre, sinon exception → ligne ignorée).
* Propriété : nom insensible à la casse ; sans `=` → valeur vide. Échappements dans la valeur : `\\` → `\`, `\s` → `;`, `\r` → CR, `\n` → LF.
* Les propriétés sont appliquées **dans l'ordre** ; une valeur invalide (ex. `left=abc`) lève une exception qui interrompt la création de ce contrôle (déjà créé et à moitié configuré) et l'empêche d'être ajouté à la liste des valeurs renvoyées.
* Couleurs : format HTML `#RRGGBB` ou `RRGGBB` (`HtmlToColor`, `UnitCustomForms.pas:36-47`).
* `anchors` : sous-ensemble des lettres `l`, `r`, `t`, `b` (ex. `anchors=lrtb`) ; présent → remplace les ancres par défaut (`lt`).
* `hint` : info-bulle (activée si non vide).
* `bphint` (edit, maskedit, combo) : bulle d'aide BankPerfect (`FormMain.ShowAlert`, style « flat hint » avec icône n°1) affichée quand le contrôle **reçoit le focus**.
* Communes à tous : `left`, `top`, `width`, `height` (entiers, pixels, coordonnées client).

#### Contrôles

| `type` | Contrôle | Propriétés spécifiques (en plus de left/top/width/height/hint/anchors) | Valeur renvoyée |
|---|---|---|---|
| `label` | `TLabel` (AutoSize) | `caption`, `color` (couleur du **texte**), `bold=1` | — |
| `edit` | `TEdit` | `text`, `bphint`, `color` (texte), `enabled` (`1`), `readonly` (`1`) | `nom=<texte>` |
| `maskedit` | `TMaskEdit` | `text`, `mask` (masque VCL `EditMask`, ex. `!99/99/0000;1;_`), `bphint`, `enabled`, `readonly` | ⚠ **`nom` seul, sans `=valeur`** (`TMaskEdit` n'hérite pas de `TEdit`, bogue) |
| `combo` | `TComboBox` style **liste déroulante non éditable** (`csDropDownList`) | `text` (= éléments, un par ligne : `a\r\nb`), `itemindex`, `enabled`, `bphint` ; si aucun élément sélectionné et liste non vide → élément 0 sélectionné | `nom=<itemindex>` |
| `listbox` | `TListBox` dessinée par BankPerfect (`lbOwnerDrawFixed`, `FormMain.ListBoxDrawItem` : lignes alternées aux couleurs BankPerfect) | `text` (éléments), `itemindex`, `enabled` ; sélection 0 par défaut | `nom=<itemindex>` |
| `checkbox` | `TCheckBox` | `caption`, `checked` (`1`), `enabled` | `nom=1` / `nom=0` |
| `radio` | `TRadioButton` (tous dans le même groupe : la fenêtre) | `caption`, `checked`, `enabled` | `nom=1` / `nom=0` |
| `date` | `TDateTimePicker` format **long** (`dfLong`) | `date` (`DD-MM-YYYY`, positions fixes), `enabled` | `nom=DD-MM-YYYY` |
| `memo` | `TMemo` avec ascenseurs H+V | `text` (`\r\n` pour les retours), `font` (nom de police), `wrap` (`1`), `enabled`, `readonly` | `nom=<texte échappé>` (cf. ci-dessous) |
| `button` | `TButton`, `ModalResult = mrOK` (ferme la fenêtre) | `caption`, `default=1` (Entrée), `cancel=1` (Échap), `enabled` | — (le nom du bouton cliqué est renvoyé dans `SelectedButton`) |
| `picture` | `TImage` `AutoSize` | `path` (fichier image) ; ⚠ `height` ignoré (comparaison avec `'Height'` en majuscule, bogue) | — |
| `grid` | `TDrawGrid` (cf. ci-dessous) | `cols`, `rows`, `cells`, `align`, `colors`, `index`/`rowindex`/`itemindex` | `nom=<ligne sélectionnée - 1>` (0 = première ligne de données) |

Types inconnus : aucun contrôle créé, mais le nom est ajouté à la liste des valeurs (ligne `nom` sans `=`) — sauf si le type est une sous-chaîne de `"label button picture"` (test `Pos(AType, ...)`, `UnitCustomForms.pas:658`).

**Grille (`grid`)** — `UnitCustomForms.pas:579-639, 793-855` :
* options : traits fixes et lignes, **sélection de ligne entière**, sélection de plage, `ThumbTracking` ; pas d'édition ; `FixedRows = 1` (ligne 0 = en-tête), `FixedCols = 0`, colonnes 135 px par défaut, lignes 20 px, `Ctl3D = False`.
* `cells` : toutes les cellules, ligne par ligne, séparées par `\n` (`\r\n` accepté) : la cellule (col, ligne) est l'élément `ligne × cols + col`. Doit être donné **après** `cols`/`rows` pour que la mise en forme soit initialisée correctement (les tableaux d'alignement sont dimensionnés en fin de création).
* rendu : en-tête fond `clBtnFace` texte noir ; lignes de données fond `BPColors[0]` (impaires) / `BPColors[1]` (paires), texte `BPColors[2]`/`[3]`, sélection fond `BPColors[4]` texte `BPColors[5]` ; texte centré verticalement, centré horizontalement par défaut.
* `align` : liste séparée par `,` d'éléments `clé:valeur`, valeur `l` (gauche), `c` (centre), `r` (droite) ; clé `cN` = colonne N, `rN` = ligne N, `rNcM` = cellule — **indices à partir de 1** (ex. `align=c1:l,c3:r,r1:c`). Appliqués dans l'ordre.
* `colors` : même syntaxe avec des couleurs `#RRGGBB` — ⚠ analysé mais **jamais utilisé** au rendu (bogue) ; le nouvel hôte peut l'implémenter comme couleur de fond de cellule (sur-ensemble).
* cellule `barchart:<n>` : dessine une barre horizontale (hauteur = moitié centrale de la cellule, longueur `|n|/1000` de la largeur, verte `#33CC33` si `n ≥ 0`, rouge `#CC0000` sinon, contour noir) au lieu du texte.
* `index`/`rowindex`/`itemindex` = n : sélectionne la ligne n si `0 < n < rows`.
* redimensionnement de la fenêtre : largeur de colonne par défaut = (largeur grille − largeur d'ascenseur) / nb colonnes, colonnes égales.

#### Valeur de retour

Chaîne multi-lignes (séparateur CRLF, avec CRLF final), une ligne par contrôle « à valeur » **dans l'ordre de création**, puis `SelectedButton=<nom du bouton cliqué>` (vide si la fenêtre est fermée par la croix ou Échap sans bouton `cancel`). Valeurs lues dans `OnCloseQuery` (donc aussi en cas de fermeture par la croix).

Échappement des valeurs de `memo` (`PropertyToText`, `UnitCustomForms.pas:167-173`) : CR → `\r`, LF → `\n`, `;` → `\s`. ⚠ L'échappement de `\` est perdu (bogue : première ligne écrasée), donc un `\` littéral n'est pas doublé. Les valeurs `edit` ne sont **pas** échappées (un retour à la ligne est impossible dans un `TEdit`, mais un `=` ou `;` est renvoyé tel quel).

Exemple (doc) :

```
label=label1;left=10;top=10;caption=Text de mon label
edit=edit1;left=150;top=10;width=200;text=texte contenant un point-virgule : \s
combo=cb1;left=150;top=40;width=200;text=option1\r\noption2;itemindex=0
date=dt1;left=150;top=70;width=200;date=13-10-2004
memo=memo1;left=10;top=110;width=550;height=100;text=Voici,\r\nun exemple;wrap=0
button=ButtonOK;left=10;top=230;width=100;height=30;caption=OK
```
→
```
edit1=texte contenant un point-virgule : ;
cb1=0
dt1=13-10-2004
memo1=Voici,\r\nun exemple
SelectedButton=ButtonOK
```

### 4.2 Fenêtres VCL pilotées par Python (`CreateComponent`) — **surface critique**

Mécanisme : `TPyDelphiWrapper` (Python4Delphi, unités WrapDelphi/WrapDelphiVCL) lié au module `__main__` (`dmPython.dfm`). Il expose les composants VCL réels avec accès par RTTI à leurs propriétés *published*, leurs méthodes et leurs évènements. Les sources de P4D ne sont pas dans le dépôt : ce qui suit est l'**inventaire exhaustif de ce que les 43 scripts utilisent réellement** (35 scripts ont une interface ; 591 sites de création), et constitue donc le contrat minimal que la nouvelle GUI doit rendre. Tout ce qui n'est pas listé ici n'est utilisé par aucun script livré.

Scripts sans interface : `Export HTML`, `Export QIF`, `Export XLS`, `Export XML`, `Manuel`, `Scheduler/scheduler_add.py`, `Scheduler/scheduler_auto.py`, `UpdateStock`.

#### 4.2.1 Noms globaux fournis par l'hôte (en plus de `BPEval` & graphiques)

| Nom | Occ. / fichiers | Usage | Exemple |
|---|---|---|---|
| `CreateComponent(class_name: str, owner: composant ou None) -> composant` | 539 / 35 (dont un alias `CC = CreateComponent`, `Import CSV/import_csv.py:169`) | crée un composant ; `owner=None` pour les fenêtres ; le nom de classe est parfois dans une variable (`ck="TCheckBox"`, `Auto Fields/auto_fields.py:8-10`, `Export CSV/export_csv.py:84`) | `AllAccountsView/all_accounts_view.py:31` |
| `Rect(left, top, right, bottom) -> TRect` | 24 / 10 | rectangle pour `Canvas.TextRect`/`FillRect` | `AllAccountsView/all_accounts_view.py:223` |
| `MB_YESNO` (4), `MB_ICONINFORMATION` (64), `MB_ICONQUESTION` (32), `IDYES` (6) | 18, 12, 4, 12 | constantes Win32 pour `BP.MsgBox` (fournies par P4D, probablement WrapDelphiWindows) — d'autres scripts définissent leurs propres `ID_YES = 6`, `mrOk`… | `Auto Fields/auto_fields.py:266`, `BudgetYear/budget_year.py:222` |

Le nouvel hôte doit fournir **au minimum** ces noms, et devrait fournir par sécurité toutes les constantes `MB_*`, `ID*`, `mr*` (`mrNone 0, mrOk 1, mrCancel 2, mrAbort 3, mrRetry 4, mrIgnore 5, mrYes 6, mrNo 7, mrAll 8, mrNoToAll 9, mrYesToAll 10`). **Aucun** script n'utilise `Application`, `Screen`, les constantes `cl*`/`fs*`/`al*`/`bs*`/`po*`/`gd*` comme identifiants, ni `SingleLine` ou `BP.CustomForm`.

#### 4.2.2 Modèle objet commun

* **Propriétés** lues/écrites comme attributs Python (`b.Caption = "OK"`, `x = e.Text`), y compris imbriquées (`f.Font.Name`, `f.Constraints.MinWidth`, `grid.Canvas.Brush.Color`, `cb.Items.Text`). Affectations multiples et chaînées fréquentes (`L2.Left = L3.Left = x`, `Grid.ColCount, Grid.RowCount = a, b`, `f.Height = f.Constraints.MinHeight = h`), `+=` sur `Caption`, `Text`, `Left`, `Top`.
* **`SetProps(**kwargs)`** : voie principale de configuration (sur la quasi-totalité des 591 objets), ex. `CreateComponent("TButton", f).SetProps(Parent=f, Left=100, Top=100, Width=90, Height=25, Caption="OK", ModalResult=1, Default=1)` (`Rib/rib.py:39`). Doit renvoyer le composant ou `None` (le résultat n'est jamais utilisé, mais l'appel est souvent chaîné sur un objet anonyme). En Python 2.3 l'ordre des kwargs n'est pas garanti ; plusieurs appels combinent `Checked=…` et `OnClick=…` (`Budget/budget.py:601-607`) ⇒ **appliquer les `On*` en dernier** et ne pas déclencher d'évènements pendant `SetProps`.
* **`SetBounds(left, top, width, height)`** : 63 appels (TButton, TDrawGrid, TListBox, TGroupBox, TPanel, TMemo).
* **Noms de propriétés insensibles à la casse** (RTTI Delphi) — utilisé réellement : `onResize`, `onchange`, `onClick`, `onDblClick`, `Modalresult`, `Readonly`, `Wordwrap`, `Scrollbars`, `font.color`, `canvas.brush.color`, `Font.style`, `defaultColWidth`, `label.width`, `checked`. Valeurs d'énumération aussi insensibles à la casse (`"AkLeft"`, `Import CSV/import_csv.py:333`).
* **Coercitions** : booléens acceptés en `0/1`, `bool` Python ou chaîne (`Checked="True"`, `WordWrap="false"`) ; `Text`/`Caption` acceptent `int`/`float` (convertis en texte, `'%.12g'` pour les flottants) ; propriétés entières (positions, tailles) reçues en `float` dès que la division devient vraie en Py3 ⇒ **tronquer** (`int()`) ; énumérations acceptées en chaîne **ou** entier ordinal (`Position=2`, `Brush.Style=1` = `bsClear`, `Cursor=-21` = `crHandPoint`).
* **Ensembles** : toujours des **listes de chaînes** (`Font.Style = ["fsBold"]` ou `[]`, `Anchors = ["akLeft","akTop","akRight","akBottom"]`, `BorderIcons = ["biSystemMenu","biMinimize"]`, `Options = ["goRowSelect", ...]`), concaténables (`a + ["akTop"]`). En lecture, renvoyer une liste de chaînes.
* **Énumérations** : chaînes au nom Delphi (`"poMainFormCenter"`, `"poScreenCenter"`, `"bsSingle"`, `"csDropDownList"`, `"ssBoth"`, `"ssVertical"`, `"bsBottomLine"`, `"bvNone"`, `"ecUpperCase"`).
* **Couleurs** : entiers TColor Delphi `0x00BBGGRR` (ex. `0x000000CC` rouge, `0x00CC0000` bleu, `0xFFFFFF` blanc), éventuellement issus de `int(BPEval("color:%d" % i))` (palette, peut contenir des couleurs système négatives `0xFF0000xx` à résoudre).
* **Identité** : `==`/`is` entre wrappers d'un même composant doivent être vrais (`Sender == BLocal`, `c == current_field`) ⇒ un wrapper unique par composant (cache).
* `ClassName` est un **attribut** chaîne (`Sender.ClassName == "TDrawGrid"`, `BudgetYear/budget_year.py:525`) ; `str(c.__class__)` doit **contenir le nom de classe VCL** (test `"Edit" in str(c.__class__)` / `"Button"`, `Currencies/currencies.py:114,181`).
* `Name` (écrit et lu ; sert à retrouver les panneaux de graphique et dans `Sender.Name`), `Tag` (entier), `Parent` (écrit uniquement : TForm, TGroupBox, TPanel ; peut différer de l'owner, voire être une autre fenêtre : `Scheduler/scheduler.py:550-551`), `Visible`, `Enabled`, `Hint` (multi-ligne) + `ShowHint`, `Anchors`, `Components` (itérable des composants possédés : `for c in f.Components`).
* **Durée de vie** : les scripts ne libèrent jamais rien (`Free` absent) ; les fenêtres sont réutilisées pour plusieurs `ShowModal` ; des composants sont créés pendant qu'une fenêtre est affichée. Toutes les fenêtres d'un script doivent être détruites à la fin de l'exécution (et `OnDestroy` déclenché à ce moment).
* Les handlers sont des fonctions Python ordinaires, souvent **appelées directement** par le script avec `None` ou un autre objet comme `Sender` (`draw_grid(None)`, `form_resize(f)`).

#### 4.2.3 Classes et membres utilisés

Légende : R lecture, W écriture, SP via `SetProps`.

**`TForm`** (54)
* Propriétés : `Caption` ; `Position` (`"poMainFormCenter"` ×51, `"poScreenCenter"`, `2`) ; `Width`, `Height` (R/W) ; `ClientWidth`, `ClientHeight` (R ×39, `ClientHeight` W) ; `BorderStyle="bsSingle"` (sinon redimensionnable par défaut) ; `BorderIcons` ; `Constraints.MinWidth/MinHeight` ; `Font.Name` (`"Tahoma"` ×52, `"Arial"`), `Font.Size` ; `ModalResult` (R après `ShowModal`, W `= 7` pour fermer depuis un handler) ; `Components`.
* Méthodes : **`ShowModal() -> int`** (54, imbrication jusqu'à 3 niveaux : `BudgetYear`), `Close()` (15, depuis un handler).
* Évènements : `OnResize` (35 — la mise en page est faite là : doit être déclenché **avant le premier affichage**, après création de tous les composants, puis à chaque redimensionnement), `OnActivate` (5 ; remis à `None` après usage pour s'exécuter une fois), `OnShow` (2), `OnDestroy` (3).
* Rendu attendu : fenêtre modale, police de la fenêtre héritée par les enfants (`ParentFont`), centrage sur la fenêtre principale, taille extérieure `Width×Height` incluant la barre de titre (les scripts calculent leur mise en page avec `ClientWidth/ClientHeight`).

**`TButton`** (151) — `Caption`, géométrie, `Default=1` (Entrée), `Cancel=1` (Échap ; aussi sur des boutons sans `ModalResult`), `ModalResult` (1, 2, 6, 7, 9, 10 : ferme la fenêtre et devient le retour de `ShowModal`), `Enabled`, `Font.Size`, `Hint`/`ShowHint`, `Name`, `Tag`, `Anchors` ; `SetBounds` ; `OnClick` (84).

**`TBitBtn`** (1, créé 40 fois dans `Calc/calc.py:66`) — `Caption`, géométrie, `Font.Color`, `Font.Name="Symbol"` (légendes `"¬"`, `"Ö"`, `"p"` affichées ←, √, π par la police Symbol : prévoir une table de correspondance de glyphes), `OnClick`.

**`TLabel`** (168) — `Caption` (W/R, `+=`), géométrie, **`Width`/`Height` lus après affectation du texte** (AutoSize réel selon police : indispensable à la mise en page), `AutoSize=0` + `WordWrap=1`, `Font.Style=["fsBold"]`, `Font.Color`, `Cursor=-21`, `Tag` ; évènements `OnClick`, `OnMouseDown`, `OnMouseUp` (simulation de bouton : `Sender.Top += 1`).

**`TEdit`** (58) — `Text` (R/W/`+=`, reçoit aussi des nombres), géométrie, `Hint`/`ShowHint` (multi-ligne), `Enabled`, `Visible`, `Font.Color`, `Color`, `SelStart`, `MaxLength`, `Name`, `CharCase="ecUpperCase"`, `Tag`, `Anchors` ; méthodes `SetFocus()`, `SelectAll()` ; évènements `OnChange` (déclenché **aussi par une affectation programmatique de `Text`** : des scripts s'en protègent), `OnExit`, `OnEnter`, `OnKeyPress`.

**`TComboBox`** (55) — toujours `Style="csDropDownList"` (non éditable) ; `Items.Text` (éléments séparés par `\n` ou `\r\n`, ligne vide finale ignorée), `Items[i]`, `len(Items)` ; `ItemIndex` (R/W) ; `Text` (R : libellé de l'élément sélectionné) ; géométrie, `Anchors` ; `OnChange` (16 ; **pas** déclenché par une affectation programmatique d'`ItemIndex`).

**`TCheckBox`** (51) — `Caption`, `Checked` (R/W/SP ; une affectation programmatique **déclenche `OnClick`**), géométrie, `Hint`/`ShowHint`, `Anchors` ; `OnClick` (11).

**`TRadioButton`** (4) — `Caption`, `Checked`, géométrie, `Parent` (groupes = conteneur parent, ex. deux `TGroupBox`). Aucun évènement.

**`TGroupBox`** (7) — `Caption`, `Parent`, géométrie, `Anchors`, `SetBounds`, `Components` ; conteneur (coordonnées des enfants relatives à la zone client du cadre).

**`TPanel`** (3) — `Name` (`"ChartZone"`, `"loan_panel_chart"`, `"PanelCharts"`), `Caption=" "`, `Color`, `ShowHint=1`, `BevelInner/BevelOuter="bvNone"`, `SetBounds`, `Top/Height` (R) ; sert d'hôte aux graphiques (§3.4) et d'image (`TImage`).

**`TBevel`** (1) — `Shape="bsBottomLine"` (ligne de séparation), géométrie.

**`TImage`** (1) — géométrie, `Parent`, `Picture.LoadFromFile(chemin_png)` (`test/test.py:58`).

**`TListBox`** (7) — `Items.Text`, `Items[i]`, `len(Items)`, `Items.Add(s)`, `Items.Delete(i)`, `Items.Assign(liste_python)` ; `ItemIndex` (R/W, -1) ; `MultiSelect=1` ; **`Selected` = liste d'index sélectionnés** en lecture et écriture (`list_categs.Selected = [..]`, `for c in list_categs.Selected`, `BudgetYear/budget_year.py:533-543`) — extension propre à BankPerfect (history.txt 6.51), différente de la propriété indexée VCL ; `SetBounds` ; `OnClick`, `OnDblClick`. Rendu standard (non dessiné par l'hôte).

**`TMemo`** (9) — `Lines.Text` (R/W ; lecture avec fins de ligne `\r\n`), `WordWrap`, `ReadOnly`, `ScrollBars` (`"ssVertical"`, `"ssBoth"`), `Anchors`, géométrie, `SetBounds` ; `OnChange`.

**`TDateTimePicker`** (5) — `Date` en **numéro de série TDateTime Delphi** (jours depuis le 30/12/1899 ; écrit en entier, lu en `float`) ; `Format` (`"ddd d MMMM yyyy"`, `"dddd dd MMMM yyyy"` — chaîne de format Windows de date, affichée en français) ; géométrie.

**`TDrawGrid`** (16) — grille entièrement dessinée par le script :
* propriétés : `ColCount`, `RowCount`, `FixedCols`, `FixedRows` (mis après avoir vérifié `RowCount > 1` — la VCL refuse `FixedRows >= RowCount`), `DefaultColWidth`, `DefaultRowHeight`, `ColWidths[i]` (R/W, **modifié aussi pendant `OnDrawCell`**), `Selection.Top`, `Selection.Left` (R), `Row` (W), `Options` (`goFixedHorzLine`, `goHorzLine`, `goVertLine`, `goDrawFocusSelected`, `goRowSelect`, `goThumbTracking`), `ScrollBars`, `Hint`/`ShowHint`, `Anchors` ;
* `Canvas.TextWidth(s)` / `Canvas.TextHeight("A")` appelés **avant l'affichage** (mesure de texte hors peinture) ;
* méthodes : `Repaint()`, `SetBounds` ;
* évènements : **`OnDrawCell(Sender, ACol, ARow, Rect, State)`** (16), `OnDblClick` (6), `OnClick`, `OnKeyPress`, `OnKeyUp`, `OnMouseUp` ;
* non utilisés : `Cells`, `MouseToCell`, `Col`, `TopRow`.

#### 4.2.4 Signatures des évènements

| Évènement | Signature Python | Arguments |
|---|---|---|
| `OnClick`, `OnDblClick`, `OnChange`, `OnExit`, `OnEnter`, `OnResize`, `OnShow`, `OnActivate`, `OnDestroy` | `f(Sender)` | composant émetteur |
| `OnDrawCell` | `f(Sender, ACol, ARow, Rect, State)` | `Rect` : objet `TRect` **mutable** (`R.Left += 14`, affectation des 4 champs, `R.Right = …`) ; `State` : conteneur de chaînes, seul test utilisé `"gdSelected" in State` (10 sites) ; fournir aussi `gdFixed`, `gdFocused` |
| `OnKeyPress` | `f(Sender, Key)` | `Key` paramètre *var* : objet avec `.Value` = chaîne d'un caractère ; `Key.Value = None` (ou `"\0"`) **annule** la touche ; `ord(Key.Value) == 13` |
| `OnKeyUp` | `f(Sender, Key, Shift)` | `Key.Value` = **code de touche entier** (32 = espace) ; `Shift` liste de chaînes (`ssShift`…) |
| `OnMouseDown`, `OnMouseUp` | `f(Sender, Button, Shift, X, Y)` | seul `Sender` est utilisé |

Affecter `None` à un évènement le débranche (`f.OnActivate = None`, `E.OnChange = None`).

#### 4.2.5 Canvas (dans `OnDrawCell` ou hors peinture)

Obtenu par `Sender.Canvas` / `grid.Canvas`. Membres utilisés : `Brush.Color` (W ×71), `Brush.Style = 1` (bsClear, en fin de dessin), `Font.Color`, `Font.Style` (`["fsBold"]`, `[]`), `FillRect(rect)` (31), **`TextRect(rect, x, y, texte)`** (30, toujours 4 arguments : texte écrit en (x, y) et **découpé** au rectangle ; x/y possiblement `float` → tronquer), `TextWidth(s)`, `TextHeight(s)`. Non utilisés : `TextOut`, `Draw`, `Pen`, `MoveTo`, `LineTo`. `BPEval("draw_mark:<canvas>;...")` (§3.3) attend l'adresse du `TCanvas` : un seul usage dans les scripts ; le nouvel hôte acceptera l'objet canvas wrappé ou un identifiant opaque (`id`) et dessinera la coche dans le canvas correspondant.

#### 4.2.6 Comportements implicites à reproduire

1. `ShowModal` bloque le script et renvoie le `ModalResult` du bouton cliqué (ou affecté par `f.ModalResult = n`, ou 2 = `mrCancel` à la fermeture par la croix) ; les scripts comparent aussi `f.ModalResult` après coup et utilisent des valeurs 6/7/9/10 comme clés de dictionnaire.
2. Entrée active le bouton `Default`, Échap le bouton `Cancel` (son `OnClick` puis sa `ModalResult`).
3. `OnChange` sur affectation programmatique de `TEdit.Text` / `TMemo.Lines.Text` ; `OnClick` sur affectation programmatique de `TCheckBox.Checked` ; **pas** d'`OnChange` sur affectation d'`ItemIndex`.
4. `OnResize` avant le premier affichage (une fois les composants créés) puis à chaque redimensionnement ; `Anchors` et `Constraints.Min*` appliqués.
5. Mesure de texte (`TLabel.Width` après `Caption`, `Canvas.TextWidth`) disponible avant affichage et cohérente avec la police (Tahoma 8 pt par défaut à 96 dpi : les scripts sont calibrés en pixels pour cette métrique — prévoir une mise à l'échelle globale HiDPI plutôt qu'une mise en page différente).
6. Sélection de ligne entière dans les `TDrawGrid` (`goRowSelect`), `Selection.Top` = ligne sélectionnée (≥ `FixedRows`).
7. Couleurs des grilles : les scripts dessinent eux-mêmes en utilisant la palette `BPEval("color:n")`.

#### 4.2.7 Graphiques (rappel §3.4)

`CreateChart(nom)` est appelé dans `OnActivate` (une fois), le panneau étant parfois enfant d'un `TGroupBox` ⇒ **recherche récursive par `Name`** dans toutes les fenêtres ouvertes. `DrawChart` est appelé avec 6 ou **7** arguments (`Charts/charts.py:241`, `Loan/loan.py:550`) : le 7ᵉ est ignoré par l'hôte Delphi (seuls les index 0-5 sont lus). Dans `names`, un élément `"/s=Nom de série"` (avec une valeur 0 à la même position) démarre une nouvelle série (multi-séries, `AACharts.pas:120-123, 621-630`). `DestroyChart` est appelé dans `OnDestroy`. `ExportChart(nom, chemin)` : résultat testé `is None`.

---

## 5. Incompatibilités Python 2.3 → 3 et shims proposés

Objectif : exécuter les scripts existants **sans les modifier à la main**. Stratégie en trois couches, appliquées par l'hôte au chargement de chaque `.py` :

1. **Décodage + normalisation du source** (§5.1) ;
2. **Conversion syntaxique automatique 2→3** avec cache (§5.2) ;
3. **Environnement d'exécution de compatibilité** : modules et builtins de remplacement injectés dans le namespace du script, et API hôte tolérante (§5.3, §5.4).

Les scripts livrés seront en plus migrés une fois pour toutes (et versionnés en Python 3) ; la conversion à la volée sert aux scripts tiers/utilisateurs.

### 5.1 Source

| Problème | Python 2.3 | Python 3 | Solution hôte |
|---|---|---|---|
| Encodage | octets CP1252 exécutés tels quels ; cookies présents : `iso-8859-1` valides (auto_fields, loan, multiple_input, totals — **loan et multiple_input contiennent pourtant des octets 0x80-0x9F**), formes invalides ignorées `#coding = iso-8859-1` (all_accounts_view, budget, budget_year, categs_totals, manage_plugins, people_totals), `# coding = iso-8859-15` en ligne 3 (import_ofx) | source décodé en UTF-8 par défaut → `SyntaxError` sur les accents | lire en octets ; BOM UTF-8 ou cookie `utf-8` → UTF-8 ; cookie `latin-1`/`iso-8859-1`/`cp1252` ou absence de cookie → **cp1252** (ignorer de fait les cookies latin-1/iso-8859-15 des scripts) ; retirer/neutraliser le cookie avant `compile()` du texte décodé |
| Fins de ligne | CRLF (P4D normalise) | CRLF accepté | rien |
| Tabulations | mélange tab/espaces toléré (tab = colonne multiple de 8) | `TabError` si incohérent | ré-indenter : remplacer les tabulations d'indentation par des espaces (tab stop 8) via `tokenize` avant compilation |
| Mots devenus réservés | `True`, `False`, `None` (réaffectables en 2.3), `nonlocal`, `print`/`exec` comme instructions ; `async`/`await` utilisables comme identifiants | mots-clés | renommage automatique des identifiants conflictuels (`async` → `async_`) lors de la conversion |

### 5.2 Syntaxe (conversion automatique)

`lib2to3` a été retiré de CPython 3.13 : embarquer un fork maintenu (`fissix`, ou une copie figée de `lib2to3`) dans l'hôte, ou un convertisseur maison basé sur `ast`/`tokenize`. Procédure : tenter `compile()` en Python 3 ; en cas de `SyntaxError` (ou toujours, si le fichier n'a pas de marqueur « py3 »), appliquer les fixers puis compiler ; cacher le résultat (clé = hash du source). Fixers nécessaires (constructions Python 2 susceptibles d'apparaître dans du code 2.3) :

| Construction 2.x | Py3 | Fixer |
|---|---|---|
| `print x, y` / `print >>f, x` / `print` | `print(...)` | `print` |
| `except E, e:` | `except E as e:` | `except` |
| `raise E, "msg"` / `raise E, msg, tb` | `raise E("msg")` | `raise` |
| `raise "texte"` (exceptions chaînes, obsolètes) | — | réécrire en `raise Exception("texte")` (fixer maison) |
| `` `x` `` | `repr(x)` | `repr` |
| `a <> b` | `a != b` | `ne` |
| `0777`, `123L`, `ur"..."` | `0o777`, `123`, `"..."` | `numliterals`, `unicode` |
| `exec "code"`, `exec code in d` | `exec(...)` | `exec` |
| `execfile(f)` | `exec(open(f).read())` | `execfile` |
| `d.has_key(k)` | `k in d` | `has_key` |
| `d.keys()[0]`, `d.keys().sort()`, `d.items()` indexé | vues | `dict` (enveloppe `list(...)`) |
| `d.iteritems()/iterkeys()/itervalues()` | `items()`… | `dict` |
| `map/filter/zip(...)` utilisés comme listes | itérateurs | `map`, `filter`, `zip` (enveloppe `list(...)`) |
| `xrange`, `raw_input`, `unicode`, `basestring`, `long`, `unichr`, `apply`, `reduce`, `intern`, `buffer` | — | fixers + builtins de repli (§5.3) |
| `def f(a, (b, c)):` / `lambda (a, b): ...` | interdit | `tuple_params` |
| `x.next()` | `next(x)` | `next` |
| `import cPickle, string, sets, md5, sha, StringIO, urllib, urllib2, httplib, Tkinter, copy_reg, exceptions` | renommés/supprimés | **ne pas** utiliser `fix_imports` : fournir des modules de compatibilité du même nom (§5.3), plus fidèles |
| `sorted(l, cmp)` / `l.sort(cmp)` / `l.sort(cmp=f)` | `key=` | fixer maison → `key=functools.cmp_to_key(f)` |
| `string.letters`, `string.maketrans` | — | module `string` de compatibilité |
| division `/` entre entiers | division vraie | **transformation AST** : `a / b` → `__bp_div__(a, b)` et `a /= b` → `a = __bp_div__(a, b)`, où `__bp_div__` fait `a // b` si les deux opérandes sont des `int` (bool compris), sinon `a / b` — reproduit exactement la division classique 2.3 |

### 5.3 Modules et builtins de compatibilité (injectés)

Placés dans un dossier `bp_compat/` en tête de `sys.path` et dans les builtins du namespace du script (sans polluer `builtins` global si l'interpréteur est partagé : utiliser un dict `__builtins__` dédié par exécution).

**Builtins ajoutés** : `long = int`, `unicode = str`, `basestring = str`, `unichr = chr`, `xrange = range`, `raw_input = input`, `reduce = functools.reduce`, `apply(f, args=(), kw={})`, `cmp(a, b)` (`(a > b) - (a < b)` avec ordre total Python 2 : `None` < nombres < autres types, pour ne pas lever de `TypeError`), `file = open`, `execfile`, `intern = sys.intern`, `coerce`, `StandardError = Exception`, `__bp_div__`, **`round`** à la Python 2 (arrondi « au plus loin de zéro » sur la valeur décimale, retourne un `float` : `round(2.5) == 3.0`, `round(-0.5) == -1.0`) — **critique pour des montants**, et `open` de compatibilité (ci-dessous).

**`open()`/`file()` de compatibilité** : en mode texte, `encoding='cp1252'`, `errors='replace'` par défaut (Python 2 écrivait des octets CP1252) ; `newline` laissé à `None` (sous Windows, `\n` ↔ `\r\n` comme Python 2) — sur Linux, pour garder des exports CSV/QIF identiques, option `newline='\r\n'` à l'écriture. Le mode binaire (`'rb'`, `'wb'`) renvoie/accepte des `bytes` : les scripts qui manipulent le contenu comme du texte après `'rb'` doivent être signalés (liste au §4.2/rapport d'analyse).

**`cPickle` / `pickle`** (stockage des données dans le .bp, §1.12) :

```python
# bp_compat/cPickle.py (esquisse)
import pickle as _p
from bp_compat.codec import CP1252W   # cp1252 « sans perte » : 0x81,0x8D,0x8F,0x90,0x9D <-> U+0081… (comme MultiByteToWideChar)
def dumps(obj, protocol=0):
    # Python 2 : cPickle.dumps -> str, concaténé à "save_script_data:clé:" ou écrit dans un fichier texte
    return _p.dumps(obj, protocol=0).decode(CP1252W)
def loads(s):
    if isinstance(s, str):
        s = s.encode(CP1252W)          # retrouve les octets d'origine (valeur .bp ou fichier lu en cp1252)
    return _p.loads(s, encoding="cp1252", errors="replace", fix_imports=True)
def dump(obj, f, protocol=0):
    f.write(dumps(obj, protocol) if "b" not in getattr(f, "mode", "") else dumps(obj).encode(CP1252W))
def load(f):
    return loads(f.read())
PicklingError, UnpicklingError = _p.PicklingError, _p.UnpicklingError
```

Le même codec `CP1252W` est utilisé partout où l'hôte convertit des octets « Windows » en `str` (valeurs `bp_script_data`, `open()` de compatibilité), ce qui garantit l'aller-retour octet pour octet.

* Les pickles Python 2 sont en protocole 0 : chaînes `S'caf\xe9'` (octets CP1252 échappés) → décodés en `str` via `encoding='cp1252'` ; entiers longs `L123L` acceptés ; références `copy_reg._reconstructor` / `__builtin__` traduites par `fix_imports`.
* Les pickles écrits par Python 3 (opcode `V`, raw-unicode-escape : caractères ≤ U+00FF en octets bruts, au-delà `\uXXXX`) sont relus à l'identique grâce au codec sans perte.
* Usages réels : 14 fichiers ; `"save_script_data:clé:" + cPickle.dumps(...)` ×12 (ex. `AllAccountsView/all_accounts_view.py:90-91`) ; `load`/`dump` sur des fichiers ouverts en mode **texte** `"r"`/`"w"` : `Auto Fields/auto_fields.py:254,379`, `Notes/notes.py:78,88`, `Notes/notes_auto.py:16`, `Receipt/receipt.py:73,79`, `Scheduler/scheduler.py:46,54` (d'où le `dump` acceptant un fichier texte).
* Classes définies dans le script et picklées (`__main__.X`) : rechargées si le script redéfinit la classe avant `loads` (identique à Python 2).

**`nt`** : module natif de CPython **sous Windows uniquement** (et sans `popen3` en Py3). Usages réels (13 appels) : `lstat` ×4 (`Backups/backups.py:10,16`, `ManagePlugins/manage_plugins.py:8`, `Scheduler/scheduler.py:324`, avec test `st_mode & 0170000 == 0040000`), `listdir` ×3, `rename` ×2, `unlink` ×2, `rmdir` ×1, **`popen3`** ×1 (`ManagePlugins/manage_plugins.py:71`). Fournir **toujours** `bp_compat/nt.py` (prioritaire sur le module natif) ré-exportant `os` + `popen3(cmd)` émulé par `subprocess.Popen` renvoyant `(stdin, stdout, stderr)` en mode texte.

**`string`** : la stdlib 3 garde le module `string` sans les fonctions : surcharger avec `split`, `join`, `joinfields`, `splitfields`, `strip`, `lstrip`, `rstrip`, `lower`, `upper`, `capitalize`, `replace`, `find`, `rfind`, `index`, `count`, `zfill`, `atoi`, `atof`, `atol`, `ljust`, `rjust`, `center`, `expandtabs`, `translate`, `maketrans`, constantes `letters`, `lowercase`, `uppercase`, `digits`, `whitespace`, `punctuation`.

**Autres modules** : `sets` (`Set = set`, `ImmutableSet = frozenset`), `md5`/`sha` (via `hashlib`), `StringIO`/`cStringIO` (`io.StringIO`), `copy_reg` (`copyreg`), `exceptions`, `urllib`/`urllib2`/`httplib` (sous-ensemble : `urlopen`, `quote`, `urlencode`), `time` (ajouter `time.clock = time.perf_counter`), `sys` (attributs `maxint = sys.maxsize`, `setdefaultencoding` no-op, `exc_type`/`exc_value` via `sys.exc_info()`), `os.getcwdu`.

### 5.3 bis Constructions Python 2 réellement présentes dans les scripts livrés

| Construction | Occ. | Emplacements | Traitement |
|---|---|---|---|
| instruction `print` | 1 | `MultipleInput/multiple_input.py:94` | fixer `print` |
| `print(a, b)` (affiche un tuple en 2.x) | 1 | `Transfer/transfer.py:108` | sans importance (sortie perdue) |
| `raise X, "msg"` | 2 | `Import CSV/import_csv.py:33,47` | fixer `raise` |
| `d.has_key(k)` | 9 | `Notes/notes.py:16`, `Scheduler/scheduler.py:505`, `Import QIF/import_qif.py:219-220`, `Transfer/convert_line_to_transfer.py:114`, `Import CSV/import_csv.py:589-590`, `Export QIF/export_qif.py:25`, `Export CSV/export_csv.py:23` | fixer `has_key` |
| `<>` | 3 | `Auto Fields/auto_fields.py:486,493,500` | fixer `ne` |
| `long(...)` | 6 | `Numerals/numerals.py:6,39`, `Receipt/receipt.py:8`, `Rib/rib.py:9` | builtin `long = int` |
| octaux `0170000`, `0040000` | 6 | `Backups/backups.py:10`, `ManagePlugins/manage_plugins.py:8`, `Scheduler/scheduler.py:324` | fixer `numliterals` |
| `.next()`, `xreadlines()` | 2 / 1 | `Import CSV/import_csv.py:49,59` / `:34` | fixers `next` ; `xreadlines` → itération directe |
| `keys = d.keys(); keys.sort()` | 16 | ex. `Notes/notes_auto.py:22`, `Scheduler/previsions.py:58`, `Budget/budget.py:639`, `Filters/setfilters.py:60,159`, `Import OFX/import_ofx.py:118,204` | fixer `dict` (→ `sorted(...)`/`list(...)`) |
| `list.sort()` de tuples `(clé, dict)` | 2 | `BudgetYear/budget_year.py:68-69`, **`Scheduler/scheduler.py:61-62`** (dates égales fréquentes ⇒ comparaison de `dict` ⇒ `TypeError` en Py3) | fixer maison : `sort(key=lambda t: t[0])` quand le 2ᵉ élément est un dict, ou `cmp` Python 2 complet (ordre des dict compris) via `functools.cmp_to_key(__bp_cmp__)` appliqué à tout `.sort()`/`sorted()` sans `key` |
| `str.translate(table, deletechars)` | 3 | `Import CSV/import_csv.py:152`, `Import QIF/import_qif.py:14`, `MultipleInput/multiple_input.py:7` | fixer maison → `__bp_translate__(s, table, deletechars)` |
| `str.translate(table_256)` / `string.maketrans` | 4 | `Currencies/currencies.py:11,14,144`, `PeopleTotals/people_totals.py:25` | fonctionne en Py3 : une `str` de 256 caractères est acceptée comme table indexée par code point (caractères > U+00FF, ex. `€`, laissés inchangés) ; fournir `string.maketrans` de compatibilité |
| `round(x, 2)` | 5 | `Ventilate/ventilate.py:7,19,30,65` | builtin `round` Python 2 |
| division `/` | 185 opérateurs | critiques (résultat utilisé comme entier) : calcul de dates `to_delphi_date` (`Budget/budget.py:158`, `Export XLS/export_xls.py:45`, `Loan/loan.py:42`, `Scheduler/scheduler.py:129`, `Transfer/transfer.py:57`), Pâques (`Scheduler/scheduler.py:201-204`, `Scheduler/scheduler_auto.py:81-84`), jour de semaine (`Scheduler/scheduler.py:197-198`), `DecToText` (**boucle infinie** en Py3 : `Numerals/numerals.py:50,52,67`, `Receipt/receipt.py:19,21,36`), `Import CSV/import_csv.py:6`, index de liste `BudgetYear/budget_year.py:335`, `Calc/calc.py:67`, `Scheduler/previsions.py:105`, ~100 calculs de pixels | transformation AST `__bp_div__` (§5.2) — **indispensable** |
| `global` au niveau module | 3 | `Scheduler/scheduler_auto.py:5`, `ExtraValues/extra_values_edit.py:5`, `Export QIF/export_qif.py:6` | accepté par Py3 (sans effet) |
| échappement invalide `"C:\Windows\Media\Tada.wav"` | 1 | `test/test.py:21` | `SyntaxWarning` seulement (à filtrer) |
| `except X, e`, backticks, `exec`, `unicode`, `xrange`, `apply`, `reduce`, `cmp`, `sort(cmp)`, module `string` | 0 | — | shims fournis quand même pour les scripts tiers |

Bogues latents des scripts (masqués aujourd'hui, à ne pas « corriger » par l'hôte) : `Loan/loan.py:394` (`rates` non défini, dans un `try/except: pass`).

### 5.4 Différences sémantiques non couvrables par conversion

| Différence | Risque | Atténuation hôte |
|---|---|---|
| `str` = octets (2.3) vs Unicode (3) | comparaisons/concaténations mixtes, `len()` d'une chaîne accentuée (identique en CP1252 : 1 octet = 1 caractère), `ord()` > 255 pour `€` (0x80 en CP1252 vs U+20AC) | toutes les entrées/sorties hôte en `str` ; `ord('€')` diffère (rarement utilisé) |
| `str(float)` : 12 chiffres significatifs en 2.3 (`str(0.1+0.2) == '0.3'`), repr minimal en 3 (`'0.30000000000000004'`) | affichage de montants, chaînes renvoyées à l'hôte, `%s` sur des flottants | côté hôte : toute conversion float→texte reçue d'un script (setters, propriétés VCL `Text`/`Caption` recevant un float) utilise `'%.12g'` ; impossible de corriger `str()` dans le script lui-même → l'analyseur signale les `str(x)`/`"%s" % x` sur flottants pour revue |
| `repr(float)` : 17 chiffres en 2.3 | idem, rare | — |
| division entière | cf. §5.2 (transformation AST) | — |
| `round()` | banker's rounding + `int` en 3 | builtin de compatibilité |
| comparaisons hétérogènes (`None < 0`, `"a" < 1`, `[] < ()`) | `TypeError` en 3 (tri de listes contenant `None`, `max()`…) | aucune correction générique fiable ; `cmp` de compat ; repérer les tris |
| ordre des `dict` | arbitraire en 2.3, insertion en 3 | sans risque (aucun script ne peut dépendre d'un ordre arbitraire) |
| `int`/`long` | unifiés ; `repr(10L) == '10L'` en 2 | sans effet hors affichage |
| `True`/`False` | en 2.3 ce sont des `int` 1/0, `str(True) == 'True'` | identique |
| exceptions | attributs `e.args` identiques ; `e.message` n'existe pas en 2.3 | — |
| `print` en mode GUI | perdu | rediriger vers un journal |
| `input()` | évaluait l'expression | aucun usage attendu (pas de console) |
| itérateurs `map/filter/zip/dict.keys()` | corrigés par 2to3 sauf usages indirects | — |
| locales | `DecimalSeparator` Windows influençait les conversions hôte (`AccountCurrency`, `OperationAmount` en écriture) | l'hôte accepte `,` et `.` en entrée ; en sortie, choisir `.` pour `AccountCurrency1/2` (les scripts font `float(x.replace(",", "."))` ou `split("\|")`) — à valider sur les scripts |

### 5.5 Compatibilité de l'API hôte (exigences pour le portage)

* Toutes les fonctions acceptent des arguments **positionnels**, tolèrent les arguments en trop, convertissent `int`/`float`/`str` entre eux comme le faisait le Variant Delphi (`"3"` accepté pour un entier, `3` pour une chaîne).
* Séquences `BP.Account*`, `BP.Categ*`, `BP.Operation*` : implémenter `__len__`, `__getitem__` (index négatifs, tranches **bornées**), `__setitem__` (ou `TypeError` pour les attributs RO de niveau 1, silence pour les RO de niveau 2), `__contains__` insensible à la casse, `__iter__`, `__repr__` au format `"[a, b]"`.
* Valeurs de retour **à l'identique** (formats des chaînes, séparateurs `;` `|` `-` `=`, CRLF finaux, `"OK"`/`"KO"`, `"ok"`/`"ko"`, `"1"`/`"0"`) — y compris les écarts à la doc signalés ⚠, car les scripts ont été écrits contre le comportement réel.
* Corrections de bogues sans risque (sur-ensemble) : `Categs()` en 2-uplets, `LineSelect(n>=0)` renvoie `n`, `BankPerfectCloseFile()` renvoie `None`, tranches bornées, `TMaskEdit` renvoie sa valeur dans `CustomForm`, `picture.height` pris en compte, `ExportChart(panel)` avec chemin par défaut `bp_export_folder + "bankperfect_graph.pdf"`, erreurs hôte → `BP.BPError`.
* Un interpréteur Python 3 **unique** (PyO3) peut être conservé entre exécutions pour la performance, à condition de **réinitialiser complètement** le namespace (`__main__` neuf, modules `BP`/compat ré-importés, `sys.modules` des modules utilisateur purgé) afin de reproduire « un interpréteur neuf par exécution » ; conserver la non-réentrance (file d'attente ou refus silencieux) ; exécuter dans le thread GUI ou avec un modèle d'appels synchrones vers la GUI (les scripts bloquent sur `ShowModal`/`MsgBox`).

---

## 6. Plugins DLL (abandonnés) et interface COM

* `Plugins\**\*.dll` exportant `procedure Execute(BP: IBankPerfectPlugin); stdcall` + `plugin.ini` (`Name`, `Path` = chemin de menu `A>B` sous le menu Plugins, `ReadOnly`, `Hidden=1`), icône `<dll>.png` (`LoadPlugins`, `unitMain.pas:6527-6597` ; `LoadPlugin`, `unitMain.pas:6106-6127`). Deux plugins présents (`AMD-Cube`, `soldeProgressif`). **Abandonnés** : ne pas réimplémenter.
* `IBankPerfect.pas` déclare l'interface `IBankPerfectPlugin` (GUID `{E5860C65-4A93-4619-8A15-1C9D19E804BA}`, méthodes `safecall`). **Recouvrement total** : le module `BP` n'est qu'un adaptateur Python de cette interface (mêmes noms, mêmes sémantiques) ; seules les séquences `Operation*`/`Account*` sont présentées en style indexé Python, et `OperationExtra` (lecture par clé) n'est exposé qu'au travers de `extra_get`/`extra_set`. `UnitPlugins.pas` (classe `TBankPerfectPlugin`) en est l'**implémentation unique**, partagée par plugins et scripts : elle doit être portée.
* `ObjectPlugins.pas` est vide.
* La doc indique que `BankPerfectPluginPath()` « retourne le chemin complet du plugin qui l'appelle » : pour les scripts c'est le format `"|script"` du §3.1.

---

## 7. Points d'incertitude

1. **Python4Delphi** n'est pas présent dans le dépôt : le comportement exact de WrapDelphi/WrapDelphiVCL (noms enregistrés dans `__main__`, conversion des ensembles/énumérations, propriété `Selected` ajoutée aux `TListBox` par l'auteur d'après history.txt 6.51, accès au `Canvas`) est **déduit des usages des scripts** (§4.2), pas du code source.
2. Valeur exacte de `sys.path[0]` en Python 2.3 embarqué (§1.2).
3. Conversion `WideString` → objet Python par `VariantAsPyObject` (`str` vs `unicode`) : déduite des scripts (concaténations avec des littéraux accentués) — sans objet en Python 3.
4. Comportement de P4D quand un handler renvoie `NULL` sans exception (`BankPerfectCloseFile`, erreurs `GetURL`) : `SystemError` présumé.
5. Liste et numérotation des ~58 icônes internes (`Icon=<n>`) : à extraire de la ressource `PNGImageList1` de `unitMain.dfm`.
6. Sortie de `print` en mode GUI : présumée perdue.
7. Ordre exact de déclenchement des évènements VCL lors des affectations programmatiques (`OnChange`, `OnClick`) et de `SetProps` : déduit des gardes présentes dans les scripts, à valider par des tests de non-régression script par script.
8. Valeur de retour de `ShowModal` à la fermeture par la croix (`mrCancel` = 2 en VCL standard) et absence de libération des fenêtres : comportement VCL standard présumé.
9. Les constantes `MB_*`/`IDYES` et `Rect` sont supposées venir des unités P4D (WrapDelphiWindows / WrapDelphiTypes) : liste complète des noms exportés inconnue — l'hôte doit fournir au moins ceux du §4.2.1.
10. Format exact des lignes `ftp_lst` (`nature` = `d`/`-` ?) dépend de `ParseLine` d'`UnitDownload.pas` (non détaillé) ; l'usage FTP est de toute façon lié à des serveurs disparus.
