#!/usr/bin/env sh
# Installe BankPerfect pour l'utilisateur courant (sans droits administrateur) :
# binaire, entrée de menu (.desktop), icônes et association des fichiers .bp.
#
#   ./install.sh               installe dans ~/.local
#   PREFIX=/usr/local sudo -E ./install.sh   installation pour tout le système
#   ./install.sh --uninstall   désinstalle
#
# Fonctionne depuis l'archive de la release (fichiers à côté du script) ou
# depuis le dépôt (après « cargo build --release »).
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
if [ "$prefix" = "$HOME/.local" ]; then
  data="${XDG_DATA_HOME:-$HOME/.local/share}"
else
  data="$prefix/share"
fi
bin="$prefix/bin"

# Emplacement des fichiers : archive ou dépôt.
if [ -f "$here/bankperfect" ]; then
  binary="$here/bankperfect"
  desktop="$here/bankperfect.desktop"
  mime="$here/bankperfect-mime.xml"
  icons="$here/icons"
else
  root="$(cd "$here/../.." && pwd)"
  binary="$root/target/release/bankperfect"
  desktop="$here/bankperfect.desktop"
  mime="$here/bankperfect-mime.xml"
  icons="$root/crates/bp-app/assets/icon"
fi

refresh() {
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$data/applications" || true
  command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$data/mime" >/dev/null 2>&1 || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$bin/bankperfect" "$data/applications/bankperfect.desktop" "$data/mime/packages/bankperfect.xml"
  for s in 16 32 48 64 128 256 512; do
    rm -f "$data/icons/hicolor/${s}x${s}/apps/bankperfect.png"
  done
  rm -f "$data/icons/hicolor/scalable/apps/bankperfect.svg"
  refresh
  echo "BankPerfect désinstallé (vos fichiers de comptes et réglages sont conservés)."
  exit 0
fi

if [ ! -x "$binary" ]; then
  echo "Binaire introuvable : $binary" >&2
  echo "Depuis le dépôt, lancez d'abord : cargo build --release -p bankperfect" >&2
  exit 1
fi

install -Dm755 "$binary" "$bin/bankperfect"
# Chemin absolu dans le lanceur : ~/.local/bin n'est pas toujours dans le PATH
# des sessions graphiques.
mkdir -p "$data/applications"
sed "s|^Exec=bankperfect|Exec=$bin/bankperfect|" "$desktop" > "$data/applications/bankperfect.desktop"
chmod 644 "$data/applications/bankperfect.desktop"
install -Dm644 "$mime" "$data/mime/packages/bankperfect.xml"
for s in 16 32 48 64 128 256 512; do
  install -Dm644 "$icons/bankperfect-$s.png" "$data/icons/hicolor/${s}x${s}/apps/bankperfect.png"
done
install -Dm644 "$icons/bankperfect.svg" "$data/icons/hicolor/scalable/apps/bankperfect.svg"
refresh

echo "BankPerfect installé : $bin/bankperfect"
echo "Il apparaît dans le menu des applications et ouvre les fichiers .bp."
