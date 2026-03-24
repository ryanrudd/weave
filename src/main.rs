mod crdt;
mod document;
mod repository;
mod strategy;

use std::env;
use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use weave::crdt::SiteId;
use weave::repository::storage;
use weave::repository::Repository;
use weave::strategy::LineCRDT;

#[derive(Parser)]
#[command(name = "weave", about = "A CRDT-based version control system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new weave repository
    Init,
    /// Show the status of tracked files
    Status,
    /// Add a file to be tracked (reads current contents from disk)
    Add {
        /// File path to add
        file: String,
    },
    /// Commit staged changes
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,
    },
    /// Show commit history
    Log,
    /// Create a new branch
    Branch {
        /// Branch name to create
        name: String,
    },
    /// Switch to a different branch
    Checkout {
        /// Branch name to switch to
        branch: String,
    },
    /// Merge a branch into the current branch
    Merge {
        /// Branch to merge from
        branch: String,
    },
    /// Show the contents of a tracked file
    Cat {
        /// File path to display
        file: String,
    },
    /// List all branches
    Branches,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init(),
        Commands::Status => with_repo(|repo, _| cmd_status(&repo)),
        Commands::Add { file } => with_repo(|mut repo, dir| cmd_add(&mut repo, &dir, &file)),
        Commands::Commit { message } => {
            with_repo(|mut repo, dir| cmd_commit(&mut repo, &dir, &message))
        }
        Commands::Log => with_repo(|repo, _| cmd_log(&repo)),
        Commands::Branch { name } => with_repo(|mut repo, dir| cmd_branch(&mut repo, &dir, &name)),
        Commands::Checkout { branch } => {
            with_repo(|mut repo, dir| cmd_checkout(&mut repo, &dir, &branch))
        }
        Commands::Merge { branch } => {
            with_repo(|mut repo, dir| cmd_merge(&mut repo, &dir, &branch))
        }
        Commands::Cat { file } => with_repo(|repo, _| cmd_cat(&repo, &file)),
        Commands::Branches => with_repo(|repo, _| cmd_branches(&repo)),
    }
}

/// Helper: find the .weave dir, load the repo, run a closure, then save.
fn with_repo(f: impl FnOnce(Repository<LineCRDT>, PathBuf)) {
    let cwd = env::current_dir().expect("Could not get current directory");
    let weave_dir = match storage::find_weave_dir(&cwd) {
        Some(d) => d,
        None => {
            eprintln!("Not a weave repository (or any parent). Run 'weave init' first.");
            std::process::exit(1);
        }
    };

    let repo: Repository<LineCRDT> =
        storage::load(&weave_dir, LineCRDT::new).expect("Failed to load repository");

    f(repo, weave_dir);
}

/// Helper: save repo back to disk after mutation.
fn save_repo(repo: &Repository<LineCRDT>, weave_dir: &PathBuf) {
    storage::save(repo, weave_dir).expect("Failed to save repository");
}

// --- Command implementations ---

fn cmd_init() {
    let cwd = env::current_dir().expect("Could not get current directory");

    // Generate a simple site ID from timestamp
    let site_id = SiteId(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    );

    match storage::init(&cwd, site_id) {
        Ok(()) => println!("Initialized empty weave repository in {}", cwd.display()),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_status(repo: &Repository<LineCRDT>) {
    println!("On branch {}", repo.current_branch());

    // Show tracked files and their state
    let files = repo.tracked_files();
    if files.is_empty() {
        println!("No tracked files");
    } else {
        println!("Tracked files:");
        for file in files {
            println!("  {}", file);
        }
    }
}

fn cmd_add(repo: &mut Repository<LineCRDT>, weave_dir: &PathBuf, file: &str) {
    // Read the file from the working directory (the actual filesystem)
    let repo_root = weave_dir.parent().unwrap();
    let file_path = repo_root.join(file);

    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading '{}': {}", file, e);
            std::process::exit(1);
        }
    };

    // Replace the document's content with what's on disk.
    // We clear and re-add all lines. A smarter version would diff.
    let doc = repo.open_file(file);
    for line in content.lines() {
        doc.append(line.to_string());
    }

    save_repo(repo, weave_dir);
    println!("Added '{}'", file);
}

fn cmd_commit(repo: &mut Repository<LineCRDT>, weave_dir: &PathBuf, message: &str) {
    let commit_id = repo.commit(message);
    save_repo(repo, weave_dir);
    println!("[{}] {}", commit_id.0, message);
}

fn cmd_log(repo: &Repository<LineCRDT>) {
    // Walk from current branch head backwards
    let branch_name = repo.current_branch();
    let branches = repo.branches();
    let branch = &branches[branch_name];
    let mut id = branch.head;

    loop {
        let commit = match repo.get_commit(id) {
            Some(c) => c,
            None => break,
        };

        let files_changed: usize = commit.operations.len();
        println!("commit {}", id.0);
        if commit.parents.len() > 1 {
            let parent_ids: Vec<String> = commit.parents.iter().map(|p| p.0.to_string()).collect();
            println!("Merge: {}", parent_ids.join(" "));
        }
        println!("  {}", commit.message);
        if files_changed > 0 {
            println!("  ({} file(s) changed)", files_changed);
        }
        println!();

        // Follow first parent (like git log --first-parent)
        if let Some(parent) = commit.parents.first() {
            id = *parent;
        } else {
            break;
        }
    }
}

fn cmd_branch(repo: &mut Repository<LineCRDT>, weave_dir: &PathBuf, name: &str) {
    repo.create_branch(name);
    save_repo(repo, weave_dir);
    println!("Created branch '{}'", name);
}

fn cmd_checkout(repo: &mut Repository<LineCRDT>, weave_dir: &PathBuf, branch: &str) {
    match repo.checkout(branch) {
        Ok(()) => {
            save_repo(repo, weave_dir);
            println!("Switched to branch '{}'", branch);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_merge(repo: &mut Repository<LineCRDT>, weave_dir: &PathBuf, branch: &str) {
    match repo.merge(branch) {
        Ok(commit_id) => {
            save_repo(repo, weave_dir);
            println!("Merged '{}' (commit {})", branch, commit_id.0);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_cat(repo: &Repository<LineCRDT>, file: &str) {
    match repo.read_file(file) {
        Some(content) => println!("{}", content),
        None => {
            eprintln!("File '{}' not found", file);
            std::process::exit(1);
        }
    }
}

fn cmd_branches(repo: &Repository<LineCRDT>) {
    let current = repo.current_branch();
    let mut names: Vec<&String> = repo.branches().keys().collect();
    names.sort();
    for name in names {
        if name == current {
            println!("* {}", name);
        } else {
            println!("  {}", name);
        }
    }
}
