# KDL Layout Syntax

**Directory:** `~/.config/zellij/layouts/`
**Launch:** `zellij --layout ~/.config/zellij/layouts/NAME.kdl` or `zellij --layout NAME` (if in layouts dir)

## Layout Grammar

```kdl
layout {
    // Template applied to all tabs that don't override
    default_tab_template {
        pane size=1 borderless=true {
            plugin location="zellij:tab-bar"    // Top bar
        }
        children                                 // Tab content goes here
        pane size=1 borderless=true {
            plugin location="zellij:status-bar"  // Bottom bar
        }
    }

    // Tab definition
    tab name="TabName" focus=true cwd="/path" {
        pane split_direction="vertical" {
            pane size="50%" name="left" focus=true
            pane size="50%" {
                pane size="50%" name="top-right"
                pane size="50%" name="bot-right"
            }
        }
        // Optional floating plugin
        floating_panes {
            pane {
                plugin location="file:~/.config/zellij/plugins/NAME.wasm" {
                    config_key "value"
                }
            }
        }
    }

    // Swap layout (Alt+[ / Alt+] to cycle)
    swap_tiled_layout name="stacked" {
        tab {
            pane { children }
        }
    }
}
```

## Pane Properties

| Property | Values | Notes |
|----------|--------|-------|
| `size` | `"50%"`, `1`, `"10"` | Percentage (string) or fixed rows/cols (number) |
| `name` | `"string"` | Visible label in pane frame |
| `focus` | `true` | Which pane gets focus on tab load |
| `split_direction` | `"vertical"`, `"horizontal"` | How children split |
| `borderless` | `true` | No frame (used for tab-bar, status-bar) |
| `command` | `"nvim"` | Auto-launch command |
| `args` | `"-c" "command"` | Arguments to command |
| `cwd` | `"/path"` | Working directory override |
| `stacked` | `true` | Stack children (one expanded, rest collapsed) |
| `expanded` | `true` | Which stacked pane is visible (on child pane) |

## Pane Types

### Split Pane (container)
```kdl
pane split_direction="vertical" {
    pane size="60%" name="main"
    pane size="40%" name="side"
}
```

### Command Pane (auto-launches program)
```kdl
pane name="editor" command="nvim" {
    args "src/main.rs"
}
// Or inline bash
pane name="services" command="bash" {
    args "-c" "echo 'hello' && exec bash"
}
```

### Plugin Pane (WASM plugin)
```kdl
pane size=1 borderless=true {
    plugin location="zellij:tab-bar"
}
// Or custom WASM
pane {
    plugin location="file:~/.config/zellij/plugins/zjstatus.wasm" {
        // Plugin config
    }
}
```

### Stacked Panes
```kdl
pane stacked=true size="35%" {
    pane name="editor" expanded=true   // Visible
    pane name="review"                 // Collapsed
    pane name="shell"                  // Collapsed
}
```

### Floating Panes (overlay)
```kdl
tab name="MyTab" {
    pane name="main"
    floating_panes {
        pane {
            plugin location="file:~/.config/zellij/plugins/swarm-orchestrator.wasm" {
                quality_threshold "0.80"
            }
        }
    }
}
```

## The Gold Standard Pattern (Synth-Orchestrator)

```
6 tabs, consistent 3-pane layout per tab:
├─ LEFT (50%) — primary content/operator
└─ RIGHT (50%)
   ├─ TOP-RIGHT (50%) — secondary/monitor
   └─ BOT-RIGHT (50%) — tertiary/logs

Tab naming: descriptive + status suffix for fleet ("[IDLE]")
Pane naming: tab-context prefix (ALPHA-Left, ALPHA-TopRight)
CWD: all tabs use ~/claude-code-workspace
```

## Layout Variants Summary

| Layout | Tabs | Key Feature |
|--------|------|-------------|
| synth-orchestrator | 6 | Gold standard: all panes named, floating swarm plugin |
| devenv | 6 | Auto-launch commands (devenv status, nvim, yazi), API cheat sheets |
| swarm-orchestrator | 4 | Fleet-focused: 1 orch + 3 wings |
| ultraplate | 6 | General purpose: no auto-launch commands |
| review | 1 | 65/35 split: agent + stacked editor/review/shell |
| review-minimal | 1 | Same but with stacked=true (native Zellij stacking) |

## Creating a New Layout

1. Copy closest existing layout
2. Modify tab/pane structure
3. Always include `default_tab_template` with tab-bar + status-bar
4. Always include at least one `swap_tiled_layout` for Alt+[] cycling
5. Name ALL panes for instant identification
6. Set `focus=true` on the primary tab and primary pane within each tab
7. Test: `zellij --layout path/to/layout.kdl --session test-layout`
