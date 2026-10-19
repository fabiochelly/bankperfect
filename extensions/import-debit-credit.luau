-- Format d'import : relevé CSV « Date;Libellé;Débit;Crédit », la mise en
-- page de nombreuses banques françaises. Modèle à adapter au relevé de votre
-- banque : changez detect (comment reconnaître le fichier) et parse
-- (comment lire chaque ligne).
--
-- Fichier › Importer (ou glisser le fichier sur la fenêtre) : quand detect
-- reconnaît le fichier, l'aperçu habituel s'ouvre avec les opérations lues
-- ici (choix du compte, doublons, catégories).

-- Texte dans la langue de l'interface : bp.language vaut "fr" ou "en" (les
-- extensions sont rechargées quand l'utilisateur change de langue).
local function tr(fr, en)
	return if bp.language == "en" then en else fr
end

bp.extension({
	name = tr("Import Débit/Crédit", "Debit/Credit import"),
	description = tr(
		"Lit les relevés CSV à colonnes Date ; Libellé ; Débit ; Crédit.",
		"Reads CSV statements with Date; Description; Debit; Credit columns."
	),
	version = "1.0",
	author = "BankPerfect",
})

-- Texte en minuscules sans accents, pour comparer les en-têtes.
local function plain(text)
	text = text:lower()
	for from, to in { ["é"] = "e", ["è"] = "e", ["ê"] = "e", ["à"] = "a", ["ç"] = "c" } do
		text = text:gsub(from, to)
	end
	return text
end

-- Cellules d'une ligne CSV (séparateur « ; », guillemets facultatifs).
local function cells(line)
	local out = {}
	for cell in (line .. ";"):gmatch("(.-);") do
		table.insert(out, (cell:gsub('^%s*"?', ""):gsub('"?%s*$', "")))
	end
	return out
end

-- Rang des colonnes d'après l'en-tête, ou nil si ce n'est pas ce format.
-- La reconnaissance ne dépend pas de la langue de l'interface : un relevé
-- reste dans la langue de la banque (« Libellé » ou « Description »).
local function columns(header)
	local c = {}
	for i, name in cells(plain(header)) do
		if name:find("^date") and not c.date then
			c.date = i
		elseif name:find("libelle") or name:find("description") then
			c.label = i
		elseif name:find("debit") then
			c.debit = i
		elseif name:find("credit") then
			c.credit = i
		end
	end
	if c.date and c.label and c.debit and c.credit then
		return c
	end
	return nil
end

bp.importer({
	name = tr("Débit/Crédit", "Debit/Credit"),
	extensions = { "csv", "txt" },
	detect = function(file)
		local header = file.text:match("^[^\r\n]*")
		return columns(header) ~= nil
	end,
	parse = function(file)
		local lines = {}
		for line in file.text:gmatch("[^\r\n]+") do
			table.insert(lines, line)
		end
		local c = columns(lines[1])
		local operations = {}
		for i = 2, #lines do
			local row = cells(lines[i])
			local debit = bp.parse_amount(row[c.debit] or "")
			local credit = bp.parse_amount(row[c.credit] or "")
			-- Un débit est une dépense, qu'il soit écrit avec ou sans signe.
			local amount = if debit then -math.abs(debit) else credit
			-- Une ligne sans date ou sans montant est signalée dans l'aperçu.
			table.insert(operations, {
				date = row[c.date],
				payee = row[c.label],
				amount = amount,
			})
		end
		return operations
	end,
})
