#!/usr/bin/env sh
# Lance la version de développement de BankPerfect : recompile si les sources
# ont changé (« cargo build --release »), puis lance target/release/bankperfect.
# Équivaut à « cargo run --release -p bankperfect -- <fichier> », utilisable
# depuis le menu des applications.
#
#   bankperfect-dev.sh [fichier.bp]   compile si besoin et lance
#   bankperfect-dev.sh --install      installe deux lanceurs dans le menu :
#                                     « BankPerfect » (~/.local/bin/bankperfect,
#                                     lien vers target/release/bankperfect, lancé
#                                     tel quel, sans recompilation) et
#                                     « BankPerfect-dev » (~/.local/bin/bankperfect-dev,
#                                     lien vers ce script : suit les sources)
set -eu

script="$(readlink -f "$0")"
root="$(cd "$(dirname "$script")/../.." && pwd)"

if [ "${1:-}" = "--install" ]; then
  bin="$HOME/.local/bin"
  data="${XDG_DATA_HOME:-$HOME/.local/share}"
  desktop="$root/packaging/linux/bankperfect.desktop"
  mkdir -p "$bin" "$data/applications"
  ln -sfn "$root/target/release/bankperfect" "$bin/bankperfect"
  ln -sfn "$script" "$bin/bankperfect-dev"
  sed "s|^Exec=bankperfect|Exec=$bin/bankperfect|" "$desktop" \
    > "$data/applications/bankperfect.desktop"
  # Version de développement : pas d'association de fichiers (le
  # gestionnaire par défaut des .bp reste « BankPerfect »).
  sed -e "s|^Exec=bankperfect|Exec=$bin/bankperfect-dev|" \
    -e "s|^Name=BankPerfect$|Name=BankPerfect-dev|" \
    -e "s|^Comment=.*|Comment=BankPerfect recompilé depuis les sources avant le lancement|" \
    -e "/^MimeType=/d" "$desktop" \
    > "$data/applications/bankperfect-dev.desktop"
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$data/applications" || true
  echo "Lanceurs installés :"
  echo "  BankPerfect     : $bin/bankperfect -> $root/target/release/bankperfect"
  echo "  BankPerfect-dev : $bin/bankperfect-dev -> $script"
  exit 0
fi

# Les sessions graphiques n'ont pas toujours ~/.cargo/bin dans le PATH.
PATH="$HOME/.cargo/bin:$PATH"
export PATH

log_dir="${XDG_STATE_HOME:-$HOME/.local/state}/bankperfect"
mkdir -p "$log_dir"
log="$log_dir/build.log"
binary="$root/target/release/bankperfect"

notify() {
  command -v notify-send >/dev/null 2>&1 && notify-send -a BankPerfect -i bankperfect "$@" || true
}

# Pas de cd : les chemins relatifs passés en argument doivent rester valides.
cargo build --release -p bankperfect --manifest-path "$root/Cargo.toml" >"$log" 2>&1 &
build=$!
# Notification seulement si la compilation dure (sinon rien n'a changé).
# Le délai court en arrière-plan : sans recompilation, le lancement n'attend
# que la vérification de cargo (~0,15 s), pas la seconde entière.
(
  sleep 1
  if kill -0 "$build" 2>/dev/null; then
    notify "BankPerfect" "Compilation de la dernière version…"
  fi
) &
watcher=$!

if ! wait "$build"; then
  errors="$(grep -m 5 -A 3 '^error' "$log" || tail -n 10 "$log")"
  if [ -x "$binary" ]; then
    notify -u critical "BankPerfect : échec de compilation" \
      "Lancement de la dernière version compilée. Détails : $log

$errors"
  else
    notify -u critical "BankPerfect : échec de compilation" "Détails : $log

$errors"
    exit 1
  fi
fi

kill "$watcher" 2>/dev/null || true
exec "$binary" "$@"
