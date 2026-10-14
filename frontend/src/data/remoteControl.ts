import { backendUrl } from "./backend";
import { links } from "./links";

export type Song = {
  id: string;
  title: string;
  artist: string | null;
  user: string | null;
};

export type MusicStatus = {
  playing: boolean;
  currentSongIndex: number | null;
  songs: Song[];
  songNotFound: boolean | null;
};

export type Streamer = {
  key: string;
  id: string;
};

export type Paywall = {
  email: string;
  price: number;
  currency: string;
  merchantId?: string;
};

export type Settings = {
  id: string;
  connected: boolean;
  name: string | null;
  paywall: Paywall | null;
  paypal: boolean;
};

export type Price = {
  price: number;
  currency: string;
};

export type PublicPaywall = Price & {
  clientId: string | null;
  merchantId: string;
};

export const currencies = [
  "AUD",
  "BRL",
  "CAD",
  "CHF",
  "CZK",
  "DKK",
  "EUR",
  "GBP",
  "HKD",
  "MXN",
  "NOK",
  "NZD",
  "PLN",
  "SEK",
  "SGD",
  "USD",
];

export const formatPrice = (price: Price) =>
  new Intl.NumberFormat(undefined, { style: "currency", currency: price.currency }).format(
    price.price / 100,
  );

export class BackendError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${backendUrl}/remote-control${path}`, init);
  if (!response.ok) {
    throw new BackendError(response.status, await response.text());
  }
  return response.json();
}

export const createStreamer = () => call<Streamer>("/streamers", { method: "POST" });

export const musicStatus = (id: string) => call<MusicStatus>(`/${encodeURIComponent(id)}/music`);

const songRequest = (song: string, user: string) => ({
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ song, user: user || null }),
});

export const requestSong = (id: string, song: string, user: string) =>
  call<object>(`/${encodeURIComponent(id)}/music`, songRequest(song, user));

export const streamerPaywall = async (id: string) =>
  (await call<{ paywall: PublicPaywall | null }>(`/${encodeURIComponent(id)}/paywall`)).paywall;

export const orderSong = async (id: string, song: string, user: string) =>
  (
    await call<{ orderId: string }>(
      `/${encodeURIComponent(id)}/music/orders`,
      songRequest(song, user),
    )
  ).orderId;

export const captureSong = (id: string, orderId: string) =>
  call<object>(`/${encodeURIComponent(id)}/music/orders/${encodeURIComponent(orderId)}/capture`, {
    method: "POST",
  });

export const streamerSettings = (key: string) =>
  call<Settings>(`/streamer/${encodeURIComponent(key)}/settings`);

export const saveStreamerSettings = (key: string, settings: Pick<Settings, "name" | "paywall">) =>
  call<Settings>(`/streamer/${encodeURIComponent(key)}/settings`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      name: settings.name,
      paywall: settings.paywall,
    }),
  });

export const idOfName = async (name: string) =>
  (await call<{ id: string }>(`/names/${encodeURIComponent(name)}`)).id;

export const keyOf = (text: string) => text.trim().split("/").filter(Boolean).at(-1) ?? "";

export const streamerUrl = (key: string) =>
  `${import.meta.env.DEV ? "ws://localhost:8080" : links.api.replace(/^https/, "wss")}/remote-control/streamer/${key}`;
