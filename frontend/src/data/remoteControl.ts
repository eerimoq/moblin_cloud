import { BackendError, backendUrl, call as callBackend, post } from "./backend";
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

export type Feature<T> = T & { enabled: boolean };

export const isOn = <T>(feature: Feature<T> | null) => feature?.enabled === true;

export const tierNumbers = [1, 2, 3] as const;

export const tierCount = (tiers: Tiers, tier: number) =>
  tier === 1 ? tiers.tier1 : tier === 2 ? tiers.tier2 : tiers.tier3;

export type Page = {
  message: string | null;
  accent: string | null;
  font: string | null;
};

export type Settings = Page & {
  id: string;
  connected: boolean;
  currency: string;
  paywall: Feature<Paywall> | null;
  tips: Feature<Tips> | null;
  spin: Feature<Spin> | null;
  subSongs: Feature<Tiers> | null;
  subSpins: Feature<Tiers> | null;
  maxSongLength: Feature<{ minutes: number }> | null;
  paypal: boolean;
  merchantId: string | null;
  paypalPending: boolean;
  paypalSandbox: boolean;
  stripe: boolean;
  stripeAccountId: string | null;
  stripePending: boolean;
  stripeTest: boolean;
  payable: boolean;
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

const loadedFonts = new Set<string>();

export const loadFonts = (names: string[]) => {
  const families = names
    .filter((name) => !loadedFonts.has(name))
    .flatMap((name) => {
      loadedFonts.add(name);
      return fontFamilies[name] ?? [];
    });
  if (families.length === 0) {
    return;
  }
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = `https://fonts.googleapis.com/css2?${families.map((family) => `family=${family}`).join("&")}&display=swap`;
  document.head.append(link);
};

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

export const providerName = (provider: Provider) => (provider === "paypal" ? "PayPal" : "Stripe");

export const spinsText = (spins: number) =>
  spins === 1 ? t("1 spin") : t("{count} spins", { count: spins });

export const formatPrice = (price: Price) =>
  new Intl.NumberFormat(undefined, { style: "currency", currency: price.currency }).format(
    price.price / 100,
  );

export const hundredths = (amount: string) =>
  Math.round(Number.parseFloat(amount.trim().replace(",", ".")) * 100);

const call = <T>(path: string, init?: RequestInit, viewer?: string) =>
  callBackend<T>(`/pages${path}`, viewer, init);

const pagePath = (id: string, rest = "") => `/${encodeURIComponent(id)}${rest}`;

const segments = { song: "songs", tip: "tips", spin: "spin" } as const;

export const captureOrder = async (kind: keyof typeof segments, id: string, orderId: string) => {
  const path = pagePath(id, `/${segments[kind]}/orders/${encodeURIComponent(orderId)}/capture`);
  for (let attempt = 1; ; attempt++) {
    try {
      return await call<object>(path, post());
    } catch (error) {
      if ((error instanceof BackendError && error.status < 500) || attempt === 4) {
        throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
    }
  }
};

export const musicStatus = (id: string) => call<MusicStatus>(pagePath(id, "/songs/queue"));

export type Profile = Page & {
  name: string;
  login: string;
  avatar: string | null;
  image: string | null;
};

export const pageImageUrl = (id: string, version: string) =>
  `${backendUrl}/pages/${encodeURIComponent(id)}/image?v=${encodeURIComponent(version)}`;

export const streamerProfile = (id: string) => call<Profile>(pagePath(id));

export const streamerSongs = (id: string) =>
  call<{ paywall: PublicPaywall | null; subSongs: Tiers | null; maxSongMinutes: number | null }>(
    pagePath(id, "/songs"),
  );

export type Allowance = {
  name: string;
  tier: number | null;
  allowance: number;
  left: number;
};

export const viewerSignInUrl = (id: string) =>
  `${backendUrl}/auth/twitch/login?${new URLSearchParams({ page: id })}`;

export const fetchAllowance = (kind: "song" | "spin", id: string, viewer: string) =>
  call<Allowance>(pagePath(id, `/${segments[kind]}/subscriber`), undefined, viewer);

export const requestSubscriberSong = (id: string, viewer: string, song: string, message: string) =>
  call<{ left: number }>(
    pagePath(id, "/songs/subscriber"),
    post({ song, message: message || null }),
    viewer,
  );

export const orderSong = (
  id: string,
  song: string,
  user: string,
  message: string,
  provider: Provider,
) =>
  call<Order>(
    pagePath(id, "/songs/orders"),
    post({ song, user: user || null, message: message || null, provider }),
  );

export const streamerTips = async (id: string) =>
  (await call<{ tips: PublicTips | null }>(pagePath(id, "/tips"))).tips;

export const orderTip = (
  id: string,
  amount: number,
  user: string,
  message: string,
  provider: Provider,
) =>
  call<Order>(
    pagePath(id, "/tips/orders"),
    post({ amount, user: user || null, message: message || null, provider }),
  );

export type Wheel = {
  options: { text: string; weight: number }[];
};

export const streamerSpin = (id: string) =>
  call<{
    spin: PublicSpin | null;
    subSpins: Tiers | null;
    wheel: Wheel | null;
    connected: boolean;
  }>(pagePath(id, "/spin"));

export const requestSubscriberSpin = (id: string, viewer: string, message: string) =>
  call<{ left: number }>(
    pagePath(id, "/spin/subscriber"),
    post({ message: message || null }),
    viewer,
  );

export const orderSpin = (
  id: string,
  option: SpinOption,
  user: string,
  message: string,
  provider: Provider,
) =>
  call<Order>(
    pagePath(id, "/spin/orders"),
    post({
      spins: option.spins,
      price: option.price,
      user: user || null,
      message: message || null,
      provider,
    }),
  );

export const idOfTwitchLogin = async (login: string) =>
  (await call<{ id: string }>(`/twitch/${encodeURIComponent(login)}`)).id;
