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

export DEBIAN_FRONTEND=noninteractive
apt-get update -q && apt-get install -yq git curl jq tar gzip ca-certificates

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
REL=$(gh -X POST "$API/releases" -d "$(jq -n --arg t "$TAG" --arg c "$(git -C engine rev-parse HEAD)" \
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
START=$(date -u +%s)

progress() {
	{
		echo "tag $TAG, engine $(git -C engine rev-parse --short HEAD), Gscouter ${GSCOUTER_COMMIT:0:12}"
		echo "$NCPU CPUs, $JOBS jobs of $PER workers, deadline $DEADLINE"
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
progress "started"
for k in $(seq 0 $((JOBS - 1))); do
	"$WORK/compare" -gscouter "$WORK/gscouter" -gcsim-only -only library,variants,probes -sample 4 \
		-shard "$k/$JOBS" -j "$PER" -deadline "$DEADLINE" -cache "$CACHE" \
		-json "$LOGS/$k.json" -md "$LOGS/$k.md" > "$LOGS/$k.out" 2> "$LOGS/$k.err" &
done
while [ -n "$(jobs -r)" ]; do
	sleep 600
	progress "running"
done
wait || true

# the cache, in parts below GitHub's 2 GiB per asset
cd "$CACHE"
find . -type f -name '*.json' | sort > "$WORK/files.txt"
split -l 200000 -d "$WORK/files.txt" "$WORK/part-"
for p in "$WORK"/part-*; do
	n=${p##*/part-}
	tar -czf "$WORK/gcsim-cache-$n.tar.gz" -T "$p"
	upload "$WORK/gcsim-cache-$n.tar.gz" "gcsim-cache-$n.tar.gz"
done
tar -czf "$WORK/reports.tar.gz" -C "$LOGS" .
upload "$WORK/reports.tar.gz" reports.tar.gz
progress "DONE: the cache is uploaded; the server can be deleted"
