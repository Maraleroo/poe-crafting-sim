---
name: managing-azure-boards
description: Exact az boards commands for reading and updating work items in this project's Azure Boards. Use whenever a work item must be read, created, linked, estimated, commented or moved to another state.
---

# Azure Boards

Organization and project are preconfigured with `az devops configure`. Work items are in German.
The warning "Auto-detect was enabled but no Azure DevOps remote was found" is expected: the code
lives on GitHub, so az falls back to the configured defaults. Ignore it.
Process: Agile. States for User Story: New, Active, Resolved, Closed, Removed.
States for Task: New, Active, Closed, Removed.

- Read: `az boards work-item show --id <id> --output json`
- Set state: `az boards work-item update --id <id> --state Active`
- Comment: `az boards work-item update --id <id> --discussion "[Claude · <Rolle>] <text>"`
- Story points: `az boards work-item update --id <id> --fields "Microsoft.VSTS.Scheduling.StoryPoints=3"`
- Create task: `az boards work-item create --type Task --title "<title>" --description "<text>" --iteration "<iteration path of the story>" --output json`
- Link task to story: `az boards work-item relation add --id <task-id> --relation-type parent --target-id <story-id>`
- Query: `az boards query --wiql "SELECT [System.Id], [System.Title], [System.State] FROM workitems WHERE [System.Parent] = <id>"`

Console output on Windows:
- On Windows, the Azure CLI prints text in Windows-1252, not UTF-8. `az.cmd` starts Python in
  isolated mode, so environment variables have no effect. In Git Bash, pipe the output through
  `iconv -f WINDOWS-1252 -t UTF-8`. An environment variable such as `PYTHONUTF8` in
  `.claude/settings.json` does not change this.
- A wrongly displayed character in the console does not mean that the stored data is broken.
- Verbatim copies from work items are checked byte for byte against the work item.

Lossless copy of work item fields (UTF-8):
- Write the request to a file in the session's scratchpad, for example `req.json` with
  `{"ids":[<id>],"fields":["System.Title","System.Description","Microsoft.VSTS.Common.AcceptanceCriteria"]}`.
  Then run:
  `az devops invoke --area wit --resource workitemsbatch --http-method POST --api-version 7.1 --in-file req.json --out-file result.json`
  The result file is UTF-8 and loses no characters. The endpoint only reads, although it uses POST.
- Use this command for every verbatim copy from a work item.
- iconv cannot restore characters outside Windows-1252.

Rules:
- Only create Tasks. Never create User Stories, Features or Epics.
- Write titles and comments in German, matching the board.
- Every comment an agent writes starts with `[Claude · <Rolle>]`, for example
  `[Claude · Scrum Master]`, `[Claude · Tech Lead]`, `[Claude · Test Designer]` or
  `[Claude · Developer]`. All comments appear under the PO's name, so the prefix shows who wrote them.
