/** Streamers who recently went live with Moblin, served by the backend in `backend/`. */
import { createResource, createRoot } from "solid-js";
import { backendUrl } from "./backend";

export type StreamerPlatform = "twitch" | "youtube" | "kick";

export type StreamerChannel = {
  platform: StreamerPlatform;
  /** The streamer's handle on that platform, which usually differs between platforms. */
  name: string;
  /** The channel's profile image URL, once the backend has looked it up on the platform. */
  avatar: string | null;
  /** The name shown on the platform, once looked up; Twitch never has one. */
  displayName: string | null;
  live: boolean;
  /** What the channel streams, such as "Just Chatting", while live and when the platform knows. */
  category: string | null;
  /** The stream's title, while live and when the platform knows. */
  title: string | null;
  thumbnail: string | null;
};

/** The name to show for the channel: its display name when known, its handle otherwise. */
export const channelLabel = ({ displayName, name }: StreamerChannel) => displayName ?? name;

export const channelUrl = ({ platform, name }: StreamerChannel) => {
  const encodedName = encodeURIComponent(name);
  switch (platform) {
    case "twitch":
      return `https://www.twitch.tv/${encodedName}`;
    case "youtube":
      return `https://www.youtube.com/@${encodedName}`;
    case "kick":
      return `https://kick.com/${encodeURIComponent(name.replaceAll("_", "-"))}`;
  }
};

export type Streamer = {
  /** Every platform they stream to, at least one. */
  channels: StreamerChannel[];
  /** Their Moblin Cloud page, when they have an account. */
  page: { platform: "twitch" | "kick"; login: string; icon: string | null } | null;
};

export const snapshotUrl = ({ channels }: Streamer) =>
  channels.find((channel) => channel.live && channel.thumbnail)?.thumbnail ?? null;

/** Live streamers first, then the most recently live. */
async function fetchStreamers(): Promise<Streamer[]> {
  const res = await fetch(`${backendUrl}/streamers`);
  if (!res.ok) throw new Error(`fetching streamers failed with status ${res.status}`);
  const body: { streamers: Streamer[] } = await res.json();
  return body.streamers;
}

/** Fetched once; the section shows placeholders until it answers. */
export const [streamers] = createRoot(() => createResource(fetchStreamers));
