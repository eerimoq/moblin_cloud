import type { IconName } from "../components/Icon";
import { BackendError } from "./backend";
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

export const hasAlignment = hasPosition;

export { isHorizontalCenter, isLeft, isVerticalCenter };

export const mirrorsHorizontally = (alignment: Alignment) =>
  alignment === "TopRight" || alignment === "BottomRight" || alignment === "RightCenter";

export const mirrorsVertically = (alignment: Alignment) =>
  alignment === "BottomLeft" || alignment === "BottomRight" || alignment === "BottomCenter";

export const canvasWidth = 1920;

export const canvasHeight = 1080;

export const widgetIcons: Record<WidgetType, IconName> = {
  Image: "image",
  Browser: "globe",
  Text: "text",
  Crop: "crop",
  Map: "map",
  Snapshot: "aperture",
  Chat: "chat",
  "Chat emote combo": "sparkles",
  Scene: "scenes",
  Slideshow: "film",
  "QR code": "qr-code",
  Alerts: "megaphone",
  "Video source": "video",
  Scoreboard: "scoreboard",
  VTuber: "user",
  PNGTuber: "user",
  "Wheel of luck": "flame",
  "Bingo card": "grid",
  "Pomodoro timer": "timer",
};

export const nestedSceneId = (widget: Widget) =>
  (widget.scene as { sceneId?: string } | undefined)?.sceneId;

export type WidgetInScene = { scene: Scene; layout: SceneWidget; widget: Widget };

export const sceneWidgetsOf = (scene: Scene, config: SceneConfig): WidgetInScene[] => {
  const added = new Set<string>();
  const collect = (scene: Scene): WidgetInScene[] =>
    scene.widgets.flatMap((layout) => {
      const widget = config.widgets.find((widget) => sameId(widget.id, layout.widgetId));
      if (!widget) {
        return [];
      }
      const nested = widget.type === "Scene" ? nestedSceneId(widget) : undefined;
      if (widget.type !== "Scene") {
        return [{ scene, layout, widget }];
      }
      if (!nested || added.has(nested.toUpperCase())) {
        return [];
      }
      added.add(nested.toUpperCase());
      const inner = config.scenes.find((candidate) => sameId(candidate.id, nested));
      return [{ scene, layout, widget }, ...(inner ? collect(inner) : [])];
    });
  const found = new Set<string>();
  return collect(scene).filter((item) => {
    const id = item.widget.id.toUpperCase();
    if (found.has(id)) {
      return false;
    }
    found.add(id);
    return true;
  });
};

export type WidgetOutline = {
  key: string;
  widget: Widget;
  layout: SceneWidget;
  rect: [number, number, number, number];
  sized: boolean;
  inScene: boolean;
};

const outlineSize = (widget: Widget, layout: SceneWidget): [number, number] | null => {
  const box: [number, number] = [
    (layout.size * canvasWidth) / 100,
    (layout.size * canvasHeight) / 100,
  ];
  const side = (layout.size * canvasHeight) / 100;
  const fit = (width: number, height: number): [number, number] | null => {
    if (width <= 0 || height <= 0) {
      return null;
    }
    const scale = Math.min(box[0] / width, box[1] / height);
    return [width * scale, height * scale];
  };
  switch (widget.type) {
    case "Map":
    case "QR code":
      return [side, side];
    case "Bingo card":
      return [side + 10, side + 10];
    case "Browser":
      return fit(widget.browser?.width ?? 0, widget.browser?.height ?? 0) ?? box;
    case "Chat emote combo":
      return [2 * side, side];
    case "Pomodoro timer":
      return [
        ((widget.pomodoroTimer as { width?: number } | undefined)?.width ?? 1.6) * side,
        0.64 * side,
      ];
    case "Wheel of luck": {
      const wheel = (400 * canvasWidth) / 1920;
      return [wheel + (0.3 * wheel) / 12, wheel];
    }
    case "Chat": {
      const chat = widget.chat as { fontSize?: number; height?: number } | undefined;
      return [20 * (chat?.fontSize ?? 19), (chat?.height ?? 1) * canvasHeight];
    }
    case "Text":
    case "Alerts":
    case "Scoreboard":
    case "Crop":
    case "Scene":
      return null;
    default:
      return box;
  }
};

const pixelOffset = (offset: number, size: number, total: number, center: boolean, near: boolean) =>
  center
    ? (total - size) / 2
    : near
      ? (offset * total) / 100
      : total - (offset * total) / 100 - size;

export const outlinesOf = (sceneId: string, config: SceneConfig): WidgetOutline[] => {
  const scene = config.scenes.find((candidate) => sameId(candidate.id, sceneId));
  if (!scene) {
    return [];
  }
  const outlines: WidgetOutline[] = [];
  const added: Scene[] = [];
  const draw = (current: Scene) => {
    if (added.includes(current)) {
      return;
    }
    added.push(current);
    for (const layout of current.widgets) {
      const widget = config.widgets.find((candidate) => sameId(candidate.id, layout.widgetId));
      if (!widget?.enabled) {
        continue;
      }
      if (widget.type === "Scene") {
        const nested = nestedSceneId(widget);
        const inner = nested && config.scenes.find((candidate) => sameId(candidate.id, nested));
        if (inner) {
          draw(inner);
        }
      } else if (
        !outlines.some((outline) => sameId(outline.widget.id, widget.id)) &&
        hasPosition(widget.type)
      ) {
        const size = outlineSize(widget, layout);
        const [width, height] = size ?? [0, 0];
        outlines.push({
          key: `${current.id}:${layout.id}`,
          widget,
          layout,
          rect: [
            pixelOffset(
              layout.x,
              width,
              canvasWidth,
              isHorizontalCenter(layout.alignment),
              isLeft(layout.alignment),
            ),
            pixelOffset(
              layout.y,
              height,
              canvasHeight,
              isVerticalCenter(layout.alignment),
              isTop(layout.alignment),
            ),
            width,
            height,
          ],
          sized: size !== null,
          inScene: current === scene,
        });
      }
    }
  };
  draw(scene);
  return outlines;
};

export const uniqueName = (name: string, existing: { name: string }[]) => {
  const names = existing.map((item) => item.name);
  if (!names.includes(name)) {
    return name;
  }
  let number = 1;
  while (names.includes(`${name} ${number}`)) {
    number += 1;
  }
  return `${name} ${number}`;
};

export const clonedScene = (scene: Scene, name: string): Scene => ({
  ...JSON.parse(JSON.stringify(scene)),
  id: newId(),
  name,
  widgets: scene.widgets.map((layout) => ({ ...layout, id: newId() })),
});

export const widgetTypeDescriptions = (): Partial<Record<WidgetType, string>> => ({
  Text: t("A text widget shows text, weather, clock and much more."),
  Browser: t("A browser widget shows a webpage."),
  Image: t("An image widget shows an image."),
  Alerts: t("An alerts widget shows various alerts (subscriptions, raids, ...)."),
  Map: t("A map widget shows a map with your location."),
  Chat: t("A chat widget shows your chat."),
  "QR code": t("A QR code widget shows a QR code of any text."),
  "Wheel of luck": t("A wheel of luck widget shows a wheel of luck that you can spin."),
  "Bingo card": t("A bingo card widget shows an interactive bingo card."),
  "Pomodoro timer": t("A Pomodoro timer widget shows a focus and break timer."),
});

export const sceneErrorText = (error: unknown) =>
  error instanceof BackendError && [403, 409, 502].includes(error.status) && error.message
    ? error.message
    : error instanceof BackendError && error.status === 504
      ? t("Moblin did not answer. Editing scenes needs a recent version of Moblin.")
      : t("Something went wrong. Try again.");
