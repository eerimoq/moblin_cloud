import { type Accessor, createRoot, createSignal, onCleanup } from "solid-js";
import {
  controlDevice,
  deviceEventsUrl,
  devicePreviewUrl,
  type Control,
  type DeviceSettings,
  type DeviceSnapshot,
  type DeviceState,
  type DeviceStatus,
  type Sample,
  type StatusUpdate,
} from "../data/account";
import { isSignedOut } from "../data/backend";
import { subscribe } from "../sse";

export type DeviceFeed = {
  loaded: Accessor<boolean>;
  name: Accessor<string>;
  streaming: Accessor<boolean>;
  connected: Accessor<boolean>;
  status: Accessor<DeviceStatus>;
  state: Accessor<DeviceState>;
  settings: Accessor<DeviceSettings | null>;
  history: Accessor<Sample[]>;
  pending: Accessor<string[]>;
  scenesVersion: Accessor<number>;
  control: (control: Control) => Promise<void>;
};

const emptyStatus = (): DeviceStatus => ({ general: {}, topLeft: {}, topRight: {} });

const filled = <T extends object>(part: T) => (Object.keys(part).length > 0 ? part : undefined);

const perItem = ["setSrtConnectionPriority", "startMacro", "stopMacro"];

type Shared<T> = { value: T; users: number; dispose: () => void };

const shared = <T>(cache: Map<string, Shared<T>>, key: string, create: () => T): T => {
  let entry = cache.get(key);
  if (!entry) {
    entry = createRoot((dispose) => ({ value: create(), users: 0, dispose }));
    cache.set(key, entry);
  }
  const used = entry;
  used.users += 1;
  onCleanup(() => {
    used.users -= 1;
    if (used.users === 0) {
      cache.delete(key);
      used.dispose();
    }
  });
  return used.value;
};

const feeds = new Map<string, Shared<DeviceFeed>>();

const previews = new Map<string, Shared<Accessor<string | null>>>();

function openDeviceFeed(
  session: string,
  page: string,
  device: string,
  onSignOut: () => void,
): DeviceFeed {
  const [loaded, setLoaded] = createSignal(false);
  const [name, setName] = createSignal("");
  const [streaming, setStreaming] = createSignal(false);
  const [connected, setConnected] = createSignal(false);
  const [status, setStatus] = createSignal<DeviceStatus>(emptyStatus());
  const [state, setState] = createSignal<DeviceState>({});
  const [settings, setSettings] = createSignal<DeviceSettings | null>(null);
  const [history, setHistory] = createSignal<Sample[]>([]);
  const [pending, setPending] = createSignal<string[]>([]);
  const [scenesVersion, setScenesVersion] = createSignal(0);

  const stop = subscribe(deviceEventsUrl(page, device), session, {
    onOpen: () => setStreaming(true),
    onEvent: (kind, data) => {
      switch (kind) {
        case "snapshot": {
          const snapshot = data as DeviceSnapshot;
          setLoaded(true);
          setName(snapshot.name);
          setConnected(snapshot.connected);
          if (snapshot.connected) {
            setStatus((current) => ({
              general: filled(snapshot.status.general) ?? current.general,
              topLeft: filled(snapshot.status.topLeft) ?? current.topLeft,
              topRight: filled(snapshot.status.topRight) ?? current.topRight,
            }));
            setState((current) => ({ ...current, ...snapshot.state }));
            setSettings((current) => snapshot.settings ?? current);
          } else {
            setStatus(emptyStatus());
            setState({});
            setSettings(snapshot.settings);
          }
          break;
        }
        case "status": {
          const update = data as StatusUpdate;
          setStatus((current) => ({
            general: update.general ?? current.general,
            topLeft: update.topLeft ?? current.topLeft,
            topRight: update.topRight ?? current.topRight,
          }));
          if (update.sample) {
            const sample = update.sample;
            setHistory((current) => [...current.slice(-59), sample]);
          }
          break;
        }
        case "state":
          setState((current) => ({ ...current, ...(data as { data: DeviceState }).data }));
          break;
        case "settings":
          setSettings((data as { data: DeviceSettings }).data);
          break;
        case "scenes":
          setScenesVersion((version) => version + 1);
          break;
        case "connection": {
          const connected = (data as { connected: boolean }).connected;
          setConnected(connected);
          if (!connected) {
            setStatus(emptyStatus());
            setState({});
          }
          break;
        }
        default:
          break;
      }
    },
    onError: (code) => {
      setStreaming(false);
      if (code === 401) {
        onSignOut();
      }
    },
  });
  onCleanup(stop);

  const control = async (control: Control) => {
    const kind = Object.keys(control)[0] ?? "";
    const body = Object.values(control)[0] as { id?: string };
    const key = perItem.includes(kind) ? `${kind}:${body.id}` : kind;
    setPending((current) => [...current, key]);
    try {
      await controlDevice(session, page, device, control);
    } catch (error) {
      if (isSignedOut(error)) {
        onSignOut();
      }
    } finally {
      setPending((current) => current.filter((pending) => pending !== key));
    }
  };

  return {
    loaded,
    name,
    streaming,
    connected,
    status,
    state,
    settings,
    history,
    pending,
    scenesVersion,
    control,
  };
}

export const useDeviceFeed = (
  session: string,
  page: string,
  device: string,
  onSignOut: () => void,
) => shared(feeds, `${page}/${device}`, () => openDeviceFeed(session, page, device, onSignOut));

export const usePreviewFeed = (session: string, page: string, device: string) =>
  shared(previews, `${page}/${device}`, () => {
    const [preview, setPreview] = createSignal<string | null>(null);
    onCleanup(
      subscribe(devicePreviewUrl(page, device), session, {
        onEvent: (kind, data) => {
          if (kind === "frame") {
            setPreview((data as { image: string }).image);
          }
        },
      }),
    );
    return preview;
  });
