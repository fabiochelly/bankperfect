-- Épargne par arrondis : chaque paiement par carte est arrondi à l'euro
-- (ou aux 5 €) supérieur et la différence est virée vers un compte épargne.

bp.extension({
	name = "Épargne par arrondis",
	description = "À chaque paiement par carte saisi ou importé, la différence avec l'arrondi supérieur est transférée vers le compte d'épargne de votre choix.",
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

-- Réglages rangés dans le fichier de comptes : ils suivent le fichier.
local function settings()
	return bp.store.get("reglages") or { active = false, step = 1 }
end

local function rounding(amount, step)
	local spent = -amount
	local rounded = math.ceil(spent / step - 1e-9) * step
	return bp.round(rounded - spent)
end

bp.on("operation_added", function(op)
	local s = settings()
	if not s.active or op.mode ~= "Carte" or op.amount >= 0 then
		return
	end
	local savings = bp.account(s.account)
	if savings == nil or savings == op.account then
		return
	end
	local extra = rounding(op.amount, s.step)
	if extra > 0 then
		bp.transfer({
			from = op.account,
			to = savings,
			amount = extra,
			date = op.date,
			details = "Arrondi " .. op.payee,
		})
	end
end)

bp.command({
	id = "reglages",
	title = "Épargne par arrondis…",
	icon = "piggy-bank",
	run = function()
		local s = settings()
		local values = ui.form({
			title = "Épargne par arrondis",
			icon = "piggy-bank",
			subtitle = "Les paiements par carte ajoutés ensuite seront arrondis ; la différence part sur le compte choisi.",
			section = "Réglages",
			ui.switch({ "active", label = "Arrondir les paiements par carte", value = s.active }),
			ui.account({ "account", label = "Compte d'épargne", value = s.account and bp.account(s.account) }),
			ui.choice({
				"step",
				label = "Arrondir à",
				value = s.step,
				options = { { 1, "1 €" }, { 2, "2 €" }, { 5, "5 €" }, { 10, "10 €" } },
			}),
			validate = function(v)
				if v.active and v.account == nil then
					return "Choisissez le compte d'épargne."
				end
				if v.active and v.account == bp.current_account() then
					return "Le compte d'épargne doit être différent du compte courant."
				end
				return nil
			end,
		})
		if values then
			bp.store.set("reglages", {
				active = values.active == true,
				account = values.account and values.account.name,
				step = values.step,
			})
			ui.toast(if values.active then "Arrondis activés" else "Arrondis désactivés")
		end
	end,
})
