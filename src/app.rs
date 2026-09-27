//! Application state and action dispatch.

use crate::form::{Event as FormEvent, Form, Kind, edit_line};
use crate::generator::Generator;
use crate::generator::SETTINGS;
use crate::keys::{Action, Keymap, Scope};
use crate::rbw;
use crate::vault::{Focus, Vault};
use anyhow::{Result, anyhow};
use arboard::Clipboard;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Vault,
    Generator,
    Config,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Vault, Tab::Generator, Tab::Config];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Vault => "Vault",
            Tab::Generator => "Generator",
            Tab::Config => "Config",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }
}

/// A clickable screen region, registered by the UI on every draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Tab(Tab),
    Search,
    Entry(usize),
    Field(usize),
    GenRow(usize),
    Adjust(usize, isize),
    FormField(usize),
    Scroll(Pane),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    List,
    Detail,
    Generator,
}

pub struct Status {
    pub text: String,
    pub error: bool,
}

pub struct App {
    pub tab: Tab,
    pub running: bool,
    pub vault: Vault,
    pub generator: Generator,
    pub config: Vec<(String, String)>,
    pub status: Option<Status>,
    pub keymap: Keymap,
    /// Interactive rbw command to run with the terminal suspended (e.g. unlock).
    pub suspend: Option<Vec<&'static str>>,
    /// Modal form that captures all keys while open.
    pub form: Option<Form>,
    /// Click regions from the last draw; later entries sit on top.
    pub hits: Vec<(Rect, Hit)>,
    clipboard: Option<Clipboard>,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            tab: Tab::Vault,
            running: true,
            vault: Vault::new(Vec::new()),
            generator: Generator::new(),
            config: Vec::new(),
            status: None,
            keymap: Keymap::default(),
            suspend: None,
            form: None,
            hits: Vec::new(),
            clipboard: Clipboard::new().ok(),
        };
        app.reload();
        let r = app.generator.generate();
        if let Err(e) = r {
            app.report(Err(e));
        }
        app
    }

    pub fn reload(&mut self) {
        let result = rbw::list().map(|e| self.vault.set_entries(e));
        self.config = rbw::config().unwrap_or_default();
        self.report(result.map(|_| format!("{} entries", self.vault.entries.len())));
    }

    pub fn scope(&self) -> Scope {
        match (self.tab, self.vault.focus) {
            (Tab::Vault, Focus::List) => Scope::List,
            (Tab::Vault, Focus::Detail) => Scope::Detail,
            (Tab::Generator, _) => Scope::Generator,
            (Tab::Config, _) => Scope::Config,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.form.is_some() {
            return self.on_form_key(key);
        }
        if self.tab == Tab::Vault && self.vault.searching {
            return self.on_search_key(key);
        }
        if let Some(action) = self.keymap.feed(self.scope(), key) {
            self.status = None;
            self.dispatch(action);
        }
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        let pos = Position::new(m.column, m.row);
        let hit = |scroll: bool| {
            self.hits.iter().rev().find_map(|&(r, h)| {
                (r.contains(pos) && matches!(h, Hit::Scroll(_)) == scroll).then_some(h)
            })
        };
        match (m.kind, hit(false), hit(true)) {
            (MouseEventKind::Down(MouseButton::Left), Some(h), _) => self.click(h),
            (MouseEventKind::ScrollDown, _, Some(Hit::Scroll(p))) => self.scroll(p, Action::Down),
            (MouseEventKind::ScrollUp, _, Some(Hit::Scroll(p))) => self.scroll(p, Action::Up),
            _ => {}
        }
    }

    /// Clicking selects; clicking what is already selected activates it (open or yank).
    fn click(&mut self, hit: Hit) {
        if let Some(form) = &mut self.form {
            if let Hit::FormField(i) = hit {
                form.focus = i;
            }
            return;
        }
        self.status = None;
        self.vault.searching = false;
        let v = &mut self.vault;
        match hit {
            Hit::Tab(tab) => self.tab = tab,
            Hit::Search => self.dispatch(Action::Search),
            Hit::Entry(i) if v.focus == Focus::List && v.list.selected() == Some(i) => {
                v.focus = Focus::Detail
            }
            Hit::Entry(i) => {
                v.focus = Focus::List;
                v.select(Some(i));
            }
            Hit::Field(i) if v.focus == Focus::Detail && v.fields.selected() == Some(i) => {
                self.dispatch(Action::Yank)
            }
            Hit::Field(i) => {
                v.focus = Focus::Detail;
                v.fields.select(Some(i));
            }
            Hit::GenRow(row) if row >= SETTINGS.len() && self.generator.row == row => {
                self.dispatch(Action::Yank)
            }
            Hit::GenRow(row) => self.generator.move_to(row),
            Hit::Adjust(row, delta) => {
                self.generator.move_to(row);
                self.adjust(delta);
            }
            Hit::FormField(_) | Hit::Scroll(_) => {}
        }
    }

    fn scroll(&mut self, pane: Pane, action: Action) {
        if self.form.is_some() {
            return;
        }
        match pane {
            Pane::List => self.vault.focus = Focus::List,
            Pane::Detail => self.vault.focus = Focus::Detail,
            Pane::Generator => {}
        }
        self.navigate(action);
    }

    /// Insert mode for the search box.
    fn on_search_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let v = &mut self.vault;
        match key.code {
            KeyCode::Char('c') if ctrl => self.running = false,
            KeyCode::Esc => v.searching = false,
            KeyCode::Enter => {
                v.searching = false;
                v.focus = Focus::Detail;
            }
            KeyCode::Down => v.move_by(1),
            KeyCode::Up => v.move_by(-1),
            KeyCode::Char('n') if ctrl => v.move_by(1),
            KeyCode::Char('p') if ctrl => v.move_by(-1),
            _ => {
                if edit_line(&mut v.query, key) {
                    v.refilter();
                }
            }
        }
    }

    fn on_form_key(&mut self, key: KeyEvent) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        match form.on_key(key) {
            FormEvent::None => {}
            FormEvent::Cancel => self.form = None,
            FormEvent::Submit => {
                let kind = form.kind;
                self.submit(kind);
            }
        }
    }

    fn submit(&mut self, kind: Kind) {
        let Some(form) = &self.form else { return };
        match kind {
            Kind::SaveEntry => {
                let name = form.value("Name").to_string();
                let password = self.generator.selected().unwrap_or_default().to_string();
                if name.is_empty() {
                    return self.report(Err(anyhow!("name is required")));
                }
                let (user, uri, folder) = (
                    form.value("Username"),
                    form.value("URI"),
                    form.value("Folder"),
                );
                match rbw::add(&name, user, uri, folder, &password) {
                    Ok(()) => {
                        self.form = None;
                        self.reload();
                        self.report(Ok(format!("saved {name} to the vault")));
                    }
                    Err(e) => self.report(Err(e)),
                }
            }
        }
    }

    fn dispatch(&mut self, action: Action) {
        let n = Tab::ALL.len();
        match action {
            Action::Quit => self.running = false,
            Action::NextTab => self.tab = Tab::ALL[(self.tab.index() + 1) % n],
            Action::PrevTab => self.tab = Tab::ALL[(self.tab.index() + n - 1) % n],
            Action::GoTab(i) => self.tab = Tab::ALL[i.min(n - 1)],
            Action::Search => {
                self.vault.focus = Focus::List;
                self.vault.searching = true;
            }
            Action::ClearSearch => {
                self.vault.query.clear();
                self.vault.refilter();
            }
            Action::FocusDetail => self.vault.focus = Focus::Detail,
            Action::FocusList => self.vault.focus = Focus::List,
            Action::Down
            | Action::Up
            | Action::Top
            | Action::Bottom
            | Action::HalfDown
            | Action::HalfUp => self.navigate(action),
            Action::ToggleReveal => self.vault.revealed = !self.vault.revealed,
            Action::Yank if self.tab == Tab::Generator => {
                let out = self.generator.selected().map(str::to_string);
                let r = out
                    .ok_or(anyhow!("nothing generated yet"))
                    .and_then(|p| self.copy(&p, "password"));
                self.report(r);
            }
            Action::Yank => {
                let rows = self.vault.rows();
                let row = self.vault.fields.selected().and_then(|i| rows.get(i));
                let r = row
                    .ok_or(anyhow!("no field selected"))
                    .and_then(|r| self.copy(&r.value, &r.label));
                self.report(r);
            }
            Action::YankPassword => self.yank_field("password"),
            Action::YankUser => self.yank_field("username"),
            Action::YankTotp => {
                let id = self.vault.selected().map(|e| e.id.clone());
                let r = id
                    .ok_or(anyhow!("no entry selected"))
                    .and_then(|id| rbw::code(&id));
                let r = r.and_then(|code| self.copy(&code, "totp code"));
                self.report(r);
            }
            Action::OpenBrowser => {
                let r = self.find_field("uri").and_then(|uri| {
                    let url = if uri.contains("://") {
                        uri
                    } else {
                        format!("https://{uri}")
                    };
                    open::that_detached(&url)?;
                    Ok(format!("opened {url}"))
                });
                self.report(r);
            }
            Action::Sync => match rbw::sync() {
                Ok(()) => self.reload(),
                Err(e) => self.report(Err(e)),
            },
            Action::Lock => {
                let r = rbw::lock();
                self.vault.clear_cache();
                self.report(r.map(|_| "locked".to_string()));
            }
            Action::Unlock => self.suspend = Some(vec!["unlock"]),
            Action::Login => self.suspend = Some(vec!["login"]),
            Action::Generate => {
                let r = self.generator.generate();
                self.generator.move_to(self.generator.row);
                self.report(r.map(|_| String::new()));
            }
            Action::SaveEntry if self.generator.selected().is_none() => {
                self.report(Err(anyhow!("nothing generated yet")))
            }
            Action::SaveEntry => {
                let labels = ["Name", "Username", "URI", "Folder"];
                self.form = Some(Form::new(Kind::SaveEntry, "Save to vault", &labels));
            }
            Action::Increase => self.adjust(1),
            Action::Decrease => self.adjust(-1),
            Action::IncreaseMore => self.adjust(10),
            Action::DecreaseMore => self.adjust(-10),
        }
    }

    /// Change the selected generator setting and regenerate, like Bitwarden does.
    fn adjust(&mut self, delta: isize) {
        if self.generator.adjust(delta) {
            let r = self.generator.generate();
            self.report(r.map(|_| String::new()));
        }
    }

    fn navigate(&mut self, action: Action) {
        if self.tab == Tab::Generator {
            let g = &mut self.generator;
            return match action {
                Action::Down => g.move_to(g.row + 1),
                Action::Up => g.move_to(g.row.saturating_sub(1)),
                Action::Top => g.move_to(0),
                _ => g.move_to(usize::MAX),
            };
        }
        let v = &mut self.vault;
        let page = v.page as isize;
        if v.focus == Focus::Detail {
            let len = v.rows().len();
            let cur = v.fields.selected().unwrap_or(0);
            let next = match action {
                Action::Down => cur + 1,
                Action::Up => cur.saturating_sub(1),
                Action::Top => 0,
                _ => usize::MAX,
            };
            v.fields.select(Some(next.min(len.saturating_sub(1))));
            return;
        }
        match action {
            Action::Down => v.move_by(1),
            Action::Up => v.move_by(-1),
            Action::HalfDown => v.move_by(page),
            Action::HalfUp => v.move_by(-page),
            Action::Top => v.select(Some(0)),
            _ => v.select(Some(usize::MAX)),
        }
    }

    fn find_field(&mut self, label: &str) -> Result<String> {
        if let Some(Err(e)) = self.vault.item() {
            return Err(anyhow!("{e}"));
        }
        let rows = self.vault.rows();
        let row = rows
            .into_iter()
            .find(|r| r.label == label && !r.value.is_empty());
        row.map(|r| r.value).ok_or(anyhow!("entry has no {label}"))
    }

    fn yank_field(&mut self, label: &str) {
        let r = self.find_field(label).and_then(|v| self.copy(&v, label));
        self.report(r);
    }

    fn copy(&mut self, text: &str, what: &str) -> Result<String> {
        let clip = self
            .clipboard
            .as_mut()
            .ok_or(anyhow!("no clipboard available"))?;
        clip.set_text(text)?;
        Ok(format!("copied {what} to clipboard"))
    }

    fn report(&mut self, result: Result<String>) {
        self.status = match result {
            Ok(text) if text.is_empty() => None,
            Ok(text) => Some(Status { text, error: false }),
            Err(e) => Some(Status {
                text: e.to_string(),
                error: true,
            }),
        };
    }
}
