mod app;
mod form;
mod generator;
mod keys;
mod rbw;
mod ui;
mod vault;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use std::process::Command;

fn init() -> Result<DefaultTerminal> {
    let terminal = ratatui::init();
    execute!(std::io::stdout(), EnableMouseCapture)?;
    Ok(terminal)
}

fn restore() {
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
}

fn main() -> Result<()> {
    // Unlock before taking over the terminal, so pinentry can use the tty.
    if !rbw::unlocked() {
        Command::new("rbw").arg("unlock").status()?;
    }
    let mut terminal = init()?;
    let mut app = app::App::new();
    let result = (|| -> Result<()> {
        while app.running {
            terminal.draw(|f| ui::draw(f, &mut app))?;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
                Event::Mouse(mouse) => app.on_mouse(mouse),
                _ => {}
            }
            if let Some(args) = app.suspend.take() {
                restore();
                Command::new("rbw").args(&args).status()?;
                terminal = init()?;
                app.reload();
            }
        }
        Ok(())
    })();
    restore();
    result
}
