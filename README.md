# BankPerfect

Gestion de comptes bancaires personnels, rapide et moderne, pour Linux, Windows et macOS.

BankPerfect 9 est la réécriture complète en Rust du logiciel BankPerfect (Delphi), avec la même richesse fonctionnelle, une interface repensée et des fichiers chiffrés. Il ouvre directement les fichiers `.bp` des anciennes versions.

![Tableau de bord](docs/captures/tableau-de-bord.png)

## Fonctionnalités

- **Comptes et opérations** : saisie rapide avec suggestions, modes de paiement en couleur, pointage et rapprochement (avec dates de pointage), ventilation sur plusieurs catégories, transferts liés entre comptes, recherche et filtres instantanés, annulation et rétablissement.
- **Imports** : relevés OFX, QIF et CSV, avec détection des doublons et règles de catégorisation automatique.
- **Échéancier** : opérations récurrentes insérées automatiquement, solde prévisionnel.
- **Rapports et graphiques** : totaux par catégorie et par tiers, relevé multicompte, graphiques en anneau, barres et courbes, export vers un tableur (`.xlsx`).
- **Budgets** : budget mensuel par catégorie et tableaux de budget annuel.
- **Outils** : simulateur de crédit, convertisseur de devises (cours de la BCE), cotations des titres, calculatrice, notes, palette de commandes (Ctrl+K).
- **Sécurité** : fichiers chiffrés (XChaCha20-Poly1305, mot de passe dérivé par Argon2id) avec clé de secours de 24 mots, sauvegardes automatiques.

| Saisie | Graphiques |
| --- | --- |
| ![Saisie](docs/captures/saisie.png) | ![Graphiques](docs/captures/graphiques.png) |
| **Budget** | **Simulateur de crédit** |
| ![Budget](docs/captures/budget.png) | ![Simulateur de crédit](docs/captures/simulateur-credit.png) |

## Installation

Les binaires sont publiés sur la page [Releases](https://github.com/fabiochelly/bankperfect/releases).

| Système | Commande |
| --- | --- |
| Windows | `winget install FabioChelly.BankPerfect` |
| macOS | `brew install --cask fabiochelly/bankperfect/bankperfect` |
| Arch Linux | `yay -S bankperfect-bin` (AUR) |
| Autres Linux | archive `linux-x86_64.tar.gz` de la dernière release |

Les binaires ne sont pas signés : Windows peut afficher un avertissement SmartScreen (« Informations complémentaires », puis « Exécuter quand même »). Sous macOS, l'installation par Homebrew retire la mise en quarantaine ; avec l'archive, faites un clic droit sur l'application puis « Ouvrir » la première fois.

## Compilation

Prérequis : [Rust](https://rustup.rs) stable (édition 2024). Sous Linux, les en-têtes `libxkbcommon` et `wayland` sont nécessaires.

```sh
cargo run --release -p bankperfect
cargo test --workspace
```

Le dépôt contient deux crates :

- `crates/bp-core` : formats de fichiers (lecture des anciens `.bp`, nouveau format chiffré), modèle de données et calculs ;
- `crates/bp-app` : l'application (egui, rendu wgpu).

`docs/inventaire/` décrit l'ancienne version fonctionnalité par fonctionnalité, et `docs/corrections.md` liste les bogues de l'ancienne version corrigés dans la réécriture.

## Licence

[MIT](LICENSE) © Fabio Chelly
