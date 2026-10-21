import { createSignal, onMount } from "solid-js";
import { BackendError, backendUrl, call, post } from "./backend";
import { type MusicCommand, type MusicStatus, type Page, type Settings } from "./remoteControl";

export type Device = {
  id: string;
  name: string;
  key: string;
  enabled: boolean;
  connected: boolean;
};

export type Platform = "twitch" | "kick";

export type Account = {
  name: string;
  login: string;
  platform?: Platform;
  avatar: string | null;
  devices: Device[];
  musicDevice: string | null;
  wheelDevice: string | null;
  settings: Settings;
  admin: boolean;
  premium: boolean;
  moderating: Moderating[];
  moderators: ChannelModerator[];
};

export type ChannelModerator = {
  id: string;
  login: string;
  name: string;
  enabled: boolean;
};

export type Moderating = {
  id: string;
  name: string;
  login: string;
  avatar: string | null;
};

export type Recent = {
  id: string;
  at: number;
  user: string;
  amount: number;
  currency: string;
  name?: string;
  message?: string;
  spins?: number;
  tier?: number;
  moderator?: boolean;
  provider?: "paypal" | "stripe";
  status: "delivered" | "waiting" | "notAdded";
  devices?: { name: string; received: boolean }[];
};

export type RecentKind = "song" | "tip" | "spin";

export type RecentList = { items: Recent[]; more: boolean };

export type AdminStats = {
  server: {
    version: string;
    started: number;
    now: number;
    memory: number | null;
    integrations: { twitch: boolean; appAttest: boolean; paypal: boolean; stripe: boolean };
  };
  accounts: { total: number; paypal: number; stripe: number; premium: number };
  moblin: { connectedDevices: number };
  pending: { orders: number; deliveries: number };
  database: { bytes: number };
  counters: { name: string; today: number; week: number; total: number }[];
};

const sessionKey = "songRequestSession";

export function readSession() {
  try {
    return localStorage.getItem(sessionKey);
  } catch {
    return null;
  }
}

export function saveSession(session: string | null) {
  try {
    if (session) {
      localStorage.setItem(sessionKey, session);
    } else {
      localStorage.removeItem(sessionKey);
    }
  } catch {}
}

export function takeSignInResult(): { session: string | null; failed: boolean } {
  const hash = location.hash.slice(1);
  const session = new URLSearchParams(hash).get("session");
  const failed = hash === "signInFailed";
  if (session || failed) {
    history.replaceState(null, "", location.pathname + location.search);
  }
  return { session, failed };
}

export const signInUrl = `${backendUrl}/auth/twitch/login`;

export const kickSignInUrl = `${backendUrl}/auth/kick/login`;

export function createAccountSession() {
  const [session, setSession] = createSignal<{ session: string; account: Account } | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [signInFailed, setSignInFailed] = createSignal(false);

  onMount(async () => {
    const result = takeSignInResult();
    if (result.session) {
      saveSession(result.session);
    }
    setSignInFailed(result.failed);
    const stored = readSession();
    if (stored) {
      try {
        setSession({ session: stored, account: await fetchAccount(stored) });
      } catch (error) {
        if (error instanceof BackendError && error.status === 401) {
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

  return { session, loading, signInFailed, signOut, signOutAll };
}

const accountPath = (page?: string) =>
  page === undefined ? "/account" : `/account/pages/${encodeURIComponent(page)}`;

export const fetchAccount = (session: string, page?: string) =>
  call<Account>(accountPath(page), session);

export const fetchAdminStats = (session: string) => call<AdminStats>("/admin/stats", session);

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

export const enableModerator = (session: string, moderator: string, enabled: boolean) =>
  call<Account>(`/account/moderators/${encodeURIComponent(moderator)}`, session, post({ enabled }));

export const moderateMusic = (
  session: string,
  page: string,
  command: MusicCommand,
  song?: string,
) => call<MusicStatus>(`${accountPath(page)}/music`, session, post({ command, id: song }));

export const addModeratorSong = (session: string, page: string, song: string, message: string) =>
  call<object>(`${accountPath(page)}/songs`, session, post({ song, message: message || null }));

export const signOutEverywhere = (session: string) =>
  call<void>("/account/sign-out", session, post());

export const deleteAccount = (session: string) => call<void>("/account/delete", session, post());

export const addDevice = (session: string, name: string) =>
  call<Account>("/account/devices", session, post({ name }));

export const chooseDevices = (session: string, music: string, wheel: string) =>
  call<Account>("/account/devices/choose", session, post({ music, wheel }));

export const newDeviceKey = (session: string, device: string) =>
  call<Account>(`/account/devices/${encodeURIComponent(device)}/key`, session, post());

export const removeDevice = (session: string, device: string) =>
  call<Account>(`/account/devices/${encodeURIComponent(device)}/remove`, session, post());

export const enableDevice = (session: string, device: string, enabled: boolean) =>
  call<Account>(
    `/account/devices/${encodeURIComponent(device)}/enabled`,
    session,
    post({ enabled }),
  );

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
  Pick<
    Settings,
    "currency" | "songs" | "tips" | "spin" | "subSongs" | "subSpins" | "maxSongLength"
  >;

export const sendTest = (
  session: string,
  kind: RecentKind,
  body: { song?: string; message?: string },
  page?: string,
) => call<object>(`${accountPath(page)}/test/${kind}`, session, post(body));

export const saveAccountSettings = (session: string, settings: SettingsChanges, page?: string) =>
  call<Settings>(
    `${accountPath(page)}/settings`,
    session,
    post({
      currency: settings.currency,
      songs: settings.songs,
      tips: settings.tips,
      spin: settings.spin,
      subSongs: settings.subSongs,
      subSpins: settings.subSpins,
      maxSongLength: settings.maxSongLength,
      message: settings.message,
      accent: settings.accent,
      font: settings.font,
    }),
  );
