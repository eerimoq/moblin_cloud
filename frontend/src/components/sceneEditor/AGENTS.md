# Scene editor

Read the root and frontend `AGENTS.md` before changing this subtree.
Implementation details are in
[scene editor architecture](../../../../docs/architecture/frontend-scene-editor.md).

These components port Moblin's Swift editor. Consult the corresponding Swift
source and `scripts/moblin_ports.json` before changing behavior. Keep settings
serialization, selection, coordinate transforms and device synchronization
compatible with the app. Preserve intentional omissions documented in the note.

Use the existing editor state and controls. Check affected conversions and
synchronization with targeted frontend tests and the production build. Report
any device behavior that could not be exercised. Follow `scripts/AGENTS.md`
before advancing tracked Swift commits, and update the architecture note when
the port changes.
