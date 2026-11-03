#!/usr/bin/env bash
# warm-host.sh runs gcsim on every library team into compare's cache on a
# large machine outside CI (a cloud server's startup script), and uploads
# the cache as assets of a prerelease of this repository. A manual run of
# .github/workflows/compare.yml on main with import_release set to its tag
# adds them to main's cache, so that CI runs only the engine for them.
#
# gcsim's results do not depend on the engine, so this runs every team, the
# ones the engine does not support yet too: the library build, the variants
# CI samples (-sample 4, the same hash) and the probes, at compare's
# iterations (-iter). Build space and pages need the engine and stay in CI.
#
# Needs GH_TOKEN: a fine-grained token for cenoda/gscouter-engine only, with
# Contents read and write (to make the prerelease and upload its assets).
# Optional: DEADLINE (default 20h, compare's -deadline: past it the jobs
# start no more builds and the cache is uploaded as it is), TAG.
#
# Several servers share the work by slices: the teams are cut into SHARDS
# slices (compare's -shard k/SHARDS) and this server runs SLICES, a range
# such as 0-23 (default: all of them). Give each server its own range and
# TAG; import_release takes one tag per run. A server rerun on the same
# disk keeps its cache, so only builds not yet cached run again.
#
# The cache is uploaded in increments every UPLOAD_EVERY (default 2h), so
# CI can import what is done before the end.
#
# REVERSE=1 takes the teams from the end (compare -reverse): a second
# server started beside one already running forward meets it in the middle
# instead of running the same teams first.
#
# A rerun on a new server with the same TAG first downloads the increments
# already uploaded, so it does not run them again.
#
# Progress: the prerelease's progress.txt asset, replaced every 10 minutes.
# When it says DONE, the server can be deleted.
set -euo pipefail

: "${GH_TOKEN:?set GH_TOKEN to a token with Contents write on cenoda/gscouter-engine}"
REPO=cenoda/gscouter-engine
DEADLINE=${DEADLINE:-20h}
TAG=${TAG:-gcsim-cache-$(date -u +%Y%m%d-%H%M)}
WORK=${WORK:-/root/warm}
API=https://api.github.com/repos/$REPO
UPLOADS=https://uploads.github.com/repos/$REPO

# a startup script may run without HOME, and go needs it for its caches
export HOME=${HOME:-/root}
export GOPATH=${GOPATH:-$HOME/go}
export DEBIAN_FRONTEND=noninteractive
apt-get update -q && apt-get install -yq git curl jq tar gzip bc ca-certificates

mkdir -p "$WORK" && cd "$WORK"
[ -d engine ] || git clone -q https://github.com/$REPO engine
GSCOUTER_COMMIT=$(awk '/GSCOUTER_COMMIT:/{print $2; exit}' engine/.github/workflows/compare.yml)
[ -d gscouter ] || git clone -q https://github.com/cenoda/Gscouter gscouter
git -C gscouter checkout -q "$GSCOUTER_COMMIT"

GOVER=$(awk '/^go /{print $2; exit}' engine/reference/go.mod)
if ! command -v go >/dev/null || [ "$(go env GOVERSION)" != "go$GOVER" ]; then
	curl -sSfL "https://go.dev/dl/go$GOVER.linux-amd64.tar.gz" | tar -xz -C /usr/local
fi
export PATH=/usr/local/go/bin:$PATH GOFLAGS=-mod=mod

gh() { curl -sSf -H "Authorization: Bearer $GH_TOKEN" -H "Accept: application/vnd.github+json" "$@"; }

# the prerelease first, so that a bad token stops here, in minutes
# a rerun with the same TAG keeps its prerelease
REL=$(gh "$API/releases/tags/$TAG" 2>/dev/null | jq -r .id || true)
[ -n "$REL" ] && [ "$REL" != null ] || REL=$(gh -X POST "$API/releases" -d "$(jq -n --arg t "$TAG" --arg c "$(git -C engine rev-parse HEAD)" \
	'{tag_name:$t, target_commitish:$c, name:$t, prerelease:true, body:"gcsim cache made by reference/warm-host.sh (outside CI). Add it to main'"'"'s cache: run compare.yml on main with import_release set to this tag."}')" | jq -r .id)
upload() { # file, name: replaces the asset of that name
	local old
	old=$(gh "$API/releases/$REL/assets?per_page=100" | jq -r --arg n "$2" '.[] | select(.name==$n) | .id')
	[ -z "$old" ] || gh -X DELETE "$API/releases/assets/$old" >/dev/null
	gh -X POST -H "Content-Type: application/octet-stream" --data-binary @"$1" "$UPLOADS/releases/$REL/assets?name=$2" >/dev/null
}

CACHE=$WORK/cache
LOGS=$WORK/logs
mkdir -p "$CACHE" "$LOGS"
NCPU=$(nproc)
PER=8                      # gcsim workers per job
JOBS=$(( NCPU / PER > 0 ? NCPU / PER : 1 ))
SHARDS=${SHARDS:-$JOBS}
SLICES=${SLICES:-0-$((SHARDS - 1))}
UPLOAD_EVERY=${UPLOAD_EVERY:-2h}
ORDER=""
[ "${REVERSE:-0}" = 1 ] && ORDER=-reverse
DEADLINE_AT=$(( $(date -u +%s) + $(echo "$DEADLINE" | sed 's/h/*3600+/;s/m/*60+/;s/s//;s/+$//' | bc) ))
START=$(date -u +%s)

progress() {
	{
		echo "tag $TAG, engine $(git -C engine rev-parse --short HEAD), Gscouter ${GSCOUTER_COMMIT:0:12}"
		echo "$NCPU CPUs, $JOBS jobs of $PER workers, slices $SLICES of $SHARDS, deadline $DEADLINE"
		echo "elapsed $(( ($(date -u +%s) - START) / 60 )) min, $(find "$CACHE" -type f | wc -l) chunks, $(du -sh "$CACHE" | cut -f1)"
		echo "load: $(cut -d' ' -f1-3 /proc/loadavg)"
		for k in $(seq 0 $((JOBS - 1))); do
			printf 'job %d: %s lines, last: %s\n' "$k" "$(wc -l < "$LOGS/$k.err" 2>/dev/null || echo 0)" "$(tail -n1 "$LOGS/$k.err" 2>/dev/null | cut -c1-160)"
		done
		echo "$1"
	} > "$WORK/progress.txt"
	upload "$WORK/progress.txt" progress.txt || true
}

cd "$WORK/engine/reference"
go build -o "$WORK/compare" ./compare
# a server that replaces another on the same TAG (a Spot VM evicted, a
# bigger machine) starts from the increments already uploaded, and numbers
# its own after them
INC=0
touch "$WORK/uploaded.txt"
for a in $(gh "$API/releases/$REL/assets?per_page=100" | jq -r '.[] | select(.name|test("^gcsim-cache-inc[0-9]+-")) | "\(.id)/\(.name)"'); do
	gh -H "Accept: application/octet-stream" -L "$API/releases/assets/${a%%/*}" | tar -xz -C "$CACHE"
	n=$(echo "${a#*/}" | sed 's/^gcsim-cache-inc0*\([0-9][0-9]*\)-.*/\1/;s/^gcsim-cache-inc0*-.*/0/')
	[ "$n" -lt "$INC" ] || INC=$((n + 1))
done
(cd "$CACHE" && find . -type f -name '*.json' | sort) > "$WORK/uploaded.txt"
progress "started"
# job k runs every JOBS-th slice of this server's range, one after another
read -r LO HI <<< "$(echo "$SLICES" | tr '-' ' ')"
for k in $(seq 0 $((JOBS - 1))); do
	(
		for s in $(seq $((LO + k)) "$JOBS" "$HI"); do
			left=$(( DEADLINE_AT - $(date -u +%s) ))
			[ "$left" -gt 60 ] || break
			"$WORK/compare" -gscouter "$WORK/gscouter" -gcsim-only -only library,variants,probes -sample 4 $ORDER \
				-shard "$s/$SHARDS" -j "$PER" -deadline "${left}s" -cache "$CACHE" \
				-json "$LOGS/$s.json" -md "$LOGS/$s.md" >> "$LOGS/$k.out" 2>> "$LOGS/$k.err" || true
		done
	) &
done

# increments: the chunks not uploaded yet, as gcsim-cache-incNN-XX.tar.gz
increment() {
	(cd "$CACHE" && find . -type f -name '*.json' | sort) > "$WORK/all.txt"
	comm -13 "$WORK/uploaded.txt" "$WORK/all.txt" > "$WORK/new.txt"
	[ -s "$WORK/new.txt" ] || return 0
	local n
	n=$(printf 'inc%02d' "$INC")
	split -l 200000 -d "$WORK/new.txt" "$WORK/new-"
	for p in "$WORK"/new-*; do
		tar -czf "$WORK/gcsim-cache-$n-${p##*/new-}.tar.gz" -C "$CACHE" -T "$p"
		upload "$WORK/gcsim-cache-$n-${p##*/new-}.tar.gz" "gcsim-cache-$n-${p##*/new-}.tar.gz"
		rm -f "$p" "$WORK/gcsim-cache-$n-${p##*/new-}.tar.gz"
	done
	sort -m "$WORK/uploaded.txt" "$WORK/new.txt" > "$WORK/up.tmp" && mv "$WORK/up.tmp" "$WORK/uploaded.txt"
	INC=$((INC + 1))
}
EVERY=$(( $(echo "$UPLOAD_EVERY" | sed 's/h/*3600+/;s/m/*60+/;s/s//;s/+$//' | bc) ))
NEXT=$(( $(date -u +%s) + EVERY ))
while [ -n "$(jobs -r)" ]; do
	sleep 600
	if [ "$(date -u +%s)" -ge "$NEXT" ]; then
		increment || true
		NEXT=$(( $(date -u +%s) + EVERY ))
	fi
	progress "running"
done
wait || true

# the rest of the cache
increment
tar -czf "$WORK/reports.tar.gz" -C "$LOGS" .
upload "$WORK/reports.tar.gz" reports.tar.gz
progress "DONE: the cache is uploaded; the server can be deleted"
