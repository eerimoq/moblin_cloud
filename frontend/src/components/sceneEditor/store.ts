import {
  createMemo,
  createRoot,
  createStore,
  onCleanup,
  reconcile,
  snapshot,
  type Accessor,
  type StoreSetter,
  runWithOwner,
} from "solid-js";
import { removeWhere } from "../../store";
import { BackendError } from "../../data/backend";
import {
  encodeScene,
  encodeWidget,
  normalizeScene,
  normalizeWidget,
  sameId,
  type AlertsMediaGallery,
  type EditorOptions,
  type ScoreboardPlayer,
  type SettingsScene,
  type SettingsWidget,
  type VideoSource,
} from "../../data/moblinSettings";
import { t } from "../../i18n";
import type { Control } from "../../data/account";

export type EditorState = {
  loading: boolean;
  loaded: boolean;
  error: string | null;
  scenes: SettingsScene[];
  widgets: SettingsWidget[];
  videoSources: VideoSource[];
  gallery: AlertsMediaGallery | null;
  options: EditorOptions | null;
  scoreboardPlayers: ScoreboardPlayer[];
  macros: { id: string; name: string }[];
  currentMic: string | null;
  preview: string | null;
  portrait: boolean;
  sceneId: string | null;
  selection: string | null;
  showingSettings: boolean;
  effectWizard: string | null;
  snapshots: Record<string, string | null>;
  syncing: boolean;
  dirty: boolean;
};

export type Editor = {
  state: EditorState;
  set: StoreSetter<EditorState>;
  reload: () => Promise<void>;
  loadSnapshot: (widgetId: string, again?: boolean) => void;
  request: <T>(control: Control) => Promise<T>;
  send: (control: Control) => Promise<void>;
  secrets: Accessor<boolean>;
  readOnly: Accessor<boolean>;
  scene: Accessor<SettingsScene | undefined>;
  selectScene: (id: string) => void;
  editScene: (id: string, change: (scene: SettingsScene) => void) => void;
  editWidget: (id: string, change: (widget: SettingsWidget) => void) => void;
  addScene: (scene: SettingsScene) => void;
  removeScene: (id: string) => void;
  addWidget: (widget: SettingsWidget) => void;
  removeWidget: (id: string) => void;
  setScoreboardPlayers: (players: ScoreboardPlayer[]) => void;
  flush: () => Promise<void>;
  dispose: () => void;
};

export const clone = <T>(value: T): T => structuredClone(snapshot(value));

export const same = (first: unknown, second: unknown) =>
  JSON.stringify(first) === JSON.stringify(second);

export const dataUrl = (data: string) => `data:image/jpeg;base64,${data}`;

export const errorText = (error: unknown) =>
  error instanceof BackendError && [403, 409, 422, 502, 504].includes(error.status)
    ? error.message
    : t("Something went wrong. Try again.");

export const mergeFresh = <T extends { id: string }>(fresh: T[], draft: T[], old: T[]): T[] => {
  const kept: T[] = [];
  for (const item of fresh) {
    const current = draft.find((candidate) => sameId(candidate.id, item.id));
    const previous = old.find((candidate) => sameId(candidate.id, item.id));
    kept.push(current && previous && !same(current, previous) ? current : item);
  }
  for (const item of draft) {
    const previous = old.find((candidate) => sameId(candidate.id, item.id));
    const inFresh = fresh.some((candidate) => sameId(candidate.id, item.id));
    if (!inFresh && (!previous || !same(item, previous))) {
      kept.push(item);
    }
  }
  const draftOrder = draft.map((item) => item.id.toUpperCase());
  const oldOrder = old.map((item) => item.id.toUpperCase());
  if (!same(draftOrder, oldOrder)) {
    kept.sort((first, second) => {
      const a = draftOrder.indexOf(first.id.toUpperCase());
      const b = draftOrder.indexOf(second.id.toUpperCase());
      return (a < 0 ? Infinity : a) - (b < 0 ? Infinity : b);
    });
  }
  return kept;
};

export function createEditor(
  request: <T>(control: Control) => Promise<T>,
  control: (control: Control) => Promise<void>,
  currentScene: Accessor<string | undefined>,
  secrets: Accessor<boolean>,
  readOnly: Accessor<boolean> = () => false,
): Editor {
  return runWithOwner(null, () =>
    createRoot((dispose) => {
      const [state, setState] = createStore<EditorState>({
        loading: true,
        loaded: false,
        error: null,
        scenes: [],
        widgets: [],
        videoSources: [],
        gallery: null,
        options: null,
        scoreboardPlayers: [],
        macros: [],
        currentMic: null,
        preview: null,
        portrait: false,
        sceneId: null,
        selection: null,
        showingSettings: false,
        effectWizard: null,
        snapshots: {},
        syncing: false,
        dirty: false,
      });
      let dirty = false;
      const setDirty = (value: boolean) => {
        dirty = value;
        setState((draft) => {
          draft.dirty = value;
        });
      };
      let synced = {
        scenes: [] as SettingsScene[],
        widgets: [] as SettingsWidget[],
        players: [] as ScoreboardPlayer[],
      };
      let timer: ReturnType<typeof setTimeout> | undefined;
      let syncing: Promise<void> | null = null;
      let lastSync = 0;
      let pendingSwitch: string | null = null;
      let disposed = false;

      const fetchAll = async () => {
        const [scenes, widgets, videoSources, gallery, options] = await Promise.all([
          request<{ scenes: Record<string, unknown>[] }>({ getScenes: {} }),
          request<{ widgets: Record<string, unknown>[] }>({ getWidgets: {} }),
          request<{ videoSources: VideoSource[] }>({ getVideoSources: {} }),
          request<{ gallery: AlertsMediaGallery }>({ getAlertsMediaGallery: {} }),
          request<{ options: EditorOptions }>({ getEditorOptions: {} }),
        ]);
        return {
          scenes: scenes.scenes.map(normalizeScene),
          widgets: widgets.widgets.map(normalizeWidget),
          videoSources: videoSources.videoSources,
          gallery: gallery.gallery,
          options: options.options,
        };
      };

      const load = async () => {
        try {
          const fresh = await fetchAll();
          if (disposed) {
            return;
          }
          const scenes = mergeFresh(fresh.scenes, clone(state.scenes), synced.scenes);
          const widgets = mergeFresh(fresh.widgets, clone(state.widgets), synced.widgets);
          synced = {
            scenes: clone(fresh.scenes),
            widgets: clone(fresh.widgets),
            players: clone(fresh.options.scoreboardPlayers),
          };
          setState((draft) => {
            reconcile(scenes, "id")(draft.scenes);
            reconcile(widgets, "id")(draft.widgets);
            draft.videoSources = fresh.videoSources;
            draft.gallery = fresh.gallery;
            draft.options = fresh.options;
            if (same(draft.scoreboardPlayers, synced.players)) {
              draft.scoreboardPlayers = fresh.options.scoreboardPlayers;
            }
            draft.loaded = true;
            draft.error = null;
          });
        } catch (error) {
          setState((draft) => {
            draft.error = errorText(error);
          });
        } finally {
          setState((draft) => {
            draft.loading = false;
          });
        }
      };

      const reload = async () => {
        setState((draft) => {
          draft.loading = true;
        });
        await load();
      };

      const loadSnapshot = (widgetId: string, again = false) => {
        const key = widgetId.toUpperCase();
        let known = false;
        setState((draft) => {
          known = key in draft.snapshots;
          if (!known) {
            draft.snapshots[key] = null;
          }
        });
        if (!again && known) {
          return;
        }
        void request<{ image: string | null }>({ getVideoSourceSnapshot: { id: widgetId } })
          .then((answer) => {
            if (!disposed) {
              setState((draft) => {
                draft.snapshots[key] = answer.image ? dataUrl(answer.image) : null;
              });
            }
          })
          .catch(() => {});
      };

      const scene = createMemo(() => {
        const edited = state.sceneId
          ? state.scenes.find((candidate) => sameId(candidate.id, state.sceneId))
          : undefined;
        if (edited) {
          return edited;
        }
        const current = currentScene();
        return state.scenes.find((candidate) => candidate.enabled && sameId(candidate.id, current));
      });

      const markDirty = () => {
        setDirty(true);
        if (timer !== undefined || syncing || disposed) {
          return;
        }
        const wait = lastSync + 1000 - Date.now();
        if (wait <= 0) {
          void sync();
        } else {
          timer = setTimeout(() => {
            timer = undefined;
            void sync();
          }, wait);
        }
      };

      const step = async (send: Control, after: () => void) => {
        await request(send);
        after();
      };

      const adoptSceneWidgets = async (
        widgetId: string,
        sceneIds: string[],
        scenes: SettingsScene[],
      ) => {
        const fresh = await request<{ scenes: Record<string, unknown>[] }>({ getScenes: {} });
        for (const freshScene of fresh.scenes.map(normalizeScene)) {
          const added = freshScene.widgets.find((sceneWidget) =>
            sameId(sceneWidget.widgetId, widgetId),
          );
          if (!added || !sceneIds.some((id) => sameId(id, freshScene.id))) {
            continue;
          }
          synced.scenes = synced.scenes.map((scene) =>
            sameId(scene.id, freshScene.id) ? freshScene : scene,
          );
          const adopt = (scene: SettingsScene) => {
            scene.widgets = scene.widgets.map((sceneWidget) =>
              sameId(sceneWidget.widgetId, widgetId)
                ? { ...added, ...sceneWidget, id: added.id }
                : sceneWidget,
            );
          };
          const local = scenes.find((scene) => sameId(scene.id, freshScene.id));
          if (local) {
            adopt(local);
          }
          const index = state.scenes.findIndex((scene) => sameId(scene.id, freshScene.id));
          if (index < 0) {
            continue;
          }
          const placeholderId = state.scenes[index].widgets.find((sceneWidget) =>
            sameId(sceneWidget.widgetId, widgetId),
          )?.id;
          setState((draft) => {
            adopt(draft.scenes[index]);
            if (placeholderId !== undefined && sameId(draft.selection, placeholderId)) {
              draft.selection = added.id;
            }
          });
        }
      };

      const sync = async () => {
        if (syncing) {
          return;
        }
        const run = async () => {
          lastSync = Date.now();
          setState((draft) => {
            draft.syncing = true;
          });
          setDirty(false);
          try {
            await Promise.resolve();
            const scenes = clone(state.scenes);
            const widgets = clone(state.widgets);
            const players = clone(state.scoreboardPlayers);
            for (const widget of widgets) {
              const old = synced.widgets.find((candidate) => sameId(candidate.id, widget.id));
              if (!old) {
                const sceneIds = scenes
                  .filter(
                    (scene) =>
                      synced.scenes.some((candidate) => sameId(candidate.id, scene.id)) &&
                      scene.widgets.some((sceneWidget) => sameId(sceneWidget.widgetId, widget.id)),
                  )
                  .map((scene) => scene.id);
                await step({ createWidget: { widget: encodeWidget(widget), sceneIds } }, () =>
                  synced.widgets.push(widget),
                );
                if (sceneIds.length > 0) {
                  await adoptSceneWidgets(widget.id, sceneIds, scenes);
                }
              } else if (!same(old, widget)) {
                await step({ updateWidget: { widget: encodeWidget(widget) } }, () => {
                  synced.widgets = synced.widgets.map((candidate) =>
                    sameId(candidate.id, widget.id) ? widget : candidate,
                  );
                });
              }
            }
            for (const scene of scenes) {
              const old = synced.scenes.find((candidate) => sameId(candidate.id, scene.id));
              if (!old) {
                await step({ createScene: { scene: encodeScene(scene) } }, () =>
                  synced.scenes.push(scene),
                );
              } else if (!same(old, scene)) {
                await step({ updateScene: { scene: encodeScene(scene) } }, () => {
                  synced.scenes = synced.scenes.map((candidate) =>
                    sameId(candidate.id, scene.id) ? scene : candidate,
                  );
                });
              }
            }
            const removedScenes = synced.scenes.filter(
              (old) => !scenes.some((scene) => sameId(scene.id, old.id)),
            );
            for (const old of removedScenes) {
              await step({ deleteScene: { id: old.id } }, () => {
                synced.scenes = synced.scenes.filter((candidate) => !sameId(candidate.id, old.id));
              });
            }
            const removedWidgets = synced.widgets.filter(
              (old) => !widgets.some((widget) => sameId(widget.id, old.id)),
            );
            for (const old of removedWidgets) {
              await step({ deleteWidget: { id: old.id } }, () => {
                synced.widgets = synced.widgets.filter(
                  (candidate) => !sameId(candidate.id, old.id),
                );
                synced.scenes = synced.scenes.map((scene) => ({
                  ...scene,
                  widgets: scene.widgets.filter(
                    (sceneWidget) => !sameId(sceneWidget.widgetId, old.id),
                  ),
                }));
              });
            }
            if (!same(players, synced.players)) {
              await step({ setScoreboardPlayers: { players } }, () => {
                synced.players = players;
              });
            }
            if (
              pendingSwitch &&
              synced.scenes.some((scene) => scene.enabled && sameId(scene.id, pendingSwitch))
            ) {
              const id = pendingSwitch;
              pendingSwitch = null;
              if (sameId(state.sceneId, id)) {
                setState((draft) => {
                  draft.sceneId = null;
                });
              }
              void control({ setScene: { id } });
            }
            setState((draft) => {
              draft.error = null;
            });
          } catch (error) {
            setState((draft) => {
              draft.error = errorText(error);
            });
            setDirty(true);
            if (!disposed) {
              timer = setTimeout(() => {
                timer = undefined;
                void sync();
              }, 5000);
            }
          } finally {
            setState((draft) => {
              draft.syncing = false;
            });
          }
        };
        syncing = run();
        try {
          await syncing;
        } finally {
          syncing = null;
        }
        if (dirty && timer === undefined) {
          markDirty();
        }
      };

      const flush = async () => {
        while (syncing) {
          await syncing;
        }
        if (timer !== undefined) {
          clearTimeout(timer);
          timer = undefined;
        }
        if (dirty) {
          await sync();
        }
      };

      const editor: Editor = {
        state,
        set: setState,
        reload,
        loadSnapshot,
        request,
        send: control,
        secrets,
        readOnly,
        scene,
        selectScene: (id) => {
          const viewOnly = readOnly();
          const current = currentScene();
          let switchNow = false;
          setState((draft) => {
            const picked = draft.scenes.find((candidate) => sameId(candidate.id, id));
            if (!picked) {
              return;
            }
            if (viewOnly) {
              draft.sceneId = id;
            } else if (
              picked.enabled &&
              !synced.scenes.some((scene) => scene.enabled && sameId(scene.id, id))
            ) {
              draft.sceneId = id;
              pendingSwitch = id;
            } else if (picked.enabled) {
              draft.sceneId = null;
              pendingSwitch = null;
              switchNow = !sameId(current, id);
            } else {
              draft.sceneId = id;
              if (sameId(pendingSwitch, id)) {
                pendingSwitch = null;
              }
            }
          });
          if (switchNow) {
            void control({ setScene: { id } });
          }
        },
        editScene: (id, change) => {
          if (readOnly()) {
            return;
          }
          let found = false;
          setState((draft) => {
            const scene = draft.scenes.find((scene) => sameId(scene.id, id));
            if (scene) {
              change(scene);
              found = true;
            }
          });
          if (found) {
            markDirty();
          }
        },
        editWidget: (id, change) => {
          if (readOnly()) {
            return;
          }
          let found = false;
          setState((draft) => {
            const widget = draft.widgets.find((widget) => sameId(widget.id, id));
            if (widget) {
              change(widget);
              found = true;
            }
          });
          if (found) {
            markDirty();
          }
        },
        addScene: (scene) => {
          if (readOnly()) {
            return;
          }
          setState((draft) => {
            draft.scenes.push(scene);
          });
          markDirty();
        },
        removeScene: (id) => {
          if (readOnly()) {
            return;
          }
          const clearScene = (widgets: SettingsWidget[]) => {
            for (const widget of widgets) {
              if (sameId(widget.scene.sceneId, id)) {
                widget.scene.sceneId = null;
              }
            }
          };
          setState((draft) => {
            removeWhere(draft.scenes, (scene) => sameId(scene.id, id));
            clearScene(draft.widgets);
            if (sameId(draft.sceneId, id)) {
              draft.sceneId = null;
            }
          });
          clearScene(synced.widgets);
          markDirty();
        },
        addWidget: (widget) => {
          if (readOnly()) {
            return;
          }
          setState((draft) => {
            draft.widgets.push(widget);
          });
          markDirty();
        },
        removeWidget: (id) => {
          if (readOnly()) {
            return;
          }
          setState((draft) => {
            removeWhere(draft.widgets, (widget) => sameId(widget.id, id));
            for (const scene of draft.scenes) {
              removeWhere(scene.widgets, (sceneWidget) => sameId(sceneWidget.widgetId, id));
            }
          });
          markDirty();
        },
        setScoreboardPlayers: (players) => {
          if (readOnly()) {
            return;
          }
          setState((draft) => {
            draft.scoreboardPlayers = players;
          });
          markDirty();
        },
        flush,
        dispose: () => {
          disposed = true;
          if (timer !== undefined) {
            clearTimeout(timer);
          }
          dispose();
        },
      };
      onCleanup(() => {
        disposed = true;
      });
      void load();
      return editor;
    }),
  );
}
