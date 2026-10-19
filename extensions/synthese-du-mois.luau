-- Synthèse du mois : dépenses, recettes et répartition par catégorie du
-- compte affiché, mois par mois.

-- Texte dans la langue de l'interface : bp.language vaut "fr" ou "en" (les
-- extensions sont rechargées quand l'utilisateur change de langue).
local function tr(fr, en)
	return if bp.language == "en" then en else fr
end

bp.extension({
	name = tr("Synthèse du mois", "Monthly summary"),
	description = tr(
		"Tableau de bord mensuel du compte affiché : dépenses, recettes, plus grosses dépenses, répartition par catégorie et six derniers mois.",
		"Monthly dashboard for the account shown: expenses, income, largest expenses, breakdown by category and the last six months."
	),
	version = "1.0",
	author = "BankPerfect",
})

local ui = bp.ui

local UNCATEGORIZED = tr("Sans catégorie", "Uncategorized")

local function summary(account, month)
	local from, to = month:start_of_month(), month:end_of_month()
	local spent, earned = 0, 0
	local by_category = {}
	local biggest = {}
	for _, op in account:operations({ from = from, to = to }) do
		if op.amount < 0 then
			spent += -op.amount
			local family = (op.category or UNCATEGORIZED):match("^[^>]+") or UNCATEGORIZED
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

local SHORT_MONTHS = tr(
	{ "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc." },
	{ "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec" }
)

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
	title = tr("Synthèse du mois…", "Monthly summary…"),
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
			title = tr("Synthèse — ", "Monthly summary — ") .. account.name,
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
				local name = s.month.month_name -- dans la langue de l'interface
				return {
					ui.row({
						ui.button({ tr("Mois précédent", "Previous month"), icon = "caret-left", on_click = function(st)
							st.month = st.month:add_months(-1)
						end }),
						ui.heading(name:sub(1, 1):upper() .. name:sub(2) .. " " .. s.month.year),
						ui.button({ tr("Mois suivant", "Next month"), icon = "caret-right", on_click = function(st)
							st.month = st.month:add_months(1)
						end }),
					}),
					ui.space(6),
					ui.row({
						ui.stat({
							label = tr("Dépenses", "Expenses"),
							value = ui.money(-spent),
							icon = "arrow-down-right",
							color = "negative",
						}),
						ui.stat({
							label = tr("Recettes", "Income"),
							value = ui.money(earned),
							icon = "arrow-up-right",
							color = "positive",
						}),
						ui.stat({
							label = tr("Solde du mois", "Net for the month"),
							value = ui.money(earned - spent),
							icon = "scales",
							color = if earned >= spent then "positive" else "negative",
						}),
					}),
					ui.space(8),
					ui.columns({
						ui.section({
							title = tr("Par catégorie", "By category"),
							icon = "chart-bar-horizontal",
							if #bars > 0
								then ui.bars({ data = bars })
								else ui.muted(tr("Aucune dépense ce mois-ci.", "No expenses this month.")),
						}),
						ui.section({
							title = tr("Plus grosses dépenses", "Largest expenses"),
							icon = "receipt",
							ui.table({
								columns = {
									{ "Date", width = 96 },
									tr("Tiers", "Payee"),
									tr("Catégorie", "Category"),
									{ tr("Montant", "Amount"), align = "right", width = 110 },
								},
								rows = top,
								on_activate = function(_, op)
									bp.select({ op })
								end,
								empty = tr("Rien à afficher", "Nothing to show"),
							}),
							ui.text(
								tr(
									"Double-cliquez pour retrouver l'opération dans la grille.",
									"Double-click to find the transaction in the grid."
								),
								{ style = "small" }
							),
						}),
					}),
					ui.section({
						title = tr("Sur six mois", "Last six months"),
						icon = "chart-bar",
						(function()
							local labels, spent6, earned6 = six_months(account, s.month)
							return ui.chart({
								style = "columns",
								labels = labels,
								series = {
									{ name = tr("Dépenses", "Expenses"), values = spent6 },
									{ name = tr("Recettes", "Income"), values = earned6 },
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
