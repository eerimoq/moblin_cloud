import {
  derivedId,
  idOf,
  isPlainObject,
  merged,
  mergedList,
  newUuid,
  type SettingsAlignment,
} from "./moblinSettings";

export type StreamResolution =
  | "4032x3024"
  | "3840x2160"
  | "2560x1440"
  | "1920x1440"
  | "1920x1080"
  | "1664x936"
  | "1280x720"
  | "1024x768"
  | "960x540"
  | "854x480"
  | "640x360"
  | "426x240";

export const streamResolutions: { id: StreamResolution; name: string }[] = [
  { id: "4032x3024", name: "3024p (4:3)" },
  { id: "3840x2160", name: "4K" },
  { id: "2560x1440", name: "1440p" },
  { id: "1920x1440", name: "1440p (4:3)" },
  { id: "1920x1080", name: "1080p" },
  { id: "1664x936", name: "936p" },
  { id: "1280x720", name: "720p" },
  { id: "1024x768", name: "768p (4:3)" },
  { id: "960x540", name: "540p" },
  { id: "854x480", name: "480p" },
  { id: "640x360", name: "360p" },
  { id: "426x240", name: "240p" },
];

export const streamFpss = [120, 100, 60, 50, 30, 25, 15];

export type StreamCodec = "H.265/HEVC" | "H.264/AVC";
export type StreamH264Profile = "Baseline" | "Main" | "High";
export type StreamColorRange = "Full" | "Limited";
export type StreamRateControl = "ABR" | "CBR" | "VBR";
export type StreamAudioCodec = "AAC" | "OPUS";
export type SrtImplementation = "Moblin" | "Official";
export type DnsLookupStrategy = "System" | "IPv4" | "IPv6" | "IPv4 and IPv6";
export type SrtAdaptiveBitrateAlgorithm =
  | { belabox: object }
  | { fastIrl: object }
  | { slowIrl: object }
  | { customIrl: object };
export type SrtAdaptiveBitrateAlgorithmName = "belabox" | "fastIrl" | "slowIrl" | "customIrl";
export type WhipHttpTransport = { standard: object } | { remoteControl: object };
export type ReplayTransitionType = "fade" | "stingers" | "none";
export type YouTubeVisibility = "public" | "private" | "unlisted";
export type StreamProtocol = "RTMP" | "SRT" | "RIST" | "WHIP" | "Mobcam";

export type SrtConnectionPriority = {
  id: string;
  name: string;
  priority: number;
  enabled: boolean;
  relayId?: string | null;
};

export type SettingsStreamSrt = {
  latency: number;
  maximumBandwidthFollowInput: boolean;
  overheadBandwidth: number;
  adaptiveBitrateEnabled: boolean;
  adaptiveBitrate: {
    algorithm: SrtAdaptiveBitrateAlgorithm;
    fastIrlSettings: { packetsInFlight: number; minimumBitrate: number };
    customSettings: {
      packetsInFlight: number;
      pifDiffIncreaseFactor: number;
      rttDiffHighDecreaseFactor: number;
      rttDiffHighAllowedSpike: number;
      rttDiffHighMinimumDecrease: number;
      minimumBitrate: number;
    };
    belaboxSettings: { minimumBitrate: number };
  };
  connectionPriorities: { enabled: boolean; priorities: SrtConnectionPriority[] };
  mpegtsPacketsPerPacket: number;
  dnsLookupStrategy: DnsLookupStrategy;
  implementation: SrtImplementation;
  bigPackets: boolean;
  bigPacketsMigrated: boolean;
  implemenationMigrated: boolean;
};

export type HttpHeader = { name: string; value: string };

export type SettingsStreamRecording = {
  overrideStream: boolean;
  resolution: StreamResolution;
  fps: number;
  videoCodec: StreamCodec;
  videoBitrate: number;
  maxKeyFrameInterval: number;
  audioBitrate: number;
  autoStartRecording: boolean;
  autoStopRecording: boolean;
  cleanRecordings: boolean;
  cleanSnapshots: boolean;
  recordingPath?: string | null;
};

export type ReplayStinger = { id: string; name: string; transitionPoint: number };

export type SettingsStreamReplay = {
  enabled: boolean;
  transitionType: ReplayTransitionType;
  inStinger: ReplayStinger;
  outStinger: ReplayStinger;
  postTriggerDelay: number;
  x: number;
  y: number;
  size: number;
  alignment: SettingsAlignment;
  positioningLock: boolean;
};

export type MultiStreamingDestination = {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
};

export type SettingsTwitchAlerts = {
  follows: boolean;
  subscriptions: boolean;
  giftSubscriptions: boolean;
  resubscriptions: boolean;
  rewards: boolean;
  raids: boolean;
  cheers: boolean;
  minimumCheerBits: number;
  watchStreaks: boolean;
  minimumWatchStreak: number;
  sharedChat: boolean;
};

export type SettingsKickAlerts = {
  subscriptions: boolean;
  giftedSubscriptions: boolean;
  rewards: boolean;
  hosts: boolean;
  bans: boolean;
  kicks: boolean;
  minimumKicks: number;
};

export type SettingsStream = {
  id: string;
  name: string;
  enabled: boolean;
  url: string;
  twitchChannelName: string;
  twitchChannelId: string;
  twitchChatAlerts: SettingsTwitchAlerts;
  twitchToastAlerts: SettingsTwitchAlerts;
  twitchLoggedIn: boolean;
  kickChannelName: string;
  kickChannelId?: string | null;
  kickChatroomChannelId?: string | null;
  kickSlug?: string | null;
  kickLoggedIn: boolean;
  kickChatAlerts: SettingsKickAlerts;
  kickToastAlerts: SettingsKickAlerts;
  youTubeVideoId: string;
  youTubeWantsToBeLoggedIn: boolean;
  youTubeHandle: string;
  youTubeScheduleStreamTitle: string;
  youTubeScheduleStreamVisibility: YouTubeVisibility;
  youTubeScheduleStreamAutoStop: boolean;
  afreecaTvChannelName: string;
  afreecaTvStreamId: string;
  openStreamingPlatformUrl: string;
  openStreamingPlatformChannelId: string;
  obsWebSocketEnabled: boolean;
  obsWebSocketUrl: string;
  obsWebSocketPassword: string;
  obsSourceName: string;
  obsMainScene: string;
  obsBrbScene: string;
  obsBrbSceneVideoSourceBroken: boolean;
  obsAutoStartStream: boolean;
  obsAutoStopStream: boolean;
  obsAutoStartRecording: boolean;
  obsAutoStopRecording: boolean;
  streamingDirectlyToObs: boolean;
  discordSnapshotWebhook: string;
  discordChatBotSnapshotWebhook: string;
  discordSnapshotWebhookOnlyWhenLive: boolean;
  resolution: StreamResolution;
  fps: number;
  autoFps: boolean;
  bitrate: number;
  bitrateRateControl: StreamRateControl;
  codec: StreamCodec;
  h264Profile: StreamH264Profile;
  colorRange: StreamColorRange;
  bFrames: boolean;
  adaptiveEncoderResolution: boolean;
  adaptiveEncoderResolutionThreashold: number;
  adaptiveBitrate: boolean;
  srt: SettingsStreamSrt;
  rtmp: { adaptiveBitrateEnabled: boolean };
  rist: { adaptiveBitrateEnabled: boolean; bonding: boolean };
  whip: { headers: HttpHeader[]; httpTransport: WhipHttpTransport };
  maxKeyFrameInterval: number;
  audioCodec: StreamAudioCodec;
  audioBitrate: number;
  chat: { bttvEmotes: boolean; ffzEmotes: boolean; seventvEmotes: boolean };
  recording: SettingsStreamRecording;
  realtimeIrlEnabled: boolean;
  realtimeIrlBaseUrl: string;
  realtimeIrlPushKey: string;
  portrait: boolean;
  backgroundStreaming: boolean;
  backgroundStreamingPiP: boolean;
  estimatedViewerDelay: number;
  ntpPoolAddress: string;
  timecodesEnabled: boolean;
  replay: SettingsStreamReplay;
  goLiveNotificationDiscordMessage: string;
  goLiveNotificationDiscordWebhookUrl: string;
  goLiveNotificationMoblinWebsite: boolean;
  multiStreaming: { destinations: MultiStreamingDestination[] };
  previewStream: { url: string; resolution: StreamResolution; bitrate: number };
  autoGoLive: boolean;
};

export const defaultStreamUrl = "srt://my_public_ip:4000";
export const defaultRtmpStreamUrl = "rtmp://my_public_ip:1935/live/foobar";
export const mobcamStreamPort = 7790;

const twitchAlerts = (): SettingsTwitchAlerts => ({
  follows: true,
  subscriptions: true,
  giftSubscriptions: true,
  resubscriptions: true,
  rewards: true,
  raids: true,
  cheers: true,
  minimumCheerBits: 0,
  watchStreaks: true,
  minimumWatchStreak: 5,
  sharedChat: false,
});

const kickAlerts = (): SettingsKickAlerts => ({
  subscriptions: true,
  giftedSubscriptions: true,
  rewards: true,
  hosts: true,
  bans: true,
  kicks: true,
  minimumKicks: 0,
});

const connectionPriority = (id: string, name: string): SrtConnectionPriority => ({
  id,
  name,
  priority: 1,
  enabled: true,
});

export const defaultStream = (id: string, name = "My stream"): SettingsStream => ({
  id,
  name,
  enabled: false,
  url: defaultStreamUrl,
  twitchChannelName: "",
  twitchChannelId: "",
  twitchChatAlerts: twitchAlerts(),
  twitchToastAlerts: twitchAlerts(),
  twitchLoggedIn: false,
  kickChannelName: "",
  kickLoggedIn: false,
  kickChatAlerts: kickAlerts(),
  kickToastAlerts: kickAlerts(),
  youTubeVideoId: "",
  youTubeWantsToBeLoggedIn: false,
  youTubeHandle: "",
  youTubeScheduleStreamTitle: "",
  youTubeScheduleStreamVisibility: "public",
  youTubeScheduleStreamAutoStop: true,
  afreecaTvChannelName: "",
  afreecaTvStreamId: "",
  openStreamingPlatformUrl: "",
  openStreamingPlatformChannelId: "",
  obsWebSocketEnabled: false,
  obsWebSocketUrl: "",
  obsWebSocketPassword: "",
  obsSourceName: "",
  obsMainScene: "",
  obsBrbScene: "",
  obsBrbSceneVideoSourceBroken: false,
  obsAutoStartStream: false,
  obsAutoStopStream: false,
  obsAutoStartRecording: false,
  obsAutoStopRecording: false,
  streamingDirectlyToObs: false,
  discordSnapshotWebhook: "",
  discordChatBotSnapshotWebhook: "",
  discordSnapshotWebhookOnlyWhenLive: true,
  resolution: "1920x1080",
  fps: 30,
  autoFps: false,
  bitrate: 5_000_000,
  bitrateRateControl: "ABR",
  codec: "H.265/HEVC",
  h264Profile: "Main",
  colorRange: "Full",
  bFrames: false,
  adaptiveEncoderResolution: false,
  adaptiveEncoderResolutionThreashold: 1,
  adaptiveBitrate: true,
  srt: {
    latency: 3000,
    maximumBandwidthFollowInput: true,
    overheadBandwidth: 25,
    adaptiveBitrateEnabled: true,
    adaptiveBitrate: {
      algorithm: { belabox: {} },
      fastIrlSettings: { packetsInFlight: 200, minimumBitrate: 250 },
      customSettings: {
        packetsInFlight: 200,
        pifDiffIncreaseFactor: 100,
        rttDiffHighDecreaseFactor: 0.9,
        rttDiffHighAllowedSpike: 50,
        rttDiffHighMinimumDecrease: 250,
        minimumBitrate: 250,
      },
      belaboxSettings: { minimumBitrate: 250 },
    },
    connectionPriorities: {
      enabled: false,
      priorities: [
        connectionPriority(derivedId(`${id}:priority:Cellular`), "Cellular"),
        connectionPriority(derivedId(`${id}:priority:WiFi`), "WiFi"),
      ],
    },
    mpegtsPacketsPerPacket: 7,
    dnsLookupStrategy: "System",
    implementation: "Moblin",
    bigPackets: true,
    bigPacketsMigrated: true,
    implemenationMigrated: true,
  },
  rtmp: { adaptiveBitrateEnabled: true },
  rist: { adaptiveBitrateEnabled: true, bonding: true },
  whip: { headers: [], httpTransport: { standard: {} } },
  maxKeyFrameInterval: 2,
  audioCodec: "AAC",
  audioBitrate: 128_000,
  chat: { bttvEmotes: false, ffzEmotes: false, seventvEmotes: false },
  recording: {
    overrideStream: false,
    resolution: "1920x1080",
    fps: 30,
    videoCodec: "H.265/HEVC",
    videoBitrate: 0,
    maxKeyFrameInterval: 0,
    audioBitrate: 128_000,
    autoStartRecording: false,
    autoStopRecording: false,
    cleanRecordings: false,
    cleanSnapshots: false,
  },
  realtimeIrlEnabled: false,
  realtimeIrlBaseUrl: "https://rtirl.com/api",
  realtimeIrlPushKey: "",
  portrait: false,
  backgroundStreaming: false,
  backgroundStreamingPiP: true,
  estimatedViewerDelay: 8,
  ntpPoolAddress: "time.apple.com",
  timecodesEnabled: false,
  replay: {
    enabled: false,
    transitionType: "fade",
    inStinger: { id: derivedId(`${id}:inStinger`), name: "", transitionPoint: 0.5 },
    outStinger: { id: derivedId(`${id}:outStinger`), name: "", transitionPoint: 0.5 },
    postTriggerDelay: 3,
    x: 0,
    y: 0,
    size: 100,
    alignment: "TopLeft",
    positioningLock: false,
  },
  goLiveNotificationDiscordMessage: "",
  goLiveNotificationDiscordWebhookUrl: "",
  goLiveNotificationMoblinWebsite: false,
  multiStreaming: { destinations: [] },
  previewStream: { url: "", resolution: "640x360", bitrate: 500_000 },
  autoGoLive: false,
});

export const defaultDestination = (name: string): MultiStreamingDestination => ({
  id: newUuid(),
  name,
  url: defaultRtmpStreamUrl,
  enabled: false,
});

export const normalizeStream = (raw: Record<string, unknown>): SettingsStream => {
  const id = idOf(raw, "stream");
  const stream = merged(defaultStream(id), raw);
  const srt = isPlainObject(raw.srt) ? raw.srt : {};
  const priorities = isPlainObject(srt.connectionPriorities) ? srt.connectionPriorities : {};
  if (Array.isArray(priorities.priorities)) {
    stream.srt.connectionPriorities.priorities = mergedList(priorities.priorities, (item, index) =>
      connectionPriority(idOf(item, `${id}:priority:${index}`), ""),
    );
  }
  const whip = isPlainObject(raw.whip) ? raw.whip : {};
  stream.whip.headers = mergedList(whip.headers, () => ({ name: "", value: "" }));
  const multiStreaming = isPlainObject(raw.multiStreaming) ? raw.multiStreaming : {};
  stream.multiStreaming.destinations = mergedList(multiStreaming.destinations, (_, index) => ({
    id: derivedId(`${id}:destination:${index}`),
    name: "My destination",
    url: defaultRtmpStreamUrl,
    enabled: false,
  }));
  return stream;
};

export const encodeStream = (stream: SettingsStream): Record<string, unknown> => ({
  ...stream,
  multiStreaming: {
    ...stream.multiStreaming,
    destinations: stream.multiStreaming.destinations.map(({ name, url, enabled }) => ({
      name,
      url,
      enabled,
    })),
  },
});

export const cloneStream = (stream: SettingsStream): SettingsStream => {
  const copy = structuredClone(stream);
  copy.id = newUuid();
  copy.enabled = false;
  copy.twitchLoggedIn = false;
  copy.kickLoggedIn = false;
  copy.youTubeWantsToBeLoggedIn = false;
  for (const priority of copy.srt.connectionPriorities.priorities) {
    priority.id = newUuid();
  }
  for (const destination of copy.multiStreaming.destinations) {
    destination.id = newUuid();
  }
  return copy;
};

export const srtAlgorithmName = (
  algorithm: SrtAdaptiveBitrateAlgorithm,
): SrtAdaptiveBitrateAlgorithmName =>
  (Object.keys(algorithm)[0] as SrtAdaptiveBitrateAlgorithmName | undefined) ?? "belabox";

export const urlScheme = (url: string) => {
  const match = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(url);
  return match ? match[1].toLowerCase() : "";
};

export const streamProtocol = (stream: SettingsStream): StreamProtocol => {
  switch (urlScheme(stream.url)) {
    case "srt":
    case "srtla":
      return "SRT";
    case "rist":
      return "RIST";
    case "whip":
    case "whips":
      return "WHIP";
    case "mobcam":
      return "Mobcam";
    default:
      return "RTMP";
  }
};

export const isSrtla = (stream: SettingsStream) => urlScheme(stream.url) === "srtla";
