# /compose -- Topology-Aware Command Composition

Compose existing commands using the 5 composition algebra patterns.
Each composition wires commands with topology-aware connectors.

topology: meta

## Usage

```
/compose hourglass /intel /battern       # probe then dispatch
/compose amplified /gate /broadcast      # pipeline then broadcast
/compose mapreduce /battern /harvest     # divergent then convergent
/compose consensus /broadcast /sweep     # broadcast then verify
/compose metabolic /nerve /metabolic     # continuous mesh then evolution
```

## Patterns

| Composition | Formula | Example |
|-------------|---------|---------|
| HOURGLASS | CONVERGENT → decision → DIVERGENT | /intel then dispatch based on findings |
| MAPREDUCE | DIVERGENT → CONVERGENT | dispatch to fleet then harvest results |
| AMPLIFIED | PIPELINE → BROADCAST | quality gate then persist to all substrates |
| METABOLIC | MESH → PIPELINE | continuous monitoring then evolution cycle |
| CONSENSUS | BROADCAST → CONVERGENT | propagate then verify across substrates |

## Implementation

When the user runs `/compose <pattern> <cmd1> <cmd2>`:

1. Identify the composition pattern
2. Run cmd1 first (the "input" side)
3. Analyze cmd1 output to determine the connection logic
4. Run cmd2 with context from cmd1 (the "output" side)
5. Report the composed result

The key insight: **compositions carry fusion logic between commands**. The connector transforms cmd1's output into cmd2's input based on the pattern.

For HOURGLASS: cmd1 (convergent probe) produces a health snapshot → if fitness dropped, cmd2 dispatches investigation. If emergence spiked, cmd2 dispatches analysis.

For AMPLIFIED: cmd1 (pipeline) produces pass/fail → if pass, cmd2 broadcasts success to all memory substrates. If fail, cmd2 broadcasts the failure for learning.
