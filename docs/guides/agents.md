---
title: Using with AI agents
description: Instruction lines, MCP, JSON output and agent assets.
redirect_from: [/agents/]
---

# Using with AI agents

> [!TIP]
> Give the agent the lines from `spec agents`, or serve every command over MCP with `spec mcp`.
> Every command takes `--json` and prints the next command to run.

## The agent loop

1. **Read.** `spec context <path> --json` lists the documents that govern a file.
2. **Change.** `spec edit` and `spec set` update a document without hand-editing it.
3. **Check.** `spec check --staged -q` runs before the commit.

## Instructions and MCP

```console
$ spec agents --for claude
$ spec mcp
$ spec help --json
```

`spec agents --for <claude|codex|gemini|cursor|copilot|generic>` prints the lines for an agent's
instruction file. `spec mcp` serves every command as an MCP tool over stdio. `spec help --json`
describes every command, argument, example and output schema.

`spec mcp` replaces built-in secrets (keys, tokens, connection strings with a password, JWTs)
with `[redacted: <pattern>]` and reports the count as `redacted`. `spec edit` and `spec set`
refuse text that carries such a marker, so a redacted value is never written back.
`[mcp] redact = false` turns redaction off.

## Agent assets

Skills, rules, subagents and instruction files (`AGENTS.md`, `CLAUDE.md`) are declared in
`spec.toml` and read beside the documents. They carry no ids.

```toml
[[assets]]
kind = "skill"            # skill, rule, agent or instructions
path = ".agents/skills"
```

`spec assets` lists what was found and the documents each asset names. `spec check` reports
asset front matter (**A1**), duplicate names (**A2**) and paths or `spec` commands that do not
exist (**A3**).

## Copies that never drift

A tool copy can declare its source:

```toml
[[assets]]
kind = "skill"
path = ".claude/skills"
copy_of = ".agents/skills"
```

`spec sync assets` previews the copies; `--write` applies them, only inside roots with
`copy_of`. Files the source lacks stay unless `--prune` (with `--yes`, since it deletes). `spec audit --kind drift` names copies
that differ.

## This site for agents

Every page has its Markdown source at `<page>/index.md`, listed in [llms.txt](../llms.txt).
