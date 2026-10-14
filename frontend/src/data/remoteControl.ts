import { backendUrl } from "./backend";
import { links } from "./links";

export type Song = {
  id: string;
  title: string;
  artist: string | null;
  user: string | null;
};

export type MusicStatus = {
  playing: boolean;
  currentSongIndex: number | null;
  songs: Song[];
  songNotFound: boolean | null;
};

export type Streamer = {
  key: string;
  id: string;
};

export class BackendError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${backendUrl}/remote-control${path}`, init);
  if (!response.ok) {
    throw new BackendError(response.status, await response.text());
  }
  return response.json();
}

export const createStreamer = () => call<Streamer>("/streamers", { method: "POST" });

export const musicStatus = (id: string) => call<MusicStatus>(`/${encodeURIComponent(id)}/music`);

export const addSong = (id: string, song: string, user: string) =>
  call<MusicStatus>(`/${encodeURIComponent(id)}/music`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ song, user: user || null }),
  });

export const streamerUrl = (key: string) =>
  `${import.meta.env.DEV ? "ws://localhost:8080" : links.api.replace(/^https/, "wss")}/remote-control/streamer/${key}`;
