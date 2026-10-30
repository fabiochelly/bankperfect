**Français** · [English](README.en.md)

# BankPerfect

Gestion de comptes bancaires personnels, rapide et moderne, pour Linux, Windows et macOS.

BankPerfect 9 est la réécriture complète en Rust du logiciel BankPerfect (Delphi), avec la même richesse fonctionnelle, une interface repensée et des fichiers chiffrés. Il ouvre directement les fichiers `.bp` des anciennes versions.

![Tableau de bord](docs/captures/tableau-de-bord.png)

## Fonctionnalités

- **Comptes et opérations** : saisie rapide avec suggestions, modes de paiement en couleur, pointage et rapprochement (avec dates de pointage), pointage intelligent d'après le solde en banque (lignes déjà passées retrouvées en tenant compte des délais des cartes et des chèques), ventilation sur plusieurs catégories, transferts liés entre comptes, recherche et filtres instantanés, annulation et rétablissement.
- **Imports** : relevés OFX, QIF et CSV, avec détection des doublons et règles de catégorisation automatique.
- **Catégorisation automatique**, hors connexion : d'après vos opérations passées (même tiers, montant proche) puis un dictionnaire intégré d'plus de 2 000 enseignes et mots génériques (boulangerie, pharmacie, péage…) et les codes commerçant de la synchronisation. Catégories devinées à l'import, pendant la saisie (baguette magique, F9) ou en lot pour tout le fichier ou les lignes filtrées (Ctrl+J) ; recours facultatif à l'IA (votre clé d'API Anthropic) pour les libellés inconnus.
- **Synchronisation bancaire** (expérimentale) : import direct des opérations par [Enable Banking](https://enablebanking.com), prestataire agréé DSP2, avec votre propre application Enable Banking (offre gratuite pour un usage personnel) ; authentification sur le site de la banque, identifiants jamais transmis à BankPerfect.
- **Échéancier** : opérations récurrentes insérées automatiquement, solde prévisionnel.
- **Rapports et graphiques** : totaux par catégorie et par tiers, relevé multicompte, graphiques en anneau, barres et courbes, export vers un tableur (`.xlsx`).
- **Budgets** : budget mensuel par catégorie et tableaux de budget annuel.
- **Outils** : simulateur de crédit, convertisseur de devises (cours de la BCE), cotations des titres, calculatrice, notes, palette de commandes (Ctrl+K).
- **Extensions** : scripts [Luau](https://luau.org) en bac à sable qui ajoutent commandes (dans les menus de leur choix et au clic droit), raccourcis, fenêtres natives (formulaires, tableaux, statistiques, graphiques), formats d'import de relevés et automatismes sur les opérations, l'échéancier, le budget et les règles, avec annulation intégrée ; six exemples fournis. Voir la [documentation des extensions](docs/extensions.md).
- **Thèmes** : 19 thèmes sombres et clairs (Tokyo Night, Catppuccin, Dracula, Nord, Gruvbox, Rosé Pine, One Dark, Everforest, Kanagawa, Solarized…), dont « BankPerfect classic » aux couleurs de l'ancienne version, avec un mode qui suit le système.
- **Sécurité** : fichiers chiffrés (XChaCha20-Poly1305, mot de passe dérivé par Argon2id) avec clé de secours de 24 mots, sauvegardes automatiques.

| Saisie | Graphiques |
| --- | --- |
| ![Saisie](docs/captures/saisie.png) | ![Graphiques](docs/captures/graphiques.png) |
| **Budget** | **Simulateur de crédit** |
| ![Budget](docs/captures/budget.png) | ![Simulateur de crédit](docs/captures/simulateur-credit.png) |
| **Catégories devinées à l'import** | **Baguette magique de la saisie** |
| ![Catégorisation à l'import](docs/captures/categorisation-import.png) | ![Catégorie devinée pendant la saisie](docs/captures/categorisation-saisie.png) |

## Installation

Les binaires sont publiés sur la page [Releases](https://github.com/fabiochelly/bankperfect/releases).

| Système | Commande |
| --- | --- |
| Windows | archive `windows-x86_64.zip` de la dernière release, à décompresser (paquet winget en cours de validation) |
| macOS | `brew install --cask fabiochelly/bankperfect/bankperfect` |
| Linux | archive `linux-x86_64.tar.gz` de la dernière release, puis `./install.sh` (paquet AUR à venir) |

Sous Linux, `install.sh` installe BankPerfect pour l'utilisateur (dans `~/.local`) avec son entrée dans le menu des applications, ses icônes et l'ouverture des fichiers `.bp` ; `./install.sh --uninstall` le retire. Depuis le dépôt, après `cargo build --release`, lancez `packaging/linux/install.sh`.

### Windows : si BankPerfect ne s'ouvre pas

BankPerfect 9 est encore trop récent pour être certifié par Microsoft : son exécutable n'est pas signé par un certificat que Windows reconnaît. Sous Windows 11, le Contrôle intelligent des applications (Smart App Control) peut alors bloquer son ouverture, parfois sans aucun message, même en mode administrateur.

- Si Windows affiche « Windows a protégé votre ordinateur », cliquez sur « Informations complémentaires » puis sur « Exécuter quand même ».
- Si rien ne s'ouvre, ouvrez **Sécurité Windows** > **Contrôle des applications et du navigateur** > **Paramètres du Contrôle intelligent des applications**, puis choisissez **Désactivé**.

Cette protection s'applique à toutes les applications, pas seulement à BankPerfect. Sur un Windows 11 à jour (24H2 ou 25H2, depuis avril 2026), elle se réactive au même endroit ; sur une version plus ancienne, la réactiver demande de réinstaller Windows.

### macOS : si BankPerfect ne s'ouvre pas

BankPerfect n'est pas notarié par Apple : au premier lancement depuis l'archive, macOS affiche « BankPerfect ne peut pas être ouvert car le développeur ne peut pas être vérifié ». L'installation par Homebrew évite ce message.

- **macOS 12 à 14** : faites un clic droit (ou Ctrl + clic) sur l'application, choisissez « Ouvrir », puis « Ouvrir » dans la fenêtre qui suit. Ou bien, après le message, ouvrez **Préférences Système** (**Réglages Système** sous macOS 13 et 14) > **Sécurité et confidentialité** (**Confidentialité et sécurité**) et cliquez sur **Ouvrir quand même**.
- **macOS 15 et plus** : le clic droit ne suffit plus. Après le message, ouvrez **Réglages Système** > **Confidentialité et sécurité**, cliquez sur **Ouvrir quand même** en bas de la page et confirmez.
- **Si macOS indique que l'application est « endommagée »** : placez-la dans le dossier Applications, puis tapez dans le Terminal `xattr -dr com.apple.quarantine /Applications/BankPerfect.app`.

## Compilation

Prérequis : [Rust](https://rustup.rs) stable (édition 2024). Sous Linux, les en-têtes `libxkbcommon` et `wayland` sont nécessaires.

```sh
cargo run --release -p bankperfect
cargo test --workspace
```

Le dépôt contient trois crates :

- `crates/bp-core` : formats de fichiers (lecture des anciens `.bp`, nouveau format chiffré), modèle de données, calculs et traductions ;
- `crates/bp-ext` : les extensions Luau (moteur, API, fenêtres déclaratives) ;
- `crates/bp-app` : l'application (egui, rendu wgpu).

`docs/inventaire/` décrit l'ancienne version fonctionnalité par fonctionnalité, et `docs/corrections.md` liste les bogues de l'ancienne version corrigés dans la réécriture.

## Licence

[MIT](LICENSE) © Fabio Chelly
