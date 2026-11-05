# GitHub workflows

Read the root `AGENTS.md` first. Inspect the actual workflow triggers and
permissions before editing; do not assume a workflow only runs tests.

- `workflows/ci.yml` runs the root verification recipes.
- `workflows/deploy-frontend.yml` builds and publishes GitHub Pages.
- `workflows/deploy-images.yml` publishes frontend/backend images on tags.

Keep verification separate from publication. Do not dispatch a publication
workflow or create a tag merely to test a change. Publishing needs to be part
of the requested work.

Preserve the least required token permissions. Do not expose secrets to
untrusted pull-request code or broaden write access to repair a failing check.
Treat cache, artifact and environment handling as part of the workflow's scope.

When using GitHub tools, explicitly select the repository and confirm branch/
base before a mutation. A local clone can have multiple remotes; do not rely on
a CLI's inferred repository for writes.

Keep workflow commands aligned with the `justfile` and package/Cargo manifests.
Do not claim unit tests replace a production frontend build or system tests.
Validate a workflow change using the affected workflow on its actual commit,
and distinguish queued, approval-blocked, failed and successful runs.

Update `README.md` or the relevant setup document when contributors' commands or
release behavior change. Do not add application or deployment changes to a
documentation-only task.
