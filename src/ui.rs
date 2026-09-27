//! Rendering. Each tab has its own draw function.

use crate::app::{App, Hit, Pane, Tab};
use crate::form::Form;
use crate::generator::SETTINGS;
use crate::keys;
use crate::vault::Focus;
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, BorderType, Clear, HighlightSpacing, LineGauge, List, ListItem, ListState, Padding,
    Paragraph, Tabs, Wrap,
};

const MASK: &str = "••••••••••••";
const ACCENT: Color = Color::Cyan;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let [top, body, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(f.area());
    let titles = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{} {}", i + 1, t.title()));
    let selected = Tab::ALL.iter().position(|&t| t == app.tab);
    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(Style::new().fg(ACCENT).bold().reversed())
        .divider("│");
    f.render_widget(tabs, top);
    // Tabs pads each title by one cell on both sides and puts a divider between them.
    let mut x = top.x;
    for (i, &tab) in Tab::ALL.iter().enumerate() {
        let w = Line::from(format!("{} {}", i + 1, tab.title())).width() as u16 + 2;
        app.hits.push((Rect::new(x, top.y, w, 1), Hit::Tab(tab)));
        x += w + 1;
    }
    match app.tab {
        Tab::Vault => draw_vault(f, body, app),
        Tab::Generator => draw_generator(f, body, app),
        Tab::Config => draw_config(f, body, app),
    }
    if let Some(form) = &app.form {
        draw_form(f, form, &mut app.hits);
    }
    draw_footer(f, bottom, app);
}

fn draw_form(f: &mut Frame, form: &Form, hits: &mut Vec<(Rect, Hit)>) {
    let height = form.fields.len() as u16 + 4;
    let area = f
        .area()
        .centered(Constraint::Percentage(60), Constraint::Length(height));
    let width = form.fields.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let lines: Vec<Line> = form
        .fields
        .iter()
        .enumerate()
        .map(|(i, (label, value))| {
            let style = if i == form.focus {
                Style::new().fg(ACCENT).bold()
            } else {
                Style::new()
            };
            Line::from(vec![
                Span::styled(format!("{label:>width$}  "), style),
                Span::raw(value),
            ])
        })
        .collect();
    let popup = block(form.title, true).padding(Padding::uniform(1));
    let value = &form.fields[form.focus].1;
    let cursor = (
        area.x + 2 + width as u16 + 2 + value.chars().count() as u16,
        area.y + 2 + form.focus as u16,
    );
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(popup), area);
    f.set_cursor_position(cursor);
    for i in 0..form.fields.len() as u16 {
        let row = Rect::new(area.x + 1, area.y + 2 + i, area.width.saturating_sub(2), 1);
        hits.push((row, Hit::FormField(i as usize)));
    }
}

/// Register one hit per visible list row, walking down from the list's scroll offset.
fn row_hits(
    hits: &mut Vec<(Rect, Hit)>,
    area: Rect,
    offset: usize,
    heights: impl Iterator<Item = u16>,
    hit: impl Fn(usize) -> Hit,
) {
    let inner = area.inner(Margin::new(1, 1));
    let mut y = inner.y;
    for (i, h) in heights.enumerate().skip(offset) {
        if y >= inner.bottom() {
            break;
        }
        let h = h.min(inner.bottom() - y);
        hits.push((Rect::new(inner.x, y, inner.width, h), hit(i)));
        y += h;
    }
}

fn block(title: &str, focused: bool) -> Block<'_> {
    let color = if focused { ACCENT } else { Color::DarkGray };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(format!(" {title} "))
}

fn draw_vault(f: &mut Frame, area: Rect, app: &mut App) {
    let v = &mut app.vault;
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area);
    let [search, list_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(left);

    let prompt = Line::from(vec![
        Span::styled("/", Style::new().fg(ACCENT)),
        Span::raw(v.query.as_str()),
    ]);
    f.render_widget(
        Paragraph::new(prompt).block(block("Search", v.searching)),
        search,
    );
    if v.searching {
        f.set_cursor_position((search.x + 2 + v.query.chars().count() as u16, search.y + 1));
    }

    let items: Vec<ListItem> = v
        .filtered
        .iter()
        .map(|&i| {
            let e = &v.entries[i];
            let mut spans = highlight(&e.name, v.regex.as_ref());
            if let Some(user) = e.user.as_deref().filter(|u| !u.is_empty()) {
                spans.push(Span::styled(
                    format!("  {user}"),
                    Style::new().fg(Color::DarkGray),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let title = format!("Entries {}/{}", v.filtered.len(), v.entries.len());
    let list = List::new(items)
        .block(block(&title, v.focus == Focus::List && !v.searching))
        .highlight_style(Style::new().bg(Color::DarkGray).bold())
        .highlight_symbol("▌");
    v.page = (list_area.height as usize / 2).max(1);
    f.render_stateful_widget(list, list_area, &mut v.list);
    app.hits.push((search, Hit::Search));
    app.hits.push((list_area, Hit::Scroll(Pane::List)));
    let heights = std::iter::repeat_n(1, v.filtered.len());
    row_hits(
        &mut app.hits,
        list_area,
        v.list.offset(),
        heights,
        Hit::Entry,
    );

    draw_detail(f, right, app);
}

/// Split `text` into spans, with regex matches highlighted.
fn highlight<'a>(text: &'a str, re: Option<&regex::Regex>) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut last = 0;
    for m in re
        .into_iter()
        .flat_map(|re| re.find_iter(text))
        .filter(|m| !m.is_empty())
    {
        spans.push(Span::raw(&text[last..m.start()]));
        spans.push(Span::styled(
            m.as_str(),
            Style::new().fg(Color::Yellow).bold(),
        ));
        last = m.end();
    }
    spans.push(Span::raw(&text[last..]));
    spans
}

fn draw_detail(f: &mut Frame, area: Rect, app: &mut App) {
    let v = &mut app.vault;
    app.hits.push((area, Hit::Scroll(Pane::Detail)));
    let focused = v.focus == Focus::Detail;
    let title = match v.item() {
        None => {
            f.render_widget(
                Paragraph::new("No entry selected").block(block("Details", focused)),
                area,
            );
            return;
        }
        Some(Err(e)) => {
            let msg = Paragraph::new(e.clone())
                .fg(Color::Red)
                .wrap(Wrap { trim: true });
            f.render_widget(msg.block(block("Details", focused)), area);
            return;
        }
        Some(Ok(item)) => match &item.folder {
            Some(folder) => format!("{folder}/{}", item.name),
            None => item.name.clone(),
        },
    };
    let revealed = v.revealed;
    let rows = v.rows();
    let width = rows
        .iter()
        .map(|r| r.label.chars().count())
        .max()
        .unwrap_or(0);
    let items: Vec<ListItem> = rows
        .iter()
        .map(|r| {
            let label = Span::styled(format!("{:>width$}  ", r.label), Style::new().fg(ACCENT));
            let value = if r.secret && !revealed {
                MASK
            } else {
                r.value.as_str()
            };
            let mut lines = value.lines();
            let first = Line::from(vec![label, Span::raw(lines.next().unwrap_or(""))]);
            let pad = " ".repeat(width + 2);
            let rest = lines.map(|l| Line::raw(format!("{pad}{l}")));
            ListItem::new(Text::from_iter(std::iter::once(first).chain(rest)))
        })
        .collect();
    let list = List::new(items).block(block(&title, focused));
    let list = if focused {
        list.highlight_style(Style::new().bg(Color::DarkGray).bold())
    } else {
        list
    };
    f.render_stateful_widget(list, area, &mut v.fields);
    let heights = rows.iter().map(|r| match r.secret && !revealed {
        true => 1,
        false => r.value.lines().count().max(1) as u16,
    });
    row_hits(&mut app.hits, area, v.fields.offset(), heights, Hit::Field);
}

fn draw_generator(f: &mut Frame, area: Rect, app: &mut App) {
    let g = &app.generator;
    app.hits.push((area, Hit::Scroll(Pane::Generator)));
    let [output, bottom] =
        Layout::vertical([Constraint::Length(7), Constraint::Min(0)]).areas(area);
    let [settings, history] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(bottom);

    let out_block = block("Password", true).padding(Padding::new(2, 2, 1, 0));
    let [pass_area, gauge_area] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)])
        .areas(out_block.inner(output));
    f.render_widget(out_block, output);
    let pass = Paragraph::new(Line::from(colorize(g.current().unwrap_or("")))).bold();
    f.render_widget(pass.centered().wrap(Wrap { trim: false }), pass_area);
    f.render_widget(strength(g.entropy()), gauge_area);

    let on_settings = g.row < SETTINGS.len();
    let values = [
        format!("{} {}", g.len(), g.unit()),
        g.mode.label().to_string(),
    ];
    let widths = values.clone().map(|v| Line::from(v).width() as u16);
    let items = SETTINGS.iter().zip(values).map(|(label, value)| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("{label:>7}  "), Style::new().fg(ACCENT)),
            Span::styled("‹ ", Style::new().fg(Color::DarkGray)),
            Span::raw(value),
            Span::styled(" ›", Style::new().fg(Color::DarkGray)),
        ]))
    });
    let mut state = ListState::default().with_selected(on_settings.then_some(g.row));
    let list = List::new(items)
        .block(block("Settings", on_settings))
        .highlight_style(Style::new().bg(Color::DarkGray).bold())
        .highlight_symbol("▌")
        .highlight_spacing(HighlightSpacing::Always);
    f.render_stateful_widget(list, settings, &mut state);
    row_hits(&mut app.hits, settings, 0, [1, 1].into_iter(), Hit::GenRow);
    // The ‹ › arrows sit after the symbol (1) and the padded label (9).
    let inner = settings.inner(Margin::new(1, 1));
    for (i, w) in widths.into_iter().enumerate().take(inner.height as usize) {
        let (y, dec) = (inner.y + i as u16, inner.x + 10);
        app.hits.push((Rect::new(dec, y, 2, 1), Hit::Adjust(i, -1)));
        app.hits
            .push((Rect::new(dec + 2 + w, y, 2, 1), Hit::Adjust(i, 1)));
    }

    let items = g
        .history
        .iter()
        .map(|p| ListItem::new(Line::from(colorize(p))));
    let mut state = ListState::default().with_selected(g.row.checked_sub(SETTINGS.len()));
    let title = format!("History {}", g.history.len());
    let list = List::new(items)
        .block(block(&title, !on_settings))
        .highlight_style(Style::new().bg(Color::DarkGray).bold())
        .highlight_symbol("▌");
    f.render_stateful_widget(list, history, &mut state);
    let heights = std::iter::repeat_n(1, g.history.len());
    row_hits(&mut app.hits, history, state.offset(), heights, |i| {
        Hit::GenRow(SETTINGS.len() + i)
    });
}

/// Color digits and symbols differently from letters, like Bitwarden does.
fn colorize(pass: &str) -> Vec<Span<'_>> {
    pass.char_indices()
        .map(|(i, c)| {
            let color = match c {
                '0'..='9' => Color::LightBlue,
                c if c.is_alphabetic() || c == ' ' => Color::Reset,
                _ => Color::LightRed,
            };
            Span::styled(&pass[i..i + c.len_utf8()], Style::new().fg(color))
        })
        .collect()
}

fn strength(bits: f64) -> LineGauge<'static> {
    let (label, color) = match bits {
        b if b < 40.0 => ("weak", Color::Red),
        b if b < 60.0 => ("fair", Color::Yellow),
        b if b < 80.0 => ("good", Color::LightGreen),
        b if b < 120.0 => ("strong", Color::Green),
        _ => ("very strong", Color::Green),
    };
    LineGauge::default()
        .ratio((bits / 128.0).min(1.0))
        .label(format!("{label:<11} ~{bits:.0} bits "))
        .filled_style(Style::new().fg(color))
        .unfilled_style(Style::new().fg(Color::DarkGray))
        .filled_symbol(symbols::line::THICK_HORIZONTAL)
        .unfilled_symbol(symbols::line::THICK_HORIZONTAL)
}

fn draw_config(f: &mut Frame, area: Rect, app: &App) {
    let width = app.config.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let lines: Vec<Line> = app
        .config
        .iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!("{k:>width$}  "), Style::new().fg(ACCENT)),
                Span::raw(v),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines).block(block("rbw config", true)), area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let line = match &app.status {
        Some(s) => Line::from(s.text.as_str()).fg(if s.error { Color::Red } else { Color::Green }),
        None if app.form.is_some() => {
            Line::from("<Tab>/<S-Tab> next/prev field  <CR> save  <Esc> cancel").fg(Color::DarkGray)
        }
        None if app.tab == Tab::Vault && app.vault.searching => {
            Line::from("<Esc> normal  <CR> details  <C-n>/<C-p> move  <C-u> clear  <C-w> word")
                .fg(Color::DarkGray)
        }
        None => {
            let mut spans = vec![Span::styled(
                app.keymap.pending().to_string(),
                Style::new().fg(Color::Yellow),
            )];
            for (k, label) in keys::hints(app.scope()) {
                spans.push(Span::styled(format!(" {k}"), Style::new().fg(ACCENT)));
                spans.push(Span::styled(
                    format!(" {label} "),
                    Style::new().fg(Color::DarkGray),
                ));
            }
            Line::from(spans)
        }
    };
    f.render_widget(line, area);
}
