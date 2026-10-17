import type { IconName } from "../components/Icon";
import { t } from "../i18n";
import { links } from "./links";

export type Step = {
  /** Bold lead-in, e.g. "1." or "Streamer:" */
  lead?: string;
  text: string;
  /** Rendered as inline code in place of `{code}` in the text. */
  code?: string;
  /** Rendered bold in place of `{path}` in the text, for settings paths. */
  path?: string;
};

export type Link = { label: string; href: string; icon?: IconName };

export type Badge = { src: string; alt: string; href: string };

export type Product = {
  id: string;
  name: string;
  tagline: string;
  description: string;
  logo?: string;
  icon?: IconName;
  steps: Step[];
  /** Store badge image shown instead of the primary button. */
  badge?: Badge;
  primary: Link;
  secondary: Link;
};

export const products: Product[] = [
  {
    id: "moblink",
    name: "Moblink",
    tagline: t("Spare Android phones as extra connections"),
    description: t(
      "Turn old Android phones into extra bonding connections for your Moblin stream. More networks, fewer dropouts. iPhones do this out of the box with Moblin.",
    ),
    logo: "/logos/logo-moblink.png",
    steps: [
      {
        lead: "1.",
        text: t("Install Moblink on the spare phone, ideally on a different carrier."),
      },
      { lead: "2.", text: t("Turn off battery saving so it stays awake.") },
      {
        lead: "3.",
        text: t("Enable Moblink in Moblin. The phones find each other."),
      },
    ],
    badge: {
      src: "badges/google-play.svg",
      alt: t("Get it on Google Play"),
      href: links.moblink.playStore,
    },
    primary: { label: t("Get it on Google Play"), href: links.moblink.playStore },
    secondary: { label: "GitHub", href: links.moblink.github, icon: "github" },
  },
  {
    id: "mobcam",
    name: "Mobcam",
    tagline: t("Your iPhone as a webcam for OBS"),
    description: t(
      "Plug in a USB cable and use your iPhone or iPad running Moblin as a low latency camera in OBS Studio on Mac, Windows or Linux.",
    ),
    logo: "/logos/logo-mobcam.png",
    steps: [
      {
        lead: "1.",
        text: t("Install the Mobcam plugin for OBS Studio 28 or newer."),
      },
      {
        lead: "2.",
        text: t("In Moblin, set your stream URL to {code}"),
        code: "mobcam://localhost:7790",
      },
      { lead: "3.", text: t("Add a Mobcam source in OBS. Done.") },
    ],
    primary: { label: t("Get the plugin"), href: links.mobcam.releases },
    secondary: { label: t("Install guide"), href: links.mobcam.obsPluginGuide },
  },
  {
    id: "obs-relay",
    name: "OBS Remote Control Relay",
    tagline: t("Control OBS at home while you're out"),
    description: t(
      "Switch scenes and watch your bitrate in OBS from Moblin, wherever you are. No port forwarding, no public IP, no VPN. Hosted for free.",
    ),
    icon: "obs-relay",
    steps: [
      {
        lead: "1.",
        text: t("Open the relay page on your streaming computer and copy the Moblin URL."),
      },
      {
        lead: "2.",
        text: t("Paste it in Moblin under {path}"),
        path: t("Settings › Streams › My stream › OBS remote control › URL"),
      },
    ],
    primary: { label: t("Open the relay"), href: links.obsRelay.site },
    secondary: { label: "GitHub", href: links.obsRelay.github, icon: "github" },
  },
  {
    id: "moblin-relay",
    name: "Moblin Remote Control Relay",
    tagline: t("Let a helper run your stream"),
    description: t(
      "A friend at home sees your stream status, changes scenes and fixes settings while you keep walking. Works over the internet, no setup on your router.",
    ),
    icon: "assistant-relay",
    steps: [
      {
        lead: t("Streamer:"),
        text: t("paste the Assistant URL under {path}"),
        path: t("Settings › Remote control › Streamer"),
      },
      {
        lead: t("Helper:"),
        text: t("enter the server port under {path}"),
        path: t("Settings › Remote control › Assistant"),
      },
    ],
    primary: { label: t("Open the relay"), href: links.moblinRelay.site },
    secondary: {
      label: "GitHub",
      href: links.moblinRelay.github,
      icon: "github",
    },
  },
];
