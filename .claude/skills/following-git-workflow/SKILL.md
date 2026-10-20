---
name: following-git-workflow
description: Branch naming, Conventional Commits and pull request conventions for this repository. Use whenever creating branches, commits or pull requests.
---

# Git workflow

## Branches
- One branch per work item: `<prefix>/<id>-<short-name>`, created from `origin/main`
  without upstream: `git fetch origin`, then
  `git switch --no-track -c <prefix>/<id>-<short-name> origin/main`. Without `--no-track`
  the branch would track `origin/main` and a plain `git push` would target `main`.
- The prefix is the commit type of the main change: `feat` (product functionality),
  `fix` (bug in the product), `chore` (maintenance and team configuration),
  `ci` (CI and workflows only), `docs` (documentation only).
- Push with `git push -u origin <branch>`. Never push to `main`. Never force-push.

## Commits (Conventional Commits)
Format: `<type>(<scope>): <description>` in English, imperative, lower case, no period.
- The type follows the kind of change, not the kind of work item:
  - `feat`: new functionality of the product (the simulator). Never for team, CI or docs changes.
  - `fix`: a bug in the product.
  - `test`: tests only. `refactor`, `perf`: product code without a behaviour change.
  - `docs`: documentation only (README, ADRs).
  - `ci`: GitHub workflows.
  - `build`: Cargo, Trunk and dependencies.
  - `chore`: maintenance, including the team configuration.
- Scopes: `core` (`crafting-core/`), `web` (`web/`), `data` (`data/`),
  `team` (`.claude/` and `CLAUDE.md`). The scope is optional.
- No scope that repeats the type: `ci: add review workflow`, not `ci(ci): ...`.
- Breaking change: `!` after the scope, e.g. `feat(web)!: change share link format`.
- Examples: `feat(core): add transmutation orb`, `test(core): add acceptance tests for AB#12`,
  `chore(team): add verification task to the tech lead plan`, `docs: add ADR 0006`.

## Pull requests
- Title: one Conventional Commit line. It becomes the squash commit on `main`.
- Body: the PR template. Must contain `Fixes AB#<id>`. If the story has acceptance criteria
  that can only be checked after the merge (the tech lead's plan says so), write
  `Related AB#<id>` instead, so the merge does not close the story.
- Always create with `gh pr create --title "<title>" --body-file <file>`. Never pass the body
  with `--body`, `-b` or a heredoc on the command line: shell quoting breaks backticks and
  Markdown. Write the body file outside the repository (scratchpad or temp directory), so it
  is never committed.
- Never merge. Merging is the PO's acceptance.
