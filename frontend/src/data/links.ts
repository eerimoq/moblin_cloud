import { asset } from "../asset";

declare global {
  interface Window {
    config: { backendUrl: string };
  }
}

export const links = {
  api: window.config.backendUrl,
  website: "https://moblin.app",
  docs: "https://moblin.app/docs/",
  discord: "https://discord.gg/kh3KMng4JV",
  discordPremium: "https://discord.gg/YzpsZfNSZ",
  github: "https://github.com/eerimoq/moblin",
  privacyPolicy: "https://eerimoq.github.io/moblin/privacy-policy/en.html",
} as const;

/** Pages, relative to the site root. */
export const pages = {
  dashboard: "/",
  assisted: "/dashboard/",
  account: "/account/",
  connect: "/connect/",
  device: "/device/",
  monitor: "/monitor/",
  admin: "/admin/",
  terms: "/terms/",
} as const;

export const deviceUrl = (page: string, device: string) =>
  `${asset(pages.device)}${encodeURIComponent(page)}/${encodeURIComponent(device)}`;

export const monitorUrl = (page: string) => `${asset(pages.monitor)}${encodeURIComponent(page)}`;
