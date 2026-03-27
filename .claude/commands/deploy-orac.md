# /deploy-orac — ORAC Build + Deploy Cycle

Full pipeline: quality gate, release build, stop, deploy, start, verify. Encodes all known traps (exit 144, cp alias, SIGPIPE).

**IMPORTANT:** Run /gate first. This command assumes the gate passed.

## Steps

1. Build release binary:
```bash
cd /home/louranicas/claude-code-workspace/orac-sidecar
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release --features full 2>&1 | tail -3
```

2. Stop ORAC (separate command — pkill exit 144 kills && chains):
```bash
pkill -f "orac-sidecar" 2>/dev/null; true
```

3. Wait + deploy binary (use /usr/bin/cp — cp is aliased to interactive):
```bash
sleep 2
/usr/bin/cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/orac-sidecar
```

4. Start ORAC (nohup to file — never stdout in daemons, SIGPIPE death):
```bash
nohup ~/.local/bin/orac-sidecar > /tmp/orac-sidecar.log 2>&1 &
echo "ORAC PID $!"
```

5. Verify (wait for startup):
```bash
sleep 5
curl -s localhost:8133/health | jq '{status, ralph_gen, ralph_fitness, ipc_state, sessions}'
```
