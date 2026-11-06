#!/bin/sh
# Builds the WebAssembly module and checks that it gives, byte for byte, what
# the engine gives natively (engine/examples/slice.rs) on the reference
# configs, and parses (gse --parse) the same on every config and
# malformed fixture. Run from the repository's root.
set -eu
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
(cd wasm && cargo build --release --target wasm32-unknown-unknown)
(cd engine && cargo build --release --example slice)
wasm=wasm/target/wasm32-unknown-unknown/release/gse_wasm.wasm
out=$(mktemp -d)
for args in "team-looped.txt 150 1 0 100" "hutao-solo-looped.txt 150 7 40 60" "team-onepass.txt 0 1 0 100" "team-site.txt 0 1 0 100"; do
  set -- $args
  engine/target/release/examples/slice "configs/$1" "$2" "$3" "$4" "$5" > "$out/native.json"
  node wasm/run.mjs "$wasm" "configs/$1" "$2" "$3" "$4" "$5" > "$out/wasm.json" 2>/dev/null
  cmp "$out/native.json" "$out/wasm.json"
  echo "$1: identical"
done
(cd engine && cargo build --release --bin gse)
engine/target/release/gse --catalog > "$out/native.json"
node wasm/run.mjs "$wasm" --catalog > "$out/wasm.json"
cmp "$out/native.json" "$out/wasm.json"
echo "catalog: identical"
files=$(ls configs/*.txt reference/parsecheck/malformed/*.txt)
for f in $files; do
  st=0
  engine/target/release/gse --parse "$f" > "$out/one.json" || st=1
  printf '== %s %s\n' "$f" "$st"
  cat "$out/one.json"
done > "$out/native.json"
node wasm/run.mjs "$wasm" --parse-many $files > "$out/wasm.json"
cmp "$out/native.json" "$out/wasm.json"
echo "parse: identical ($(echo $files | wc -w) configs)"
ls -l "$wasm"
