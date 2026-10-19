-- Total de la sélection : somme, moyenne, dépenses et recettes des lignes
-- sélectionnées, d'un clic droit dans la grille.

-- Texte dans la langue de l'interface : bp.language vaut "fr" ou "en" (les
-- extensions sont rechargées quand l'utilisateur change de langue).
local function tr(fr, en)
	return if bp.language == "en" then en else fr
end

bp.extension({
	name = tr("Total de la sélection", "Selection total"),
	description = tr(
		"Additionne les opérations sélectionnées : total, moyenne, dépenses, recettes.",
		"Adds up the selected transactions: total, average, expenses, income."
	),
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

bp.command({
	id = "total",
	title = tr("Total de la sélection…", "Selection total…"),
	icon = "sigma",
	shortcut = "Ctrl+Alt+T",
	needs = "selection", -- grisée tant qu'aucune ligne n'est sélectionnée
	menu = "grid", -- clic droit sur les opérations
	run = function()
		local ops = bp.selection()
		if #ops == 0 then
			return
		end
		local currency = bp.current_account().currency
		local total, debits, credits = 0, 0, 0
		local first, last
		for _, op in ops do
			total += op.amount
			if op.amount < 0 then
				debits += op.amount
			else
				credits += op.amount
			end
			if first == nil or op.date < first then
				first = op.date
			end
			if last == nil or last < op.date then
				last = op.date
			end
		end
		ui.window({
			title = tr("Total de la sélection", "Selection total"),
			icon = "sigma",
			size = "small",
			subtitle = tr(`{#ops} opération(s), du {first} au {last}.`, `{#ops} transaction(s), from {first} to {last}.`),
			view = function()
				return {
					ui.columns({
						ui.stat({ label = "Total", value = ui.money(total, { currency = currency }), icon = "sigma" }),
						ui.stat({
							label = tr("Moyenne", "Average"),
							value = ui.money(total / #ops, { currency = currency }),
							icon = "equals",
						}),
					}),
					ui.columns({
						ui.stat({
							label = tr("Dépenses", "Expenses"),
							value = ui.money(debits, { currency = currency }),
							icon = "arrow-down-right",
							color = "negative",
						}),
						ui.stat({
							label = tr("Recettes", "Income"),
							value = ui.money(credits, { currency = currency }),
							icon = "arrow-up-right",
							color = "positive",
						}),
					}),
				}
			end,
		})
	end,
})
