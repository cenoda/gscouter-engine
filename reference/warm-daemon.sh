#!/usr/bin/env bash
# warm-daemon.sh keeps a server (an Azure spot VM, x86) running gcsim into
# compare's cache, so that CI's compare runs mostly the engine. GitHub never
# sends work to the server: it pulls. It runs only main's code, takes from
# the repository nothing but team ids as data, and sends back only gcsim's
# results, which it uploads as release assets for CI's cache job to import.
# No port is open, and no pull request (fork or not) reaches it.
#
# Each round (every POLL, default 10m) it updates to origin/main, builds
# compare and gse, and runs, in this order:
#  1. requests: the team ids, one per line, in requests.txt on the branch
#     warm-requests (which only those with write access can push), that it
#     has not run yet: gcsim on their library builds, variants and probes,
#     as compare.yml's "warm" input does. A porting session asks for its
#     teams so, in place of dispatching "warm".
#  2. the next schedule's parts: the variants, build-space and pages parts
#     compare.yml's schedule will run next (its PARTS/PER_DAY formula), so
#     that those jobs find gcsim cached. Once per schedule day. compare
#     -supported-only takes the teams CI's compare takes, so that part k
#     here is CI's part k (unless main supports other teams by then).
# It runs JOBS (nproc / PER) compare processes at once, and looks at the
# requests again between two batches of parts.
#
# After each item that added chunks it uploads them as a prerelease
# gcsim-daemon-<time> (gcsim-cache-*.tar.gz, the format import_release
# takes) and, once no cache job of main is queued or running, dispatches
# compare.yml on main with import_release set to it. It waits for that
# because GitHub keeps one waiting job per concurrency group: a third cache
# job would cancel a waiting one and lose that run's chunks. Tags not yet
# imported are kept in $WORK/pending.txt and dispatched in order.
#
# Several servers share the work with no setting: before an item (a part
# of the schedule, a requested team, an import) a server claims it by
# creating the tag warm-claim/<item>, which GitHub creates for one server
# only; the others skip it. Claims of past days are deleted.
#
# Each round it also compares its installed copy with main's
# reference/warm-daemon.sh, and installs main's and restarts when they
# differ, so a fix on main reaches every server.
#
# Needs GH_TOKEN: a fine-grained token for cenoda/gscouter-engine only, with
# Contents read and write (releases) and Actions read and write (dispatch).
# Put it in /etc/warm-daemon.env as GH_TOKEN=..., mode 600, and run
#   sudo bash warm-daemon.sh install
# which installs Go, Rust and a systemd service (warm-daemon) that starts
# again with the VM (a spot VM deallocated on eviction keeps its disk, so
# its cache and state). "sudo bash warm-daemon.sh run" runs it in the
# foreground. Progress: journalctl -u warm-daemon, and $WORK/status.txt.
set -euo pipefail

REPO=cenoda/gscouter-engine
API=https://api.github.com/repos/$REPO
UPLOADS=https://uploads.github.com/repos/$REPO
WORK=${WORK:-/var/lib/warm-daemon}
POLL=${POLL:-600}
PER=${PER:-8}               # gcsim workers per compare process

if [ "${1:-}" = install ]; then
	[ -f /etc/warm-daemon.env ] || { echo "write GH_TOKEN=... to /etc/warm-daemon.env (mode 600) first" >&2; exit 1; }
	export DEBIAN_FRONTEND=noninteractive
	apt-get update -q && apt-get install -yq build-essential git curl jq tar gzip ca-certificates
	command -v rustup >/dev/null || [ -x /root/.cargo/bin/rustup ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
	install -m 755 "$0" /usr/local/bin/warm-daemon.sh
	cat > /etc/systemd/system/warm-daemon.service <<EOF
[Unit]
Description=gcsim cache daemon for $REPO (reference/warm-daemon.sh)
After=network-online.target
Wants=network-online.target

[Service]
EnvironmentFile=/etc/warm-daemon.env
Environment=HOME=/root
ExecStart=/usr/local/bin/warm-daemon.sh run
Restart=always
RestartSec=60
Nice=5

[Install]
WantedBy=multi-user.target
EOF
	systemctl daemon-reload
	systemctl enable warm-daemon
	systemctl restart warm-daemon
	echo "warm-daemon started: journalctl -u warm-daemon -f"
	exit 0
fi
[ "${1:-}" = run ] || { echo "usage: warm-daemon.sh install | run" >&2; exit 2; }
: "${GH_TOKEN:?set GH_TOKEN (Contents and Actions read and write on $REPO)}"

export HOME=${HOME:-/root} GOPATH=${GOPATH:-/root/go}
export PATH=/usr/local/go/bin:/root/.cargo/bin:$PATH GOFLAGS=-mod=mod
gh() { curl -sSf -H "Authorization: Bearer $GH_TOKEN" -H "Accept: application/vnd.github+json" "$@"; }
log() { echo "$(date -u +%FT%TZ) $*"; echo "$(date -u +%FT%TZ) $*" > "$WORK/status.txt"; }

# claim NAME: true when this server created the tag warm-claim/NAME (a
# tag GitHub creates once: another server's claim makes this fail)
claim() {
	gh -X POST "$API/git/refs" -d "$(jq -n --arg r "refs/tags/warm-claim/$1" --arg s "$BUILT" '{ref:$r, sha:$s}')" >/dev/null 2>&1
}
today() { echo $(( $(date -u +%s) / 86400 )); }
# claims of days before yesterday: d<day>-...
prune_claims() {
	local t ref day
	t=$(today)
	for ref in $(gh "$API/git/matching-refs/tags/warm-claim/d" | jq -r '.[].ref'); do
		day=${ref#refs/tags/warm-claim/d}; day=${day%%-*}
		[[ $day =~ ^[0-9]+$ ]] && [ "$day" -lt $((t - 1)) ] || continue
		gh -X DELETE "$API/git/$ref" >/dev/null 2>&1 || true
	done
}

mkdir -p "$WORK"/{cache,logs}
cd "$WORK"
CACHE=$WORK/cache
touch requests-done.txt pending.txt prewarmed.txt uploaded.txt
[ -d engine ] || git clone -q https://github.com/$REPO engine
[ -d gscouter ] || git clone -q https://github.com/cenoda/Gscouter gscouter
JOBS=$(( $(nproc) / PER > 0 ? $(nproc) / PER : 1 ))
BUILT=""

update() {
	git -C engine fetch -q origin main
	git -C engine checkout -q -f origin/main
	# main's copy of this script replaces the installed one
	if [ "$0" = /usr/local/bin/warm-daemon.sh ] && ! cmp -s engine/reference/warm-daemon.sh /usr/local/bin/warm-daemon.sh; then
		install -m 755 engine/reference/warm-daemon.sh /usr/local/bin/warm-daemon.sh
		log "installed main's warm-daemon.sh; restarting"
		exit 0
	fi
	local head
	head=$(git -C engine rev-parse HEAD)
	[ "$head" != "$BUILT" ] || return 0
	local gscouter_commit gover
	gscouter_commit=$(awk '/GSCOUTER_COMMIT:/{print $2; exit}' engine/.github/workflows/compare.yml)
	git -C gscouter fetch -q origin
	git -C gscouter checkout -q -f "$gscouter_commit"
	gover=$(awk '/^go /{print $2; exit}' engine/reference/go.mod)
	if ! command -v go >/dev/null || [ "$(go env GOVERSION)" != "go$gover" ]; then
		rm -rf /usr/local/go
		curl -sSfL "https://go.dev/dl/go$gover.linux-amd64.tar.gz" | tar -xz -C /usr/local
	fi
	(cd engine/reference && go build -o "$WORK/compare" ./compare)
	(cd engine/engine && cargo build -q --release)
	BUILT=$head
	log "built main ${head:0:7}"
}

# runs compare -gcsim-only once per argument set given (one per line on
# stdin), JOBS at a time with PER workers each, and publishes what they added
warm() { # name; argument sets on stdin
	local name=$1 i=0 args
	log "running $name"
	while read -r args; do
		# shellcheck disable=SC2086
		"$WORK/compare" -gscouter "$WORK/gscouter" -gse "$WORK/engine/engine/target/release/gse" -gcsim-only \
			$args -j "$PER" -cache "$CACHE" -json "$WORK/logs/$name-$i.json" -md "$WORK/logs/$name-$i.md" \
			< /dev/null >> "$WORK/logs/$name.out" 2>> "$WORK/logs/$name.err" &
		i=$((i + 1))
		[ "$(jobs -rp | wc -l)" -lt "$JOBS" ] || wait -n || true
	done
	wait || true
	publish "$name"
}

# the chunks not uploaded yet (uploaded.txt), as a prerelease for
# import_release; what a failed upload leaves goes with the next
publish() {
	(cd "$CACHE" && find . -type f -name '*.json' | sort) > "$WORK/all.txt"
	comm -13 "$WORK/uploaded.txt" "$WORK/all.txt" > "$WORK/new.txt"
	[ -s "$WORK/new.txt" ] || { log "$1: nothing new"; return 0; }
	local tag rel n
	tag=gcsim-daemon-$(date -u +%Y%m%d-%H%M%S)
	n=$(wc -l < "$WORK/new.txt")
	tar -czf "$WORK/gcsim-cache-$tag.tar.gz" -C "$CACHE" -T "$WORK/new.txt"
	rel=$(gh -X POST "$API/releases" -d "$(jq -n --arg t "$tag" --arg c "$BUILT" --arg b "$1: $n chunks of gcsim made by reference/warm-daemon.sh at main ${BUILT:0:7}. Imported into main's cache by compare.yml's import_release." \
		'{tag_name:$t, target_commitish:$c, name:$t, prerelease:true, body:$b}')" | jq -r .id)
	gh -X POST -H "Content-Type: application/octet-stream" --data-binary @"$WORK/gcsim-cache-$tag.tar.gz" \
		"$UPLOADS/releases/$rel/assets?name=gcsim-cache-$tag.tar.gz" >/dev/null
	rm -f "$WORK/gcsim-cache-$tag.tar.gz"
	sort -m "$WORK/uploaded.txt" "$WORK/new.txt" > "$WORK/up.tmp" && mv "$WORK/up.tmp" "$WORK/uploaded.txt"
	echo "$tag" >> "$WORK/pending.txt"
	log "$1: $n chunks uploaded as $tag"
}

# whether a cache job of main is queued or running in any run
main_cache_busy() {
	local id
	for st in queued in_progress waiting; do
		for id in $(gh "$API/actions/workflows/compare.yml/runs?branch=main&status=$st&per_page=50" | jq -r '.workflow_runs[].id'); do
			gh "$API/actions/runs/$id/jobs?per_page=100" | jq -e '.jobs[] | select(.name=="cache" and .status!="completed")' >/dev/null && return 0
		done
	done
	return 1
}

# dispatch the oldest pending tag when main's cache job is free
import_pending() {
	local tag
	tag=$(head -n1 "$WORK/pending.txt")
	[ -n "$tag" ] || return 0
	if main_cache_busy; then log "import of $tag waits for main's cache job"; return 0; fi
	# one dispatch in all servers per 10 minutes, so that two servers do
	# not dispatch at once past the check above
	claim "d$(today)-import-$(( $(date -u +%s) % 86400 / 600 ))" || { log "import of $tag waits for its turn"; return 0; }
	gh -X POST "$API/actions/workflows/compare.yml/dispatches" -d "$(jq -n --arg t "$tag" '{ref:"main", inputs:{import_release:$t}}')" >/dev/null
	sed -i 1d "$WORK/pending.txt"
	log "dispatched import_release=$tag on main"
}

next_request() {
	git -C engine fetch -q origin warm-requests 2>/dev/null || return 1
	git -C engine show FETCH_HEAD:requests.txt 2>/dev/null | tr -d '\r' | sed 's/#.*//;s/[[:space:]]//g' | grep -E '^[A-Za-z0-9_.-]+$' |
		grep -vxF -f "$WORK/requests-done.txt" | head -n 20
}

# the schedule's day after now (its cron is 17 3 * * *) and its parts
next_schedule() {
	local now day f
	now=$(date -u +%s)
	day=$(( now / 86400 ))
	[ $(( now % 86400 )) -lt $(( 3 * 3600 + 17 * 60 )) ] || day=$(( day + 1 ))
	f=engine/.github/workflows/compare.yml
	v() { awk -v k="  $1:" 'index($0,k)==1{print $2; exit}' "$f"; }
	local P PD B BD G GD
	P=$(v PARTS); PD=$(v PER_DAY); B=$(v BS_PARTS); BD=$(v BS_PER_DAY); G=$(v PG_PARTS); GD=$(v PG_PER_DAY)
	echo "$day variants $P $(( (day % (P / PD)) * PD )) $PD"
	echo "$day buildspace $B $(( (day % (B / BD)) * BD )) $BD"
	echo "$day pages $G $(( (day % (G / GD)) * GD )) $GD"
}

serve_requests() {
	local ids id
	while true; do
		ids=""
		for id in $(next_request || true); do
			echo "$id" >> "$WORK/requests-done.txt"
			claim "req-$id" && ids="$ids,$id"
		done
		ids=${ids#,}
		[ -n "$ids" ] || break
		for k in $(seq 0 $((JOBS - 1))); do
			echo "-teams $ids -only library,variants,probes -sample 4 -shard $k/$JOBS"
		done | warm "request-$(date -u +%H%M%S)"
		import_pending
	done
}

while true; do
	update || log "update failed; using ${BUILT:0:7}"
	[ -n "$BUILT" ] || { sleep "$POLL"; continue; }
	[ "$(cat "$WORK/pruned" 2>/dev/null)" = "$(today)" ] || { prune_claims || true; today > "$WORK/pruned"; }
	import_pending || true
	serve_requests || true
	# the next schedule's parts not run yet, JOBS at a time, the requests
	# looked at between batches
	next_schedule | while read -r day kind n first count; do
		for s in $(seq "$first" $((first + count - 1))); do
			grep -qxF "$day $kind $s" "$WORK/prewarmed.txt" || echo "$day $kind $s $n"
		done
	done > "$WORK/todo.txt"
	while [ -s "$WORK/todo.txt" ]; do
		# the next JOBS items this server claims; the others are another's
		: > "$WORK/batch.txt"
		while [ -s "$WORK/todo.txt" ] && [ "$(wc -l < "$WORK/batch.txt")" -lt "$JOBS" ]; do
			read -r day kind s n < "$WORK/todo.txt"
			sed -i 1d "$WORK/todo.txt"
			if claim "d$day-$kind-$s"; then
				echo "$day $kind $s $n" >> "$WORK/batch.txt"
			else
				echo "$day $kind $s" >> "$WORK/prewarmed.txt"
			fi
		done
		[ -s "$WORK/batch.txt" ] || break
		awk '{print "-supported-only -only " $2 " -sample 4 -shard " $3 "/" $4}' "$WORK/batch.txt" | warm "schedule-day$(head -c 5 "$WORK/batch.txt")"
		cut -d' ' -f1-3 "$WORK/batch.txt" >> "$WORK/prewarmed.txt"
		import_pending || true
		serve_requests || true
	done
	log "idle"
	sleep "$POLL"
done
