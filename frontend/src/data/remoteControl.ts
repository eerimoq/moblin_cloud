import { backendUrl } from "./backend";

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

export type Paywall = {
  email: string;
  price: number;
  currency: string;
  merchantId?: string;
};

export type Page = {
  title: string | null;
  message: string | null;
  accent: string | null;
};

export type Settings = Page & {
  id: string;
  connected: boolean;
  paywall: Paywall | null;
  paypal: boolean;
};

export const accents = ["green", "purple", "blue", "pink", "orange", "red"];

export const defaultTitle = "Pick the next song.";

export type Price = {
  price: number;
  currency: string;
};

export type PublicPaywall = Price & {
  clientId: string | null;
  merchantId: string | null;
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

export const musicStatus = (id: string) => call<MusicStatus>(`/${encodeURIComponent(id)}/music`);

const songRequest = (song: string, user: string) => ({
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ song, user: user || null }),
});

export const requestSong = (id: string, song: string, user: string) =>
  call<object>(`/${encodeURIComponent(id)}/music`, songRequest(song, user));

export type Profile = Page & {
  name: string;
  login: string;
  avatar: string | null;
};

export const streamerProfile = (id: string) => call<Profile>(`/${encodeURIComponent(id)}/profile`);

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

export const idOfTwitchLogin = async (login: string) =>
  (await call<{ id: string }>(`/names/twitch/${encodeURIComponent(login)}`)).id;
