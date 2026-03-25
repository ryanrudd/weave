use std::fs;
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

/// Modal overlay states — text inputs and dialogs.
#[derive(Clone, PartialEq)]
pub enum Modal {
    /// No modal — normal navigation.
    None,
    /// Text input for commit message.
    CommitInput,
    /// Text input for new branch name.
    BranchInput,
    /// Help overlay showing all keybindings.
    Help,
    /// Confirmation dialog for merge.
    MergeConfirm { branch: String },
}

/// A temporary notification shown at the bottom.
pub struct Toast {
    pub message: String,
    pub is_error: bool,
    /// Ticks remaining before the toast disappears.
    pub ttl: u8,
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
    /// Current modal state.
    pub modal: Modal,
    /// Text input buffer for modals.
    pub input_buf: String,
    /// Toast notification.
    pub toast: Option<Toast>,
    /// Files on disk that could be added (in repo root, not yet tracked or changed).
    pub disk_files: Vec<DiskFile>,
}

/// A file on disk relative to the repo root.
pub struct DiskFile {
    pub name: String,
    pub status: FileStatus,
}

#[derive(PartialEq)]
pub enum FileStatus {
    /// File is tracked and content matches.
    Clean,
    /// File is tracked but disk content differs.
    Modified,
    /// File exists on disk but isn't tracked.
    Untracked,
}

/// Simplified commit info for display.
pub struct CommitInfo {
    pub id: u64,
    pub message: String,
    pub is_merge: bool,
    pub _files_changed: usize,
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
            modal: Modal::None,
            input_buf: String::new(),
            toast: None,
            disk_files: Vec::new(),
        };
        app.refresh();
        app
    }

    fn show_toast(&mut self, message: &str, is_error: bool) {
        self.toast = Some(Toast {
            message: message.to_string(),
            is_error,
            ttl: 15, // ~15 key events before it disappears
        });
    }

    fn tick_toast(&mut self) {
        if let Some(ref mut toast) = self.toast {
            toast.ttl = toast.ttl.saturating_sub(1);
            if toast.ttl == 0 {
                self.toast = None;
            }
        }
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
        self.scan_disk_files();
    }

    fn scan_disk_files(&mut self) {
        let repo_root = match self.weave_dir.parent() {
            Some(p) => p.to_path_buf(),
            None => return,
        };

        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(&repo_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Skip hidden files and .weave internals
                    if name.starts_with('.') {
                        continue;
                    }
                    let status = if self.files.contains(&name) {
                        // Check if content differs
                        let disk_content = fs::read_to_string(&path).unwrap_or_default();
                        let tracked_content = self.repo.read_file(&name).unwrap_or_default();
                        if disk_content.trim_end() == tracked_content.trim_end() {
                            FileStatus::Clean
                        } else {
                            FileStatus::Modified
                        }
                    } else {
                        FileStatus::Untracked
                    };
                    files.push(DiskFile { name, status });
                }
            }
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        self.disk_files = files;
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
                _files_changed: commit.operations.len(),
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
            Tab::Files => self.disk_files.len(),
            Tab::Log => self.commits.len(),
            Tab::Branches => self.branch_names.len(),
        }
    }

    /// Get the preview content for the current selection.
    pub fn preview_content(&self) -> Vec<PreviewLine> {
        match self.tab {
            Tab::Files => {
                if let Some(df) = self.disk_files.get(self.selected) {
                    // Show tracked content if available, else disk content
                    let content = self.repo.read_file(&df.name).or_else(|| {
                        let repo_root = self.weave_dir.parent()?;
                        fs::read_to_string(repo_root.join(&df.name)).ok()
                    });
                    match content {
                        Some(text) => text
                            .lines()
                            .enumerate()
                            .map(|(i, line)| PreviewLine {
                                line_no: Some(i + 1),
                                content: line.to_string(),
                                style: LineStyle::Normal,
                            })
                            .collect(),
                        None => vec![PreviewLine::plain("(empty)")],
                    }
                } else {
                    vec![PreviewLine::plain("No files")]
                }
            }
            Tab::Log => {
                if let Some(info) = self.commits.get(self.selected) {
                    let commit = self.repo.get_commit(crate::repository::CommitId(info.id));
                    match commit {
                        Some(c) => {
                            let mut lines = vec![
                                PreviewLine::header(&format!("Commit {}", c.id.0)),
                                PreviewLine::plain(""),
                                PreviewLine::label("Message", &c.message),
                            ];
                            if !c.parents.is_empty() {
                                let parents: Vec<String> =
                                    c.parents.iter().map(|p| p.0.to_string()).collect();
                                lines.push(PreviewLine::label("Parents", &parents.join(", ")));
                            }
                            if c.parents.len() > 1 {
                                lines.push(PreviewLine {
                                    line_no: None,
                                    content: "MERGE COMMIT".to_string(),
                                    style: LineStyle::Merge,
                                });
                            }
                            lines.push(PreviewLine::plain(""));
                            lines.push(PreviewLine::header(&format!(
                                "Files changed ({})",
                                c.operations.len()
                            )));
                            for fo in &c.operations {
                                lines.push(PreviewLine {
                                    line_no: None,
                                    content: format!("  {} ({} ops)", fo.filename, fo.ops.len()),
                                    style: LineStyle::Added,
                                });
                            }
                            lines
                        }
                        None => vec![PreviewLine::plain("Commit not found")],
                    }
                } else {
                    vec![PreviewLine::plain("No commits")]
                }
            }
            Tab::Branches => {
                if let Some(name) = self.branch_names.get(self.selected) {
                    let current = self.repo.current_branch();
                    let is_current = name == current;
                    let branches = self.repo.branches();
                    let branch = &branches[name];
                    let mut lines = vec![
                        PreviewLine::header(name),
                        PreviewLine::plain(""),
                        PreviewLine::label("Head", &format!("commit {}", branch.head.0)),
                    ];
                    if is_current {
                        lines.push(PreviewLine {
                            line_no: None,
                            content: "  (this is the current branch)".to_string(),
                            style: LineStyle::Added,
                        });
                    }

                    // Show recent commits on this branch
                    lines.push(PreviewLine::plain(""));
                    lines.push(PreviewLine::header("Recent commits"));
                    let mut cid = branch.head;
                    let mut count = 0;
                    while let Some(commit) = self.repo.get_commit(cid) {
                        if count >= 5 {
                            lines.push(PreviewLine::dim("  ..."));
                            break;
                        }
                        let marker = if commit.parents.len() > 1 { "M" } else { "*" };
                        lines.push(PreviewLine {
                            line_no: None,
                            content: format!("  {} {} {}", marker, cid.0, commit.message),
                            style: LineStyle::Normal,
                        });
                        count += 1;
                        if let Some(parent) = commit.parents.first() {
                            cid = *parent;
                        } else {
                            break;
                        }
                    }
                    lines
                } else {
                    vec![PreviewLine::plain("No branches")]
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        self.tick_toast();

        // Handle modal states first
        match &self.modal.clone() {
            Modal::Help => {
                // Any key dismisses help
                self.modal = Modal::None;
                return;
            }
            Modal::CommitInput => {
                self.handle_text_input(key, InputTarget::Commit);
                return;
            }
            Modal::BranchInput => {
                self.handle_text_input(key, InputTarget::Branch);
                return;
            }
            Modal::MergeConfirm { branch } => {
                let branch = branch.clone();
                match key {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        if self.repo.merge(&branch).is_ok() {
                            storage::save(&self.repo, &self.weave_dir).ok();
                            self.refresh();
                            self.show_toast(&format!("Merged '{}'", branch), false);
                        } else {
                            self.show_toast("Merge failed", true);
                        }
                        self.modal = Modal::None;
                    }
                    _ => {
                        self.modal = Modal::None;
                    }
                }
                return;
            }
            Modal::None => {}
        }

        // Normal key handling
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => self.modal = Modal::Help,
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
            // --- File tab actions ---
            KeyCode::Char('a') if self.tab == Tab::Files => {
                self.action_add_file();
            }
            KeyCode::Char('c') => {
                self.input_buf.clear();
                self.modal = Modal::CommitInput;
            }
            // --- Branch tab actions ---
            KeyCode::Enter if self.tab == Tab::Branches => {
                if let Some(name) = self.branch_names.get(self.selected).cloned() {
                    if name != self.repo.current_branch() && self.repo.checkout(&name).is_ok() {
                        storage::save(&self.repo, &self.weave_dir).ok();
                        self.refresh();
                        self.show_toast(&format!("Switched to '{}'", name), false);
                    }
                }
            }
            KeyCode::Char('m') if self.tab == Tab::Branches => {
                if let Some(name) = self.branch_names.get(self.selected).cloned() {
                    if name != self.repo.current_branch() {
                        self.modal = Modal::MergeConfirm { branch: name };
                    }
                }
            }
            KeyCode::Char('n') if self.tab == Tab::Branches => {
                self.input_buf.clear();
                self.modal = Modal::BranchInput;
            }
            // --- Refresh ---
            KeyCode::Char('r') => {
                self.refresh();
                self.show_toast("Refreshed", false);
            }
            _ => {}
        }
    }

    fn handle_text_input(&mut self, key: KeyCode, target: InputTarget) {
        match key {
            KeyCode::Esc => {
                self.modal = Modal::None;
                self.input_buf.clear();
            }
            KeyCode::Enter => {
                let value = self.input_buf.clone();
                self.input_buf.clear();
                self.modal = Modal::None;
                if !value.is_empty() {
                    match target {
                        InputTarget::Commit => self.action_commit(&value),
                        InputTarget::Branch => self.action_create_branch(&value),
                    }
                }
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Char(c) => {
                self.input_buf.push(c);
            }
            _ => {}
        }
    }

    fn action_add_file(&mut self) {
        if let Some(df) = self.disk_files.get(self.selected) {
            let filename = df.name.clone();
            let repo_root = match self.weave_dir.parent() {
                Some(p) => p.to_path_buf(),
                None => return,
            };
            let file_path = repo_root.join(&filename);
            let content = match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(_) => {
                    self.show_toast(&format!("Can't read '{}'", filename), true);
                    return;
                }
            };

            let new_lines: Vec<&str> = content.lines().collect();
            let existing_lines: Vec<String> = self
                .repo
                .read_file(&filename)
                .map(|s| s.lines().map(|l| l.to_string()).collect())
                .unwrap_or_default();

            let doc = self.repo.open_file(&filename);
            if existing_lines.is_empty() {
                for line in &new_lines {
                    doc.append(line.to_string());
                }
            } else {
                let common_prefix = existing_lines
                    .iter()
                    .zip(new_lines.iter())
                    .take_while(|(a, b)| a.as_str() == **b)
                    .count();

                if common_prefix == existing_lines.len() && new_lines.len() > existing_lines.len() {
                    for line in &new_lines[common_prefix..] {
                        doc.append(line.to_string());
                    }
                } else if new_lines.iter().map(|s| s.to_string()).collect::<Vec<_>>()
                    != existing_lines
                {
                    let ids = doc.visible_ids();
                    for id in ids {
                        doc.delete(id);
                    }
                    for line in &new_lines {
                        doc.append(line.to_string());
                    }
                }
            }

            storage::save(&self.repo, &self.weave_dir).ok();
            self.refresh();
            self.show_toast(&format!("Added '{}'", filename), false);
        }
    }

    fn action_commit(&mut self, message: &str) {
        let commit_id = self.repo.commit(message);
        storage::save(&self.repo, &self.weave_dir).ok();
        self.refresh();
        self.show_toast(&format!("[{}] {}", commit_id.0, message), false);
    }

    fn action_create_branch(&mut self, name: &str) {
        self.repo.create_branch(name);
        storage::save(&self.repo, &self.weave_dir).ok();
        self.refresh();
        self.show_toast(&format!("Created branch '{}'", name), false);
    }
}

enum InputTarget {
    Commit,
    Branch,
}

/// A line in the preview pane with optional styling.
pub struct PreviewLine {
    pub line_no: Option<usize>,
    pub content: String,
    pub style: LineStyle,
}

#[derive(PartialEq)]
pub enum LineStyle {
    Normal,
    Header,
    Label,
    Added,
    Merge,
    Dim,
}

impl PreviewLine {
    pub fn plain(s: &str) -> Self {
        PreviewLine {
            line_no: None,
            content: s.to_string(),
            style: LineStyle::Normal,
        }
    }
    pub fn header(s: &str) -> Self {
        PreviewLine {
            line_no: None,
            content: s.to_string(),
            style: LineStyle::Header,
        }
    }
    pub fn label(key: &str, val: &str) -> Self {
        PreviewLine {
            line_no: None,
            content: format!("  {}: {}", key, val),
            style: LineStyle::Label,
        }
    }
    pub fn dim(s: &str) -> Self {
        PreviewLine {
            line_no: None,
            content: s.to_string(),
            style: LineStyle::Dim,
        }
    }
}

/// Launch the TUI.
pub fn run(weave_dir: &Path) -> io::Result<()> {
    let repo: Repository<LineCRDT> =
        storage::load(weave_dir, LineCRDT::new).map_err(io::Error::other)?;

    let mut app = App::new(repo, weave_dir.to_path_buf());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
