---
title: Exporting to Markdown
description: Write the specs as a Markdown tree for Obsidian, MkDocs, Docusaurus or a GitHub wiki.
---

# Exporting to Markdown

> [!TIP]
> `spec export markdown --target <target> --out <dir>` writes every committed document as plain
> Markdown, with `[[references]]` turned into the target's own links.

## Targets

| Target | Layout | Links |
| --- | --- | --- |
| `obsidian` | `<owner>/<type>/<slug>.md` | `[[file#Heading\|title]]` |
| `mkdocs` | `<owner>/<type>/<slug>.md` and `index.md` | relative `.md` links; headings keep `{#anchor}` (enable `attr_list`) |
| `docusaurus` | `<owner>/<type>/<slug>.md` and `index.md` | relative `.md` links; headings keep `{#anchor}` |
| `github-wiki` | flat `<slug>.md`, `Home.md`, `_Sidebar.md` | `[title](page#heading)` |

```sh
spec export markdown --target obsidian --out vault
spec export markdown --target mkdocs --out docs/specs --banner
```

## What goes in

- Committed documents of every status. Superseded and rejected documents carry a status line.
- Documents in `.spec.local/` only with `--local`.
- Frontmatter is flattened: `id`, `type`, `status`, `title`, `aliases`, `tags`, `created`,
  `updated` and one `rel_<kind>` list per relation. Docusaurus pages also get `id`,
  `specfold_id`, `sidebar_label` and `sidebar_position`. GitHub wiki pages have no frontmatter.
- `--banner` starts each page with a note naming its source file.

## References that lead nowhere

A reference that does not resolve, names a remote, or names a document left out of the export
is written as plain text and listed in the output. `--fail-on unresolved` writes nothing and
exits 1 instead.

## The output directory

`--out` must be missing, empty, or an earlier export. Each export writes
`.specfold-export.json`, listing its files; the next export removes those files and keeps
everything else (an `.obsidian/` folder, for example). Specfold refuses a directory that holds
the repository or sits inside `.spec`, `.spec.local` or `.git`.

## Exporter plugins

`markdown` is reserved for the built-in exporter. A plugin that wants more than the compact
payload lists it in `describe`:

```json
{"protocol": "specfold.plugin.v1", "name": "site", "wants": ["body", "links"],
 "provides": {"exporters": ["html"]}}
```

`export.run` then also sends, per document, `hash` (the `source_rev` hash), `body` (the text
after the frontmatter) and `links`: each reference with its byte `span` in that body, `raw`,
`target_id`, `anchor`, `remote`, `resolved_path`, `title`, `status`, and `reason` when it does not
resolve. Bundle chapters come as `chapter_bodies`. Once the bodies pass `--max-bytes`, the rest
are sent as `body_path` for the plugin to read itself. Plugins that ask for nothing get the same
payload as before.
