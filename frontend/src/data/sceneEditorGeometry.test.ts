import { describe, expect, test } from "vitest";
import type { SettingsAlignment } from "./moblinSettings";
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

  test.each(allAlignments)("resizeKeepsAlignedPoint %s", (alignment) => {
    const rect = { x: 500, y: 300, width: 400, height: 300 };
    const fixed = sceneEditorFixedPoint(rect, alignment, alignment);
    const resized = sceneEditorResizedRect(rect, fixed, 1.5);
    expectPoint(sceneEditorFixedPoint(resized, alignment, alignment), fixed);
    expect(sizeOf(resized)).toEqual({ width: 600, height: 450 });
  });

  test.each<[SettingsAlignment, SettingsAlignment, Point]>([
    ["TopLeft", "BottomRight", { x: 900, y: 600 }],
    ["TopLeft", "TopRight", { x: 900, y: 300 }],
    ["BottomRight", "TopLeft", { x: 500, y: 300 }],
    ["TopCenter", "BottomRight", { x: 700, y: 600 }],
    ["LeftCenter", "BottomRight", { x: 900, y: 450 }],
    ["Center", "TopLeft", { x: 700, y: 450 }],
  ])("resizeFixedPointIsOppositeHandleUnlessCentered %s %s", (alignment, anchor, expected) => {
    const rect = { x: 500, y: 300, width: 400, height: 300 };
    expect(sceneEditorFixedPoint(rect, alignment, anchor)).toEqual(expected);
  });

  test.each(corners)("resizeFromAnyCornerKeepsOppositeCorner %s", (anchor) => {
    const rect = { x: 500, y: 300, width: 400, height: 300 };
    const fixed = sceneEditorFixedPoint(rect, "TopLeft", anchor);
    const resized = sceneEditorResizedRect(rect, fixed, 0.5);
    const layout = sceneEditorLayout(makeLayout("TopLeft"), resized, canvasSize);
    const position = layoutPosition(layout, sizeOf(resized), canvasSize);
    const moved = { ...position, ...sizeOf(resized) };
    expectPoint(sceneEditorFixedPoint(moved, "TopLeft", anchor), fixed);
  });

  test("handleIsOppositeAlignedCorner", () => {
    const rect = { x: 500, y: 300, width: 400, height: 300 };
    expect(sceneEditorHandlePoint(rect, "TopLeft")).toEqual({ x: 900, y: 600 });
    expect(sceneEditorHandlePoint(rect, "TopRight")).toEqual({ x: 500, y: 600 });
    expect(sceneEditorHandlePoint(rect, "BottomLeft")).toEqual({ x: 900, y: 300 });
    expect(sceneEditorHandlePoint(rect, "BottomRight")).toEqual({ x: 500, y: 300 });
    expect(sceneEditorHandlePoint(rect, "Center")).toEqual({ x: 900, y: 600 });
  });

  test.each([
    [-300, 400, -300],
    [-500, 400, -400],
    [1800, 400, 1800],
    [2500, 400, 1920],
    [300, 2000, 300],
  ])("snapAxisKeepsWidgetTouchingCanvas %d %d", (position, length, expected) => {
    expect(sceneEditorSnapAxis(position, length, 1920, 10, []).position).toBe(expected);
  });

  test.each(corners)("snapAxisBoundsMatchLayoutBounds %s", (alignment) => {
    const size = { width: 400, height: 300 };
    for (const position of [
      { x: -1000, y: -1000 },
      { x: 3000, y: 3000 },
    ]) {
      const x = sceneEditorSnapAxis(position.x, size.width, 1920, 10, []);
      const y = sceneEditorSnapAxis(position.y, size.height, 1080, 10, []);
      const rect = { x: x.position, y: y.position, ...size };
      const layout = sceneEditorLayout(makeLayout(alignment), rect, canvasSize);
      for (const offset of [layout.x, layout.y]) {
        expect(offset).toBeGreaterThanOrEqual(-100);
        expect(offset).toBeLessThanOrEqual(100);
      }
    }
  });

  test("snapAxisSnapsToEdgesAndCenter", () => {
    expect(sceneEditorSnapAxis(-5, 400, 1920, 10, []).position).toBe(0);
    expect(sceneEditorSnapAxis(1525, 400, 1920, 10, []).position).toBe(1520);
    const center = sceneEditorSnapAxis(765, 400, 1920, 10, []);
    expect(center.position).toBe(760);
    expect(center.guide).toBe(960);
    const unsized = sceneEditorSnapAxis(955, 0, 1920, 10, []);
    expect(unsized.position).toBe(960);
    expect(unsized.guide).toBe(960);
  });

  test("snapAxisSnapsToOtherWidgets", () => {
    const snap = sceneEditorSnapAxis(1003, 400, 1920, 10, [1000]);
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
  test("layoutOfWidgetLargerThanCanvasStaysWithinMoblinsBounds", () => {
    const rect = { x: -2000, y: -1200, width: 2000, height: 1200 };
    const layout = sceneEditorLayout(makeLayout("TopLeft"), rect, canvasSize);
    expect(layout.x).toBe(-100);
    expect(layout.y).toBe(-100);
  });
});
