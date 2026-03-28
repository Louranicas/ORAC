# /topology -- Structural Census

2-second scan of codebase structure across key service directories.

```bash
echo "=== Habitat Topology ==="
echo ""
for dir in orac-sidecar pane-vortex-v2 the_maintenance_engine developer_environment_manager/synthex; do
  full="$HOME/claude-code-workspace/$dir"
  if [ -d "$full/src" ]; then
    files=$(fd -e rs --no-ignore "$full/src" 2>/dev/null | wc -l)
    loc=$(fd -e rs --no-ignore "$full/src" -x wc -l 2>/dev/null | tail -1 | awk '{print $1}')
    tests=$(rg --hidden -c '#\[test\]' "$full/src" 2>/dev/null | awk -F: '{s+=$2}END{print s+0}')
    name=$(basename "$dir")
    printf "  %-25s %4d files  %6s LOC  %5d tests\n" "$name" "$files" "$loc" "$tests"
  fi
done
echo ""
echo "=== Service Ports ==="
for p in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 8133 9001 10001; do
  code=$(curl -s -o /dev/null -w '%{http_code}' -m 1 "localhost:$p/health" 2>/dev/null)
  [ "$code" != "200" ] && code=$(curl -s -o /dev/null -w '%{http_code}' -m 1 "localhost:$p/api/health" 2>/dev/null)
  printf "  :%d %s\n" "$p" "$([ "$code" = "200" ] && echo "UP" || echo "DOWN ($code)")"
done
```
