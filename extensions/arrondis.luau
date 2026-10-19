-- Épargne par arrondis : chaque paiement par carte est arrondi à l'euro
-- (ou aux 5 €) supérieur et la différence est virée vers un compte épargne.

-- Texte dans la langue de l'interface : bp.language vaut "fr" ou "en" (les
-- extensions sont rechargées quand l'utilisateur change de langue).
local function tr(fr, en)
	return if bp.language == "en" then en else fr
end

bp.extension({
	name = tr("Épargne par arrondis", "Round-up savings"),
	description = tr(
		"À chaque paiement par carte saisi ou importé, la différence avec l'arrondi supérieur est transférée vers le compte d'épargne de votre choix.",
		"Every time a card payment is entered or imported, the difference up to the next whole euro (or €5) is transferred to the savings account of your choice."
	),
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

-- Réglages rangés dans le fichier de comptes : ils suivent le fichier. La clé
-- "reglages" et les valeurs enregistrées ne dépendent pas de la langue.
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
	-- Les modes de paiement standard gardent leur nom français dans l'API,
	-- quelle que soit la langue de l'interface.
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
			details = tr("Arrondi ", "Round-up ") .. op.payee,
		})
	end
end)

bp.command({
	id = "reglages",
	title = tr("Épargne par arrondis…", "Round-up savings…"),
	icon = "piggy-bank",
	run = function()
		local s = settings()
		-- « 5 € », « $5 » : dans la devise du compte affiché.
		local cur = bp.current_account().currency
		local function step_label(n)
			return tr(`{n} {cur}`, `{cur}{n}`)
		end
		local values = ui.form({
			title = tr("Épargne par arrondis", "Round-up savings"),
			icon = "piggy-bank",
			subtitle = tr(
				"Les paiements par carte ajoutés ensuite seront arrondis ; la différence part sur le compte choisi.",
				"Card payments added from now on will be rounded up; the difference goes to the chosen account."
			),
			section = tr("Réglages", "Settings"),
			ui.switch({ "active", label = tr("Arrondir les paiements par carte", "Round up card payments"), value = s.active }),
			ui.account({
				"account",
				label = tr("Compte d'épargne", "Savings account"),
				value = s.account and bp.account(s.account),
			}),
			ui.choice({
				"step",
				label = tr("Arrondir à", "Round up to"),
				value = s.step,
				options = {
					{ 1, step_label(1) },
					{ 2, step_label(2) },
					{ 5, step_label(5) },
					{ 10, step_label(10) },
				},
			}),
			validate = function(v)
				if v.active and v.account == nil then
					return tr("Choisissez le compte d'épargne.", "Choose the savings account.")
				end
				if v.active and v.account == bp.current_account() then
					return tr(
						"Le compte d'épargne doit être différent du compte courant.",
						"The savings account must be different from the current account."
					)
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
			if values.active then
				ui.toast(tr("Arrondis activés", "Round-ups turned on"))
			else
				ui.toast(tr("Arrondis désactivés", "Round-ups turned off"))
			end
		end
	end,
})
