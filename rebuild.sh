#!/bin/bash
# Rebuild and restart hledger-helper after ANY code change (Rust or static files,
# which are embedded at compile time). Verifies the server comes back healthy.
set -e
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

echo "== building =="
cargo build --release

echo "== restarting =="
pkill -f "target/release/hledger-helper" || true
sleep 2
setsid nohup ./target/release/hledger-helper -c ./config.yaml > /tmp/hledger-helper.log 2>&1 < /dev/null &

echo "== health check =="
for i in $(seq 1 15); do
  if curl -s -o /dev/null --max-time 3 http://localhost:8080/; then
    echo "server healthy"
    exit 0
  fi
  sleep 2
done
echo "server did not come up; see /tmp/hledger-helper.log"
exit 1
