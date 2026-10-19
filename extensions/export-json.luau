-- Export JSON : les opérations d'un compte sur une période, pour un tableur,
-- un script ou une autre application.

-- Texte dans la langue de l'interface : bp.language vaut "fr" ou "en" (les
-- extensions sont rechargées quand l'utilisateur change de langue).
local function tr(fr, en)
	return if bp.language == "en" then en else fr
end

bp.extension({
	name = tr("Export JSON", "JSON export"),
	description = tr(
		"Exporte les opérations d'un compte sur une période au format JSON.",
		"Exports an account's transactions over a period as JSON."
	),
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

bp.command({
	id = "exporter",
	title = tr("Exporter des opérations en JSON…", "Export transactions as JSON…"),
	icon = "brackets-curly",
	menu = "export", -- sous-menu Fichier › Exporter
	run = function()
		local today = bp.today()
		local v = ui.form({
			title = tr("Exporter en JSON", "Export as JSON"),
			icon = "brackets-curly",
			section = tr("Opérations à exporter", "Transactions to export"),
			ok = tr("Exporter…", "Export…"),
			ui.account({ "account", label = tr("Compte", "Account"), value = bp.current_account() }),
			ui.date({ "from", label = tr("Du", "From"), value = today:start_of_year() }),
			ui.date({ "to", label = tr("Au", "To"), value = today }),
			ui.checkbox({ "pretty", label = tr("Mise en forme lisible", "Readable layout (indented)"), value = true }),
			validate = function(s)
				if s.account == nil then
					return tr("Choisissez un compte.", "Choose an account.")
				end
				if s.from == nil or s.to == nil or s.to < s.from then
					return tr("Période invalide.", "Invalid period.")
				end
				return nil
			end,
		})
		if v == nil then
			return
		end
		-- Les clés du JSON et les modes de paiement (noms stables de l'API) ne
		-- dépendent pas de la langue : le fichier se relit de la même façon.
		local list = {}
		for _, op in v.account:operations({ from = v.from, to = v.to }) do
			table.insert(list, {
				uid = op.uid,
				date = op.date:iso(),
				mode = op.mode,
				tiers = op.payee,
				detail = op.details,
				categorie = op.category,
				montant = op.amount,
				pointage = op.tack,
			})
		end
		local text = bp.json.encode({
			compte = v.account.name,
			devise = v.account.currency,
			du = v.from:iso(),
			au = v.to:iso(),
			operations = list,
		}, v.pretty)
		local path = bp.files.save({
			title = tr("Exporter en JSON", "Export as JSON"),
			name = v.account.name .. ".json",
			text = text,
			filters = { JSON = { "json" } },
		})
		if path then
			ui.toast(tr(`{#list} opération(s) exportée(s)`, `{#list} transaction(s) exported`))
		end
	end,
})
