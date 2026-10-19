---
name: following-git-workflow
description: Branch naming, Conventional Commits and pull request conventions for this repository. Use whenever creating branches, commits or pull requests.
---

# Git workflow

## Branches
- One branch per work item, created from `main`: `<prefix>/<id>-<short-name>`.
- Prefixes: `feat` (user story), `fix` (bug), `chore` (maintenance and team configuration),
  `ci` (CI and workflows only), `docs` (documentation only).
- Push with `git push -u origin <branch>`. Never push to `main`. Never force-push.

## Commits (Conventional Commits)
Format: `<type>(<scope>): <description>` in English, imperative, lower case, no period.
- Types: `feat`, `fix`, `test`, `refactor`, `perf`, `docs`, `ci`, `build`, `chore`.
- Scopes: `core`, `web`, `data`, `ci`, `docs`.
- Breaking change: `!` after the scope, e.g. `feat(web)!: change share link format`.
- Examples: `feat(core): add transmutation orb`, `test(core): add acceptance tests for AB#12`.

## Pull requests
- Title: one Conventional Commit line. It becomes the squash commit on `main`.
- Body: the PR template. Must contain `Fixes AB#<id>`.
- Create with `gh pr create --title "<title>" --body-file <file>`.
- Never merge. Merging is the PO's acceptance.
