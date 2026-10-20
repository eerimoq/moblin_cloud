import { backendUrl } from "./backend";
import { t } from "../i18n";

export type MusicCommand = "play" | "pause" | "clear" | "remove" | "select";

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
  songRequests: boolean;
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
};

export type Tips = {
  amounts: number[];
};

export type SpinOption = {
  spins: number;
  price: number;
};

export type Spin = {
  options: SpinOption[];
};

export type Tiers = {
  tier1: number;
  tier2: number;
  tier3: number;
};

export const tierNumbers = [1, 2, 3] as const;

export const tierCount = (tiers: Tiers, tier: number) =>
  tier === 1 ? tiers.tier1 : tier === 2 ? tiers.tier2 : tiers.tier3;

export type Page = {
  title: string | null;
  message: string | null;
  accent: string | null;
  font: string | null;
};

export type Settings = Page & {
  id: string;
  connected: boolean;
  currency: string;
  paywall: Paywall | null;
  tips: Tips | null;
  spin: Spin | null;
  subSongs: Tiers | null;
  subSpins: Tiers | null;
  maxSongMinutes: number | null;
  paypal: boolean;
  merchantId: string | null;
  paypalPending: boolean;
  paypalSandbox: boolean;
  stripe: boolean;
  stripeAccountId: string | null;
  stripePending: boolean;
  stripeTest: boolean;
  image: string | null;
};

export const accents = ["green", "purple", "blue", "pink", "orange", "red"];

export const isColor = (accent: string | null | undefined) =>
  accent != null && /^#[0-9a-f]{6}$/i.test(accent);

export const accentAttributes = (accent: string | null | undefined) =>
  isColor(accent)
    ? { "data-accent": "custom", style: { "--accent": accent! } }
    : { "data-accent": accent ?? undefined };

const fontFamilies: Record<string, string[]> = {
  rounded: [],
  modern: ["Poppins:wght@500;600;700"],
  serif: ["Playfair+Display:wght@500..700"],
  mono: ["JetBrains+Mono:wght@500..700"],
  handwritten: ["Caveat:wght@500..700"],
  pixel: ["Pixelify+Sans:wght@500..700"],
};

export const fonts = Object.keys(fontFamilies);

export const loadFonts = (names: string[]) => {
  const families = names.flatMap((name) => fontFamilies[name] ?? []);
  if (families.length === 0) {
    return;
  }
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = `https://fonts.googleapis.com/css2?${families.map((family) => `family=${family}`).join("&")}&display=swap`;
  document.head.append(link);
};

export const defaultTitle = t("Thanks for watching!");

export type Price = {
  price: number;
  currency: string;
};

export type PayPalConfig = {
  clientId: string;
  merchantId: string;
  environment: "production" | "sandbox";
};

export type StripeConfig = {
  publishableKey: string;
  accountId: string;
};

export type Provider = "paypal" | "stripe";

export type PublicPayment = {
  clientId: string | null;
  merchantId: string | null;
  environment: "production" | "sandbox" | null;
  stripe?: StripeConfig | null;
};

export type PublicPaywall = Price & PublicPayment;

export type PublicTips = PublicPayment & {
  amounts: number[];
  currency: string;
};

export type PublicSpin = PublicPayment & {
  options: SpinOption[];
  currency: string;
};

export type Order = {
  orderId: string;
  clientSecret?: string;
  price?: number;
  currency?: string;
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

export const hundredths = (amount: string) =>
  Math.round(Number.parseFloat(amount.trim().replace(",", ".")) * 100);

export class BackendError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${backendUrl}/pages${path}`, init);
  if (!response.ok) {
    throw new BackendError(response.status, await response.text());
  }
  return response.json();
}

const capture = async (path: string) => {
  for (let attempt = 1; ; attempt++) {
    try {
      return await call<object>(path, { method: "POST" });
    } catch (error) {
      if ((error instanceof BackendError && error.status < 500) || attempt === 4) {
        throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
    }
  }
};

export const musicStatus = (id: string) =>
  call<MusicStatus>(`/${encodeURIComponent(id)}/songs/queue`);

const post = (body: unknown) => ({
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

export type Profile = Page & {
  name: string;
  login: string;
  avatar: string | null;
  image: string | null;
};

export const pageImageUrl = (id: string, version: string) =>
  `${backendUrl}/pages/${encodeURIComponent(id)}/image?v=${encodeURIComponent(version)}`;

export const streamerProfile = (id: string) => call<Profile>(`/${encodeURIComponent(id)}`);

export const streamerSongs = (id: string) =>
  call<{ paywall: PublicPaywall | null; subSongs: Tiers | null; maxSongMinutes: number | null }>(
    `/${encodeURIComponent(id)}/songs`,
  );

export type Allowance = {
  name: string;
  tier: number | null;
  allowance: number;
  left: number;
};

export const viewerSignInUrl = (id: string, login?: string) =>
  `${backendUrl}/auth/twitch/login?${new URLSearchParams({ page: id, ...(login ? { login } : {}) })}`;

export const fetchSongAllowance = (id: string, viewer: string) =>
  call<Allowance>(`/${encodeURIComponent(id)}/songs/subscriber`, {
    headers: { authorization: `Bearer ${viewer}` },
  });

export const requestSubscriberSong = (id: string, viewer: string, song: string, message: string) =>
  call<{ left: number }>(`/${encodeURIComponent(id)}/songs/subscriber`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${viewer}` },
    body: JSON.stringify({ song, message: message || null }),
  });

export const orderSong = (
  id: string,
  song: string,
  user: string,
  message: string,
  provider: Provider,
) =>
  call<Order>(
    `/${encodeURIComponent(id)}/songs/orders`,
    post({ song, user: user || null, message: message || null, provider }),
  );

export const captureSong = (id: string, orderId: string) =>
  capture(`/${encodeURIComponent(id)}/songs/orders/${encodeURIComponent(orderId)}/capture`);

export const streamerTips = async (id: string) =>
  (await call<{ tips: PublicTips | null }>(`/${encodeURIComponent(id)}/tips`)).tips;

export const orderTip = (
  id: string,
  amount: number,
  user: string,
  message: string,
  provider: Provider,
) =>
  call<Order>(
    `/${encodeURIComponent(id)}/tips/orders`,
    post({ amount, user: user || null, message: message || null, provider }),
  );

export const captureTip = (id: string, orderId: string) =>
  capture(`/${encodeURIComponent(id)}/tips/orders/${encodeURIComponent(orderId)}/capture`);

export type Wheel = {
  options: { text: string; weight: number }[];
};

export const streamerSpin = (id: string) =>
  call<{
    spin: PublicSpin | null;
    subSpins: Tiers | null;
    wheel: Wheel | null;
    connected: boolean;
  }>(`/${encodeURIComponent(id)}/spin`);

export const fetchSpinAllowance = (id: string, viewer: string) =>
  call<Allowance>(`/${encodeURIComponent(id)}/spin/subscriber`, {
    headers: { authorization: `Bearer ${viewer}` },
  });

export const requestSubscriberSpin = (id: string, viewer: string, message: string) =>
  call<{ left: number }>(`/${encodeURIComponent(id)}/spin/subscriber`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${viewer}` },
    body: JSON.stringify({ message: message || null }),
  });

export const orderSpin = (
  id: string,
  spins: number,
  user: string,
  message: string,
  provider: Provider,
) =>
  call<Order>(
    `/${encodeURIComponent(id)}/spin/orders`,
    post({ spins, user: user || null, message: message || null, provider }),
  );

export const captureSpin = (id: string, orderId: string) =>
  capture(`/${encodeURIComponent(id)}/spin/orders/${encodeURIComponent(orderId)}/capture`);

export const idOfTwitchLogin = async (login: string) =>
  (await call<{ id: string }>(`/twitch/${encodeURIComponent(login)}`)).id;
