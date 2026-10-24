# 0005: AI review in CI

- Status: accepted
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
  `pull_request` on `main` (types `opened`, `synchronize`, `reopened`). Never
  `pull_request_target`. The job has no condition and is never skipped: a skipped job
  counts as passing a required check, so a skipped review would look like an approval.
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
  and the final step always succeeds. There are no label triggers: after setting
  `review-override`, the PO re-runs the `Claude review` job.
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
  in depth, by deny rules in `.claude/settings.json` for label changes, including `--label` on
  `gh pr create` and `gh issue create` and `gh api graphql` (`gh pr edit` stays allowed for the
  PR description). The CI reviewer has no tool that can change labels.

## Amendment (AB#24, 2026-10-20)
- The deny rules in `.claude/settings.json` are only a safeguard for the hard rule in
  `CLAUDE.md`, not the control itself. They match the command text, so other spellings slip
  through, for example combined short flags such as `gh pr create -lreview-override`. The
  rule in `CLAUDE.md` applies regardless of whether a deny rule catches a command; the deny
  list is not extended to every possible spelling.
- Changes to `.claude/settings.json` (allow and deny rules) go through the tech lead's plan:
  the plan names the change and its reason as a step, and the PR shows it in the diff.

## Amendment (AB#25, 2026-10-20)
- Short label flag: the deny list also catches the short flag `-l` of `gh pr create` and
  `gh issue create` when the value is attached (`-lreview-override`, `-l=review-override`),
  not only `-l review-override`. This narrows the AB#24 amendment: the attached short form is
  covered because it is the most likely other spelling; the list is still not extended to every
  possible spelling. The patterns are:
  - `Bash(gh pr create -l*)` and `Bash(gh issue create -l*)`: the flag right after the subcommand;
  - `Bash(gh pr create * -l*)` and `Bash(gh issue create * -l*)`: the flag after a space anywhere
    later in the command.
  They replace `Bash(gh pr create *-l *)` and `Bash(gh issue create *-l *)`, which they include.
  The leading space is deliberate: an unanchored `*-l*` would also block PR descriptions with
  words such as "file-level", "text-level" or "multi-line", which this project uses often.
  Known false positive: a description that contains a space followed by `-l` (for example
  "ls -l"); use `--body-file` for such a description. Known gaps: combined short flags
  (`-dl review-override`), a quoted flag (`"-lreview-override"`), a flag after a line break
  without indentation, and label changes through other tools. The hard rule in `CLAUDE.md`
  covers them.
- Re-run hint: every red `Claude review` check names the override path in its error message:
  the PO adds the label `review-override` and then re-runs the `Claude review` job, because the
  label is only read while the job runs.
- Removing the label: the label is only read while the job runs. When the PO removes
  `review-override` after a green run, the check stays green until the next run of the
  `Claude review` job (a new push or a manual re-run); only that run can turn it red. To make
  the removal take effect at once, the PO re-runs the job after removing the label.
- Dependabot PRs (ADR 0006): the `Claude review` check is red on every Dependabot PR, for
  three independent reasons:
  1. Workflows started by Dependabot get Dependabot secrets, not Actions secrets, so
     `CLAUDE_CODE_OAUTH_TOKEN` is empty.
  2. `claude-code-action` refuses runs started by a bot unless the bot is listed in its input
     `allowed_bots` (default: none).
  3. Updates of `actions/checkout` or `anthropics/claude-code-action` change `review.yml`
     (validation mismatch, see above).
  Adding the token as a Dependabot secret alone would therefore not help, and it is not done.
  For a Dependabot PR the PO checks that CI is green and reads the diff and the release notes
  (or asks the Scrum Master to run the local `reviewer` subagent), then adds `review-override`,
  re-runs the `Claude review` job and merges. Reviewing Dependabot PRs in CI (`allowed_bots`
  plus a Dependabot secret) would be a separate story, and it would still not cover updates
  that change `review.yml`.

## Amendment (AB#100, 2026-10-24)
- Model: the CI review runs with Claude Opus (`--model opus` in `claude_args`), the same model as
  the local `reviewer` subagent in `.claude/agents/reviewer.md`. Before, the action's default
  model (Sonnet) was used. Every review run therefore takes longer and uses more of the PO's
  Claude subscription.
- Denied tool calls: the action hides Claude's messages by default (`show_full_output: false`);
  its result summary only shows `permission_denials_count`. A step after the Claude step reads
  the action's execution file (step output `execution_file`) and prints every denied tool call
  as one line: the tool name and its input as compact JSON, truncated to a fixed length. The step
  also prints the number of denied calls when it is 0, so the log shows that the check ran. Tool
  results (file contents, command output) are never printed. Every printed line starts with
  fixed text, so text that Claude wrote cannot become a workflow command (`::...`).
- Secrets in the log: GitHub masks registered secrets in the job log: the Actions secret
  `CLAUDE_CODE_OAUTH_TOKEN`, the workflow token and the Claude App token (the action registers
  it with `core.setSecret`). A denied tool input is text that Claude wrote; it can only contain
  a secret if Claude has read one, and the public review comment would then be the larger
  channel. Masking does not catch transformed values (for example base64); this residual risk
  is accepted.
- Checkout credentials: the checkout step uses `persist-credentials: false`. The review only
  runs local git commands (`git show origin/main:...`, `git log`, `git diff`) on the history
  that `fetch-depth: 0` fetches during the checkout, and `gh` authenticates with the token in
  its environment, so no step needs a stored git credential. Without this setting,
  `actions/checkout` v7 keeps the workflow token in a credentials file under the runner's
  temporary directory, included through `includeIf.gitdir` in `.git/config`. The action's
  own removal of checkout credentials only looks at `include.path` and reported "No existing
  authentication headers to remove" on PR #17, so that token stayed usable and readable while
  Claude ran. Limit: the action itself writes the Claude App token into the `origin` URL in
  `.git/config` before Claude runs ("Updated remote URL with authentication token"), and `gh`
  needs a token in Claude's environment. `persist-credentials: false` cannot prevent either;
  the App token is revoked at the end of the action step. The only switch in the action that
  avoids the token in the URL, `allowed_non_write_users`, lets users without write access
  trigger the action and is not used.
- Not chosen for the denied calls: `show_full_output: true`, because it prints all messages
  including tool results, and the action itself warns that secrets can appear in this public
  log; uploading the execution file as an artifact, because artifacts are not masked;
  `display_report`, because it puts Claude-authored content into the step summary.
- Posting the comment: Claude writes the review comment with the Write tool to one file in the
  runner's temporary directory and posts it with `gh pr comment <n> --body-file <file>`. The
  tool list gains one rule for exactly this file and nothing else: `Edit(//<runner temp>/review-comment.md)`.
  Claude Code checks file writes, including the Write tool, against `Edit` rules only; a rule
  `Write(<path>)` is accepted but never matches (Claude Code documentation, "Permissions", section
  "Read and Edit"), so it would leave Write denied. The tool `Write` without a path is not allowed.
  The `//` prefix marks an absolute path. A comment passed inline
  with `--body` contains backticks and quotes from the comment format, which the shell treats as
  command substitution or quoting; such a call may not match `Bash(gh pr comment *)` and is
  then denied. A file avoids shell quoting entirely.
- Exactly one comment: the verdict step counts the `claude[bot]` comments created during the
  run. The check is red when there is none or more than one. When the check is red because the
  verdict or the comment is missing or does not match, its message also names the denied tool
  calls of the run, so the reason is visible on the check without opening the log.
- Cause on PR #17 (AB#100 criterion 1): the logs of the runs on PR #17 show for both BLOCKED
  attempts one denied tool call (`permission_denials_count: 1`), a verdict in the structured
  output and no `claude[bot]` comment, but not which call was denied, because the action hid
  the tool calls and the execution file was not kept. The cause is determined after the merge
  from the log of the denied calls.

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
- Fork PRs get no secrets and no OIDC token; they are not reviewed. Their check ends red
  (no verdict), so they cannot pass unnoticed.
