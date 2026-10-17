import { t } from "../i18n";
import type { IconName } from "../components/Icon";

export type Feature = {
  icon: IconName;
  title: string;
  text: string;
  /** Screenshots shown faintly behind the top right corner of the card. */
  shots?: { src: string; alt: string }[];
};

export const shopIcons = [
  "king",
  "queen",
  "tetris",
  "tub",
  "billionaire",
  "pixels",
  "happy",
  "pink",
].map((name) => `logos/icons/${name}.png`);

export const features: Feature[] = [
  {
    icon: "chat",
    title: t("Chat on screen"),
    text: t(
      "Twitch, YouTube and Kick chat over your camera view. Read it, and show it on stream if you want.",
    ),
  },
  {
    icon: "bell",
    title: t("Alerts and widgets"),
    text: t(
      "Follows, subs, text, a live map, QR codes and more. Drop them anywhere on the picture.",
    ),
  },
  {
    icon: "bonding",
    title: t("Bonding for bad signal"),
    text: t(
      "Use WiFi and cellular at the same time, or add spare phones. When one drops, the stream keeps going.",
    ),
  },
  {
    icon: "watch",
    title: t("Apple Watch companion"),
    text: t(
      "Preview, audio level, bitrate and viewer count on your wrist. The phone stays in your pocket.",
    ),
    shots: [
      {
        src: "screenshots/watch.webp",
        alt: t("Apple Watch showing the stream preview, bitrate, zoom and scene controls"),
      },
      {
        src: "screenshots/watch-chat.webp",
        alt: t("Apple Watch showing Twitch chat messages"),
      },
    ],
  },
  {
    icon: "remote",
    title: t("Remote control"),
    text: t(
      "Run OBS at home from your phone, or let a friend keep an eye on your stream while you walk.",
    ),
  },
  {
    icon: "plug",
    title: t("Integrations"),
    text: t(
      "Works with GoPro and DJI cameras, gimbals, game controllers, heart rate monitors and even your Tesla.",
    ),
  },
];

export const steps = [
  {
    mascot: "default",
    title: t("Install Moblin"),
    text: t("Free on the App Store for iPhone and iPad. Nothing to sign up for."),
  },
  {
    mascot: "looking",
    title: t("Set up your stream"),
    text: t(
      "Pick your platform, paste your stream key and adjust the settings if you like. The defaults work fine to begin with.",
    ),
  },
  {
    mascot: "thumbs-up",
    title: t("Tap Go live"),
    text: t("Chat, bitrate and viewers show up right on screen. On your Apple Watch too."),
  },
] as const;
