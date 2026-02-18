# Repository Policies

## Documentation Synchronization

`README.md` (English) and `README_ru.md` (Russian) must remain equivalent documents.  
If one is updated, update the other in the same commit.

## GitHub CI

If there is an issue with GitHub CI checks (a GitHub run failed):
- First, always list failed checks: `gh run list --event pull_request --json databaseId,conclusion,displayTitle,startedAt --jq '.[] | select(.conclusion=="failure") | {id:.databaseId, title:.displayTitle, startedAt: .startedAt}'`. GitHub creates a new run with a new ID each time, so you must fetch the list first and inspect the current (latest) run.
- Take the most recent run ID and open logs: `gh run view <CHECK_ID> --log-failed`.
- Analyze the logs and fix the root cause.
