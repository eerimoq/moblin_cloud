import {
  isHorizontalCenter,
  isHorizontalOrVerticalCenter,
  isLeft,
  isTop,
  isVerticalCenter,
  mirrorPositionHorizontally,
  mirrorPositionVertically,
  type SettingsAlignment,
} from "./moblinSettings";
import { layoutPosition, type Layout, type Point, type Rect, type Size } from "./widgetOutlines";

export const clamped = (value: number, minimum: number, maximum: number) =>
  Math.min(maximum, Math.max(minimum, value));

export const distance = (first: Point, second: Point) =>
  Math.hypot(first.x - second.x, first.y - second.y);

export const rectMaxX = (rect: Rect) => rect.x + rect.width;
export const rectMaxY = (rect: Rect) => rect.y + rect.height;
export const rectMidX = (rect: Rect) => rect.x + rect.width / 2;
export const rectMidY = (rect: Rect) => rect.y + rect.height / 2;

export const widgetOutlineViewRect = (rect: Rect, canvas: Size, size: Size): Rect => {
  const scale = canvas.width / size.width;
  return {
    x: rect.x / scale,
    y: rect.y / scale,
    width: rect.width / scale,
    height: rect.height / scale,
  };
};

export type Snap = { position: number; guide?: number };

export const sceneEditorSnapAxis = (
  position: number,
  length: number,
  total: number,
  threshold: number,
  near: boolean,
  targets: number[],
): Snap => {
  const [low, high] = near ? [0, total] : [-length, total - length];
  const clampedPosition = clamped(position, low, high);
  const candidates: Snap[] = [
    { position: 0 },
    { position: total - length },
    { position: (total - length) / 2, guide: total / 2 },
  ];
  for (const target of targets) {
    for (const anchor of [0, length / 2, length]) {
      candidates.push({ position: target - anchor, guide: target });
    }
  }
  let closest: Snap | undefined;
  for (const candidate of candidates) {
    if (candidate.position < low || candidate.position > high) {
      continue;
    }
    if (
      !closest ||
      Math.abs(candidate.position - clampedPosition) < Math.abs(closest.position - clampedPosition)
    ) {
      closest = candidate;
    }
  }
  if (closest && Math.abs(closest.position - clampedPosition) < threshold) {
    return closest;
  }
  return { position: clampedPosition };
};

export type ResizeSnap = { factor: number; distance: number; guide: number };

export const sceneEditorSnapResizeAxis = (
  edges: number[],
  fixed: number,
  factor: number,
  factors: [number, number],
  threshold: number,
  targets: number[],
): ResizeSnap | undefined => {
  let closest: ResizeSnap | undefined;
  for (const edge of edges) {
    if (Math.abs(edge - fixed) <= 1) {
      continue;
    }
    for (const target of targets) {
      const snapFactor = (target - fixed) / (edge - fixed);
      const snapDistance = Math.abs(snapFactor - factor);
      if (
        snapDistance < threshold &&
        snapFactor >= factors[0] &&
        snapFactor <= factors[1] &&
        snapDistance < (closest?.distance ?? Infinity) - 1e-9
      ) {
        closest = { factor: snapFactor, distance: snapDistance, guide: target };
      }
    }
  }
  return closest;
};

export const sceneEditorHandlePoint = (rect: Rect, alignment: SettingsAlignment): Point => ({
  x: mirrorPositionHorizontally(alignment) ? rect.x : rectMaxX(rect),
  y: mirrorPositionVertically(alignment) ? rect.y : rectMaxY(rect),
});

export const sceneEditorFixedPoint = (rect: Rect, alignment: SettingsAlignment): Point => ({
  x: isHorizontalCenter(alignment) ? rectMidX(rect) : isLeft(alignment) ? rect.x : rectMaxX(rect),
  y: isVerticalCenter(alignment) ? rectMidY(rect) : isTop(alignment) ? rect.y : rectMaxY(rect),
});

export const sceneEditorResizedRect = (rect: Rect, fixed: Point, factor: number): Rect => ({
  x: fixed.x + (rect.x - fixed.x) * factor,
  y: fixed.y + (rect.y - fixed.y) * factor,
  width: rect.width * factor,
  height: rect.height * factor,
});

const layoutOffsetFromPosition = (position: number, size: number, total: number, near: boolean) =>
  near ? (position * 100) / total : ((total - position - size) * 100) / total;

export const sceneEditorLayout = <T extends Layout>(layout: T, rect: Rect, canvas: Size): T => {
  const result = { ...layout };
  if (!isHorizontalCenter(layout.alignment)) {
    result.x = layoutOffsetFromPosition(rect.x, rect.width, canvas.width, isLeft(layout.alignment));
  }
  if (!isVerticalCenter(layout.alignment)) {
    result.y = layoutOffsetFromPosition(
      rect.y,
      rect.height,
      canvas.height,
      isTop(layout.alignment),
    );
  }
  return result;
};

export const sceneEditorAlignedLayout = <T extends Layout>(
  layout: T,
  alignment: SettingsAlignment,
  size: Size,
  canvas: Size,
): T => {
  const position = layoutPosition(layout, size, canvas);
  const rect = { x: position.x, y: position.y, width: size.width, height: size.height };
  const result = { ...layout, alignment };
  if (isHorizontalOrVerticalCenter(alignment)) {
    result.positioningLock = false;
  }
  return sceneEditorLayout(result, rect, canvas);
};

export const sceneEditorTapSelection = (
  hitIds: string[],
  selectedId: string | null,
  sameLocation: boolean,
): string | null => {
  const index = selectedId === null ? -1 : hitIds.indexOf(selectedId);
  if (index < 0) {
    return hitIds[0] ?? null;
  }
  if (sameLocation) {
    return index + 1 < hitIds.length ? hitIds[index + 1]! : null;
  }
  return index === 0 ? null : (hitIds[0] ?? null);
};

export const movesDiagonally = (layout: Layout) =>
  layout.positioningLock && !isHorizontalOrVerticalCenter(layout.alignment);

export const diagonalDirection = (alignment: SettingsAlignment): Size => ({
  width: isLeft(alignment) ? 1 : -1,
  height: isTop(alignment) ? 1 : -1,
});

export const diagonalPosition = (
  alignment: SettingsAlignment,
  size: Size,
  position: Point,
  threshold: number,
  canvas: Size,
): Point => {
  const direction = diagonalDirection(alignment);
  const start = {
    x: isLeft(alignment) ? 0 : canvas.width - size.width,
    y: isTop(alignment) ? 0 : canvas.height - size.height,
  };
  const maximum = Math.min(canvas.width, canvas.height);
  let offset =
    ((position.x - start.x) * direction.width + (position.y - start.y) * direction.height) / 2;
  offset = clamped(offset, 0, maximum);
  if (offset < threshold) {
    offset = 0;
  }
  return { x: start.x + direction.width * offset, y: start.y + direction.height * offset };
};

export const hitRect = (rect: Rect, sized: boolean, alignment: SettingsAlignment): Rect => {
  if (sized) {
    const dx = Math.min(0, (rect.width - 44) / 2);
    const dy = Math.min(0, (rect.height - 44) / 2);
    return {
      x: rect.x + dx,
      y: rect.y + dy,
      width: rect.width - 2 * dx,
      height: rect.height - 2 * dy,
    };
  }
  const width = 120;
  const height = 60;
  const x = isHorizontalCenter(alignment)
    ? rect.x - width / 2
    : isLeft(alignment)
      ? rect.x - 22
      : rect.x - width + 22;
  const y = isVerticalCenter(alignment)
    ? rect.y - height / 2
    : isTop(alignment)
      ? rect.y - 22
      : rect.y - height + 22;
  return { x, y, width, height };
};

export const rectContains = (rect: Rect, point: Point) =>
  point.x >= rect.x && point.x <= rectMaxX(rect) && point.y >= rect.y && point.y <= rectMaxY(rect);
