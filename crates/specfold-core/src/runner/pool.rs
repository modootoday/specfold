//! Runs a batch of commands: identical ones once, light ones on a worker pool, heavy ones one at a
//! time in their own lane alongside it. Results come back in the order the jobs were given.

use super::shell::{self, Outcome, Verdict};
use super::tools;
use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

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
}

impl Settings {
    fn workers(&self) -> usize {
        if self.jobs > 0 {
            return self.jobs;
        }
        std::thread::available_parallelism().map(|n| n.get().min(8)).unwrap_or(2)
    }
}

fn one(root: &Path, job: &Job) -> Outcome {
    let missing = tools::missing_programs(root, &job.cmd);
    if !missing.is_empty() {
        return Outcome {
            verdict: Verdict::CannotRun(format!("not installed: {}", missing.join(", "))),
            tail: String::new(),
            elapsed: Duration::ZERO,
        };
    }
    shell::run(root, &job.cmd, &job.env, job.timeout)
}

fn drain(root: &Path, queue: &Mutex<VecDeque<usize>>, unique: &[&Job], done: &Mutex<Vec<Option<Outcome>>>) {
    loop {
        let Some(i) = queue.lock().ok().and_then(|mut q| q.pop_front()) else { return };
        let o = one(root, unique[i]);
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
    std::thread::scope(|s| {
        s.spawn(|| drain(root, &serial, &unique, &done));
        for _ in 0..settings.workers() {
            s.spawn(|| drain(root, &light, &unique, &done));
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
        let settings = Settings { jobs: 3, serial: vec![Regex::new("^exit").unwrap()] };
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
}
