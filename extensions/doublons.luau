-- Recherche des doublons : opérations de même montant, à quelques jours
-- d'écart, souvent dues à un import fait deux fois.

-- Texte dans la langue de l'interface : bp.language vaut "fr" ou "en" (les
-- extensions sont rechargées quand l'utilisateur change de langue).
local function tr(fr, en)
	return if bp.language == "en" then en else fr
end

bp.extension({
	name = tr("Recherche des doublons", "Find duplicates"),
	description = tr(
		"Repère les opérations en double (même montant, dates proches) et supprime celles que vous choisissez.",
		"Spots duplicate transactions (same amount, close dates) and deletes the ones you pick."
	),
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

-- Paires d'opérations semblables du compte, la plus récente en second.
local function find_pairs(account, days, same_payee)
	local ops = account:operations()
	local pairs = {}
	for i = 1, #ops do
		local a = ops[i]
		for j = i + 1, #ops do
			local b = ops[j]
			if b.date - a.date > days then
				break
			end
			if a.amount == b.amount and (not same_payee or a.payee:lower() == b.payee:lower()) then
				table.insert(pairs, { first = a, second = b })
			end
		end
	end
	return pairs
end

bp.command({
	id = "rechercher",
	title = tr("Rechercher les doublons…", "Find duplicates…"),
	icon = "copy",
	menu = "operations", -- dans le menu Opérations plutôt que dans Extensions
	shortcut = "Ctrl+Alt+D",
	description = tr("Sur le compte affiché.", "In the account shown."),
	run = function()
		local account = bp.current_account()
		ui.window({
			title = tr(`Doublons de « {account.name} »`, `Duplicates in “{account.name}”`),
			icon = "copy",
			size = "large",
			subtitle = tr(
				"Chaque ligne est la plus récente d'une paire. Sélectionnez celles à supprimer (Ctrl ou Maj pour en prendre plusieurs).",
				"Each row is the more recent transaction of a pair. Select the ones to delete (Ctrl or Shift to pick several)."
			),
			state = { days = 3, same_payee = true, picked = {} },
			view = function(s)
				local found = find_pairs(account, s.days or 0, s.same_payee)
				local rows = {}
				for _, p in found do
					local op = p.second
					table.insert(rows, {
						op.date,
						op.payee,
						ui.cell(op.details, { color = "muted" }),
						ui.cell(tr(`doublon du {p.first.date}`, `duplicate of {p.first.date}`), { color = "warning" }),
						ui.money(op.amount),
						id = op,
					})
				end
				s.count = #rows
				return {
					ui.section({
						title = tr("Critères", "Criteria"),
						icon = "sliders-horizontal",
						ui.row({
							ui.number({ "days", label = tr("Écart maximal (jours)", "Maximum gap (days)") }),
							ui.switch({ "same_payee", label = tr("Même tiers", "Same payee") }),
						}),
					}),
					ui.row({
						ui.stat({
							label = tr("Paires trouvées", "Pairs found"),
							value = #rows,
							icon = "copy",
							color = if #rows > 0 then "warning" else "positive",
						}),
						ui.stat({ label = tr("Sélectionnées", "Selected"), value = #s.picked, icon = "check-square" }),
					}),
					ui.table({
						columns = {
							{ "Date", width = 100 },
							tr("Tiers", "Payee"),
							tr("Détail", "Memo"),
							tr("Remarque", "Note"),
							{ tr("Montant", "Amount"), align = "right", width = 120 },
						},
						rows = rows,
						select = "picked",
						multiple = true,
						empty = tr("Aucun doublon avec ces critères.", "No duplicates with these criteria."),
					}),
				}
			end,
			buttons = function(s)
				return {
					ui.button({ tr("Fermer", "Close"), cancel = true }),
					ui.button({
						tr(`Supprimer {#s.picked} ligne(s)`, `Delete {#s.picked} row(s)`),
						icon = "trash",
						danger = true,
						enabled = #s.picked > 0,
						on_click = function(state)
							local n = #state.picked
							local question = tr(
								`{n} opération(s) seront supprimées (annulable avec Ctrl+Z).`,
								`{n} transaction(s) will be deleted (Ctrl+Z to undo).`
							)
							if ui.confirm(question, { ok = tr("Supprimer", "Delete"), danger = true }) then
								for _, op in state.picked do
									op:delete()
								end
								ui.toast(tr(`{n} doublon(s) supprimé(s)`, `{n} duplicate(s) deleted`))
								state.picked = {}
							end
						end,
					}),
				}
			end,
		})
	end,
})
