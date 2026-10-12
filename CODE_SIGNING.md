# Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

L'exécutable Windows (`bankperfect.exe`) publié sur la page [Releases](https://github.com/fabiochelly/bankperfect/releases) est signé par SignPath Foundation. Il est construit automatiquement par GitHub Actions ([`release.yml`](.github/workflows/release.yml)) à partir des sources de ce dépôt, sans binaire tiers signé ; chaque release est approuvée manuellement avant signature.

## Rôles

| Rôle | Membres |
| --- | --- |
| Auteurs (committers) | [Fabio Chelly](https://github.com/fabiochelly) |
| Relecteurs (reviewers) | [Fabio Chelly](https://github.com/fabiochelly) |
| Approbateurs (approvers) | [Fabio Chelly](https://github.com/fabiochelly) |

Les contributions extérieures (pull requests) sont relues par un relecteur avant d'être intégrées. Les comptes GitHub et SignPath des membres sont protégés par une authentification à deux facteurs.

## Confidentialité

BankPerfect ne transmet aucune information à d'autres systèmes en réseau, sauf à la demande explicite de l'utilisateur :

- **Cours des devises** : le bouton « Mettre à jour les cours » télécharge le flux public de la Banque centrale européenne (`www.ecb.europa.eu`) ; aucune donnée n'est envoyée.
- **Cotations** : la mise à jour des cours d'un portefeuille envoie à Yahoo Finance (`query1.finance.yahoo.com`) les seuls identifiants des titres (ISIN ou symbole).
- **Synchronisation bancaire** (expérimentale) : elle ne fonctionne qu'avec l'application Enable Banking que l'utilisateur a lui-même créée et configurée ; BankPerfect échange alors avec `api.enablebanking.com` les données nécessaires à la connexion aux banques choisies et à l'import des opérations.

Les fichiers de comptes restent sur l'ordinateur de l'utilisateur.
