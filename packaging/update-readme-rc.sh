#!/usr/bin/env bash
# Met à jour l'encart « dernière version de test » des README (liens directs
# vers les binaires d'une pré-release) : ./packaging/update-readme-rc.sh 9.13.0-rc.16
# Lancé par release.yml à chaque tag de pré-release.
set -euo pipefail
version="${1:?usage : $0 VERSION}"
root="$(cd "$(dirname "$0")/.." && pwd)"
base="https://github.com/fabiochelly/bankperfect/releases"
dl="$base/download/v${version}"
links="[Windows](${dl}/bankperfect-${version}-windows-x86_64.zip) · [macOS](${dl}/BankPerfect-${version}-macos-universal.zip) · [Linux](${dl}/bankperfect-${version}-linux-x86_64.tar.gz)"
fr="> **Dernière version de test : [${version}](${base}/tag/v${version})**, à privilégier : elle contient les dernières corrections. Téléchargement direct : ${links}"
en="> **Latest test version: [${version}](${base}/tag/v${version})**, recommended: it has the latest fixes. Direct download: ${links}"
replace() {
  local file="$1" text="$2"
  TEXT="$text" perl -0pi -e 's/(<!-- derniere-rc:debut -->)\n?.*?\n?(<!-- derniere-rc:fin -->)/$1\n$ENV{TEXT}\n$2/s' "$file"
  grep -qF "$version" "$file" || { echo "encart introuvable dans $file" >&2; exit 1; }
}
replace "$root/README.md" "$fr"
replace "$root/README.en.md" "$en"
echo "README : dernière version de test ${version}."
