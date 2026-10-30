//! L7: a document that pins a remote document by content (`pins: {<remote>:<id>: sha256:…}`)
//! is told when that document changes, using the same body hash as a translation's source_rev.

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::remotes::{self, Remote};
use crate::translation;

pub struct PinsHold;

impl Rule for PinsHold {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L7",
            title: "pinned remote documents are unchanged",
            explain: "`pins` maps a remote document (`<remote>:<id>`, see [remotes]) to the body revision it was read at, `sha256:<12 hex>`, the same hash as a translation's source_rev. When the remote directory is present and the body has changed, L7 warns: read the change, update this document if it depends on it, then record the new revision with `spec set <id> --pin <remote>:<id>`. A pin that names no document there is an error; a key that is not a remote reference, or a value that is not sha256:<12 hex>, is a warning. An absent remote directory is skipped.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let cfg = &ctx.corpus.cfg;
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let Some(pins) = d.fm().and_then(|m| m.get("pins")) else { continue };
            let line = key_line(ctx, &d.path, "pins");
            let Some(pins) = pins.as_object() else {
                out.push(Finding::warn("L7", &d.path, line, "pins is not a map of <remote>:<id> to sha256:<12 hex>"));
                continue;
            };
            for (target, rev) in pins {
                let rev = rev.as_str().unwrap_or_default();
                let fix = format!("spec set {} --pin {target}", d.id);
                if let Some(p) = translation::rev_problem(rev) {
                    out.push(Finding::warn("L7", &d.path, line, format!("pin of {target}: {p}")).fix_cmd(fix));
                    continue;
                }
                match remotes::resolve(cfg, target) {
                    Remote::Local => out.push(Finding::warn(
                        "L7",
                        &d.path,
                        line,
                        format!("pins names {target}, which is not in a configured remote; pins are for [remotes]"),
                    )),
                    Remote::Absent { .. } => {}
                    Remote::Found { .. } => {
                        if let Some((rid, now)) = remotes::rev(cfg, target)
                            && now != rev
                        {
                            out.push(
                                Finding::warn(
                                    "L7",
                                    &d.path,
                                    line,
                                    format!("{rid} changed since the pinned {rev}: review it, then pin again"),
                                )
                                .fix_cmd(fix),
                            );
                        }
                    }
                    Remote::Missing { name, id } => out.push(Finding::error(
                        "L7",
                        &d.path,
                        line,
                        format!("pin names no document in the remote {name} ({id})"),
                    )),
                    Remote::MissingAnchor { name, id, anchor } => out.push(Finding::error(
                        "L7",
                        &d.path,
                        line,
                        format!("pin {name}:{id} has no anchor #{anchor}; pin the document"),
                    )),
                    Remote::Ambiguous { name, ids } => out.push(Finding::error(
                        "L7",
                        &d.path,
                        line,
                        format!("pin is ambiguous in the remote {name}: {}", ids.join(", ")),
                    )),
                }
            }
        }
    }
}
