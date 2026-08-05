# planr-cli

CLI tool for the `planr` skill in the `agent-skills` tap. Provides
automated backlog management: ticket creation, branch claiming, structural
linting, board summaries, and merge gating.

## Installation

Prebuilt binaries: (coming soon — GitHub Releases)

From source: (coming soon — language-specific instructions)

## Usage

```bash
planr new-ticket <kind> <slug> <title>      # Scaffold a ticket file
planr board                                   # Backlog + in-flight board
planr lint                                    # Structural checks
planr claim <task>                            # Create worktree, set in_progress
planr review <task>                           # Brief a reviewer
planr merge-task <task>                       # Gate check → merge → done
```

## Development

Backlog tracked in `.plan/` — tickets use the same format this tool manages.
