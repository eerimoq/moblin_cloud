# Scene editor

A copy of Moblin's scene editor (`SceneEditorView.swift` and friends in the Moblin repo) in the phone replica, replacing the whole screen while open like Moblin's full screen cover. `scripts/moblin_ports.json` says what each file ports; see `just moblin-drift` in the root `AGENTS.md`.

- It edits a local draft of the device's scenes and widgets (`store.ts`, `createEditor`) and syncs it after a change, then at most once a second, so a slider or a dragged widget follows along on the phone. There is no save button. Only what differs from what was last synced is sent, one request at a time, and a failure is shown and retried after 5 s.
- A new widget's `createWidget` carries as `sceneIds` the scenes already on the device that the draft added it to, like Moblin's wizard, and `adoptSceneWidgets` then takes the scene widget ids Moblin made, keeping the draft's layout.
- Moblin clears the Scene widgets pointing at a scene it deletes, so `removeScene` clears them in both the draft and `synced` and no `updateWidget` is sent: a Moblin older than 40.0.0-462 refuses a `sceneId` of `null`, and a refused update blocks the sync before it reaches `deleteScene`.
- `data/moblinSettings.ts` holds the types of Moblin's `SettingsScene` and `SettingsWidget` JSON. `normalizeScene` and `normalizeWidget` fill missing keys with Moblin's decode defaults and deterministic ids for missing UUIDs, so a refetch never looks like a change.
- `update*` replaces whole objects, and the synthesized Codable sub objects need every field.
- `encodeWidget` and `encodeScene` port Moblin's hand-written encoders: they leave out what Moblin itself would not write and keep any field they do not know, so a newer Moblin's settings survive an update.
- The canvas size comes from the editor options' `canvasSize`. The outlines are never mirrored, since the preview is the stream's own image, not the phone's mirrored front camera view.
- Per type forms in `settings/widgets/` are one file per widget type, named like its directory in Moblin's `Settings/Scenes/Widgets/Widget/`.
- The Numeric input toggle is kept in the browser, since the protocol does not carry Moblin's setting. `EditorOptions` carries the phone state Moblin's views read.
- The Scene widgets quick button's panel (`SceneWidgetsPanel.tsx`, Moblin's `QuickButtonSceneWidgetsView`) shares the scene editor's `Editor` with the full screen editor: `MoblinScreen` keeps one while either is open, so opening the editor from the panel never races the panel's unsent changes, and the draft is flushed when the last of them closes. The panel lists the phone's selected scene, not the editor's `sceneId`, like Moblin.

- Assistants with only `view` access open the scene editor and the Scene widgets panel read-only: `MoblinScreen` provides `ReadOnly` from `Form.tsx`, which the form rows and the forms' own controls disable themselves for while navigation stays usable. The toolbar hides what edits, the canvas only selects, the store ignores edits, and picking a scene only shows it instead of switching the phone's scene.

Left out on purpose:

- Controls that act on the phone's running effects rather than on settings, in the forms and in the Scene widgets panel's quick controls: text widget timers, stopwatches, checkboxes, ratings and lap times, the wheel's spin and results, the scoreboard's score and clock, the Pomodoro controls, the alert Test button, the interactive browser widgets toggle, Twitch rewards and TTS Monster and ElevenLabs voices. The wheel's shuffle and the bingo card's marks are settings, so the panel has them.
- The Scene widgets panel's Scenes shortcut, since Moblin's scenes settings view is not ported; the scene editor covers it.
- Picking files: the protocol has no file uploads, so images, VTuber and PNGTuber models, gallery media and alert videos are picked on the phone. The forms show what the phone has, and the gallery has no previews or playback.
