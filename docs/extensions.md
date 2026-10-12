# Extensions BankPerfect

Les extensions ajoutent à BankPerfect des commandes, des fenêtres et des automatismes. Elles s'écrivent en [Luau](https://luau.org), un Lua moderne, rapide et typé (celui de Roblox), et s'affichent avec les composants de l'application : une fenêtre d'extension a exactement l'allure d'une fenêtre de BankPerfect, dans tous les thèmes.

![Une fenêtre d'extension](captures/extensions-doublons.png)

- [Premiers pas](#premiers-pas)
- [Installer, activer, recharger](#installer-activer-recharger)
- [Structure d'une extension](#structure-dune-extension)
- [Comptes et opérations](#comptes-et-opérations)
- [Dates et montants](#dates-et-montants)
- [Catégories, modes, tiers](#catégories-modes-tiers)
- [Boîtes de dialogue](#boîtes-de-dialogue)
- [Fenêtres](#fenêtres)
- [Éléments de fenêtre](#éléments-de-fenêtre)
- [Évènements](#évènements)
- [Stockage](#stockage)
- [Fichiers, Internet, JSON](#fichiers-internet-json)
- [Annulation, erreurs et limites](#annulation-erreurs-et-limites)
- [Sécurité](#sécurité)
- [Référence rapide](#référence-rapide)

## Premiers pas

Créez un fichier `bonjour.luau` dans le dossier des extensions (menu **Extensions › Ouvrir le dossier des extensions**) :

```lua
bp.extension({
	name = "Bonjour",
	description = "Ma première extension.",
	version = "1.0",
})

bp.command({
	title = "Dire bonjour…",
	icon = "hand-waving",
	shortcut = "Ctrl+Alt+B",
	run = function()
		local compte = bp.current_account()
		bp.ui.alert(`Le solde de « {compte.name} » est de {bp.money(compte.balance)}.`)
	end,
})
```

Dans **Extensions › Gérer les extensions…**, cliquez sur **Recharger** : la commande apparaît dans le menu **Extensions**, dans la palette de commandes (Ctrl+K) et répond à son raccourci.

Pour découvrir ce qu'il est possible de faire, cliquez sur **Exemples** dans la même fenêtre : quatre extensions commentées sont copiées dans votre dossier.

| Exemple | Ce qu'il montre |
| --- | --- |
| `doublons.luau` | fenêtre avec critères, tableau à sélection multiple, confirmation, suppression |
| `synthese-du-mois.luau` | statistiques, barres, colonnes, navigation de mois en mois, retour à la grille |
| `arrondis.luau` | réaction aux nouvelles opérations, transferts, formulaire de réglages, `bp.store` |
| `export-json.luau` | formulaire avec validation, `bp.json`, enregistrement d'un fichier |

## Installer, activer, recharger

Une extension est :

- soit un fichier `nom.luau` ;
- soit un dossier `nom/` contenant `init.luau`, et éventuellement d'autres modules chargés par `require("module")` (fichier `module.luau` du même dossier).

Le dossier des extensions est celui de la configuration de BankPerfect : `~/.config/bankperfect/extensions` sous Linux, `~/Library/Application Support/net.chelly.bankperfect/extensions` sous macOS, `%APPDATA%\chelly\bankperfect\config\extensions` sous Windows.

![Gestion des extensions](captures/extensions-gestion.png)

La fenêtre **Gérer les extensions** liste les extensions trouvées, leurs commandes et raccourcis, les évènements qu'elles suivent et les autorisations qu'elles demandent. Une case **Activée** désactive une extension sans la supprimer. Une erreur au chargement y est affichée, avec la ligne fautive ; le **journal** montre les dernières lignes écrites par `print`.

Après une modification du code, cliquez sur **Recharger**.

## Structure d'une extension

### `bp.extension{ … }`

Décrit l'extension (facultatif, mais recommandé).

| Champ | Rôle |
| --- | --- |
| `name` | nom affiché (par défaut : nom du fichier) |
| `description` | une phrase, affichée dans la fenêtre de gestion |
| `version`, `author` | informations affichées |
| `permissions` | autorisations : `{ "network" }` pour accéder à Internet |

### `bp.command{ … }`

Ajoute une commande au menu **Extensions** et à la palette. À déclarer au chargement du fichier.

| Champ | Rôle |
| --- | --- |
| `title` | libellé (obligatoire) ; terminez-le par « … » s'il ouvre une fenêtre |
| `run` | fonction appelée (obligatoire) |
| `icon` | nom d'une icône [Phosphor](https://phosphoricons.com) : `"copy"`, `"piggy-bank"`, `"chart-pie-slice"`… |
| `shortcut` | raccourci : `"Ctrl+Alt+D"`, `"Ctrl+Maj+F9"`, `"F9"`. Ctrl correspond à Cmd sous macOS. Un raccourci sans Ctrl ni Alt n'est accepté que pour une touche de fonction ; un raccourci déjà pris par BankPerfect est ignoré |
| `needs` | `"file"` (par défaut : un fichier doit être ouvert), `"selection"` (des lignes doivent être sélectionnées) ou `"nothing"` (marche sans fichier) |
| `description` | aide affichée dans la fenêtre de gestion |
| `id` | identifiant stable (par défaut : le titre) |

### `bp.on(évènement, fonction)`

Fait réagir l'extension à ce qui se passe dans BankPerfect. Voir [Évènements](#évènements).

### Modules

Dans une extension en dossier, `require("outils")` charge `outils.luau` (une seule fois) et renvoie ce que le module renvoie :

```lua
-- outils.luau
local M = {}
function M.mensuel(montant) return montant / 12 end
return M
```

```lua
-- init.luau
local outils = require("outils")
```

## Comptes et opérations

### Accès

| Fonction | Renvoie |
| --- | --- |
| `bp.accounts()` | les comptes, dans l'ordre d'affichage (`bp.accounts{ all = true }` inclut les comptes désactivés) |
| `bp.account(nom)` | le compte de ce nom (casse ignorée), ou `nil` |
| `bp.current_account()` | le compte affiché |
| `bp.selection()` | les opérations sélectionnées et affichées, dans l'ordre de la grille |
| `bp.visible()` | les opérations affichées (filtres compris), dans l'ordre de la grille |
| `bp.find(uid)` | l'opération de cet identifiant, tous comptes confondus |
| `bp.file()` | `{ name, path, user }` du fichier ouvert, ou `nil` |

### Compte

| Champ | | |
| --- | --- | --- |
| `name`, `bank`, `agency`, `number`, `iban`, `bic`, `currency` | texte | modifiables |
| `balance` | nombre | solde, lecture seule |
| `count` | nombre | nombre d'opérations |
| `disabled`, `virtual`, `current` | booléens | lecture seule |
| `uid` | texte | identifiant du compte |

| Méthode | |
| --- | --- |
| `compte:operations(filtre?)` | opérations, triées par date |
| `compte:add{ … }` | ajoute une opération et la renvoie |
| `compte:find(uid)` | opération de cet identifiant |
| `compte:balance_at(date)` | solde à une date (incluse) |
| `compte:show()` | affiche ce compte dans la grille |

Le filtre de `operations` accepte : `from`, `to` (dates incluses), `payee` (texte contenu dans le tiers), `text` (dans le tiers ou le détail), `category` (une catégorie mère inclut ses sous-catégories ; `false` = sans catégorie), `mode`, `tack`, `min`, `max` (montants signés). La recherche de texte ignore la casse et les accents.

```lua
local courses = compte:operations({ category = "Alimentation", from = bp.today():start_of_month() })
```

`bp.add_account{ name = "Livret A", initial = 100, currency = "€" }` crée un compte et le renvoie.

### Opération

| Champ | | |
| --- | --- | --- |
| `date` | date | modifiable (la date de pointage suit tant que la ligne n'est pas pointée) |
| `amount` | nombre | signé : négatif pour une dépense |
| `payee`, `details` | texte | tiers et détail |
| `mode` | texte | `"Carte"`, `"Prélèvement"`, `"Virement reçu"`… ou un mode personnalisé |
| `category` | texte ou `nil` | libellé complet, `"Habitation > EDF"` ; s'affecte par libellé, par nom s'il est unique, par identifiant ou par table catégorie |
| `category_id` | nombre ou `nil` | lecture seule |
| `tack` | texte | `"none"`, `"pointed"` ou `"reconciled"` ; un changement met la date de pointage au jour |
| `mark_date` | date | date de pointage |
| `uid` | nombre | identifiant, lecture seule |
| `account` | compte | lecture seule (voir `move_to`) |
| `cheque` | texte ou `nil` | numéro de chèque |
| `is_transfer` | booléen | ligne de transfert entre comptes |

| Méthode | |
| --- | --- |
| `op:field(nom)` / `op:set_field(nom, valeur)` | champs supplémentaires (F3) ; `nil` ou `""` supprime |
| `op:fields()` | table de tous les champs supplémentaires |
| `op:delete()` | supprime l'opération |
| `op:move_to(compte)` | déplace l'opération (compte ou nom de compte) |
| `op:copy_to(compte)` | copie l'opération et renvoie la copie |
| `op:sister()` | l'autre ligne d'un transfert, ou `nil` |

Deux références à la même opération sont égales (`==`) ; `tostring(op)` donne « date tiers montant ».

### Ajouter

```lua
local op = bp.current_account():add({
	date = "05/03/2026",        -- ou une date ; par défaut aujourd'hui
	payee = "Boulangerie",
	details = "Pain",
	amount = -4.20,              -- obligatoire ; "4,20" accepté
	mode = "Carte",              -- par défaut : Carte (dépense) ou Virement reçu (recette)
	category = "Alimentation > Epicerie",
	tack = "pointed",            -- par défaut "none"
	fields = { ticket = 1234 },  -- champs supplémentaires
})
```

`bp.transfer{ from, to, amount, date?, details?, category? }` crée un transfert lié entre deux comptes et renvoie ses deux lignes.

`bp.select(opérations)` affiche le compte de la première opération et sélectionne celles de ce compte dans la grille, à la fin de la commande.

## Dates et montants

Une date est un objet :

```lua
local d = bp.date(2026, 1, 31)       -- ou bp.date("31/01/2026"), bp.date("2026-01-31")
local aujourd_hui = bp.today()

d.day, d.month, d.year               -- 31, 1, 2026
d.weekday, d.weekday_name            -- 6, "samedi" (1 = lundi)
d.month_name                         -- "janvier"
d + 30, d - 7                        -- dates
bp.date(2026, 3, 1) - d              -- 29 (jours)
d < aujourd_hui                      -- comparaisons
d:add_months(1)                      -- 28/02/2026 (reste dans le mois)
d:add_years(1), d:add_days(10)
d:start_of_month(), d:end_of_month(), d:start_of_year(), d:end_of_year()
d:format("jj mois aaaa")             -- "31 janvier 2026" ; jj, mm, aa, aaaa, mois
d:iso()                              -- "2026-01-31"
tostring(d)                          -- "31/01/2026"
```

Partout où une date est attendue, un texte `jj/mm/aaaa` convient aussi (`jj/mm` et `jj` complètent avec l'année et le mois en cours).

Les montants sont des nombres (euros, ou devise du compte), gardés au dix-millième près. Utiles :

- `bp.money(-1234.5)` → `-1 234,50 €` (`bp.money(x, "$")` pour une autre devise) ;
- `bp.round(x, 2)` arrondit ;
- `bp.parse_amount("1 234,56 €")` → `1234.56` (ou `nil`).

## Catégories, modes, tiers

| Fonction | |
| --- | --- |
| `bp.categories()` | liste de `{ id, name, label, is_child, parent }` dans l'ordre d'affichage |
| `bp.category(libellé ou id)` | une catégorie, ou `nil` |
| `bp.add_category(nom, mère?)` | crée la catégorie (ou renvoie celle qui existe déjà) |
| `bp.rename_category(catégorie, nom)` | renomme |
| `bp.delete_category(catégorie)` | supprime (avec ses sous-catégories) ; renvoie le nombre d'opérations qui n'ont plus de catégorie |
| `bp.modes()` | liste de `{ name, debit, custom }` |
| `bp.add_mode(nom, dépense?)` | crée un mode personnalisé (dépense par défaut) |
| `bp.payees()` | tiers mémorisés : `{ caption, payee, details, mode, category, amount }` |

## Boîtes de dialogue

Les boîtes de dialogue attendent la réponse de l'utilisateur, comme une fonction ordinaire : le code reprend à la ligne suivante quand il a répondu.

```lua
bp.ui.alert("Import terminé.", { title = "Import" })

if bp.ui.confirm("Supprimer 3 lignes ?", { ok = "Supprimer", danger = true }) then
	-- …
end

local nom = bp.ui.prompt("Nom du compte", { default = "Épargne", placeholder = "Nom" })
if nom then
	-- nil si l'utilisateur annule
end

bp.ui.toast("Terminé")  -- message discret en bas de la fenêtre, sans attendre
```

### Formulaire

`bp.ui.form` affiche une fenêtre de saisie et renvoie les valeurs, ou `nil` si l'utilisateur annule :

![Formulaire](captures/extensions-formulaire.png)

```lua
local v = bp.ui.form({
	title = "Exporter en JSON",
	icon = "brackets-curly",
	section = "Opérations à exporter",   -- titre de la carte (facultatif)
	ok = "Exporter…",
	bp.ui.account({ "compte", label = "Compte", value = bp.current_account() }),
	bp.ui.date({ "du", label = "Du", value = bp.today():start_of_year() }),
	bp.ui.date({ "au", label = "Au", value = bp.today() }),
	bp.ui.checkbox({ "lisible", label = "Mise en forme lisible", value = true }),
	validate = function(v)
		if v.au < v.du then return "Période invalide." end
		return nil
	end,
})
if v then
	print(v.compte.name, v.du, v.au, v.lisible)
end
```

`validate` reçoit les valeurs et renvoie un message pour refuser (affiché dans la fenêtre), ou `nil`.

## Fenêtres

Pour une vraie fenêtre (aperçu en direct, tableaux, boutons qui agissent sans fermer…), utilisez `bp.ui.window`. La fenêtre garde un **état** (`state`) ; sa fonction **`view(state)`** renvoie la liste des éléments à afficher. Les champs sont liés à une clé de l'état : quand l'utilisateur tape ou choisit, l'état est mis à jour et `view` est rappelée. Les gestionnaires (`on_click`…) modifient l'état ou le fichier, et la fenêtre se redessine.

```lua
local ui = bp.ui

local resultat = ui.window({
	title = "Grosses dépenses",
	icon = "receipt",
	size = "medium",                -- "small", "medium", "large" ou une fraction de l'écran (0.6)
	subtitle = "Opérations du compte affiché au-delà d'un seuil.",
	state = { seuil = 100, choix = {} },
	view = function(s)
		local lignes = {}
		for _, op in bp.current_account():operations({ max = -(s.seuil or 0) }) do
			table.insert(lignes, { op.date, op.payee, ui.money(op.amount), id = op })
		end
		return {
			ui.section({
				title = "Critère",
				icon = "sliders-horizontal",
				ui.amount({ "seuil", label = "Seuil" }),
			}),
			ui.table({
				columns = { { "Date", width = 100 }, "Tiers", { "Montant", align = "right" } },
				rows = lignes,
				select = "choix",
				multiple = true,
			}),
			#s.choix > 0 and ui.text(#s.choix .. " ligne(s) choisie(s)"),
		}
	end,
	buttons = function(s)
		return {
			ui.button({ "Annuler", cancel = true }),
			ui.button({ "Pointer", icon = "check", on_click = function(st)
				for _, op in st.choix do
					op.tack = "pointed"
				end
			end }),
			ui.button({ "Valider", primary = true, close = true }),
		}
	end,
})
-- resultat : l'état (bouton close), ou nil (Annuler, Échap)
```

Règles utiles :

- un élément `nil` ou `false` dans la liste est ignoré : `condition and ui.text("…")` affiche un élément selon l'état ;
- `buttons` est une liste ou une fonction de l'état ; les boutons sont alignés à droite, le dernier tout à droite ; sans boutons, la fenêtre a un bouton **Fermer** ;
- un gestionnaire reçoit `(state, win)` ; `win:close(valeur)` ferme la fenêtre, que `bp.ui.window` renvoie alors ;
- un bouton `close = true` ferme et renvoie l'état, après son `on_click` éventuel (qui peut renvoyer `false` pour empêcher la fermeture, ou une autre valeur à renvoyer) ; `cancel = true` ferme et renvoie `nil` ; Échap aussi ;
- `on_change = function(state, clé, win)` est appelée après chaque changement d'un champ ;
- un gestionnaire peut ouvrir une boîte de dialogue ou une autre fenêtre ;
- `view` doit seulement décrire la fenêtre : elle ne peut pas ouvrir de boîte de dialogue.

## Éléments de fenêtre

Tous les éléments sont construits par `bp.ui` (ici `ui`). Les conteneurs reçoivent leurs enfants à la suite de leurs options.

### Mise en page

| Élément | |
| --- | --- |
| `ui.section{ title, icon, … }` | carte avec un titre en petites capitales (sans `title` : carte simple) |
| `ui.row{ …, align = "right" }` | éléments côte à côte, largeur partagée |
| `ui.columns{ … }` | colonnes de même largeur (souvent des sections) |
| `ui.separator()`, `ui.space(px)` | séparation, espace |

### Texte et valeurs

| Élément | |
| --- | --- |
| `ui.text(texte, { style, color, align })` | `style` : `"muted"`, `"strong"`, `"heading"`, `"small"`, `"mono"` |
| `ui.heading(texte)`, `ui.muted(texte)`, `ui.strong(texte)` | raccourcis |
| `ui.money(montant, { currency, colored })` | montant formaté, rouge ou vert selon le signe |
| `ui.stat{ label, value, icon, color, hint }` | chiffre clé dans une carte ; `value` peut être un `ui.money` |
| `ui.pill(texte, couleur)` | pastille |
| `ui.notice(texte, { color, icon })` | encadré d'information ou d'avertissement |
| `ui.progress(fraction, { text, color })` | barre de progression (0 à 1) |
| `ui.bars{ data = { { label, value, text?, color? }, … }, currency }` | barres horizontales proportionnelles |

Couleurs : `"accent"`, `"positive"`, `"negative"`, `"warning"`, `"muted"` (elles suivent le thème) ou `"#RRGGBB"`.

### Champs

Le premier élément est la clé de l'état ; `label` place un libellé à gauche, `hint` une aide dessous, `placeholder` un texte d'invite ; `disabled = true` grise le champ ; `value` donne la valeur initiale dans `bp.ui.form`.

| Élément | Valeur dans l'état |
| --- | --- |
| `ui.input{ "clé" }`, `ui.password{…}`, `ui.textarea{…}` | texte |
| `ui.number{ "clé", decimals = 0 }` | nombre (virgule ou point acceptés) |
| `ui.amount{ "clé" }` | montant |
| `ui.date{ "clé" }` | date (avec calendrier) |
| `ui.checkbox{ "clé", label }`, `ui.switch{ "clé", label }` | booléen |
| `ui.select{ "clé", options }` | valeur de l'option choisie (liste déroulante) |
| `ui.choice{ "clé", options }` | valeur de l'option choisie (boutons segmentés) |
| `ui.account{ "clé" }` | compte |
| `ui.category{ "clé" }` | libellé de catégorie, avec icônes et couleurs de l'application |
| `ui.mode{ "clé" }` | nom de mode de paiement |

Les options s'écrivent `{ "a", "b" }` ou `{ { valeur, "Libellé" }, … }` (ou `{ value = …, label = …, icon = … }`).

Tant qu'un nombre, un montant ou une date tapés ne sont pas valides, l'état garde l'ancienne valeur et le champ le signale.

### Boutons

`ui.button{ "Libellé", on_click, icon, primary, danger, style = "link", enabled, close, cancel }`, dans le corps ou dans `buttons`.

### Tableaux

```lua
ui.table({
	columns = { { "Date", width = 100 }, "Tiers", { "Montant", align = "right", width = 120 } },
	rows = {
		{ op.date, op.payee, ui.money(op.amount), id = op },
		{ "…", ui.cell("en retard", { color = "warning", icon = "clock" }), 12 },
	},
	select = "choix",       -- clé de l'état : id de la ligne choisie
	multiple = true,        -- liste d'id (Ctrl et Maj pour sélectionner plusieurs lignes)
	on_select = function(state, valeur, win) end,
	on_activate = function(state, id, win) end,   -- double-clic
	height = 300,
	empty = "Aucune ligne",
})
```

Une cellule est un texte, un nombre, une date, `ui.money(…)` ou `ui.cell(texte, { color, bold, align, icon })`. L'`id` d'une ligne est ce que la sélection renvoie : une opération, un texte, un nombre… (par défaut, le numéro de la ligne). Les tableaux longs restent fluides : seules les lignes visibles sont dessinées.

## Évènements

```lua
bp.on("operation_added", function(op)
	if op.payee:lower():find("carrefour") and op.category == nil then
		op.category = "Alimentation > Epicerie"
	end
end)
```

| Évènement | Quand | Argument |
| --- | --- | --- |
| `"operation_added"` | une opération est saisie ou importée (une fois par opération) | l'opération |
| `"operation_changed"` | une opération est modifiée dans le formulaire de saisie | l'opération |
| `"file_opened"` | un fichier vient d'être ouvert | — |
| `"before_save"` | juste avant l'enregistrement | — |

Les modifications faites par une réaction à `operation_added` ou `operation_changed` rejoignent l'étape d'annulation de la saisie : un seul Ctrl+Z annule la saisie et ce que l'extension y a ajouté. Les opérations créées par les extensions ne déclenchent pas d'évènement.

## Stockage

| | Où | Pour |
| --- | --- | --- |
| `bp.store.get(clé)` / `bp.store.set(clé, valeur)` | dans le fichier de comptes (chiffré avec lui) | réglages liés aux comptes, données de l'extension |
| `bp.prefs.get(clé)` / `bp.prefs.set(clé, valeur)` | dans la configuration de l'utilisateur | préférences personnelles, communes à tous les fichiers |

Les valeurs sont du texte, des nombres, des booléens, des tables (listes ou dictionnaires) et des dates (rangées sous forme de texte). `set(clé, nil)` efface. Les comptes et opérations ne se rangent pas : gardez leur nom ou leur `uid`.

## Fichiers, Internet, JSON

```lua
local f = bp.files.open({ title = "Relevé", filters = { CSV = { "csv", "txt" } } })
if f then
	for ligne in f.text:gmatch("[^\r\n]+") do
		-- f.name, f.path ; le texte est décodé (UTF-8, sinon Windows-1252)
	end
end

local chemin = bp.files.save({ title = "Exporter", name = "export.csv", text = contenu, filters = { CSV = { "csv" } } })
```

Les extensions n'ont accès qu'aux fichiers choisis par l'utilisateur dans ces fenêtres du système.

```lua
bp.extension({ name = "Cours", permissions = { "network" } })

local r = bp.http.get("https://api.exemple.com/cours?devise=USD", { headers = { Accept = "application/json" } })
if r.ok then
	local data = bp.json.decode(r.body)
end
-- bp.http.post(url, corps, { headers = … }) ; r.status, r.ok, r.body
```

Sans `permissions = { "network" }`, `bp.http` refuse. `bp.open_url(url)` ouvre une page dans le navigateur.

`bp.json.encode(valeur, lisible?)` et `bp.json.decode(texte)` convertissent entre tables et JSON.

## Annulation, erreurs et limites

- **Une commande = une étape d'annulation.** Tout ce qu'une commande modifie (opérations, catégories, modes, informations des comptes) s'annule d'un Ctrl+Z, nommé d'après la commande ; seule la création d'un compte (`bp.add_account`) n'est pas défaite (le compte reste, avec son solde initial). Dans une fenêtre, chaque clic sur un bouton qui modifie le fichier est une étape.
- **Une erreur annule les modifications** faites depuis la dernière boîte de dialogue ; le message, avec le fichier et la ligne, s'affiche. Utilisez `error("message")` pour arrêter une commande proprement, et `pcall` pour rattraper une erreur.
- **Lignes rapprochées verrouillées** : si l'option est active (menu Opérations), les extensions ne peuvent ni modifier ni supprimer ces lignes.
- **Temps** : un appel qui dure plus de 20 secondes sans rendre la main est arrêté (boucle infinie…).
- **Mémoire** : 512 Mo par extension.
- `print(…)` écrit dans le journal de l'extension (fenêtre de gestion).
- Les objets compte et opération ne sont valables que pour le fichier ouvert.

## Sécurité

Chaque extension tourne dans sa propre machine virtuelle Luau, en bac à sable :

- pas d'accès direct au disque ni au système (`io`, `os.execute`, chargement de bibliothèques… n'existent pas) ;
- fichiers uniquement par les fenêtres Ouvrir / Enregistrer, choisis par l'utilisateur ;
- Internet seulement si l'extension le déclare, ce que la fenêtre de gestion signale ;
- les variables globales de BankPerfect (`bp`…) sont en lecture seule.

Une extension a accès à vos comptes : n'installez que des extensions dont vous connaissez l'auteur ou dont vous avez lu le code.

## Référence rapide

Pour l'auto-complétion et la vérification des types dans votre éditeur, installez [luau-lsp](https://github.com/JohnnyMorganz/luau-lsp) et indiquez-lui le fichier de définitions [`bankperfect.d.luau`](bankperfect.d.luau) (option `luau-lsp.types.definitionFiles` dans VS Code).

```text
bp.extension{ name, description, version, author, permissions }
bp.command{ title, run, icon, shortcut, needs, description, id }
bp.on(évènement, fonction)

bp.accounts{ all? }   bp.account(nom)   bp.current_account()   bp.add_account{ name, initial, currency }
bp.selection()   bp.visible()   bp.find(uid)   bp.select(ops)   bp.transfer{ from, to, amount, date, details, category }
bp.categories()   bp.category(x)   bp.add_category(nom, mère?)   bp.rename_category(c, nom)   bp.delete_category(c)
bp.modes()   bp.add_mode(nom, dépense?)   bp.payees()   bp.file()

bp.date(a, m, j) | bp.date(texte)   bp.today()   bp.is_date(x)
bp.money(x, devise?)   bp.round(x, décimales?)   bp.parse_amount(texte)

bp.ui.alert(msg, opts)   bp.ui.confirm(msg, opts) → booléen   bp.ui.prompt(libellé, opts) → texte|nil   bp.ui.toast(msg)
bp.ui.form{ … } → valeurs|nil   bp.ui.window{ title, icon, size, subtitle, state, view, buttons, on_change } → valeur

bp.store.get/set   bp.prefs.get/set   bp.json.encode/decode
bp.files.open{…}   bp.files.save{…}   bp.http.get(url, opts)   bp.http.post(url, corps, opts)   bp.open_url(url)
print(…)   require("module")   bp.version   bp.id
```
