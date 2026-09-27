//! Vault tab state: entry list, grep-style filter and the selected entry's fields.

use crate::rbw::{self, Entry, Item};
use ratatui::widgets::ListState;
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;

const SECRET_KEYS: &[&str] = &[
    "password",
    "totp",
    "number",
    "code",
    "ssn",
    "passport_number",
    "license_number",
    "private_key",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Detail,
}

pub struct Row {
    pub label: String,
    pub value: String,
    pub secret: bool,
}

pub struct Vault {
    pub entries: Vec<Entry>,
    pub filtered: Vec<usize>,
    pub list: ListState,
    pub fields: ListState,
    pub query: String,
    pub regex: Option<Regex>,
    pub searching: bool,
    pub focus: Focus,
    pub revealed: bool,
    pub page: usize,
    cache: HashMap<String, Result<Item, String>>,
}

impl Vault {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut v = Self {
            entries,
            filtered: Vec::new(),
            list: ListState::default(),
            fields: ListState::default(),
            query: String::new(),
            regex: None,
            searching: false,
            focus: Focus::List,
            revealed: false,
            page: 10,
            cache: HashMap::new(),
        };
        v.refilter();
        v
    }

    pub fn set_entries(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
        self.cache.clear();
        self.refilter();
    }

    /// Smart-case regex like ripgrep; invalid patterns fall back to a literal match.
    pub fn refilter(&mut self) {
        let insensitive = !self.query.chars().any(char::is_uppercase);
        let build = |p: &str| {
            RegexBuilder::new(p)
                .case_insensitive(insensitive)
                .build()
                .ok()
        };
        self.regex = match self.query.as_str() {
            "" => None,
            q => build(q).or_else(|| build(&regex::escape(q))),
        };
        self.filtered = (0..self.entries.len())
            .filter(|&i| {
                self.regex
                    .as_ref()
                    .is_none_or(|re| re.is_match(&haystack(&self.entries[i])))
            })
            .collect();
        self.select(Some(0));
    }

    pub fn selected(&self) -> Option<&Entry> {
        self.list
            .selected()
            .and_then(|i| self.filtered.get(i))
            .map(|&i| &self.entries[i])
    }

    pub fn select(&mut self, i: Option<usize>) {
        let before = self.selected().map(|e| e.id.clone());
        let i = i
            .filter(|_| !self.filtered.is_empty())
            .map(|i| i.min(self.filtered.len() - 1));
        self.list.select(i);
        if self.selected().map(|e| &e.id) != before.as_ref() {
            self.revealed = false;
            self.fields.select(Some(0));
        }
    }

    pub fn move_by(&mut self, delta: isize) {
        let cur = self.list.selected().unwrap_or(0) as isize;
        self.select(Some(cur.saturating_add(delta).max(0) as usize));
    }

    /// Selected entry's details, fetched from rbw once and cached.
    pub fn item(&mut self) -> Option<&Result<Item, String>> {
        let id = self.selected()?.id.clone();
        Some(
            self.cache
                .entry(id)
                .or_insert_with_key(|id| rbw::get(id).map_err(|e| e.to_string())),
        )
    }

    pub fn rows(&mut self) -> Vec<Row> {
        match self.item() {
            Some(Ok(item)) => rows(item),
            _ => Vec::new(),
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

fn haystack(e: &Entry) -> String {
    let (user, folder) = (
        e.user.as_deref().unwrap_or(""),
        e.folder.as_deref().unwrap_or(""),
    );
    format!("{folder}/{} {user}", e.name)
}

/// Flatten an item into labelled rows; secrets are marked so the UI can star them.
fn rows(item: &Item) -> Vec<Row> {
    let mut out = Vec::new();
    let mut push = |label: &str, value: String, secret: bool| {
        out.push(Row {
            label: label.to_string(),
            value,
            secret,
        })
    };
    if let Some(serde_json::Value::Object(data)) = &item.data {
        for (key, value) in data {
            match value {
                serde_json::Value::Null => {}
                serde_json::Value::Array(uris) if key == "uris" => {
                    for u in uris.iter().filter_map(|u| u["uri"].as_str()) {
                        push("uri", u.to_string(), false);
                    }
                }
                v => push(key, rbw::display(v), SECRET_KEYS.contains(&key.as_str())),
            }
        }
    }
    for f in &item.fields {
        let name = f.name.as_deref().unwrap_or("field");
        push(
            name,
            f.value.clone().unwrap_or_default(),
            f.kind.as_deref() == Some("hidden"),
        );
    }
    if let Some(notes) = &item.notes {
        push("notes", notes.clone(), false);
    }
    out
}
