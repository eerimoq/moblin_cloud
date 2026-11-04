import { describe, expect, test } from "vitest";
import type { SettingsAlignment } from "./moblinSettings";
import { isLeft, isTop } from "./moblinSettings";
import {
  sceneEditorAlignedLayout,
  sceneEditorFixedPoint,
  sceneEditorHandlePoint,
  sceneEditorLayout,
  sceneEditorResizedRect,
  sceneEditorSnapAxis,
  sceneEditorTapSelection,
} from "./sceneEditorGeometry";
import { layoutPosition, type Layout, type Point, type Rect, type Size } from "./widgetOutlines";

const canvasSize = { width: 1920, height: 1080 };

const corners: SettingsAlignment[] = ["TopLeft", "TopRight", "BottomLeft", "BottomRight"];

const allAlignments: SettingsAlignment[] = [
  ...corners,
  "TopCenter",
  "BottomCenter",
  "LeftCenter",
  "RightCenter",
  "Center",
];

const makeLayout = (alignment: SettingsAlignment): Layout => ({
  x: 0,
  y: 0,
  size: 100,
  alignment,
  positioningLock: false,
});

const sizeOf = (rect: Rect): Size => ({ width: rect.width, height: rect.height });

const expectPoint = (actual: Point, expected: Point) => {
  expect(actual.x).toBeCloseTo(expected.x, 3);
  expect(actual.y).toBeCloseTo(expected.y, 3);
};

const expectPercentage = (value: number) => {
  expect(value).toBeGreaterThanOrEqual(0);
  expect(value).toBeLessThanOrEqual(100);
};

describe("SceneEditorGeometrySuite", () => {
  test.each(corners)("layoutPositionMatchesRect %s", (alignment) => {
    const rects = [
      { x: 500, y: 300, width: 400, height: 300 },
      { x: 0, y: 780, width: 400, height: 300 },
      { x: 1520, y: 0, width: 400, height: 300 },
    ];
    for (const rect of rects) {
      const layout = sceneEditorLayout(makeLayout(alignment), rect, canvasSize);
      expect(layout.alignment).toBe(alignment);
      expectPoint(layoutPosition(layout, sizeOf(rect), canvasSize), rect);
      expectPercentage(layout.x);
      expectPercentage(layout.y);
    }
  });

  test("moveKeepsAlignment", () => {
    const rect = { x: 1500, y: 780, width: 400, height: 300 };
    const layout = sceneEditorLayout(makeLayout("TopLeft"), rect, canvasSize);
    expect(layout.alignment).toBe("TopLeft");
    expectPoint(layoutPosition(layout, sizeOf(rect), canvasSize), rect);
  });

  test("moveKeepsCenteredAxis", () => {
    let layout = { ...makeLayout("TopCenter"), x: 7 };
    layout = sceneEditorLayout(layout, { x: 100, y: 200, width: 400, height: 300 }, canvasSize);
    expect(layout.alignment).toBe("TopCenter");
    expect(layout.x).toBe(7);
    expectPoint(layoutPosition(layout, { width: 400, height: 300 }, canvasSize), {
      x: 760,
      y: 200,
    });
  });

  test.each(allAlignments)("alignCenteredWidgetKeepsPosition %s", (alignment) => {
    const rect = { x: 760, y: 390, width: 400, height: 300 };
    const layout = sceneEditorLayout(makeLayout("TopLeft"), rect, canvasSize);
    const aligned = sceneEditorAlignedLayout(layout, alignment, sizeOf(rect), canvasSize);
    expect(aligned.alignment).toBe(alignment);
    expect(aligned.size).toBe(layout.size);
    expectPoint(layoutPosition(aligned, sizeOf(rect), canvasSize), rect);
  });

  test.each(corners)("alignCornerWidgetKeepsPosition %s", (alignment) => {
    for (const size of [
      { width: 400, height: 300 },
      { width: 0, height: 0 },
    ]) {
      const rect = { x: 100, y: 150, ...size };
      const layout = sceneEditorLayout(makeLayout("BottomRight"), rect, canvasSize);
      const aligned = sceneEditorAlignedLayout(layout, alignment, size, canvasSize);
      expectPoint(layoutPosition(aligned, size, canvasSize), rect);
    }
  });

  test("alignCenteredClearsPositioningLock", () => {
    const layout = { ...makeLayout("TopLeft"), positioningLock: true };
    const zero = { width: 0, height: 0 };
    expect(sceneEditorAlignedLayout(layout, "LeftCenter", zero, canvasSize).positioningLock).toBe(
      false,
    );
    expect(sceneEditorAlignedLayout(layout, "BottomRight", zero, canvasSize).positioningLock).toBe(
      true,
    );
  });

  test("moveWithPositioningLockKeepsCorner", () => {
    let layout = { ...makeLayout("TopLeft"), positioningLock: true };
    const rect = { x: 1500, y: 780, width: 400, height: 300 };
    layout = sceneEditorLayout(layout, rect, canvasSize);
    expect(layout.alignment).toBe("TopLeft");
    expect(layout.positioningLock).toBe(true);
    expectPoint(layoutPosition(layout, sizeOf(rect), canvasSize), rect);
  });

  test.each(allAlignments)("resizeKeepsAlignedPoint %s", (alignment) => {
    const rect = { x: 500, y: 300, width: 400, height: 300 };
    const fixed = sceneEditorFixedPoint(rect, alignment);
    const resized = sceneEditorResizedRect(rect, fixed, 1.5);
    expectPoint(sceneEditorFixedPoint(resized, alignment), fixed);
    expect(sizeOf(resized)).toEqual({ width: 600, height: 450 });
  });

  test("handleIsOppositeAlignedCorner", () => {
    const rect = { x: 500, y: 300, width: 400, height: 300 };
    expect(sceneEditorHandlePoint(rect, "TopLeft")).toEqual({ x: 900, y: 600 });
    expect(sceneEditorHandlePoint(rect, "TopRight")).toEqual({ x: 500, y: 600 });
    expect(sceneEditorHandlePoint(rect, "BottomLeft")).toEqual({ x: 900, y: 300 });
    expect(sceneEditorHandlePoint(rect, "BottomRight")).toEqual({ x: 500, y: 300 });
    expect(sceneEditorHandlePoint(rect, "Center")).toEqual({ x: 900, y: 600 });
  });

  test("snapAxisKeepsOffsetWithinBounds", () => {
    expect(sceneEditorSnapAxis(-300, 400, 1920, 10, true, []).position).toBe(0);
    expect(sceneEditorSnapAxis(1800, 400, 1920, 10, true, []).position).toBe(1800);
    expect(sceneEditorSnapAxis(2500, 400, 1920, 10, true, []).position).toBe(1920);
    expect(sceneEditorSnapAxis(-300, 400, 1920, 10, false, []).position).toBe(-300);
    expect(sceneEditorSnapAxis(-500, 400, 1920, 10, false, []).position).toBe(-400);
    expect(sceneEditorSnapAxis(1800, 400, 1920, 10, false, []).position).toBe(1520);
    expect(sceneEditorSnapAxis(300, 2000, 1920, 10, true, []).position).toBe(300);
  });

  test.each(corners)("snapAxisBoundsMatchLayoutBounds %s", (alignment) => {
    const size = { width: 400, height: 300 };
    for (const position of [
      { x: -1000, y: -1000 },
      { x: 3000, y: 3000 },
    ]) {
      const x = sceneEditorSnapAxis(position.x, size.width, 1920, 10, isLeft(alignment), []);
      const y = sceneEditorSnapAxis(position.y, size.height, 1080, 10, isTop(alignment), []);
      const rect = { x: x.position, y: y.position, ...size };
      const layout = sceneEditorLayout(makeLayout(alignment), rect, canvasSize);
      expectPercentage(layout.x);
      expectPercentage(layout.y);
      expect([0, 100]).toContain(layout.x);
      expect([0, 100]).toContain(layout.y);
    }
  });

  test("snapAxisSnapsToEdgesAndCenter", () => {
    expect(sceneEditorSnapAxis(-5, 400, 1920, 10, true, []).position).toBe(0);
    expect(sceneEditorSnapAxis(1525, 400, 1920, 10, true, []).position).toBe(1520);
    const center = sceneEditorSnapAxis(765, 400, 1920, 10, true, []);
    expect(center.position).toBe(760);
    expect(center.guide).toBe(960);
    const unsized = sceneEditorSnapAxis(955, 0, 1920, 10, true, []);
    expect(unsized.position).toBe(960);
    expect(unsized.guide).toBe(960);
  });

  test("snapAxisSnapsToOtherWidgets", () => {
    const snap = sceneEditorSnapAxis(1003, 400, 1920, 10, true, [1000]);
    expect(snap.position).toBe(1000);
    expect(snap.guide).toBe(1000);
  });

  test("tapSelectsTopWidget", () => {
    expect(sceneEditorTapSelection(["small", "big"], null, false)).toBe("small");
    expect(sceneEditorTapSelection(["small", "big"], "big", false)).toBe("small");
    expect(sceneEditorTapSelection([], "big", false)).toBeNull();
  });

  test("tapOnSelectedInFrontDeselects", () => {
    expect(sceneEditorTapSelection(["small", "big"], "small", false)).toBeNull();
    expect(sceneEditorTapSelection(["big"], "big", false)).toBeNull();
  });

  test("tapAtSameLocationCycles", () => {
    const hits = ["small", "medium", "big"];
    expect(sceneEditorTapSelection(hits, "small", true)).toBe("medium");
    expect(sceneEditorTapSelection(hits, "medium", true)).toBe("big");
    expect(sceneEditorTapSelection(hits, "big", true)).toBeNull();
    expect(sceneEditorTapSelection(hits, null, true)).toBe("small");
  });
});

describe("sceneEditorLayout", () => {
  test("layoutAtFarEdgesIsNeverNegative", () => {
    const rect = { x: 1920 - 1.1, y: 1080 - 1.1, width: 1.1, height: 1.1 };
    const layout = sceneEditorLayout(makeLayout("BottomRight"), rect, canvasSize);
    expect(layout.x).toBe(0);
    expect(layout.y).toBe(0);
  });
});
