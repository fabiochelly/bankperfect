# 02 — Spécification du format de fichier `.bp` / `.bp0`

> Objectif : permettre à une implémentation Rust de **lire** tous les fichiers BankPerfect (formats actuel et anciens) et de **réécrire** un `.bp` **octet pour octet identique** à ce que produirait l'application Delphi (sources de mars 2019, `unitMain.pas` 7459 lignes).
>
> Toutes les références `fichier.pas:ligne` renvoient aux sources Delphi de `/home/fabio/Work/bankperfect` (numérotation identique en CP1252 et après `iconv`).
>
> Les exemples proviennent exclusivement de `Exemple.bp`.

---

## 0. Vue d'ensemble

Un fichier `.bp` est un **texte CP1252** obfusqué par une substitution polyalphabétique (« codec V5 »). Une fois déchiffré :

```
BP-FILE-5utilisateur=...\n          <- marqueur de version collé à la 1re ligne d'en-tête
guid=...\n
UserName=...\n
modes=...\n
compte0=...\n
...
compteN-1=...\n
categ=...\n
accounts_order=...\n
who0=...\n
...
compte_defaut=...\n
pass=...\n
bp_script_data_xxx=...\n            <- 0..n lignes
TNSLPPBNTSOGPTQBC280673<op1 compte0>\n   <- séparateur + opérations du compte 0
<op2 compte0>\n
...
TNSLPPBNTSOGPTQBC280673<op1 compte1>\n   <- séparateur + opérations du compte 1
...
```

Couches, de l'extérieur vers l'intérieur :

| Couche | Description | Section |
|---|---|---|
| Physique | octets du fichier ; fins de ligne LF uniquement à l'écriture | §2 |
| Obfuscation | codec V5 (alphabet de 77 caractères, décalage dépendant de la position et de la longueur totale) | §2 |
| Marqueur | `BP-FILE-5` (9 octets) en tête du texte déchiffré | §2.5 |
| En-tête | lignes `clé=valeur` (sémantique `TStringList`) jusqu'au 1er séparateur | §4 |
| Comptes | un bloc par compte, introduit par `TNSLPPBNTSOGPTQBC280673` | §5 |
| Opérations | une ligne par opération, 9 champs séparés par `;` | §6 |
| Champs supplémentaires | 9e champ : paires `clé=valeur`, `;` encodé `<\|\|>` | §6.10 |

Formats anciens, en **lecture seule** (jamais réécrits) : UCL/NRV2E « BPXENCODE » (BankPerfect 5.x, 2005) et « Shuffle » ISAAC (BankPerfect 3.5 → 4.x, 2003). Voir §11.

---

## 1. Détection du format à l'ouverture

`TFormMain.loadArchiveFiles` (`unitMain.pas:4805-4882`) essaie, **dans cet ordre, avec court-circuit** :

1. `LoadFileV5(CurrentBP, FileContent)` — `UnitStrings.pas:191-218` (format courant, §2)
2. `UPXLoadFile(CurrentBP, FileContent)` — `ucl.pas:29-57` (§11.1)
3. `ShuffleLoad(CurrentBP, FileContent)` — `ObjectShuffle.pas:142-159` (§11.2)

Si les trois échouent → `Result := False` → `CorruptedFile` (`unitMain.pas:5652`) propose de lancer le script `backups.py` (liste des sauvegardes). Toute exception pendant le parsing (ex. tiers mal formé, §4.8) mène au même résultat (`unitMain.pas:4876-4880`).

Les trois loaders renvoient le **texte en clair sans marqueur** (`BP-FILE-5`, `BPXENCODE`, `FABIO_SHUFFLE` retirés). Le parsing qui suit est commun.

Le même enchaînement est utilisé par l'outil caché de déchiffrement (§12).

---

## 2. Couche physique et codec V5

### 2.1 Encodage des caractères

- Les chaînes Delphi 7 sont des `AnsiString` (1 octet = 1 caractère) dans la page de code ANSI du système, **CP1252** sur un Windows français. Aucun BOM.
- Le fichier est lu/écrit en binaire brut (`OpenTextFile`/`SaveTextFile`, `UnitStrings.pas:108-133` : `TFileStream`, `SetString`, `WriteBuffer`).
- Le codec ne touche **que** les 77 caractères de l'alphabet (§2.2) ; tous les autres octets (lettres accentuées, `€` = 0x80, `!"#$%&'*/?@\^`{|}~`, CR, LF, octets ≥ 0x80…) sont recopiés tels quels.
- **Exigence Rust** : manipuler le contenu comme `Vec<u8>` / décodage CP1252 **sans perte**. Les octets non définis en CP1252 (0x81, 0x8D, 0x8F, 0x90, 0x9D) doivent survivre à un aller-retour (ex. `encoding_rs::WINDOWS_1252`, qui les mappe sur U+0081… et les ré-encode à l'identique, ou travailler directement en octets).

### 2.2 Tables du codec (`UnitStrings.pas:69-83`)

Alphabet `chars[0..76]` (77 caractères, ordre exact, l'espace est en position 5) :

```
_8lL6 ,BZu2jp-ND0<(CPeE1AQyFTMG;[3Rxk4H)qdn.JVz7obW]:=OgKXIf>rmsYt+5ahicwSvU9
```

`positions[c]` = index de `c` dans `chars`, ou −1 (vérifié : la table `positions` de 256 entrées du source correspond exactement à l'inverse de `chars`).

Caractères ASCII imprimables **absents** de l'alphabet (donc non chiffrés) : `! " # $ % & ' * / ? @ \ ^ ` { | } ~`.

`deltas[0..99]` :

```
241,208,211,321,243,266,267,159,305, 22,223, 23,  6,152,120,396,215,376,297,  2,
101,185, 16,  2, 80,116,291,147,314,  5, 47,184, 32,223, 16, 74,396, 61,344,362,
278,330,341, 65, 55,199,276,229,139,157,377,116,227,361,262, 67,154,243, 98,127,
230,351, 98,219,262,293,223,182,385,287, 18,112,154,272,193,115, 98,393, 16,136,
 41,271,318,345, 47, 82,165, 79,356,129,112,296,326,190,  5, 46,249, 13,367, 44
```

Ces mêmes tables sont dupliquées dans l'outil `codec/UnitMain.pas:52-68` (identiques).

### 2.3 Chiffrement — `EncryptCodec5` (`UnitStrings.pas:157-175`)

```
L = longueur totale (octets) de la chaîne à chiffrer
pour i = 1..L (indices 1-based) :
    p = positions[s[i]]
    si p >= 0 : s[i] = chars[(p + deltas[i mod 100] + L) mod 77]
```

Points d'attention :
- `i` est **1-based** : le 1er octet utilise `deltas[1]`, le 100e `deltas[0]`.
- `L` est la longueur de la chaîne **complète** (marqueur `BP-FILE-5` et fins de ligne LF compris). La clé dépend donc de la taille du fichier : un fichier tronqué ou dont la longueur change (conversion LF→CRLF, ajout d'un octet) devient illisible.

### 2.4 Déchiffrement — `DecryptCodec5` (`UnitStrings.pas:135-155`)

```
L = longueur ; compensation = 77 * (L div 77 + 1) + 770 - L     // ≡ -L (mod 77), toujours >= 771
pour i = 1..L :
    p = positions[s[i]]
    si p >= 0 : s[i] = chars[(p - deltas[i mod 100] + compensation) mod 77]
```

Équivalent mathématique : `chars[(p - deltas[i % 100] - L).rem_euclid(77)]`. La « compensation » ne sert qu'à éviter un modulo négatif en Pascal.

### 2.5 `SaveFileV5` — écriture physique (`UnitStrings.pas:220-227`)

```
FileContent = "BP-FILE-5" + FileContent
FileContent = remplacer tout CRLF par LF
FileContent = remplacer tout CR restant par LF
EncryptCodec5(FileContent)
écrire FileContent tel quel (fmCreate : création/troncature)
```

→ **Un fichier écrit par BankPerfect ne contient que des LF**, jamais de CR. Le fichier se termine par LF (dernière ligne d'opération) sauf si le dernier compte est vide (voir §5.3).

### 2.6 `LoadFileV5` — lecture physique (`UnitStrings.pas:191-218`)

```
raw = contenu brut
crlf = GetCRLF(raw)                // "\r\n" si présent, sinon "\r" si présent, sinon "\n"   (UnitStrings.pas:177-182)

# Tentative 1
c = remplacer (crlf+crlf) par crlf dans raw        // 1 passe, gauche→droite, sans chevauchement
DecryptCodec5(c)
si c commence par "BP-FILE-5" : OK

# Tentative 2 (le fichier a peut-être subi une conversion de fins de ligne)
crlf2 = "\r\n" si crlf == "\n" sinon "\n"
c = remplacer (crlf+crlf) par crlf dans raw, puis remplacer crlf par crlf2
DecryptCodec5(c)
si c commence par "BP-FILE-5" : OK
sinon : échec (FileContent = '')

OK : retirer les 9 premiers octets ("BP-FILE-5")
```

Conséquences :
- Un fichier passé en CRLF (transfert FTP texte, Git `autocrlf`…) reste lisible : la tentative 2 convertit CRLF→LF, ce qui restaure la longueur d'origine. **`Exemple.bp` est dans ce cas** (12 942 octets avec CRLF ; 12 799 octets après normalisation).
- La « réduction des lignes vides » (`crlf+crlf → crlf`) est appliquée **avant** déchiffrement ; si le contenu légitime contenait une ligne vide, la longueur changerait et le déchiffrement échouerait. BankPerfect n'écrit jamais de ligne vide (sauf données pathologiques contenant CR/LF, §6.12).
- `FastReplace` (`FastStrings.pas:907`) remplace toutes les occurrences en une passe (non récursive) : `\n\n\n\n` → `\n\n`.
- `IsValidV5File` (`UnitStrings.pas:184-189`) : comparaison exacte, sensible à la casse, des 9 premiers caractères.

### 2.7 Outil de réparation (`codec/UnitMain.pas`)

Programme séparé `codec-bp.exe` : ouvre/édite/réenregistre un `.bp` en clair. Sa fonction « Corriger » (`TestFile`, `codec/UnitMain.pas:139-183`) essaie les 77 valeurs de compensation possibles (771..847) avec `DecryptCodecLen` (`:117-137`) jusqu'à trouver une chaîne attendue (par défaut `utilisateur=`) : permet de récupérer un fichier dont la longueur a changé sans décalage de position. Non nécessaire pour la parité, mais une commande « réparer » équivalente serait un plus.

---

## 3. Contenu en clair : structure globale

### 3.1 Découpage en-tête / comptes (`unitMain.pas:4822-4831`, `4852-4862`)

```
FileSeparator = 'TNSLPPBNTSOGPTQBC280673'     // unitMain.pas:658  (23 octets)
LengthFS      = 22                            // unitMain.pas:659
```

- `i = Pos(FileSeparator, contenu)` (1-based). S'il n'y a **aucun** séparateur : tout le contenu est l'en-tête et il n'y a aucun compte → `OL.Count = 0` → fichier considéré **corrompu** (`unitMain.pas:4864-4869`).
- Sinon en-tête = `Copy(contenu, 1, i-1)` puis `Delete(contenu, 1, i + LengthFS)`. Avec `i` 1-based, `Delete(…, 1, i+22)` supprime les positions 1..i+22, c'est-à-dire **exactement** l'en-tête et les 23 octets du séparateur (le `22` n'est pas une erreur, c'est `Length-1` compensé par le 1-based).
- Ensuite, tant qu'un séparateur est trouvé : le texte avant lui est le bloc du compte suivant ; le reste après lui continue. Le texte restant final, **s'il est non vide**, est le dernier bloc.

### 3.2 Le séparateur `TNSLPPBNTSOGPTQBC280673`

- Chaîne magique opaque de 23 caractères : 17 majuscules `TNSLPPBNTSOGPTQBC` + `280673` (le 28/06/1973 ; le même nombre sert au mot de passe maître §10.2 et à l'outil caché §12). Aucune signification fonctionnelle n'a été trouvée dans le code : c'est une sentinelle choisie pour ne jamais apparaître dans des données.
- Ce n'est **pas** une ligne : il est collé au début du bloc. Comme la ligne précédente se termine par LF, il apparaît en début de ligne, et la **première opération du compte suit immédiatement sur la même ligne** : `TNSLPPBNTSOGPTQBC28067339444;;22;hyper u ;;0;-1,78;39444;UId=1`.
- Compte sans opération : le bloc est vide, deux séparateurs se suivent (`…\nTNSLPP…280673TNSLPP…280673…`).
- Aucune donnée ne doit contenir cette chaîne (aucun échappement n'existe ; le parseur la prendrait pour un séparateur).

### 3.3 Fins de ligne et découpage en lignes

- À l'écriture : chaque ligne (en-tête et opérations) est terminée par CRLF dans la `TStringList` (`TStrings.Text`, `sLineBreak = #13#10`), puis **convertie en LF** par `SaveFileV5`. Sur disque, après déchiffrement : **LF uniquement**.
- À la lecture : en-tête et blocs sont découpés par `TStringList.Text := …` (Delphi 7 `TStrings.SetTextStr`) :
  - coupe sur CR, LF ou CRLF (CRLF = un seul saut) ;
  - une fin de ligne finale ne crée pas de ligne vide supplémentaire ;
  - une ligne vide intermédiaire crée une entrée vide (ignorée ensuite : moins de 7 champs) ;
  - **un octet NUL (#0) termine le découpage** (la suite du bloc est ignorée) — comportement Delphi 7 à connaître, pas à reproduire volontairement.

### 3.4 Champs et séparateur `;`

- `Split(List, s, ';')` (`UnitStrings.pas:374-387`) : découpage simple, **sans échappement ni guillemets, sans trim** ; chaîne vide → **0 élément** ; `"a;"` → 2 éléments (`"a"`, `""`).
- `Merge(List, ';')` (`UnitStrings.pas:389-401`) : jointure simple.
- Il n'existe **aucun mécanisme d'échappement** du `;` dans les champs texte (tiers, détail, noms de comptes, catégories, modes…). L'interface l'interdit à la saisie : `filtre_edit` (`unitMain.pas:1971-1982`) et `EPeopleKeyPress` (`unitMain.pas:4525-4528`) suppriment `;` et Entrée ; `UnitAccounts.pas:684` remplace `;` par espace ; `UnitModes.pas:112` supprime `;` ; l'API plugins remplace `;` par espace (`UnitPlugins.pas:244,255,320,324`). Seul le 9e champ des opérations a un échappement (§6.10).

---

## 4. En-tête

### 4.1 Sémantique de lecture (`TStringList.Values`)

L'en-tête est chargé dans une `TStringList` (`User`/`lignes_groupe`). Les accès se font par `Values[clé]` / `IndexOfName(clé)` (Delphi 7) :
- nom = texte avant le **premier** `=` ; valeur = tout ce qui suit (peut contenir d'autres `=` et des `;`) ;
- comparaison des noms **insensible à la casse** (`AnsiCompareText`) ;
- en cas de doublon, **la première** ligne gagne ;
- clé absente → `''`.
- Les lignes sans `=` sont ignorées.

**Clés inconnues : perdues à la sauvegarde.** Seules les clés listées ci-dessous sont réécrites (`unitMain.pas:2591-2614`).

### 4.2 Ordre exact d'écriture (`TFormMain.ASaveExecute`, `unitMain.pas:2590-2614`)

| # | Ligne écrite | Source de la valeur |
|---|---|---|
| 1 | `utilisateur=<nom_groupe>` | §4.3 |
| 2 | `guid=<BPFileGUID>` | §4.4 |
| 3 | `UserName=<UserName>` | §4.5 |
| 4 | `modes=<Merge(Modes, ';')>` | §4.6 |
| 5 | `compte<i>=<TAccount.SaveToString>` pour i = 0..AccountList.Count−1 | §4.7 |
| 6 | `categ=<CategsList.SaveToString>` | §4.9 |
| 7 | `accounts_order=<AccountsOrderList.SaveToString>` | §4.10 |
| 8 | `who<i>=<TPeople.SaveToString>` pour i = 0..Peoples.Count−1 | §4.8 |
| 9 | `compte_defaut=<RawAccountsListIndex>` | §4.11 |
| 10 | `pass=<EPassword>` | §4.12 |
| 11 | `bp_script_data_<clé>=<valeur>` pour chaque donnée de script, dans l'ordre interne | §4.13 |

Puis, pour chaque compte i = 0..`RawAccountsList.Count`−1 : `FileSeparator` + lignes d'opérations de `ol_comptes[i]` (§5, §6).

`Exemple.bp` respecte exactement cet ordre.

### 4.3 `utilisateur`

- Lecture : `nom_groupe := Values['utilisateur']` (`unitMain.pas:4836`), `''` si absent.
- Écriture : valeur brute.
- À la création d'un fichier : nom saisi pour le fichier (sans extension) (`unitMain.pas:2684`, `ANewFileExecute` `:2692-2722`). Aucune interface ne le modifie ensuite.
- Exemple : `utilisateur=Fabio`.

### 4.4 `guid`

- Lecture : `BPFileGUID := Values['guid']` ; si vide → `CreateBPGUID` (`unitMain.pas:4838-4839`, `6637-6644`) :
  `GUIDToString(CreateGUID)` (format `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`, hexadécimal **majuscule**) + `-` + `FormatDateTime('ddmmyyyyhhnnsszzz', Now)` + `-` + chemin complet du fichier courant.
- Le GUID généré n'est écrit qu'à la prochaine sauvegarde (le fichier n'est pas marqué modifié).
- Exemple : `guid={35F83405-0CDC-4DC8-8F71-6530905AE342}-25032006093419843-C:\devs\BankPerfect\Omer Dalors.bp`.
- Usage : identifiant stable du fichier, exposé aux scripts.

### 4.5 `UserName`

- Lecture : `s := Values['UserName']; if s <> '' then UserName := s` (`unitMain.pas:4099-4100`).
- **Particularité** : si la clé est absente ou vide, la variable **garde la valeur du fichier précédemment ouvert** dans la session (et sera écrite telle quelle). Aucun écran ne modifie `UserName`. Une implémentation iso doit soit reproduire ce comportement (variable de session), soit — recommandé — réécrire la valeur lue (vide). À trancher (question ouverte Q3).
- Écriture : valeur brute. Exemple : `UserName=Fabio`.

### 4.6 `modes` — modes de paiement personnalisés

- Lecture (`unitMain.pas:4846-4852`) : `Split(l, Values['modes'])` ; on ajoute chaque élément `e` à `Modes` si `Trim(e) <> ''` **et** `Modes.IndexOf(e) = -1` (comparaison **insensible à la casse**). Les doublons et vides sont donc supprimés à la lecture.
- Préfixe `-` : mode de **dépense** (montant négatif) ; sans préfixe : mode de **recette** (`GetSignFromIndex`, `unitMain.pas:4735-4746` ; affichage `UnitModes.pas:60-68`).
- Liste des modes affichée (`ModeList`, `CreateModeList`, `unitMain.pas:4791-4802`) = les 10 modes standard (§6.4) puis les modes personnalisés **sans** le `-`.
- Écriture : `Merge(Modes, ';')` (avec les `-`). Liste vide → `modes=` (ligne présente, valeur vide).
- Exemple : `modes=-Carte FNAC` (un mode de dépense « Carte FNAC »).

### 4.7 `compteN` — comptes

#### Lecture (`ObjectAccountList.pas:133-144`, `151-195`)

```
i = 0
tant que IndexOfName('compte' + i) > 0 :     // strictement > 0 : la clé ne doit pas être la 1re ligne de l'en-tête
    s = Values['compte' + i]
    ajouter TAccount.Create(s, i)
    i += 1
```
- Les clés doivent être **contiguës** depuis `compte0` ; la première absente arrête la lecture.
- `IndexOfName > 0` (et non `>= 0`) : si `compte0` était la toute première ligne, aucun compte ne serait lu (impossible avec un fichier écrit par BP, `utilisateur` étant toujours en 1re position).
- `TAccount.Create` : `Split(s, ';')`, champ absent → `''`.

#### Champs positionnels (17 champs, index 0-based)

| Idx | Champ Delphi | Signification / interface (`UnitAccounts.pas:160-175`, `206-234`) | Lecture | Écriture (`SaveToString`, `ObjectAccountList.pas:228-255`) |
|---|---|---|---|---|
| 0 | `BankName` | Nom de la banque | brut | `%s` |
| 1 | `AgencyName` | Nom de l'agence | brut | `%s` |
| 2 | `AgencyAddress` | Adresse de l'agence | brut | `%s` |
| 3 | `AccName` | **Nom du compte** (affiché partout ; utilisé pour retrouver le compte d'un transfert « Transfert vers le compte <AccName> ») | brut | `%s` |
| 4 | `AccNumber` | Numéro de compte (RIB) | brut | `%s` |
| 5 | `Rate1` | Taux de la devise principale (en pratique `1`) | `Str2Extended` ; exception → `1` | `%g` (§4.7.1) |
| 6 | `Curr1` | Symbole devise principale (`€`) | brut | `%s` |
| 7 | `Rate2` | Taux de la devise secondaire (ex. `6,55957` F/€) | `Str2Extended` ; exception → `6.55957` | `%g` |
| 8 | `Curr2` | Symbole devise secondaire (`F`) | brut | `%s` |
| 9 | `BankNumber` | Code banque (5 chiffres, complété à gauche par des 0 à la saisie) | brut | `%s` |
| 10 | `AgencyNumber` | Code guichet | brut | `%s` |
| 11 | `BIC` | BIC | brut | `%s` |
| 12 | `Picture` | Nom affiché du logo de banque (ex. `CIC`, `SOCIETE GENERALE`, `Aucun`) ; fichier `logos\<nom en minuscules, espaces→_>.png` (`UnitAccounts.pas:481-486`) | brut | `%s` |
| 13 | `IBAN` | IBAN (texte libre, espaces conservés) | brut | `%s` |
| 14 | `IsVirtual` | `virtual` = compte virtuel ; toute autre valeur = réel | `= 'virtual'` (sensible à la casse) | `virtual` ou `real` |
| 15 | `AccUID` | Identifiant unique du compte | brut ; si vide → `CreateAccountUID(i)` | `%s` |
| 16 | `IsDisabled` | `disabled` = compte désactivé (masqué du menu des comptes) | `= 'disabled'` | `disabled` ou **chaîne vide** |

- La clé RIB n'est **pas** stockée (recalculée, `calcul_clef`).
- `IsVirtualAccount` (`ObjectAccountList.pas:120-126`) considère aussi comme virtuel un compte dont `AccName` est vide ou commence par `_` (règle d'affichage/calcul, pas de stockage).
- **Le champ 16 est toujours écrit** : un compte actif se termine donc par `;` (champ vide). Les fichiers écrits avant BankPerfect 8.0 bêta 4 (septembre 2014, `history.txt:7`) n'ont que 16 champs (c'est le cas de `Exemple.bp`) ; la version actuelle ajoutera le `;` final à la première sauvegarde.
- `CreateAccountUID(index)` (`ObjectAccountList.pas:58-71`) : `FormatDateTime('yyyymmddhhnnss', Now) + '-' + IntToStr(index) + '-' +` 5 caractères tirés par `RandomRange(1, 62)` dans `abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789` (1-based, donc le dernier caractère `9` n'est jamais tiré). Exemple : `20140910070720-0-VkqQU`. Généré au chargement s'il manque, écrit à la sauvegarde suivante.
- Nouveau compte (`AddNewAccount`, `ObjectAccountList.pas:75-79`) : chaîne `;;;;;1;€;6<Sep1>55957;F;;;` (après saisie dans la fenêtre des comptes, tous les champs sont recopiés depuis l'interface, `Picture` = élément sélectionné, `Aucun` par défaut).

#### 4.7.1 Format `%g` des taux

`Format('%g', [Extended])` Delphi 7 (`SysUtils.pas:6997-7024`, `8807-8889`) : `FloatToText(ffGeneral, Precision = 15, Digits = 3)` :
- 15 chiffres significatifs maximum, zéros finaux supprimés, séparateur décimal seulement si nécessaire ;
- notation fixe si l'exposant décimal (nombre de chiffres avant la virgule) est compris entre −3 et 15 inclus, sinon scientifique `d,dddE-ddd` (au moins **3** chiffres d'exposant, signe `-` seulement si négatif, pas de `+`) ;
- séparateur décimal = `DecimalSeparator` courant (§6.7).
- Équivalent pratique : Python `'%.15g' % x` (mêmes seuils : fixe si −4 ≤ exposant scientifique < 15), en remplaçant `.` par `DecimalSeparator` et `e±XX` par `E[-]XXX`. En Rust : arrondir à 15 chiffres significatifs (`format!("{:.14e}", x)`), puis choisir fixe/scientifique selon l'exposant et supprimer les zéros finaux.
- Exemples : `1` → `1` ; `6.55957` → `6,55957` ; `6.5596` → `6,5596` ; `0.5` → `0,5`.

#### Exemples (`Exemple.bp`)

```
compte0=Test;Test;Test;Courant;1234567890A;1;€;6,5596;F;12345;67890;Test;CIC;FR78 1234 5678 9012 3456 7890 A04 ;real;20140910070720-0-VkqQU
compte4=Mylène Micoton;qui m'a prêté de l'argent;;Rbts prêt;1234567890A;1;€;6,55957;F;12345;67890;;SOCIETE GENERALE;FR78 1234 5678 9012 3456 7890 A04 ;virtual;20140910070720-4-MPEV7
```
(format 16 champs : écrit par une version antérieure à la désactivation de compte ; réécrit par la version actuelle avec un `;` final.)

### 4.8 `whoN` — tiers mémorisés

- Lecture (`ObjectPeople.pas:74-87`) : `i = 0 ; tant que Values['who'+i] <> '' : AddPeople(...)`. Clés contiguës depuis `who0` ; une valeur vide arrête.
- `TPeople.LoadFromString` (`ObjectPeople.pas:104-120`) : `Split` puis accès **direct** à `sl[0]..sl[5]` → **moins de 6 champs = exception = fichier déclaré corrompu** (l'exception remonte dans `loadArchiveFiles`). Champs au-delà du 6e ignorés.

| Idx | Champ | Sens | Lecture | Écriture (`SaveToString`, `:122-125`) |
|---|---|---|---|---|
| 0 | `Caption` | Nom du tiers (clé de recherche/auto-complétion) | brut | `%s` |
| 1 | `Mode` | **Index** dans la liste des modes affichée (`ModeList` : 0..9 standard, 10+ personnalisés) — ce n'est **pas** le code fichier §6.4 | `StrToIntDef(…, 0)` | `%d` |
| 2 | `Category` | Index (identifiant) de catégorie, −1 = aucune | `StrToIntDef(…, -1)` | `%d` |
| 3 | `Who` | Texte « tiers » à insérer dans l'opération | brut | `%s` |
| 4 | `Details` | Texte « détail » à insérer | brut | `%s` |
| 5 | `Value` | Montant par défaut, **chaîne libre telle que saisie** (ex. `0.00`, `500`, vide) | brut | `%s` |

- Ordre conservé tel quel (le tri `SortLines` par `CompareStr` sur `Caption` n'est appliqué que par l'écran des tiers).
- Exemples : `who0=Coin;7;5;Coin;;500`, `who2=Fnac;10;7;Totor;test;` (mode 10 = premier mode personnalisé).

### 4.9 `categ` — catégories

#### Lecture (`ObjectCategs.pas:298-324`)

```
Split(sl, Values['categ'])
pour k, s dans sl :
    p = Pos('=', s)          // 1-based, 0 si absent
    si p > 0 et (p == Length(s) ou Trim(Copy(s, p+1, ∞)) == '') : ignorer      // nom vide
    si p > 0 :
        Index = StrToIntDef(Copy(s, 1, p-1), k)
        nom   = Copy(s, p+1, p+51)          // ATTENTION : longueur p+51, donc nom tronqué à (p+51) caractères
    sinon :
        Index = k ; nom = s                  // format ancien sans identifiant
    AddObject(nom, Index)
UpdateCategs
```

- **Hiérarchie** : un nom commençant par une espace est une **sous-catégorie** du dernier parent qui le précède ; un nom sans espace initiale est un parent (`IsMaster`/`IsChild`, `ObjectCategs.pas:82-86`, `280-283`). L'interface utilise 5 espaces (`IndentLine`, `RenameLine`) mais toute espace initiale suffit. Deux niveaux seulement.
- Si la 1re catégorie commence par une espace, elle est `Trim`-ée (`UpdateCategs`, `:404`, `:415`).
- L'**identifiant** (`Index`) est stable : c'est lui que référencent les opérations (champ 6) et les tiers. L'**ordre** des entrées est l'ordre d'affichage. Nouvel identifiant = max + 1 (`AddItem`, `:439-447`, `GetLastIndex` `:382-392`).
- Libellé complet d'une sous-catégorie : `Parent > Enfant` (`FormattedValues`, `UpdateCategs` `:394-419`).
- `IsInvalidIndex(i)` = `i > LastIndex` ou identifiant absent (`:449-452`).
- Piège : un élément sans `=` et vide (ex. `;;`) provoquerait une erreur d'accès (`Self[Index][1]` sur chaîne vide) dans `IsMaster` — ne se produit pas avec les fichiers écrits par BP.

#### Écriture (`ObjectCategs.pas:326-338`)

`<Index>=<nom brut avec espaces initiales>` joints par `;`, dans l'ordre d'affichage. Liste vide → `categ=`.

Exemple : `categ=0=Alimentation;1=     Epicerie 200;48=     Restaurant déjeuner;2=     Restaurant loisirs;3=Décaissement;…`

Nouveau fichier : catégories par défaut **sans** identifiant (`TXT_DEFAULT_CATEGS1+2`, `UnitRStrings.pas:62-63`) → identifiants = positions 0..n−1 ; ils seront écrits `N=` à la première sauvegarde.

### 4.10 `accounts_order` — ordre d'affichage des comptes

- Lecture (`ObjectAccountsOrder.pas:58-78`) : vide → ordre identité `0..Count-1` ; sinon `StrToInt` de chaque élément (**exception non gérée** si non numérique — elle remonte hors du `try/finally` de `LoadBankPerfectFile`, `unitMain.pas:4109`).
- Chaque valeur = index « réel » (position dans les `compteN`) ; la position dans la liste = position d'affichage (`RealIndex(Position)`, `:80-84`).
- Aucune validation (longueur, doublons) à la lecture.
- Écriture (`:86-97`) : entiers joints par `;`. Exemple : `accounts_order=0;4;2;1;3`.
- Création d'un compte → ajout de `Count` à la fin (`AddAccount`, `:119-122`) ; suppression → retrait et décrément des index supérieurs (`DeleteAccount`, `:34-44`).

### 4.11 `compte_defaut`

- Compte affiché à l'ouverture = **index réel** (`RawAccountsListIndex`) du compte courant au moment de la sauvegarde.
- Lecture : `StrToIntDef(…, 0)` ; si < 0 → 0 (`unitMain.pas:4113`, `4118-4123`). Pas de contrôle de borne supérieure.
- Écriture : `IntToStr(RawAccountsListIndex)`. Exemple : `compte_defaut=0`.

### 4.12 `pass` — mot de passe

Voir §10. Stocké **en clair** dans le texte (seulement obfusqué par le codec V5). `pass=` vide = pas de mot de passe.

### 4.13 `bp_script_data_*` — données persistantes des scripts

- Lecture (`ObjectScriptsData.pas:141-160`) : pour **chaque** ligne de l'en-tête contenant `=` dont le nom commence par `bp_script_data_` (**sensible à la casse**) : `clé = nom[15:]`, `valeur = tout après le 1er '='`. Stockées dans une liste ordonnée de paires (doublons conservés).
- Écriture (`:162-174`) : `bp_script_data_<clé>=<valeur>` dans l'ordre de stockage (ordre de lecture, puis nouvelles clés ajoutées en fin par `SetData`).
- La valeur est stockée **encodée** (`SimpleEncode`, `:37-65`) et n'est décodée qu'à la demande d'un script (`GetData`, `SimpleDecode` `:67-97`) : les données sont donc préservées octet pour octet si aucun script ne les modifie.
- `SimpleEncode` : chaque octet est conservé s'il appartient à ``  !"%&'()*+,-./0-9:;<=>?A-Z[]a-z{}`` ; sinon remplacé par `_` + 2 chiffres hexadécimaux **minuscules** (ex. LF → `_0a`, `\` → `_5c`, `_` → `_5f`, `#` → `_23`, `$` → `_24`, `@` → `_40`, `^` → `_5e`, `` ` `` → `_60`, `|` → `_7c`, `~` → `_7e`, tout octet ≥ 0x7F → `_7f`…`_ff`). Résultat : une seule ligne, sans CR/LF.
- `SimpleDecode` : `_hh` → octet (ne gère correctement que les hexa minuscules).
- La clé passe par `SimpleKey` (`:31-35`) lors de `SetData/GetData` : `SimpleString` (`UnitStrings.pas:664`, minuscules, accents retirés, autres caractères → espace) puis suppression des espaces.
- En pratique la valeur décodée est un **pickle Python protocole 0** produit par les scripts (ex. `(lp1_0a.` = `(lp1\n.` = liste vide). Le contenu relève des scripts (traités ailleurs) ; le cœur doit seulement le conserver.
- Clés observées dans `Exemple.bp` : `echeancier`, `fabiobpscheduler`, `fabiobpbudget`, `fabiobpautofields`, `fabiobpautofilters`, `fabiobptransfer`, `fabiobpbudgetyear`, `fabiobpviewall`, `bpcashcards` ; autres connues : `fabiobpabbreviations`, `fabiobpmailaddress`.
- Aucune ligne n'est écrite s'il n'y a pas de données (`unitMain.pas:2605-2613`).

---

## 5. Rattachement des opérations aux comptes

### 5.1 Règle

Le **k-ième bloc** (k-ième séparateur) contient les opérations du compte **`compte<k-1>`** (ordre « réel » = ordre des clés `compteN`), **indépendamment** de `accounts_order`. En mémoire : `ol_comptes[k]` (liste de `TOperation`).

### 5.2 Écriture

`unitMain.pas:2616-2620` :
```
s = texte_en_tête   (lignes terminées par CRLF)
pour i = 0..RawAccountsList.Count-1 :
    BankLinesToFile(ol_comptes[i], fiches)      // §6
    s = s + FileSeparator + fiches.Text          // fiches.Text = lignes terminées par CRLF, '' si aucune
SecuritySave(s, ...)                             // SaveFileV5 : CRLF → LF puis codec
```
Il y a donc toujours **exactement un séparateur par compte**, même vide.

### 5.3 Cas limites (comportement Delphi)

| Cas | Comportement Delphi | Recommandation Rust |
|---|---|---|
| Dernier compte sans opération | Fichier se termine par le séparateur ; à la relecture le texte final est vide → **aucun bloc ajouté** → `ol_comptes` a un élément de moins que les comptes → incohérence (erreur d'index à la sauvegarde suivante, exception non rattrapée dans `ASaveExecute`). | Lire : créer une liste vide pour chaque `compteN` sans bloc. Écrire : identique à Delphi (séparateur final). |
| Compte intermédiaire vide | Deux séparateurs consécutifs ; lu correctement (bloc vide). | idem |
| Plus de blocs que de comptes | Blocs en trop chargés puis **perdus** à la sauvegarde (boucle sur `RawAccountsList.Count`). | Conserver le comportement (ou avertir). |
| Aucun bloc | Fichier corrompu. | idem |

Un compte nouvellement créé reçoit toujours une opération « Solde initial » (`UnitAccounts.pas:307-350`), mais l'utilisateur peut la supprimer.

### 5.4 Ordre des opérations dans un bloc

Ordre = ordre de la liste en mémoire. Il n'y a pas de tri à l'écriture, **mais** l'affichage trie physiquement la liste du compte courant à chaque rafraîchissement (`AFillGridExecute` → `SortAccount`, `unitMain.pas:3422`, `2286-2296` ; `GetNextChqNumber` trie par date, `:3730`). Les autres comptes ne sont pas retriés. Pour une parité stricte sur les égalités, le tri Delphi est le **QuickSort instable** de `Classes.TList.Sort` (Delphi 7) :

```pascal
procedure QuickSort(SortList: PPointerList; L, R: Integer; SCompare: TListSortCompare);
var I, J: Integer; P, T: Pointer;
begin
  repeat
    I := L; J := R; P := SortList^[(L + R) shr 1];
    repeat
      while SCompare(SortList^[I], P) < 0 do Inc(I);
      while SCompare(SortList^[J], P) > 0 do Dec(J);
      if I <= J then begin T := SortList^[I]; SortList^[I] := SortList^[J]; SortList^[J] := T; Inc(I); Dec(J); end;
    until I > J;
    if L < J then QuickSort(SortList, L, J, SCompare);
    L := I;
  until I >= R;
end;
```

Comparateurs (`unitMain.pas:2221-2284`) : date (`Date` ou `MarkDate` si affichage par date de valeur) → mode (`CompareStr`, ordinal) → tiers (`CompareStr` après retrait d'un préfixe de ventilation `[...]` de ≤ 7 caractères) → montant (`CompareValue` à 0,001 près) → détail (`CompareStr`). Colonnes : 0 date, 1 mode, 2 tiers, 3 catégorie (libellé formaté), 4 montant. Le sens (ascendant/descendant) n'influe que sur l'affichage.

---

## 6. Lignes d'opérations

### 6.1 Structure en mémoire (`TOperation`, `UnitStrings.pas:9-21`)

| Champ | Type | Sens |
|---|---|---|
| `Date` | `TDate` (double, jours) | Date d'opération |
| `Tack` | Integer | Pointage : 0 = non pointé, 1 = pointé (P), 2 = rapproché (R) |
| `Mode` | string | **Libellé** du mode (ex. `Carte`, `Chq 3104512`, `Carte FNAC`) |
| `Who` | string | Tiers |
| `Info` | string | Détail |
| `Categ` | Integer | Identifiant de catégorie (−1 = aucune) |
| `Value` | `Currency` | Montant signé (entier 64 bits × 10⁻⁴) |
| `MarkDate` | `TDate` | Date de valeur |
| `Extra` | string | Champs supplémentaires `k=v;k=v` (§6.10) |

### 6.2 Format d'une ligne

Écriture (`BankLinesToFile`, `unitMain.pas:3983-4021`) :

```
Format('%d;%s;%s;%s;%s;%d;%.<Precision>f;%d;%s',
       [Trunc(Date), Tack, ModeCode, Who, Info, Categ, Value, Trunc(MarkDate), Extra])
```

→ **toujours 9 champs**, même si `Extra` est vide (la ligne se termine alors par `;`).

| # | Champ | Écriture | Lecture (`FileToBankLines`, `unitMain.pas:3934-3980`) |
|---|---|---|---|
| 1 | Date | `Trunc(Date)` en `%d` | `StrToIntDef(l[0], Trunc(Date du jour))` |
| 2 | Pointage | `''` / `P` / `R` | `P` ou `1` → 1 ; `R` → 2 ; autre → 0 |
| 3 | Mode | code numérique ou libellé (§6.4) | §6.4 |
| 4 | Tiers | brut | brut |
| 5 | Détail | brut | brut |
| 6 | Catégorie | `%d` | `StrToIntDef(l[5], -1)` ; si ≥ 0 et `IsInvalidIndex` → −1 |
| 7 | Montant | `%.<Precision>f` (§6.7) | vide → 0 ; sinon `Str2Currency` ; erreur → 0 (silencieux) |
| 8 | Date de valeur | `Trunc(MarkDate)` en `%d` | si ≥ 8 champs : `StrToIntDef(l[7], Trunc(Date))` ; sinon = Date |
| 9 | Champs supplémentaires | `;` → `<\|\|>` | si 9 champs : `<\|\|>` → `;` ; sinon `''` |

**Filtre de validité** : une ligne dont le nombre de champs (après `Split`) est < 7 ou > 9 est **ignorée silencieusement** (et donc perdue à la sauvegarde suivante). Cela couvre les lignes vides et les lignes corrompues par un `;` dans un texte.

Formats acceptés en lecture : 7 champs (anciens fichiers, et ligne initiale d'un fichier neuf §9), 8 champs, 9 champs.

Exemples :
```
39444;;22;hyper u ;;0;-1,78;39444;UId=1
40119;R;42;GF;Participation emprunt maison;21;600,00;40126;UId=18
40205;;Carte FNAC;CHEQUE N;n°3104513;28;-22,00;40205;UId=85
37876;;43;10 France Telecom acheté 30.00;FTE.PA;-1;176,40;37876;UId=96
```

### 6.3 Dates (champs 1 et 8)

- Entier = nombre de jours depuis le **30/12/1899** (`TDateTime` Delphi, partie entière). `39444` = 28/12/2007 ; `40574` = 31/01/2011 ; `2` = 01/01/1900.
- Jamais de partie décimale à l'écriture (`Trunc`). Une valeur non entière (`39444.5`) n'est pas acceptée par `StrToIntDef` → remplacée par la date du jour à la lecture.
- Rust : `NaiveDate::from_ymd(1899,12,30) + Duration::days(n)` (pas de bug « 1900 bissextile » pour n ≥ 61, et Delphi n'en a pas non plus : 60 = 28/02/1900, 61 = 01/03/1900).

### 6.4 Mode de paiement (champ 3)

#### Modes standard (`tabMode`, `unitMain.pas:656` ; libellés `UnitRStrings.pas:19-28`)

| Index liste | Libellé | Code fichier | Signe à la saisie |
|---|---|---|---|
| 0 | `Carte` | 20 | − |
| 1 | `Retrait DAB` | 21 | − |
| 2 | `Chèque émis` | 22 | − |
| 3 | `Prélèvement` | 23 | − |
| 4 | `Virement émis` | 24 | − |
| 5 | `TIP` | 25 | − |
| 6 | `Versement` | 40 | + |
| 7 | `Dépôt de chèque` | 41 | + |
| 8 | `Virement reçu` | 42 | + |
| 9 | `Titre` (action boursière) | 43 | + |
| 10+ | modes personnalisés (`modes=`) | **libellé en clair** | selon préfixe `-` |

#### Écriture (`unitMain.pas:4001-4005`)

```
si Op.Mode commence par 'Chq' (TXT_LITTLE_CHECK, sensible à la casse) : code = 22
sinon : code = IndexOf(Op.Mode dans ModeList, insensible à la casse) + 20
si code > 25 : code += 14
si code > 43 : écrire Op.Mode (libellé) ; sinon écrire code en décimal
```

Conséquences :
- Mode standard → `20`..`25`, `40`..`43`. Mode personnalisé (index ≥ 10) → libellé brut (ex. `Carte FNAC`).
- Libellé **introuvable** (mode personnalisé renommé/supprimé, valeur exotique d'un script) → index −1 → **`19`**.
- Un libellé de mode personnalisé identique (casse ignorée) à un mode standard est encodé avec le code standard.

#### Lecture (`unitMain.pas:3959-3961`, `mode_paiement` `:2016-2059`)

```
m = StrToIntDef(l[2], -1)
si m == -1 : Mode = l[2]                 // libellé en clair
sinon Mode = mode_paiement(m, l[4]) :
    m <= 20          → 'Carte'           // donc 19, 0, négatifs… → Carte
    21..39           → tabMode[m-20]     // 21..29 valides ; 30..39 : lecture hors tableau ($R-) → comportement indéfini
    m >= 40          → k = m-34 ; k <= 9 → tabMode[k] ; k > 9 → ModeList[k] (mode perso par index, format ancien ; hors borne → exception rattrapée → 'Carte')
    si m == 22 : si l[4] (détail) contient '°' suivi d'un nombre (jusqu'à l'espace suivante, validé par Str2Currency) :
                 Mode = 'Chq ' + nombre        // ex. détail 'n°3104512' → mode 'Chq 3104512'
```

Non-idempotences à connaître (reproduites par Delphi) :
- `19` → `Carte` → réécrit `20`. (Observé sur un fichier réel lors du test d'aller-retour.)
- `26`..`29` → `Versement`…`Titre` → réécrits `40`..`43`.
- Libellé purement numérique (ex. mode perso nommé `45`) → interprété comme un code.
- Le numéro de chèque n'est **pas** stocké dans le mode mais dans le détail (`n°1234`) ; seul le code 22 est écrit.

### 6.5 Pointage (champ 2)

`tacks: array[0..2] of string = ('', 'P', 'R')` (`unitMain.pas:3985`). En mémoire, une valeur temporaire 3..5 marque la ligne venant d'être saisie (`unitMain.pas:3894`) et est ramenée à 0..2 au rafraîchissement (`:3443-3447`) ; une valeur ≥ 3 au moment de l'écriture serait hors tableau (ne doit pas arriver).

### 6.6 Catégorie (champ 6)

Identifiant stable (§4.9), `-1` = aucune. Un identifiant inconnu est remplacé par −1 **à la lecture** (et aussi au rafraîchissement : `unitMain.pas:3439`) → perte définitive à la sauvegarde suivante.

### 6.7 Montant (champ 7) — format décimal

- Écriture : `Format('%.' + IntToStr(Precision) + 'f', [Currency])`
  - `Precision` = option `[APPLICATION] Precision` de `bankperfect.ini`, défaut **2**, bornes 0..4 (`unitMain.pas:929`, `unitOptions.dfm:221-229`).
  - **Séparateur décimal = `DecimalSeparator` de Windows** (paramètres régionaux), forcé à `.` s'il n'est ni `.` ni `,` (`unitMain.pas:1228`). Sur un Windows français : `,`. Pas de séparateur de milliers. Négatif : `-` en tête. Ex. `-1,78`, `600,00`, `1700,99`, `0,00`.
  - Arrondi à `Precision` décimales : demi-unité arrondie en s'éloignant de zéro (`FloatToDecimal` Delphi) — voir incertitude U1.
  - ⚠ `Precision = 0` écrit des montants **sans centimes** (perte de données à la relecture). À reproduire uniquement si l'on veut l'iso stricte ; recommandé : garder ce réglage pour l'affichage seulement (question Q1).
  - Paramètre Rust : `DecimalSeparator` (`,` par défaut pour reproduire un Windows FR) et `Precision`.
- Lecture : `Str2Currency` (`UnitStrings.pas:420-458`) :
  1. ne garder que les caractères `-` `.` `,` `0-9` (tout le reste, espaces compris, est supprimé) ;
  2. (si `,` et `.` coexistent, un `FastReplace` devrait retirer le séparateur de milliers mais son résultat est ignoré → **bug** : la conversion échoue) ;
  3. `StrToCurr` avec `,` comme séparateur, sinon avec `.` ; sinon exception → montant 0 (le message `TXT_WRONG_VALUE` est préparé mais jamais affiché, `unitMain.pas:3968-3971`).
  4. `Currency` = 4 décimales (arrondi au plus proche pair pour la 5e décimale, FPU).
  → Un fichier écrit avec `.` (Windows anglais) est relu correctement avec `,` et inversement.

### 6.8 Date de valeur (champ 8)

Voir §6.3. À la saisie, `MarkDate = Date` (`unitMain.pas:3846`) ; modifiable ensuite.

### 6.9 Tiers et détail (champs 4 et 5)

Texte brut, espaces conservées (ex. `hyper u ` avec espace finale). Interdits de fait : `;`, CR, LF, et la chaîne séparateur. Conventions portées par ces textes (pas de champ dédié) :
- Transferts compte à compte : tiers `Transfert vers le compte <AccName>` / `Transfert depuis le compte <AccName>` (`UnitRStrings.pas:17-18`) ; l'opération sœur est retrouvée via les champs supplémentaires `#sisterAccUid`/`#sisterLineUid` puis, à défaut, par nom de compte + date + montant opposé (`GetTransferSister`, `unitMain.pas:5192-5245`).
- Titres (code 43) : tiers `<quantité> <libellé> acheté <prix>` et détail = code valeur (ex. `FTE.PA`) (`unitMain.pas:3871-3889`).
- Ventilation : préfixe `[...]` dans le tiers (`RemoveVentilationPart`, `:2227-2237`).
- Chèques : `n°<numéro>` dans le détail (§6.4).
- Opérations auto (scripts) : préfixe `[auto] ` dans le tiers (convention des scripts).

### 6.10 Champs supplémentaires (champ 9)

Deux niveaux d'échappement :

| Niveau | Contenu | Échappement |
|---|---|---|
| Fichier (champ 9) | `Extra` avec chaque `;` remplacé par `<\|\|>` | `FastReplace(…, ';', '<\|\|>')` à l'écriture, inverse à la lecture (sensible à la casse) |
| Mémoire (`Extra`) | `clé=valeur` séparés par `;` | dans une valeur : `;` → `<fab_pv>`, CRLF/CR/LF → `<fab_cr>` (`SetExtraValue`, `unitMain.pas:6028-6058`) ; décodage `<fab_pv>` → `;`, `<fab_cr>` → CRLF (`GetExtraValue`, `:6074-6100`) |

Exemple fichier : `UId=96` ; avec plusieurs clés : `UId=12<||>#sisterAccUid=20140910070720-1-o13k1<||>#sisterLineUid=95`.

Règles de manipulation (à reproduire pour l'API) :
- `SetExtraValue(op, k, v)` : `Split(Extra)` ; `k = 'UId'` → **refusé** (l'UId n'est pas modifiable par cette voie) ; `k = '__internal_BP_UId'` → traité comme `UId` (usage interne) ; `v = ''` → supprime toutes les entrées commençant par `k=` (sensible à la casse) ; sinon `sl.Values[k] := v_échappé` (remplace la 1re entrée de même nom — nom insensible à la casse — ou ajoute en fin) ; puis `Merge`.
- `GetExtraValue(op, k)` : 1re entrée commençant par `k=` (sensible à la casse) ; `k = ''` → liste des noms séparés par `;`.
- Clés spéciales : `UId` (identifiant de ligne, §7), `#sisterAccUid` (AccUID du compte de l'opération liée), `#sisterLineUid` (UId de l'opération liée), `freeicon` (icône libre), toute clé commençant par `#` est masquée dans le panneau d'affichage (`unitMain.pas:6876-6894`).

### 6.11 Construction d'une ligne neuve (référence)

`ASendOperationExecute` (`unitMain.pas:3756-3900`) : `Extra := 'UId=' + IntToStr(++last_line_id)` pour un ajout ; conservé tel quel pour une modification. Copier/coller (`APasteLinesExecute`, `:2920-2936`) attribue un nouvel UId.

### 6.12 Robustesse — ce que le format ne protège pas

Le format n'échappe ni `;` ni CR/LF dans les champs 3-5. Si un script/plugin y injecte un `;` (le chemin Python n'est pas filtré partout) la ligne a > 9 champs et est **perdue** à la relecture ; un CR/LF la coupe en deux lignes invalides ; une ligne vide fait échouer le déchiffrement (§2.6). Recommandation Rust : à l'écriture, refuser/neutraliser ces caractères dans les champs texte (au minimum `;` → espace, comme l'API plugins), tout en conservant le format.

---

## 7. Identifiants de ligne (UId)

- Stockés dans `Extra` sous `UId=<n>` (n entier ≥ 1, unique dans le fichier, tous comptes confondus).
- Au chargement (`unitMain.pas:4873-4874`, `loadLastLineId` `:4890-4915`) : `last_line_id = max(UId)` ; mais **si une seule ligne a un UId absent, nul ou non numérique**, la fonction renvoie 0 et `addIds` (`:4918-4937`) **renumérote toutes les lignes** de 1 à N dans l'ordre fichier (compte 0 puis 1…, ordre des lignes) : pour chaque ligne `SetExtraValue('__internal_BP_UId', n)` (remplace la valeur de l'entrée `UId` existante sur place, ou ajoute `UId=n` en fin) puis `SetExtraValue('#sisterLineUid', '')` (supprime le lien de transfert, devenu faux).
- `addIds` active « Enregistrer », mais `LoadBankPerfectFile` le désactive juste après (`unitMain.pas:4124`) : la renumérotation n'est écrite que lors d'une sauvegarde ultérieure.
- Cas courant déclenchant la renumérotation : la ligne « Solde initial » d'un compte créé via la fenêtre des comptes n'a **pas** d'UId (`UnitAccounts.pas:328-338`), ni la ligne initiale d'un fichier neuf (§9).

---

## 8. Sauvegarde : algorithme exact

### 8.1 `ASaveExecute` (`unitMain.pas:2561-2634`)

```
si Saving ou CurrentBP == '' : sortir
NN = LastBackup('LastBackupNumber')                   // §8.3
base = Ini[SAVE].PathBackup (défaut PathAppData) + nom_du_fichier_sans_extension
BName = base + '_' + NN + '.bp0'                      // ex. ...\Exemple_14.bp0
CurrentFileCopy = base + '_current.bp0'
FCopy(CurrentBP, BName)                               // copie de l'ANCIENNE version (CopyFile, écrase)
s = en-tête (§4.2) + pour chaque compte : FileSeparator + lignes (§5.2)
SecuritySave(s, CurrentBP, CurrentFileCopy)
```

### 8.2 `SecuritySave` (`unitMain.pas:2526-2559`) — écriture sûre

```
T1 = dossier(CurrentBP) + 'file1_bp.tmp' ; T2 = dossier(CurrentBP) + 'file2_bp.tmp'
répéter jusqu'à 9 fois (pause 200 ms entre essais) :
    SaveFileV5(s, T1) ; FCopy(T1, T2)
    si LoadFileV5(T2) réussit :
        SyncMoveFile(T1, CurrentBP)            // DeleteFile(dest) + MoveFile ; attend l'apparition du fichier
        si LoadFileV5(CurrentBP) échoue :
            DeleteFile(CurrentBP) ; SyncMoveFile(T2, CurrentBP)
            si LoadFileV5(CurrentBP) échoue : essai suivant
        DeleteFile(CurrentFileCopy) ; FCopy(CurrentBP, CurrentFileCopy)     // NOUVELLE version → _current.bp0
        fin (succès)
échec → message « Impossible de sauvegarder le fichier… Essayez Enregistrer sous »
finalement : suppression de T1/T2 s'ils existent
```
Note : `SyncMoveFile` (`:2507-2524`) boucle sans incrémenter son compteur si le fichier n'apparaît pas (boucle infinie potentielle) — ne pas reproduire. Équivalent Rust : écrire un temporaire dans le même dossier, relire/vérifier, `rename` atomique, puis copier vers `_current.bp0`.

### 8.3 Sauvegardes `.bp0`

- `LastBackup(Name)` (`unitMain.pas:726-735`) : lit `[SAVE] <Name>` (défaut 0), incrémente, revient à **1 après 20**, écrit la nouvelle valeur dans l'ini, renvoie `Format('%.2d')` → `01`..`20`.
- Le compteur `LastBackupNumber` est **global** (commun à tous les fichiers), pas par fichier : `Exemple_12.bp0` et `Exemple_14.bp0` peuvent coexister avec des numéros intermédiaires attribués à d'autres fichiers.
- Rotation : 20 emplacements, le plus ancien est écrasé.
- `<nom>_NN.bp0` = état **avant** la sauvegarde n ; `<nom>_current.bp0` = état **après** la dernière sauvegarde réussie (copie exacte du `.bp`).
- Format : identique au `.bp` (copie binaire).
- Dossier : `[SAVE] PathBackup` (défaut `%APPDATA%\BankPerfect\`, ou le dossier de l'exécutable en mode portable — fichier `portable` à côté de l'exe, `unitMain.pas:1189-1199`).
- Sauvegarde distante FTP/e-mail : nom `<nom>_<NN>.bp` avec compteur séparé `[SAVE] LastOnlineBackupNumber` (`UploadFileName`, `unitMain.pas:4284-4288`) — fonctionnalité réseau (voir inventaire 01).

### 8.4 Autres écritures

- `SaveFileTo` / « Enregistrer sous » (`unitMain.pas:2636-2651`) : force l'extension `.bp` puis `ASave`.
- Sauvegarde automatique à la fermeture si `[SAVE] Autosave` (`CloseCurrentFile`, `unitMain.pas:4232-4245`).

---

## 9. Création d'un fichier neuf (`CreateBankPerfectFile`, `unitMain.pas:2676-2690`)

Contenu en clair exact (avant `SaveFileV5`) :

```
utilisateur=<Nom>\r\n
compte0=;;;Courant;;1;€;6.55957;F;;;;;;real\r\n
categ=<TXT_DEFAULT_CATEGS1><TXT_DEFAULT_CATEGS2>TNSLPPBNTSOGPTQBC280673<Trunc(Date)>;R;<23 si solde négatif, sinon 40>;Création du compte courant;;-1;<Solde>
```
- Pas de `guid`, `UserName`, `modes`, `accounts_order`, `compte_defaut`, `pass`, ni UId ; la ligne d'opération a 7 champs et pas de fin de ligne. Tout est complété au chargement (GUID, UID de compte, UId de ligne) puis écrit à la première sauvegarde.
- Le taux est écrit avec un `.` littéral (`6.55957`) quel que soit le séparateur.
- `<Solde>` = texte saisi, séparateur décimal converti en `Sep1`, `0` si annulé/invalide.
- Mode 23 = Prélèvement si le solde commence par `-`, sinon 40 = Versement ; pointage `R`.
- Bug API plugin : `BankPerfectNewFile` appelle `CreateBankPerfectFile(Path, UserName, …)` avec les deux premiers arguments inversés (`UnitPlugins.pas:456`).

---

## 10. Chargement complet et mot de passe

### 10.1 `LoadBankPerfectFile(Path, Password = '', Silent = False)` (`unitMain.pas:4023-4134`)

Résultat : 0 = échec, 1 = succès, 2 = mot de passe refusé en mode silencieux.

```
CurrentBP = Path ; si vide ou inexistant : 0
ext = minuscules(extension) ; si ext ∉ {'.bp', '.bp0'} : message TXT_NOT_VALID, 0
si lecteur 'A:' : copie dans le dossier de l'exe (avec message), on charge la copie        // spécifique Windows/disquette
si extension == '.bp0' (comparaison SENSIBLE à la casse) :
    si Silent : 0
    s = chemin tronqué au DERNIER '_' (FPos avec Index=-1, UnitStrings.pas:403-418) + '.bp'      // dans le dossier du .bp0
    si s existe et l'utilisateur répond « Non » à TXT_LOAD_BACKUP : 0
    FCopy(.bp0 → s) (écrase) ; CurrentBP = s                                                    // restauration
si fichier en lecture seule : message d'avertissement (on continue)
loadArchiveFiles (§1, §3, §4.3-4.9, §5, §6, §7) ; échec → CorruptedFile, 0
mot de passe (§10.2)
UserName (§4.5), ScriptsData (§4.13), comptes (§4.7), accounts_order (§4.10), compte_defaut (§4.11)
ASave.Enabled = False ; chargement du compte, des filtres, titre ; scripts « au chargement »
```

Ordre interne de `loadArchiveFiles` : tiers → `utilisateur` → `guid` → catégories → modes → blocs d'opérations (les catégories doivent être chargées avant les opérations, car la validation du champ 6 en dépend ; les modes aussi, pour `ModeList`) → UId.

### 10.2 Mot de passe (`unitMain.pas:4083-4097`, `ASetPasswordExecute` `:4642-4671`)

- Stockage : clé `pass` **en clair** (après déchiffrement codec).
- Vérification : `s = minuscules(Values['pass'])` ; boucle tant que `s <> minuscules(saisie)` **et** `minuscules(saisie) <> 'ch280673'` → redemande (`EnterText`, masque `MPass`) ; Annuler → échec. Donc :
  - comparaison **insensible à la casse** ;
  - un fichier sans mot de passe (`pass=` vide) s'ouvre avec la saisie vide initiale sans rien demander ;
  - **mot de passe maître** `ch280673` accepté pour tout fichier (porte dérobée de l'auteur) — question Q2 : le conserver ?
- Après ouverture : `EPassword := s` (la version **minuscule** stockée) → réécrit en minuscules à la prochaine sauvegarde.
- Définition/modification : saisie + confirmation ; la valeur est conservée **telle que saisie** (casse comprise) jusqu'au prochain rechargement ; saisie vide = suppression.
- Ce n'est pas un chiffrement : quiconque connaît le codec lit les données et le mot de passe.

---

## 11. Formats anciens (lecture seule)

Historique (`history.txt`) : 3.5 (juillet 2003) « clé codée sur 8192 bits » = Shuffle/ISAAC ; 5.0 (septembre 2005) « format plus compact » = UCL ; 6.0 (novembre 2006) « nouveau format… cryptage plus efficace », non lisible par les versions antérieures = V5. Les versions < 3.5 ne sont plus lues (`history.txt:606-609`). Aucun fichier ancien n'est présent dans le dépôt : ces chemins n'ont pas pu être testés sur données réelles.

Le texte en clair obtenu a la même structure (en-tête, séparateur, lignes) ; les vieux fichiers ont typiquement des catégories sans identifiant, des lignes à 7 champs, pas de GUID/UID : tout est géré par les valeurs par défaut décrites plus haut.

### 11.1 UCL / NRV2E « BPXENCODE » (`ucl.pas`, `ucl/DIUcl.pas`)

Écriture historique (`UPXSaveFile`, `ucl.pas:16-27`, plus appelée) : `UclCompressStrA('BPXENCODE' + texte, niveau 6)`.

Format du fichier (`UclCompressStrA`/`UclDecompressStrA`, `DIUcl.pas:59-123`) :

```
octets 0..3 : u = taille décompressée (uint32 little-endian)
    u == 0  → données stockées non compressées : texte = octets[4..]
    u != 0  → octets[4..] = flux NRV2E (ucl_nrv2e_decompress_asm_safe_8), sortie de u octets
Si fichier < 4 octets ou erreur de décompression → échec.
Le texte doit commencer par 'BPXENCODE' (9 octets), retirés.
```

Décompresseur NRV2E (port de `n2e_d.c` d'UCL 1.0x, variante 8 bits) :

```
bb = 0 ; ilen = 0 ; olen = 0 ; last_m_off = 1
getbit() : si (bb & 0x7f) != 0 : bb = bb*2  sinon : bb = src[ilen++]*2 + 1  ; renvoyer (bb >> 8) & 1
boucle :
    tant que getbit() : dst[olen++] = src[ilen++]                    // littéraux
    m_off = 1
    boucle :
        m_off = m_off*2 + getbit()
        si getbit() : sortir
        m_off = (m_off-1)*2 + getbit()
    si m_off == 2 :
        m_off = last_m_off ; m_len = getbit()
    sinon :
        m_off = (m_off-3)*256 + src[ilen++]          // arithmétique uint32
        si m_off == 0xFFFFFFFF : FIN
        m_len = (m_off ^ 0xFFFFFFFF) & 1
        m_off >>= 1
        m_off += 1 ; last_m_off = m_off
    si m_len : m_len = 1 + getbit()
    sinon si getbit() : m_len = 3 + getbit()
    sinon :
        m_len += 1
        répéter : m_len = m_len*2 + getbit()  jusqu'à getbit() == 1
        m_len += 3
    si m_off > 0x500 : m_len += 1
    copier m_len + 1 octets depuis dst[olen - m_off] (copie octet par octet, recouvrement autorisé)
fin : succès si ilen == longueur(src) et olen == u
```
(Le flux compressé suit directement l'en-tête de 4 octets ; la variante « safe » contrôle les débordements.) Recommandation : porter `n2e_d.c` ou utiliser une crate UCL existante, et valider sur un fichier de test produit par UCL (incertitude U3).

Note : `UPXLoadFile` sur un fichier V5 interprète les 4 premiers octets comme une taille énorme ; l'allocation ou la décompression échoue → `False` (exception rattrapée). Les loaders étant essayés dans l'ordre, cela n'a pas d'effet sur les fichiers V5.

### 11.2 « Shuffle » ISAAC (`ObjectShuffle.pas`, `ObjectIsaac.pas`)

Lecture (`ShuffleLoad`, `ObjectShuffle.pas:142-159`) :

```
t = octets du fichier
remplacer 0x01 → CR et 0x02 → LF                                   // ToStr, :104-113
H[0..255] = histogramme de t en IGNORANT CR, LF, '0'..'9', '.'     // CalcHistogram, :22-33
HT = CRLF + str(H[0]) + '.' + str(H[1]) + … + '.' + str(H[255])    // GetHistogram, :35-42 (décimal)
# UnShuffle (:67-92)
rng = ISAAC initialisé avec la graine H (256 × uint32)
l = len(t) ; pour j = l..2 (décroissant) : R[j] = (rng.val() mod j) + 1
pour i = 2..l (croissant) : échanger t[R[i]] et t[i]                // indices 1-based
r = t sans ses len(HT) derniers octets
si len(r) >= 13 et r se termine par 'FABIO_SHUFFLE' : texte = r sans ces 13 octets ; succès
sinon : échec
```

Écriture historique (non présente dans les sources, reconstruite par inversion ; utile pour générer des fichiers de test) :
```
T = texte + 'FABIO_SHUFFLE' ; H = histogramme(T) (mêmes exclusions)
F = T + CRLF + GetHistogram(H)
rng = ISAAC(H) ; pour i = len(F)..2 : n = (rng.val() mod i) + 1 ; échanger F[n] et F[i]      // Shuffle, :44-65
remplacer CR → 0x01, LF → 0x02 ; écrire
```
(L'histogramme est invariant par permutation et les caractères exclus couvrent ceux de `HT`, d'où la symétrie.)

ISAAC (`ObjectIsaac.pas`) = ISAAC 32 bits standard de Bob Jenkins, arithmétique modulo 2³² :
- `Create(seed)` → `reSeed` : `mem[] = 0`, `rsl[i] = seed[i]` (0 au-delà), `Init(true)` (`:154-167`).
- `Init(flag)` (`:172-238`) : `aa=bb=cc=0` ; `a..h = 0x9e3779b9` ; 4 tours de `mix` ; pour i par pas de 8 : (si flag : `a..h += rsl[i..i+7]`) `mix` ; `mem[i..i+7] = a..h` ; si flag, seconde passe identique avec `mem` ; puis `Isaac()` et `count = 0`.
- `mix` : `a^=b<<11; d+=a; b+=c; b^=c>>2; e+=b; c+=d; c^=d<<8; f+=c; d+=e; d^=e>>16; g+=d; e+=f; e^=f<<10; h+=e; f+=g; f^=g>>4; a+=f; g+=h; g^=h<<8; b+=g; h+=a; h^=a>>9; c+=h; a+=b`.
- `Isaac()` (`:243-266`) : `cc++ ; bb += cc` ; pour i = 0..255 : `x = mem[i]` ; `aa ^= aa<<13 | aa>>6 | aa<<2 | aa>>16` selon `i & 3` = 0,1,2,3 ; `aa += mem[(i+128) & 255]` ; `y = mem[(x>>2) & 255] + aa + bb` ; `mem[i] = y` ; `bb = mem[(y>>10) & 255] + x` ; `rsl[i] = bb`.
- `val()` (`:270-279`) : renvoie `rsl[count++]` ; après 256 valeurs, `Isaac()` et `count = 0`.
- Vérifié : un portage Python de ce code, initialisé à zéro + un `Isaac()` supplémentaire, redonne le vecteur de test officiel (`f650e4c8 e448e96d 98db2fb4 f5fad54f…`) ; l'aller-retour Shuffle/UnShuffle a été validé sur un texte synthétique.

---

## 12. Outils cachés liés au format

- **Déchiffrer un .bp en texte** : Ctrl+Maj+clic sur le libellé de solde `solde01` alors que la date de saisie vaut 28/06/1973 → choix d'un fichier, essai des 3 loaders, écriture `<fichier>.txt` (texte en clair sans marqueur) (`unitMain.pas:2153-2169`).
- **Chiffrer un texte en .bp** : même geste sur `solde03` → lit un `.txt` (`TStringList.LoadFromFile`, donc fins de ligne normalisées CRLF) et écrit `<fichier>.txt.bp` via `SaveFileV5` (`unitMain.pas:4564-4580`).
Utile pour la parité « outil de diagnostic » (inventaire 01) et pour les tests.

---

## 13. Algorithme de réécriture octet pour octet (récapitulatif pour Rust)

```
fn save(model) -> Vec<u8>:
    let mut s = String (CP1252 lossless)
    push_line("utilisateur=" + nom_groupe)
    push_line("guid=" + guid)                         // généré au chargement si absent
    push_line("UserName=" + user_name)
    push_line("modes=" + join(modes_avec_prefixe, ";"))
    for (i, a) in comptes: push_line(format!("compte{i}=") + join([a.bank, a.agency, a.address, a.name, a.number,
            fmt_g(a.rate1), a.curr1, fmt_g(a.rate2), a.curr2, a.bank_code, a.agency_code, a.bic, a.picture,
            a.iban, if a.virtual {"virtual"} else {"real"}, a.uid, if a.disabled {"disabled"} else {""}], ";"))
    push_line("categ=" + join(categs.map(|c| format!("{}={}", c.id, c.raw_name)), ";"))
    push_line("accounts_order=" + join(order, ";"))
    for (i, w) in tiers: push_line(format!("who{i}={};{};{};{};{};{}", w.caption, w.mode_index, w.categ, w.who, w.details, w.value))
    push_line("compte_defaut=" + current_real_index)
    push_line("pass=" + password)
    for (k, v) in script_data: push_line("bp_script_data_" + k + "=" + v)     // v déjà encodé SimpleEncode
    for ops in ol_comptes[0..comptes.len()]:
        s += "TNSLPPBNTSOGPTQBC280673"
        for o in ops: push_line(format!("{};{};{};{};{};{};{};{};{}", o.date, ["","P","R"][o.tack],
                 mode_code(o.mode), o.who, o.info, o.categ, fmt_amount(o.value, precision, dec_sep), o.markdate,
                 o.extra.replace(";", "<||>")))
    // push_line ajoute "\n" (équivalent du CRLF → LF de SaveFileV5)
    let bytes = b"BP-FILE-5" + cp1252(s)
    encrypt_codec5(bytes)           // §2.3, L = bytes.len()
```

Pour une parité totale lors d'un « ouvrir puis enregistrer » sans modification, appliquer à la lecture les mêmes normalisations que Delphi : suppression des modes vides/doublons, troncature des noms de catégories (§4.9), catégories invalides → −1, lignes invalides supprimées, mode 19/26-29 normalisés, UId renumérotés si incomplets, GUID/UID de compte générés si absents, mot de passe en minuscules, champ « disabled » ajouté, montants reformatés avec `Precision`, taux reformatés en `%g`, clés d'en-tête inconnues supprimées.

---

## 14. Validation effectuée (aller-retour)

Un parseur + sérialiseur Python implémentant exactement les règles ci-dessus (codec V5, en-tête, comptes, catégories, tiers, modes, opérations, champs supplémentaires, `%g`, `%.2f` avec `,`) a été exécuté sur les 9 fichiers `.bp`/`.bp0` du dépôt :

| Fichier | Résultat |
|---|---|
| `Exemple.bp` | Identique octet pour octet **après normalisation CRLF→LF du fichier source** (le fichier du dépôt a subi une conversion en CRLF ; BP écrit en LF), en mode « 16 champs de compte » (fichier antérieur au champ `disabled`). En mode version actuelle : seules les 5 lignes `compteN` diffèrent (ajout du `;` final), comme attendu. |
| `Exemple_12.bp0` | Identique (16 champs). |
| `Exemple_14.bp0`, `Exemple_current.bp0` | Identiques (17 champs, version actuelle). |
| `CIC_current.bp0` | Identique (17 champs). |
| `CIC_16.bp0` | Identique en mode 16 champs ; ce fichier contient des lignes sans UId → Delphi les renumérote à l'ouverture (émulé : 160 lignes renumérotées, conforme au §7). |
| 3 fichiers personnels (`.bp`, `_12.bp0`, `_current.bp0`) | Identiques sauf **une** ligne d'opération dont le mode `19` est réécrit `20` — comportement Delphi attendu (§6.4). |

Aucune donnée personnelle n'est reproduite ici. Les scripts de test se trouvent dans le scratchpad de la session (`bpcodec.py`, `roundtrip.py`, `uidtest.py`, `shuffle.py`).

---

## 15. Incertitudes et questions ouvertes

**Incertitudes techniques**
- **U1** — Arrondi de `Format('%.Nf', [Currency])` quand le montant a plus de `Precision` décimales (cas rare : montants calculés par scripts, `Precision` < 4) : supposé « demi en s'éloignant de zéro » (`FloatToDecimal` Delphi 7, non vérifié sur binaire). Idem arrondi de `StrToCurr` à 4 décimales (supposé au plus proche pair).
- **U2** — `mode_paiement` pour les codes 30..39 lit hors du tableau `tabMode` (compilé en `$R-`) : résultat indéfini dans Delphi. Proposition : `Carte`.
- **U3** — Décompresseur NRV2E et format Shuffle non testés sur de vrais fichiers anciens (aucun dans le dépôt). Le code ISAAC est validé par vecteur de test officiel.
- **U4** — `DecimalSeparator` peut changer en cours de session si l'utilisateur modifie les paramètres régionaux Windows (mise à jour automatique VCL) ; non géré explicitement par BP.
- **U5** — La page de code est celle du Windows de l'utilisateur ; supposée CP1252 (aucun fichier non-CP1252 observé).

**Questions ouvertes (décisions produit)**
- **Q1** — Reproduire l'écriture des montants avec `Precision` (0..4), au risque de perdre des décimales, ou toujours écrire avec 2 décimales (lisible par Delphi dans tous les cas) ?
- **Q2** — Conserver le mot de passe maître `ch280673` ?
- **Q3** — `UserName` « hérité » du fichier précédemment ouvert quand la clé est vide (§4.5) : reproduire ou corriger ?
- **Q4** — Corriger les bugs de lecture (dernier compte vide perdu §5.3, `Str2Currency` avec `,` et `.`, noms de catégories tronqués) ou les reproduire à l'identique ? Proposition : lire de façon tolérante, écrire exactement comme Delphi.
- **Q5** — Conserver les clés d'en-tête inconnues à la sauvegarde (Delphi les supprime) ? Proposition : les supprimer pour l'iso, éventuellement option.
- **Q6** — Écriture des formats anciens (UCL, Shuffle) : jamais nécessaire (Delphi ne les écrit plus) — lecture seule confirmée ?
