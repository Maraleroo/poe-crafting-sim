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
- Comment: `az boards work-item update --id <id> --discussion "<text>"`
- Story points: `az boards work-item update --id <id> --fields "Microsoft.VSTS.Scheduling.StoryPoints=3"`
- Create task: `az boards work-item create --type Task --title "<title>" --description "<text>" --iteration "<iteration path of the story>" --output json`
- Link task to story: `az boards work-item relation add --id <task-id> --relation-type parent --target-id <story-id>`
- Query: `az boards query --wiql "SELECT [System.Id], [System.Title], [System.State] FROM workitems WHERE [System.Parent] = <id>"`

Rules:
- Only create Tasks. Never create User Stories, Features or Epics.
- Write titles and comments in German, matching the board.
