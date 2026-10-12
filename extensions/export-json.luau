-- Export JSON : les opérations d'un compte sur une période, pour un tableur,
-- un script ou une autre application.

bp.extension({
	name = "Export JSON",
	description = "Exporte les opérations d'un compte sur une période au format JSON.",
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

bp.command({
	id = "exporter",
	title = "Exporter en JSON…",
	icon = "brackets-curly",
	run = function()
		local today = bp.today()
		local v = ui.form({
			title = "Exporter en JSON",
			icon = "brackets-curly",
			section = "Opérations à exporter",
			ok = "Exporter…",
			ui.account({ "account", label = "Compte", value = bp.current_account() }),
			ui.date({ "from", label = "Du", value = today:start_of_year() }),
			ui.date({ "to", label = "Au", value = today }),
			ui.checkbox({ "pretty", label = "Mise en forme lisible", value = true }),
			validate = function(s)
				if s.account == nil then
					return "Choisissez un compte."
				end
				if s.from == nil or s.to == nil or s.to < s.from then
					return "Période invalide."
				end
				return nil
			end,
		})
		if v == nil then
			return
		end
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
			title = "Exporter en JSON",
			name = v.account.name .. ".json",
			text = text,
			filters = { JSON = { "json" } },
		})
		if path then
			ui.toast(#list .. " opération(s) exportée(s)")
		end
	end,
})
