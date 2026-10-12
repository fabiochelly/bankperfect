-- Synthèse du mois : dépenses, recettes et répartition par catégorie du
-- compte affiché, mois par mois.

bp.extension({
	name = "Synthèse du mois",
	description = "Tableau de bord mensuel du compte affiché : dépenses, recettes, plus grosses dépenses et répartition par catégorie.",
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

local function summary(account, month)
	local from, to = month:start_of_month(), month:end_of_month()
	local spent, earned = 0, 0
	local by_category = {}
	local biggest = {}
	for _, op in account:operations({ from = from, to = to }) do
		if op.amount < 0 then
			spent += -op.amount
			local family = (op.category or "Sans catégorie"):match("^[^>]+") or "Sans catégorie"
			family = family:gsub("%s+$", "")
			by_category[family] = (by_category[family] or 0) + -op.amount
			table.insert(biggest, op)
		else
			earned += op.amount
		end
	end
	table.sort(biggest, function(a, b)
		return a.amount < b.amount
	end)
	local bars = {}
	for name, total in by_category do
		table.insert(bars, { label = name, value = total })
	end
	table.sort(bars, function(a, b)
		return a.value > b.value
	end)
	return spent, earned, bars, biggest
end

local SHORT_MONTHS = { "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc." }

-- Dépenses et recettes des six mois qui finissent par `month`, pour ui.chart.
local function six_months(account, month)
	local labels, spent, earned = {}, {}, {}
	for i = 5, 0, -1 do
		local m = month:add_months(-i)
		local s, e = summary(account, m)
		table.insert(labels, SHORT_MONTHS[m.month])
		table.insert(spent, s)
		table.insert(earned, e)
	end
	return labels, spent, earned
end

bp.command({
	id = "synthese",
	title = "Synthèse du mois…",
	icon = "chart-pie-slice",
	menu = { "reports", "account" }, -- menu Rapports et clic droit sur un compte
	shortcut = "Ctrl+Alt+M",
	run = function()
		local account = bp.current_account()
		-- Mois de la dernière opération passée (le mois en cours d'habitude).
		local start = bp.today()
		local past = account:operations({ to = start })
		if #past > 0 then
			start = past[#past].date
		end
		ui.window({
			title = "Synthèse — " .. account.name,
			icon = "chart-pie-slice",
			size = "large",
			state = { month = start:start_of_month() },
			view = function(s)
				local spent, earned, bars, biggest = summary(account, s.month)
				local top = {}
				for i = 1, math.min(5, #biggest) do
					local op = biggest[i]
					table.insert(top, { op.date, op.payee, ui.cell(op.category or "—", { color = "muted" }), ui.money(op.amount), id = op })
				end
				local name = s.month.month_name
				return {
					ui.row({
						ui.button({ "Mois précédent", icon = "caret-left", on_click = function(st)
							st.month = st.month:add_months(-1)
						end }),
						ui.heading(name:sub(1, 1):upper() .. name:sub(2) .. " " .. s.month.year),
						ui.button({ "Mois suivant", icon = "caret-right", on_click = function(st)
							st.month = st.month:add_months(1)
						end }),
					}),
					ui.space(6),
					ui.row({
						ui.stat({ label = "Dépenses", value = ui.money(-spent), icon = "arrow-down-right", color = "negative" }),
						ui.stat({ label = "Recettes", value = ui.money(earned), icon = "arrow-up-right", color = "positive" }),
						ui.stat({
							label = "Solde du mois",
							value = ui.money(earned - spent),
							icon = "scales",
							color = if earned >= spent then "positive" else "negative",
						}),
					}),
					ui.space(8),
					ui.columns({
						ui.section({
							title = "Par catégorie",
							icon = "chart-bar-horizontal",
							if #bars > 0 then ui.bars({ data = bars }) else ui.muted("Aucune dépense ce mois-ci."),
						}),
						ui.section({
							title = "Plus grosses dépenses",
							icon = "receipt",
							ui.table({
								columns = { { "Date", width = 96 }, "Tiers", "Catégorie", { "Montant", align = "right", width = 110 } },
								rows = top,
								on_activate = function(_, op)
									bp.select({ op })
								end,
								empty = "Rien à afficher",
							}),
							ui.text("Double-cliquez pour retrouver l'opération dans la grille.", { style = "small" }),
						}),
					}),
					ui.section({
						title = "Sur six mois",
						icon = "chart-bar",
						(function()
							local labels, spent6, earned6 = six_months(account, s.month)
							return ui.chart({
								style = "columns",
								labels = labels,
								series = {
									{ name = "Dépenses", values = spent6 },
									{ name = "Recettes", values = earned6 },
								},
								height = 220,
							})
						end)(),
					}),
				}
			end,
		})
	end,
})
