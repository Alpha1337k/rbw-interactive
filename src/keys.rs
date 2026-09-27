//! Vim-style keymap: bindings are key sequences per scope, e.g. "gg" or "<C-d>".
//! Add a shortcut by adding an `Action` variant and a row to `BINDINGS`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    List,
    Detail,
    Generator,
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    NextTab,
    PrevTab,
    GoTab(usize),
    Down,
    Up,
    Top,
    Bottom,
    HalfDown,
    HalfUp,
    Search,
    ClearSearch,
    FocusDetail,
    FocusList,
    Yank,
    YankPassword,
    YankUser,
    YankTotp,
    ToggleReveal,
    OpenBrowser,
    Sync,
    Lock,
    Unlock,
    Login,
    Generate,
    SaveEntry,
    Increase,
    Decrease,
    IncreaseMore,
    DecreaseMore,
}

impl Action {
    pub fn label(self) -> &'static str {
        use Action::*;
        match self {
            Quit => "quit",
            NextTab | PrevTab | GoTab(_) => "tabs",
            Down | Up | Top | Bottom | HalfDown | HalfUp => "move",
            Search => "search",
            ClearSearch => "clear",
            FocusDetail => "details",
            FocusList => "back",
            Yank => "yank",
            YankPassword => "yank pass",
            YankUser => "yank user",
            YankTotp => "yank totp",
            ToggleReveal => "reveal",
            OpenBrowser => "browser",
            Sync => "sync",
            Lock => "lock",
            Unlock => "unlock",
            Login => "login",
            Generate => "generate",
            SaveEntry => "save",
            Increase | Decrease | IncreaseMore | DecreaseMore => "adjust",
        }
    }
}

use Action::*;
use Scope::*;

pub const BINDINGS: &[(Scope, &str, Action)] = &[
    (Global, "q", Quit),
    (Global, "<C-c>", Quit),
    (Global, "<Tab>", NextTab),
    (Global, "<S-Tab>", PrevTab),
    (Global, "gt", NextTab),
    (Global, "gT", PrevTab),
    (Global, "1", GoTab(0)),
    (Global, "2", GoTab(1)),
    (Global, "3", GoTab(2)),
    (List, "/", Search),
    (List, "i", Search),
    (List, "<Esc>", ClearSearch),
    (List, "j", Down),
    (List, "k", Up),
    (List, "<Down>", Down),
    (List, "<Up>", Up),
    (List, "gg", Top),
    (List, "G", Bottom),
    (List, "<C-d>", HalfDown),
    (List, "<C-u>", HalfUp),
    (List, "l", FocusDetail),
    (List, "<CR>", FocusDetail),
    (List, "<Right>", FocusDetail),
    (List, "y", YankPassword),
    (List, "u", YankUser),
    (List, "t", YankTotp),
    (List, "o", OpenBrowser),
    (List, "r", Sync),
    (Detail, "h", FocusList),
    (Detail, "<Esc>", FocusList),
    (Detail, "<Left>", FocusList),
    (Detail, "j", Down),
    (Detail, "k", Up),
    (Detail, "<Down>", Down),
    (Detail, "<Up>", Up),
    (Detail, "gg", Top),
    (Detail, "G", Bottom),
    (Detail, "y", Yank),
    (Detail, "<CR>", Yank),
    (Detail, "s", ToggleReveal),
    (Detail, "p", YankPassword),
    (Detail, "u", YankUser),
    (Detail, "t", YankTotp),
    (Detail, "o", OpenBrowser),
    (Detail, "/", Search),
    (Generator, "j", Down),
    (Generator, "k", Up),
    (Generator, "<Down>", Down),
    (Generator, "<Up>", Up),
    (Generator, "gg", Top),
    (Generator, "G", Bottom),
    (Generator, "l", Increase),
    (Generator, "h", Decrease),
    (Generator, "<Right>", Increase),
    (Generator, "<Left>", Decrease),
    (Generator, "+", Increase),
    (Generator, "-", Decrease),
    (Generator, "L", IncreaseMore),
    (Generator, "H", DecreaseMore),
    (Generator, "y", Yank),
    (Generator, "<CR>", Yank),
    (Generator, "r", Generate),
    (Generator, "n", Generate),
    (Generator, "s", SaveEntry),
    (Config, "s", Sync),
    (Config, "U", Unlock),
    (Config, "l", Login),
    (Config, "L", Lock),
];

/// Render a key event as a vim-style token: `j`, `G`, `<C-d>`, `<CR>`.
pub fn token(key: KeyEvent) -> Option<String> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    Some(match key.code {
        KeyCode::Char(c) if ctrl => format!("<C-{c}>"),
        KeyCode::Char(' ') => "<Space>".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "<CR>".into(),
        KeyCode::Esc => "<Esc>".into(),
        KeyCode::Tab => "<Tab>".into(),
        KeyCode::BackTab => "<S-Tab>".into(),
        KeyCode::Backspace => "<BS>".into(),
        KeyCode::Up => "<Up>".into(),
        KeyCode::Down => "<Down>".into(),
        KeyCode::Left => "<Left>".into(),
        KeyCode::Right => "<Right>".into(),
        _ => return None,
    })
}

#[derive(Default)]
pub struct Keymap {
    pending: String,
}

impl Keymap {
    /// Feed a key; returns an action once a full sequence matches.
    pub fn feed(&mut self, scope: Scope, key: KeyEvent) -> Option<Action> {
        let tok = token(key)?;
        self.pending.push_str(&tok);
        let active = || {
            BINDINGS
                .iter()
                .filter(|(s, ..)| *s == scope || *s == Global)
        };
        let exact = active()
            .filter(|(_, keys, _)| *keys == self.pending)
            .min_by_key(|(s, ..)| *s == Global);
        if let Some((_, _, action)) = exact {
            self.pending.clear();
            return Some(*action);
        }
        if active().any(|(_, keys, _)| keys.starts_with(self.pending.as_str())) {
            return None;
        }
        // Dead end: retry with just this key, so "gx" still triggers "x".
        let retry = self.pending != tok;
        self.pending.clear();
        if retry { self.feed(scope, key) } else { None }
    }

    pub fn pending(&self) -> &str {
        &self.pending
    }
}

/// One `(keys, label)` hint per distinct label, for the footer.
pub fn hints(scope: Scope) -> Vec<(&'static str, &'static str)> {
    let ordered = BINDINGS.iter().filter(|(s, ..)| *s == scope).chain(
        BINDINGS
            .iter()
            .filter(|(s, ..)| *s == Global && scope != Global),
    );
    let mut out: Vec<(&str, &str)> = Vec::new();
    for (_, keys, action) in ordered {
        if !out.iter().any(|(_, l)| *l == action.label()) {
            out.push((keys, action.label()));
        }
    }
    out
}
