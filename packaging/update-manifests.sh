#!/usr/bin/env bash
# Met à jour les manifestes (Homebrew, AUR, winget) d'après une release
# publiée : ./packaging/update-manifests.sh 9.0.0
set -euo pipefail
version="${1:?usage : $0 VERSION}"
root="$(cd "$(dirname "$0")/.." && pwd)"
sums="$(curl -fsSL "https://github.com/fabiochelly/bankperfect/releases/download/v${version}/SHA256SUMS.txt")"
sum_of() { awk -v f="$1" '$2 == f { print $1 }' <<<"$sums"; }
linux="$(sum_of "bankperfect-${version}-linux-x86_64.tar.gz")"
windows="$(sum_of "bankperfect-${version}-windows-x86_64.zip")"
macos="$(sum_of "BankPerfect-${version}-macos-universal.zip")"
[[ -n "$linux" && -n "$windows" && -n "$macos" ]] || { echo "sommes introuvables pour $version" >&2; exit 1; }

sed -i -E "s/^  version \".*\"/  version \"${version}\"/; s/^  sha256 \".*\"/  sha256 \"${macos}\"/" "$root/Casks/bankperfect.rb"
sed -i -E "s/^pkgver=.*/pkgver=${version}/; s/^pkgrel=.*/pkgrel=1/; s/^sha256sums=.*/sha256sums=('${linux}')/" "$root/packaging/aur/PKGBUILD"
for f in "$root"/packaging/winget/*.yaml; do
  sed -i -E "s/^PackageVersion: .*/PackageVersion: ${version}/; s#download/v[^/]+/bankperfect-[^/]+-windows#download/v${version}/bankperfect-${version}-windows#; s/InstallerSha256: .*/InstallerSha256: '${windows}'/; s#releases/tag/v[^ ]+#releases/tag/v${version}#" "$f"
done
echo "Manifestes mis à jour pour ${version}."
echo "AUR : cd packaging/aur && makepkg --printsrcinfo > .SRCINFO, puis pousser vers aur.archlinux.org/bankperfect-bin.git"
echo "winget : wingetcreate submit packaging/winget (ou PR vers microsoft/winget-pkgs)"
