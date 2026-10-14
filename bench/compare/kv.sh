#!/usr/bin/env bash
# Comparative key-value benchmark (T01.18, STRATEGIE_DE_TESTS.md section 9.4).
# Runs the reference and the candidate one after the other, never together,
# on the same cores, with the same memtier_benchmark load, then
# `cargo run -p xtask -- compare <out>` gives the verdict (ADR-027).
#
# Usage: bench/compare/kv.sh [--server-cpus 0] [--client-cpus 2,3]
#          [--duration 60] [--repeats 5] [--reference IMAGE] [--candidate IMAGE]
#          [--candidate-cpu-quota Q] [--out DIR]
# Both sides run `valkey-server` with bench/compare/valkey.conf for now:
# Ndaal becomes a candidate with its RESP frontend (J03).
set -euo pipefail
cd "$(dirname "$0")/../.."

# valkey/valkey:9.0.6 and redislabs/memtier_benchmark:2.5.2, pinned by digest.
VALKEY="valkey/valkey:9.0.6@sha256:63346cb24a61221e76bdf41acce99b3968a9fa83d8122144deab45394b27b4f2"
MEMTIER="redislabs/memtier_benchmark:2.5.2@sha256:389090e9ef13701f36f5ac97485742618e648adbf76c008418383594a05c1659"
# Longest wait for a server to answer PING, in seconds.
READY_TIMEOUT=30
# Keys written before measuring, then read and written by the load: every GET
# hits a 100-byte value (a load on an empty keyspace only measures misses).
KEYS=100000
# The measured load (cmp-kv-latency, short version, closed loop), recorded as
# is in metadata.json.
LOAD="--protocol=redis --ratio=1:10 --data-size=100 --key-maximum=$KEYS --threads=2 --clients=25"

server_cpus=0
client_cpus=2,3
duration=60
repeats=5
reference="$VALKEY"
candidate="$VALKEY"
candidate_quota=""
out="bench-results/kv-$(date -u +%Y%m%dT%H%M%SZ)"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --server-cpus) server_cpus="$2" ;;
    --client-cpus) client_cpus="$2" ;;
    --duration) duration="$2" ;;
    --repeats) repeats="$2" ;;
    --reference) reference="$2" ;;
    --candidate) candidate="$2" ;;
    --candidate-cpu-quota) candidate_quota="$2" ;;
    --out) out="$2" ;;
    *) echo "kv.sh: unknown option $1" >&2; exit 2 ;;
  esac
  shift 2
done

# Every requested core must exist on the Docker host (not the Windows host).
ncpu="$(docker info --format '{{.NCPU}}')"
for cpu in $(echo "$server_cpus,$client_cpus" | tr ',-' '  '); do
  if [ "$cpu" -ge "$ncpu" ]; then
    echo "kv.sh: cpu $cpu requested, the Docker host has $ncpu" >&2
    exit 2
  fi
done

if [ -n "$(ls -A "$out" 2> /dev/null)" ]; then
  # Runs of an earlier invocation would mix into the comparison.
  echo "kv.sh: $out is not empty" >&2
  exit 2
fi
mkdir -p "$out"
# Windows path under Git Bash (pwd -W) for docker -v, plain path elsewhere.
out_abs="$(cd "$out" && (pwd -W 2>/dev/null || pwd))"
conf_abs="$(cd bench/compare && (pwd -W 2>/dev/null || pwd))/valkey.conf"
network="ndaal-compare-$$"
server="ndaal-compare-server-$$"

cleanup() {
  docker rm -f "$server" > /dev/null 2>&1 || true
  docker network rm "$network" > /dev/null 2>&1 || true
}
trap cleanup EXIT
docker network create "$network" > /dev/null

caches_dropped=false
drop_caches() {
  if [ "$(uname -s)" = Linux ] && sudo -n true 2> /dev/null; then
    sync
    echo 3 | sudo tee /proc/sys/vm/drop_caches > /dev/null
    caches_dropped=true
  fi
}

# run_side <reference|candidate> <image> <repetition>
run_side() {
  local side="$1" image="$2" index="$3"
  local limits=(--cpuset-cpus "$server_cpus")
  if [ "$side" = candidate ] && [ -n "$candidate_quota" ]; then
    limits+=(--cpus "$candidate_quota")
  fi
  drop_caches
  MSYS_NO_PATHCONV=1 docker run -d --name "$server" --network "$network" "${limits[@]}" \
    -v "$conf_abs:/etc/valkey/valkey.conf:ro" "$image" \
    valkey-server /etc/valkey/valkey.conf > /dev/null
  # Wall-clock deadline: each docker exec takes time of its own.
  local deadline=$((SECONDS + READY_TIMEOUT))
  until docker exec "$server" valkey-cli ping 2> /dev/null | grep -q PONG; do
    local running
    running="$(docker inspect --format '{{.State.Running}}' "$server" 2> /dev/null || echo false)"
    if [ "$running" != true ] || [ "$SECONDS" -ge "$deadline" ]; then
      echo "kv.sh: $side server did not answer PING (running: $running)" >&2
      docker logs "$server" >&2 || true
      exit 1
    fi
    sleep 1
  done
  # Preload, not measured: every key once, in order.
  docker run --rm --network "$network" --cpuset-cpus "$client_cpus" "$MEMTIER" \
    --server="$server" --port=6379 --protocol=redis --ratio=1:0 --data-size=100 \
    --key-pattern=P:P --key-maximum="$KEYS" --requests=allkeys --hide-histogram \
    > /dev/null 2>&1
  echo "kv.sh: run $index, $side"
  # shellcheck disable=SC2086 # LOAD holds several options on purpose.
  MSYS_NO_PATHCONV=1 docker run --rm --network "$network" --cpuset-cpus "$client_cpus" \
    -v "$out_abs:/out" "$MEMTIER" \
    --server="$server" --port=6379 $LOAD --test-time="$duration" --hide-histogram \
    --json-out-file="/out/$side-$index.json" > /dev/null 2>&1
  docker rm -f "$server" > /dev/null
}

# Alternate the order (A B, B A, ...) so a drift of the machine during the
# run weighs on both sides alike.
for index in $(seq 1 "$repeats"); do
  if [ $((index % 2)) -eq 1 ]; then
    run_side reference "$reference" "$index"
    run_side candidate "$candidate" "$index"
  else
    run_side candidate "$candidate" "$index"
    run_side reference "$reference" "$index"
  fi
done

digest() { docker image inspect --format '{{index .RepoDigests 0}}' "$1"; }
cat > "$out/metadata.json" << JSON
{
  "bench": "kv",
  "load": "memtier_benchmark $LOAD, after a preload of $KEYS keys",
  "durability": "none (save \"\", appendonly no)",
  "reference": "$(digest "$reference")",
  "candidate": "$(digest "$candidate")",
  "memtier": "$(digest "$MEMTIER")",
  "server_cpus": "$server_cpus",
  "client_cpus": "$client_cpus",
  "candidate_cpu_quota": "${candidate_quota:-none}",
  "duration_seconds": $duration,
  "repeats": $repeats,
  "caches_dropped": $caches_dropped,
  "kernel": "$(docker info --format '{{.KernelVersion}}')",
  "docker_host_cpus": $ncpu,
  "ndaal_commit": "$(git rev-parse HEAD)",
  "date_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
JSON
echo "kv.sh: results in $out"
