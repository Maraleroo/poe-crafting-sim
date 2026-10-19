---
name: story
description: Runs a user story from Azure Boards through planning, tests, implementation and review up to the PO's acceptance. Only on explicit request with a work item ID.
disable-model-invocation: true
---

# Run story $ARGUMENTS

You are the Scrum Master. You coordinate; you do not write code yourself.
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
   acceptance criteria, required game rules present, no open questions, dependencies merged.
   For a bug: repro steps, expected and actual behaviour; the first acceptance test must
   reproduce the bug.
   If anything is missing, list it for the PO and stop.
2. Set the story to Active. Create the branch from `main`: `feat/<id>-<short-name>` for a
   user story, `fix/<id>-<short-name>` for a bug.
3. Run the `tech-lead` subagent. If it estimates more than 8 story points, stop and
   present its split proposal to the PO.
4. Run the `test-designer` subagent. Run the tests yourself and confirm they fail
   because the feature is missing.
5. Run the `developer` subagent until the PR is open.
6. Wait for CI. The review runs there too (`.github/workflows/review.yml`): claude[bot] posts
   the review as a PR comment and the check `Claude review` follows its verdict. If the review
   blocks, send the critical findings to the developer and wait for the review of the new push.
   After the second blocked round, stop and escalate to the PO. A PR that changes `review.yml`
   cannot be reviewed in CI and its check is red: run the `reviewer` subagent locally on it
   instead, and tell the PO that only the label `review-override` (set by the PO, followed by a
   re-run of the `Claude review` job) turns the check green. Never change labels yourself.
7. Report to the PO: what changed, the PR link, open questions. Start the local preview.
8. After the PO has merged, write a short retrospective: what went wrong, which skill
   or instruction should change. Propose the change; do not apply it yourself.
