#!/usr/bin/env bash
# Crée la clé qui signe l'APK, hors du dépôt. À ne faire qu'une fois : sans cette
# clé, une mise à jour impose de désinstaller l'application.
set -euo pipefail

dir=$HOME/.config/bankperfect
store=$dir/android-release.jks
props=$dir/android-signing.properties
[ -e "$store" ] && { echo "$store existe déjà"; exit 0; }

mkdir -p "$dir"
chmod 700 "$dir"
password=$(head -c 24 /dev/urandom | base64 | tr -d '/+=')
keytool -genkeypair -keystore "$store" -storepass "$password" -keypass "$password" \
    -alias bankperfect -keyalg RSA -keysize 4096 -validity 36500 \
    -dname "CN=BankPerfect" -storetype PKCS12
umask 077
cat > "$props" <<PROPS
storeFile=$store
password=$password
keyAlias=bankperfect
PROPS
chmod 600 "$store"
echo "Clé créée : sauvegarde $store et $props"
