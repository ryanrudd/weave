# weave

A version control system built on CRDTs (Conflict-free Replicated Data Types). Branches always merge cleanly — no merge conflicts, ever.

## The idea

Git uses three-way merge to combine branches. When two people edit nearby lines, you get a merge conflict and have to resolve it manually. Most of these conflicts are purely syntactic — the edits don't actually interfere with each other.

Weave takes a different approach: instead of storing snapshots and diffing them, it stores **operations** (insert line, delete line) and uses a CRDT to guarantee that operations from any branch can be combined in any order and always converge to the same result.

The tradeoff: weave will never block you with a merge conflict, but it also won't catch semantic conflicts (two changes that are syntactically fine but logically incompatible). That's what your CI pipeline and tests are for.

```
         git                          weave
    ┌──────────┐                ┌──────────────┐
    │ snapshot  │                │  operations  │
    │  diffing  │                │   (CRDTs)    │
    ├──────────┤                ├──────────────┤
    │ 3-way    │                │ operation    │
    │ merge    │                │ replay       │
    ├──────────┤                ├──────────────┤
    │ CONFLICT │ ← this goes → │ always       │
    │ possible │    away        │ merges       │
    └──────────┘                └──────────────┘
```

## Install

**Download a binary** from the [latest release](https://github.com/ryanrudd/weave/releases/latest):

```bash
# macOS (Apple Silicon)
curl -L https://github.com/ryanrudd/weave/releases/latest/download/weave-macos-aarch64 -o weave

# macOS (Intel)
curl -L https://github.com/ryanrudd/weave/releases/latest/download/weave-macos-x86_64 -o weave

# Linux (x86_64)
curl -L https://github.com/ryanrudd/weave/releases/latest/download/weave-linux-x86_64 -o weave

chmod +x weave
sudo mv weave /usr/local/bin/
```

**Or build from source:**

```bash
git clone https://github.com/ryanrudd/weave.git
cd weave
cargo build --release
# binary is at target/release/weave
```

## Quick start

```bash
# Create a new repo
mkdir myproject && cd myproject
weave init

# Track a file and commit
echo "hello world" > hello.txt
weave add hello.txt
weave commit -m "Initial commit"

# Branch, edit, and merge — no conflicts
weave branch feature
weave checkout feature
echo -e "hello world\nfrom feature branch" > hello.txt
weave add hello.txt
weave commit -m "Feature work"

weave checkout main
echo -e "hello world\nfrom main branch" > hello.txt
weave add hello.txt
weave commit -m "Main work"

weave merge feature
weave cat hello.txt
# hello world
# from main branch
# from feature branch
```

Both branches' changes are preserved. No conflict resolution needed.

## Commands

| Command | Description |
|---------|-------------|
| `weave init` | Initialize a new repository |
| `weave add <file>` | Track a file (reads current contents from disk) |
| `weave commit -m "msg"` | Commit staged changes |
| `weave log` | Show commit history |
| `weave cat <file>` | Show a tracked file's contents |
| `weave branch <name>` | Create a new branch |
| `weave checkout <name>` | Switch to a branch |
| `weave merge <name>` | Merge a branch into the current branch |
| `weave branches` | List all branches |
| `weave status` | Show tracked files |

## How it works

Weave uses an **RGA (Replicated Growable Array)** CRDT at its core. Each line in a file is an element with:

- A globally unique ID (Lamport timestamp + site ID)
- A reference to what it was inserted after
- A tombstone flag for deletions

When two branches insert at the same position, the unique IDs break the tie deterministically. When one branch deletes a line and another inserts next to it, the tombstone preserves the reference so the insert still lands in the right place.

Operations are stored in commits (not snapshots). Merging replays the source branch's operations through the CRDT rather than diffing text — this is what makes it conflict-free.

### Architecture

```
┌─────────────────────────────┐
│      CLI (clap)             │
├─────────────────────────────┤
│      Repository             │
│  (branches, commits, HEAD)  │
├─────────────────────────────┤
│      Document               │
│  (file-level operations)    │
├─────────────────────────────┤
│      MergeStrategy trait    │  ← strategy pattern
├──────┬──────────┬───────────┤
│ Line │ Char     │ AST       │
│ CRDT │ (future) │ (future)  │
└──────┴──────────┴───────────┘
```

The `MergeStrategy` trait means the CRDT granularity is pluggable. Currently implemented: line-level. Character-level and AST-level are planned.

## Status

This is an experimental project exploring what version control looks like when you remove merge conflicts from the equation. It's not a replacement for git — it's a playground for ideas.

**What works:**
- Full branch/commit/merge workflow
- Conflict-free merging across divergent branches
- Disk persistence (`.weave` directory)
- 113 tests covering CRDT properties, edge cases, and integration scenarios

**What's next:**
- Landing page with deeper explanation
- Character-level CRDT strategy
- `weave diff` command
- TUI interface
- Repo-to-repo sync (the truly distributed part)

## License

MIT
