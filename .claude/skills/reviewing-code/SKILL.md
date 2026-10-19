---
name: reviewing-code
description: Review checklist for pull requests in this repository, with the only criteria that may block a merge and the required comment format. Use for every pull request review. The review runs in CI.
---

# Reviewing code

## Critical: blocks the merge
1. An acceptance criterion is not met, or an acceptance test was changed, deleted or disabled.
2. Expected test values are hard-coded, or special cases exist only for tests.
3. Game mechanics contradict `poe-crafting-rules` or the imported data.
4. Security: a secret in code or logs, a workflow using `pull_request_target`,
   or workflow permissions broader than needed.
5. Quality gates bypassed: clippy warnings silenced globally, CI steps removed.

## Never blocking
Style, naming, refactoring ideas, performance without a measurement, "could also" ideas.
Report these as warnings or suggestions, at most five.

## Rounds and override
- If two earlier reviews on this PR were BLOCKED, do not block again: verdict ESCALATED.
- If the PR has the label `review-override`, report findings but use verdict APPROVED.

## Comment format
First line: `Review: APPROVED`, `Review: BLOCKED` or `Review: ESCALATED`.
Then one line per finding:
`[critical|warning|suggestion] <file>:<line>: <problem and why> (criterion <n>)`
The review runs in CI (`.github/workflows/review.yml`) and is posted by claude[bot]. The check
`Claude review` is red only when the first line is `Review: BLOCKED` (or when no valid review
was posted); with the label `review-override` it is always green. Only the PO sets that label.
PRs that change `review.yml` cannot be reviewed in CI; the `reviewer` subagent reviews them locally.
