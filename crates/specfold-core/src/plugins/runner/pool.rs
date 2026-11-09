//! Runs a batch of commands: identical ones once, light ones on a worker pool, heavy ones one at a
//! time in their own lane alongside it. Results come back in the order the jobs were given. Each
//! command's deadline starts when it is spawned, so time spent queued never counts against it.

use super::shell::{self, Outcome, Verdict};
use super::tools;
use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

/// Rules run in parallel and X1 and G1 each call `run`; one batch at a time keeps the process
/// within `jobs` light commands plus one heavy one.
static BATCH: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Job {
    pub cmd: String,
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// Workers for light commands; 0 means the number of CPUs, at most 8.
    pub jobs: usize,
    /// Commands matching any of these run one at a time.
    pub serial: Vec<Regex>,
    /// The shell to run commands in; `sh` when unset.
    pub shell: Option<String>,
}

impl Settings {
    fn workers(&self) -> usize {
        if self.jobs > 0 {
            return self.jobs;
        }
        std::thread::available_parallelism().map(|n| n.get().min(8)).unwrap_or(2)
    }
}

fn one(root: &Path, job: &Job, shell_prog: Option<&str>) -> Outcome {
    let missing = tools::missing_programs(root, &job.cmd);
    if !missing.is_empty() {
        return Outcome {
            verdict: Verdict::CannotRun(format!("not installed: {}", missing.join(", "))),
            tail: String::new(),
            elapsed: Duration::ZERO,
        };
    }
    shell::run(root, &job.cmd, &job.env, job.timeout, shell_prog)
}

fn drain(
    root: &Path,
    queue: &Mutex<VecDeque<usize>>,
    unique: &[&Job],
    done: &Mutex<Vec<Option<Outcome>>>,
    shell_prog: Option<&str>,
) {
    loop {
        let Some(i) = queue.lock().ok().and_then(|mut q| q.pop_front()) else { return };
        let o = one(root, unique[i], shell_prog);
        if let Ok(mut d) = done.lock() {
            d[i] = Some(o);
        }
    }
}

pub fn run(root: &Path, jobs: &[Job], settings: &Settings) -> Vec<Outcome> {
    let mut index: HashMap<&Job, usize> = HashMap::new();
    let mut unique: Vec<&Job> = vec![];
    let slots: Vec<usize> = jobs
        .iter()
        .map(|j| {
            *index.entry(j).or_insert_with(|| {
                unique.push(j);
                unique.len() - 1
            })
        })
        .collect();
    let heavy = |j: &Job| settings.serial.iter().any(|r| r.is_match(&j.cmd));
    let light: Mutex<VecDeque<usize>> = Mutex::new((0..unique.len()).filter(|&i| !heavy(unique[i])).collect());
    let serial: Mutex<VecDeque<usize>> = Mutex::new((0..unique.len()).filter(|&i| heavy(unique[i])).collect());
    let done: Mutex<Vec<Option<Outcome>>> = Mutex::new(vec![None; unique.len()]);
    let shell_prog = settings.shell.as_deref();
    let _turn = BATCH.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    std::thread::scope(|s| {
        s.spawn(|| drain(root, &serial, &unique, &done, shell_prog));
        for _ in 0..settings.workers() {
            s.spawn(|| drain(root, &light, &unique, &done, shell_prog));
        }
    });
    let done = done.into_inner().unwrap_or_default();
    slots
        .into_iter()
        .map(|i| {
            done.get(i).cloned().flatten().unwrap_or(Outcome {
                verdict: Verdict::CannotRun("not run".into()),
                tail: String::new(),
                elapsed: Duration::ZERO,
            })
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn job(cmd: &str) -> Job {
        Job { cmd: cmd.into(), env: vec![], timeout: Duration::from_secs(10) }
    }

    #[test]
    fn identical_commands_run_once_and_results_keep_order() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = "echo x >> count.txt";
        let jobs = [job(cmd), job("exit 2"), job(cmd), job("true")];
        let settings = Settings { jobs: 3, serial: vec![Regex::new("^exit").unwrap()], shell: None };
        let out = run(dir.path(), &jobs, &settings);
        let verdicts: Vec<&Verdict> = out.iter().map(|o| &o.verdict).collect();
        assert_eq!(verdicts, [&Verdict::Pass, &Verdict::Fail(Some(2)), &Verdict::Pass, &Verdict::Pass]);
        assert_eq!(std::fs::read_to_string(dir.path().join("count.txt")).unwrap(), "x\n");
    }

    #[test]
    fn a_missing_program_is_not_run() {
        let dir = tempfile::tempdir().unwrap();
        let out = run(dir.path(), &[job("no-such-tool-xyz && touch ran")], &Settings::default());
        assert!(matches!(&out[0].verdict, Verdict::CannotRun(m) if m.contains("no-such-tool-xyz")));
        assert!(!dir.path().join("ran").exists());
    }

    /// Jobs that each record how many of them were running at their start, in `dir/peak`.
    fn counted(prefix: &str, n: usize, timeout: Duration) -> Vec<Job> {
        (0..n)
            .map(|i| Job {
                cmd: format!("touch run.{prefix}{i}; ls run.* | wc -l >> peak; sleep 0.8; rm run.{prefix}{i}"),
                env: vec![],
                timeout,
            })
            .collect()
    }

    fn peak(dir: &Path) -> usize {
        let text = std::fs::read_to_string(dir.join("peak")).expect("every job appends to peak");
        text.lines().filter_map(|l| l.trim().parse::<usize>().ok()).max().expect("peak holds counts")
    }

    #[test]
    fn parallel_batches_take_turns_within_one_pool_budget() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings { jobs: 2, serial: vec![], shell: None };
        std::thread::scope(|s| {
            for prefix in ["a", "b"] {
                let (dir, settings) = (dir.path(), &settings);
                s.spawn(move || run(dir, &counted(prefix, 2, Duration::from_secs(10)), settings));
            }
        });
        assert!(peak(dir.path()) <= 2, "two batches of two ran at most two at once, saw {}", peak(dir.path()));
    }

    #[test]
    fn queued_commands_keep_their_whole_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let cap = Duration::from_millis(2000);
        let started = std::time::Instant::now();
        let out = run(dir.path(), &counted("q", 6, cap), &Settings { jobs: 2, serial: vec![], shell: None });
        assert!(started.elapsed() > cap, "six jobs two at a time outlast one cap: {:?}", started.elapsed());
        assert!(out.iter().all(|o| o.verdict == Verdict::Pass), "{out:?}");
        assert!(peak(dir.path()) <= 2, "at most two ran at once, saw {}", peak(dir.path()));
    }

    #[test]
    fn a_hang_in_the_pool_fails_at_its_cap_and_frees_its_worker() {
        let dir = tempfile::tempdir().unwrap();
        let cap = Duration::from_millis(500);
        let hang = Job { cmd: "sleep 30".into(), env: vec![], timeout: cap };
        let after = Job { cmd: "true".into(), env: vec![], timeout: cap };
        let out = run(dir.path(), &[hang, after], &Settings { jobs: 1, serial: vec![], shell: None });
        assert_eq!(out[0].verdict, Verdict::Timeout);
        assert!(out[0].elapsed >= cap && out[0].elapsed < Duration::from_secs(5), "{:?}", out[0].elapsed);
        assert_eq!(out[1].verdict, Verdict::Pass);
    }
}
