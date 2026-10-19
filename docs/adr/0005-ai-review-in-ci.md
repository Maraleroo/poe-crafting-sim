# 0005: AI review in CI

- Status: proposed
- Date: 2026-10-19
- Work item: AB#16

## Context
Until now the Scrum Master runs the `reviewer` subagent locally and the review comment is
posted under the PO's GitHub account. Every PR on `main` should instead get an automatic
Claude review in CI, posted by the Claude bot account, and the review verdict should be a
check on the PR. The Claude GitHub App is installed, the secret `CLAUDE_CODE_OAUTH_TOKEN`
exists, and `.github/workflows/claude.yml` already uses `anthropics/claude-code-action@v1`
for `@claude` mentions.

Facts from the action's documentation and source (v1):
- Comments appear as `claude[bot]` only when the action gets its GitHub token from the
  Claude App through the OIDC token exchange, which needs `id-token: write` and no
  `github_token` input. Passing `github_token` makes comments appear under that token's
  identity.
- The OIDC token exchange is refused with a "workflow validation" error when the workflow
  file of the run does not match the one on the default branch. The action then logs a
  warning, skips Claude and ends the step successfully, without any output. The action
  says this is expected for new workflows and for PRs that change the workflow.
- With `--json-schema` in `claude_args`, the action exposes Claude's validated answer as
  the step output `structured_output`.
- The app token is revoked at the end of the action step, so later steps cannot use it.

## Decision
- A separate workflow `.github/workflows/review.yml`, job `review`, triggered by
  `pull_request` on `main` (types `opened`, `synchronize`, `reopened`, `labeled`,
  `unlabeled`; label events only when the label is `review-override`). Never
  `pull_request_target`. Only PRs from branches of this repository are reviewed.
- Permissions: workflow level `permissions: {}`; job level exactly `contents: read`,
  `pull-requests: read`, `id-token: write`. The comment is written with the Claude App
  token, so the workflow token needs no write permission.
- `claude-code-action@v1` with `claude_code_oauth_token` and without `github_token`.
  Full history checkout (`fetch-depth: 0`) so the reviewer can find the `test(...)` commit.
  Claude may only read files, run read-only `git log` / `git diff` / `git show`,
  `gh pr view`, `gh pr diff` and `gh pr comment`. `--setting-sources user` keeps the
  repository's own permission rules (which allow `git commit` and `git push`) out of the
  CI run, so the tool list above is the complete list.
- The review rules (`.claude/skills/reviewing-code/SKILL.md` and `.claude/agents/reviewer.md`)
  are read from `origin/main`, not from the PR, so a PR cannot weaken its own review.
- Verdict: Claude posts exactly one comment in the `reviewing-code` format and returns
  `{"verdict": "APPROVED" | "BLOCKED" | "ESCALATED"}` as structured output. A final shell
  step reads the newest `claude[bot]` comment created during this run and requires that
  its first line is `Review: <verdict>`. The step fails when the verdict is `BLOCKED`, or
  when no verdict or no matching comment exists (fail closed: a skipped or crashed review
  never looks like an approval).
- Override: when the PR has the label `review-override` (read at run time with `gh pr view`,
  so a re-run sees a newly added label), the review still runs and posts its findings,
  and the final step always succeeds.
- A PR that changes `review.yml` cannot be reviewed in CI (validation mismatch, see above).
  Its review check is red with an explanatory message; the Scrum Master runs the
  `reviewer` subagent locally as a fallback, or the PO adds `review-override`.
- Testing follows ADR 0004: the workflow file is a file-level contract and is checked by
  acceptance tests in `crafting-core/tests/acceptance/story_16.rs` (no new dependency;
  line-based checks). Behaviour that needs a real run (comment author, check colour,
  override, test-change detection) is verified by the PO on a verification PR after the
  merge, because the workflow cannot run on the PR that adds it.

## Decisions confirmed by the PO
- Fail closed: a PR that changes `review.yml` (including the PR that introduces it) gets a red
  `Claude review` check with an explanatory message. The fallback is the local `reviewer`
  subagent, or the PO adding `review-override`.
- Label rule: only the PO creates, adds and removes the label `review-override`, because it
  switches the check to green. This is enforced by a hard rule in `CLAUDE.md` and, as defense
  in depth, by deny rules in `.claude/settings.json` for label changes (`gh pr edit` stays
  allowed for the PR description). The CI reviewer has no tool that can change labels.

## Alternatives considered
- Passing `github_token: ${{ secrets.GITHUB_TOKEN }}`: works on the PR that adds the
  workflow, but comments appear as `github-actions[bot]`, not as the Claude bot.
- Verdict only from the posted comment, or only from structured output: either one alone
  can drift from what the PO sees. Requiring both to agree keeps the check and the first
  line of the comment consistent.
- Treating a skipped review as green: simpler on workflow PRs, but a missing review would
  look like an approval.
- Adding the review as a job to `ci.yml`: every change to `ci.yml` would then also skip the
  review (validation mismatch), and the review would share CI's concurrency rules.

## Consequences
- The review comment is posted by `claude[bot]` and the check `Claude review` reflects the
  verdict. To make BLOCKED PRs unmergeable, the PO can require this check on `main`.
- The workflow cannot be tested on the PR that introduces it; the first real run is on the
  verification PR after the merge. Every later PR that changes `review.yml` needs the
  local fallback.
- Every push to a PR runs a full review on the PO's Claude subscription.
- The CI reviewer cannot read Azure Boards; it relies on the acceptance criteria copied
  into the PR description (PR template) and on the acceptance tests.
- Fork PRs get no secrets and no OIDC token; they are not reviewed.
