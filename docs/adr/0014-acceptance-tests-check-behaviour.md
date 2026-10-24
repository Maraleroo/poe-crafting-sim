# 0014: Acceptance tests check behaviour, confirmed replacements without override

- Status: accepted
- Date: 2026-10-24
- Work item: AB#121

## Context
Existing acceptance tests are protected: no agent may change them, and the review blocks every
change to one (ADR 0005); only the PO overrides that with the label `review-override`. The rule
keeps the developer from weakening a test to make the implementation pass.

In AB#79, AB#80 and AB#113 the PO had to change existing acceptance tests and set
`review-override`. Every conflict came from a test that fixes exact wording: a line in
`CLAUDE.md`, the footer text, an error message, a rule line. Tests of game mechanics never
caused a conflict. ADR 0004 asks for "the exact texts from the acceptance criteria" in the
translation file, and the lean text-level checks for process stories (ADR 0004) mostly compare
sentences in team files.

## Decision
- Permanent acceptance tests check behaviour and structure: game mechanics and statistics,
  privacy, the security of the workflows, and that texts come from translation or game data
  files instead of being hard-coded.
- They do not check the exact wording of game rules, UI texts, game texts or team files. That
  wording is checked at the acceptance: rules byte for byte in step 2 of the `story` skill,
  everything else in the review against the criteria in the PR and by the PO in the preview.
  This replaces "the exact texts from the acceptance criteria" in ADR 0004 for new tests.
- Exception: legal texts (disclaimer, privacy policy, Impressum) are still checked word for
  word by permanent tests.
- A story whose criteria are only wording gets no permanent test; there is then no red check.
- When the PO confirms for a story that one of its criteria replaces a criterion of an earlier
  story (comment on the work item), the test designer adapts the old test in the story's
  `test(...)` commit, before any implementation exists. The PR description names each adapted
  test with the replaced and the replacing criterion. The review does not block such a change;
  every other change to an existing acceptance test still blocks, and only `review-override`
  turns it green (ADR 0005). The developer never changes acceptance tests.
- Existing wording tests are not cleaned up in advance. A story that runs into one follows the
  procedure above.

## Alternatives considered
- Keep wording tests and the override for every replacement: safe, but almost every PR that
  touches team files or texts needs the PO's code change and the label.
- Remove all existing wording tests at once: fewer conflicts later, but a large change to
  protected tests without a story that needs it.
- Let the developer adapt old tests after the PO's confirmation: simpler, but it removes the
  separation between the one who writes the tests and the one who makes them pass.

## Consequences
- Wording errors in team files and UI texts are no longer caught by `cargo test`; the review
  and the PO's preview catch them.
- The CI review cannot read Azure Boards. It accepts an adapted test on the basis of the
  `test(...)` commit and the PR description; the PO checks the confirmation on the work item
  before merging.
- Older test files keep their wording checks until a story replaces them.
