import { backendUrl } from "./backend";
import { BackendError, type Page, type Settings } from "./remoteControl";

export type Device = {
  id: string;
  name: string;
  key: string;
  connected: boolean;
};

export type Account = {
  name: string;
  login: string;
  devices: Device[];
  musicDevice: string | null;
  wheelDevice: string | null;
  settings: Settings | null;
  admin: boolean;
  premium: boolean;
};

export type Recent = {
  id: string;
  at: number;
  user: string;
  amount: number;
  currency: string;
  name?: string;
  link?: string;
  message?: string;
  spins?: number;
  status: "delivered" | "waiting" | "notAdded";
  devices?: { name: string; received: boolean }[];
};

export type RecentKind = "song" | "tip" | "spin";

export type RecentList = { items: Recent[]; more: boolean };

export type AdminStats = {
  accounts: { total: number; withPage: number; paypal: number; stripe: number; images: number };
  moblin: { connectedDevices: number };
  pending: { orders: number; deliveries: number };
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

async function call<T>(session: string | null, path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  if (session) {
    headers.set("authorization", `Bearer ${session}`);
  }
  const response = await fetch(`${backendUrl}${path}`, { ...init, headers });
  if (!response.ok) {
    throw new BackendError(response.status, await response.text());
  }
  return response.json();
}

const post = (body: unknown) => ({
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

export const fetchAccount = (session: string) => call<Account>(session, "/account");

export const fetchAdminStats = (session: string) => call<AdminStats>(session, "/admin/stats");

export const fetchRecent = (session: string, kind: RecentKind, limit: number, before?: string) =>
  call<RecentList>(
    session,
    `/account/recent?${new URLSearchParams({ kind, limit: String(limit), ...(before ? { before } : {}) })}`,
  );

export async function signOutEverywhere(session: string) {
  await fetch(`${backendUrl}/account/sign-out`, {
    method: "POST",
    headers: { authorization: `Bearer ${session}` },
  });
}

export const createStreamer = (session: string) =>
  call<Account>(session, "/account/page", { method: "POST" });

export const addDevice = (session: string, name: string) =>
  call<Account>(session, "/account/devices", post({ name }));

export const chooseDevices = (session: string, music: string, wheel: string) =>
  call<Account>(session, "/account/devices/choose", post({ music, wheel }));

export const newDeviceKey = (session: string, device: string) =>
  call<Account>(session, `/account/devices/${encodeURIComponent(device)}/key`, {
    method: "POST",
  });

export const removeDevice = (session: string, device: string) =>
  call<Account>(session, `/account/devices/${encodeURIComponent(device)}/remove`, {
    method: "POST",
  });

export const connectPayPal = async (session: string) =>
  (await call<{ url: string }>(session, "/account/paypal/connect", { method: "POST" })).url;

export const checkPayPal = (session: string) =>
  call<Settings>(session, "/account/paypal/check", { method: "POST" });

export const disconnectPayPal = (session: string) =>
  call<Settings>(session, "/account/paypal/disconnect", { method: "POST" });

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
  (await call<{ url: string }>(session, "/account/stripe/connect", post({ country }))).url;

export const checkStripe = (session: string) =>
  call<Settings>(session, "/account/stripe/check", { method: "POST" });

export const disconnectStripe = (session: string) =>
  call<Settings>(session, "/account/stripe/disconnect", { method: "POST" });

export const uploadImage = (session: string, image: File) =>
  call<Settings>(session, "/account/image", {
    method: "POST",
    headers: { "content-type": image.type || "application/octet-stream" },
    body: image,
  });

export const removeImage = (session: string) =>
  call<Settings>(session, "/account/image/remove", { method: "POST" });

export type SettingsChanges = Page & Pick<Settings, "currency" | "paywall" | "tips" | "spin">;

export const sendTest = (
  session: string,
  kind: "song" | "tip" | "spin",
  body: { song?: string; message?: string },
) => call<object>(session, `/account/test/${kind}`, post(body));

export const saveAccountSettings = (session: string, settings: SettingsChanges) =>
  call<Settings>(
    session,
    "/account/settings",
    post({
      currency: settings.currency,
      paywall: settings.paywall,
      tips: settings.tips,
      spin: settings.spin,
      title: settings.title,
      message: settings.message,
      accent: settings.accent,
      font: settings.font,
    }),
  );
