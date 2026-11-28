import {
  createRoot,
  createStore,
  onCleanup,
  reconcile,
  type Accessor,
  runWithOwner,
  type StoreSetter,
} from "solid-js";
import { removeWhere } from "../../store";
import { clone, errorText, same } from "../sceneEditor/store";
import { BackendError } from "../../data/backend";
import { sameId, type EditorOptions } from "../../data/moblinSettings";
import {
  encodeStream,
  normalizeStream,
  type SettingsStream,
} from "../../data/moblinStreamSettings";
import type { Control, DeviceSettings } from "../../data/account";

export type StreamItem = { id: string; name: string; enabled: boolean };

export type StreamsState = {
  options: EditorOptions | null;
  drafts: SettingsStream[];
  loading: string[];
  added: string[];
  removed: string[];
  selected: string | null;
  error: string | null;
  syncing: boolean;
  dirty: boolean;
};

export type Streams = {
  state: StreamsState;
  set: StoreSetter<StreamsState>;
  list: () => StreamItem[];
  names: () => string[];
  load: (id: string) => Promise<SettingsStream | undefined>;
  loading: (id: string) => boolean;
  stream: (id: string) => SettingsStream | undefined;
  edit: (id: string, change: (stream: SettingsStream) => void) => void;
  add: (stream: SettingsStream) => void;
  remove: (id: string) => void;
  select: (id: string) => void;
  flush: () => Promise<void>;
  dispose: () => void;
};

const refused = (error: unknown) => error instanceof BackendError && error.status === 409;

const has = (ids: string[], id: string) => ids.some((candidate) => sameId(candidate, id));

export function createStreams(
  request: <T>(control: Control) => Promise<T>,
  control: (control: Control) => Promise<void>,
  settings: Accessor<DeviceSettings | null>,
  readOnly: Accessor<boolean> = () => false,
): Streams {
  return runWithOwner(null, () =>
    createRoot((dispose) => {
      const [state, setState] = createStore<StreamsState>({
        options: null,
        drafts: [],
        loading: [],
        added: [],
        removed: [],
        selected: null,
        error: null,
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
      let synced: SettingsStream[] = [];
      let deleted: string[] = [];
      let timer: ReturnType<typeof setTimeout> | undefined;
      let syncing: Promise<void> | null = null;
      let lastSync = 0;
      let pendingSelect: string | null = null;
      let refusal: string | null = null;
      let disposed = false;

      const draft = (id: string) => state.drafts.find((stream) => sameId(stream.id, id));

      const syncedStream = (id: string) => synced.find((stream) => sameId(stream.id, id));

      const list = (): StreamItem[] => {
        const known = settings()?.streams ?? [];
        const current = state.selected ?? settings()?.currentStreamId;
        const items = known
          .filter((stream) => !has(state.removed, stream.id))
          .map((stream) => ({ id: stream.id, name: draft(stream.id)?.name ?? stream.name }));
        for (const id of state.added) {
          const stream = draft(id);
          if (stream && !has(state.removed, id) && !items.some((item) => sameId(item.id, id))) {
            items.push({ id: stream.id, name: stream.name });
          }
        }
        return items.map((item) => ({ ...item, enabled: sameId(item.id, current) }));
      };

      const setDraft = (stream: SettingsStream) => {
        setState((draft) => {
          const index = draft.drafts.findIndex((candidate) => sameId(candidate.id, stream.id));
          if (index < 0) {
            draft.drafts.push(stream);
          } else {
            reconcile(stream)(draft.drafts[index]);
          }
        });
      };

      const load = async (id: string) => {
        const loaded = draft(id);
        const old = syncedStream(id);
        if (loaded && (!old || !same(loaded, old))) {
          return loaded;
        }
        const before = loaded ? clone(loaded) : undefined;
        setState((draft) => {
          draft.loading.push(id);
        });
        try {
          const [answer, options] = await Promise.all([
            request<{ stream: Record<string, unknown> }>({ getStream: { id } }),
            state.options
              ? Promise.resolve({ options: state.options })
              : request<{ options: EditorOptions }>({ getEditorOptions: {} }),
          ]);
          if (disposed) {
            return undefined;
          }
          const fresh = normalizeStream(answer.stream);
          const now = draft(id);
          const editedMeanwhile = now !== undefined && !same(now, before);
          synced = [...synced.filter((stream) => !sameId(stream.id, id)), clone(fresh)];
          if (editedMeanwhile) {
            markDirty();
          } else {
            setDraft(fresh);
          }
          setState((draft) => {
            draft.options = options.options;
          });
          setState((draft) => {
            draft.error = null;
          });
          await Promise.resolve();
          return draft(id);
        } catch (error) {
          setState((draft) => {
            draft.error = errorText(error);
          });
          return undefined;
        } finally {
          setState((draft) => removeWhere(draft.loading, (candidate) => sameId(candidate, id)));
        }
      };

      const forget = (id: string) => {
        synced = synced.filter((stream) => !sameId(stream.id, id));
        const deselect = pendingSelect !== null && sameId(pendingSelect, id);
        if (deselect) {
          pendingSelect = null;
        }
        setState((draft) => {
          removeWhere(draft.drafts, (stream) => sameId(stream.id, id));
          removeWhere(draft.added, (candidate) => sameId(candidate, id));
          if (deselect) {
            draft.selected = null;
          }
        });
      };

      const refetch = async (id: string) => {
        try {
          const answer = await request<{ stream: Record<string, unknown> }>({ getStream: { id } });
          const fresh = normalizeStream(answer.stream);
          synced = [...synced.filter((stream) => !sameId(stream.id, id)), clone(fresh)];
          setDraft(fresh);
        } catch (error) {
          if (!refused(error)) {
            throw error;
          }
          forget(id);
        }
      };

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
            for (const id of state.removed.filter((id) => !has(deleted, id))) {
              try {
                if (!has(state.added, id) || syncedStream(id)) {
                  await request({ deleteStream: { id } });
                }
                deleted.push(id);
                synced = synced.filter((stream) => !sameId(stream.id, id));
              } catch (error) {
                if (!refused(error)) {
                  throw error;
                }
                refusal = errorText(error);
                setState((draft) =>
                  removeWhere(draft.removed, (candidate) => sameId(candidate, id)),
                );
              }
            }
            for (const stream of clone(state.drafts)) {
              const old = syncedStream(stream.id);
              try {
                if (!old && has(state.added, stream.id)) {
                  await request({ createStream: { stream: encodeStream(stream) } });
                  synced.push(stream);
                } else if (old && !same(old, stream)) {
                  await request({ updateStream: { stream: encodeStream(stream) } });
                  synced = synced.map((candidate) =>
                    sameId(candidate.id, stream.id) ? stream : candidate,
                  );
                }
              } catch (error) {
                if (!refused(error)) {
                  throw error;
                }
                refusal = errorText(error);
                if (old) {
                  await refetch(stream.id);
                } else {
                  forget(stream.id);
                }
              }
            }
            if (pendingSelect && syncedStream(pendingSelect)) {
              const id = pendingSelect;
              pendingSelect = null;
              void control({ setStream: { id } }).finally(() =>
                setState((draft) => {
                  draft.selected = null;
                }),
              );
            }
            setState((draft) => {
              draft.error = refusal;
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

      const streams: Streams = {
        state,
        set: setState,
        list,
        names: () => list().map((item) => item.name),
        load,
        loading: (id) => has(state.loading, id),
        stream: draft,
        edit: (id, change) => {
          if (readOnly()) {
            return;
          }
          refusal = null;
          let found = false;
          setState((draft) => {
            const stream = draft.drafts.find((stream) => sameId(stream.id, id));
            if (stream) {
              change(stream);
              found = true;
            }
          });
          if (found) {
            markDirty();
          }
        },
        add: (stream) => {
          if (readOnly()) {
            return;
          }
          refusal = null;
          setState((draft) => {
            draft.drafts.push(stream);
            draft.added.push(stream.id);
          });
          markDirty();
        },
        remove: (id) => {
          if (readOnly()) {
            return;
          }
          refusal = null;
          setState((draft) => {
            draft.removed.push(id);
            removeWhere(draft.drafts, (stream) => sameId(stream.id, id));
          });
          markDirty();
        },
        select: (id) => {
          if (readOnly()) {
            return;
          }
          refusal = null;
          let unsynced = false;
          setState((draft) => {
            draft.selected = id;
            unsynced = has(draft.added, id) && !syncedStream(id);
          });
          if (unsynced) {
            pendingSelect = id;
            markDirty();
          } else {
            void control({ setStream: { id } }).finally(() =>
              setState((draft) => {
                draft.selected = null;
              }),
            );
          }
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
      return streams;
    }),
  );
}
