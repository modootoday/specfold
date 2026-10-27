//! One shell command with a deadline. On timeout the whole process group is killed, so a
//! command that started children cannot keep the run waiting on their open pipes.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail(Option<i32>),
    Timeout,
    /// The command could not run: a program is missing, or the shell itself failed to start.
    CannotRun(String),
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub verdict: Verdict,
    /// The last line of output, stderr first, for the report.
    pub tail: String,
    pub elapsed: Duration,
}

/// Exit status shells use for "command not found".
const NOT_FOUND: i32 = 127;
/// How long to wait for output pipes after a kill before giving up on them.
const DRAIN: Duration = Duration::from_secs(1);

fn spawn(root: &Path, cmd: &str, env: &[(String, String)], shell: Option<&str>) -> std::io::Result<Child> {
    let (prog, flag) = if cfg!(windows) { ("cmd", "/C") } else { (shell.unwrap_or("sh"), "-c") };
    let mut c = Command::new(prog);
    c.arg(flag).arg(cmd).current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in env {
        c.env(k, v);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c.spawn()
}

fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    {
        // procps-ng 3.3 reads `kill -KILL -<pgid>` as a signal to its own group; `-s … --` does not.
        let _ = Command::new("kill").args(["-s", "KILL", "--"]).arg(format!("-{}", child.id())).status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_string(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

fn last_line(text: &str) -> Option<String> {
    text.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().chars().take(200).collect())
}

pub fn run(root: &Path, cmd: &str, env: &[(String, String)], timeout: Duration, shell: Option<&str>) -> Outcome {
    let start = Instant::now();
    let mut child = match spawn(root, cmd, env, shell) {
        Ok(c) => c,
        Err(e) => {
            return Outcome {
                verdict: Verdict::CannotRun(e.to_string()),
                tail: String::new(),
                elapsed: start.elapsed(),
            };
        }
    };
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let deadline = start + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() >= deadline => {
                kill_group(&mut child);
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    let stderr = err.recv_timeout(DRAIN).unwrap_or_default();
    let stdout = out.recv_timeout(DRAIN).unwrap_or_default();
    let tail = last_line(&stderr).or_else(|| last_line(&stdout)).unwrap_or_default();
    let verdict = match status {
        None => Verdict::Timeout,
        Some(s) if s.success() => Verdict::Pass,
        Some(s) if s.code() == Some(NOT_FOUND) => Verdict::CannotRun(format!("command not found ({tail})")),
        Some(s) => Verdict::Fail(s.code()),
    };
    Outcome { verdict, tail, elapsed: start.elapsed() }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn verdicts() {
        let root = std::env::temp_dir();
        let t = Duration::from_secs(5);
        assert_eq!(run(&root, "true", &[], t, None).verdict, Verdict::Pass);
        let failed = run(&root, "echo nope >&2; exit 3", &[], t, None);
        assert_eq!((failed.verdict, failed.tail.as_str()), (Verdict::Fail(Some(3)), "nope"));
        assert!(matches!(run(&root, "definitely-not-a-program-xyz", &[], t, None).verdict, Verdict::CannotRun(_)));
        let env = [("SPECFOLD_T".to_string(), "1".to_string())];
        assert_eq!(run(&root, "test \"$SPECFOLD_T\" = 1", &env, t, None).verdict, Verdict::Pass);
    }

    #[test]
    fn a_configured_shell_runs_its_own_syntax() {
        let root = std::env::temp_dir();
        let t = Duration::from_secs(5);
        assert_eq!(run(&root, "diff <(echo a) <(echo a)", &[], t, Some("bash")).verdict, Verdict::Pass);
    }

    #[test]
    fn a_hung_command_and_its_children_are_killed() {
        let root = std::env::temp_dir();
        let o = run(&root, "sleep 30 & sleep 30", &[], Duration::from_millis(300), None);
        assert_eq!(o.verdict, Verdict::Timeout);
        assert!(o.elapsed < Duration::from_secs(5), "{:?}", o.elapsed);
    }
}
