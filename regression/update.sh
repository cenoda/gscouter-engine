#!/usr/bin/env bash
# Regenerates the regression's inputs or baseline (docs/regression.md).
#
#   regression/update.sh inputs      the configs, from compare -dry-run: needs Go
#                                    and Gscouter at the commit compare.yml pins
#                                    ($GSCOUTER, default ../Gscouter)
#   regression/update.sh baseline    the expected outputs: the engine alone
#
# `baseline` prints the reviewed diff (which builds moved, by how much) and
# keeps it in regression/target/baseline-diff.md for the commit message.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
reg="$root/regression"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pin=$(sed -n 's/^  GSCOUTER_COMMIT: *//p' "$root/.github/workflows/compare.yml")

build() {
	(cd "$root/engine" && cargo build --release -q)
	(cd "$reg" && cargo build --release -q)
}

case "${1:-}" in
inputs)
	gs=$(cd "${GSCOUTER:-$root/../Gscouter}" && pwd)
	head=$(git -C "$gs" rev-parse HEAD)
	[ "$head" = "$pin" ] || { echo "Gscouter at $gs is $head, compare.yml pins $pin" >&2; exit 1; }
	build
	mkdir -p "$tmp/inputs/configs"
	(cd "$root/reference" && go run ./compare -gscouter "$gs" -gse "$root/engine/target/release/gse" -only library,probes -dry-run -configs "$tmp/inputs/configs" -cache "" > "$tmp/dry.log" 2>&1) || { tail -20 "$tmp/dry.log"; exit 1; }
	echo "$pin" > "$tmp/inputs/gscouter-commit"
	tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner -C "$tmp/inputs" -cf - gscouter-commit configs | xz -9e > "$reg/inputs.tar.xz"
	echo "regression/inputs.tar.xz: $(ls "$tmp/inputs/configs" | wc -l) configs, $(wc -c < "$reg/inputs.tar.xz") bytes; now run: regression/update.sh baseline"
	;;
baseline)
	build
	tar -xJf "$reg/inputs.tar.xz" -C "$tmp"
	iter=$(sed -n 's/^# iterations //p' "$reg/baseline.txt" 2>/dev/null || true)
	engine=$(git -C "$root" rev-parse HEAD)
	if [ -n "$(git -C "$root" status --porcelain -- engine)" ]; then engine="$engine+changes"; fi
	"$reg/target/release/regress" run --gse "$root/engine/target/release/gse" --configs "$tmp/configs" --iter "${ITER:-${iter:-50}}" \
		--engine "$engine" --gscouter "$(cat "$tmp/gscouter-commit")" -o "$tmp/baseline.txt"
	if [ -f "$reg/baseline.txt" ]; then
		"$reg/target/release/regress" diff "$reg/baseline.txt" "$tmp/baseline.txt" | tee "$reg/target/baseline-diff.md" || true
	fi
	cp "$tmp/baseline.txt" "$reg/baseline.txt"
	echo "regression/baseline.txt written; review the diff above before committing it"
	;;
*)
	sed -n '2,11p' "$0" >&2
	exit 2
	;;
esac
