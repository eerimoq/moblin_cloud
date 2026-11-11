import {
  defaultStream,
  defaultStreamUrl,
  mobcamStreamPort,
  type SettingsStream,
  type StreamAudioCodec,
  type StreamCodec,
} from "./moblinStreamSettings";
import { newUuid } from "./moblinSettings";
import { cleanUrl, isValidUrl, isValidWebSocketUrl } from "./streamUrls";

export type WizardPlatform = "twitch" | "kick" | "youTube" | "soop" | "custom" | "obs" | "mobcam";

export type WizardNetworkSetup = "obs" | "belaboxCloudObs" | "direct" | "myServers";

export type WizardCustomProtocol = "none" | "srt" | "rtmp" | "rist" | "whip";

export type CreateStreamWizard = {
  platform: WizardPlatform;
  networkSetup: WizardNetworkSetup;
  customProtocol: WizardCustomProtocol;
  name: string;
  backgroundStreaming: boolean;
  autoGoLive: boolean;
  goLiveNotificationMoblinWebsite: boolean;
  twitchChannelName: string;
  twitchChannelId: string;
  kickChannelName: string;
  youTubeHandle: string;
  soopChannelName: string;
  soopStreamId: string;
  obsAddress: string;
  obsPort: string;
  obsRemoteControlEnabled: boolean;
  obsRemoteControlUrl: string;
  obsRemoteControlPassword: string;
  obsRemoteControlSourceName: string;
  obsRemoteControlMainScene: string;
  obsRemoteControlBrbScene: string;
  directIngest: string;
  directStreamKey: string;
  belaboxUrl: string;
  customSrtUrl: string;
  customSrtStreamId: string;
  customRtmpUrl: string;
  customRtmpStreamKey: string;
  customRistUrl: string;
  customWhipUrl: string;
};

export const defaultWizard = (): CreateStreamWizard => ({
  platform: "custom",
  networkSetup: "direct",
  customProtocol: "none",
  name: "",
  backgroundStreaming: false,
  autoGoLive: false,
  goLiveNotificationMoblinWebsite: false,
  twitchChannelName: "",
  twitchChannelId: "",
  kickChannelName: "",
  youTubeHandle: "",
  soopChannelName: "",
  soopStreamId: "",
  obsAddress: "",
  obsPort: "",
  obsRemoteControlEnabled: false,
  obsRemoteControlUrl: "",
  obsRemoteControlPassword: "",
  obsRemoteControlSourceName: "",
  obsRemoteControlMainScene: "",
  obsRemoteControlBrbScene: "",
  directIngest: "",
  directStreamKey: "",
  belaboxUrl: "",
  customSrtUrl: "",
  customSrtStreamId: "",
  customRtmpUrl: "",
  customRtmpStreamKey: "",
  customRistUrl: "",
  customWhipUrl: "",
});

const customCodecs: Record<WizardCustomProtocol, StreamCodec> = {
  none: "H.264/AVC",
  srt: "H.265/HEVC",
  rtmp: "H.264/AVC",
  rist: "H.265/HEVC",
  whip: "H.264/AVC",
};

const customAudioCodecs: Record<WizardCustomProtocol, StreamAudioCodec> = {
  none: "AAC",
  srt: "AAC",
  rtmp: "AAC",
  rist: "AAC",
  whip: "OPUS",
};

const decode = (text: string) => {
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
};

const encode = (text: string, allowed: RegExp) =>
  Array.from(new TextEncoder().encode(text))
    .map((byte) => {
      const character = String.fromCharCode(byte);
      return byte < 128 && allowed.test(character)
        ? character
        : `%${byte.toString(16).toUpperCase().padStart(2, "0")}`;
    })
    .join("");

const queryItemAllowed = /[A-Za-z0-9\-._~!$'()*+,;:@/?]/;

const pathAllowed = /[A-Za-z0-9\-._~!$&'()*+,;=:@/]/;

type QueryItem = { name: string; value: string | null };

const splitUrl = (url: string) => {
  const fragmentIndex = url.indexOf("#");
  const beforeFragment = fragmentIndex === -1 ? url : url.slice(0, fragmentIndex);
  const fragment = fragmentIndex === -1 ? "" : url.slice(fragmentIndex);
  const queryIndex = beforeFragment.indexOf("?");
  return {
    base: queryIndex === -1 ? beforeFragment : beforeFragment.slice(0, queryIndex),
    query: queryIndex === -1 ? null : beforeFragment.slice(queryIndex + 1),
    fragment,
  };
};

const queryItems = (query: string): QueryItem[] =>
  query === ""
    ? []
    : query.split("&").map((item) => {
        const index = item.indexOf("=");
        return index === -1
          ? { name: decode(item), value: null }
          : { name: decode(item.slice(0, index)), value: decode(item.slice(index + 1)) };
      });

export const extractSrtStreamId = (url: string): string | null => {
  if (/\s/.test(url)) {
    return null;
  }
  const { query } = splitUrl(url);
  if (query === null) {
    return null;
  }
  let streamId: string | null = null;
  for (const item of queryItems(query)) {
    if (item.name === "streamid" && item.value !== null) {
      streamId = item.value;
    }
  }
  return streamId;
};

const replaceSrtStreamId = (url: string, streamId: string) => {
  const { base, query, fragment } = splitUrl(url);
  if (query === null) {
    return url;
  }
  const items = queryItems(query).filter((item) => item.name !== "streamid");
  items.push({ name: "streamid", value: streamId });
  const encoded = items
    .map((item) =>
      item.value === null
        ? encode(item.name, queryItemAllowed)
        : `${encode(item.name, queryItemAllowed)}=${encode(item.value, queryItemAllowed)}`,
    )
    .join("&");
  return `${base}?${encoded}${fragment}`;
};

const appendRtmpStreamKey = (url: string, streamKey: string) => {
  const trimmed = url.replace(/^\/+|\/+$/g, "");
  const { base, query, fragment } = splitUrl(trimmed);
  return `${base}/${encode(streamKey, pathAllowed)}${query === null ? "" : `?${query}`}${fragment}`;
};

const customUrl = (wizard: CreateStreamWizard): string | null => {
  switch (wizard.customProtocol) {
    case "none":
      return null;
    case "srt":
      return replaceSrtStreamId(wizard.customSrtUrl.trim(), wizard.customSrtStreamId.trim());
    case "rtmp":
      return appendRtmpStreamKey(wizard.customRtmpUrl.trim(), wizard.customRtmpStreamKey.trim());
    case "rist":
      return wizard.customRistUrl.trim();
    case "whip":
      return wizard.customWhipUrl.trim();
  }
};

const cleanWizardUrl = (url: string) => {
  const cleaned = cleanUrl(url);
  return isValidUrl(cleaned) === null ? cleaned : defaultStreamUrl;
};

const wizardUrl = (wizard: CreateStreamWizard) => {
  let url = defaultStreamUrl;
  switch (wizard.platform) {
    case "custom":
      url = customUrl(wizard) ?? url;
      break;
    case "mobcam":
      url = `mobcam://localhost:${mobcamStreamPort}`;
      break;
    default:
      switch (wizard.networkSetup) {
        case "obs":
          url = `srt://${wizard.obsAddress}:${wizard.obsPort}`;
          break;
        case "belaboxCloudObs":
          url = wizard.belaboxUrl;
          break;
        case "direct":
          url = `${wizard.directIngest.replace(/^\/+|\/+$/g, "")}/${wizard.directStreamKey}`;
          break;
        case "myServers":
          url = customUrl(wizard) ?? url;
          break;
      }
  }
  return cleanWizardUrl(url);
};

export const createStreamFromWizard = (
  wizard: CreateStreamWizard,
  highestBitrate = 5_000_000,
): SettingsStream => {
  const stream = defaultStream(newUuid(), wizard.name.trim());
  stream.backgroundStreaming = wizard.backgroundStreaming;
  stream.autoGoLive = wizard.autoGoLive;
  stream.goLiveNotificationMoblinWebsite = wizard.goLiveNotificationMoblinWebsite;
  if (
    wizard.platform !== "custom" &&
    wizard.networkSetup !== "direct" &&
    wizard.obsRemoteControlEnabled
  ) {
    const url = cleanUrl(wizard.obsRemoteControlUrl.trim());
    if (isValidWebSocketUrl(url) === null) {
      stream.obsWebSocketEnabled = true;
      stream.obsWebSocketUrl = url;
      stream.obsWebSocketPassword = wizard.obsRemoteControlPassword.trim();
      stream.obsSourceName = wizard.obsRemoteControlSourceName.trim();
      stream.obsMainScene = wizard.obsRemoteControlMainScene.trim();
      stream.obsBrbScene = wizard.obsRemoteControlBrbScene.trim();
      stream.streamingDirectlyToObs = wizard.networkSetup === "obs";
    }
  }
  switch (wizard.platform) {
    case "twitch":
      stream.twitchChannelName = wizard.twitchChannelName.trim();
      stream.twitchChannelId = wizard.twitchChannelId.trim();
      break;
    case "kick":
      stream.kickChannelName = wizard.kickChannelName.trim();
      break;
    case "youTube":
      stream.youTubeHandle = wizard.youTubeHandle.trim();
      break;
    case "soop":
      if (wizard.soopChannelName !== "" && wizard.soopStreamId !== "") {
        stream.afreecaTvChannelName = wizard.soopChannelName.trim();
        stream.afreecaTvStreamId = wizard.soopStreamId.trim();
      }
      break;
    case "obs":
    case "custom":
    case "mobcam":
      break;
  }
  stream.chat.bttvEmotes = false;
  stream.chat.ffzEmotes = false;
  stream.chat.seventvEmotes = false;
  stream.url = wizardUrl(wizard);
  stream.bitrateRateControl =
    stream.url.startsWith("rtmp") || stream.url.startsWith("mobcam") ? "CBR" : "ABR";
  stream.audioBitrate = 128_000;
  switch (wizard.platform) {
    case "custom":
      stream.codec = customCodecs[wizard.customProtocol];
      stream.audioCodec = customAudioCodecs[wizard.customProtocol];
      break;
    case "mobcam":
      stream.codec = "H.265/HEVC";
      stream.bitrate = highestBitrate;
      stream.audioCodec = "AAC";
      stream.audioBitrate = 192_000;
      break;
    default:
      switch (wizard.networkSetup) {
        case "obs":
        case "belaboxCloudObs":
          stream.codec = "H.265/HEVC";
          break;
        case "direct":
          stream.codec = "H.264/AVC";
          break;
        case "myServers":
          stream.codec = customCodecs[wizard.customProtocol];
          stream.audioCodec = customAudioCodecs[wizard.customProtocol];
          break;
      }
  }
  return stream;
};
