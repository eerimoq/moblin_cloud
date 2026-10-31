import { createSignal, onMount } from "solid-js";
import { BackendError, backendUrl, call, isSignedOut, post } from "./backend";
import { type Identity, type Page, type Platform, type Settings } from "./remoteControl";
import { readStored, store } from "../storage";

export type Device = {
  id: string;
  name: string;
  connected: boolean;
  reconnecting: boolean;
  wheel: boolean;
  lastSeen: number | null;
};

export type Account = {
  name: string;
  avatar: string | null;
  twitch: Identity | null;
  kick: Identity | null;
  key: string | null;
  devices: Device[];
  settings: Settings;
  admin: boolean;
  premium: boolean;
  preview: boolean;
  goLive: boolean;
  editTips: boolean;
  remoteControl: Access;
  assisting: Assisting[];
  twitchAssistants: Assistant[];
  kickAssistants: Assistant[];
};

export type Access = "none" | "view" | "control";

export type Assistant = {
  id: string;
  login: string;
  name: string;
  avatar: string | null;
  preview: boolean;
  goLive: boolean;
  editTips: boolean;
  remoteControl: Access;
};

export type Assisting = {
  id: string;
  platform: Platform;
  name: string;
  login: string;
  avatar: string | null;
  twitch: Identity | null;
  kick: Identity | null;
};

export type StatusItem = { message: string; ok?: boolean };

export type AudioLevel = { muted: object } | { unknown: object } | { value: { _0: number } };

export type TopLeftItem =
  | "stream"
  | "camera"
  | "mic"
  | "zoom"
  | "obs"
  | "events"
  | "chat"
  | "viewers";

export type TopRightItem =
  | "audioLevel"
  | "rtmpServer"
  | "remoteControl"
  | "gameController"
  | "bitrate"
  | "uptime"
  | "location"
  | "srtla"
  | "srtlaRtts"
  | "recording"
  | "replay"
  | "browserWidgets"
  | "moblink"
  | "djiDevices"
  | "systemMonitor";

type StatusGeneral = {
  batteryCharging?: boolean;
  batteryLevel?: number;
  flame?: "White" | "Yellow" | "Red";
  wiFiSsid?: string;
  isLive?: boolean;
  isRecording?: boolean;
  isMuted?: boolean;
};

type StatusTopRight = {
  audioInfo?: { audioLevel: AudioLevel; numberOfAudioChannels: number };
  bitrateInfo?: { bitsPerSecond: number; totalBytes: number };
  bondingInfo?: { name: string; bytesPerSecond: number; rtt?: number }[];
} & Partial<Record<TopRightItem, StatusItem>>;

export type DeviceStatus = {
  general: StatusGeneral;
  topLeft: Partial<Record<TopLeftItem, StatusItem>>;
  topRight: StatusTopRight;
};

export type Named = { id: string; name: string };

export type DeviceSettings = {
  streams: Named[];
  scenes: Named[];
  autoSceneSwitchers?: Named[];
  bitratePresets: { id: string; bitrate: number }[];
  mics: Named[];
  srt: {
    connectionPrioritiesEnabled: boolean;
    connectionPriorities: { id: string; name: string; priority: number; enabled: boolean }[];
  };
};

export type DeviceState = {
  scene?: string;
  autoSceneSwitcher?: { id?: string };
  mic?: string;
  bitrate?: string;
  zoom?: number;
  zoomPresets?: Named[];
  zoomPreset?: string;
  streaming?: boolean;
  recording?: boolean;
  previewStream?: boolean;
  muted?: boolean;
  stealthMode?: boolean;
  torchOn?: boolean;
  batteryCharging?: boolean;
  gimbalTracking?: boolean;
  filters?: unknown[];
  macros?: { id: string; name: string; running: boolean }[];
};

export type Sample = {
  at: number;
  bitrate: number | null;
  totalBytes: number | null;
  bonding: { name: string; bytesPerSecond: number; rtt: number | null }[];
};

export type DeviceSnapshot = {
  name: string;
  connected: boolean;
  status: DeviceStatus;
  state: DeviceState;
  settings: DeviceSettings | null;
};

export type StatusUpdate = {
  general: StatusGeneral | null;
  topLeft: DeviceStatus["topLeft"] | null;
  topRight: StatusTopRight | null;
  sample: Sample | null;
};

export type Control =
  | { setRecord: { on: boolean } }
  | { setLive: { on: boolean } }
  | { setMute: { on: boolean } }
  | { setStealthMode: { on: boolean } }
  | { setTorch: { on: boolean } }
  | { setZoom: { x: number } }
  | { setZoomPreset: { id: string } }
  | { setScene: { id: string } }
  | { setBitratePreset: { id: string } }
  | { setMic: { id: string } }
  | { setSrtConnectionPriority: { id: string; priority: number; enabled: boolean } }
  | { setSrtConnectionPrioritiesEnabled: { enabled: boolean } }
  | { reloadBrowserWidgets: object }
  | { instantReplay: object }
  | { setAutoSceneSwitcher: { id?: string } }
  | { setPreviewStream: { on: boolean } }
  | { setFilter: { filter: Record<MoblinFilter, object>; on: boolean } }
  | { startMacro: { id: string } }
  | { stopMacro: { id: string } }
  | { setGimbalTracking: { on: boolean } }
  | { sendMessage: { text: string } };

export type MoblinFilter =
  | "pixellate"
  | "movie"
  | "grayScale"
  | "sepia"
  | "triple"
  | "twin"
  | "fourThree"
  | "crt"
  | "pinch"
  | "whirlpool"
  | "poll"
  | "blurFaces"
  | "privacy"
  | "beauty"
  | "moblinInMouth"
  | "cameraMan";

export type Recent = {
  id: string;
  at: number;
  user: string;
  amount: number;
  currency: string;
  message?: string;
  spins?: number;
  effect?: string;
  tier?: number;
  kickSubscriber?: boolean;
  provider?: "paypal" | "stripe";
  status: "delivered" | "waiting";
  devices?: { name: string; received: boolean }[];
};

export type RecentKind = "tip" | "spin" | "effect";

export type RecentList = { items: Recent[]; more: boolean };

export type AdminStats = {
  server: {
    version: string;
    started: number;
    now: number;
    memory: number | null;
    integrations: {
      twitch: boolean;
      kick: boolean;
      appAttest: boolean;
      paypal: boolean;
      stripe: boolean;
    };
    requests: {
      total: number;
      statuses: Record<string, number>;
      endpoints: { path: string; total: number; statuses: Record<string, number> }[];
    };
  };
  accounts: { total: number; paypal: number; stripe: number; premium: number };
  moblin: {
    connectedDevices: number;
    messages: Record<"sent" | "received", { kind: string; messages: number; bytes: number }[]>;
  };
  pending: { orders: number; deliveries: number };
  database: { bytes: number; files: { name: string; bytes: number }[] };
  counters: { name: string; today: number; week: number; total: number }[];
};

const sessionKey = "session";

export const readSession = () => readStored(sessionKey);

const saveSession = (session: string | null) => store(sessionKey, session || null);

function takeSignInResult(): {
  session: string | null;
  failed: boolean;
  link: string | null;
  linkFailed: boolean;
} {
  const hash = location.hash.slice(1);
  const params = new URLSearchParams(hash);
  const session = params.get("session");
  const link = params.get("link");
  const failed = hash === "signInFailed";
  const linkFailed = hash === "linkFailed";
  if (session || failed || link || linkFailed) {
    history.replaceState(
      null,
      "",
      `${location.pathname}${location.search}${link || linkFailed ? "#linked" : ""}`,
    );
  }
  return { session, failed, link, linkFailed };
}

export const signInUrl = (platform: Platform) => `${backendUrl}/auth/${platform}/login`;

export const linkUrl = (platform: Platform) => `${backendUrl}/auth/${platform}/login?link=1`;

export type LinkResult = { linked: true } | { linked: false; error: string | null };

export function createAccountSession() {
  const [session, setSession] = createSignal<{ session: string; account: Account } | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [signInFailed, setSignInFailed] = createSignal(false);
  const [linkResult, setLinkResult] = createSignal<LinkResult | null>(null);

  onMount(async () => {
    const result = takeSignInResult();
    if (result.session) {
      saveSession(result.session);
    }
    setSignInFailed(result.failed);
    if (result.linkFailed) {
      setLinkResult({ linked: false, error: null });
    }
    const stored = readSession();
    if (stored) {
      try {
        let account = await fetchAccount(stored);
        if (result.link) {
          try {
            account = await linkAccount(stored, result.link);
            setLinkResult({ linked: true });
          } catch (error) {
            setLinkResult({
              linked: false,
              error: error instanceof BackendError && error.message ? error.message : null,
            });
          }
        }
        setSession({ session: stored, account });
      } catch (error) {
        if (isSignedOut(error)) {
          saveSession(null);
        }
      }
    }
    setLoading(false);
  });

  const signOut = () => {
    saveSession(null);
    setSession(null);
  };

  const signOutAll = async () => {
    const current = session();
    if (current) {
      try {
        await signOutEverywhere(current.session);
      } catch {}
    }
    signOut();
  };

  return { session, loading, signInFailed, linkResult, signOut, signOutAll };
}

const accountPath = (page?: string) =>
  page === undefined ? "/account" : `/account/pages/${encodeURIComponent(page)}`;

export const fetchAccount = (session: string, page?: string) =>
  call<Account>(accountPath(page), session);

export const adminStatsEventsUrl = `${backendUrl}/admin/stats/events`;

export const pageEventsUrl = (page: string) => `${backendUrl}${accountPath(page)}/events`;

export const fetchRecent = (
  session: string,
  page: string | undefined,
  kind: RecentKind,
  limit: number,
  before?: string,
) =>
  call<RecentList>(
    `${accountPath(page)}/recent?${new URLSearchParams({ kind, limit: String(limit), ...(before ? { before } : {}) })}`,
    session,
  );

export const resendRecent = (session: string, id: string, page?: string) =>
  call<object>(`${accountPath(page)}/recent/${encodeURIComponent(id)}/resend`, session, post());

export const fetchMonitorLayout = (session: string, page: string) =>
  call<{ layout: unknown }>(`${accountPath(page)}/monitor`, session);

export const saveMonitorLayout = (session: string, page: string, layout: object) =>
  call<void>(`${accountPath(page)}/monitor`, session, { ...post({ layout }), keepalive: true });

export const addAssistant = (session: string, platform: Platform, username: string) =>
  call<Account>(`/account/assistants/${platform}`, session, post({ username }));

const assistantCall = (
  session: string,
  platform: Platform,
  assistant: string,
  action: string,
  body?: object,
) =>
  call<Account>(
    `/account/assistants/${platform}/${encodeURIComponent(assistant)}/${action}`,
    session,
    post(body),
  );

export const removeAssistant = (session: string, platform: Platform, assistant: string) =>
  assistantCall(session, platform, assistant, "remove");

export const setAssistantPreview = (
  session: string,
  platform: Platform,
  assistant: string,
  on: boolean,
) => assistantCall(session, platform, assistant, "preview", { on });

export const setAssistantGoLive = (
  session: string,
  platform: Platform,
  assistant: string,
  on: boolean,
) => assistantCall(session, platform, assistant, "go-live", { on });

export const setAssistantEditTips = (
  session: string,
  platform: Platform,
  assistant: string,
  on: boolean,
) => assistantCall(session, platform, assistant, "edit-tips", { on });

export const setAssistantRemoteControl = (
  session: string,
  platform: Platform,
  assistant: string,
  access: Access,
) => assistantCall(session, platform, assistant, "remote-control", { access });

const linkAccount = (session: string, token: string) =>
  call<Account>("/account/link", session, post({ token }));

export const unlinkAccount = (session: string, platform: Platform) =>
  call<Account>(`/account/unlink/${platform}`, session, post());

const signOutEverywhere = (session: string) => call<void>("/account/sign-out", session, post());

export const deleteAccount = (session: string) => call<void>("/account/delete", session, post());

export const connectPendingKey = "connectPending";

export const decideConnect = (session: string, code: string, approve: boolean) =>
  call<void>("/account/connect", session, post({ code, approve }));

export const newMoblinKey = (session: string) => call<Account>("/account/key", session, post());

export const renameDevice = (session: string, device: string, name: string) =>
  call<Account>(`/account/devices/${encodeURIComponent(device)}/name`, session, post({ name }));

export const removeDevice = (session: string, device: string) =>
  call<Account>(`/account/devices/${encodeURIComponent(device)}/remove`, session, post());

const devicePath = (page: string, device: string) =>
  `${accountPath(page)}/devices/${encodeURIComponent(device)}`;

export const deviceEventsUrl = (page: string, device: string) =>
  `${backendUrl}${devicePath(page, device)}/events`;

export const devicePreviewUrl = (page: string, device: string) =>
  `${backendUrl}${devicePath(page, device)}/preview`;

export const controlDevice = (session: string, page: string, device: string, control: Control) =>
  call<object>(`${devicePath(page, device)}/control`, session, post(control));

export const connectPayPal = async (session: string) =>
  (await call<{ url: string }>("/account/paypal/connect", session, post())).url;

export const checkPayPal = (session: string) =>
  call<Settings>("/account/paypal/check", session, post());

export const disconnectPayPal = (session: string) =>
  call<Settings>("/account/paypal/disconnect", session, post());

export const stripeCountries = [
  "AE",
  "AT",
  "AU",
  "BE",
  "BG",
  "BR",
  "CA",
  "CH",
  "CY",
  "CZ",
  "DE",
  "DK",
  "EE",
  "ES",
  "FI",
  "FR",
  "GB",
  "GI",
  "GR",
  "HK",
  "HR",
  "HU",
  "IE",
  "IT",
  "JP",
  "LI",
  "LT",
  "LU",
  "LV",
  "MT",
  "MX",
  "MY",
  "NL",
  "NO",
  "NZ",
  "PL",
  "PT",
  "RO",
  "SE",
  "SG",
  "SI",
  "SK",
  "TH",
  "US",
];

export const connectStripe = async (session: string, country: string) =>
  (await call<{ url: string }>("/account/stripe/connect", session, post({ country }))).url;

export const checkStripe = (session: string) =>
  call<Settings>("/account/stripe/check", session, post());

export const disconnectStripe = (session: string) =>
  call<Settings>("/account/stripe/disconnect", session, post());

export const uploadImage = (session: string, image: File, page?: string) =>
  call<Settings>(`${accountPath(page)}/image`, session, {
    method: "POST",
    headers: { "content-type": image.type || "application/octet-stream" },
    body: image,
  });

export const removeImage = (session: string, page?: string) =>
  call<Settings>(`${accountPath(page)}/image/remove`, session, post());

export type SettingsChanges = Page &
  Pick<Settings, "currency" | "tips" | "spin" | "effects" | "subSpins" | "subEffects">;

export const sendTest = (
  session: string,
  kind: RecentKind,
  body: { message?: string; effect?: string },
  page?: string,
) => call<object>(`${accountPath(page)}/test/${kind}`, session, post(body));

export const saveAccountSettings = (session: string, settings: SettingsChanges, page?: string) =>
  call<Settings>(
    `${accountPath(page)}/settings`,
    session,
    post({
      currency: settings.currency,
      tips: settings.tips,
      spin: settings.spin,
      effects: settings.effects,
      subSpins: settings.subSpins,
      subEffects: settings.subEffects,
      message: settings.message,
      accent: settings.accent,
      font: settings.font,
    }),
  );
