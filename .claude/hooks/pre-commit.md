# ORAC Pre-Commit Checks

## Before Every Commit
1. Run quality gate (check → clippy → pedantic → test)
2. Verify no `unwrap()` in production code: `rg 'unwrap\(\)' src/ --glob '!*test*' | head -5`
3. Verify no `unsafe`: `rg 'unsafe' src/ | head -5`
4. Verify no `println!`: `rg 'println!' src/ --glob '!*test*' | head -5`
5. Verify doc coverage: `rg 'pub (fn|struct|enum|trait)' src/ -c` vs `rg '///' src/ -c`

## Anti-Pattern Scan
```bash
# Check for known anti-patterns
rg 'unwrap\(\)|expect\(' src/ --glob '!*test*' && echo "FAIL: unwrap in prod" || echo "OK: no unwrap"
rg 'unsafe\s*\{' src/ && echo "FAIL: unsafe block" || echo "OK: no unsafe"
rg 'println!|eprintln!' src/ --glob '!*test*' && echo "FAIL: stdout in daemon" || echo "OK: no stdout"
rg 'http://' src/ --glob '*bridge*' && echo "WARN: http:// in bridge URL (BUG-033)" || echo "OK: no http prefix"
```

## Commit Message Convention
```
<type>(<scope>): <description>

Types: feat, fix, refactor, test, docs, chore
Scopes: hooks, wire, intelligence, bridges, monitoring, evolution, core
```
