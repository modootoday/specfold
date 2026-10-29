---
title: Using with AI agents
description: Instruction lines, MCP and agent assets.
nav_order: 5
---

# Using with AI agents

## Instructions and MCP

- `spec agents --for <claude|codex|gemini|cursor|copilot|generic>` prints the lines to put in an
  agent's instruction file.
- `spec mcp` serves every command as an MCP tool over stdio.
- `spec help --json` describes every command, argument, example and output schema.

## Agent assets

Skills, rules, subagents and instruction files (AGENTS.md, CLAUDE.md) are declared in
`spec.toml` and read beside the documents. They carry no ids and specfold never writes them:

```toml
[[assets]]
kind = "skill"            # skill, rule, agent or instructions
path = ".agents/skills"   # a directory or a glob
```

`spec assets` lists what was found, with the documents each asset names by id or path; `spec show <id>` lists the assets that name a document. `spec check` reports asset frontmatter (A1: a missing name or
description is an error, the rest warnings), duplicate names (A2) and paths or `spec` commands in
asset text that do not exist (A3). `.agents/` is the source; `spec audit --kind drift` names
copies that differ from it. `spec new skill|rule|agent --title <t> [--description <d>]` writes a
template and never overwrites an existing file.
