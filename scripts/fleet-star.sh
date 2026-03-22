#!/bin/bash
# fleet-star.sh — Star graph of Claude Code instance status
# Gen 6: Burn-rate coloring, ORAC/SX/POVM stats, r-trend, auto-delegate, --watch, task tracking
# Usage: fleet-star.sh [--watch N] [--delegate] [--no-scan]
set -euo pipefail

# ── Args ──
WATCH=0; DELEGATE=0; NO_SCAN=0
while [[ $# -gt 0 ]]; do
    case $1 in
        --watch)   WATCH="${2:-10}"; shift 2 ;;
        --delegate) DELEGATE=1; shift ;;
        --no-scan) NO_SCAN=1; shift ;;
        *) shift ;;
    esac
done

# ── Colors ──
RED='\033[1;31m'; GRN='\033[1;32m'; YEL='\033[1;33m'
CYN='\033[1;36m'; WHT='\033[1;37m'
DIM='\033[2m'; RST='\033[0m'

declare -A PANE_STATUS PANE_TOKENS

# ── Scan ──
check_pane() {
    local tab=$1 pos=$2 label=$3
    local file="/tmp/star-${label}.txt"
    zellij action go-to-tab "$tab" 2>/dev/null; sleep 0.05
    case $pos in
        left)  zellij action move-focus left  2>/dev/null ;;
        tr)    zellij action move-focus right 2>/dev/null; sleep 0.03; zellij action move-focus up 2>/dev/null ;;
        br)    zellij action move-focus down  2>/dev/null ;;
        right) zellij action move-focus right 2>/dev/null ;;
        up)    zellij action move-focus up    2>/dev/null ;;
        down)  zellij action move-focus down  2>/dev/null ;;
    esac
    sleep 0.05; zellij action dump-screen "$file" 2>/dev/null
    local tok; tok=$(grep -oP '[0-9]+ tokens' "$file" 2>/dev/null | tail -1 | grep -oP '[0-9]+' || echo "0")
    local is_idle; is_idle=$(tail -3 "$file" 2>/dev/null | grep -c "^❯" || true)
    PANE_TOKENS[$label]="$tok"
    [[ "$is_idle" -gt 0 ]] && PANE_STATUS[$label]="idle" || PANE_STATUS[$label]="busy"
}

do_scan() {
    check_pane 1 right "O-R";  check_pane 1 up "O-U";  check_pane 1 down "O-D"
    check_pane 4 left "A-L";   check_pane 4 tr "A-TR";  check_pane 4 br "A-BR"
    check_pane 5 left "B-L";   check_pane 5 tr "B-TR";  check_pane 5 br "B-BR"
    check_pane 6 left "G-L";   check_pane 6 tr "G-TR";  check_pane 6 br "G-BR"
    zellij action go-to-tab 1 2>/dev/null; zellij action move-focus left 2>/dev/null
}

# ── Icons with burn-rate coloring ──
icon() {
    local label=$1 t="${PANE_TOKENS[$1]:-0}"
    if [[ "${PANE_STATUS[$label]}" == "idle" ]]; then
        echo -e "${DIM}○${RST}"; return
    fi
    # Color by token burn: green <100K, yellow 100-150K, red >150K
    if [[ "$t" -gt 150000 ]]; then echo -e "${RED}●${RST}"
    elif [[ "$t" -gt 100000 ]]; then echo -e "${YEL}●${RST}"
    else echo -e "${GRN}●${RST}"
    fi
}

tok() {
    local t="${PANE_TOKENS[$1]:-0}"
    if [[ "$t" -gt 150000 ]]; then echo -e "${RED}$((t/1000))K${RST}"
    elif [[ "$t" -gt 100000 ]]; then echo -e "${YEL}$((t/1000))K${RST}"
    elif [[ "$t" -gt 1000 ]]; then echo -e "${GRN}$((t/1000))K${RST}"
    else echo -e "${DIM}${t}${RST}"
    fi
}

lbl() {
    [[ "${PANE_STATUS[$1]}" == "busy" ]] && echo -e "${WHT}$1${RST}" || echo -e "${DIM}$1${RST}"
}

# ── Service probes ──
probe_services() {
    SVC_OK=0
    for p in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 8133 9001 10001; do
        local hp="/health"
        [[ "$p" == "8080" || "$p" == "8090" ]] && hp="/api/health"
        [[ "$(curl -s -o /dev/null -w '%{http_code}' "localhost:$p$hp" 2>/dev/null)" == "200" ]] && ((SVC_OK++)) || true
    done

    # PV2 field
    PV_DATA=$(curl -s localhost:8132/health 2>/dev/null || echo '{}')
    PV_R=$(echo "$PV_DATA" | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'{d.get(\"r\",0):.3f}')" 2>/dev/null || echo "?.???")
    PV_SPH=$(echo "$PV_DATA" | python3 -c "import sys,json;print(json.load(sys.stdin).get('spheres',0))" 2>/dev/null || echo "?")
    PV_TICK=$(echo "$PV_DATA" | python3 -c "import sys,json;print(json.load(sys.stdin).get('tick',0))" 2>/dev/null || echo "?")

    # R trend (compare to previous)
    local prev_r_file="/tmp/star-prev-r.txt"
    R_TREND="─"
    if [[ -f "$prev_r_file" ]]; then
        local prev_r; prev_r=$(cat "$prev_r_file")
        local cmp; cmp=$(python3 -c "print('↑' if float('$PV_R') > float('$prev_r')+0.005 else '↓' if float('$PV_R') < float('$prev_r')-0.005 else '─')" 2>/dev/null || echo "─")
        R_TREND="$cmp"
    fi
    echo "$PV_R" > "$prev_r_file"

    # ORAC
    ORAC_ST=$(curl -s -o /dev/null -w '%{http_code}' localhost:8133/health 2>/dev/null || echo "000")
    [[ "$ORAC_ST" == "200" ]] && ORAC_ST="${GRN}UP${RST}" || ORAC_ST="${RED}DN${RST}"

    # SYNTHEX thermal
    SX_T=$(curl -s localhost:8090/v3/thermal 2>/dev/null | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'{d.get(\"temperature\",0):.2f}')" 2>/dev/null || echo "?.??")

    # Fix reports count
    FIX_REPORTS=$(/usr/bin/find ~/projects/shared-context/tasks/ -name "fix-*" 2>/dev/null | wc -l)

    # POVM pathway count
    POVM_MEM=$(curl -s localhost:8125/hydrate 2>/dev/null | python3 -c "import sys,json;d=json.load(sys.stdin);print(d.get('memory_count',0))" 2>/dev/null || echo "?")
    POVM_PATH=$(curl -s localhost:8125/hydrate 2>/dev/null | python3 -c "import sys,json;d=json.load(sys.stdin);print(d.get('pathway_count',0))" 2>/dev/null || echo "?")

    # ME fitness
    ME_FIT=$(curl -s localhost:8080/api/observer 2>/dev/null | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'{d.get(\"last_report\",{}).get(\"current_fitness\",0):.3f}')" 2>/dev/null || echo "?")

    # Task completion: count checked items in queue files
    TASKS_DONE=0; TASKS_TOTAL=0
    for qf in ~/projects/shared-context/tasks/queue-*.md; do
        [[ -f "$qf" ]] || continue
        local completed; completed=$(grep -c '^\- \[x\]' "$qf" 2>/dev/null || true)
        local total; total=$(grep -c '^\- \[' "$qf" 2>/dev/null || true)
        TASKS_DONE=$((TASKS_DONE + completed))
        TASKS_TOTAL=$((TASKS_TOTAL + total))
    done
    TASKS_REMAIN=$((TASKS_TOTAL - TASKS_DONE))

    # Context exhaustion ETA (1M context, tokens consumed)
    TOTAL_TOKENS=0
    for k in "${!PANE_TOKENS[@]}"; do
        TOTAL_TOKENS=$((TOTAL_TOKENS + PANE_TOKENS[$k]))
    done
    # Each pane has ~1M limit; panes above 800K are at risk
    PANES_HOT=0
    for k in "${!PANE_TOKENS[@]}"; do
        [[ "${PANE_TOKENS[$k]}" -gt 800000 ]] && ((PANES_HOT++)) || true
    done
}

# ── Auto-delegate ──
auto_delegate() {
    local dispatched=0
    declare -A QUEUE_MAP=(
        ["O-R"]="queue-orch-right.md" ["O-U"]="queue-orch-up.md" ["O-D"]="queue-orch-down.md"
        ["A-L"]="queue-alpha-left.md" ["A-TR"]="queue-alpha-tr.md" ["A-BR"]="queue-alpha-br.md"
        ["B-L"]="queue-beta-left.md" ["B-TR"]="queue-beta-tr.md" ["B-BR"]="queue-beta-br.md"
        ["G-L"]="queue-gamma-left.md" ["G-TR"]="queue-gamma-tr.md" ["G-BR"]="queue-gamma-br.md"
    )
    declare -A TAB_MAP=(["O-R"]="1:right" ["O-U"]="1:up" ["O-D"]="1:down"
        ["A-L"]="4:left" ["A-TR"]="4:tr" ["A-BR"]="4:br"
        ["B-L"]="5:left" ["B-TR"]="5:tr" ["B-BR"]="5:br"
        ["G-L"]="6:left" ["G-TR"]="6:tr" ["G-BR"]="6:br")

    for label in "${!PANE_STATUS[@]}"; do
        if [[ "${PANE_STATUS[$label]}" == "idle" ]]; then
            local queue="${QUEUE_MAP[$label]:-}"
            local tabpos="${TAB_MAP[$label]:-}"
            [[ -z "$queue" || -z "$tabpos" ]] && continue
            local tab="${tabpos%%:*}" pos="${tabpos##*:}"

            zellij action go-to-tab "$tab" 2>/dev/null; sleep 0.05
            case $pos in
                left)  zellij action move-focus left 2>/dev/null ;;
                right) zellij action move-focus right 2>/dev/null ;;
                up)    zellij action move-focus up 2>/dev/null ;;
                down)  zellij action move-focus down 2>/dev/null ;;
                tr)    zellij action move-focus right 2>/dev/null; sleep 0.03; zellij action move-focus up 2>/dev/null ;;
                br)    zellij action move-focus down 2>/dev/null ;;
            esac
            sleep 0.05
            zellij action write-chars "Read ~/projects/shared-context/tasks/${queue} and work on the next uncompleted task. Run quality gate after code changes." && zellij action write 13
            ((dispatched++))
            PANE_STATUS[$label]="dispatched"
        fi
    done
    zellij action go-to-tab 1 2>/dev/null; zellij action move-focus left 2>/dev/null
    DELEGATE_COUNT=$dispatched
}

# ── Render ──
render() {
    local BUSY_COUNT=0 IDLE_COUNT=0 IDLE_LIST=""
    for k in "${!PANE_STATUS[@]}"; do
        if [[ "${PANE_STATUS[$k]}" == "busy" || "${PANE_STATUS[$k]}" == "dispatched" ]]; then
            ((BUSY_COUNT++))
        else
            ((IDLE_COUNT++))
            IDLE_LIST="$IDLE_LIST $k"
        fi
    done
    ((BUSY_COUNT++)) # coordinator
    local TOTAL=$((BUSY_COUNT + IDLE_COUNT))

    # R trend color
    local R_ICON
    case "$R_TREND" in
        "↑") R_ICON="${GRN}↑${RST}" ;;
        "↓") R_ICON="${RED}↓${RST}" ;;
        *)   R_ICON="${DIM}─${RST}" ;;
    esac

    clear
    echo ""
    echo -e "${CYN}╔════════════════════════════════════════════════════════════════════╗${RST}"
    echo -e "${CYN}║${RST}  ${WHT}ULTRAPLATE FLEET — STAR GRAPH${RST}    gen $(cat /tmp/star-gen.txt 2>/dev/null || echo 1)               ${CYN}║${RST}"
    echo -e "${CYN}║${RST}  ${DIM}$(date '+%H:%M:%S')${RST}  r=${WHT}${PV_R}${RST}${R_ICON}  ${SVC_OK}/17 svc  ${PV_SPH} sph  t=${PV_TICK}        ${CYN}║${RST}"
    echo -e "${CYN}║${RST}  ORAC:${ORAC_ST}  SX:T=${SX_T}  ME:${ME_FIT}  POVM:${POVM_MEM}m/${POVM_PATH}p     ${CYN}║${RST}"
    echo -e "${CYN}║${RST}  fixes:${FIX_REPORTS}  delegate:${DELEGATE_COUNT:-0}                                     ${CYN}║${RST}"
    echo -e "${CYN}╠════════════════════════════════════════════════════════════════════╣${RST}"
    echo -e "${CYN}║${RST}                                                                    ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                      $(icon A-TR) $(lbl A-TR) $(tok A-TR)                               ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                     ╱                                              ${CYN}║${RST}"
    echo -e "${CYN}║${RST}   $(icon O-U) $(lbl O-U) $(tok O-U)   ╱    $(icon A-L) $(lbl A-L) $(tok A-L)                        ${CYN}║${RST}"
    echo -e "${CYN}║${RST}        ╲        ╱        ╲                                         ${CYN}║${RST}"
    echo -e "${CYN}║${RST}         ╲      ╱          $(icon A-BR) $(lbl A-BR) $(tok A-BR)                      ${CYN}║${RST}"
    echo -e "${CYN}║${RST}  $(icon O-R) $(lbl O-R)───${GRN}●${RST} ${WHT}COORD${RST}───────────$(icon B-L) $(lbl B-L) $(tok B-L)          ${CYN}║${RST}"
    echo -e "${CYN}║${RST}  $(tok O-R)      ╱    ╲               ╱                              ${CYN}║${RST}"
    echo -e "${CYN}║${RST}          ╱      ╲          $(icon B-TR) $(lbl B-TR) $(tok B-TR)                     ${CYN}║${RST}"
    echo -e "${CYN}║${RST}         ╱        ╲          ╱                                       ${CYN}║${RST}"
    echo -e "${CYN}║${RST}   $(icon O-D) $(lbl O-D) $(tok O-D)    $(icon G-L) $(lbl G-L) $(tok G-L)                         ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                     ╲    ╱    ╲                                     ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                      ╲  ╱      $(icon B-BR) $(lbl B-BR) $(tok B-BR)                 ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                       ╲╱                                            ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                 $(icon G-TR) $(lbl G-TR) $(tok G-TR)                                  ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                       │                                             ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                 $(icon G-BR) $(lbl G-BR) $(tok G-BR)                                  ${CYN}║${RST}"
    echo -e "${CYN}║${RST}                                                                    ${CYN}║${RST}"
    echo -e "${CYN}╠════════════════════════════════════════════════════════════════════╣${RST}"
    echo -e "${CYN}║${RST}  ${GRN}●${RST} BUSY: ${BUSY_COUNT}   ${DIM}○${RST} IDLE: ${IDLE_COUNT}   TOTAL: ${TOTAL}                          ${CYN}║${RST}"
    echo -e "${CYN}║${RST}  ${GRN}●${RST}<100K  ${YEL}●${RST}100-150K  ${RED}●${RST}>150K  ${DIM}○${RST}idle                          ${CYN}║${RST}"
    echo -e "${CYN}║${RST}  Tasks: ${GRN}${TASKS_DONE}${RST}/${TASKS_TOTAL} done  ${YEL}${TASKS_REMAIN}${RST} remain  ${DIM}${TOTAL_TOKENS}${RST} total tok          ${CYN}║${RST}"
    [[ "$PANES_HOT" -gt 0 ]] && echo -e "${CYN}║${RST}  ${RED}⚠ ${PANES_HOT} pane(s) >800K tokens — context exhaustion risk${RST}       ${CYN}║${RST}"
    [[ -n "$IDLE_LIST" ]] && echo -e "${CYN}║${RST}  ${RED}IDLE:${IDLE_LIST}${RST}                                             ${CYN}║${RST}"
    echo -e "${CYN}╚════════════════════════════════════════════════════════════════════╝${RST}"
    echo ""
}

# ── Main ──
run_once() {
    cat /tmp/star-gen.txt 2>/dev/null || echo 1 > /dev/null
    [[ "$NO_SCAN" -eq 0 ]] && do_scan
    probe_services
    DELEGATE_COUNT=0
    [[ "$DELEGATE" -eq 1 ]] && auto_delegate
    render
}

if [[ "$WATCH" -gt 0 ]]; then
    GEN=1
    while true; do
        echo "$GEN" > /tmp/star-gen.txt
        run_once
        echo -e "${DIM}Next refresh in ${WATCH}s (Ctrl+C to stop)${RST}"
        sleep "$WATCH"
        ((GEN++))
    done
else
    echo "1" > /tmp/star-gen.txt
    run_once
fi
