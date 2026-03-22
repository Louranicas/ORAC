# Config.kdl Anatomy

**File:** `~/.config/zellij/config.kdl` (603 lines)
**Backup:** `~/.config/zellij/config.kdl.bak`
**Requires restart** for: plugin aliases, load_plugins, themes

## Structure

```kdl
keybinds clear-defaults=true {
    // Mode blocks — each defines keybinds for that mode
    locked { ... }
    pane { ... }
    tab { ... }
    resize { ... }
    move { ... }
    scroll { ... }
    search { ... }
    session { ... }

    // Shared blocks — apply across multiple modes
    shared_except "locked" { ... }          // All modes except locked
    shared_except "locked" "move" { ... }   // Except locked and move
    shared_among "scroll" "search" { ... }  // Only scroll and search
}

plugins {
    // Aliases: name location="protocol:path" { config }
    // protocol: "zellij:" (built-in) or "file:" (local WASM)
    about location="zellij:about"
    swarm-orchestrator location="file:~/.config/zellij/plugins/swarm-orchestrator.wasm" {
        quality_threshold "0.80"
        max_iterations "5"
    }
}

load_plugins {
    // Auto-loaded on session start (background)
    "file:~/.config/zellij/plugins/zellij-autolock.wasm" {
        triggers "nvim|vim|pi|git|lazygit"
        reaction "lock"
        watch_triggers "nvim|vim|pi"
        watch_interval "1.0"
    }
    "file:~/.config/zellij/plugins/zellij-attention.wasm" {}
}

// Global options
show_startup_tips false
// theme "dracula"
// default_mode "normal"      // or "locked"
// default_shell "bash"
// mouse_mode true
// pane_frames true
// scroll_buffer_size 10000
// copy_command "wl-copy"     // or xclip, pbcopy
// copy_on_select true
// session_serialization true
// support_kitty_keyboard_protocol true
```

## Keybind Syntax

```kdl
bind "Ctrl g" { SwitchToMode "normal"; }           // Simple action
bind "Alt v" {                                      // Plugin launch
    LaunchOrFocusPlugin "file:path/to/plugin.wasm" {
        floating true
        move_to_focused_tab true
        // Plugin-specific config key-value pairs
        custom_param "value"
    }
}
bind "d" { NewPane "down"; SwitchToMode "normal"; } // Action + mode switch
bind "1" { GoToTab 1; SwitchToMode "normal"; }      // Chained actions
```

## Plugin Configuration in Keybinds

```kdl
// Ghost plugin with global completion menu
bind "Alt g" {
    LaunchOrFocusPlugin "file:~/.config/zellij/plugins/ghost.wasm" {
        floating true
        shell "bash"
        shell_flag "-ic"
        global_completion r#"
            lazygit
            btm
            litecli ~/path/to/db.db
            nvim --listen /tmp/nvim.sock
            claude
            yazi
        "#
    }
}

// Room with fuzzy options
bind "Ctrl y" {
    LaunchOrFocusPlugin "file:~/.config/zellij/plugins/room.wasm" {
        floating true
        ignore_case true
        quick_jump true
    }
}
```

## Built-in Plugin Aliases

| Alias | Location | Purpose |
|-------|----------|---------|
| about | zellij:about | Version info |
| compact-bar | zellij:compact-bar | Slim status bar |
| configuration | zellij:configuration | Settings UI |
| filepicker | zellij:strider | File browser (cwd "/") |
| plugin-manager | zellij:plugin-manager | Manage loaded plugins |
| session-manager | zellij:session-manager | Session switcher |
| status-bar | zellij:status-bar | Default status bar |
| strider | zellij:strider | File explorer |
| tab-bar | zellij:tab-bar | Tab strip |
| welcome-screen | zellij:session-manager | Startup screen |

## Editing Config

1. Edit `~/.config/zellij/config.kdl`
2. For keybind changes: start a new session (keybinds load on session start)
3. For plugin aliases / load_plugins: requires full Zellij restart
4. For themes: restart if theme changed, hot-reload if only colors changed

## KDL Syntax Notes

- Strings: `"quoted"` or `r#"raw string"#` (for multi-line like ghost completions)
- Booleans: `true` / `false` (no quotes)
- Numbers: bare `1`, `1000` (no quotes)
- Comments: `//` line comments, `/* */` block comments
- Node children: `{ }` braces
- Semicolons optional (newlines delimit)
