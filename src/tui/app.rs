use std::io;
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::repository::{storage, Repository};
use crate::strategy::LineCRDT;

use super::ui;

/// Which panel is currently focused.
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Files,
    Log,
    Branches,
}

/// Application state for the TUI.
pub struct App {
    pub repo: Repository<LineCRDT>,
    pub weave_dir: std::path::PathBuf,
    pub tab: Tab,
    pub should_quit: bool,
    /// Selected index in the current list.
    pub selected: usize,
    /// Scroll offset for the preview pane.
    pub preview_scroll: u16,
    /// Cached list data for the current tab.
    pub files: Vec<String>,
    pub commits: Vec<CommitInfo>,
    pub branch_names: Vec<String>,
}

/// Simplified commit info for display.
pub struct CommitInfo {
    pub id: u64,
    pub message: String,
    pub is_merge: bool,
}

impl App {
    pub fn new(repo: Repository<LineCRDT>, weave_dir: std::path::PathBuf) -> Self {
        let mut app = App {
            repo,
            weave_dir,
            tab: Tab::Files,
            should_quit: false,
            selected: 0,
            preview_scroll: 0,
            files: Vec::new(),
            commits: Vec::new(),
            branch_names: Vec::new(),
        };
        app.refresh();
        app
    }

    /// Refresh cached data from the repo.
    pub fn refresh(&mut self) {
        self.files = self
            .repo
            .tracked_files()
            .iter()
            .map(|s| s.to_string())
            .collect();
        self.branch_names = {
            let mut names: Vec<String> = self.repo.branches().keys().cloned().collect();
            names.sort();
            names
        };
        self.commits = self.build_commit_log();
    }

    fn build_commit_log(&self) -> Vec<CommitInfo> {
        let mut commits = Vec::new();
        let branch_name = self.repo.current_branch();
        let branches = self.repo.branches();
        let branch = &branches[branch_name];
        let mut id = branch.head;

        while let Some(commit) = self.repo.get_commit(id) {
            commits.push(CommitInfo {
                id: id.0,
                message: commit.message.clone(),
                is_merge: commit.parents.len() > 1,
            });
            if let Some(parent) = commit.parents.first() {
                id = *parent;
            } else {
                break;
            }
        }
        commits
    }

    /// Get the length of the current tab's list.
    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::Files => self.files.len(),
            Tab::Log => self.commits.len(),
            Tab::Branches => self.branch_names.len(),
        }
    }

    /// Get the preview content for the current selection.
    pub fn preview_content(&self) -> String {
        match self.tab {
            Tab::Files => {
                if let Some(filename) = self.files.get(self.selected) {
                    self.repo
                        .read_file(filename)
                        .unwrap_or_else(|| "(empty)".to_string())
                } else {
                    "No files tracked".to_string()
                }
            }
            Tab::Log => {
                if let Some(info) = self.commits.get(self.selected) {
                    let commit = self.repo.get_commit(crate::repository::CommitId(info.id));
                    match commit {
                        Some(c) => {
                            let mut lines = vec![
                                format!("Commit: {}", c.id.0),
                                format!("Message: {}", c.message),
                            ];
                            if !c.parents.is_empty() {
                                let parents: Vec<String> =
                                    c.parents.iter().map(|p| p.0.to_string()).collect();
                                lines.push(format!("Parents: {}", parents.join(", ")));
                            }
                            lines.push(String::new());
                            lines.push(format!("Files changed: {}", c.operations.len()));
                            for fo in &c.operations {
                                lines.push(format!("  {} ({} ops)", fo.filename, fo.ops.len()));
                            }
                            lines.join("\n")
                        }
                        None => "Commit not found".to_string(),
                    }
                } else {
                    "No commits".to_string()
                }
            }
            Tab::Branches => {
                if let Some(name) = self.branch_names.get(self.selected) {
                    let current = self.repo.current_branch();
                    let is_current = name == current;
                    let branches = self.repo.branches();
                    let branch = &branches[name];
                    let mut lines = vec![
                        format!("Branch: {}", name),
                        format!("Head: commit {}", branch.head.0),
                    ];
                    if is_current {
                        lines.push("(current branch)".to_string());
                    }
                    lines.join("\n")
                } else {
                    "No branches".to_string()
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('1') => {
                self.tab = Tab::Files;
                self.selected = 0;
                self.preview_scroll = 0;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Log;
                self.selected = 0;
                self.preview_scroll = 0;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::Branches;
                self.selected = 0;
                self.preview_scroll = 0;
            }
            KeyCode::Tab => {
                self.tab = match self.tab {
                    Tab::Files => Tab::Log,
                    Tab::Log => Tab::Branches,
                    Tab::Branches => Tab::Files,
                };
                self.selected = 0;
                self.preview_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.preview_scroll = 0;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.list_len() {
                    self.selected += 1;
                    self.preview_scroll = 0;
                }
            }
            KeyCode::Char('J') => {
                self.preview_scroll = self.preview_scroll.saturating_add(1);
            }
            KeyCode::Char('K') => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
            KeyCode::Enter => {
                // On Branches tab, Enter checks out the selected branch
                if self.tab == Tab::Branches {
                    if let Some(name) = self.branch_names.get(self.selected).cloned() {
                        if name != self.repo.current_branch()
                            && self.repo.checkout(&name).is_ok()
                        {
                            storage::save(&self.repo, &self.weave_dir).ok();
                            self.refresh();
                        }
                    }
                }
            }
            KeyCode::Char('m') => {
                // On Branches tab, 'm' merges the selected branch
                if self.tab == Tab::Branches {
                    if let Some(name) = self.branch_names.get(self.selected).cloned() {
                        if name != self.repo.current_branch()
                            && self.repo.merge(&name).is_ok()
                        {
                            storage::save(&self.repo, &self.weave_dir).ok();
                            self.refresh();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Launch the TUI.
pub fn run(weave_dir: &Path) -> io::Result<()> {
    let repo: Repository<LineCRDT> = storage::load(weave_dir, LineCRDT::new)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut app = App::new(repo, weave_dir.to_path_buf());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.handle_key(key.code);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
