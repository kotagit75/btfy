#!/usr/bin/env bash
set -euo pipefail

COMPOSE_FILE="$(dirname "$0")/docker-compose.yaml"
RESULT_DIR="$(dirname "$0")/results"
NODE_A_API="http://localhost:8080"
NODE_B_API="http://localhost:8081"
P2P_IP_B="172.28.0.3:62698"
P2P_IP_A="172.28.0.2:62697"
TIMEOUT=60
SLEEP_INTERVAL=2

log() { printf "%s %s\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
ensure_cmds() {
  for cmd in curl jq docker; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
      echo "required command not found: $cmd" >&2
      exit 2
    fi
  done
}

http_get() { curl -sS -f "$1"; }
http_post_json() { curl -sS -f -X POST -H "Content-Type: application/json" -d "$2" "$1"; }

wait_for_health() {
  local url="$1"; local deadline=$((SECONDS+TIMEOUT))
  while :; do
    if curl -s -o /dev/null -w '%{http_code}' "$url/health" 2>/dev/null | grep -q '^200$'; then
      log "health OK at $url"
      return 0
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
      log "timeout waiting for health at $url"
      return 1
    fi
    sleep $SLEEP_INTERVAL
  done
}

safe_jq() {
  # usage: safe_jq <filter> <json>
  echo "$2" | jq -r --argjson null null "$1" 2>/dev/null || echo ""
}

record() {
  mkdir -p "$RESULT_DIR"
  echo "$1" >> "$RESULT_DIR/p2p_test_$(date +%Y%m%dT%H%M%S).log"
}

cleanup() {
  log "Cleaning up docker compose"
  docker compose -f "$COMPOSE_FILE" down -v >/dev/null 2>&1 || true
}

fail() {
  log "ERROR: $*"
  cleanup
  exit 1
}

# ensure cleanup runs on exit as a safety net
trap cleanup EXIT

main() {
  ensure_cmds

  log "Starting compose: $COMPOSE_FILE"
  docker compose -f "$COMPOSE_FILE" up -d --build

  log "Waiting for Node A and B health"
  wait_for_health "$NODE_A_API" || { log "Node A failed to become healthy"; docker compose -f "$COMPOSE_FILE" down -v; exit 1; }
  wait_for_health "$NODE_B_API" || { log "Node B failed to become healthy"; docker compose -f "$COMPOSE_FILE" down -v; exit 1; }

  log "Fetching addresses"
  addr_a=$(http_get "$NODE_A_API/address" 2>/dev/null || echo "")
  addr_b=$(http_get "$NODE_B_API/address" 2>/dev/null || echo "")
  log "Node A address: ${addr_a:-(none)}"
  log "Node B address: ${addr_b:-(none)}"
  record "Node A: $addr_a\nNode B: $addr_b"

  log "Adding peer (Node A -> Node B: $P2P_IP_B)"
  if ! addres=$(http_post_json "$NODE_A_API/peer" "{\"addr\": \"$P2P_IP_B\"}" 2>&1); then
    fail "addpeer failed: $addres"
  fi
  sleep 2

  log "Verifying peers on both nodes"
  peers_a=$(http_get "$NODE_A_API/peers" || echo "")
  peers_b=$(http_get "$NODE_B_API/peers" || echo "")
  log "Node A peers: $peers_a"
  log "Node B peers: $peers_b"
  record "peers_a: $peers_a\npeers_b: $peers_b"

  if echo "$peers_a" | grep -q "$P2P_IP_B" && echo "$peers_b" | grep -q "$P2P_IP_A"; then
    log "Peer registration appears mutual"
  else
    log "WARNING: peer registration not mutual or not visible yet"
  fi

  log "Testing idempotent addpeer (duplicate)"
  http_post_json "$NODE_A_API/peer" "{\"addr\": \"$P2P_IP_B\"}" >/dev/null 2>&1 || true
  log "Duplicate addpeer sent"

  log "Waiting for Node A chain length >= 1 before issuing tx"
  deadline_chain=$((SECONDS+TIMEOUT))
  while :; do
    len_a=$(http_get "$NODE_A_API/chain" 2>/dev/null | jq .blocks | jq -r 'if type=="array" then length else (.length // 0) end' 2>/dev/null || echo 0)
    if [ "$len_a" -ge 1 ]; then
      log "Node A chain length is $len_a"
      break
    fi
    if [ "$SECONDS" -ge "$deadline_chain" ]; then
      fail "timeout waiting for Node A chain length >=1"
    fi
    sleep $SLEEP_INTERVAL
  done

  log "Issuing sample transaction to Node A"
  if ! txres=$(http_post_json "$NODE_A_API/tx" '{"recipient":"30a", "send_amount": 1, "fee": 0}' 2>&1); then
    fail "tx post failed: $txres"
  fi
  log "tx result: ${txres:0:200}"

  log "Waiting for chain propagation to Node B"
  # get chain length helper
  get_chain_len() {
    local url="$1"
    local raw
    raw=$(http_get "$url/chain" || echo "")
    echo "$raw" | jq .blocks | jq -r '. | if type=="array" then length else (.length // 0) end' 2>/dev/null || echo 0
  }
  get_chain_hash() {
    local url="$1"
    raw=$(http_get "$url/chain" || echo "")
    echo "$raw" | jq .blocks | jq -r 'if type=="array" then (.[-1].hash // .[-1].block_hash // .[-1].header.hash) else (.last.hash // .latest.hash // "") end' 2>/dev/null || echo ""
  }

  local_deadline=$((SECONDS+TIMEOUT))
  while [ "$SECONDS" -lt "$local_deadline" ]; do
    len_a=$(get_chain_len "$NODE_A_API")
    len_b=$(get_chain_len "$NODE_B_API")
    hash_a=$(get_chain_hash "$NODE_A_API")
    hash_b=$(get_chain_hash "$NODE_B_API")
    log "chain lengths: A=$len_a B=$len_b"
    if [ "$len_b" -ge "$len_a" ] && [ -n "$hash_a" ] && [ "$hash_a" = "$hash_b" ]; then
      log "Chain propagated to Node B"
      break
    fi
    sleep $SLEEP_INTERVAL
  done

  if [ "$SECONDS" -ge "$local_deadline" ]; then
    fail "Timeout waiting for chain propagation"
  fi

  log "Abnormal addpeer test: non-existent IP"
  badip="172.28.255.254:62697"
  if curl -s -X POST -H "Content-Type: application/json" -d "{\"ip\": \"$badip\"}" "$NODE_A_API/peer" >/dev/null 2>&1; then
    log "addpeer to non-existent IP returned success (check behavior)"
  else
    log "addpeer to non-existent IP returned error or timed out (expected)"
  fi

  log "Collecting docker logs"
  mkdir -p "$RESULT_DIR"
  docker logs btfy-node-a > "$RESULT_DIR/node-a.log" 2>&1 || true
  docker logs btfy-node-b > "$RESULT_DIR/node-b.log" 2>&1 || true

  log "Test finished; cleaning up compose"
  docker compose -f "$COMPOSE_FILE" down -v >/dev/null
  log "Results saved to $RESULT_DIR"
}

main "$@"
