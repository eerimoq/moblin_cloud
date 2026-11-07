import { asset } from "../asset";

declare global {
  interface Window {
    config: {
      backendUrl: string;
      landingPage?: "default" | "compact";
      instanceName?: string;
    };
  }
}

export const isCompactLanding = () => window.config.landingPage === "compact";

export const instanceName = () => window.config.instanceName?.trim();

export const cloudName = () => {
  const name = instanceName();
  return name ? `Moblin Cloud — ${name}` : "Moblin Cloud";
};

export const links = {
  api: window.config.backendUrl,
  website: "https://moblin.app",
  docs: "https://moblin.app/docs/",
  discord: "https://discord.gg/kh3KMng4JV",
  discordPremium: "https://discord.gg/YzpsZfNSZ",
  github: "https://github.com/eerimoq/moblin",
  cloudGithub: "https://github.com/eerimoq/moblin_cloud",
  privacyPolicy: "https://eerimoq.github.io/moblin/privacy-policy/en.html",
  rickRoll: "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
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
