-- Prélude des extensions BankPerfect : constructeurs des éléments de
-- fenêtre et boîtes de dialogue. Les boîtes de dialogue suspendent la
-- coroutine de l'extension ; l'application la reprend avec la réponse.

local ui = bp.ui

-- Textes affichés, traduits par l'application (fonction réservée au prélude).
local text = __bp_text
__bp_text = nil

local function copy(spec)
	local t = {}
	if spec then
		for k, v in spec do
			t[k] = v
		end
	end
	return t
end

local function ask(kind, spec)
	if not coroutine.isyieldable() then
		error(text("not_here", kind), 3)
	end
	spec.__bp = kind
	return coroutine.yield(spec)
end

-- ─────────────────────────── Boîtes de dialogue ───────────────────────────

function ui.alert(message, opts)
	local spec = copy(opts)
	spec.message = tostring(message)
	ask("alert", spec)
end

function ui.confirm(message, opts)
	local spec = copy(opts)
	spec.message = tostring(message)
	return ask("confirm", spec) == true
end

function ui.prompt(label, opts)
	local spec = copy(if type(opts) == "string" then { default = opts } else opts)
	spec.label = label
	return ask("prompt", spec)
end

function ui.window(spec)
	if type(spec) ~= "table" then
		error(text("window_table"), 2)
	end
	return ask("window", spec)
end

-- ───────────────────────────── Éléments ─────────────────────────────

local function element(kind)
	return function(spec)
		local t = if type(spec) == "table" then copy(spec) else { spec }
		t.type = kind
		return t
	end
end

function ui.text(text, opts)
	local t = copy(opts)
	t.type = "text"
	t[1] = text
	return t
end

function ui.heading(text)
	return { type = "text", text, style = "heading" }
end

function ui.muted(text)
	return { type = "text", text, style = "muted" }
end

function ui.strong(text)
	return { type = "text", text, style = "strong" }
end

function ui.money(value, opts)
	local t = if type(opts) == "string" then { currency = opts } else copy(opts)
	t.type = "money"
	t.value = value
	return t
end

function ui.cell(text, opts)
	local t = copy(opts)
	t.type = "text"
	t[1] = text
	return t
end

ui.section = element("section")
ui.card = element("card")
ui.row = element("row")
ui.columns = element("columns")
ui.table = element("table")
ui.stat = element("stat")
ui.bars = element("bars")
ui.chart = element("chart")

function ui.pill(text, color)
	return { type = "pill", text, color = color }
end

function ui.notice(text, opts)
	local t = if type(opts) == "string" then { color = opts } else copy(opts)
	t.type = "notice"
	t[1] = text
	return t
end

function ui.progress(value, opts)
	local t = copy(opts)
	t.type = "progress"
	t.value = value
	return t
end

function ui.separator()
	return { type = "separator" }
end

function ui.space(size)
	return { type = "space", size = size }
end

local function field(kind)
	return function(spec)
		local t = if type(spec) == "table" then copy(spec) else { spec }
		t.type = "input"
		t.field = kind
		return t
	end
end

ui.input = field("text")
ui.textarea = field("multiline")
ui.password = field("password")
ui.number = field("number")
ui.amount = field("amount")
ui.date = field("date")
ui.checkbox = field("checkbox")
ui.switch = field("switch")
ui.select = field("select")
ui.choice = field("choice")
ui.account = field("account")
ui.category = field("category")
ui.mode = field("mode")

function ui.button(spec)
	local t = if type(spec) == "table" then copy(spec) else { spec }
	t.type = "button"
	if t.primary then
		t.style = "primary"
	elseif t.danger then
		t.style = "danger"
	end
	local handler = t.on_click
	if t.cancel then
		t.on_click = function(state, win)
			if handler then
				handler(state, win)
			end
			win:close(nil)
		end
	elseif t.close then
		t.on_click = function(state, win)
			local result = if handler then handler(state, win) else nil
			if result ~= false then
				win:close(if result == nil or result == true then state else result)
			end
		end
	end
	return t
end

-- ───────────────────────────── Formulaire ─────────────────────────────

-- Fenêtre de saisie simple : renvoie la table des valeurs, ou nil si
-- l'utilisateur annule.
function ui.form(spec)
	local state = {}
	local fields = {}
	for k, v in spec do
		if type(k) == "number" and type(v) == "table" then
			if v.type == "input" and v[1] ~= nil and v.value ~= nil then
				state[v[1]] = v.value
			end
			table.insert(fields, { k, v })
		end
	end
	table.sort(fields, function(a, b)
		return a[1] < b[1]
	end)
	local function body(s)
		local section = { type = "section", title = spec.section, icon = spec.section_icon or "sliders-horizontal" }
		for _, item in fields do
			table.insert(section, item[2])
		end
		local list = { section }
		if s.__error then
			table.insert(list, ui.notice(s.__error, { color = "negative", icon = "warning" }))
		end
		return list
	end
	local result = ui.window({
		title = spec.title,
		icon = spec.icon,
		subtitle = spec.subtitle,
		size = spec.size or "small",
		state = state,
		view = body,
		buttons = {
			ui.button({ text("cancel"), cancel = true }),
			ui.button({
				spec.ok or text("ok"),
				primary = true,
				close = true,
				on_click = function(s)
					s.__error = nil
					if spec.validate then
						local message = spec.validate(s)
						if message then
							s.__error = message
							return false
						end
					end
					return nil
				end,
			}),
		},
	})
	if result then
		result.__error = nil
	end
	return result
end
