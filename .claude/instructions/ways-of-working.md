---
title: "Ways of Working"
type: instruction
status: active
date: 2026-07-01
---

# Ways of Working

## Golden rule — never commit to main

**ALWAYS** work on a branch. Direct commits to `main` are forbidden. Every change — no matter how small — must go through a branch, a PR, and merge into `main`.

## Worktree-first sessions

**Every agent session must call `EnterWorktree` before making any file change in this repo, and must merge back into `main` (see Branch workflow) before finishing.** Multiple sessions — interactive
and background, or several people/agents — can share one local checkout. Without a worktree, a session that switches or commits on a branch changes `HEAD` for every other session pointed at the
same directory.

- Call `EnterWorktree` as the first action of any session that will `Write`/`Edit` a file or run a mutating `Bash` command (branch, commit, etc.) — read-only exploration first is fine.
- Untracked files do not carry over into a new worktree automatically. If a session already has uncommitted new files before calling `EnterWorktree`, copy them into the worktree directory after
  entering it (they live in a separate working tree, sharing only the `.git` object store).
- Run `mise trust` once per worktree directory before the first commit. `mise`'s trust is path-scoped, so a freshly created worktree path starts untrusted, and `hk`'s `mise`-shelled pre-commit jobs
  fail until it's trusted.
- Finish inside the worktree per Branch workflow below, then `ExitWorktree action: "keep"`, squash-merge from the main checkout, and clean up (see After merge — squash-merged worktree branches need
  `-D`, not `-d`).
- **Caveat:** this is enforced by discipline, not a hard technical gate, for interactive/foreground sessions.

## Branch workflow

This repo has a remote (`origin` = `thoroc/street-view-movie-maker` on GitHub) — branch, push, and open a PR like any normal repo.

1. **Start inside a worktree, from the latest `main`.** Call `EnterWorktree` (see Worktree-first sessions above) — it creates an isolated worktree already branched from `main`. Do not manually run
   `git checkout -b` in the shared checkout; fetch the latest state before branching:

   ```bash
   git checkout main && git pull && git checkout -b <type>/<short-description>
   ```

   Never branch from a stale local `main` — a `git pull` right before `checkout -b` is mandatory.

2. Use conventional prefixes: `feat/`, `fix/`, `docs/`, `refactor/`, `chore/`.

3. Commit as you go — small, atomic commits with conventional messages:

   ```text
   feat(scorer): add D9 mutation coverage
   fix(hook): regenerate index on plan changes
   docs: update README install section
   ```

4. If `main` has diverged, rebase instead of merging:

   ```bash
   git fetch origin && git rebase origin/main
   ```

   This keeps history linear. Resolve conflicts if they arise.

5. Run checks before pushing:

   ```text
   hk check -c       # read-only gate: cargo fmt + clippy (see hk.pkl)
   ```

   Use `hk check -c` (or `HK_FIX=0 hk check`) for a non-mutating run; a bare `hk check` defaults to fix mode and will rewrite files. `hk fix` applies fixes to the working tree on demand. `cargo check`,
   `cargo clippy`, and `cargo test` run at `pre-push`, not `check` — see `hk.pkl` for the exact hook-to-job mapping before assuming a given check runs at a given stage.

6. Push and open a PR:

   ```bash
   git push -u origin <branch-name>
   ```

   Use `gh pr create` or push and open via GitHub.

## Merge strategy — always squash

**ALWAYS squash merge into `main`.** Every PR lands as a single squashed commit, regardless of how many commits are on the branch. This means:

- Do not reshape or squash the branch locally to reduce commit count. Small, atomic commits on the branch are encouraged; they collapse into one commit on `main` at merge time.
- Set the merge to squash when opening or merging the PR (`gh pr merge --squash`, or "Squash and merge" in the GitHub UI).
- The squashed commit message should be a conventional message summarising the whole change.

## Keeping plans and findings in sync with implementation

When you implement what a plan describes, update its frontmatter `status: active → done` in the same PR. The `context-index` hk step will regenerate `.context/index.yaml` on the next commit.

The same rule applies to any `FINDING` that fed the work: once the plan or fix built from a finding's recommendation has shipped, flip that finding's `status` to `done` too (or `superseded` if a
later finding replaced its conclusions), in the same PR. Add a short `## Outcome` section noting what shipped and where (a commit, a PR, or files created as a result — e.g. the ADRs a
`adr-capture` recommendation produced). A finding is investigative input, not a standing task list; leaving it `active` after its recommendation is fully acted on is just as misleading as a stale
plan, and nothing else in this workflow catches that mismatch either.

This also applies when the connection isn't obvious. Before committing a change, check `.context/index.yaml` for active plans, findings, or follow-ups that overlap the files or problem you just
touched, even if you weren't working "from" that plan/finding or arrived at the fix by a different route. If your change satisfies part or all of an active plan's scope, update that plan in the
same commit: mark the relevant part done, correct any design description that no longer matches what actually shipped, and note the commit that did it. Do not leave a plan or finding describing
work as still-to-do once the work is done, no matter how it got done — either can be independently resolved by an unrelated commit that never references it.

## After merge

**Deleting the merged branch is mandatory, not optional — never leave merged branches straggling.** As soon as a branch lands in `main`, delete it locally (GitHub auto-deletes remote branches after PR
merge):

```bash
git checkout main && git pull && git branch -d <branch-name>
```

`git branch -d` is a safe delete: it refuses any branch not fully merged, so it will never drop unmerged work — use this form for a repo whose merges are real merge commits, not squashes.

**Squash-merged branches in this repo need `-D`, not `-d`.** Because the golden path here is squash-merge into `main` (see Merge strategy below), the branch's own commits are never ancestors of the
new squashed commit — `git branch -d` will refuse with "not fully merged" even though the content landed correctly. Once you've confirmed the squash commit is on `main` (e.g.
`git log --oneline -1 main` shows it), force-delete is expected and safe:

```bash
git worktree remove <worktree-path>   # if the branch was checked out in a worktree — required before deleting the branch
git branch -D <branch-name>
```

If merged branches have accumulated, prune them in one pass. `EnterWorktree` names branches `worktree-<name>`, not `worktree-agent-*` — match that pattern (adjust if your naming differs):

```bash
git branch --merged main | grep -vE '^\*|main$' | xargs -r git branch -d   # true merges
git branch | grep -E '^\s*worktree-' | sed 's/^\*\?\s*//' | xargs -r -n1 git branch -D   # squash-merged worktree branches — verify each is actually merged first
```
