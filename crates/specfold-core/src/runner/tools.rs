//! The programs a shell command needs, and which of them are not installed. A missing program
//! inside a substitution (`diff <(jq …) <(jq …)`) can make a check compare two empty outputs and
//! pass, so substitutions are read too.

use std::path::Path;

const BUILTINS: &[&str] = &[
    "cd", "echo", "printf", "test", "[", "[[", "true", "false", "exit", "export", "set", "unset", "read", "shift",
    "return", "local", "eval", "source", ".", ":", "pwd", "type", "wait", "trap", "alias", "ulimit", "umask", "let",
    "declare", "readonly", "break", "continue", "getopts", "hash", "kill",
];
/// Words after which the next word is a command.
const LEADING: &[&str] =
    &["if", "then", "else", "elif", "while", "until", "do", "!", "{", "(", "exec", "command", "nohup"];
/// Words that open a construct whose first word is not a command.
const OPAQUE: &[&str] = &["for", "case", "select", "function", "fi", "done", "esac", "}", ")", "in"];
/// Wrappers that take options or a value before the command they run.
const WRAPPERS: &[&str] = &["env", "time", "nice", "timeout", "xargs", "sudo"];

/// Splits off `$( … )`, `<( … )`, `>( … )` and backtick bodies, returning the outer text with
/// each replaced by a placeholder word, and the bodies.
fn substitutions(cmd: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = cmd.chars().collect();
    let (mut outer, mut inner) = (String::new(), vec![]);
    let (mut single, mut i) = (false, 0);
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            single = !single;
        }
        let opens = !single && i + 1 < chars.len() && matches!(c, '$' | '<' | '>') && chars[i + 1] == '(';
        if opens || (!single && c == '`') {
            let start = if c == '`' { i + 1 } else { i + 2 };
            let mut depth = 1;
            let mut j = start;
            while j < chars.len() && depth > 0 {
                match (c, chars[j]) {
                    ('`', '`') => depth = 0,
                    (_, '(') if c != '`' => depth += 1,
                    (_, ')') if c != '`' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            let end = if depth == 0 { j - 1 } else { j };
            inner.push(chars[start..end].iter().collect());
            outer.push_str(" _sub_ ");
            i = j;
            continue;
        }
        outer.push(c);
        i += 1;
    }
    (outer, inner)
}

/// An `&` that belongs to a redirection (`2>&1`, `<&0`, `&>file`, `>&2`) rather than a list.
fn redirect_amp(chars: &[char], i: usize) -> bool {
    let prev = i.checked_sub(1).map(|p| chars[p]);
    let next = chars.get(i + 1).copied();
    matches!(prev, Some('>' | '<')) || (next == Some('>') && prev != Some('&'))
}

/// Splits on unquoted `&&`, `||`, `;`, `|`, `&` and newlines.
fn segments(cmd: &str) -> Vec<String> {
    let chars: Vec<char> = cmd.chars().collect();
    let (mut out, mut cur) = (vec![], String::new());
    let (mut single, mut double) = (false, false);
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '&' if !single && !double && redirect_amp(&chars, i) => {}
            ';' | '|' | '&' | '\n' if !single && !double => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur);
    out
}

/// Whitespace-separated words with quotes removed.
fn words(segment: &str) -> Vec<String> {
    let (mut out, mut cur) = (vec![], String::new());
    let (mut single, mut double) = (false, false);
    for c in segment.chars() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            c if c.is_whitespace() && !single && !double => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn is_assignment(w: &str) -> bool {
    w.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// The word in a segment that names the program it runs, if one can be read without guessing.
fn program(segment: &str) -> Option<String> {
    let mut ws = words(segment).into_iter().peekable();
    while let Some(w) = ws.next() {
        if OPAQUE.contains(&w.as_str()) {
            return None;
        }
        if LEADING.contains(&w.as_str()) || is_assignment(&w) {
            continue;
        }
        if WRAPPERS.contains(&w.as_str()) {
            while ws.peek().is_some_and(|n| {
                n.starts_with('-')
                    || is_assignment(n)
                    || n.chars().all(|c| c.is_ascii_digit() || c == '.' || c == 's' || c == 'm')
            }) {
                ws.next();
            }
            continue;
        }
        if w == "_sub_" || w.contains(['$', '*', '?', '<', '>']) || BUILTINS.contains(&w.as_str()) {
            return None;
        }
        return Some(w);
    }
    None
}

fn installed(root: &Path, prog: &str) -> bool {
    if prog.contains('/') {
        let p = Path::new(prog);
        return if p.is_absolute() { p.exists() } else { root.join(p).exists() };
    }
    let Some(path) = std::env::var_os("PATH") else { return true };
    std::env::split_paths(&path).any(|dir| is_executable(&dir.join(prog)))
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Programs the command needs that cannot be found. Words that cannot be read without
/// running the shell (variables, globs) are not judged.
pub fn missing_programs(root: &Path, cmd: &str) -> Vec<String> {
    if cfg!(windows) {
        return vec![];
    }
    let (outer, inner) = substitutions(cmd);
    let mut out: Vec<String> =
        segments(&outer).iter().filter_map(|s| program(s)).filter(|p| !installed(root, p)).collect();
    for body in inner {
        out.extend(missing_programs(root, &body));
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progs(cmd: &str) -> Vec<String> {
        let (outer, inner) = substitutions(cmd);
        let mut v: Vec<String> = segments(&outer).iter().filter_map(|s| program(s)).collect();
        for b in inner {
            v.extend(progs(&b));
        }
        v
    }

    #[test]
    fn reads_the_programs_a_command_runs() {
        assert_eq!(progs("cargo test -q && grep -c x f | wc -l"), ["cargo", "grep", "wc"]);
        assert_eq!(progs("FOO=1 BAR=2 node scripts/check.mjs"), ["node"]);
        assert_eq!(progs("env -i PATH=/bin jq . a.json"), ["jq"]);
        assert_eq!(progs("diff <(jq -S . a) <(yq -o json b)"), ["diff", "jq", "yq"]);
        assert_eq!(progs("test \"$(rg -c x f)\" = 0"), ["rg"]);
        assert_eq!(progs("! grep -q 'a | b; c' file"), ["grep"]);
        assert_eq!(progs("echo \"x && y\""), Vec::<String>::new());
        assert_eq!(progs("$TOOL run"), Vec::<String>::new());
        assert_eq!(progs("if [ -f a ]; then make; fi"), ["make"]);
        assert_eq!(progs("timeout 30 cargo test"), ["cargo"]);
    }

    #[test]
    fn redirections_are_not_commands() {
        assert_eq!(progs("[ -d a ] && ls a/*.toml >/dev/null 2>&1"), ["ls"]);
        assert_eq!(progs("make &> build.log && cat build.log"), ["make", "cat"]);
        assert_eq!(progs("echo x >&2; jq . a <&0"), ["jq"]);
        assert_eq!(progs("sleep 1 & wait"), ["sleep"]);
    }

    #[cfg(unix)]
    #[test]
    fn reports_only_missing_programs() {
        let root = std::env::temp_dir();
        assert!(missing_programs(&root, "sh -c true && cat /dev/null").is_empty());
        assert_eq!(missing_programs(&root, "diff <(no-such-tool-xyz a) b"), ["no-such-tool-xyz"]);
        assert_eq!(missing_programs(&root, "./no/such/script.sh"), ["./no/such/script.sh"]);
    }
}
