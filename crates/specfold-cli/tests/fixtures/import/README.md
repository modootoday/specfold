Import fixtures in the shapes the tools write:

- `adr-tools/`: output of `adr new` and `adr new -s 1` (adr-tools src/template.md and
  src/_adr_add_link; compare tests/supercede-existing-adr.expected in that project). Note the
  tool's spelling, "Supercedes" / "Superceded by".
- `madr2/`: MADR 2.1.2 template/template.md, with the `* Status:` and `* Date:` bullets.
- `madr4/`: MADR 4 template/adr-template.md frontmatter, with a compound status
  (`superseded by ADR-0002`).
- `adr-tools-spelling/`: adr-tools records written by hand with the usual spelling
  ("Supersedes" / "Superseded by"), a relative link between records, and a Korean file name.
- `madr-linked/`: MADR 3/4 frontmatter whose status links the replacing record
  (`superseded by [ADR-0002](0002-use-a-queue.md)`), and one naming a record outside the import.
- `log4brains/`: Log4brains docs/adr/template.md (`- Status:` bullets, dated names), its
  `index.md` and `template.md`, and a package's records under packages/<name>/docs/adr.

The decision text is placeholder; only the layout matters.

A directory this repository forbids (`**/decisions` in spec.toml) is stored with a `.fixture`
suffix; the tests copy it under its real name.
