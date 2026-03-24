use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use super::app::{App, FileStatus, LineStyle, Modal, Tab};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // main content
            Constraint::Length(2), // status bar / toast
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);
    draw_main(f, app, chunks[1]);
    draw_status_bar(f, app, chunks[2]);

    // Draw modal overlays on top
    match &app.modal {
        Modal::None => {}
        Modal::Help => draw_help_overlay(f),
        Modal::CommitInput => draw_input_overlay(f, "Commit message", &app.input_buf),
        Modal::BranchInput => draw_input_overlay(f, "New branch name", &app.input_buf),
        Modal::MergeConfirm { branch } => {
            draw_confirm_overlay(f, &format!("Merge '{}' into current branch?", branch));
        }
    }
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["[1] Files", "[2] Log", "[3] Branches"];
    let selected = match app.tab {
        Tab::Files => 0,
        Tab::Log => 1,
        Tab::Branches => 2,
    };

    let branch = app.repo.current_branch();
    let uncommitted: usize = app
        .disk_files
        .iter()
        .filter(|f| f.status != FileStatus::Clean)
        .count();
    let title = if uncommitted > 0 {
        format!(" weave ~ {} ({} changed) ", branch, uncommitted)
    } else {
        format!(" weave ~ {} ", branch)
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(title))
        .select(selected)
        .style(Style::default().fg(Color::DarkGray))
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
                .disk_files
                .iter()
                .enumerate()
                .map(|(i, df)| {
                    let is_selected = i == app.selected;
                    let (status_icon, status_color) = match df.status {
                        FileStatus::Clean => (" ", Color::White),
                        FileStatus::Modified => ("M", Color::Yellow),
                        FileStatus::Untracked => ("?", Color::Green),
                    };
                    let name_style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(status_color)
                    };
                    let prefix = if is_selected { "> " } else { "  " };
                    let line = Line::from(vec![
                        Span::raw(prefix),
                        Span::styled(
                            format!("{} ", status_icon),
                            Style::default().fg(status_color),
                        ),
                        Span::styled(df.name.clone(), name_style),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            let tracked = app.files.len();
            let title = format!("Files ({})", tracked);
            (title, items)
        }
        Tab::Log => {
            let items: Vec<ListItem> = app
                .commits
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let is_selected = i == app.selected;
                    let base_style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let prefix = if is_selected { "> " } else { "  " };
                    let graph = if c.is_merge { "M" } else { "*" };
                    let graph_color = if c.is_merge {
                        Color::Magenta
                    } else {
                        Color::Yellow
                    };
                    let line = Line::from(vec![
                        Span::raw(prefix),
                        Span::styled(format!("{} ", graph), Style::default().fg(graph_color)),
                        Span::styled(
                            format!("{} ", c.id),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::DIM),
                        ),
                        Span::styled(c.message.clone(), base_style),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            let title = format!("Commits ({})", app.commits.len());
            (title, items)
        }
        Tab::Branches => {
            let current = app.repo.current_branch().to_string();
            let items: Vec<ListItem> = app
                .branch_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let is_current = *name == current;
                    let is_selected = i == app.selected;
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else if is_current {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let prefix = if is_selected { "> " } else { "  " };
                    let marker = if is_current { " *" } else { "" };
                    ListItem::new(format!("{}{}{}", prefix, name, marker)).style(style)
                })
                .collect();
            let title = format!("Branches ({})", app.branch_names.len());
            (title, items)
        }
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", title))
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(list, area);
}

fn draw_preview_panel(f: &mut Frame, app: &App, area: Rect) {
    let lines = app.preview_content();
    let title = match app.tab {
        Tab::Files => app
            .disk_files
            .get(app.selected)
            .map(|df| format!(" {} ", df.name))
            .unwrap_or_else(|| " Preview ".to_string()),
        Tab::Log => app
            .commits
            .get(app.selected)
            .map(|c| format!(" Commit {} ", c.id))
            .unwrap_or_else(|| " Preview ".to_string()),
        Tab::Branches => app
            .branch_names
            .get(app.selected)
            .map(|n| format!(" {} ", n))
            .unwrap_or_else(|| " Preview ".to_string()),
    };

    // Convert PreviewLines to ratatui Lines
    let text_lines: Vec<Line> = lines
        .iter()
        .map(|pl| {
            let mut spans = Vec::new();

            // Line number gutter
            if let Some(n) = pl.line_no {
                spans.push(Span::styled(
                    format!("{:>4} ", n),
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    "| ".to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let content_style = match pl.style {
                LineStyle::Normal => Style::default().fg(Color::White),
                LineStyle::Header => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                LineStyle::Label => Style::default().fg(Color::Gray),
                LineStyle::Added => Style::default().fg(Color::Green),
                LineStyle::Merge => Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
                LineStyle::Dim => Style::default().fg(Color::DarkGray),
            };
            spans.push(Span::styled(pl.content.clone(), content_style));
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(text_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    // If there's a toast, show it instead of hints
    if let Some(ref toast) = app.toast {
        let style = if toast.is_error {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        let bar = Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(&toast.message, style),
        ]));
        f.render_widget(bar, area);
        return;
    }

    let hints = match app.tab {
        Tab::Files => vec![
            ("q", "quit"),
            ("a", "add file"),
            ("c", "commit"),
            ("j/k", "nav"),
            ("J/K", "scroll"),
            ("?", "help"),
        ],
        Tab::Log => vec![
            ("q", "quit"),
            ("c", "commit"),
            ("j/k", "nav"),
            ("J/K", "scroll"),
            ("?", "help"),
        ],
        Tab::Branches => vec![
            ("q", "quit"),
            ("Enter", "checkout"),
            ("m", "merge"),
            ("n", "new branch"),
            ("c", "commit"),
            ("?", "help"),
        ],
    };

    let spans: Vec<Span> = hints
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {} ", desc), Style::default().fg(Color::Gray)),
            ]
        })
        .collect();

    let bar = Paragraph::new(Line::from(spans));
    f.render_widget(bar, area);
}

// --- Modal overlays ---

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn draw_input_overlay(f: &mut Frame, prompt: &str, input: &str) {
    let area = centered_rect(50, 5, f.area());
    f.render_widget(Clear, area);

    let cursor = "_";
    let text = format!("{}{}", input, cursor);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", prompt))
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(text, Style::default().fg(Color::White)),
    ]))
    .block(block);
    f.render_widget(paragraph, area);
}

fn draw_confirm_overlay(f: &mut Frame, message: &str) {
    let area = centered_rect(50, 5, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .border_style(Style::default().fg(Color::Yellow));
    let paragraph = Paragraph::new(vec![
        Line::from(Span::styled(
            format!(" {}", message),
            Style::default().fg(Color::White),
        )),
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                " y/Enter ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" yes  "),
            Span::styled(
                " any ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" cancel"),
        ]),
    ])
    .block(block);
    f.render_widget(paragraph, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(55, 22, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            " Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        key_line("j / k / Up / Down", "Move selection"),
        key_line("J / K", "Scroll preview"),
        key_line("1 / 2 / 3", "Switch to tab"),
        key_line("Tab", "Next tab"),
        key_line("r", "Refresh"),
        Line::from(""),
        Line::from(Span::styled(
            " Files tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        key_line("a", "Add selected file"),
        key_line("c", "Commit staged changes"),
        Line::from(""),
        Line::from(Span::styled(
            " Branches tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        key_line("Enter", "Checkout branch"),
        key_line("m", "Merge branch"),
        key_line("n", "Create new branch"),
        key_line("c", "Commit staged changes"),
        Line::from(""),
        key_line("q / Esc", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Press any key to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, area);
}

fn key_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<20}", key),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(Color::Gray)),
    ])
}
