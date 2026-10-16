import { backendUrl } from "./backend";

export type Song = {
  id: string;
  title: string;
  artist: string | null;
  duration: number | null;
  user: string | null;
};

export type MusicStatus = {
  playing: boolean;
  currentSongIndex: number | null;
  songs: Song[];
  songNotFound: boolean | null;
};

export const formatDuration = (duration: number) => {
  const seconds = String(duration % 60).padStart(2, "0");
  if (duration < 3600) {
    return `${Math.floor(duration / 60)}:${seconds}`;
  }
  const minutes = String(Math.floor(duration / 60) % 60).padStart(2, "0");
  return `${Math.floor(duration / 3600)}:${minutes}:${seconds}`;
};

export type Paywall = {
  price: number;
  currency: string;
};

export type Tips = {
  currency: string;
  amounts: number[];
  webhookUrl: string | null;
};

export type SpinOption = {
  spins: number;
  price: number;
};

export type Spin = {
  options: SpinOption[];
  currency: string;
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
  tips: Tips | null;
  spin: Spin | null;
  paypal: boolean;
  merchantId: string | null;
  paypalPending: boolean;
  paypalSandbox: boolean;
};

export const accents = ["green", "purple", "blue", "pink", "orange", "red"];

export const defaultTitle = "Thanks for watching!";

export type Price = {
  price: number;
  currency: string;
};

export type PublicPaywall = Price & {
  clientId: string | null;
  merchantId: string | null;
  environment: "production" | "sandbox" | null;
};

export type PayPalConfig = {
  clientId: string;
  merchantId: string;
  environment: "production" | "sandbox";
};

export type PublicTips = {
  amounts: number[];
  currency: string;
  clientId: string | null;
  merchantId: string;
  environment: "production" | "sandbox" | null;
};

export type PublicSpin = {
  options: SpinOption[];
  currency: string;
  clientId: string | null;
  merchantId: string;
  environment: "production" | "sandbox" | null;
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

export const streamerTips = async (id: string) =>
  (await call<{ tips: PublicTips | null }>(`/${encodeURIComponent(id)}/tips`)).tips;

export const orderTip = async (id: string, amount: number, user: string, message: string) =>
  (
    await call<{ orderId: string }>(`/${encodeURIComponent(id)}/tips/orders`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ amount, user: user || null, message: message || null }),
    })
  ).orderId;

export const captureTip = (id: string, orderId: string) =>
  call<object>(`/${encodeURIComponent(id)}/tips/orders/${encodeURIComponent(orderId)}/capture`, {
    method: "POST",
  });

export type Wheel = {
  options: { text: string; weight: number }[];
};

export const streamerSpin = (id: string) =>
  call<{ spin: PublicSpin | null; wheel: Wheel | null }>(`/${encodeURIComponent(id)}/spin`);

export const orderSpin = async (id: string, spins: number, user: string, message: string) =>
  (
    await call<{ orderId: string }>(`/${encodeURIComponent(id)}/spin/orders`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ spins, user: user || null, message: message || null }),
    })
  ).orderId;

export const captureSpin = (id: string, orderId: string) =>
  call<object>(`/${encodeURIComponent(id)}/spin/orders/${encodeURIComponent(orderId)}/capture`, {
    method: "POST",
  });

export const idOfTwitchLogin = async (login: string) =>
  (await call<{ id: string }>(`/names/twitch/${encodeURIComponent(login)}`)).id;
