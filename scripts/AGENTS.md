# Maintenance scripts

Read the root `AGENTS.md` first. Paths below are relative to the repository root.
These scripts maintain translations and ports from Moblin's Swift source.

## Translations

`scripts/translate.py` finds literal frontend strings and HTML titles, then uses
the configured Claude tooling to update `frontend/src/locales/`.

After adding or changing localized frontend strings, use the project's
`just machine-translate` workflow. Check the generated locale diff and preserve
keys, placeholders and intentional untranslated content. Do not run it for
unrelated code/documentation edits or overwrite unrelated locale changes.

If the required tooling or authorized service access is unavailable, report
which translations remain outstanding instead of claiming they were generated.
This is the translation workflow; architecture notes describe the mechanism
and must not add a different translation policy.

## Moblin ports

`scripts/moblin_drift.py` uses `scripts/moblin_ports.json` to map local ports to
Swift paths and commits in the configured Moblin checkout.

Use `just moblin-drift` to inspect changes; select `--ref`/`--diff` when needed.
`--mark` writes the tracking file. Only mark a port after reviewing and applying
the relevant Swift changes and checking the corresponding local behavior.
Do not silence missing-path, missing-map or ancestry errors by blindly advancing
tracked commits.

Use the existing Python check configuration. Test changed argument handling or
mapping behavior without invoking live translation services unnecessarily.
Update the port map when adding a covered scene-editor file, and keep the
relevant architecture note current.
