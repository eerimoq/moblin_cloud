import { batch, createMemo, createRoot, createSignal, onCleanup, type Accessor } from "solid-js";
import { createStore, produce, reconcile, unwrap, type SetStoreFunction } from "solid-js/store";
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
  set: SetStoreFunction<EditorState>;
  reload: () => Promise<void>;
  loadSnapshot: (widgetId: string, again?: boolean) => void;
  request: <T>(control: Control) => Promise<T>;
  send: (control: Control) => Promise<void>;
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

const clone = <T>(value: T): T => structuredClone(unwrap(value));

const same = (first: unknown, second: unknown) => JSON.stringify(first) === JSON.stringify(second);

export const dataUrl = (data: string) => `data:image/jpeg;base64,${data}`;

export const errorText = (error: unknown) =>
  error instanceof BackendError && [403, 409, 422, 502, 504].includes(error.status)
    ? error.message
    : t("Something went wrong. Try again.");

export function createEditor(
  request: <T>(control: Control) => Promise<T>,
  control: (control: Control) => Promise<void>,
  currentScene: Accessor<string | undefined>,
): Editor {
  return createRoot((dispose) => {
    const [state, set] = createStore<EditorState>({
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
    const [, setVersion] = createSignal(0);

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

    const mergeFresh = <T extends { id: string }>(fresh: T[], draft: T[], old: T[]): T[] => {
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

    const reload = async () => {
      set("loading", true);
      try {
        const fresh = await fetchAll();
        if (disposed) {
          return;
        }
        batch(() => {
          const scenes = mergeFresh(fresh.scenes, clone(state.scenes), synced.scenes);
          const widgets = mergeFresh(fresh.widgets, clone(state.widgets), synced.widgets);
          synced = {
            scenes: clone(fresh.scenes),
            widgets: clone(fresh.widgets),
            players: clone(fresh.options.scoreboardPlayers),
          };
          set("scenes", reconcile(scenes, { key: "id" }));
          set("widgets", reconcile(widgets, { key: "id" }));
          set("videoSources", fresh.videoSources);
          set("gallery", fresh.gallery);
          set("options", fresh.options);
          set(
            "scoreboardPlayers",
            same(state.scoreboardPlayers, synced.players)
              ? fresh.options.scoreboardPlayers
              : state.scoreboardPlayers,
          );
          set("loaded", true);
          set("error", null);
        });
        setVersion((version) => version + 1);
      } catch (error) {
        set("error", errorText(error));
      } finally {
        set("loading", false);
      }
    };

    const loadSnapshot = (widgetId: string, again = false) => {
      const key = widgetId.toUpperCase();
      if (!again && key in state.snapshots) {
        return;
      }
      if (!(key in state.snapshots)) {
        set("snapshots", key, null);
      }
      void request<{ image: string | null }>({ getVideoSourceSnapshot: { id: widgetId } })
        .then((answer) => {
          if (!disposed) {
            set("snapshots", key, answer.image ? dataUrl(answer.image) : null);
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
      set("dirty", true);
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
        const placeholder = state.scenes[index].widgets.find((sceneWidget) =>
          sameId(sceneWidget.widgetId, widgetId),
        );
        batch(() => {
          set("scenes", index, produce(adopt));
          if (placeholder && sameId(state.selection, placeholder.id)) {
            set("selection", added.id);
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
        set("syncing", true);
        set("dirty", false);
        try {
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
              synced.widgets = synced.widgets.filter((candidate) => !sameId(candidate.id, old.id));
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
          if (pendingSwitch && synced.scenes.some((scene) => sameId(scene.id, pendingSwitch))) {
            const id = pendingSwitch;
            pendingSwitch = null;
            if (sameId(state.sceneId, id)) {
              set("sceneId", null);
            }
            void control({ setScene: { id } });
          }
          set("error", null);
        } catch (error) {
          set("error", errorText(error));
          set("dirty", true);
          if (!disposed) {
            timer = setTimeout(() => {
              timer = undefined;
              void sync();
            }, 5000);
          }
        } finally {
          set("syncing", false);
        }
      };
      syncing = run();
      try {
        await syncing;
      } finally {
        syncing = null;
      }
      if (state.dirty && timer === undefined) {
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
      if (state.dirty) {
        await sync();
      }
    };

    const editor: Editor = {
      state,
      set,
      reload,
      loadSnapshot,
      request,
      send: control,
      scene,
      selectScene: (id) => {
        const picked = state.scenes.find((candidate) => sameId(candidate.id, id));
        if (!picked) {
          return;
        }
        if (picked.enabled && !synced.scenes.some((scene) => sameId(scene.id, id))) {
          set("sceneId", id);
          pendingSwitch = id;
        } else if (picked.enabled) {
          set("sceneId", null);
          pendingSwitch = null;
          if (!sameId(currentScene(), id)) {
            void control({ setScene: { id } });
          }
        } else {
          set("sceneId", id);
        }
      },
      editScene: (id, change) => {
        const index = state.scenes.findIndex((scene) => sameId(scene.id, id));
        if (index >= 0) {
          set("scenes", index, produce(change));
          markDirty();
        }
      },
      editWidget: (id, change) => {
        const index = state.widgets.findIndex((widget) => sameId(widget.id, id));
        if (index >= 0) {
          set("widgets", index, produce(change));
          markDirty();
        }
      },
      addScene: (scene) => {
        set("scenes", (scenes) => [...scenes, scene]);
        markDirty();
      },
      removeScene: (id) => {
        const clearScene = (widgets: SettingsWidget[]) => {
          for (const widget of widgets) {
            if (sameId(widget.scene.sceneId, id)) {
              widget.scene.sceneId = null;
            }
          }
        };
        batch(() => {
          set("scenes", (scenes) => scenes.filter((scene) => !sameId(scene.id, id)));
          set("widgets", produce(clearScene));
          clearScene(synced.widgets);
          if (sameId(state.sceneId, id)) {
            set("sceneId", null);
          }
        });
        markDirty();
      },
      addWidget: (widget) => {
        set("widgets", (widgets) => [...widgets, widget]);
        markDirty();
      },
      removeWidget: (id) => {
        batch(() => {
          set("widgets", (widgets) => widgets.filter((widget) => !sameId(widget.id, id)));
          set(
            "scenes",
            produce((scenes) => {
              for (const scene of scenes) {
                scene.widgets = scene.widgets.filter(
                  (sceneWidget) => !sameId(sceneWidget.widgetId, id),
                );
              }
            }),
          );
        });
        markDirty();
      },
      setScoreboardPlayers: (players) => {
        set("scoreboardPlayers", players);
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
    void reload();
    return editor;
  });
}
