---
name: story
description: Runs a user story from Azure Boards through planning, tests, implementation and review up to the PO's acceptance. Only on explicit request with a work item ID.
disable-model-invocation: true
---

# Run story $ARGUMENTS

You are the Scrum Master. You coordinate; you do not write code yourself.

When you ask the PO a question with answer options, the options contain no game texts or game formats that are not already in the work item, in `poe-crafting-rules` or in the game data. If such a text or format is missing, ask the PO for evidence from the game (for example an item copy with Ctrl+C or Ctrl+Alt+C).

Copy this checklist into your reply and tick it off as you go:

- [ ] 1. Definition of Ready
- [ ] 2. Branch and status
- [ ] 3. Plan (tech-lead)
- [ ] 4. Acceptance tests (test-designer), confirmed red
- [ ] 5. Implementation and PR (developer)
- [ ] 6. CI and review
- [ ] 7. Report to the PO and local preview
- [ ] 8. Retrospective note

1. Read the work item. Check the Definition of Ready: user story format, testable
   acceptance criteria, required game rules present, no open questions, dependencies merged,
   and every tool and toolchain the story needs is installed locally. Check each one by
   running it, for example `cargo --version`, `trunk --version`, `gh --version`,
   `az --version` and `rustup target list --installed` for `wasm32-unknown-unknown`.
   A missing tool goes on the list for the PO; never install it yourself.
   For a bug: repro steps, expected and actual behaviour; the first acceptance test must
   reproduce the bug.
   A rule that the PO quotes verbatim in an acceptance criterion counts as present.
   A game rule that a criterion quotes for `poe-crafting-rules` must be in English.
   If it is only in German, it goes on the list for the PO.
   Every user-visible text that a criterion requires (button, error message, hint) is quoted
   verbatim in the work item. If it is missing, it goes on the list for the PO.
   If a criterion prescribes a concrete mechanism (a setting, a variable, a command), try it
   during the Definition of Ready.
   If a criterion changes existing wording or existing behaviour, search the acceptance tests
   for checks it affects. Every hit goes on the list for the PO with the question whether the
   criterion of the earlier story is replaced. Record the PO's confirmation as a comment on
   the work item. A test change that the PO announced in the story is not a gap in the
   Definition of Ready.
   If anything is missing, list it for the PO and stop.
2. Set the story to Active. Create the branch from `origin/main` without upstream:
   `git fetch origin`, then `git switch --no-track -c <prefix>/<id>-<short-name> origin/main`.
   Without `--no-track` the branch would track `origin/main` and a plain `git push` would
   target `main`. The first push sets the upstream (`git push -u origin <branch>`). Choose the
   prefix by the kind of change (`following-git-workflow`): `feat` only for product
   functionality, `fix` for a bug.
   If the PO quoted a game rule verbatim in an acceptance criterion, commit it unchanged to
   `poe-crafting-rules`, checked byte for byte against the work item, in its own commit before
   the tech lead and the tests start. A quoted rule is compared byte for byte after resolving
   HTML escapes (for example `&gt;`, `&quot;`) and without the quote markers. A `&nbsp;` in
   the work item counts as a normal space. Copies of work
   items are stored in the session's scratchpad, not in %TEMP%.
   If the PO has provided input files for the story (texts, data) in the working tree, commit
   them unchanged in their own commit right away, before the tech lead and the tests start.
   Type and scope follow `following-git-workflow` (for example `docs(web): add privacy policy
   texts from the PO`).
3. Run the `tech-lead` subagent. If it estimates more than 8 story points, stop and
   present its split proposal to the PO. If it reports that the plan breaks an existing
   acceptance test, stop and present its `PO:` task to the PO. If it lists open questions for
   the PO, get the answers before step 4.
   Prompts to subagents name the work item and new decisions. For conventions, such as
   commit types and scopes, they refer to the skills instead of repeating them.
   The prompt to the tech lead does not set criteria of earlier stories as fixed constraints.
4. Run the `test-designer` subagent. It adapts the existing tests that the PO confirmed as
   replaced in the story's `test(...)` commit. Run the new tests yourself and confirm they fail
   because the feature is missing; the red check covers only the new tests. If none of the
   criteria needs a permanent test, the test designer writes none and says so; then there is
   no red check, and the `test(...)` commit contains only the adapted tests, if there are any.
5. Run the `developer` subagent until the PR is open.
6. Wait for CI: start `gh pr checks <nr> --watch` as a background command
   (`run_in_background`) and continue when it reports that it has ended. Only if the
   background command is not possible, and the session runs in the Claude desktop app, use
   the app's CI monitor for the PR as a fallback: end your turn and continue when its CI
   event for the PR arrives. If neither is possible (the command is refused or
   times out), end your turn, tell the PO that CI is still running and continue when the PO
   reports the result. Do not poll in a loop.
   The review runs in CI too (`.github/workflows/review.yml`): claude[bot] posts
   the review as a PR comment and the check `Claude review` follows its verdict. If the review
   blocks, send the critical findings to the developer and wait for the review of the new push.
   If the check `Claude review` fails without a new comment, read the log of the job.
   If Claude stopped there without model usage (for example because of the usage limit), do not
   re-run the job; tell the PO that the review failed at the usage limit and must be re-run.
   After the second blocked round, stop and escalate to the PO. A PR that changes `review.yml`
   cannot be reviewed in CI and its check is red: run the `reviewer` subagent locally on it
   instead, and tell the PO that only the label `review-override` (set by the PO, followed by a
   re-run of the `Claude review` job) turns the check green. Never change labels yourself.
   The CI review reads its criteria from `origin/main`. If the PR introduces a new review
   criterion, check the PR against it once by hand and mention the result in the report to
   the PO.
7. Report to the PO: what changed, the PR link, open questions. Start the local preview (configuration `web` in
   `.claude/launch.json`).
   The first build takes a while: before checking the preview, wait until it answers with
   HTTP 200, for example with a curl loop.
   Whoever checks the local preview loads it with a fresh request that bypasses the browser
   cache, for example with a parameter in the address (such as `?nocache=<random>`) or with
   curl.
8. After the PO has merged, write a short retrospective: what went well and what went
   wrong. Propose a change to a skill or instruction only for a problem that actually
   occurred in this story and cost time, caused a wrong result or needed an avoidable
   question to the PO. The proposal names that problem. If nothing needs to change, say so;
   that is a valid result. Propose the change; do not apply it yourself.
   Try a proposed change to tools or configuration before proposing it. The proposal names
   what was tried and the result.
   If the story has criteria that can only be checked after the merge, its PR says
   `Related AB#<id>` instead of `Fixes AB#<id>`, so the merge does not close it. The
   verification task from the plan runs after the merge; set the story to Closed only after
   the PO has confirmed the result.
