# frontend scene editor

[Architecture index](../architecture.md). Keep this note current with its implementation.

**Scene editor** (`components/sceneEditor/`): a copy of Moblin's scene editor
(`SceneEditorView.swift` and friends in the Moblin repo) in the phone replica, opened by
the Scene editor quick button (needs `control` access and a connected device) and
replacing the whole screen while open, like Moblin's full screen cover. It edits a local
copy of the device's scenes and widgets (`store.ts`, `createEditor`: `getScenes`,
`getWidgets`, `getVideoSources`, `getAlertsMediaGallery` and `getEditorOptions` on open)
and syncs the draft to the device at once after a change and then at most once a second,
so a slider or a dragged widget follows along on the phone (`sync`: `createWidget`,
`updateWidget`, `createScene`, `updateScene`, `deleteScene`, `deleteWidget` and
`setScoreboardPlayers`, only for what differs from what was last synced, one request at
a time, a failure shown in a notice and retried after 5 s; a new widget's `createWidget`
carries as `sceneIds` the scenes already on the device that the draft added it to, like
Moblin's create widget wizard, and `adoptSceneWidgets` then takes the scene widget ids
Moblin made from `getScenes`, keeping the draft's layout); there is no save button.
Picking an enabled scene sends `setScene` at once (a scene not on the device yet once it
is), so the preview shows it under the outlines, and a disabled scene is edited over the
"Disabled scene" cover. The types of Moblin's `SettingsScene` and `SettingsWidget` JSON
with their defaults are `data/moblinSettings.ts` (`normalizeScene` and `normalizeWidget`
fill missing keys with Moblin's decode defaults, deterministic ids for missing UUIDs, so
a refetch never looks like a change; `update*` replaces whole objects, and the
synthesized Codable sub objects need every field); the sync sends them through
`encodeWidget` and `encodeScene`, ports of Moblin's hand-written encoders, which leave
out what Moblin itself would not write: the sub objects of the other widget types (none
for an image widget), the sub objects of the other video effect types and the camera ids
of the other camera positions (of a scene and of the video source, VTuber and PNGTuber
widgets), keeping any field they do not know, so a newer Moblin's settings survive an
update; `data/sceneResolver.ts`, `data/widgetOutlines.ts` and
`data/sceneEditorGeometry.ts` are ports of Moblin's `SceneResolver`,
`updateWidgetOutlines`/`widgetOutlineSize`/`layoutPosition` and
`SceneEditorGeometry.swift` (the canvas size comes from the editor options'
`canvasSize`; the outlines are never mirrored, since the preview is the stream's own
image, not the phone's mirrored front camera view). `SceneEditor.tsx` is the layout
(video area with the preview, the 350 pt settings panel, the 52 pt toolbar),
`Canvas.tsx` the outlines with tap, move and resize (snapping and guides), `Toolbar.tsx`
the buttons and the scenes, add widget and alignment popovers (`Popover.tsx`, rendered
outside the toolbar's scroll column), `Stack.tsx` the panel's navigation stack,
`Form.tsx` the iOS form parts (sections, rows, pickers, sliders, `ValueEditCompact`,
colors, reorderable lists with a trash button in place of swipe to delete),
`settings/SceneSettings.tsx` and `settings/SceneWidgetSettings.tsx` Moblin's scene and
layout pages, `settings/WidgetSettings.tsx` dispatching to the per type forms in
`settings/widgets/` (one file per widget type, named like its directory in Moblin's
`Settings/Scenes/Widgets/Widget/`, `shared.tsx` the rows several share, `effects.tsx`
with the crop and mask editors over the widget's video source snapshot,
`getVideoSourceSnapshot` kept in `snapshots` and refetched with the mask canvas's
refresh button, or `public/gamla-linkoping.jpg`, Moblin's placeholder, for other
widgets) and `wizards/` the create scene, create widget and create effect sheets (the
effect one opened from a widget's Effects section through `effectWizard` in the store,
since the sheet covers the whole editor). Left out on purpose: the controls that act on
the phone's running effects rather than on settings (text widget timers, stopwatches,
checkboxes, ratings and lap times, the wheel's spin, shuffle and results, the Pomodoro
controls, the alert Test button, the interactive browser widgets toggle, Twitch rewards,
TTS Monster and ElevenLabs voices) and picking files (the image widget's image, the
VTuber and PNGTuber models, the alert media gallery's custom images and sounds and alert
videos: the forms show what the phone has and say to pick it there, the gallery only
lists them, and the widget wizard creates image, VTuber and PNGTuber widgets without
one). `public/alert-face.png` is Moblin's face image for the alert position editor. The
editor options carry the phone state Moblin's views read (`EditorOptions` in
`data/moblinSettings.ts`): the text widget's workout warning (`TextFormatWarnings`), the
quick switch group warning, the inactive video source badge and the Apple workout heart
rate suggestion and variable only on an iPhone. The Numeric input toggle is kept in the
browser (`sceneNumericInput` in `localStorage`), since the protocol does not carry
Moblin's setting, and the media gallery has no previews or playback, since it does not
carry the media either.
