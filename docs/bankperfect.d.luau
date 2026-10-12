-- Définitions de l'API des extensions BankPerfect, pour luau-lsp
-- (auto-complétion et vérification des types dans l'éditeur).
-- VS Code : "luau-lsp.types.definitionFiles": ["chemin/vers/bankperfect.d.luau"]

declare class BPDate
	day: number
	month: number
	year: number
	weekday: number
	weekday_name: string
	month_name: string
	serial: number
	function add_days(self, days: number): BPDate
	function add_months(self, months: number): BPDate
	function add_years(self, years: number): BPDate
	function start_of_month(self): BPDate
	function end_of_month(self): BPDate
	function start_of_year(self): BPDate
	function end_of_year(self): BPDate
	function days_until(self, other: BPDate | string): number
	function format(self, pattern: string?): string
	function iso(self): string
	function __add(self, days: number): BPDate
	function __sub(self, other: number | BPDate): any
	function __lt(self, other: BPDate): boolean
	function __le(self, other: BPDate): boolean
end

type DateLike = BPDate | string
type Tack = "none" | "pointed" | "reconciled"

declare class BPOperation
	uid: number
	account: BPAccount
	date: BPDate
	mark_date: BPDate
	payee: string
	details: string
	amount: number
	mode: string
	category: string?
	category_id: number?
	tack: Tack
	cheque: string?
	is_transfer: boolean
	function field(self, name: string): string?
	function set_field(self, name: string, value: any): ()
	function fields(self): { [string]: string }
	function delete(self): ()
	function move_to(self, account: BPAccount | string): ()
	function copy_to(self, account: BPAccount | string): BPOperation
	function sister(self): BPOperation?
end

type OperationFilter = {
	from: DateLike?,
	to: DateLike?,
	payee: string?,
	text: string?,
	category: (string | number | boolean)?,
	mode: string?,
	tack: Tack?,
	min: number?,
	max: number?,
}

type NewOperation = {
	amount: number | string,
	date: DateLike?,
	payee: string?,
	details: string?,
	mode: string?,
	category: (string | number | Category)?,
	tack: Tack?,
	mark_date: DateLike?,
	fields: { [string]: any }?,
}

declare class BPAccount
	name: string
	bank: string
	agency: string
	number: string
	iban: string
	bic: string
	currency: string
	uid: string
	balance: number
	count: number
	disabled: boolean
	virtual: boolean
	current: boolean
	function operations(self, filter: OperationFilter?): { BPOperation }
	function add(self, operation: NewOperation): BPOperation
	function find(self, uid: number): BPOperation?
	function balance_at(self, date: DateLike): number
	function show(self): ()
end

type Category = { id: number, name: string, label: string, is_child: boolean, parent: string? }
type Mode = { name: string, debit: boolean, custom: boolean }
type Payee = { caption: string, payee: string, details: string, mode: string, category: string?, amount: number? }

declare class BPWindowHandle
	function close(self, value: any?): ()
end

type Widget = { [any]: any }
type Handler = (state: { [string]: any }, win: BPWindowHandle) -> any

type CommandSpec = {
	title: string,
	run: () -> (),
	icon: string?,
	shortcut: string?,
	needs: ("file" | "selection" | "nothing")?,
	description: string?,
	id: string?,
}

type WindowSpec = {
	title: string?,
	icon: string?,
	size: ("small" | "medium" | "large" | number)?,
	subtitle: string?,
	state: { [string]: any }?,
	view: ((state: { [string]: any }) -> { Widget? })?,
	buttons: ({ Widget } | (state: { [string]: any }) -> { Widget })?,
	on_change: ((state: { [string]: any }, key: string, win: BPWindowHandle) -> ())?,
}

type HttpResponse = { status: number, ok: boolean, body: string }
type FileFilters = { [string]: { string } }

declare bp: {
	version: string,
	id: string,

	extension: (spec: { name: string?, description: string?, version: string?, author: string?, permissions: { string }? }) -> (),
	command: (spec: CommandSpec) -> (),
	on: (event: "operation_added" | "operation_changed" | "file_opened" | "before_save", handler: (op: BPOperation?) -> ()) -> (),
	log: (...any) -> (),

	accounts: (options: { all: boolean? }?) -> { BPAccount },
	account: (name: string) -> BPAccount?,
	current_account: () -> BPAccount,
	add_account: (spec: { name: string, initial: number?, currency: string? }) -> BPAccount,
	selection: () -> { BPOperation },
	visible: () -> { BPOperation },
	find: (uid: number) -> BPOperation?,
	select: (operations: BPOperation | { BPOperation }) -> (),
	transfer: (spec: { from: BPAccount | string, to: BPAccount | string, amount: number, date: DateLike?, details: string?, category: any? }) -> (BPOperation, BPOperation),
	file: () -> { name: string, path: string, user: string }?,

	categories: () -> { Category },
	category: (category: string | number) -> Category?,
	add_category: (name: string, parent: (string | number | Category)?) -> Category,
	rename_category: (category: string | number | Category, name: string) -> (),
	delete_category: (category: string | number | Category) -> number,
	modes: () -> { Mode },
	add_mode: (name: string, debit: boolean?) -> string,
	payees: () -> { Payee },

	date: ((year: number, month: number, day: number) -> BPDate) & ((text: DateLike?) -> BPDate),
	today: () -> BPDate,
	is_date: (value: any) -> boolean,
	money: (amount: number, currency: string?) -> string,
	round: (value: number, decimals: number?) -> number,
	parse_amount: (text: string) -> number?,

	store: { get: (key: string) -> any, set: (key: string, value: any) -> () },
	prefs: { get: (key: string) -> any, set: (key: string, value: any) -> () },
	json: { encode: (value: any, pretty: boolean?) -> string, decode: (text: string) -> any },
	http: {
		get: (url: string, options: { headers: { [string]: string }? }?) -> HttpResponse,
		post: (url: string, body: string?, options: { headers: { [string]: string }? }?) -> HttpResponse,
	},
	files: {
		open: (spec: { title: string?, filters: FileFilters? }?) -> { name: string, path: string, text: string }?,
		save: (spec: { title: string?, name: string?, text: string, filters: FileFilters? }) -> string?,
	},
	open_url: (url: string) -> (),

	ui: {
		alert: (message: any, options: { title: string? }?) -> (),
		confirm: (message: any, options: { title: string?, ok: string?, danger: boolean? }?) -> boolean,
		prompt: (label: string, options: { title: string?, default: string?, placeholder: string?, ok: string? }?) -> string?,
		toast: (message: any) -> (),
		form: (spec: { [any]: any }) -> { [string]: any }?,
		window: (spec: WindowSpec) -> any,

		text: (text: any, options: { style: string?, color: string?, align: string? }?) -> Widget,
		heading: (text: any) -> Widget,
		muted: (text: any) -> Widget,
		strong: (text: any) -> Widget,
		money: (amount: number, options: (string | { currency: string?, colored: boolean?, style: string? })?) -> Widget,
		cell: (text: any, options: { color: string?, bold: boolean?, align: string?, icon: string? }?) -> Widget,
		section: (spec: Widget) -> Widget,
		card: (spec: Widget) -> Widget,
		row: (spec: Widget) -> Widget,
		columns: (spec: Widget) -> Widget,
		table: (spec: Widget) -> Widget,
		stat: (spec: Widget) -> Widget,
		bars: (spec: Widget) -> Widget,
		pill: (text: any, color: string?) -> Widget,
		notice: (text: any, options: (string | { color: string?, icon: string? })?) -> Widget,
		progress: (value: number, options: { text: string?, color: string? }?) -> Widget,
		separator: () -> Widget,
		space: (size: number?) -> Widget,
		input: (spec: Widget) -> Widget,
		textarea: (spec: Widget) -> Widget,
		password: (spec: Widget) -> Widget,
		number: (spec: Widget) -> Widget,
		amount: (spec: Widget) -> Widget,
		date: (spec: Widget) -> Widget,
		checkbox: (spec: Widget) -> Widget,
		switch: (spec: Widget) -> Widget,
		select: (spec: Widget) -> Widget,
		choice: (spec: Widget) -> Widget,
		account: (spec: Widget) -> Widget,
		category: (spec: Widget) -> Widget,
		mode: (spec: Widget) -> Widget,
		button: (spec: Widget | string) -> Widget,
	},
}
