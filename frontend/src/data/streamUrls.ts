import { t } from "../i18n";

type Parsed = { scheme: string | null; host: string | null; port: number | null };

const parse = (value: string): Parsed | null => {
  if (value === "" || /\s/.test(value)) {
    return null;
  }
  const scheme = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(value)?.[1] ?? null;
  if (scheme === null) {
    return { scheme: null, host: null, port: null };
  }
  try {
    const url = new URL(value);
    return {
      scheme,
      host:
        value.slice(scheme.length + 1).startsWith("//") && url.hostname !== ""
          ? url.hostname
          : null,
      port: url.port === "" ? null : Number(url.port),
    };
  } catch {
    return null;
  }
};

export const makeRtmpUri = (url: string) => url.split("/").slice(0, -1).join("/");

export const makeRtmpStreamKey = (url: string) => url.split("/").filter(Boolean).at(-1) ?? "";

export const cleanUrl = (value: string) => {
  const stripped = value.replaceAll(" ", "");
  const scheme = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(stripped)?.[1];
  return scheme ? scheme.toLowerCase() + stripped.slice(scheme.length) : stripped;
};

const isValidRtmpUrl = (url: string, rtmpStreamKeyRequired: boolean) => {
  if (!rtmpStreamKeyRequired) {
    return null;
  }
  if (makeRtmpUri(url) === "") {
    return t("Malformed RTMP URL");
  }
  if (makeRtmpStreamKey(url) === "") {
    return t("RTMP stream key missing");
  }
  return null;
};

export const isValidUrl = (
  value: string,
  allowedSchemes?: string[],
  rtmpStreamKeyRequired = true,
): string | null => {
  const url = parse(value);
  if (url === null) {
    return t("Malformed URL");
  }
  if (url.scheme !== null && url.host === null) {
    return t("Host missing");
  }
  if (url.scheme === null) {
    return t("Scheme missing");
  }
  if (url.port !== null && (url.port <= 0 || url.port > 65535)) {
    return t("Bad port");
  }
  if (allowedSchemes && !allowedSchemes.includes(url.scheme)) {
    return `Only ${allowedSchemes.join(" and ")} allowed, not ${url.scheme}`;
  }
  switch (url.scheme) {
    case "rtmp":
    case "rtmps":
      return isValidRtmpUrl(value, rtmpStreamKeyRequired);
    case "srt":
    case "srtla":
      return url.port === null ? t("SRT(LA) port number missing") : null;
    case "rist":
      return url.port === null ? t("RIST port number missing") : null;
    case "whip":
    case "whips":
    case "rtsp":
      return null;
    case "mobcam":
      if (!["localhost", "127.0.0.1"].includes((url.host ?? "").toLowerCase())) {
        return t("Mobcam host must be localhost");
      }
      return url.port === null ? t("Mobcam port number missing") : null;
    case "http":
    case "https":
      return isValidHttpUrl(value);
    default:
      return t("Unsupported scheme {scheme}", { scheme: url.scheme });
  }
};

const isValidSchemeUrl = (value: string, schemes: string[]): string | null => {
  if (value === "") {
    return null;
  }
  const url = parse(value);
  if (url === null) {
    return t("Malformed URL");
  }
  if (url.scheme === null) {
    return t("Scheme missing");
  }
  if (url.host === null) {
    return t("Host missing");
  }
  if (!schemes.includes(url.scheme)) {
    return t("Unsupported scheme {scheme}", { scheme: url.scheme });
  }
  return null;
};

export const isValidWebSocketUrl = (value: string) => isValidSchemeUrl(value, ["ws", "wss"]);

export const isValidHttpUrl = (value: string) => isValidSchemeUrl(value, ["http", "https"]);

export const urlPort = (value: string) => parse(value)?.port ?? null;
