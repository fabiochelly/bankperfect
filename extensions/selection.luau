-- Total de la sélection : somme, moyenne, dépenses et recettes des lignes
-- sélectionnées, d'un clic droit dans la grille.

bp.extension({
	name = "Total de la sélection",
	description = "Additionne les opérations sélectionnées : total, moyenne, dépenses, recettes.",
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

bp.command({
	id = "total",
	title = "Total de la sélection…",
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
			title = "Total de la sélection",
			icon = "sigma",
			size = "small",
			subtitle = `{#ops} opération(s), du {first} au {last}.`,
			view = function()
				return {
					ui.columns({
						ui.stat({ label = "Total", value = ui.money(total, { currency = currency }), icon = "sigma" }),
						ui.stat({
							label = "Moyenne",
							value = ui.money(total / #ops, { currency = currency }),
							icon = "equals",
						}),
					}),
					ui.columns({
						ui.stat({
							label = "Dépenses",
							value = ui.money(debits, { currency = currency }),
							icon = "arrow-down-right",
							color = "negative",
						}),
						ui.stat({
							label = "Recettes",
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
