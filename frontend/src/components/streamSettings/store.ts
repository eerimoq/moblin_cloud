import {
  createRoot,
  createStore,
  flush as flushUpdates,
  onCleanup,
  reconcile,
  type Accessor,
  runWithOwner,
} from "solid-js";
import { removeWhere, storeSetter, type SetStore } from "../../store";
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
  set: SetStore<StreamsState>;
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
      const set = storeSetter(setState);
      const write = storeSetter(setState, flushUpdates);
      const update = (change: (draft: StreamsState) => void) => {
        setState(change);
        flushUpdates();
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
        const index = state.drafts.findIndex((candidate) => sameId(candidate.id, stream.id));
        update((draft) => {
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
        update((draft) => {
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
          write("options", options.options);
          write("error", null);
          return draft(id);
        } catch (error) {
          write("error", errorText(error));
          return undefined;
        } finally {
          update((draft) => removeWhere(draft.loading, (candidate) => sameId(candidate, id)));
        }
      };

      const forget = (id: string) => {
        synced = synced.filter((stream) => !sameId(stream.id, id));
        const deselect = pendingSelect !== null && sameId(pendingSelect, id);
        if (deselect) {
          pendingSelect = null;
        }
        update((draft) => {
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
        write("dirty", true);
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
          write("syncing", true);
          write("dirty", false);
          try {
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
                update((draft) => removeWhere(draft.removed, (candidate) => sameId(candidate, id)));
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
              void control({ setStream: { id } }).finally(() => write("selected", null));
            }
            write("error", refusal);
          } catch (error) {
            write("error", errorText(error));
            write("dirty", true);
            if (!disposed) {
              timer = setTimeout(() => {
                timer = undefined;
                void sync();
              }, 5000);
            }
          } finally {
            write("syncing", false);
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

      const streams: Streams = {
        state,
        set,
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
          const index = state.drafts.findIndex((stream) => sameId(stream.id, id));
          if (index >= 0) {
            update((draft) => change(draft.drafts[index]));
            markDirty();
          }
        },
        add: (stream) => {
          if (readOnly()) {
            return;
          }
          refusal = null;
          update((draft) => {
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
          update((draft) => {
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
          write("selected", id);
          if (has(state.added, id) && !syncedStream(id)) {
            pendingSelect = id;
            markDirty();
          } else {
            void control({ setStream: { id } }).finally(() => write("selected", null));
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
