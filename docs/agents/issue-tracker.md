# Issue tracker: Linear

Issues and PRDs for this repo are tracked in Linear, in the **Slay The Spire Solver** project under the **Hollowtube** team (key `HOL`).

## Conventions

- All issues for this repo go into the "Slay The Spire Solver" project (Hollowtube team)
- Triage state is recorded via labels — see `triage-labels.md` for the role-to-label mapping
- Humans may use the Linear MCP tools (`mcp__linear-server__*`) to create, read, update, and label issues, and to publish PRDs as Linear documents or issues
- Agents write through the code-task pipeline's tracker CLI (`tracker.py`: `create`, `comment`, `move`, `tag`, `describe`), which keeps lanes and labels consistent. It lives at `~/.claude/skills/code-task/scripts/tracker.py` and only dry-runs unless `--apply` is passed. Linear MCP reads are fine

## When a skill says "publish to the issue tracker"

Create a new issue (or document, for PRDs) in the "Slay The Spire Solver" project with `tracker.py create` (humans may use the Linear MCP tools), applying the appropriate triage label(s).

## When a skill says "fetch the relevant ticket"

Look up the issue in Linear by ID, title, or URL with `tracker.py get` / `list` (or the Linear MCP `get_issue`, `list_issues` reads).
