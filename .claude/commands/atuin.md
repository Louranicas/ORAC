# /atuin — Shell Intelligence CLI Primitive

Access Atuin's history intelligence, KV store, and script library as CLI commands.

## Arguments
- `$ARGUMENTS` — subcommand: search, stats, density, scripts, run, kv-get, kv-set, service

## Usage
```
/atuin search curl 8133           — find commands that hit ORAC
/atuin stats 1d                   — command frequency last 24h
/atuin density                    — which services get most CLI attention
/atuin service 8132               — all commands that touched PV2
/atuin scripts                    — list all 61+ Atuin scripts
/atuin run pswarm-status          — run a named script
/atuin kv-get habitat.alert.latest — read coordination signal
/atuin kv-set habitat.session 081  — write coordination signal
```

## Action

```bash
CMD="${1:-scripts}"
shift 2>/dev/null
atuin-exec "$CMD" "$@"
```

## Why Use This

Atuin tracks every command with timestamps, duration, exit codes, and working directory. `atuin-exec density` reveals which services get the most attention. `atuin-exec service 8133` shows the history of ORAC interactions. The KV store is the zero-dependency coordination backbone — any agent can read/write without needing a running service. Every `atuin-exec` call through ORAC hooks adds tool diversity to STDP learning.
