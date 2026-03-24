use std::fs;
use std::process::Command;

/// Path to the weave binary.
fn weave_bin() -> String {
    // Build first to ensure binary is up to date
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/debug/weave", manifest)
}

/// Create a temp dir and init a weave repo in it.
fn init_test_repo() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "weave_cli_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();

    let output = Command::new(weave_bin())
        .arg("init")
        .current_dir(&dir)
        .output()
        .expect("Failed to run weave");
    assert!(output.status.success(), "init failed");

    dir
}

fn run_weave(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(weave_bin())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("Failed to run weave")
}

#[test]
fn double_init_fails() {
    let dir = init_test_repo();
    let output = run_weave(&dir, &["init"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Already"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn add_nonexistent_file_fails() {
    let dir = init_test_repo();
    let output = run_weave(&dir, &["add", "ghost.txt"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn cat_unknown_file_fails() {
    let dir = init_test_repo();
    let output = run_weave(&dir, &["cat", "nope.txt"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn checkout_nonexistent_branch_fails_cli() {
    let dir = init_test_repo();
    let output = run_weave(&dir, &["checkout", "nope"]);
    assert!(!output.status.success());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn merge_nonexistent_branch_fails_cli() {
    let dir = init_test_repo();
    let output = run_weave(&dir, &["merge", "nope"]);
    assert!(!output.status.success());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn commands_outside_repo_fail() {
    let dir = std::env::temp_dir().join(format!(
        "weave_no_repo_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();

    // These should all fail gracefully (not panic)
    for cmd in &["status", "log", "branches"] {
        let output = run_weave(&dir, &[cmd]);
        assert!(
            !output.status.success(),
            "'weave {}' should fail outside a repo",
            cmd
        );
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn add_then_readd_with_append() {
    let dir = init_test_repo();

    // Create and add a file
    fs::write(dir.join("test.txt"), "line 1\nline 2\n").unwrap();
    let output = run_weave(&dir, &["add", "test.txt"]);
    assert!(output.status.success());
    run_weave(&dir, &["commit", "-m", "initial"]);

    // Append to file and re-add
    fs::write(dir.join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();
    let output = run_weave(&dir, &["add", "test.txt"]);
    assert!(output.status.success());
    run_weave(&dir, &["commit", "-m", "append"]);

    // Verify content
    let output = run_weave(&dir, &["cat", "test.txt"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line 1"));
    assert!(stdout.contains("line 2"));
    assert!(stdout.contains("line 3"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn full_workflow_via_cli() {
    let dir = init_test_repo();

    // Add and commit a file
    fs::write(dir.join("hello.txt"), "hello\nworld\n").unwrap();
    run_weave(&dir, &["add", "hello.txt"]);
    run_weave(&dir, &["commit", "-m", "add hello"]);

    // Create branch and add content
    run_weave(&dir, &["branch", "feature"]);
    run_weave(&dir, &["checkout", "feature"]);
    fs::write(dir.join("hello.txt"), "hello\nworld\nfrom feature\n").unwrap();
    run_weave(&dir, &["add", "hello.txt"]);
    run_weave(&dir, &["commit", "-m", "feature edit"]);

    // Back to main, add content
    run_weave(&dir, &["checkout", "main"]);
    fs::write(dir.join("hello.txt"), "hello\nworld\nfrom main\n").unwrap();
    run_weave(&dir, &["add", "hello.txt"]);
    run_weave(&dir, &["commit", "-m", "main edit"]);

    // Merge
    let output = run_weave(&dir, &["merge", "feature"]);
    assert!(output.status.success());

    // Verify merged content
    let output = run_weave(&dir, &["cat", "hello.txt"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
    assert!(stdout.contains("world"));
    assert!(stdout.contains("from main"));
    assert!(stdout.contains("from feature"));

    // Log should show merge
    let output = run_weave(&dir, &["log"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Merge"));

    fs::remove_dir_all(&dir).ok();
}
