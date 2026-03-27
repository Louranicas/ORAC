# /gate — ORAC Quality Gate (4-Stage Zero-Tolerance Pipeline)

Run the mandatory quality gate. All 4 stages must pass with zero errors and zero warnings before any commit or deploy.

```bash
cd /home/louranicas/claude-code-workspace/orac-sidecar

echo "━━━ STAGE 1: cargo check ━━━"
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -5
echo ""

echo "━━━ STAGE 2: clippy -D warnings ━━━"
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -5
echo ""

echo "━━━ STAGE 3: clippy pedantic ━━━"
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -5
echo ""

echo "━━━ STAGE 4: tests ━━━"
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -5
echo ""

echo "━━━ GATE RESULT ━━━"
echo "If all 4 stages show 0 errors and 0 warnings: PASS"
echo "If any stage fails: STOP. Fix before proceeding."
```
