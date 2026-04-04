# /pswarm-deploy — Build + Deploy Prometheus Swarm

Full quality gate → release build → deploy → verify cycle for Prometheus Swarm v2.0.

```bash
cd ~/claude-code-workspace/agent-swarm/prometheus_swarm/rust_core/swarm_coordinator

echo "━━━ QUALITY GATE ━━━"
echo -n "check: " && CARGO_TARGET_DIR=/tmp/cargo-prom cargo check 2>&1 | tail -1
echo -n "clippy: " && CARGO_TARGET_DIR=/tmp/cargo-prom cargo clippy -- -D warnings 2>&1 | tail -1
echo -n "tests: " && CARGO_TARGET_DIR=/tmp/cargo-prom timeout 45 cargo test --lib 2>&1 | grep "test result"

echo ""
echo "━━━ BUILD ━━━"
CARGO_TARGET_DIR=/tmp/cargo-prom cargo build --release 2>&1 | tail -1

echo ""
echo "━━━ DEPLOY ━━━"
pid=$(ss -tlnp "sport = :10002" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
[ -n "$pid" ] && kill "$pid" 2>/dev/null && sleep 2
nohup /tmp/cargo-prom/release/prometheus_swarm --port 10002 > /tmp/prometheus-swarm.log 2>&1 &
sleep 2

echo ""
echo "━━━ VERIFY ━━━"
curl -s localhost:10002/health 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
print(f'  {d[\"service\"]} v{d[\"version\"]} — {d[\"status\"]}')
"
```
