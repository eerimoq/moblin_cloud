import { BackendError, backendUrl, call as callBackend, post } from "./backend";
import { t } from "../i18n";

export type Tips = {
  amounts: number[];
};

type SpinOption = {
  spins: number;
  price: number;
};

export type Spin = {
  options: SpinOption[];
};

export type Effects = {
  price: number;
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
  tips: Feature<Tips> | null;
  spin: Feature<Spin> | null;
  effects: Feature<Effects> | null;
  subSpins: Feature<Tiers> | null;
  subEffects: Feature<Tiers> | null;
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

export type PublicTips = PublicPayment & {
  amounts: number[];
  currency: string;
};

export type PublicEffects = PublicPayment & {
  price: number;
  currency: string;
};

export type EffectOption = { id: string; kind: "sound" };

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

export const formatPrice = (price: Price, dropCents = false) =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: price.currency,
    minimumFractionDigits: dropCents && price.price % 100 === 0 ? 0 : 2,
  }).format(price.price / 100);

export const hundredths = (amount: string) =>
  Math.round(Number.parseFloat(amount.trim().replace(",", ".")) * 100);

export const amountText = (hundredths: number) => (hundredths / 100).toFixed(2);

const call = <T>(path: string, init?: RequestInit, viewer?: string) =>
  callBackend<T>(`/pages${path}`, viewer, init);

const pagePath = (id: string, rest = "") => `/${encodeURIComponent(id)}${rest}`;

const segments = { tip: "tips", spin: "spin", effect: "effects" } as const;

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

export type Platform = "twitch" | "kick";

export const platforms: Platform[] = ["twitch", "kick"];

export const platformNames: Record<Platform, string> = { twitch: "Twitch", kick: "Kick" };

export const channelUrl = (platform: Platform, login: string) =>
  platform === "twitch"
    ? `https://www.twitch.tv/${encodeURIComponent(login)}`
    : `https://kick.com/${encodeURIComponent(login)}`;

export const channelText = (platform: Platform, login: string) =>
  platform === "twitch" ? `twitch.tv/${login}` : `kick.com/${login}`;

export type Identity = {
  login: string;
  name: string;
  avatar: string | null;
};

export type Profile = Page & {
  twitch: Identity | null;
  kick: Identity | null;
  image: string | null;
  icon: string | null;
};

export const identityOf = (
  profile: { twitch: Identity | null; kick: Identity | null },
  platform: Platform | null,
): (Identity & { platform: Platform }) | null => {
  for (const name of platform ? [platform] : platforms) {
    const identity = profile[name];
    if (identity) {
      return { ...identity, platform: name };
    }
  }
  return null;
};

export const pageImageUrl = (id: string, version: string) =>
  `${backendUrl}/pages/${encodeURIComponent(id)}/image?v=${encodeURIComponent(version)}`;

export const streamerProfile = (id: string) => call<Profile>(pagePath(id));

export type Allowance = {
  platform: Platform;
  name: string;
  tier: number | null;
  allowance: number;
  left: number;
};

export const viewerSignInUrl = (platform: Platform, id: string) =>
  `${backendUrl}/auth/${platform}/login?${new URLSearchParams({ page: id })}`;

export type Perk = "spin" | "effect";

const perkSegments: Record<Perk, string> = { spin: "spin", effect: "effects" };

export const fetchAllowance = (id: string, perk: Perk, viewer: string) =>
  call<Allowance>(pagePath(id, `/${perkSegments[perk]}/subscriber`), undefined, viewer);

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

export const streamerEffects = (id: string) =>
  call<{
    effects: PublicEffects | null;
    subEffects: Tiers | null;
    options: EffectOption[];
  }>(pagePath(id, "/effects"));

export const requestSubscriberEffect = (
  id: string,
  viewer: string,
  effect: string,
  message: string,
) =>
  call<{ left: number }>(
    pagePath(id, "/effects/subscriber"),
    post({ effect, message: message || null }),
    viewer,
  );

export const orderEffect = (
  id: string,
  effect: string,
  price: number,
  user: string,
  message: string,
  provider: Provider,
) =>
  call<Order>(
    pagePath(id, "/effects/orders"),
    post({ effect, price, user: user || null, message: message || null, provider }),
  );

export type Wheel = {
  options: { text: string; weight: number }[];
};

export const pageEventsUrl = (id: string) => `${backendUrl}/pages/${encodeURIComponent(id)}/events`;

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

export const idOfLogin = async (platform: Platform, login: string) =>
  (await call<{ id: string }>(`/${platform}/${encodeURIComponent(login)}`)).id;
