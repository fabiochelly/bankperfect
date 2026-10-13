#!/usr/bin/env sh
# Lance la version de développement de BankPerfect : recompile si les sources
# ont changé (« cargo build --release »), puis lance target/release/bankperfect.
# Équivaut à « cargo run --release -p bankperfect -- <fichier> », utilisable
# depuis le menu des applications.
#
#   bankperfect-dev.sh [fichier.bp]   compile si besoin et lance
#   bankperfect-dev.sh --install      remplace ~/.local/bin/bankperfect par un
#                                     lien vers ce script (le lanceur du menu
#                                     suit alors toujours les sources)
set -eu

script="$(readlink -f "$0")"
root="$(cd "$(dirname "$script")/../.." && pwd)"

if [ "${1:-}" = "--install" ]; then
  bin="$HOME/.local/bin"
  data="${XDG_DATA_HOME:-$HOME/.local/share}"
  mkdir -p "$bin" "$data/applications"
  ln -sfn "$script" "$bin/bankperfect"
  sed "s|^Exec=bankperfect|Exec=$bin/bankperfect|" "$root/packaging/linux/bankperfect.desktop" \
    > "$data/applications/bankperfect.desktop"
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$data/applications" || true
  echo "Lanceur de développement installé : $bin/bankperfect -> $script"
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
sleep 1
if kill -0 "$build" 2>/dev/null; then
  notify "BankPerfect" "Compilation de la dernière version…"
fi

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

exec "$binary" "$@"
