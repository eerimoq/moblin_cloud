import { Match, Switch, type JSX } from "solid-js";

export type IconName =
  | "chat"
  | "bell"
  | "bonding"
  | "watch"
  | "remote"
  | "plug"
  | "star"
  | "tap"
  | "discord"
  | "github"
  | "obs-relay"
  | "assistant-relay"
  | "warning"
  | "heart"
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
  | "eye"
  | "eye-off"
  | "music"
  | "person"
  | "wallet";

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
        <Match when={props.name === "chat"}>
          <path d="M4 5h16v11H9l-5 4V5Z" />
          <path d="M8 9h8M8 12h5" />
        </Match>
        <Match when={props.name === "bell"}>
          <path d="M6 17V10a6 6 0 0 1 12 0v7l2 2H4l2-2Z" />
          <path d="M10 21h4" />
        </Match>
        <Match when={props.name === "bonding"}>
          <path d="M4 12h5l2-5 3 10 2-5h4" />
        </Match>
        <Match when={props.name === "watch"}>
          <rect x="7" y="6" width="10" height="12" rx="3" />
          <path d="M9 6V3h6v3M9 18v3h6v-3" />
        </Match>
        <Match when={props.name === "remote"}>
          <rect x="3" y="5" width="18" height="12" rx="2" />
          <path d="M8 21h8M12 17v4" />
          <circle cx="12" cy="11" r="2.5" />
        </Match>
        <Match when={props.name === "person"}>
          <circle cx="12" cy="8" r="4" />
          <path d="M4 21a8 8 0 0 1 16 0" />
        </Match>
        <Match when={props.name === "wallet"}>
          <path d="M17 7V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2H5a2 2 0 0 1-2-2" />
          <path d="M16.5 13.5h.01" />
        </Match>
        <Match when={props.name === "plug"}>
          <path d="M9 3v5M15 3v5" />
          <path d="M6 8h12v3a6 6 0 0 1-12 0V8Z" />
          <path d="M12 17v4" />
        </Match>
        <Match when={props.name === "discord"}>
          <path d="M4 6.5c2-1 4-1.5 4-1.5l.6 1.3a12 12 0 0 1 6.8 0L16 5s2 .5 4 1.5c1.5 4 2 8 1.5 11-1.6 1.3-4 2.2-4 2.2l-.9-1.6c-2.8 1.2-6.4 1.2-9.2 0L6.5 19.7s-2.4-.9-4-2.2C2 14.5 2.5 10.5 4 6.5Z" />
          <circle cx="9" cy="12.5" r="1.3" />
          <circle cx="15" cy="12.5" r="1.3" />
        </Match>
        <Match when={props.name === "github"}>
          <path d="M9 19c-4 1.5-4-2.5-6-3m12 5v-3.5c0-1 .1-1.4-.5-2 2.8-.3 5.5-1.4 5.5-6a4.6 4.6 0 0 0-1.3-3.2 4.2 4.2 0 0 0-.1-3.2s-1-.3-3.4 1.3a11.7 11.7 0 0 0-6.2 0C6.5 2.8 5.5 3.1 5.5 3.1a4.2 4.2 0 0 0-.1 3.2A4.6 4.6 0 0 0 4 9.5c0 4.6 2.7 5.7 5.5 6-.6.6-.6 1.2-.5 2V21" />
        </Match>
        <Match when={props.name === "obs-relay"}>
          <rect x="2" y="6" width="9" height="12" rx="2" />
          <rect x="13" y="6" width="9" height="12" rx="2" />
          <path d="M11 12h2" />
          <circle cx="6.5" cy="12" r="1.5" />
          <path d="M17.5 10v4M15.5 12h4" />
        </Match>
        <Match when={props.name === "assistant-relay"}>
          <circle cx="8" cy="8" r="3" />
          <path d="M2 20c0-3.3 2.7-6 6-6s6 2.7 6 6" />
          <path d="M16 4l2 2-2 2M20 6h-4" />
          <rect x="15" y="12" width="7" height="9" rx="1.5" />
        </Match>
        <Match when={props.name === "warning"}>
          <path d="M12 3l10 18H2L12 3Z" />
          <path d="M12 10v5M12 18h.01" />
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
        <Match when={props.name === "tap"}>
          <path d="M10 9.5V4a2 2 0 0 0-4 0v10" />
          <path d="M14 10V9a2 2 0 0 0-4 0v1" />
          <path d="M18 11v-1a2 2 0 0 0-4 0v1" />
          <path d="M18 11a2 2 0 1 1 4 0v3a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.9-6-2.3l-3.6-3.6a2 2 0 0 1 2.8-2.8L6 15" />
          <path d="M2.5 3.5 4 5M13.5 3.5 12 5M8 .5V2" />
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
        <Match when={props.name === "music"}>
          <path d="M9 18V5l11-2v13" />
          <circle cx="6" cy="18" r="3" />
          <circle cx="17" cy="16" r="3" />
        </Match>
      </Switch>
    </svg>
  );
}
