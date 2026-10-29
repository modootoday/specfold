---
title: Rules
description: Every rule spec check runs, grouped by family.
---

# Rules

> [!TIP]
> Each finding names a rule id. `spec explain <id>` says why the rule exists and how to fix a
> finding; `spec check --rule <id>` runs just that rule.

## Structure

| rule | checks that |
| --- | --- |
| **S1** | frontmatter parses and has the required keys |
| **S2** | id matches the path |
| **S3** | every file in a spec directory belongs to a document |
| **S4** | status is one the type allows |
| **S5** | dates are ISO and updated is not before created |
| **S6** | a status has the fields it requires |
| **S7** | a document has the sections its type requires (`spec new` writes them; the finding names the `spec edit --new-section` command) |

## Identity

| rule | checks that |
| --- | --- |
| **I1** | ids are unique across every root |
| **I2** | aliases do not shadow ids and point at one document |
| **I3** | the alias table gains no rows |

## Links

| rule | checks that |
| --- | --- |
| **L1** | every [[reference]] resolves to one document |
| **L2** | documents are cited by id, not by path |
| **L3** | relations resolve and point at the right types |
| **L4** | anchors are unique within a document |
| **L5** | governed paths exist |
| **L6** | citations outside documents still resolve |

## Lifecycle

| rule | checks that |
| --- | --- |
| **T1** | superseded documents have exactly one successor |
| **T2** | status only moves forward |
| **T3** | sealed documents keep their body |
| **T4** | an edited body moves its updated date |

## Visibility

| rule | checks that |
| --- | --- |
| **V1** | no spec documents under public paths |
| **V2** | committed documents do not cite local ones |
| **V3** | external notes cite documents that exist |

## Placement and attachments

| rule | checks that |
| --- | --- |
| **P1** | documents live with the owner of the code they govern |
| **P2** | every attachment is registered |
| **P3** | repository attachments stay small |
| **E1** | recorded attachments carry a matching hash |
| **E2** | stored attachments exist in the store |

## Retired layouts

| rule | checks that |
| --- | --- |
| **Z1** | retired layouts and identifiers stay retired |

## Verification

| rule | checks that |
| --- | --- |
| **X1** | a source of truth's verify commands pass |
| **X2** | an active source of truth says how it is verified |

## Generated documents

| rule | checks that |
| --- | --- |
| **G1** | generated documents match their generator |

## Guards

| rule | checks that |
| --- | --- |
| **U1** | guards pass |

## Agent assets

| rule | checks that |
| --- | --- |
| **A1** | agent assets have the frontmatter their tools read |
| **A2** | asset names are unique within a tool |
| **A3** | paths and spec commands in agent assets exist |
| **A4** | agent assets use repository paths, not machine paths |

## Numbers

| rule | checks that |
| --- | --- |
| **N1** | counts in spec:count markers are current |

## Hidden characters

| rule | checks that |
| --- | --- |
| **H1** | no invisible characters in documents, declared agent assets, or well-known agent files |

Well-known agent files are checked even when they are not declared: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.agents/skills/*/SKILL.md`, `.claude/skills/*/SKILL.md`, `.claude/agents/*.md`, `.claude/commands/*.md`, and `.cursor/rules/*.mdc`. Joiners used inside emoji (U+200C, U+200D) are allowed. Output never prints the characters themselves: messages, fixes, previews, and editor quick fixes show them as `\u{XXXX}`, and in an editor the warning covers just the one character. `spec fix` shows each changed line before and after.

A4 (machine-local paths) checks the same files as H1: declared agent assets and the well-known agent files, each once.

Plugin rules are named `x.<plugin>.<id>` and are explained the same way.
