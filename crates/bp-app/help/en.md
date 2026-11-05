## Getting started

BankPerfect keeps your accounts like a checkbook: each row is a transaction, and the software works out the balances for you. An accounts file (.bp extension) holds several accounts: checking account, joint account, savings…

### The window

- On the left, the accounts panel: the account on display, shown as a card, then the **Other accounts** with their balance, and the total. Click an account to display it.
- At the top, the balances of the account on display: **In bank** (cleared transactions), **Pending** (those that have not reached the bank yet) and **Final balance**.
- In the middle, the transaction table: date, payment method, payee and memo, category, amount and the balance after the transaction.
- At the bottom, the status bar: the number of transactions, the filters in use, and whether the file is **Saved** or still has **Unsaved changes**.

### Moving around the table

The most recent transactions are at the top. Click the **Date** heading to reverse the order; clicking another column heading sorts by that column. A marker separates the months.

Scroll with the mouse wheel or with the `Page Up` and `Page Down` keys. You can also drag the thumb of the scroll bar, on the right of the table: a bubble shows the month you have reached. Clicking in the bar, above or below the thumb, moves one page at a time.

`Ctrl+Home` goes to the first row of the list, `Ctrl+End` to the last one.

### Finding a command

All the menus are at the top of the window. If you cannot find a command, press `Ctrl+K`: the command palette opens. Type a few letters of what you are looking for (for example "scheduler" or "clear"), then `Enter`.

> This help opens at any time with the `F1` key or from the **Help** menu.

## Entering and editing a transaction

### Adding a transaction

Click the round **+** button at the bottom right of the table, or press `Ins`. The entry form opens on the right (or at the bottom of the window if you chose **View › Entry form at the bottom**).

1. **When and how**: the date and the payment method (card, check, direct debit…).
2. **Who and why**: the payee (the shop, the organization, the person) and an optional memo (reference, check number…).
3. **Classification**: the category, and the clearing state if needed.
4. The amount: choose **Expense** or **Income**, or simply type - or + before the amount.
5. Click **Add**, or press `Enter` in the amount, or `Ctrl+Enter`.

For the date, type just the day (15 gives the 15th of the current month) or the day and month (15/3). The up and down arrows move one day forward or back, and with `Ctrl` one month. The calendar button opens a calendar. Dates are always written DD/MM/YYYY.

While you type the payee, BankPerfect suggests the ones you have already used. `F10` copies the last transaction of that payee (method, category, amount). `F9` guesses the category.

The text cursor jumps straight to its place. If you prefer it to glide smoothly from one position to the next, tick **Animated text cursor** in **Tools › Options…**, **View** tab.

### Editing a transaction

Double-click the row, or select it and press `Enter`. Make your changes, then click **Save**. **Cancel** or `Esc` closes the form without changing anything.

### Deleting, copying, moving

- Select one or more rows, then press `Del`: BankPerfect asks for confirmation.
- To select several rows: `Ctrl`+click adds a row, `Shift`+click selects every row between two clicks.
- `Ctrl+X`, `Ctrl+C` and `Ctrl+V` cut, copy and paste rows, for example to move them to another account.
- Right-clicking a row opens a menu with the other actions: split the transaction over several categories, convert it into a transfer, add it to the scheduler…

> Made a mistake? `Ctrl+Z` undoes the last action, even a deletion.

## Clearing and reconciling

Clearing a transaction means ticking that it does appear on your bank statement. A transaction has three states:

- not cleared: no tick;
- cleared: one tick;
- reconciled: a double tick. It is a cleared transaction whose statement you have checked once and for all.

### Clearing

Click once in the clearing column, on the left of the date. Each click moves the row to the next state: not cleared, cleared, reconciled, then not cleared again. A double-click in this column counts as a single click.

You can also select one or more rows and press `Space` (or `P`), or choose **Transactions › Mark selection as cleared**. When several rows are selected, all those in the same state as the clicked row change together.

Removing the clearing of a reconciled row asks for confirmation: you cannot do it by accident.

### Reconciling

When every row of your statement is cleared and the **In bank** balance matches the bank's, choose **Transactions › Reconcile all cleared rows**: cleared rows become reconciled.

To protect them completely, tick **Transactions › Lock reconciled rows**: they can no longer be uncleared, edited or deleted while the lock is on.

### Going further

- **Transactions › Smart clearing…**: type the balance shown by your bank, click **Search**, and BankPerfect suggests the transactions to clear to reach that balance. Nothing is cleared without your agreement.
- **Transactions › Clearing date…** (`Ctrl+Shift+P`) changes the date on which a row was cleared; `F11` shows these dates under the transaction date.
- **View › Cleared column after amount** moves the clearing column between the amount and the balance.
- If you do not use reconciliation, tick **Simple clearing, without reconciliation (a line is cleared or not)** in **Tools › Options…**: a row is then simply cleared or not.

## Searching and filtering

Filtering means showing only part of the transactions: those of one month, one payee, one category… The others are not deleted, only hidden.

### Showing a period

Click the **Filters** button, at the top right of the window, then choose **Current month**, **Previous month**, **This year** or **Last year**.

For other dates, choose **Custom period…**: a row of filters opens under the column headings. Click the **Period** button, under **Date**, and type the **From** and **To** dates.

### Column filters

The magnifying glass, on the left of the **Date** heading, opens or closes the row of filters. Type in a column's field to keep only the rows that contain that text:

- several words: rows that contain all of these words;
- walmart|target: one or the other;
- /word: rows that do not contain this word;
- in **Amount**: <0 for expenses, >0 for income, 50<100 for amounts between 50 and 100.

Under the magnifying glass, a menu filters by clearing state: **Not cleared**, **Cleared**, **Reconciled**…

### When a filter is active

A colored banner appears at the top of the table: it tells how many transactions are shown out of the total, and which filters are in use. Click **Show all** to get all your transactions back. In this banner, each filter also has its own small cross to remove it alone.

By default, the balances at the top of the window then cover only the rows shown. To keep them computed on the whole account, tick **Balances computed on all lines, even when a filter is active** in **Tools › Options…**.

### Saved filters

A filter you use often can be kept: **Filters › Save current filter…**. It then appears in the **Filters** menu. **Manage filters…** (`F7`) lets you edit them.

> The 1M, 6M, 1Y, 5Y and All buttons, above the curve, only change the length of time shown by the curves. They do not filter the table.

## If transactions seem to have disappeared

Do not worry: BankPerfect never deletes a transaction without asking for confirmation. Most of the time, it is simply hidden. Check in this order.

### A filter is active

If a colored banner is shown at the top of the table, or if the status bar reads for example "11 / 1,635 transactions", a filter is hiding some rows. Click **Show all** in the banner, or choose **Filters › Clear filters**.

A filter can be set without you meaning to:

- clicking a week of the small timeline under **Pending** shows only the uncleared transactions of that week;
- dragging the mouse over the balance curve shows only the period covered.

In both cases, **Show all** brings every transaction back. Clicking the same week of the timeline again also removes its filter, and a single click on the curve removes the period.

### Unneeded rows are hidden

**View › Hide unneeded rows** (`Ctrl+H`) hides the reconciled transactions dated before the current month. This setting is kept from one session to the next: if you turned it on one day, it is still on the next time. Press `Ctrl+H` again to show everything.

### You are not on the right account

The name of the account on display is at the top of the accounts panel. `Ctrl+Tab` moves to the next account. A hidden account is in **Disabled accounts**, at the bottom of the account list.

### The transactions were archived

**Transactions › Archive transactions…** replaces a period with a few summary rows. Right-click a summary row: **View archived transactions…** or **Restore archived transactions**.

### They really were deleted

- Right after: `Ctrl+Z` (**Edit › Undo**) brings them back.
- Later: **File › Backups…** lists the copies made before each save. Choose a copy from before the mistake and click **Restore**. The current version is itself backed up before being replaced.

## Scheduler

The scheduler holds your transactions that come back regularly: salary, rent, subscriptions, direct debits… BankPerfect adds them to your accounts on the right date, and uses them to work out the balance to come.

Open it with **Accounts › Scheduler…** or `Ctrl+E`. You can also right-click a transaction in the table and choose **Add to scheduler…**: it is used as a model.

### Creating or editing a scheduled transaction

On the left, the list of scheduled transactions. Click one to edit it, or click **New scheduled transaction** to create one. On the right:

1. **Transaction**: the payee, the memo, the payment method, the category and the amount. The payment method tells whether it is an expense or income.
2. **Recurrence**: the **Next due date** (the small arrows move to the previous or next occurrence), the frequency (**Every** 1 month, 3 months, 1 year… set with the field and the - and + buttons), and the **Last due date** (leave it empty if there is no end). An occurrence that falls on a Saturday, a Sunday or a holiday can be moved to the next business day.
3. **Accounts**: the account concerned, and a destination account if it is a transfer between your accounts.

Tick **Insert automatically when the file is opened once the due date has passed** for the transaction to be added on its own.

Under **Upcoming dates**, click a date to change the amount of that occurrence only (a higher bill one month, for example).

### Saving

Your changes are saved as you go: there is no OK button. If you made a mistake, the **Undo changes** button, at the bottom of the window, puts the scheduler back as it was when you opened it. After closing the window, `Ctrl+Z` undoes them too.

### Inserting scheduled transactions

At the bottom of the window, **Insert scheduled transactions up to** adds to the accounts every occurrence due up to the chosen date. The button shows how many transactions will be added.

> **Forecast balance**, at the bottom of the window, shows how your balance will change with the upcoming scheduled transactions.

## Accounts

### Adding an account

Choose **Accounts › Manage accounts…** (`Ctrl+G`), stay on the **Accounts** tab, click **New**, type the account name and its opening balance, then **Create account**. In the same window, you can rename an account, enter its bank and bank details, change the order of the accounts with **Move up** and **Move down**, or delete one.

### Hiding old accounts

A closed account, or the account of a previous year, does not need to stay in front of you. Right-click that account in the left panel and choose **Hide this account**.

The account is not deleted: it moves to **Disabled accounts**, a folded section at the bottom of the account list, and it no longer counts in the total. Click **Disabled accounts** to see it again.

To show it in the list again, open **Accounts › Manage accounts…**, select it and untick **Inactive account: hidden from the account list**.

### Hiding the accounts panel

If you use only one account, or to give more room to the table, untick **View › Accounts panel**. The account name then appears above the balances: click it to choose another account from the list. Tick **View › Accounts panel** again to bring it back.

The panel also disappears by itself when the window is very narrow.

### Switching account with the keyboard

`Ctrl+Tab` shows the next account, `Ctrl+Shift+Tab` the previous one. These commands are also in the **Accounts** menu.

### Transfer between two accounts

**Transactions › Account-to-account transfer…** creates in one go the outgoing row on one account and the incoming row on the other. The two rows stay linked: right-click › **Go to linked row** moves from one to the other.

## Categories and payees

### Categories

Categories sort your expenses and income (Groceries, Housing, Salary…), with subcategories (Housing › Rent). They are used for totals, charts and the budget.

To change them: **Accounts › Categories…**. The **Category** and **Subcategory** buttons add one; select one to rename it, move it (**Move up**, **Move down**) or delete it.

### Automatic classification

- While you enter a transaction, BankPerfect fills in the category when it recognizes it from the payee; `F9` guesses it on request.
- **Transactions › Categorize transactions…** (`Ctrl+J`) suggests a category for every transaction that has none.
- **Transactions › Automatic rules…** (`Ctrl+F`) applies your own rules, for example: every payee containing "Comcast" goes to Housing › Internet.

### Saved payees

A saved payee is an entry template: when you pick it, the method, the category and even the amount fill in by themselves. Manage them with **Accounts › Saved payees…** (`F5`), or add one from a transaction: right-click › **Add to saved payees…**.

### Fixing several transactions at once

- **Edit › Replace…** (`Ctrl+R`) replaces one text with another in every transaction.
- Select several rows then **Transactions › Edit group…** (`Ctrl+M`) to give them, for example, the same category.

### Seeing where the money goes

The **Reports** menu offers **Totals by category** (`Ctrl+T`), totals by payee, **Charts…** and the **Monthly budget…** (`Ctrl+B`).

## Importing a bank statement

Rather than typing everything, you can fetch the transactions from your bank's website.

### Getting the file

On your bank's website, look for "Download", "Export" or "Statement" and choose the OFX format (the simplest), QIF or CSV. Save the file on your computer.

### Importing

1. Choose **File › Import transactions…** (`Ctrl+I`) and open the downloaded file.
2. Check the destination account in **To account**.
3. Look through the list. Each transaction is marked **New**, **Already imported** or **Probable duplicate**. Duplicates are not ticked: this way BankPerfect avoids adding the same transaction twice. You can tick or untick each row, or use **Check all** and **Uncheck all**.
4. Choose the state of the imported transactions: **Not cleared**, **Cleared** or **Reconciled**.
5. Click **Import**.

**Guess categories** classifies the new transactions from the ones you already have. **Create missing categories** adds the categories read in the file that do not exist yet.

If the file gives your bank balance, the window compares it with the **In bank after import** balance and shows **Matches** or the difference.

### CSV files

A CSV file is a plain table. BankPerfect tries to read it on its own; otherwise, set the separator and the date order in **Format**, then, in **Columns**, what each column contains: the date, the payee, the amount (or Debit and Credit)… The preview updates at once.

> Imported transactions can be undone right away with `Ctrl+Z`.

## Backups, undo, password

### Saving

Press `Ctrl+S` or choose **File › Save**. The status bar shows **Saved**, or **Unsaved changes** if something is left to save. When you close BankPerfect, it offers to save; the **Save automatically on closing, without asking** option (**Tools › Options…**) does it without asking.

### Undoing a mistake

`Ctrl+Z` (**Edit › Undo**) undoes the last action: entry, deletion, clearing, import… Press it several times to go further back. `Ctrl+Y` redoes what was undone. The number of actions that can be undone is set in **Tools › Options…** (**Undo levels**).

### Backups

Before each save, BankPerfect keeps a copy of the previous version of your file. **File › Backups…** lists them with their date and time. Choose one and click **Restore**: the current version is itself backed up before being replaced, so nothing is lost.

The number of copies kept is set in **Tools › Options…** (**Backups kept per file**).

> Remember also to copy your .bp file to a USB stick or an external drive from time to time.

### Protecting the file with a password

**File › File protection…**: type a password, confirm it, then click **Protect**. The file is then encrypted: without the password, nobody can read it.

BankPerfect then shows a 24-word recovery phrase. Write it down on paper and keep it safe: it is the only way to open the file if you forget the password.

The same window lets you **Change password**, create a **New recovery phrase** or remove the protection. As long as the file is not protected, the accounts panel says so at the bottom: **Unprotected file**.

## Keyboard shortcuts

### The most useful

- `F1`: this help
- `Ctrl+K`: command palette, to find a command by its name
- `Ctrl+S`: save
- `Ctrl+Z`: undo the last action; `Ctrl+Y`: redo it
- `Ins`: new transaction
- `Enter`: edit the selected transaction
- `Del`: delete the selected transactions
- `Space` or `P`: clear the selected transactions
- `Ctrl+E`: scheduler
- `Ctrl+H`: hide or show unneeded rows

### In the table

- Up and down arrows: previous or next row
- `Page Up` and `Page Down`: one page up or down
- `Home` and `End`: top or bottom of the screen; `Ctrl+Home` and `Ctrl+End`: first or last row
- `Shift` with these keys: extend the selection
- `Ctrl+X`, `Ctrl+C`, `Ctrl+V`: cut, copy, paste rows
- `Ctrl+Tab` and `Ctrl+Shift+Tab`: next or previous account
- `F8`: go to the entry form

### In the entry form

- `Ctrl+Enter`: save the transaction
- `Esc`: close without saving
- `F9`: guess the category
- `F10`: copy the last transaction of the payee typed
- In the date: up and down arrows for one day more or less, with `Ctrl` for one month

### File

- `Ctrl+N`: new accounts file
- `Ctrl+O`: open a file
- `F12`: save as
- `Ctrl+W`: close the file
- `Ctrl+I`: import transactions
- `Ctrl+P`: print statement

### Transactions and tools

- `Ctrl+Shift+P`: clearing date; `F11`: show clearing dates
- `F6`: split the transaction; `F3`: extra fields
- `Ctrl+Shift+T`: account-to-account transfer
- `Ctrl+R`: replace; `Ctrl+M`: edit group
- `Ctrl+F`: automatic rules; `Ctrl+J`: categorize transactions
- `F7`: predefined filters
- `Ctrl+G`: manage accounts; `F5`: saved payees
- `Ctrl+T`: totals by category; `Ctrl+B`: monthly budget; `Ctrl+Shift+G`: charts
- `Ctrl+L`: loan calculator; `Ctrl+D`: currency converter
- `Ctrl+Shift+C`: calculator; `Ctrl+Shift+N`: notes
- `Ctrl+,`: options

## Coming from version 8

### Opening your old file

Choose **File › Open a file…** (`Ctrl+O`) and select your usual .bp file, or drag it into the window. Everything is kept: accounts, transactions, clearing, categories, saved payees, scheduler, budget.

On the first save, the file moves to the new, safer format. Your original file is not lost: a copy is kept next to it, under the name "name.version8.bp". Version 8 can still open it if you want to go back.

If your file had a password, it is kept. BankPerfect then shows a 24-word recovery phrase: write it down on paper.

If your file contains scheduled transactions from an even older scheduler, the scheduler window says so and offers to **Recover them**.

### What does not change

- Clearing works the same way: not cleared, cleared, reconciled, with the `Space` key.
- **Hide unneeded rows** (`Ctrl+H`) still exists and is remembered from one time to the next.
- Transactions are entered with the same fields: date, method, payee, memo, category, amount.

### What changes

- The balances are shown at the top of the window: **In bank**, **Pending**, **Final balance**.
- Entry is done in a form on the right; would you rather have a bar at the bottom as before? Tick **View › Entry form at the bottom**.
- To search, use the **Filters** button at the top right, or the magnifying glass on the left of the **Date** heading.
- The old scripts are now built into the menus (**Reports**, **Tools**, **Transactions**). The old plugins no longer work; they are replaced by the **Extensions** menu.
- Cannot find a command? `Ctrl+K`, then type its name.

> Light or dark mode, themes and text size are set in the **View** menu and in **Tools › Options…**.
