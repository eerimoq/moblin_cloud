import { Match, Switch, type JSX } from "solid-js";
import type { RecentKind } from "../data/account";

export type IconName =
  | "chat"
  | "remote"
  | "smartphone"
  | "star"
  | "discord"
  | "github"
  | "warning"
  | "heart"
  | "sparkles"
  | "wheel"
  | "twitch"
  | "youtube"
  | "kick"
  | "facebook"
  | "obs"
  | "menu"
  | "close"
  | "copy"
  | "check"
  | "failed"
  | "clock"
  | "info"
  | "eye"
  | "eye-off"
  | "tip-jar"
  | "wallet"
  | "globe"
  | "image"
  | "chevron-down"
  | "play"
  | "pause"
  | "trash"
  | "users"
  | "user"
  | "sign-out"
  | "chart"
  | "screen"
  | "broadcast"
  | "camera"
  | "mic"
  | "mic-off"
  | "search"
  | "server"
  | "megaphone"
  | "speedometer"
  | "connection"
  | "record"
  | "location"
  | "gamepad"
  | "cpu"
  | "moblink"
  | "flashlight"
  | "stealth"
  | "reload"
  | "gear"
  | "flame"
  | "bolt"
  | "waveform"
  | "chevron-left"
  | "chevron-right"
  | "film"
  | "moon"
  | "grid"
  | "square"
  | "shuffle"
  | "list"
  | "target"
  | "video"
  | "layout"
  | "more";

export const kindIcons: Record<RecentKind, IconName> = {
  tip: "heart",
  spin: "wheel",
  effect: "sparkles",
};

type Props = {
  name: IconName;
  class?: string;
  style?: JSX.CSSProperties;
};

/** Stroke-based icons on a 24px grid. Color comes from `currentColor`. */
export default function Icon(props: Props) {
  return (
    <svg
      viewBox="0 0 24 24"
      class={props.class ?? "size-7"}
      style={props.style}
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <Switch>
        <Match when={props.name === "play"}>
          <path d="M7 5v14l11-7L7 5Z" />
        </Match>
        <Match when={props.name === "pause"}>
          <path d="M8 5v14M16 5v14" />
        </Match>
        <Match when={props.name === "trash"}>
          <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
        </Match>
        <Match when={props.name === "chart"}>
          <path d="M4 20h16M7 17v-7M12 17V5M17 17v-4" />
        </Match>
        <Match when={props.name === "sign-out"}>
          <path d="M10 4H5v16h5M15 8l5 4-5 4M20 12H9" />
        </Match>
        <Match when={props.name === "user"}>
          <circle cx="12" cy="8" r="4" />
          <path d="M4 21c0-4 3.5-6.5 8-6.5s8 2.5 8 6.5" />
        </Match>
        <Match when={props.name === "users"}>
          <circle cx="9" cy="8" r="3.5" />
          <path d="M2 20c0-3.5 3-5.5 7-5.5s7 2 7 5.5" />
          <path d="M15 4.7a3.5 3.5 0 0 1 0 6.6M18 14.8c2.4.6 4 2.3 4 5.2" />
        </Match>
        <Match when={props.name === "failed"}>
          <circle cx="12" cy="12" r="9" />
          <path d="M9 9l6 6M15 9l-6 6" />
        </Match>
        <Match when={props.name === "chat"}>
          <path d="M4 5h16v11H9l-5 4V5Z" />
          <path d="M8 9h8M8 12h5" />
        </Match>
        <Match when={props.name === "remote"}>
          <rect x="3" y="5" width="18" height="12" rx="2" />
          <path d="M8 21h8M12 17v4" />
          <circle cx="12" cy="11" r="2.5" />
        </Match>
        <Match when={props.name === "smartphone"}>
          <rect x="6" y="2" width="12" height="20" rx="2.5" />
          <path d="M11 18h2" />
        </Match>
        <Match when={props.name === "tip-jar"}>
          <path d="M4 10h16M5 10v8a3 3 0 0 0 3 3h8a3 3 0 0 0 3-3v-8" />
          <path d="M7 10a3 3 0 0 1 6 0" />
          <circle cx="15.5" cy="5" r="2" />
        </Match>
        <Match when={props.name === "chevron-down"}>
          <path d="M6 9l6 6 6-6" />
        </Match>
        <Match when={props.name === "globe"}>
          <circle cx="12" cy="12" r="9" />
          <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
        </Match>
        <Match when={props.name === "image"}>
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <circle cx="9" cy="9" r="2" />
          <path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21" />
        </Match>
        <Match when={props.name === "wallet"}>
          <path d="M17 7V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2H5a2 2 0 0 1-2-2" />
          <path d="M16.5 13.5h.01" />
        </Match>
        <Match when={props.name === "discord"}>
          <path d="M4 6.5c2-1 4-1.5 4-1.5l.6 1.3a12 12 0 0 1 6.8 0L16 5s2 .5 4 1.5c1.5 4 2 8 1.5 11-1.6 1.3-4 2.2-4 2.2l-.9-1.6c-2.8 1.2-6.4 1.2-9.2 0L6.5 19.7s-2.4-.9-4-2.2C2 14.5 2.5 10.5 4 6.5Z" />
          <circle cx="9" cy="12.5" r="1.3" />
          <circle cx="15" cy="12.5" r="1.3" />
        </Match>
        <Match when={props.name === "github"}>
          <path d="M9 19c-4 1.5-4-2.5-6-3m12 5v-3.5c0-1 .1-1.4-.5-2 2.8-.3 5.5-1.4 5.5-6a4.6 4.6 0 0 0-1.3-3.2 4.2 4.2 0 0 0-.1-3.2s-1-.3-3.4 1.3a11.7 11.7 0 0 0-6.2 0C6.5 2.8 5.5 3.1 5.5 3.1a4.2 4.2 0 0 0-.1 3.2A4.6 4.6 0 0 0 4 9.5c0 4.6 2.7 5.7 5.5 6-.6.6-.6 1.2-.5 2V21" />
        </Match>
        <Match when={props.name === "warning"}>
          <path d="M12 3l10 18H2L12 3Z" />
          <path d="M12 10v5M12 18h.01" />
        </Match>
        <Match when={props.name === "sparkles"}>
          <path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8L12 3ZM19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8L19 15Z" />
        </Match>
        <Match when={props.name === "heart"}>
          <path d="M12 20s-7-4.5-7-10a4 4 0 0 1 7-2.5A4 4 0 0 1 19 10c0 5.5-7 10-7 10Z" />
        </Match>
        <Match when={props.name === "wheel"}>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" />
        </Match>
        <Match when={props.name === "twitch"}>
          <path
            fill="currentColor"
            stroke="none"
            d="M11.571 4.714h1.715v5.143H11.571zm4.715 0H18v5.143h-1.714zM6 0L1.714 4.286v15.428h5.143V24l4.286-4.286h3.428L22.286 12V0zm14.571 11.143l-3.428 3.428h-3.429l-3 3v-3H6.857V1.714h13.714Z"
          />
        </Match>
        <Match when={props.name === "youtube"}>
          <path
            fill="currentColor"
            stroke="none"
            d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"
          />
        </Match>
        <Match when={props.name === "kick"}>
          <path
            fill="currentColor"
            stroke="none"
            d="M1.333 0h8v5.333H12V2.667h2.667V0h8v8H20v2.667h-2.667v2.666H20V16h2.667v8h-8v-2.667H12v-2.666H9.333V24h-8Z"
          />
        </Match>
        <Match when={props.name === "facebook"}>
          <path
            fill="currentColor"
            stroke="none"
            d="M24 12.073c0-6.627-5.373-12-12-12s-12 5.373-12 12c0 5.99 4.388 10.954 10.125 11.854v-8.385H7.078v-3.47h3.047V9.43c0-3.007 1.792-4.669 4.533-4.669 1.312 0 2.686.235 2.686.235v2.953H15.83c-1.491 0-1.956.925-1.956 1.874v2.25h3.328l-.532 3.47h-2.796v8.385C19.612 23.027 24 18.062 24 12.073z"
          />
        </Match>
        <Match when={props.name === "obs"}>
          <circle cx="12" cy="12" r="10" stroke-width="2.2" />
          <circle cx="12" cy="7.5" r="2" fill="currentColor" stroke="none" />
          <circle cx="8" cy="14.5" r="2" fill="currentColor" stroke="none" />
          <circle cx="16" cy="14.5" r="2" fill="currentColor" stroke="none" />
        </Match>
        <Match when={props.name === "star"}>
          <path
            fill="currentColor"
            stroke="none"
            d="M12 2.5l2.9 6.1 6.6.8-4.9 4.6 1.3 6.6L12 17.3l-5.9 3.3 1.3-6.6-4.9-4.6 6.6-.8L12 2.5Z"
          />
        </Match>
        <Match when={props.name === "menu"}>
          <path d="M4 7h16M4 12h16M4 17h16" />
        </Match>
        <Match when={props.name === "close"}>
          <path d="M6 6l12 12M18 6L6 18" />
        </Match>
        <Match when={props.name === "copy"}>
          <rect x="9" y="9" width="12" height="12" rx="2" />
          <path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1" />
        </Match>
        <Match when={props.name === "clock"}>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 7v5l3 2" />
        </Match>
        <Match when={props.name === "info"}>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 11v5M12 8h.01" />
        </Match>
        <Match when={props.name === "check"}>
          <path d="M5 12.5l4.5 4.5L19 7" />
        </Match>
        <Match when={props.name === "eye"}>
          <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" />
          <circle cx="12" cy="12" r="3" />
        </Match>
        <Match when={props.name === "eye-off"}>
          <path d="M10.6 5.1A10.4 10.4 0 0 1 12 5c6.5 0 10 7 10 7a17.6 17.6 0 0 1-2.4 3.4M6.6 6.6C3.7 8.5 2 12 2 12s3.5 7 10 7c1.9 0 3.5-.6 4.9-1.4M9.9 9.9a3 3 0 0 0 4.2 4.2M3 3l18 18" />
        </Match>
        <Match when={props.name === "screen"}>
          <rect x="2" y="6" width="20" height="12" rx="3" />
          <path d="M5.5 10v4" />
        </Match>
        <Match when={props.name === "broadcast"}>
          <circle cx="12" cy="12" r="1.5" />
          <path d="M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7M5.6 5.6a9 9 0 0 0 0 12.8M18.4 5.6a9 9 0 0 1 0 12.8" />
        </Match>
        <Match when={props.name === "camera"}>
          <path d="M3 8h4l2-3h6l2 3h4v11H3V8Z" />
          <circle cx="12" cy="13" r="3.5" />
        </Match>
        <Match when={props.name === "mic"}>
          <rect x="9" y="3" width="6" height="11" rx="3" />
          <path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6" />
        </Match>
        <Match when={props.name === "mic-off"}>
          <path d="M15 10V6a3 3 0 0 0-5.7-1.3M9 9v2a3 3 0 0 0 4.6 2.5M5 11a7 7 0 0 0 11.5 5.4M19 11a7 7 0 0 1-.6 2.8M12 18v3M9 21h6M3 3l18 18" />
        </Match>
        <Match when={props.name === "search"}>
          <circle cx="11" cy="11" r="6" />
          <path d="m20 20-4.5-4.5" />
        </Match>
        <Match when={props.name === "server"}>
          <rect x="4" y="4" width="16" height="7" rx="1.5" />
          <rect x="4" y="13" width="16" height="7" rx="1.5" />
          <path d="M8 7.5h.01M8 16.5h.01" />
        </Match>
        <Match when={props.name === "megaphone"}>
          <path d="M4 10v4h3l8 4V6l-8 4H4Z" />
          <path d="M18.5 9a4 4 0 0 1 0 6" />
        </Match>
        <Match when={props.name === "speedometer"}>
          <path d="M4.5 18a8.5 8.5 0 1 1 15 0" />
          <path d="m12 14 4-5" />
          <circle cx="12" cy="14" r="1" />
        </Match>
        <Match when={props.name === "connection"}>
          <path d="M5 19v-3M10 19v-6M15 19V9M20 19V5" />
        </Match>
        <Match when={props.name === "record"}>
          <circle cx="12" cy="12" r="8.5" />
          <circle cx="12" cy="12" r="4" fill="currentColor" />
        </Match>
        <Match when={props.name === "location"}>
          <path d="M12 21s-6.5-5.5-6.5-11a6.5 6.5 0 0 1 13 0c0 5.5-6.5 11-6.5 11Z" />
          <circle cx="12" cy="10" r="2.5" />
        </Match>
        <Match when={props.name === "gamepad"}>
          <path d="M7 8h10a4 4 0 0 1 4 4v2.5a2.5 2.5 0 0 1-4.6 1.4L15 14H9l-1.4 1.9A2.5 2.5 0 0 1 3 14.5V12a4 4 0 0 1 4-4Z" />
          <path d="M8 10.5v3M6.5 12h3M15.5 11h.01M17.5 13h.01" />
        </Match>
        <Match when={props.name === "cpu"}>
          <rect x="6" y="6" width="12" height="12" rx="2" />
          <path d="M10 10h4v4h-4zM9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3" />
        </Match>
        <Match when={props.name === "moblink"}>
          <rect x="3" y="3" width="11" height="7" rx="1.5" />
          <rect x="10" y="14" width="11" height="7" rx="1.5" />
          <path d="M8.5 10v7.5H10" />
        </Match>
        <Match when={props.name === "flashlight"}>
          <path d="M7 3h10v4l-2.5 3.5V21h-5V10.5L7 7V3ZM7 7h10" />
          <path d="M12 13v2.5" />
        </Match>
        <Match when={props.name === "stealth"}>
          <path d="M3 17h18M6.5 17a5.5 5.5 0 0 1 11 0M12 5v3M5.6 9.6l1.4 1.4M18.4 9.6 17 11M8 21h8" />
        </Match>
        <Match when={props.name === "reload"}>
          <path d="M20 12a8 8 0 1 1-2.3-5.7M20 3.5V8h-4.5" />
        </Match>
        <Match when={props.name === "gear"}>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
        </Match>
        <Match when={props.name === "flame"}>
          <path d="M12 21.5a6.5 6.5 0 0 0 6.5-6.5c0-4.5-3.5-7-4.5-12-2.5 2-4 4.5-4 7-1-.8-1.8-2-2-3-2 2-2.5 5-2.5 8a6.5 6.5 0 0 0 6.5 6.5Z" />
        </Match>
        <Match when={props.name === "bolt"}>
          <path d="M13 2.5 4.5 14H11l-1 7.5L18.5 10H12l1-7.5Z" fill="currentColor" />
        </Match>
        <Match when={props.name === "waveform"}>
          <path d="M3 10.5v3M7 7v10M11 3.5v17M15 7.5v9M19 10v4" />
        </Match>
        <Match when={props.name === "chevron-left"}>
          <path d="m15 5-7 7 7 7" />
        </Match>
        <Match when={props.name === "chevron-right"}>
          <path d="m9 5 7 7-7 7" />
        </Match>
        <Match when={props.name === "film"}>
          <rect x="4" y="3" width="16" height="18" rx="2" />
          <path d="M8 3v18M16 3v18M4 8h4M4 12h4M4 16h4M16 8h4M16 12h4M16 16h4" />
        </Match>
        <Match when={props.name === "moon"}>
          <path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z" />
        </Match>
        <Match when={props.name === "layout"}>
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M12 4v16M12 12h9" />
        </Match>
        <Match when={props.name === "more"}>
          <circle cx="5" cy="12" r="1" />
          <circle cx="12" cy="12" r="1" />
          <circle cx="19" cy="12" r="1" />
        </Match>
        <Match when={props.name === "grid"}>
          <rect x="4" y="4" width="16" height="16" rx="1.5" />
          <path d="M9.3 4v16M14.7 4v16M4 9.3h16M4 14.7h16" />
        </Match>
        <Match when={props.name === "square"}>
          <rect x="5" y="5" width="14" height="14" rx="1.5" />
        </Match>
        <Match when={props.name === "shuffle"}>
          <path d="M3 7h3.5c4 0 6 10 10 10H21M18 14l3 3-3 3M3 17h3.5c1.5 0 2.7-1.4 3.7-3.2M13.8 10.2c1-1.8 2.2-3.2 3.7-3.2H21M18 4l3 3-3 3" />
        </Match>
        <Match when={props.name === "list"}>
          <path d="M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01" />
        </Match>
        <Match when={props.name === "target"}>
          <circle cx="12" cy="12" r="8" />
          <circle cx="12" cy="12" r="3" />
          <path d="M12 1.5v3M12 19.5v3M1.5 12h3M19.5 12h3" />
        </Match>
        <Match when={props.name === "video"}>
          <rect x="3" y="6" width="13" height="12" rx="2" />
          <path d="m16 10.5 5-3v9l-5-3" />
        </Match>
      </Switch>
    </svg>
  );
}
