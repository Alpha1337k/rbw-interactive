//! A small modal text form, plus the line-editing keys shared with the search box.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What submitting the form does; add a variant for each new form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    SaveEntry,
}

pub enum Event {
    None,
    Submit,
    Cancel,
}

pub struct Form {
    pub kind: Kind,
    pub title: &'static str,
    pub fields: Vec<(&'static str, String)>,
    pub focus: usize,
}

impl Form {
    pub fn new(kind: Kind, title: &'static str, labels: &[&'static str]) -> Self {
        let fields = labels.iter().map(|&l| (l, String::new())).collect();
        Self {
            kind,
            title,
            fields,
            focus: 0,
        }
    }

    pub fn value(&self, label: &str) -> &str {
        self.fields
            .iter()
            .find(|(l, _)| *l == label)
            .map_or("", |(_, v)| v.trim())
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Event {
        let n = self.fields.len();
        match key.code {
            KeyCode::Esc => return Event::Cancel,
            KeyCode::Enter => return Event::Submit,
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % n,
            KeyCode::BackTab | KeyCode::Up => self.focus = (self.focus + n - 1) % n,
            _ => {
                edit_line(&mut self.fields[self.focus].1, key);
            }
        }
        Event::None
    }
}

/// Readline-ish editing: typing, `<BS>`, `<C-u>` clear, `<C-w>` delete word.
/// Returns whether the text changed.
pub fn edit_line(text: &mut String, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('u') if ctrl => text.clear(),
        KeyCode::Char('w') if ctrl => {
            let trimmed = text.trim_end().len();
            text.truncate(text[..trimmed].rfind(' ').map_or(0, |i| i + 1));
        }
        KeyCode::Backspace => {
            text.pop();
        }
        KeyCode::Char(c) if !ctrl => text.push(c),
        _ => return false,
    }
    true
}
