//! Compiles and runs one playground program in a throwaway Docker container: no network, a
//! read only root file system with a small `/tmp`, no capabilities, an unprivileged user and
//! limits on memory, CPU, processes and time. The host never compiles or runs the code itself.

use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// The longest page kept from a run, in bytes.
const MAX_PAGE: u64 = 1024 * 1024;

/// The longest compiler log kept from a run, in bytes.
const MAX_LOG: u64 = 64 * 1024;

/// How long the host waits for a container before it removes it. The script inside stops
/// compiling at 17 s and running at 3 s more.
const HOST_LIMIT: Duration = Duration::from_secs(25);

/// What a finished run gave.
pub struct Outcome {
    /// The HTML of the rendered page.
    pub page: String,
    /// The timing line of the sandbox, such as `compile_ms=2100 run_ms=12 memory_peak=...`.
    pub stats: String,
}

/// Why a run gave no page.
pub enum Failure {
    /// The code did not compile; the compiler's messages.
    Compile(String),
    /// The run went over its time or memory limit and was stopped.
    Limit,
    /// The sandbox failed for another reason.
    Sandbox(String),
}

/// The Rust file compiled for a screen body: the body becomes the body of a public screen at `/`.
pub fn source(body: &str) -> String {
    format!(
        "use zeyn::prelude::*;\n\n#[screen(route = \"/\", public)]\npub fn playground() -> Screen {{\n{body}\n}}\n\nfn main() {{\n    zeyn_playground_harness::print_page(\n        App::new(\"Playground\")\n            .home(playground)\n            .feature(Feature::new(\"playground\").screen(playground)),\n    );\n}}\n"
    )
}

/// The first line of the body in the file [`source`] builds.
const BODY_LINE: usize = 5;

/// Points compiler messages at lines of the body: `main.rs:7:9` becomes `body:3:9`.
pub fn body_lines(log: &str) -> String {
    let mut out = String::with_capacity(log.len());
    let mut rest = log;
    while let Some(at) = rest.find("main.rs:") {
        out.push_str(&rest[..at]);
        let after = &rest[at + "main.rs:".len()..];
        let digits = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        match after[..digits].parse::<usize>() {
            Ok(n) if n >= BODY_LINE => {
                out.push_str(&format!("body:{}", n + 1 - BODY_LINE));
            }
            _ => out.push_str(&format!("main.rs:{}", &after[..digits])),
        }
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

/// Compiles and runs `source` in a new container named `name`, from the image `image`.
pub async fn run(image: &str, name: &str, source: &str) -> Result<Outcome, Failure> {
    let mut child = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-i",
            "--name",
            name,
            "--network",
            "none",
            "--read-only",
            "--tmpfs",
            "/tmp:rw,exec,nosuid,nodev,size=256m",
            "--memory",
            "1g",
            "--memory-swap",
            "1g",
            "--cpus",
            "2",
            "--pids-limit",
            "256",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--ulimit",
            "fsize=268435456",
            image,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| Failure::Sandbox(format!("docker did not start: {e}")))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child
        .stdout
        .take()
        .expect("piped stdout");
    let stderr = child
        .stderr
        .take()
        .expect("piped stderr");
    let input = source.as_bytes().to_vec();
    let work = async move {
        let write = async move {
            let _ = stdin.write_all(&input).await;
            drop(stdin);
        };
        let (_, page, log, status) = tokio::join!(
            write,
            read_capped(stdout, MAX_PAGE),
            read_capped(stderr, MAX_LOG),
            child.wait()
        );
        (page, log, status)
    };
    let Ok((page, log, status)) = tokio::time::timeout(HOST_LIMIT, work).await else {
        remove(name).await;
        return Err(Failure::Limit);
    };
    let code = status
        .map_err(|e| Failure::Sandbox(e.to_string()))?
        .code();
    let (log, stats) = split_stats(&log);
    match code {
        Some(0) => Ok(Outcome { page, stats }),
        Some(3) if stats.contains("compile failed") && !log.is_empty() => {
            Err(Failure::Compile(body_lines(&log)))
        }
        Some(3 | 137) => Err(Failure::Limit),
        Some(c) => Err(Failure::Sandbox(format!("exit {c}: {log}"))),
        None => Err(Failure::Limit),
    }
}

/// Reads at most `limit` bytes as text, then closes the pipe.
async fn read_capped(pipe: impl tokio::io::AsyncRead + Unpin, limit: u64) -> String {
    let mut bytes = Vec::new();
    let _ = pipe
        .take(limit)
        .read_to_end(&mut bytes)
        .await;
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Splits the `zeyn-play:` timing line from the compiler log.
fn split_stats(log: &str) -> (String, String) {
    let mut stats = String::new();
    let mut rest = String::new();
    for line in log.lines() {
        match line.strip_prefix("zeyn-play: ") {
            Some(s) => stats = s.to_string(),
            None => {
                rest.push_str(line);
                rest.push('\n');
            }
        }
    }
    (rest, stats)
}

/// Removes a container that went over the host limit.
async fn remove(name: &str) {
    let _ = Command::new("docker")
        .args(["rm", "-f", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiler_lines_point_at_the_body() {
        assert_eq!(
            super::body_lines("--> main.rs:7:9 and main.rs:2:1"),
            "--> body:3:9 and main.rs:2:1"
        );
        let file = super::source("Screen::new(\"a\")");
        let line = file
            .lines()
            .position(|l| l == "Screen::new(\"a\")");
        assert_eq!(line, Some(super::BODY_LINE - 1));
    }
}
