# Personal fork workflow (peinan/herdr)

Fork-local instructions layered on top of `AGENTS.md` (the upstream-maintained
conventions that `CLAUDE.md` symlinks to). `AGENTS.md` stays authoritative for
engineering practice — code style, testing, screen detection, protocol, docs.
This file only covers how *this fork* is branched, synced, and landed, and
**wins over `AGENTS.md` wherever the two describe the same fork-local workflow**
(notably branch targets and the "land on master" step).

This file is committed on `develop` only. It never exists on `master`, so it
never collides with upstream. Do not add it to `.gitignore`.

## Fork model

Personal fork of upstream `ogulcancelik/herdr`. Follow upstream's engineering
conventions by default; deviate only where this file says so. Two branches, two
roles:

- **`master`** — mirrors upstream `ogulcancelik/herdr`. No personal commits ever
  land here. Keep it a clean fast-forward of upstream.
- **`develop`** — the personal integration line: all customization and feature
  work, stacked on top of `master`. This is the default working branch.

`origin` is `peinan/herdr`. There is no `upstream` remote, and none is needed.

## Syncing `master` from upstream

1. On the `peinan/herdr` GitHub page, use **"Sync fork"** to fast-forward the
   fork's `master` from `ogulcancelik/herdr`.
2. Pull it down and carry the new upstream base into `develop`:

```bash
git checkout master && git pull          # fast-forward local master
git checkout develop && git merge master # merge, not rebase: develop is published
git push origin develop
```

Prefer **merge** over rebase: `develop` is pushed to `origin/develop` and may
have open PRs/worktrees built on it, so its history must stay stable.

## Feature work: one worktree per feature

New features and customizations go in a dedicated worktree off `develop`, not in
the main checkout.

- **Location:** `../herdr-worktrees/<slug>` (sibling of the main checkout). The
  directory `<slug>` is the branch name **without** the `feat/` prefix — branch
  `feat/zoom-indicator` → `../herdr-worktrees/zoom-indicator`.
- **Branch:** `feat/<slug>` off `develop`, or `issue/<id>-<slug>` when a GitHub
  issue exists.
- Do all edits, tests, and commits inside the worktree.

```bash
git worktree add ../herdr-worktrees/<slug> -b feat/<slug> develop
```

Opening a PR is encouraged — the work is disposable. If an approach doesn't pan
out, just delete the branch and worktree; nothing is lost.

```bash
git worktree remove ../herdr-worktrees/<slug>
git branch -D feat/<slug>          # and: git push origin --delete feat/<slug>
```

## Pull requests (intra-fork → `develop`)

PRs target **`develop` inside the fork**, never upstream and never `master`.

`gh` defaults a fork PR to the upstream parent, and `gh pr create` **hangs** in
non-interactive/background runs (it prompts for the base repo with no TTY).
Create PRs via the API instead — non-interactive and intra-fork by construction:

```bash
gh api repos/peinan/herdr/pulls \
  -f title='feat: ...' -F body=@<body-file> \
  -f head=<branch> -f base=develop --jq '.html_url'
```

`head` with no owner prefix stays in `peinan/herdr`. If `develop` is local-only
on a fresh clone, `git push origin develop` first. Interactive equivalent, only
with a real TTY: `gh pr create --repo peinan/herdr --base develop`.

## Carried over from `AGENTS.md` (with one override)

- **Override the "land on master" step.** `AGENTS.md` (Multi-agent isolation)
  says to fast-forward a shared checkout and push `origin/master`. In this fork,
  personal work lands on `develop` via the intra-fork PR above — never by
  pushing `master`. `master` only moves via upstream sync.
- **Commit style still applies:** lowercase conventional commits, no emoji, **no
  AI co-author line**. Use `refs #<n>` (not `fixes`/`closes`) and only for real
  issues, not discussions. Propose the commit message and get alignment first.
- **Builds/tests need `ZIG` set** — the mise-installed zig is not on the
  non-interactive PATH:

```bash
ZIG=~/.local/share/mise/installs/zig/latest/bin/zig just check
```
