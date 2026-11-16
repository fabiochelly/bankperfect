#!/usr/bin/env bash
# Compile le cœur Rust pour Android et génère son interface Kotlin.
# Usage : build-rust.sh <dossier de sortie>
set -euo pipefail

out=$1
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-${ANDROID_HOME:?ANDROID_HOME manquant}/ndk/30.0.16248370}
export PATH=$HOME/.cargo/bin:$PATH

cargo ndk -t arm64-v8a -t x86_64 --platform 26 -o "$out/jniLibs" \
    build --release -p bp-mobile

# La bibliothèque Android de production n'a plus ses symboles : l'interface se
# lit dans celle de la machine de compilation, identique par construction.
cargo build -p bp-mobile --features bindgen --bin uniffi-bindgen
cargo build -p bp-mobile
lib=target/debug/libbp_mobile.so
[ -f "$lib" ] || lib=target/debug/libbp_mobile.dylib
target/debug/uniffi-bindgen generate --library "$lib" --language kotlin \
    --out-dir "$out/uniffi" --no-format
