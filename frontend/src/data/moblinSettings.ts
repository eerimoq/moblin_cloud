import { t } from "../i18n";

export type RgbColor = { red: number; green: number; blue: number; opacity?: number };

export const rgb = (red: number, green: number, blue: number, opacity?: number): RgbColor =>
  opacity === undefined ? { red, green, blue } : { red, green, blue, opacity };

export type SettingsAlignment =
  | "TopLeft"
  | "TopRight"
  | "BottomLeft"
  | "BottomRight"
  | "TopCenter"
  | "BottomCenter"
  | "LeftCenter"
  | "RightCenter"
  | "Center";

export const isLeft = (alignment: SettingsAlignment) =>
  alignment === "TopLeft" || alignment === "BottomLeft" || alignment === "LeftCenter";
export const isHorizontalCenter = (alignment: SettingsAlignment) =>
  alignment === "TopCenter" || alignment === "BottomCenter" || alignment === "Center";
export const isVerticalCenter = (alignment: SettingsAlignment) =>
  alignment === "LeftCenter" || alignment === "RightCenter" || alignment === "Center";
export const isHorizontalOrVerticalCenter = (alignment: SettingsAlignment) =>
  isHorizontalCenter(alignment) || isVerticalCenter(alignment);
export const isTop = (alignment: SettingsAlignment) =>
  alignment === "TopLeft" || alignment === "TopRight" || alignment === "TopCenter";
export const mirrorPositionHorizontally = (alignment: SettingsAlignment) =>
  alignment === "TopRight" || alignment === "BottomRight" || alignment === "RightCenter";
export const mirrorPositionVertically = (alignment: SettingsAlignment) =>
  alignment === "BottomLeft" || alignment === "BottomRight" || alignment === "BottomCenter";

export type CameraPosition =
  | "Back"
  | "Front"
  | "RTMP"
  | "External"
  | "SRT(LA)"
  | "SRT client"
  | "RIST"
  | "RTSP"
  | "WHIP"
  | "WHEP"
  | "Media player"
  | "Screen capture"
  | "Back triple"
  | "Back dual"
  | "Back wide dual"
  | "None";

export const screenCaptureSourceId = "00000000-CAFE-BABE-BEEF-000000000000";
export const noVideoSourceId = "00000000-FEED-B1AC-CAFE-000000000000";

export type VideoSourceFields = {
  cameraPosition: CameraPosition;
  backCameraId: string;
  frontCameraId: string;
  rtmpCameraId: string;
  srtlaCameraId: string;
  srtClientCameraId: string;
  ristCameraId: string;
  rtspCameraId: string;
  whipCameraId: string;
  whepCameraId: string;
  mediaPlayerCameraId: string;
  externalCameraId: string;
  externalCameraName: string;
};

export type VideoSource = { id: string; name: string; position: CameraPosition };

export type SettingsSceneWidget = {
  id: string;
  widgetId: string;
  x: number;
  y: number;
  size: number;
  alignment: SettingsAlignment;
  positioningLock: boolean;
};

export type VideoStabilizationMode =
  | "Off"
  | "Standard"
  | "Cinematic"
  | "Cinematic extended enhanced";

export type SettingsScene = VideoSourceFields & {
  name: string;
  id: string;
  enabled: boolean;
  widgets: SettingsSceneWidget[];
  videoSourceRotation: number;
  videoStabilizationMode: VideoStabilizationMode;
  overrideVideoStabilizationMode: boolean;
  fillFrame: boolean;
  overrideMic: boolean;
  micId: string;
  quickSwitchGroup: number | null;
  mirror: boolean;
  backgroundColor: RgbColor;
};

export type SettingsWidgetType =
  | "Text"
  | "Browser"
  | "Video source"
  | "Image"
  | "Alerts"
  | "Map"
  | "Snapshot"
  | "Chat"
  | "Chat emote combo"
  | "Scene"
  | "Slideshow"
  | "VTuber"
  | "PNGTuber"
  | "QR code"
  | "Scoreboard"
  | "Wheel of luck"
  | "Bingo card"
  | "Crop"
  | "Pomodoro timer";

export const widgetTypes: SettingsWidgetType[] = [
  "Text",
  "Browser",
  "Video source",
  "Image",
  "Alerts",
  "Map",
  "Snapshot",
  "Chat",
  "Chat emote combo",
  "Scene",
  "Slideshow",
  "VTuber",
  "PNGTuber",
  "QR code",
  "Scoreboard",
  "Wheel of luck",
  "Bingo card",
  "Crop",
  "Pomodoro timer",
];

export const widgetTypeName = (type: SettingsWidgetType): string =>
  ({
    Text: t("Text"),
    Browser: t("Browser"),
    "Video source": t("Video source"),
    Image: t("Image"),
    Alerts: t("Alerts"),
    Map: t("Map"),
    Snapshot: t("Snapshot"),
    Chat: t("Chat"),
    "Chat emote combo": t("Chat emote combo"),
    Scene: t("Scene"),
    Slideshow: t("Slideshow"),
    VTuber: t("VTuber"),
    PNGTuber: t("PNGTuber"),
    "QR code": t("QR code"),
    Scoreboard: t("Scoreboard"),
    "Wheel of luck": t("Wheel of luck"),
    "Bingo card": t("Bingo card"),
    Crop: t("Crop"),
    "Pomodoro timer": t("Pomodoro timer"),
  })[type];

export const widgetTypeDescription = (type: SettingsWidgetType): string =>
  ({
    Text: t("A text widget shows text, weather, clock and much more."),
    Browser: t("A browser widget shows a webpage."),
    "Video source": t("A video source widget shows another camera or screen capture."),
    Image: t("An image widget shows an image."),
    Alerts: t("An alerts widget shows various alerts (subscriptions, raids, ...)."),
    Map: t("A map widget shows a map with your location."),
    Snapshot: t("A snapshot widget shows snapshots when taken."),
    Chat: t("A chat widget shows your chat."),
    "Chat emote combo": t(
      "A chat emote combo widget shows an emote streak when chatters spam the same emote.",
    ),
    Scene: t("A scene widget shows a scene's widgets."),
    Slideshow: t("A slideshow widget shows a slideshow of widgets."),
    VTuber: t("A VTuber widget shows a VRM or Live2D model that imitates your facial movements."),
    PNGTuber: t("A PNGTuber widget shows a PNGTuber model that imitates your facial movements."),
    "QR code": t("A QR code widget shows a QR code of any text."),
    Scoreboard: t("A scoreboard widget shows a sports scoreboard, controlled with an Apple Watch."),
    "Wheel of luck": t("A wheel of luck widget shows a wheel of luck that you can spin."),
    "Bingo card": t("A bingo card widget shows an interactive bingo card."),
    Crop: t("A crop widget shows parts of a browser widget."),
    "Pomodoro timer": t("A Pomodoro timer widget shows a focus and break timer."),
  })[type];

export const widgetTypeSymbol = (type: SettingsWidgetType): string =>
  ({
    Text: "textformat",
    Browser: "globe",
    "Video source": "video",
    Image: "photo",
    Alerts: "megaphone",
    Map: "map",
    Snapshot: "camera.aperture",
    Chat: "message",
    "Chat emote combo": "hands.clap",
    Scene: "photo.on.rectangle",
    Slideshow: "play.rectangle",
    VTuber: "person.crop.circle",
    PNGTuber: "person.crop.circle.dashed",
    "QR code": "qrcode",
    Scoreboard: "rectangle.split.2x1",
    "Wheel of luck": "burn",
    "Bingo card": "square.grid.3x3.square",
    Crop: "crop",
    "Pomodoro timer": "timer",
  })[type];

export const hasPosition = (type: SettingsWidgetType) => type !== "Scene";

export const hasSize = (type: SettingsWidgetType) =>
  [
    "Image",
    "Browser",
    "Crop",
    "Map",
    "QR code",
    "Video source",
    "VTuber",
    "PNGTuber",
    "Snapshot",
    "Chat emote combo",
    "Slideshow",
    "Bingo card",
    "Pomodoro timer",
    "Scoreboard",
  ].includes(type);

export const hasAlignment = hasPosition;

export type FontDesign = "Default" | "Serif" | "Rounded" | "Monospaced";
export type FontWeight = "Regular" | "Light" | "Bold";
export type HorizontalAlignment = "Leading" | "Trailing" | "Center";

export type SettingsWidgetText = {
  formatString: string;
  backgroundColor: RgbColor;
  clearBackgroundColor: boolean;
  foregroundColor: RgbColor;
  clearForegroundColor: boolean;
  fontSize: number;
  fontFamily: string | null;
  fontStyle: string;
  fontDesign: FontDesign;
  fontWeight: FontWeight;
  fontMonospacedDigits: boolean;
  alignment: HorizontalAlignment;
  horizontalAlignment: HorizontalAlignment;
  verticalAlignment: "Top" | "Bottom";
  delay: number;
  timers: { id: string; delta: number; endTime: number }[];
  stopwatches: { id: string; totalElapsed: number; running: boolean }[];
  needsWeather: boolean;
  needsGeography: boolean;
  needsSubtitles: boolean;
  subtitles: { identifier?: string }[];
  checkboxes: { id: string; checked: boolean }[];
  ratings: { id: string; rating: number }[];
  lapTimes: { id: string; currentLapStartTime?: number; lapTimes: number[] }[];
  needsGForce: boolean;
  widthEnabled: boolean;
  width: number;
  cornerRadius: number;
};

export type BrowserMode = "periodicAudioAndVideo" | "audioAndVideoOnly" | "audioOnly";

export type SettingsWidgetBrowser = {
  url: string;
  width: number;
  height: number;
  mode: BrowserMode;
  fps: number;
  styleSheet: string;
  moblinAccess: boolean;
  speechToText: boolean;
  localOnly: boolean;
};

export type SettingsWidgetCrop = {
  sourceWidgetId: string;
  x: number;
  y: number;
  width: number;
  height: number;
};

export type SettingsWidgetMap = { northUp: boolean; delay: number; scale: number };

export type SettingsWidgetScene = { sceneId: string };

export type SettingsWidgetQrCode = { message: string };

export type VoiceType = "apple" | "ttsMonster" | "elevenLabs";

export type SettingsVoice = {
  type: VoiceType;
  apple: { voice: string };
  ttsMonster: { name: string; voiceId: string };
  elevenLabs: { name: string; voiceId: string };
};

export type SettingsAlert = {
  id: string;
  enabled: boolean;
  mediaType: "gifAndSound" | "video";
  imageId: string;
  imageLoopCount: number;
  soundId: string;
  videoName: string;
  textColor: RgbColor;
  accentColor: RgbColor;
  fontSize: number;
  fontDesign: FontDesign;
  fontWeight: FontWeight;
  textToSpeechEnabled: boolean;
  textToSpeechDelay: number;
  textToSpeechLanguageVoices: Record<string, SettingsVoice>;
  positionType: "Scene" | "Face";
  facePosition: { x: number; y: number; width: number; height: number };
};

export type CheerBitsAlert = {
  id: string;
  bits: number;
  comparisonOperator: "=" | ">=";
  alert: SettingsAlert;
};

export type KickGiftsAlert = {
  id: string;
  amount: number;
  comparisonOperator: "=" | ">=";
  alert: SettingsAlert;
};

export type SettingsWidgetAlerts = {
  twitch: {
    follows: SettingsAlert;
    subscriptions: SettingsAlert;
    raids: SettingsAlert;
    cheers: SettingsAlert;
    cheerBits: CheerBitsAlert[];
  };
  kick: {
    subscriptions: SettingsAlert;
    giftedSubscriptions: SettingsAlert;
    hosts: SettingsAlert;
    rewards: SettingsAlert;
    kickGifts: KickGiftsAlert[];
  };
  moblinCloud: { tips: SettingsAlert };
  chatBot: { commands: { id: string; name: string; alert: SettingsAlert; imageType: "File" }[] };
  speechToText: { strings: { id: string; string: string; alert: SettingsAlert }[] };
  needsSubtitles: boolean;
  ai: { baseUrl: string; apiKey: string; model: string; role: string };
  aiEnabled: boolean;
  ttsMonster: { apiToken: string };
};

export type SettingsWidgetVideoSource = VideoSourceFields & {
  cornerRadius: number;
  cropEnabled: boolean;
  cropX: number;
  cropY: number;
  cropWidth: number;
  cropHeight: number;
  rotation: number;
  trackFaceEnabled: boolean;
  trackFaceZoom: number;
  mirror: boolean;
  borderWidth: number;
  borderColor: RgbColor;
};

export type ScoreboardSport =
  | "generic"
  | "padel"
  | "golf"
  | "golfFullScorecard"
  | "basketball"
  | "generic2"
  | "genericSets"
  | "hockey"
  | "football"
  | "tennis"
  | "volleyball";

export type ClockDirection = { up: object } | { down: object };
export type ScoreboardLayout =
  | { stacked: object }
  | { stackedInline: object }
  | { sideBySide: object }
  | { stackHistory: object };

export type ScoreboardClock = { maximum: number; direction: ClockDirection };
export type ScoreboardTeam = { name: string; textColor: RgbColor; backgroundColor: RgbColor };
export type ScoreboardPlayer = { id: string; name: string };

export type SettingsWidgetScoreboard = {
  type: ScoreboardSport;
  textColor: RgbColor;
  primaryBackgroundColor: RgbColor;
  secondaryBackgroundColor: RgbColor;
  padel: {
    type: "Double" | "Single";
    homePlayer1: string;
    homePlayer2: string;
    awayPlayer1: string;
    awayPlayer2: string;
    score: { home: number; away: number }[];
  };
  golf: {
    eventName: string;
    numberOfHoles: number;
    currentHole: number;
    pars: number[];
    players: { id: string; name: string; scores: number[]; color: RgbColor }[];
    playerColors: boolean;
    showPars: boolean;
  };
  generic: { home: string; away: string; title: string; period: string; clock: ScoreboardClock };
  modular: {
    home: ScoreboardTeam;
    away: ScoreboardTeam;
    title: string;
    period: string;
    infoBoxText: string;
    clock: ScoreboardClock;
    layout: ScoreboardLayout;
    width: number;
    rowHeight: number;
    isBold: boolean;
    showTitle: boolean;
    showMoreStats: boolean;
    showGlobalStatsBlock: boolean;
    showClock: boolean;
  };
};

export type SettingsWidgetVTuber = VideoSourceFields & {
  id: string;
  type: "VRM" | "Live2D";
  cameraPositionY: number;
  cameraFieldOfView: number;
  modelName: string;
  mirror: boolean;
  sensitivity: { mouth: number; eyes: number };
  armsAngle: number;
};

export type SettingsWidgetPngTuber = VideoSourceFields & {
  id: string;
  modelName: string;
  mirror: boolean;
  sensitivity: { mouth: number; eyes: number };
};

export type SettingsWidgetSnapshot = { id: string; showtime: number };

export type ChatDisplayStyle = "internationalName" | "internationalNameAndUsername" | "username";

export type SettingsWidgetChat = {
  id: string;
  fontSize: number;
  fontFamily: string | null;
  fontStyle: string;
  usernameColor: RgbColor;
  messageColor: RgbColor;
  backgroundColor: RgbColor;
  backgroundColorEnabled: boolean;
  shadowColor: RgbColor;
  shadowColorEnabled: boolean;
  boldUsername: boolean;
  boldMessage: boolean;
  badges: boolean;
  displayStyle: ChatDisplayStyle;
  sharedChatIcons: boolean;
  height: number;
  maximumNumberOfMessages: number;
};

export type SettingsWidgetChatEmoteCombo = { minimumCombo: number; resetAfter: number };

export type SettingsWidgetSlideshow = {
  id: string;
  slides: { id: string; widgetId: string | null; time: number }[];
};

export type GimbalMotion =
  | { kapow: object }
  | { yes: object }
  | { no: object }
  | { wakeup: object };

export type WheelOfLuckOption = {
  id: string;
  text: string;
  weight: number;
  function: string;
  sceneId: string | null;
  widgetId: string | null;
  gimbalPresetId: string | null;
  gimbalMotion: GimbalMotion;
  macroId: string | null;
  streamDeckLayoutId: string | null;
};

export type SettingsWidgetWheelOfLuck = {
  advanced: boolean;
  onlyShowWhenSpinning: boolean;
  options: WheelOfLuckOption[];
};

export type SettingsWidgetBingoCard = {
  backgroundColor: RgbColor;
  foregroundColor: RgbColor;
  squares: { id: string; text: string; checked: boolean }[];
};

export const pomodoroFocusIcons = [
  "sun.max",
  "bolt.circle",
  "graduationcap",
  "text.book.closed",
  "arrowtriangle.right.circle",
  "brain.head.profile",
  "pencil",
  "flame",
  "timer",
  "target",
];

export const pomodoroBreakIcons = [
  "cup.and.saucer",
  "figure.dance",
  "figure.walk",
  "music.note",
  "leaf",
  "gamecontroller",
  "fork.knife",
];

export type SettingsWidgetPomodoroTimer = {
  focusDuration: number;
  breakDuration: number;
  width: number;
  focusName: string;
  breakName: string;
  focusIcon: string;
  breakIcon: string;
  backgroundColor: RgbColor;
  foregroundColor: RgbColor;
  focusColor: RgbColor;
  breakColor: RgbColor;
  focusToBreakSoundId: string | null;
  breakToFocusSoundId: string | null;
  focusToBreakChatMessage: string;
  breakToFocusChatMessage: string;
};

export type VideoEffectType =
  | "shape"
  | "grayScale"
  | "sepia"
  | "whirlpool"
  | "pinch"
  | "removeBackground"
  | "dewarp360"
  | "anamorphicLens"
  | "lut"
  | "opacity"
  | "mask";

export const videoEffectTypes: VideoEffectType[] = [
  "shape",
  "grayScale",
  "sepia",
  "whirlpool",
  "pinch",
  "removeBackground",
  "dewarp360",
  "anamorphicLens",
  "lut",
  "opacity",
  "mask",
];

export type SettingsVideoEffect = {
  id: string;
  enabled: boolean;
  type: VideoEffectType;
  removeBackground: { from: RgbColor; to: RgbColor };
  shape: {
    cornerRadius: number;
    borderWidth: number;
    borderColor: RgbColor;
    cropEnabled: boolean;
    cropX: number;
    cropY: number;
    cropWidth: number;
    cropHeight: number;
  };
  dewarp360: { pan: number; tilt: number; zoom: number };
  anamorphicLens: { scale: number };
  lut: { lut: string | null };
  opacity: { opacity: number };
  mask: {
    points: { x: number; y: number }[];
    inverted: boolean;
    tension: number;
    backgroundType: "Transparent" | "Solid" | "Checkerboard";
    backgroundColor: RgbColor;
    backgroundColor2: RgbColor;
  };
};

export type SettingsWidget = {
  name: string;
  id: string;
  type: SettingsWidgetType;
  text: SettingsWidgetText;
  browser: SettingsWidgetBrowser;
  crop: SettingsWidgetCrop;
  map: SettingsWidgetMap;
  scene: SettingsWidgetScene;
  qrCode: SettingsWidgetQrCode;
  alerts: SettingsWidgetAlerts;
  videoSource: SettingsWidgetVideoSource;
  scoreboard: SettingsWidgetScoreboard;
  vTuber: SettingsWidgetVTuber;
  pngTuber: SettingsWidgetPngTuber;
  snapshot: SettingsWidgetSnapshot;
  chat: SettingsWidgetChat;
  chatEmoteCombo: SettingsWidgetChatEmoteCombo;
  slideshow: SettingsWidgetSlideshow;
  wheelOfLuck: SettingsWidgetWheelOfLuck;
  bingoCard: SettingsWidgetBingoCard;
  pomodoroTimer: SettingsWidgetPomodoroTimer;
  enabled: boolean;
  effects: SettingsVideoEffect[];
};

export type MediaGalleryItem = { id: string; name: string };

export type AlertsMediaGallery = {
  bundledImages: MediaGalleryItem[];
  customImages: MediaGalleryItem[];
  bundledSounds: MediaGalleryItem[];
  customSounds: MediaGalleryItem[];
};

export type EditorOptions = {
  fonts: { family: string; styles: string[] }[];
  luts: { id: string; name: string }[];
  scoreboardPlayers: ScoreboardPlayer[];
  webRemote: { enabled: boolean; port: number; deviceName: string; addresses: string[] };
  workoutDevices: { id: string; name: string }[];
  streamDeckLayouts: { id: string; name: string }[];
  appleVoices: {
    language: string;
    voices: { name: string; identifier: string; language: string }[];
  }[];
  showAllSettings: boolean;
  isPhone: boolean;
  forceSceneSwitchTransition: boolean;
  workoutActive: boolean;
  inactiveVideoSourceSceneIds: string[];
};

export const sameId = (first: string | null | undefined, second: string | null | undefined) =>
  first !== null &&
  first !== undefined &&
  second !== null &&
  second !== undefined &&
  first.toUpperCase() === second.toUpperCase();

export const newUuid = () => crypto.randomUUID().toUpperCase();

const hash = (text: string, seed: number) => {
  let value = seed;
  for (let index = 0; index < text.length; index += 1) {
    value ^= text.charCodeAt(index);
    value = Math.imul(value, 16777619) >>> 0;
  }
  return value;
};

export const derivedId = (seed: string) => {
  const hex = [0x811c9dc5, 0x01000193, 0x9e3779b9, 0x7f4a7c15]
    .map((salt) => hash(seed, salt).toString(16).padStart(8, "0"))
    .join("")
    .toUpperCase();
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-A${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
};

const white = () => rgb(255, 255, 255);
const black = () => rgb(0, 0, 0);

const videoSourceFields = (seed: string, cameraPosition: CameraPosition): VideoSourceFields => ({
  cameraPosition,
  backCameraId: "",
  frontCameraId: "",
  rtmpCameraId: derivedId(`${seed}:rtmp`),
  srtlaCameraId: derivedId(`${seed}:srtla`),
  srtClientCameraId: derivedId(`${seed}:srtClient`),
  ristCameraId: derivedId(`${seed}:rist`),
  rtspCameraId: derivedId(`${seed}:rtsp`),
  whipCameraId: derivedId(`${seed}:whip`),
  whepCameraId: derivedId(`${seed}:whep`),
  mediaPlayerCameraId: derivedId(`${seed}:mediaPlayer`),
  externalCameraId: "",
  externalCameraName: "",
});

export const defaultSceneWidget = (
  widgetId: string,
  type: SettingsWidgetType,
  sport?: ScoreboardSport,
) => {
  const sceneWidget: SettingsSceneWidget = {
    id: newUuid(),
    widgetId,
    x: 0,
    y: 0,
    size: 100,
    alignment: "TopLeft",
    positioningLock: false,
  };
  switch (type) {
    case "Image":
    case "Slideshow":
      sceneWidget.size = 30;
      break;
    case "Map":
    case "QR code":
      sceneWidget.size = 23;
      break;
    case "Video source":
    case "VTuber":
    case "PNGTuber":
      sceneWidget.size = 28;
      sceneWidget.alignment = "BottomRight";
      break;
    case "Snapshot":
      sceneWidget.size = 40;
      sceneWidget.alignment = "TopRight";
      break;
    case "Chat":
      sceneWidget.alignment = "BottomLeft";
      break;
    case "Chat emote combo":
      sceneWidget.x = 2;
      sceneWidget.y = 25;
      sceneWidget.size = 10;
      break;
    case "Alerts":
      sceneWidget.x = 20;
      sceneWidget.y = 5;
      break;
    case "Scoreboard":
      sceneWidget.size = 18.52;
      sceneWidget.x = 0.78;
      sceneWidget.y = 1.388;
      if (sport === "golfFullScorecard") {
        sceneWidget.alignment = "BottomRight";
      }
      break;
    case "Wheel of luck":
      sceneWidget.alignment = "TopRight";
      sceneWidget.x = 1.3;
      sceneWidget.y = 31;
      break;
    case "Bingo card":
      sceneWidget.alignment = "TopRight";
      sceneWidget.x = 1.3;
      sceneWidget.y = 33;
      sceneWidget.size = 33;
      break;
    case "Pomodoro timer":
      sceneWidget.alignment = "TopRight";
      sceneWidget.x = 0.78;
      sceneWidget.y = 1.388;
      sceneWidget.size = 20;
      break;
    default:
      break;
  }
  return sceneWidget;
};

export const defaultScene = (id: string): SettingsScene => ({
  name: "",
  id,
  enabled: true,
  ...videoSourceFields(id, "None"),
  widgets: [],
  videoSourceRotation: 0,
  videoStabilizationMode: "Off",
  overrideVideoStabilizationMode: false,
  fillFrame: false,
  overrideMic: false,
  micId: "",
  quickSwitchGroup: null,
  mirror: false,
  backgroundColor: rgb(142, 142, 147, 0.6),
});

export const defaultVoice = (): SettingsVoice => ({
  type: "apple",
  apple: { voice: "" },
  ttsMonster: { name: "", voiceId: "" },
  elevenLabs: { name: "", voiceId: "" },
});

export const defaultAlert = (seed: string): SettingsAlert => ({
  id: derivedId(`${seed}:id`),
  enabled: true,
  mediaType: "gifAndSound",
  imageId: derivedId(`${seed}:image`),
  imageLoopCount: 1,
  soundId: derivedId(`${seed}:sound`),
  videoName: "",
  textColor: white(),
  accentColor: rgb(253, 251, 103),
  fontSize: 45,
  fontDesign: "Monospaced",
  fontWeight: "Bold",
  textToSpeechEnabled: true,
  textToSpeechDelay: 1.5,
  textToSpeechLanguageVoices: {},
  positionType: "Scene",
  facePosition: { x: 0.25, y: 0.25, width: 0.5, height: 0.5 },
});

export const defaultCheerBits = (seed: string): CheerBitsAlert => ({
  id: derivedId(`${seed}:id`),
  bits: 1,
  comparisonOperator: ">=",
  alert: defaultAlert(`${seed}:alert`),
});

export const defaultKickGifts = (seed: string): KickGiftsAlert => ({
  id: derivedId(`${seed}:id`),
  amount: 1,
  comparisonOperator: ">=",
  alert: defaultAlert(`${seed}:alert`),
});

export const defaultEffect = (id: string): SettingsVideoEffect => ({
  id,
  enabled: true,
  type: "shape",
  removeBackground: { from: rgb(220, 235, 92), to: rgb(82, 180, 203) },
  shape: {
    cornerRadius: 0.1,
    borderWidth: 0,
    borderColor: black(),
    cropEnabled: false,
    cropX: 0.25,
    cropY: 0,
    cropWidth: 0.5,
    cropHeight: 1,
  },
  dewarp360: { pan: 0, tilt: 0, zoom: 1 },
  anamorphicLens: { scale: 1.33 },
  lut: { lut: null },
  opacity: { opacity: 0.5 },
  mask: {
    points: [
      { x: 25, y: 50 },
      { x: 50, y: 75 },
      { x: 75, y: 50 },
      { x: 50, y: 25 },
    ],
    inverted: false,
    tension: 1 / 6,
    backgroundType: "Transparent",
    backgroundColor: black(),
    backgroundColor2: white(),
  },
});

export const defaultWheelOption = (id: string, text = ""): WheelOfLuckOption => ({
  id,
  text,
  weight: 1,
  function: "Unused",
  sceneId: null,
  widgetId: null,
  gimbalPresetId: null,
  gimbalMotion: { kapow: {} },
  macroId: null,
  streamDeckLayoutId: null,
});

const defaultClock = (): ScoreboardClock => ({ maximum: 45, direction: { up: {} } });

const golfPars = [4, 4, 3, 4, 5, 4, 3, 4, 4, 4, 4, 3, 5, 4, 4, 3, 4, 5];

export const defaultGolfPlayer = (id: string, name: string) => ({
  id,
  name,
  scores: Array.from({ length: 18 }, () => -1),
  color: white(),
});

export const defaultWidget = (id: string, type: SettingsWidgetType): SettingsWidget => ({
  name: "",
  id,
  type,
  text: {
    formatString: "{shortTime}",
    backgroundColor: rgb(0, 0, 0, 0.75),
    clearBackgroundColor: false,
    foregroundColor: white(),
    clearForegroundColor: false,
    fontSize: 30,
    fontFamily: null,
    fontStyle: "",
    fontDesign: "Default",
    fontWeight: "Regular",
    fontMonospacedDigits: false,
    alignment: "Leading",
    horizontalAlignment: "Leading",
    verticalAlignment: "Top",
    delay: 0,
    timers: [],
    stopwatches: [],
    needsWeather: false,
    needsGeography: false,
    needsSubtitles: false,
    subtitles: [],
    checkboxes: [],
    ratings: [],
    lapTimes: [],
    needsGForce: false,
    widthEnabled: false,
    width: 300,
    cornerRadius: 10,
  },
  browser: {
    url: "",
    width: 500,
    height: 500,
    mode: "periodicAudioAndVideo",
    fps: 5,
    styleSheet: "",
    moblinAccess: false,
    speechToText: false,
    localOnly: false,
  },
  crop: { sourceWidgetId: derivedId(`${id}:crop`), x: 0, y: 0, width: 200, height: 200 },
  map: { northUp: false, delay: 0, scale: 1000 },
  scene: { sceneId: derivedId(`${id}:scene`) },
  qrCode: { message: "" },
  alerts: {
    twitch: {
      follows: defaultAlert(`${id}:twitch:follows`),
      subscriptions: defaultAlert(`${id}:twitch:subscriptions`),
      raids: defaultAlert(`${id}:twitch:raids`),
      cheers: defaultAlert(`${id}:twitch:cheers`),
      cheerBits: [defaultCheerBits(`${id}:twitch:cheerBits`)],
    },
    kick: {
      subscriptions: defaultAlert(`${id}:kick:subscriptions`),
      giftedSubscriptions: defaultAlert(`${id}:kick:giftedSubscriptions`),
      hosts: defaultAlert(`${id}:kick:hosts`),
      rewards: defaultAlert(`${id}:kick:rewards`),
      kickGifts: [defaultKickGifts(`${id}:kick:kickGifts`)],
    },
    moblinCloud: { tips: defaultAlert(`${id}:moblinCloud:tips`) },
    chatBot: { commands: [] },
    speechToText: { strings: [] },
    needsSubtitles: false,
    ai: {
      baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
      apiKey: "",
      model: "gemini-3.5-flash-lite",
      role: "You are rude and gives insulting answers. Answer in a few sentences.",
    },
    aiEnabled: false,
    ttsMonster: { apiToken: "" },
  },
  videoSource: {
    cornerRadius: 0,
    ...videoSourceFields(`${id}:videoSource`, "None"),
    cropEnabled: false,
    cropX: 0.25,
    cropY: 0,
    cropWidth: 0.5,
    cropHeight: 1,
    rotation: 0,
    trackFaceEnabled: false,
    trackFaceZoom: 0.75,
    mirror: false,
    borderWidth: 0,
    borderColor: black(),
  },
  scoreboard: {
    type: "generic",
    textColor: white(),
    primaryBackgroundColor: rgb(11, 16, 172),
    secondaryBackgroundColor: rgb(0, 3, 91),
    padel: {
      type: "Double",
      homePlayer1: derivedId(`${id}:padel:home1`),
      homePlayer2: derivedId(`${id}:padel:home2`),
      awayPlayer1: derivedId(`${id}:padel:away1`),
      awayPlayer2: derivedId(`${id}:padel:away2`),
      score: [{ home: 0, away: 0 }],
    },
    golf: {
      eventName: "⛳ Masters 2026",
      numberOfHoles: 18,
      currentHole: 0,
      pars: [...golfPars],
      players: [
        defaultGolfPlayer(derivedId(`${id}:golf:player1`), "Player 1"),
        defaultGolfPlayer(derivedId(`${id}:golf:player2`), "Player 2"),
      ],
      playerColors: false,
      showPars: true,
    },
    generic: {
      home: "🇸🇪 Moblin",
      away: "🇸🇪 Moblin",
      title: "⚽️",
      period: "1",
      clock: defaultClock(),
    },
    modular: {
      home: { name: "🇸🇪 Moblin", textColor: white(), backgroundColor: rgb(11, 16, 172) },
      away: { name: "🇸🇪 Moblin", textColor: white(), backgroundColor: rgb(220, 38, 38) },
      title: "⚽️",
      period: "1",
      infoBoxText: "",
      clock: defaultClock(),
      layout: { stacked: {} },
      width: 350,
      rowHeight: 45,
      isBold: true,
      showTitle: false,
      showMoreStats: false,
      showGlobalStatsBlock: false,
      showClock: true,
    },
  },
  vTuber: {
    id: derivedId(`${id}:vTuber`),
    type: "VRM",
    ...videoSourceFields(`${id}:vTuber`, "None"),
    cameraPositionY: 1.37,
    cameraFieldOfView: 18,
    modelName: "",
    mirror: false,
    sensitivity: { mouth: 1, eyes: 1 },
    armsAngle: 72,
  },
  pngTuber: {
    id: derivedId(`${id}:pngTuber`),
    ...videoSourceFields(`${id}:pngTuber`, "None"),
    modelName: "",
    mirror: false,
    sensitivity: { mouth: 1, eyes: 1 },
  },
  snapshot: { id: derivedId(`${id}:snapshot`), showtime: 5 },
  chat: {
    id: derivedId(`${id}:chat`),
    fontSize: 19,
    fontFamily: null,
    fontStyle: "",
    usernameColor: rgb(255, 163, 0),
    messageColor: white(),
    backgroundColor: black(),
    backgroundColorEnabled: false,
    shadowColor: black(),
    shadowColorEnabled: true,
    boldUsername: true,
    boldMessage: true,
    badges: true,
    displayStyle: "internationalName",
    sharedChatIcons: false,
    height: 1,
    maximumNumberOfMessages: 5,
  },
  chatEmoteCombo: { minimumCombo: 3, resetAfter: 5 },
  slideshow: { id: derivedId(`${id}:slideshow`), slides: [] },
  wheelOfLuck: { advanced: false, onlyShowWhenSpinning: false, options: [] },
  bingoCard: { backgroundColor: rgb(0, 0, 0, 0.75), foregroundColor: white(), squares: [] },
  pomodoroTimer: {
    focusDuration: 30,
    breakDuration: 5,
    width: 1.6,
    focusName: "Focus",
    breakName: "Break",
    focusIcon: "sun.max",
    breakIcon: "cup.and.saucer",
    backgroundColor: rgb(0, 0, 0, 0.75),
    foregroundColor: white(),
    focusColor: rgb(122, 181, 255),
    breakColor: rgb(103, 208, 69),
    focusToBreakSoundId: null,
    breakToFocusSoundId: null,
    focusToBreakChatMessage: "",
    breakToFocusChatMessage: "",
  },
  enabled: true,
  effects: [],
});

export const newWidget = (type: SettingsWidgetType): SettingsWidget => {
  const widget = defaultWidget(newUuid(), type);
  widget.text.formatString = "";
  widget.chat.displayStyle = "internationalNameAndUsername";
  widget.wheelOfLuck.options = [t("Yes"), t("No"), t("Later"), t("Spin again")].map((text) =>
    defaultWheelOption(newUuid(), text),
  );
  return widget;
};

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const merged = <T>(defaults: T, raw: unknown): T => {
  if (!isPlainObject(defaults)) {
    return (raw === undefined ? defaults : raw) as T;
  }
  const result: Record<string, unknown> = { ...(isPlainObject(raw) ? raw : {}) };
  for (const [key, value] of Object.entries(defaults)) {
    const given = isPlainObject(raw) ? raw[key] : undefined;
    if (isPlainObject(value)) {
      result[key] = merged(value, isPlainObject(given) ? given : undefined);
    } else if (given === undefined) {
      result[key] = value;
    }
  }
  return result as T;
};

const mergedList = <T>(
  items: unknown,
  defaultsOf: (item: Record<string, unknown>, index: number) => T,
) =>
  Array.isArray(items)
    ? items.map((item, index) => merged(defaultsOf(isPlainObject(item) ? item : {}, index), item))
    : [];

const idOf = (item: Record<string, unknown>, seed: string) =>
  typeof item.id === "string" ? item.id : derivedId(seed);

export const normalizeScene = (raw: Record<string, unknown>): SettingsScene => {
  const id = idOf(raw, "scene");
  const scene = merged(defaultScene(id), raw);
  scene.widgets = mergedList(raw.widgets, (item, index) => ({
    id: idOf(item, `${id}:widget:${index}`),
    widgetId: "",
    x: 0,
    y: 0,
    size: 100,
    alignment: "TopLeft" as SettingsAlignment,
    positioningLock: false,
  }));
  if (!scene.enabled && typeof raw.enabled !== "boolean") {
    scene.enabled = true;
  }
  if (typeof raw.cameraPosition !== "string") {
    scene.cameraPosition = "Back";
  }
  return scene;
};

export const normalizeWidget = (raw: Record<string, unknown>): SettingsWidget => {
  const id = idOf(raw, "widget");
  const type = (typeof raw.type === "string" ? raw.type : "Text") as SettingsWidgetType;
  const widget = merged(defaultWidget(id, type), raw);
  widget.type = widgetTypes.includes(type) ? type : "Text";
  widget.effects = mergedList(raw.effects, (item, index) =>
    defaultEffect(idOf(item, `${id}:effect:${index}`)),
  );
  const text = isPlainObject(raw.text) ? raw.text : {};
  widget.text.timers = mergedList(text.timers, (item, index) => ({
    id: idOf(item, `${id}:timer:${index}`),
    delta: 5,
    endTime: 0,
  }));
  widget.text.stopwatches = mergedList(text.stopwatches, (item, index) => ({
    id: idOf(item, `${id}:stopwatch:${index}`),
    totalElapsed: 0,
    running: false,
  }));
  const alerts = isPlainObject(raw.alerts) ? raw.alerts : {};
  const twitch = isPlainObject(alerts.twitch) ? alerts.twitch : {};
  const kick = isPlainObject(alerts.kick) ? alerts.kick : {};
  const chatBot = isPlainObject(alerts.chatBot) ? alerts.chatBot : {};
  const speechToText = isPlainObject(alerts.speechToText) ? alerts.speechToText : {};
  if (Array.isArray(twitch.cheerBits)) {
    widget.alerts.twitch.cheerBits = mergedList(twitch.cheerBits, (item, index) =>
      defaultCheerBits(`${id}:cheerBits:${index}:${idOf(item, "")}`),
    );
  }
  if (Array.isArray(kick.kickGifts)) {
    widget.alerts.kick.kickGifts = mergedList(kick.kickGifts, (item, index) =>
      defaultKickGifts(`${id}:kickGifts:${index}:${idOf(item, "")}`),
    );
  }
  widget.alerts.chatBot.commands = mergedList(chatBot.commands, (item, index) => ({
    id: idOf(item, `${id}:command:${index}`),
    name: "myname",
    alert: defaultAlert(`${id}:command:${index}:alert`),
    imageType: "File" as const,
  }));
  widget.alerts.speechToText.strings = mergedList(speechToText.strings, (item, index) => ({
    id: idOf(item, `${id}:string:${index}`),
    string: "",
    alert: defaultAlert(`${id}:string:${index}:alert`),
  }));
  const slideshow = isPlainObject(raw.slideshow) ? raw.slideshow : {};
  widget.slideshow.slides = mergedList(slideshow.slides, (item, index) => ({
    id: idOf(item, `${id}:slide:${index}`),
    widgetId: null as string | null,
    time: 0,
  }));
  const wheel = isPlainObject(raw.wheelOfLuck) ? raw.wheelOfLuck : {};
  widget.wheelOfLuck.options = mergedList(wheel.options, (item, index) =>
    defaultWheelOption(idOf(item, `${id}:option:${index}`)),
  );
  const scoreboard = isPlainObject(raw.scoreboard) ? raw.scoreboard : {};
  const golf = isPlainObject(scoreboard.golf) ? scoreboard.golf : {};
  if (Array.isArray(golf.players)) {
    widget.scoreboard.golf.players = mergedList(golf.players, (item, index) =>
      defaultGolfPlayer(idOf(item, `${id}:golfPlayer:${index}`), "Player"),
    );
  }
  while (widget.scoreboard.golf.pars.length < 18) {
    widget.scoreboard.golf.pars.push(4);
  }
  return widget;
};

export const makeUniqueName = (base: string, names: string[]) => {
  if (!names.includes(base)) {
    return base;
  }
  let number = 1;
  while (names.includes(`${base} ${number}`)) {
    number += 1;
  }
  return `${base} ${number}`;
};
