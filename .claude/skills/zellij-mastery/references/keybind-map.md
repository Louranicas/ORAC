# Complete Keybind Map (138+ keybinds, 11 modes)

## Mode Entry (from Normal mode)

| Key | Mode | Purpose |
|-----|------|---------|
| `Ctrl+p` | Pane | Create, manage, navigate panes |
| `Ctrl+t` | Tab | Create, manage, navigate tabs |
| `Ctrl+n` | Resize | Resize panes |
| `Ctrl+h` | Move | Move panes around layout |
| `Ctrl+s` | Scroll | Scroll pane content, enter search |
| `Ctrl+o` | Session | Session management, plugin UIs |
| `Ctrl+b` | Tmux | Tmux-compatible keybinds |
| `Ctrl+g` | Locked | Disable ALL keybinds (safety) |

## PANE Mode (Ctrl+p)

| Key | Action |
|-----|--------|
| `h/j/k/l` | Move focus left/down/up/right |
| arrows | Move focus (same as hjkl) |
| `n` | New pane (default direction) |
| `d` | New pane down |
| `r` | New pane right |
| `s` | New pane stacked |
| `f` | Toggle fullscreen on focused pane |
| `e` | Toggle pane embed/floating |
| `w` | Toggle floating panes visibility |
| `i` | Toggle pane pinned |
| `c` | Rename pane (enters rename mode) |
| `z` | Toggle pane frames |
| `p` | Switch focus between two panes |
| `Ctrl+p` | Exit pane mode |

## TAB Mode (Ctrl+t)

| Key | Action |
|-----|--------|
| `1-9` | Go to tab N (direct jump) |
| `h/k/left/up` | Previous tab |
| `j/l/right/down` | Next tab |
| `n` | New tab |
| `x` | Close current tab |
| `r` | Rename tab (enters rename mode) |
| `s` | Toggle sync (typed input goes to ALL panes) |
| `b` | Break focused pane to new tab |
| `[` | Break pane left |
| `]` | Break pane right |
| `tab` | Toggle to previous tab |
| `Ctrl+t` | Exit tab mode |

## RESIZE Mode (Ctrl+n)

| Key | Action |
|-----|--------|
| `h/left` | Increase left |
| `j/down` | Increase down |
| `k/up` | Increase up |
| `l/right` | Increase right |
| `H` | Decrease left |
| `J` | Decrease down |
| `K` | Decrease up |
| `L` | Decrease right |
| `+` / `=` | Increase uniform |
| `-` | Decrease uniform |
| `Ctrl+n` | Exit resize mode |

## MOVE Mode (Ctrl+h)

| Key | Action |
|-----|--------|
| `h/left` | Move pane left |
| `j/down` | Move pane down |
| `k/up` | Move pane up |
| `l/right` | Move pane right |
| `n` / `tab` | Move pane (auto-direction) |
| `p` | Move pane backwards |
| `Ctrl+h` | Exit move mode |

## SCROLL Mode (Ctrl+s)

| Key | Action |
|-----|--------|
| `e` | Edit scrollback in $EDITOR |
| `s` | Enter search mode |

### SEARCH Mode (from Scroll → s)

| Key | Action |
|-----|--------|
| `n` | Search down (next match) |
| `p` | Search up (previous match) |
| `c` | Toggle case sensitivity |
| `o` | Toggle whole word |
| `w` | Toggle wrap |

## SESSION Mode (Ctrl+o)

| Key | Action |
|-----|--------|
| `a` | About (floating plugin) |
| `c` | Configuration (floating plugin) |
| `p` | Plugin manager (floating plugin) |
| `s` | Share session |
| `w` | Session manager (floating plugin) |
| `Ctrl+o` | Exit session mode |

## TMUX Mode (Ctrl+b) — Tmux Compatibility

| Key | Action |
|-----|--------|
| `h/j/k/l` | Move focus (vim-style) |
| `left/right/up/down` | Move focus (arrows) |
| `c` | New tab |
| `n` | Next tab |
| `p` | Previous tab |
| `1-9` | Go to tab N |
| `z` | Toggle fullscreen |
| `o` | Focus next pane |
| `"` | Split pane down (tmux syntax) |
| `%` | Split pane right (tmux syntax) |
| `d` | Detach session |
| `,` | Rename tab |
| `[` | Enter scroll mode |
| `space` | Next swap layout |
| `Ctrl+b` | Write literal Ctrl+b (for nested tmux) |

## LOCKED Mode (Ctrl+g)

| Key | Action |
|-----|--------|
| `Ctrl+g` | Unlock (return to normal mode) |

All other keybinds disabled. Auto-triggered by zellij-autolock plugin when nvim/vim/lazygit detected.

## SHARED Keybinds (All Modes Except Locked)

### Navigation
| Key | Action |
|-----|--------|
| `Alt+h` | Move focus or tab left |
| `Alt+j` | Move focus down |
| `Alt+k` | Move focus up |
| `Alt+l` | Move focus or tab right |
| `Alt+left` | Move focus or tab left |
| `Alt+right` | Move focus or tab right |
| `Alt+up` | Move focus up |
| `Alt+down` | Move focus down |

### Pane Management
| Key | Action |
|-----|--------|
| `Alt+n` | New pane |
| `Alt+f` | Toggle floating panes |
| `Alt+p` | Toggle pane in group |
| `Alt+Shift+p` | Toggle group marking |
| `Alt+[` | Previous swap layout |
| `Alt+]` | Next swap layout |
| `Alt+i` | Move tab left |
| `Alt+o` | Move tab right |

### Resize
| Key | Action |
|-----|--------|
| `Alt++` | Increase size |
| `Alt+-` | Decrease size |
| `Alt+=` | Increase size |

### Plugin Launches
| Key | Plugin |
|-----|--------|
| `Alt+v` | Harpoon (pane bookmarks) |
| `Alt+w` | Swarm Orchestrator (fleet dashboard) |
| `Alt+g` | Ghost (command palette) |
| `Alt+m` | Monocle (fullscreen focus) |
| `Alt+t` | Multitask (parallel tasks) |
| `Ctrl+y` | Room (fuzzy tab/pane switcher) |

### System
| Key | Action |
|-----|--------|
| `Ctrl+q` | Quit Zellij |
