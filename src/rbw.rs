//! Thin wrapper around the `rbw` CLI, which talks to the running rbw-agent socket.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub user: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    pub name: String,
    pub folder: Option<String>,
    pub data: Option<Value>,
    #[serde(default)]
    pub fields: Vec<Field>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Field {
    pub name: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

fn run(args: &[&str]) -> Result<String> {
    run_with_input(args, None)
}

/// Run rbw, optionally piping `input` to its stdin (rbw reads it instead of opening an editor).
fn run_with_input(args: &[&str], input: Option<&str>) -> Result<String> {
    let mut child = Command::new("rbw")
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to run rbw, is it installed?")?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.contains("access_token") {
            bail!(
                "{err} (server session expired: press l in Config to log in, or run `rbw purge` if that fails)"
            );
        }
        bail!("{err}");
    }
    Ok(String::from_utf8(out.stdout)?)
}

pub fn list() -> Result<Vec<Entry>> {
    let mut entries: Vec<Entry> = serde_json::from_str(&run(&["list", "--raw"])?)?;
    entries.sort_by_key(|e| e.name.to_lowercase());
    Ok(entries)
}

pub fn get(id: &str) -> Result<Item> {
    Ok(serde_json::from_str(&run(&["get", "--raw", id])?)?)
}

pub fn code(id: &str) -> Result<String> {
    Ok(run(&["code", id])?.trim().to_string())
}

pub fn generate(len: usize, flag: Option<&str>) -> Result<String> {
    let len = len.to_string();
    let mut args = vec!["generate", len.as_str()];
    args.extend(flag);
    Ok(run(&args)?.trim_end().to_string())
}

/// Create a login entry with the given password; empty optional fields are left out.
pub fn add(name: &str, user: &str, uri: &str, folder: &str, password: &str) -> Result<()> {
    let mut args = vec!["add"];
    for (flag, value) in [("--uri", uri), ("--folder", folder)] {
        if !value.is_empty() {
            args.extend([flag, value]);
        }
    }
    args.extend(["--", name]);
    args.extend(Some(user).filter(|u| !u.is_empty()));
    run_with_input(&args, Some(&format!("{password}\n"))).map(drop)
}

pub fn sync() -> Result<()> {
    run(&["sync"]).map(drop)
}

pub fn lock() -> Result<()> {
    run(&["lock"]).map(drop)
}

pub fn unlocked() -> bool {
    run(&["unlocked"]).is_ok()
}

/// `rbw config show` flattened into displayable key/value pairs.
pub fn config() -> Result<Vec<(String, String)>> {
    let map: serde_json::Map<String, Value> = serde_json::from_str(&run(&["config", "show"])?)?;
    Ok(map.into_iter().map(|(k, v)| (k, display(&v))).collect())
}

pub fn display(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}
