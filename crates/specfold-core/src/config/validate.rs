//! C237: patterns in spec.toml are checked when the file loads. A regex or glob that does not
//! compile used to be dropped where it was used, so the setting silently did nothing.

use super::types::RawConfig;
use crate::workspace::owners::matcher;
use regex::Regex;

/// The first pattern that does not compile: why, and the value as written (to find its line).
pub fn pattern_problem(raw: &RawConfig) -> Option<(String, String)> {
    let regexes: [(&str, &[String]); 3] = [
        ("[verify] serial", &raw.verify.serial),
        ("[forbid] patterns", &raw.forbid.patterns),
        ("[cites] history", &raw.cites.history),
    ];
    for (key, list) in regexes {
        for r in list {
            if let Err(e) = Regex::new(r) {
                let why = e.to_string().lines().last().unwrap_or("").trim().to_string();
                return Some((format!("{key} has \"{r}\", which is not a regular expression ({why})"), r.clone()));
            }
        }
    }
    let mut globs: Vec<(String, &[String])> = vec![
        ("[verify] full_run_on".into(), &raw.verify.full_run_on),
        ("[forbid] paths".into(), &raw.forbid.paths),
        ("[check] count_paths".into(), &raw.check.count_paths),
        ("[check] link_ignore".into(), &raw.check.link_ignore),
        ("[cites] paths".into(), &raw.cites.paths),
        ("[cites] exclude".into(), &raw.cites.exclude),
        ("[public] paths".into(), &raw.public.paths),
        ("[owners] retired".into(), &raw.owners.retired),
        ("[owners] pending".into(), &raw.owners.pending),
    ];
    for g in &raw.guards {
        globs.push((format!("guard '{}' paths", g.name), &g.paths));
        globs.push((format!("guard '{}' exclude", g.name), &g.exclude));
        globs.push((format!("guard '{}' deny_files", g.name), &g.deny_files));
    }
    for a in &raw.assets {
        globs.push(("[[assets]] path".into(), std::slice::from_ref(&a.path)));
        globs.push(("[[assets]] exclude".into(), &a.exclude));
    }
    for (key, list) in globs {
        if let Some(p) = list.iter().find(|p| matcher(p).is_none()) {
            return Some((
                format!("{key} has \"{p}\", which is not a glob (check its brackets and braces)"),
                p.clone(),
            ));
        }
    }
    None
}
