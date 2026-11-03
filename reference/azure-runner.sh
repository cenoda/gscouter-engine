#!/usr/bin/env bash
# azure-runner.sh sets up a VM (Ubuntu 22.04 or 24.04, e.g. an Azure spot
# F32as_v6) as RUNNERS self-hosted GitHub Actions runners for
# cenoda/gscouter-engine, labelled "azure", each a systemd service that
# starts again when the VM does (a spot VM deallocated on eviction and
# started again comes back with its runners).
#
# compare.yml runs its long jobs (variants, buildspace, pages, warm, rerun,
# rerun-gcsim) on the runners the repository variable HEAVY_RUNS_ON names:
# set it to ["self-hosted","azure"] to send them here, delete it to send
# them back to GitHub's runners. The push jobs (library, cache, manifest)
# stay on GitHub's. Each runner here passes compare -j COMPARE_J (default
# nproc / RUNNERS), so that the runners of one VM share its cores.
#
# Run once as root:
#   sudo RUNNER_TOKEN=... bash azure-runner.sh
# RUNNER_TOKEN is a registration token from the repository's Settings →
# Actions → Runners → New self-hosted runner (valid one hour). Optional:
# RUNNERS (default 8), NAME (default the host name), COMPARE_J.
set -euo pipefail

: "${RUNNER_TOKEN:?set RUNNER_TOKEN to a registration token (Settings > Actions > Runners > New self-hosted runner)}"
REPO_URL=https://github.com/cenoda/gscouter-engine
RUNNERS=${RUNNERS:-8}
NAME=${NAME:-$(hostname)}
COMPARE_J=${COMPARE_J:-$(( $(nproc) / RUNNERS > 0 ? $(nproc) / RUNNERS : 1 ))}
USER_NAME=runner
BASE=/opt/actions-runner

# what GitHub's ubuntu image has and the jobs use: a C linker for cargo,
# zstd for actions/cache, curl for rustup
export DEBIAN_FRONTEND=noninteractive
apt-get update -q
apt-get install -yq build-essential curl git jq tar zstd unzip ca-certificates

id "$USER_NAME" >/dev/null 2>&1 || useradd -m -s /bin/bash "$USER_NAME"

version=$(curl -fsSL https://api.github.com/repos/actions/runner/releases/latest | jq -r .tag_name | sed 's/^v//')
tarball="$BASE/actions-runner-linux-x64-$version.tar.gz"
mkdir -p "$BASE"
[ -f "$tarball" ] || curl -fsSL -o "$tarball" "https://github.com/actions/runner/releases/download/v$version/actions-runner-linux-x64-$version.tar.gz"

for i in $(seq 1 "$RUNNERS"); do
  dir="$BASE/r$i"
  if [ -f "$dir/.runner" ]; then
    echo "runner $i already configured"
    continue
  fi
  mkdir -p "$dir"
  tar -xzf "$tarball" -C "$dir"
  chown -R "$USER_NAME:$USER_NAME" "$dir"
  # environment of every job on this runner
  printf 'COMPARE_J=%s\n' "$COMPARE_J" > "$dir/.env"
  chown "$USER_NAME:$USER_NAME" "$dir/.env"
  sudo -u "$USER_NAME" "$dir/config.sh" --unattended --url "$REPO_URL" --token "$RUNNER_TOKEN" \
    --name "$NAME-$i" --labels azure --work _work --replace
  (cd "$dir" && ./svc.sh install "$USER_NAME" && ./svc.sh start)
done

echo "$RUNNERS runners on $NAME, compare -j $COMPARE_J each"
