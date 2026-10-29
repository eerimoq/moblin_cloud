import { t } from "../i18n";

export type Alignment =
  | "TopLeft"
  | "TopCenter"
  | "TopRight"
  | "LeftCenter"
  | "Center"
  | "RightCenter"
  | "BottomLeft"
  | "BottomCenter"
  | "BottomRight";

export type SceneWidget = {
  id: string;
  widgetId: string;
  x: number;
  y: number;
  size: number;
  alignment: Alignment;
  positioningLock?: boolean;
  [key: string]: unknown;
};

export type Scene = {
  id: string;
  name: string;
  enabled: boolean;
  cameraPosition?: string;
  widgets: SceneWidget[];
  [key: string]: unknown;
};

export type WidgetType =
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

export type Widget = {
  id: string;
  name: string;
  type: WidgetType;
  enabled: boolean;
  text?: { formatString?: string; fontSize?: number; [key: string]: unknown };
  browser?: { url?: string; width?: number; height?: number; [key: string]: unknown };
  qrCode?: { message?: string; [key: string]: unknown };
  [key: string]: unknown;
};

export type Camera = { id: string; name: string; position: string };

export type SceneConfig = { scenes: Scene[]; widgets: Widget[]; cameras: Camera[] };

export const alignments: Alignment[] = [
  "TopLeft",
  "TopCenter",
  "TopRight",
  "LeftCenter",
  "Center",
  "RightCenter",
  "BottomLeft",
  "BottomCenter",
  "BottomRight",
];

export const alignmentNames = (): Record<Alignment, string> => ({
  TopLeft: t("Top left"),
  TopCenter: t("Top center"),
  TopRight: t("Top right"),
  LeftCenter: t("Left center"),
  Center: t("Center"),
  RightCenter: t("Right center"),
  BottomLeft: t("Bottom left"),
  BottomCenter: t("Bottom center"),
  BottomRight: t("Bottom right"),
});

export const widgetTypeNames = (): Record<WidgetType, string> => ({
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
});

export const creatableWidgetTypes: WidgetType[] = [
  "Text",
  "Browser",
  "Image",
  "QR code",
  "Map",
  "Chat",
  "Alerts",
  "Wheel of luck",
  "Bingo card",
  "Pomodoro timer",
];

const unplaced: WidgetType[] = ["Scene"];

const unsized: WidgetType[] = ["Scene", "Text", "Alerts", "Chat", "Wheel of luck"];

export const hasPosition = (type: WidgetType) => !unplaced.includes(type);

export const hasSize = (type: WidgetType) => !unsized.includes(type);

const square: WidgetType[] = [
  "QR code",
  "Map",
  "Chat emote combo",
  "Bingo card",
  "Pomodoro timer",
  "Scoreboard",
];

const placeholders: Partial<Record<WidgetType, [number, number]>> = {
  Text: [0.18, 0.07],
  Alerts: [0.3, 0.3],
  Chat: [0.3, 0.45],
  "Wheel of luck": [0.22, 0.4],
};

export const boxSize = (widget: Widget | undefined, layout: SceneWidget): [number, number] => {
  const type = widget?.type ?? "Image";
  const placeholder = placeholders[type];
  if (placeholder) {
    return placeholder;
  }
  const size = Math.max(1, Math.min(100, layout.size)) / 100;
  if (type === "Browser" && widget?.browser?.width && widget.browser.height) {
    const scale = Math.min((size * 16) / widget.browser.width, (size * 9) / widget.browser.height);
    return [(widget.browser.width * scale) / 16, (widget.browser.height * scale) / 9];
  }
  if (square.includes(type)) {
    return [(size * 9) / 16, size];
  }
  return [size, size];
};

const isLeft = (alignment: Alignment) => alignment.endsWith("Left") || alignment === "LeftCenter";
const isTop = (alignment: Alignment) => alignment.startsWith("Top");
const isHorizontalCenter = (alignment: Alignment) =>
  alignment === "Center" || alignment === "TopCenter" || alignment === "BottomCenter";
const isVerticalCenter = (alignment: Alignment) =>
  alignment === "Center" || alignment === "LeftCenter" || alignment === "RightCenter";

const offset = (value: number, size: number, center: boolean, near: boolean) =>
  center ? (1 - size) / 2 : near ? value / 100 : 1 - value / 100 - size;

export const boxPosition = (layout: SceneWidget, [width, height]: [number, number]) => [
  offset(layout.x, width, isHorizontalCenter(layout.alignment), isLeft(layout.alignment)),
  offset(layout.y, height, isVerticalCenter(layout.alignment), isTop(layout.alignment)),
];

const rounded = (value: number) => Math.round(Math.max(0, Math.min(100, value)) * 10) / 10;

export const movedTo = (
  layout: SceneWidget,
  [width, height]: [number, number],
  [left, top]: [number, number],
): SceneWidget => ({
  ...layout,
  x: isHorizontalCenter(layout.alignment)
    ? layout.x
    : rounded(isLeft(layout.alignment) ? left * 100 : (1 - left - width) * 100),
  y: isVerticalCenter(layout.alignment)
    ? layout.y
    : rounded(isTop(layout.alignment) ? top * 100 : (1 - top - height) * 100),
});

const growth = (alignment: Alignment): [number, number] => [
  isHorizontalCenter(alignment) ? 2 : isLeft(alignment) ? 1 : -1,
  isVerticalCenter(alignment) ? 2 : isTop(alignment) ? 1 : -1,
];

export const resizeCorner = (alignment: Alignment) => {
  const [x, y] = growth(alignment);
  return { right: x > 0, bottom: y > 0 };
};

export const resizedBy = (
  layout: SceneWidget,
  [width, height]: [number, number],
  [dx, dy]: [number, number],
): SceneWidget => {
  const [x, y] = growth(layout.alignment);
  const factor = ((width + x * dx) / width + (height + y * dy) / height) / 2;
  return { ...layout, size: Math.max(1, rounded(layout.size * factor)) };
};

export const newId = () => crypto.randomUUID().toUpperCase();

export const newSceneWidget = (widget: Widget): SceneWidget => {
  const layout: SceneWidget = {
    id: newId(),
    widgetId: widget.id,
    x: 0,
    y: 0,
    size: 100,
    alignment: "TopLeft",
    positioningLock: false,
  };
  const defaults: Partial<Record<WidgetType, Partial<SceneWidget>>> = {
    Image: { size: 30 },
    Slideshow: { size: 30 },
    Map: { size: 23 },
    "QR code": { size: 23 },
    "Video source": { size: 28, alignment: "BottomRight" },
    VTuber: { size: 28, alignment: "BottomRight" },
    PNGTuber: { size: 28, alignment: "BottomRight" },
    Snapshot: { size: 40, alignment: "TopRight" },
    Chat: { alignment: "BottomLeft" },
    "Chat emote combo": { x: 2, y: 25, size: 10 },
    Alerts: { x: 20, y: 5 },
    Scoreboard: { size: 18.52, x: 0.78, y: 1.388 },
    "Wheel of luck": { alignment: "TopRight", x: 1.3, y: 31 },
    "Bingo card": { alignment: "TopRight", x: 1.3, y: 33, size: 33 },
    "Pomodoro timer": { alignment: "TopRight", x: 0.78, y: 1.388, size: 20 },
  };
  return { ...layout, ...defaults[widget.type] };
};

const cameraIdKeys: Record<string, string> = {
  Back: "backCameraId",
  Front: "frontCameraId",
  RTMP: "rtmpCameraId",
  "SRT(LA)": "srtlaCameraId",
  "SRT client": "srtClientCameraId",
  RIST: "ristCameraId",
  RTSP: "rtspCameraId",
  WHIP: "whipCameraId",
  WHEP: "whepCameraId",
  "Media player": "mediaPlayerCameraId",
  External: "externalCameraId",
};

export const cameraOf = (scene: Scene, cameras: Camera[]) =>
  cameras.find((camera) => {
    if (camera.position !== scene.cameraPosition) {
      return false;
    }
    const key = cameraIdKeys[camera.position];
    return key === undefined || String(scene[key]).toUpperCase() === camera.id.toUpperCase();
  });

export const withCamera = (scene: Scene, camera: Camera): Scene => {
  const key = cameraIdKeys[camera.position];
  return {
    ...scene,
    cameraPosition: camera.position,
    ...(key === undefined ? {} : { [key]: camera.id }),
    ...(camera.position === "External" ? { externalCameraName: camera.name } : {}),
  };
};

export const moved = <T>(items: T[], index: number, by: number) => {
  const target = index + by;
  if (target < 0 || target >= items.length) {
    return items;
  }
  const result = [...items];
  [result[index], result[target]] = [result[target]!, result[index]!];
  return result;
};

export const sameId = (a: string, b: string) => a.toUpperCase() === b.toUpperCase();
