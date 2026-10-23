import { t } from "../i18n";

export const cloudSteps: { title: string; text: string }[] = [
  {
    title: t("Sign in"),
    text: t("Sign in with Twitch or Kick and your page is ready."),
  },
  {
    title: t("Connect payments"),
    text: t("Connect PayPal or Stripe, so the money goes straight to you."),
  },
  {
    title: t("Connect Moblin"),
    text: t(
      "Scan the QR code in the dashboard with Moblin's device, then pick your alerts and wheel " +
        "of luck widgets in its Moblin Cloud settings.",
    ),
  },
  {
    title: t("Share your page"),
    text: t("Put the link in your chat, panels or bio."),
  },
];
