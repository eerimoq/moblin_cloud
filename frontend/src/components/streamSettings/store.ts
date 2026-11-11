import { batch, createRoot, onCleanup, type Accessor } from "solid-js";
import { createStore, produce, reconcile, type SetStoreFunction } from "solid-js/store";
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
  set: SetStoreFunction<StreamsState>;
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
  return createRoot((dispose) => {
    const [state, set] = createStore<StreamsState>({
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
      if (index < 0) {
        set("drafts", (drafts) => [...drafts, stream]);
      } else {
        set("drafts", index, reconcile(stream));
      }
    };

    const load = async (id: string) => {
      const loaded = draft(id);
      const old = syncedStream(id);
      if (loaded && (!old || !same(loaded, old))) {
        return loaded;
      }
      const before = loaded ? clone(loaded) : undefined;
      set("loading", (ids) => [...ids, id]);
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
        batch(() => {
          synced = [...synced.filter((stream) => !sameId(stream.id, id)), clone(fresh)];
          if (editedMeanwhile) {
            markDirty();
          } else {
            setDraft(fresh);
          }
          set("options", options.options);
          set("error", null);
        });
        return draft(id);
      } catch (error) {
        set("error", errorText(error));
        return undefined;
      } finally {
        set("loading", (ids) => ids.filter((candidate) => !sameId(candidate, id)));
      }
    };

    const forget = (id: string) => {
      batch(() => {
        synced = synced.filter((stream) => !sameId(stream.id, id));
        set("drafts", (drafts) => drafts.filter((stream) => !sameId(stream.id, id)));
        set("added", (ids) => ids.filter((candidate) => !sameId(candidate, id)));
        if (pendingSelect !== null && sameId(pendingSelect, id)) {
          pendingSelect = null;
          set("selected", null);
        }
      });
    };

    const refetch = async (id: string) => {
      try {
        const answer = await request<{ stream: Record<string, unknown> }>({ getStream: { id } });
        const fresh = normalizeStream(answer.stream);
        batch(() => {
          synced = [...synced.filter((stream) => !sameId(stream.id, id)), clone(fresh)];
          setDraft(fresh);
        });
      } catch (error) {
        if (!refused(error)) {
          throw error;
        }
        forget(id);
      }
    };

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

    const sync = async () => {
      if (syncing) {
        return;
      }
      const run = async () => {
        lastSync = Date.now();
        set("syncing", true);
        set("dirty", false);
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
              set("removed", (ids) => ids.filter((candidate) => !sameId(candidate, id)));
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
            void control({ setStream: { id } }).finally(() => set("selected", null));
          }
          set("error", refusal);
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
          set("drafts", index, produce(change));
          markDirty();
        }
      },
      add: (stream) => {
        if (readOnly()) {
          return;
        }
        refusal = null;
        batch(() => {
          set("drafts", (drafts) => [...drafts, stream]);
          set("added", (ids) => [...ids, stream.id]);
        });
        markDirty();
      },
      remove: (id) => {
        if (readOnly()) {
          return;
        }
        refusal = null;
        batch(() => {
          set("removed", (ids) => [...ids, id]);
          set("drafts", (drafts) => drafts.filter((stream) => !sameId(stream.id, id)));
        });
        markDirty();
      },
      select: (id) => {
        if (readOnly()) {
          return;
        }
        refusal = null;
        set("selected", id);
        if (has(state.added, id) && !syncedStream(id)) {
          pendingSelect = id;
          markDirty();
        } else {
          void control({ setStream: { id } }).finally(() => set("selected", null));
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
  });
}
