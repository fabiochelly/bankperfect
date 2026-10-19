[Français](PRIVACY.md) · **English**

# Privacy

BankPerfect is software installed on your computer. It has no user
accounts, no server that receives your data and no analytics.

## Your data stays with you

Your accounts, your transactions and your settings are stored in your
files, on your computer. Nobody else has access to them, not even the
author of the software.

## Bank sync (Enable Banking)

Bank sync goes through [Enable Banking](https://enablebanking.com), a
provider licensed under the European PSD2 directive:

- you create your own Enable Banking application. Its private key stays
  in the BankPerfect configuration folder, on your computer;
- you sign in on your bank's website. Your bank credentials never go
  through BankPerfect;
- BankPerfect calls the Enable Banking API directly from your computer,
  with no intermediary, and stores transactions only in your accounts
  file.

The processing of your data by Enable Banking is governed by its own
[privacy policy](https://enablebanking.com/privacy-policy).

## Other connections

BankPerfect also connects to the Internet in the following cases:

- **Exchange rates**: downloaded from the European Central Bank. The
  request contains nothing about you.
- **Security quotes**: requested from Yahoo Finance for the tickers of the
  securities you follow.
- **License**: if you bought BankPerfect, your license key is sent once to
  the chelly.net server, which returns a signed certificate. After that,
  the check is done offline.
- **AI categorization** (off by default): auto-categorization works
  offline, based on your history and a built-in dictionary. If you turn
  on the option and enter your own Anthropic API key, the “Ask AI” button
  in the “Categorize transactions” window sends to Anthropic, only at that
  moment, the descriptions that BankPerfect cannot classify: cleaned up
  (without numbers, references or dates), with the direction and the rough
  size of the amount, and the list of your category names. Neither your
  accounts, nor your balances, nor exact amounts are sent. The processing
  by Anthropic is governed by its own
  [privacy policy](https://www.anthropic.com/legal/privacy).

## Contact

Fabio Chelly — fabio@chelly.net
