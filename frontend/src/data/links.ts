export const links = {
  api: "https://api.moblin.app",
  appStore: "https://apps.apple.com/app/id6466745933",
  testFlight: "https://testflight.apple.com/join/PDpxEaGh",
  discord: "https://discord.gg/kh3KMng4JV",
  github: "https://github.com/eerimoq/moblin",
  chatBotHelp: "https://github.com/eerimoq/moblin/blob/main/docs/chat-bot-help.md",
  privacyPolicy: "https://eerimoq.github.io/moblin/privacy-policy/en.html",
  sponsor: "https://github.com/sponsors/eerimoq",
  paypal: "https://paypal.me/MoblinIRL?country.x=SE&locale.x=en_US",
  twitch: "https://www.twitch.tv/eerimoq",
  mobcam: {
    github: "https://github.com/eerimoq/mobcam",
    releases: "https://github.com/eerimoq/mobcam/releases",
    obsPluginGuide: "https://github.com/eerimoq/mobcam/tree/main/crates/obs-plugin",
  },
  moblink: {
    github: "https://github.com/eerimoq/moblink",
    playStore: "https://play.google.com/store/apps/details?id=com.eerimoq.moblink",
  },
  obsRelay: {
    site: "https://moblin.mys-lang.org/obs-remote-control-relay/",
    github: "https://github.com/eerimoq/obs-remote-control-relay",
  },
  moblinRelay: {
    site: "https://moblin.mys-lang.org/moblin-remote-control-relay/",
    github: "https://github.com/eerimoq/moblin-remote-control-relay",
  },
} as const;

/** Section links, anchored to the home page so they work from every page. */
export const navItems = [
  { label: "Get started", href: "/#get-started" },
  { label: "Features", href: "/#features" },
  { label: "Streamers", href: "/#streamers" },
  { label: "Support", href: "/#support" },
  { label: "Ecosystem", href: "/#ecosystem" },
  { label: "Community", href: "/#community" },
] as const;

/** Pages other than the home page, relative to the site root. */
export const pages = {
  chatBot: "/chat-bot/",
  docs: "/docs/",
  cards: "/cards/",
} as const;

export const platforms = [
  { name: "Twitch", icon: "twitch" },
  { name: "YouTube", icon: "youtube" },
  { name: "Kick", icon: "kick" },
  { name: "Facebook", icon: "facebook" },
  { name: "OBS Studio", icon: "obs" },
] as const;
