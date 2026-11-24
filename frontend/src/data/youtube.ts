const isVideoId = (id: string) => /^[A-Za-z0-9_-]{11}$/.test(id);

function parse(link: string): URL | null {
  const trimmed = link.trim();
  try {
    return new URL(/^https?:\/\//.test(trimmed) ? trimmed : `https://${trimmed}`);
  } catch {
    return null;
  }
}

export function youTubeVideoId(link: string): string | null {
  const trimmed = link.trim();
  if (isVideoId(trimmed)) {
    return trimmed;
  }
  const url = parse(trimmed);
  if (!url) {
    return null;
  }
  const host = url.hostname.toLowerCase().replace(/^(www|m|music)\./, "");
  const [first, second] = url.pathname.split("/").slice(1);
  let id: string | null | undefined;
  if (host === "youtu.be") {
    id = first;
  } else if (host === "youtube.com" && first === "watch") {
    id = url.searchParams.get("v");
  } else if (host === "youtube.com" && ["shorts", "live", "embed"].includes(first)) {
    id = second;
  }
  return id && isVideoId(id) ? id : null;
}

function seconds(time: string): number | null {
  if (/^\d+$/.test(time)) {
    return Number(time);
  }
  const parts = /^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?$/.exec(time);
  if (!parts || time === "") {
    return null;
  }
  return Number(parts[1] ?? 0) * 3600 + Number(parts[2] ?? 0) * 60 + Number(parts[3] ?? 0);
}

export function youTubeStart(link: string): number | null {
  const params = parse(link)?.searchParams;
  const time = params?.get("t") ?? params?.get("start");
  return time == null ? null : seconds(time);
}

export function parseTime(text: string): number | null {
  const parts = text.trim().split(":");
  if (parts.length > 3 || parts.some((part) => !/^\d{1,5}$/.test(part))) {
    return null;
  }
  return parts.reduce((total, part) => total * 60 + Number(part), 0);
}

export function timeText(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = String(seconds % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
}

export const youTubeThumbnail = (video: string) =>
  `https://i.ytimg.com/vi/${encodeURIComponent(video)}/mqdefault.jpg`;

export const youTubeUrl = (video: string, start: number) =>
  `https://www.youtube.com/watch?v=${encodeURIComponent(video)}${start > 0 ? `&t=${start}s` : ""}`;

export function videoLengths(max: number): number[] {
  const lengths = [15, 30, 60, 120, 300, 600].filter((length) => length < max);
  return [...lengths, max];
}
