use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use super::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // main content
            Constraint::Length(2), // status bar
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);
    draw_main(f, app, chunks[1]);
    draw_status_bar(f, app, chunks[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["[1] Files", "[2] Log", "[3] Branches"];
    let selected = match app.tab {
        Tab::Files => 0,
        Tab::Log => 1,
        Tab::Branches => 2,
    };

    let branch = app.repo.current_branch();
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" weave ~ {} ", branch)),
        )
        .select(selected)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn draw_main(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_list_panel(f, app, chunks[0]);
    draw_preview_panel(f, app, chunks[1]);
}

fn draw_list_panel(f: &mut Frame, app: &App, area: Rect) {
    let (title, items) = match app.tab {
        Tab::Files => {
            let items: Vec<ListItem> = app
                .files
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let style = if i == app.selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let prefix = if i == app.selected { "> " } else { "  " };
                    ListItem::new(format!("{}{}", prefix, name)).style(style)
                })
                .collect();
            ("Files", items)
        }
        Tab::Log => {
            let items: Vec<ListItem> = app
                .commits
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let style = if i == app.selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let prefix = if i == app.selected { "> " } else { "  " };
                    let merge_marker = if c.is_merge { " [merge]" } else { "" };
                    let text = format!("{}{} {}{}", prefix, c.id, c.message, merge_marker);
                    ListItem::new(text).style(style)
                })
                .collect();
            ("Commits", items)
        }
        Tab::Branches => {
            let current = app.repo.current_branch();
            let items: Vec<ListItem> = app
                .branch_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let is_current = name == current;
                    let style = if i == app.selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else if is_current {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let prefix = if i == app.selected { "> " } else { "  " };
                    let marker = if is_current { " *" } else { "" };
                    ListItem::new(format!("{}{}{}", prefix, name, marker)).style(style)
                })
                .collect();
            ("Branches", items)
        }
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", title)),
    );
    f.render_widget(list, area);
}

fn draw_preview_panel(f: &mut Frame, app: &App, area: Rect) {
    let content = app.preview_content();
    let title = match app.tab {
        Tab::Files => {
            if let Some(name) = app.files.get(app.selected) {
                format!(" {} ", name)
            } else {
                " Preview ".to_string()
            }
        }
        Tab::Log => {
            if let Some(c) = app.commits.get(app.selected) {
                format!(" Commit {} ", c.id)
            } else {
                " Preview ".to_string()
            }
        }
        Tab::Branches => {
            if let Some(name) = app.branch_names.get(app.selected) {
                format!(" {} ", name)
            } else {
                " Preview ".to_string()
            }
        }
    };

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let hints = match app.tab {
        Tab::Files => vec![
            ("q", "quit"),
            ("j/k", "navigate"),
            ("Tab", "switch tab"),
            ("J/K", "scroll preview"),
        ],
        Tab::Log => vec![
            ("q", "quit"),
            ("j/k", "navigate"),
            ("Tab", "switch tab"),
            ("J/K", "scroll preview"),
        ],
        Tab::Branches => vec![
            ("q", "quit"),
            ("j/k", "navigate"),
            ("Enter", "checkout"),
            ("m", "merge"),
            ("Tab", "switch tab"),
        ],
    };

    let spans: Vec<Span> = hints
        .iter()
        .enumerate()
        .flat_map(|(i, (key, desc))| {
            let mut s = vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {} ", desc), Style::default().fg(Color::Gray)),
            ];
            if i < hints.len() - 1 {
                s.push(Span::raw(" "));
            }
            s
        })
        .collect();

    let bar = Paragraph::new(Line::from(spans));
    f.render_widget(bar, area);
}
