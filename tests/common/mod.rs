//! Shared helpers: a temp FB_HOME, a temp git repo, and a clean-environment `fb` runner.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command as Proc;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

pub struct Env {
    pub home: TempDir,
    pub work: TempDir,
    pub work_path: PathBuf,
}

#[derive(Clone, Copy)]
pub enum Who {
    /// No agent markers, `--as-human`.
    Human,
    /// Paseo agent with this id.
    Agent(&'static str),
}

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not one JSON value ({e}): {:?}", self.stdout))
    }

    /// `data` of a successful envelope.
    pub fn data(&self) -> Value {
        let v = self.json();
        assert_eq!(v["ok"], true, "expected ok, got {v}");
        v["data"].clone()
    }

    pub fn error_code(&self) -> String {
        let v = self.json();
        assert_eq!(v["ok"], false, "expected error, got {v}");
        v["error"]["code"].as_str().unwrap().to_string()
    }
}

impl Env {
    /// A fresh board and a git repo with one commit on `main` and a GitHub-style origin.
    pub fn new() -> Env {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let work_path = work.path().canonicalize().unwrap();
        let env = Env {
            home,
            work,
            work_path,
        };
        env.git(&["init", "-q", "-b", "main"]);
        env.git(&["remote", "add", "origin", "git@github.com:Org/Repo.git"]);
        env.write("README.md", "hello\n");
        env.commit("initial");
        std::fs::write(env.home.path().join("config.toml"), "gh = false\n").unwrap();
        env
    }

    pub fn git(&self, args: &[&str]) -> String {
        git_in(&self.work_path, args)
    }

    pub fn write(&self, name: &str, body: &str) {
        std::fs::write(self.work_path.join(name), body).unwrap();
    }

    pub fn commit(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", msg]);
    }

    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    pub fn cmd(&self, who: Who, args: &[&str]) -> Command {
        self.cmd_in(&self.work_path, who, args)
    }

    pub fn cmd_in(&self, dir: &Path, who: Who, args: &[&str]) -> Command {
        let mut c = Command::cargo_bin("fb").unwrap();
        c.current_dir(dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", self.home.path())
            .env("FB_HOME", self.home.path())
            .env("FB_TEST_CHAIN", "")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .args(args);
        match who {
            Who::Human => {
                c.arg("--as-human");
            }
            Who::Agent(id) => {
                c.env("PASEO_AGENT_ID", id).env("AI_AGENT", "codex");
            }
        }
        c
    }

    /// Run with `--json` and capture everything.
    pub fn run(&self, who: Who, args: &[&str]) -> Run {
        self.run_in(&self.work_path, who, args)
    }

    pub fn run_in(&self, dir: &Path, who: Who, args: &[&str]) -> Run {
        let mut all = args.to_vec();
        all.push("--json");
        finish(self.cmd_in(dir, who, &all))
    }

    /// Run in text mode.
    pub fn text(&self, who: Who, args: &[&str]) -> Run {
        finish(self.cmd(who, args))
    }

    /// Run and assert success; returns `data`.
    pub fn ok(&self, who: Who, args: &[&str]) -> Value {
        let r = self.run(who, args);
        assert_eq!(r.code, 0, "{args:?} failed: {} {}", r.stdout, r.stderr);
        r.data()
    }

    /// Run, assert a specific exit code and error code.
    pub fn fails(&self, who: Who, args: &[&str], exit: i32, code: &str) {
        let r = self.run(who, args);
        assert_eq!(r.code, exit, "{args:?}: {} {}", r.stdout, r.stderr);
        assert_eq!(r.error_code(), code, "{args:?}: {}", r.stdout);
    }

    /// Add a human todo task and return its id ("fb-1").
    pub fn todo(&self, title: &str) -> String {
        let d = self.ok(Who::Human, &["add", title]);
        d["task"]["id"].as_str().unwrap().to_string()
    }
}

fn finish(mut c: Command) -> Run {
    let out = c.output().unwrap();
    Run {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

pub fn git_in(dir: &Path, args: &[&str]) -> String {
    let out = Proc::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
