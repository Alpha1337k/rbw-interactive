//! Generator tab state: settings form, live regeneration and a session history.

use crate::rbw;
use anyhow::Result;

const HISTORY: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Default,
    NoSymbols,
    OnlyNumbers,
    Nonconfusables,
    Diceware,
}

impl Mode {
    const ALL: [Mode; 5] = [
        Mode::Default,
        Mode::NoSymbols,
        Mode::OnlyNumbers,
        Mode::Nonconfusables,
        Mode::Diceware,
    ];

    fn flag(self) -> Option<&'static str> {
        match self {
            Mode::Default => None,
            Mode::NoSymbols => Some("--no-symbols"),
            Mode::OnlyNumbers => Some("--only-numbers"),
            Mode::Nonconfusables => Some("--nonconfusables"),
            Mode::Diceware => Some("--diceware"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Default => "letters, numbers & symbols",
            Mode::NoSymbols => "letters & numbers",
            Mode::OnlyNumbers => "numbers only",
            Mode::Nonconfusables => "no confusable characters",
            Mode::Diceware => "diceware words",
        }
    }

    /// Possible choices per unit, measured from rbw's output (EFF list for diceware).
    fn pool(self) -> f64 {
        match self {
            Mode::Default => 94.0,
            Mode::NoSymbols => 62.0,
            Mode::OnlyNumbers => 10.0,
            Mode::Nonconfusables => 24.0,
            Mode::Diceware => 7776.0,
        }
    }

    fn shift(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|&m| m == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

/// Rows of the settings form; history rows follow them.
pub const SETTINGS: [&str; 2] = ["Length", "Type"];

pub struct Generator {
    pub chars: usize,
    pub words: usize,
    pub mode: Mode,
    /// Newest first; the first entry is the current password.
    pub history: Vec<String>,
    pub row: usize,
}

impl Generator {
    pub fn new() -> Self {
        Self {
            chars: 20,
            words: 5,
            mode: Mode::Default,
            history: Vec::new(),
            row: 0,
        }
    }

    pub fn len(&self) -> usize {
        if self.mode == Mode::Diceware {
            self.words
        } else {
            self.chars
        }
    }

    pub fn unit(&self) -> &'static str {
        match (self.mode == Mode::Diceware, self.len() == 1) {
            (true, true) => "word",
            (true, false) => "words",
            (false, _) => "characters",
        }
    }

    pub fn current(&self) -> Option<&str> {
        self.history.first().map(String::as_str)
    }

    /// The history entry under the cursor, or the current password on a settings row.
    pub fn selected(&self) -> Option<&str> {
        match self.row.checked_sub(SETTINGS.len()) {
            Some(i) => self.history.get(i).map(String::as_str),
            None => self.current(),
        }
    }

    pub fn entropy(&self) -> f64 {
        self.len() as f64 * self.mode.pool().log2()
    }

    pub fn generate(&mut self) -> Result<()> {
        let pass = rbw::generate(self.len(), self.mode.flag())?;
        self.history.insert(0, pass);
        self.history.truncate(HISTORY);
        Ok(())
    }

    pub fn move_to(&mut self, row: usize) {
        self.row = row.min(SETTINGS.len() + self.history.len() - 1);
    }

    /// Adjust the setting under the cursor; returns whether anything changed.
    pub fn adjust(&mut self, delta: isize) -> bool {
        match self.row {
            0 if self.mode == Mode::Diceware => self.words = clamp(self.words, delta, 1, 20),
            0 => self.chars = clamp(self.chars, delta, 4, 128),
            1 => self.mode = self.mode.shift(delta.signum()),
            _ => return false,
        }
        true
    }
}

fn clamp(value: usize, delta: isize, min: usize, max: usize) -> usize {
    value.saturating_add_signed(delta).clamp(min, max)
}
