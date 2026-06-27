# gitv

A fast, native Git GUI focused on commit graph visualisation.
Built with Iced (GPU-rendered UI) and git2 (libgit2 bindings).

## Usage

```bash
gitv [directory]   # open a specific repo
gitv               # open current directory
```

## MVP

- Commit graph - DAG with branch/tag labels, lane colouring
- Commit detail panel - message, author, timestamp, changed files
- Diff viewer - per-file unified diff
- Stage/unstage, commit
- Fetch, pull, push (current branch)
- Branch create/checkout/delete
- Tags - display on graph, create/delete, push

## Planned

- Merge, rebase, squash
- Stash push/pop/drop
- Cherry pick
- Amend last commit
- Commit search/filter
- Syntax highlighting in diff
- Keyboard shortcuts

## Out of scope

- Pull requests or any forge API (GitHub, GitLab)
- Multiple repos simultaneously
- Clone UI - use `git clone` then `gitv <dir>`
- SSH key management
- Git LFS / submodules
- Conflict resolution UI
- Git blame
