import {
  isHorizontalCenter,
  isLeft,
  isTop,
  isVerticalCenter,
  hasPosition,
  sameId,
  type SettingsAlignment,
  type SettingsScene,
  type SettingsSceneWidget,
  type SettingsWidget,
} from "./moblinSettings";
import { SceneResolver, firstWidgets, resolvedEnabled } from "./sceneResolver";

export type Size = { width: number; height: number };
export type Point = { x: number; y: number };
export type Rect = { x: number; y: number; width: number; height: number };

export type Layout = {
  x: number;
  y: number;
  size: number;
  alignment: SettingsAlignment;
  positioningLock: boolean;
};

export type WidgetOutline = {
  id: string;
  sceneWidget: SettingsSceneWidget;
  widget: SettingsWidget;
  rect: Rect;
  alignment: SettingsAlignment;
  sized: boolean;
  inScene: boolean;
  enabled: boolean;
};

export const toPixels = (percentage: number, total: number) => (percentage * total) / 100;

export const layoutScale = (layout: Layout, size: Size, canvas: Size) =>
  Math.min(
    toPixels(layout.size, canvas.width) / size.width,
    toPixels(layout.size, canvas.height) / size.height,
  );

const layoutOffset = (
  offset: number,
  size: number,
  total: number,
  center: boolean,
  near: boolean,
) => {
  if (center) {
    return (total - size) / 2;
  }
  return near ? toPixels(offset, total) : total - toPixels(offset, total) - size;
};

export const layoutPosition = (layout: Layout, size: Size, canvas: Size): Point => ({
  x: layoutOffset(
    layout.x,
    size.width,
    canvas.width,
    isHorizontalCenter(layout.alignment),
    isLeft(layout.alignment),
  ),
  y: layoutOffset(
    layout.y,
    size.height,
    canvas.height,
    isVerticalCenter(layout.alignment),
    isTop(layout.alignment),
  ),
});

const fitInLayout = (size: Size | null, layout: Layout, canvas: Size): Size | null => {
  if (!size || size.width <= 0 || size.height <= 0) {
    return null;
  }
  const scale = layoutScale(layout, size, canvas);
  return { width: size.width * scale, height: size.height * scale };
};

export const widgetOutlineSize = (
  widget: SettingsWidget,
  layout: Layout,
  canvas: Size,
): Size | null => {
  const box = {
    width: toPixels(layout.size, canvas.width),
    height: toPixels(layout.size, canvas.height),
  };
  const side = toPixels(layout.size, Math.min(canvas.width, canvas.height));
  switch (widget.type) {
    case "Map":
    case "QR code":
      return { width: side, height: side };
    case "Bingo card":
      return { width: side + 10, height: side + 10 };
    case "Browser":
      return (
        fitInLayout(
          { width: widget.browser.width, height: widget.browser.height },
          layout,
          canvas,
        ) ?? box
      );
    case "Image":
      return box;
    case "Chat emote combo":
      return { width: 2 * side, height: side };
    case "Pomodoro timer":
      return { width: widget.pomodoroTimer.width * side, height: 0.64 * side };
    case "Wheel of luck": {
      const wheelSide = (400 * canvas.width) / 1920;
      return { width: wheelSide + (0.3 * wheelSide) / 12, height: wheelSide };
    }
    case "Chat":
      return { width: 20 * widget.chat.fontSize, height: widget.chat.height * canvas.height };
    case "Video source":
    case "Snapshot":
    case "Slideshow":
    case "VTuber":
    case "PNGTuber":
      return box;
    default:
      return null;
  }
};

export const widgetOutlines = (
  scene: SettingsScene | undefined,
  resolver: SceneResolver,
  canvas: Size,
): WidgetOutline[] => {
  if (!scene) {
    return [];
  }
  const outlines: WidgetOutline[] = [];
  const resolved = resolver.resolve(scene, true);
  for (const drawn of firstWidgets(resolved)) {
    if (drawn.widget.type !== "Scene" || !scene.widgets.includes(drawn.sceneWidget)) {
      continue;
    }
    outlines.push({
      id: drawn.sceneWidget.id,
      sceneWidget: drawn.sceneWidget,
      widget: drawn.widget,
      rect: { x: 0, y: 0, width: canvas.width, height: canvas.height },
      alignment: drawn.sceneWidget.alignment,
      sized: true,
      inScene: true,
      enabled: drawn.widget.enabled,
    });
  }
  for (const drawn of firstWidgets(resolved)) {
    const widget = drawn.widget;
    if (!hasPosition(widget.type)) {
      continue;
    }
    const layout = drawn.sceneWidget;
    const size = widgetOutlineSize(widget, layout, canvas);
    const position = layoutPosition(layout, size ?? { width: 0, height: 0 }, canvas);
    outlines.push({
      id: drawn.sceneWidget.id,
      sceneWidget: drawn.sceneWidget,
      widget,
      rect: { x: position.x, y: position.y, width: size?.width ?? 0, height: size?.height ?? 0 },
      alignment: layout.alignment,
      sized: size !== null,
      inScene: sameId(drawn.scene.id, scene.id),
      enabled: resolvedEnabled(drawn),
    });
  }
  return outlines;
};
