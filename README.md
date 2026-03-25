# weave

What if your branches could always merge?

Weave is a version control system that uses [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type) instead of three-way merge. The result: no merge conflicts. Two branches can edit the same file, diverge for as long as they want, and merge cleanly every time.

The catch is that "cleanly" means structurally, not semantically. Weave guarantees the merge produces a deterministic result, but it can't know if your code still makes sense. That's what tests and CI are for. The bet is that most merge conflicts in practice are just textual noise, and getting them out of the way is worth it.

This is an experiment, not a git replacement. It's a toy built to explore an idea.

## How it's different

Git stores snapshots. When you merge, it diffs two snapshots against a common ancestor and hopes the changes don't overlap. When they do, you get a conflict.

Weave stores operations — "insert this line after that one" and "delete this line." Under the hood it's an [RGA](https://hal.inria.fr/inria-00555588/document) (Replicated Growable Array), a type of CRDT designed for ordered sequences. Every operation has a globally unique ID, so when two branches insert at the same spot, there's always a deterministic tiebreaker. Deletes leave tombstones so that concurrent inserts nearby don't lose their anchor point.

Merging is just replaying one branch's operations into another. The CRDT handles the ordering. There's no diffing, no three-way comparison, and no conflicts.

## Try it

Grab a binary from the [latest release](https://github.com/ryanrudd/weave/releases/latest):

```bash
# pick your platform
curl -L https://github.com/ryanrudd/weave/releases/latest/download/weave-macos-aarch64 -o weave  # apple silicon
curl -L https://github.com/ryanrudd/weave/releases/latest/download/weave-macos-x86_64 -o weave   # intel mac
curl -L https://github.com/ryanrudd/weave/releases/latest/download/weave-linux-x86_64 -o weave   # linux

chmod +x weave && sudo mv weave /usr/local/bin/
```

Or build from source (`cargo build --release`).

## The demo

```bash
mkdir demo && cd demo
weave init

echo "hello world" > hello.txt
weave add hello.txt
weave commit -m "first commit"

# make a branch and edit the file
weave branch feature
weave checkout feature
echo -e "hello world\nfrom feature" > hello.txt
weave add hello.txt
weave commit -m "feature work"

# go back to main, make a different edit
weave checkout main
echo -e "hello world\nfrom main" > hello.txt
weave add hello.txt
weave commit -m "main work"

# merge — no conflict
weave merge feature
weave cat hello.txt
```

Output:
```
hello world
from main
from feature
```

Both edits survived. No resolution step.

## TUI

There's also a terminal UI. Run `weave tui` inside a repo.

You can browse files (with modification status), scroll through commit history, switch branches, merge, create branches, add files, and commit — all without leaving the interface. Press `?` for keybindings.

## Commands

```
weave init                 create a new repo
weave add <file>           stage a file from disk
weave commit -m "msg"      commit staged changes
weave log                  show commit history
weave cat <file>           print a tracked file
weave branch <name>        create a branch
weave checkout <name>      switch branches
weave merge <name>         merge a branch in
weave branches             list branches
weave status               show tracked files
weave tui                  interactive terminal UI
```

## Architecture

The merge strategy is behind a trait, so the CRDT granularity is pluggable. Right now it's line-level (each CRDT element = one line of text). Character-level and AST-level are on the roadmap.

```
CLI / TUI
  -> Repository (branches, commits, history)
    -> Document (per-file operations)
      -> MergeStrategy trait
        -> LineCRDT (implemented)
        -> CharCRDT (planned)
        -> AstCRDT  (planned)
```

## Status

Working: init, add, commit, branch, checkout, merge, log, TUI. 113 tests. Disk persistence via `.weave` directory.

Not yet: networking/sync between repos, diff command, character-level merging.

This is a learning project. If you find it interesting or want to poke at it, PRs and issues are welcome.

## License

MIT
