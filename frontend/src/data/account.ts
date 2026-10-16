import { backendUrl } from "./backend";
import { BackendError, type Page, type Settings } from "./remoteControl";

export type Account = {
  name: string;
  login: string;
  streamerUrl: string | null;
  settings: Settings | null;
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

export async function signOutEverywhere(session: string) {
  await fetch(`${backendUrl}/account/sign-out`, {
    method: "POST",
    headers: { authorization: `Bearer ${session}` },
  });
}

export const createStreamer = (session: string) =>
  call<Account>(session, "/account/streamer", { method: "POST" });

export const newStreamerUrl = (session: string) =>
  call<Account>(session, "/account/streamer-url", { method: "POST" });

export const connectPayPal = async (session: string) =>
  (await call<{ url: string }>(session, "/account/paypal", { method: "POST" })).url;

export const checkPayPal = (session: string) =>
  call<Settings>(session, "/account/paypal/check", { method: "POST" });

export const disconnectPayPal = (session: string) =>
  call<Settings>(session, "/account/paypal/disconnect", { method: "POST" });

export type SettingsChanges = Page &
  Pick<Settings, "songs" | "currency" | "paywall" | "tips" | "spin">;

export const saveAccountSettings = (session: string, settings: SettingsChanges) =>
  call<Settings>(
    session,
    "/account/settings",
    post({
      songs: settings.songs,
      currency: settings.currency,
      paywall: settings.paywall,
      tips: settings.tips,
      spin: settings.spin,
      title: settings.title,
      message: settings.message,
      accent: settings.accent,
    }),
  );
