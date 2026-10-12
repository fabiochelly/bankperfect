-- Recherche des doublons : opérations de même montant, à quelques jours
-- d'écart, souvent dues à un import fait deux fois.

bp.extension({
	name = "Recherche des doublons",
	description = "Repère les opérations en double (même montant, dates proches) et supprime celles que vous choisissez.",
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
	title = "Rechercher les doublons…",
	icon = "copy",
	shortcut = "Ctrl+Alt+D",
	description = "Sur le compte affiché.",
	run = function()
		local account = bp.current_account()
		ui.window({
			title = "Doublons de « " .. account.name .. " »",
			icon = "copy",
			size = "large",
			subtitle = "Chaque ligne est la plus récente d'une paire. Sélectionnez celles à supprimer (Ctrl ou Maj pour en prendre plusieurs).",
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
						ui.cell("doublon du " .. tostring(p.first.date), { color = "warning" }),
						ui.money(op.amount),
						id = op,
					})
				end
				s.count = #rows
				return {
					ui.section({
						title = "Critères",
						icon = "sliders-horizontal",
						ui.row({
							ui.number({ "days", label = "Écart maximal (jours)" }),
							ui.switch({ "same_payee", label = "Même tiers" }),
						}),
					}),
					ui.row({
						ui.stat({ label = "Paires trouvées", value = #rows, icon = "copy", color = if #rows > 0 then "warning" else "positive" }),
						ui.stat({ label = "Sélectionnées", value = #s.picked, icon = "check-square" }),
					}),
					ui.table({
						columns = { { "Date", width = 100 }, "Tiers", "Détail", "Remarque", { "Montant", align = "right", width = 120 } },
						rows = rows,
						select = "picked",
						multiple = true,
						empty = "Aucun doublon avec ces critères.",
					}),
				}
			end,
			buttons = function(s)
				return {
					ui.button({ "Fermer", cancel = true }),
					ui.button({
						"Supprimer " .. #s.picked .. " ligne(s)",
						icon = "trash",
						danger = true,
						enabled = #s.picked > 0,
						on_click = function(state)
							if ui.confirm(#state.picked .. " opération(s) seront supprimées (annulable avec Ctrl+Z).", { ok = "Supprimer", danger = true }) then
								for _, op in state.picked do
									op:delete()
								end
								ui.toast(#state.picked .. " doublon(s) supprimé(s)")
								state.picked = {}
							end
						end,
					}),
				}
			end,
		})
	end,
})
