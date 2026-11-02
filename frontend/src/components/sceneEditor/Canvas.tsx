import { createMemo, createSignal, For, Show } from "solid-js";
import Icon from "../Icon";
import { widgetIcon } from "./icons";
import {
  hasPosition,
  hasSize,
  isHorizontalCenter,
  isLeft,
  isTop,
  isVerticalCenter,
  type SettingsAlignment,
} from "../../data/moblinSettings";
import {
  clamped,
  diagonalDirection,
  diagonalPosition,
  distance,
  hitRect,
  movesDiagonally,
  rectContains,
  rectMaxX,
  rectMaxY,
  rectMidX,
  rectMidY,
  sceneEditorFixedPoint,
  sceneEditorHandlePoint,
  sceneEditorResizedRect,
  sceneEditorSnapAxis,
  sceneEditorSnapResizeAxis,
  sceneEditorTapSelection,
  widgetOutlineViewRect,
} from "../../data/sceneEditorGeometry";
import type { Point, Rect, Size, WidgetOutline } from "../../data/widgetOutlines";

type Edit = {
  outline: WidgetOutline;
  mode: "move" | "resize";
  startSize: number;
  rect: Rect;
  guideX?: number;
  guideY?: number;
  start: Point;
};

export default function Canvas(props: {
  outlines: WidgetOutline[];
  canvas: Size;
  size: Size;
  selected: string | null;
  onSelect: (id: string | null) => void;
  onMove: (outline: WidgetOutline, rect: Rect) => void;
  onResize: (outline: WidgetOutline, size: number) => void;
}) {
  const [edit, setEdit] = createSignal<Edit | null>(null);
  let lastTap: Point | null = null;
  let surface: HTMLDivElement | undefined;
  let pressed: { point: Point; moved: boolean } | null = null;

  const scale = () => props.canvas.width / props.size.width;
  const snapThreshold = () => 8 * scale();
  const editable = () => props.outlines.filter((outline) => outline.inScene);
  const selectedOutline = createMemo(() =>
    editable().find((outline) => outline.id === props.selected),
  );
  const viewRect = (rect: Rect) => widgetOutlineViewRect(rect, props.canvas, props.size);
  const viewPoint = (point: Point): Point => {
    const rect = viewRect({ x: point.x, y: point.y, width: 0, height: 0 });
    return { x: rect.x, y: rect.y };
  };
  const canvasDelta = (translation: Size): Size => ({
    width: translation.width * scale(),
    height: translation.height * scale(),
  });
  const currentRect = (outline: WidgetOutline) => {
    const current = edit();
    return current && current.outline.id === outline.id ? current.rect : outline.rect;
  };
  const localPoint = (event: PointerEvent): Point => {
    const bounds = surface!.getBoundingClientRect();
    const factor = bounds.width / props.size.width;
    return { x: (event.clientX - bounds.left) / factor, y: (event.clientY - bounds.top) / factor };
  };
  const outlinesAt = (location: Point) =>
    editable()
      .slice()
      .reverse()
      .map((outline) => ({
        outline,
        hit: hitRect(viewRect(outline.rect), outline.sized, outline.alignment),
      }))
      .filter(({ hit }) => rectContains(hit, location))
      .sort(
        (first, second) =>
          first.hit.width * first.hit.height - second.hit.width * second.hit.height,
      )
      .map(({ outline }) => outline);
  const selectAt = (location: Point) => {
    const sameLocation = lastTap ? distance(lastTap, location) < 10 : false;
    lastTap = location;
    props.onSelect(
      sceneEditorTapSelection(
        outlinesAt(location).map((outline) => outline.id),
        props.selected,
        sameLocation,
      ),
    );
  };
  const isResizable = (outline: WidgetOutline) => outline.sized && hasSize(outline.widget.type);
  const snapTargetsX = (outline: WidgetOutline) =>
    outline.sized
      ? [outline.rect.x, rectMidX(outline.rect), rectMaxX(outline.rect)]
      : [outline.rect.x];
  const snapTargetsY = (outline: WidgetOutline) =>
    outline.sized
      ? [outline.rect.y, rectMidY(outline.rect), rectMaxY(outline.rect)]
      : [outline.rect.y];

  const moveChanged = (translation: Size) => {
    let current = edit();
    if (!current) {
      const outline = selectedOutline();
      if (!outline || !hasPosition(outline.widget.type)) {
        return;
      }
      current = {
        outline,
        mode: "move",
        startSize: outline.sceneWidget.size,
        rect: outline.rect,
        start: { x: 0, y: 0 },
      };
    }
    if (current.mode !== "move") {
      return;
    }
    const delta = canvasDelta(translation);
    const rect = current.outline.rect;
    const position = { x: rect.x + delta.width, y: rect.y + delta.height };
    const layout = current.outline.sceneWidget;
    const next: Edit = { ...current, guideX: undefined, guideY: undefined };
    if (movesDiagonally(layout)) {
      const origin = diagonalPosition(
        layout.alignment,
        rect,
        position,
        snapThreshold(),
        props.canvas,
      );
      next.rect = { ...rect, x: origin.x, y: origin.y };
    } else {
      const others = props.outlines.filter((outline) => outline.id !== current.outline.id);
      const x = isHorizontalCenter(layout.alignment)
        ? { position: rect.x }
        : sceneEditorSnapAxis(
            position.x,
            rect.width,
            props.canvas.width,
            snapThreshold(),
            isLeft(layout.alignment),
            others.flatMap(snapTargetsX),
          );
      const y = isVerticalCenter(layout.alignment)
        ? { position: rect.y }
        : sceneEditorSnapAxis(
            position.y,
            rect.height,
            props.canvas.height,
            snapThreshold(),
            isTop(layout.alignment),
            others.flatMap(snapTargetsY),
          );
      next.rect = { ...rect, x: x.position, y: y.position };
      next.guideX = x.guide;
      next.guideY = y.guide;
    }
    setEdit(next);
    props.onMove(current.outline, next.rect);
  };

  const resize = (factor: number) => {
    const current = edit();
    if (!current || current.mode !== "resize") {
      return;
    }
    const rect = current.outline.rect;
    const fixed = sceneEditorFixedPoint(rect, current.outline.alignment);
    const factors: [number, number] = [1 / current.startSize, 100 / current.startSize];
    let clampedFactor = clamped(factor, factors[0], factors[1]);
    const others = props.outlines.filter((outline) => outline.id !== current.outline.id);
    const threshold =
      snapThreshold() / distance(sceneEditorHandlePoint(rect, current.outline.alignment), fixed);
    const x = sceneEditorSnapResizeAxis(
      [rect.x, rectMaxX(rect), rectMidX(rect)],
      fixed.x,
      clampedFactor,
      factors,
      threshold,
      [0, props.canvas.width / 2, props.canvas.width, ...others.flatMap(snapTargetsX)],
    );
    const y = sceneEditorSnapResizeAxis(
      [rect.y, rectMaxY(rect), rectMidY(rect)],
      fixed.y,
      clampedFactor,
      factors,
      threshold,
      [0, props.canvas.height / 2, props.canvas.height, ...others.flatMap(snapTargetsY)],
    );
    const next: Edit = { ...current, guideX: undefined, guideY: undefined };
    if (x && x.distance < (y?.distance ?? Infinity)) {
      clampedFactor = x.factor;
      next.guideX = x.guide;
    } else if (y) {
      clampedFactor = y.factor;
      next.guideY = y.guide;
    }
    next.rect = sceneEditorResizedRect(rect, fixed, clampedFactor);
    setEdit(next);
    props.onResize(current.outline, current.startSize * clampedFactor);
  };

  const handleChanged = (outline: WidgetOutline, translation: Size) => {
    if (!edit()) {
      setEdit({
        outline,
        mode: "resize",
        startSize: outline.sceneWidget.size,
        rect: outline.rect,
        start: { x: 0, y: 0 },
      });
    }
    const current = edit()!.outline;
    const fixed = sceneEditorFixedPoint(current.rect, current.alignment);
    const start = sceneEditorHandlePoint(current.rect, current.alignment);
    const delta = canvasDelta(translation);
    const point = { x: start.x + delta.width, y: start.y + delta.height };
    const startDistance = distance(start, fixed);
    if (startDistance <= 0) {
      return;
    }
    resize(distance(point, fixed) / startDistance);
  };

  const onPointerDown = (event: PointerEvent) => {
    if (event.button !== 0) {
      return;
    }
    surface?.setPointerCapture(event.pointerId);
    pressed = { point: localPoint(event), moved: false };
  };
  const onPointerMove = (event: PointerEvent) => {
    if (!pressed) {
      return;
    }
    const point = localPoint(event);
    const translation = { width: point.x - pressed.point.x, height: point.y - pressed.point.y };
    if (!pressed.moved && Math.hypot(translation.width, translation.height) < 5) {
      return;
    }
    pressed.moved = true;
    moveChanged(translation);
  };
  const onPointerUp = (event: PointerEvent) => {
    if (!pressed) {
      return;
    }
    const moved = pressed.moved;
    const point = localPoint(event);
    pressed = null;
    setEdit(null);
    if (!moved) {
      selectAt(point);
    }
  };

  const color = (outline: WidgetOutline, selected: boolean) =>
    selected ? "var(--color-phone-tint)" : outline.inScene ? "white" : "var(--color-phone-border)";
  const selectedFill = "color-mix(in srgb, var(--color-phone-tint) 25%, transparent)";

  const outlineView = (outline: WidgetOutline) => {
    const selected = () => outline.id === props.selected;
    const rect = () => viewRect(currentRect(outline));
    const stroke = () => color(outline, selected());
    const dash = () => (outline.inScene && outline.enabled ? undefined : "4 4");
    const dotSize = () => (selected() ? 12 : 8);
    const labelY = () => (rect().y > 24 && !outline.sized ? rect().y - 24 : rect().y + 4);
    return (
      <div class="pointer-events-none absolute inset-0">
        <Show
          when={outline.sized}
          fallback={
            <>
              <div
                class="absolute rounded-full"
                style={{
                  left: `${rect().x - 16}px`,
                  top: `${rect().y - 16}px`,
                  width: "32px",
                  height: "32px",
                  border: `${selected() ? 2 : 1}px ${dash() ? "dashed" : "solid"} ${stroke()}`,
                  background: selected() ? selectedFill : "transparent",
                }}
              />
              <div
                class="absolute rounded-full"
                style={{
                  left: `${rect().x - dotSize() / 2}px`,
                  top: `${rect().y - dotSize() / 2}px`,
                  width: `${dotSize()}px`,
                  height: `${dotSize()}px`,
                  background: stroke(),
                }}
              />
            </>
          }
        >
          <div
            class="absolute"
            style={{
              left: `${rect().x}px`,
              top: `${rect().y}px`,
              width: `${rect().width}px`,
              height: `${rect().height}px`,
              border: `${selected() ? 2 : 1}px ${dash() ? "dashed" : "solid"} ${stroke()}`,
              background: selected()
                ? selectedFill
                : outline.widget.type === "Scene"
                  ? "rgb(255 255 255 / 0.1)"
                  : "transparent",
            }}
          />
        </Show>
        <div
          class={`absolute flex items-center gap-[3px] rounded-[3px] px-[4px] py-[2px] text-[13px] leading-[16px] whitespace-nowrap text-white ${
            selected() ? "bg-phone-tint font-semibold" : "bg-black/60"
          }`}
          style={{ left: `${rect().x + 4}px`, top: `${labelY()}px` }}
        >
          <Icon name={widgetIcon(outline.widget.type)} class="size-[13px]" />
          <span>{outline.widget.name}</span>
        </div>
      </div>
    );
  };

  const guideLine = (start: Point, end: Point) => {
    const from = viewPoint(start);
    const to = viewPoint(end);
    return (
      <line
        x1={from.x}
        y1={from.y}
        x2={to.x}
        y2={to.y}
        stroke="var(--color-phone-warn)"
        stroke-width="1"
      />
    );
  };
  const diagonalGuideLine = (alignment: SettingsAlignment) => {
    const direction = diagonalDirection(alignment);
    const length = Math.min(props.canvas.width, props.canvas.height);
    const corner = {
      x: isLeft(alignment) ? 0 : props.canvas.width,
      y: isTop(alignment) ? 0 : props.canvas.height,
    };
    return guideLine(corner, {
      x: corner.x + direction.width * length,
      y: corner.y + direction.height * length,
    });
  };

  const handlePoint = () => {
    const outline = selectedOutline();
    if (!outline || !isResizable(outline) || edit()?.mode === "move") {
      return null;
    }
    return viewPoint(sceneEditorHandlePoint(currentRect(outline), outline.alignment));
  };

  let handlePressed: Point | null = null;
  return (
    <div
      ref={(element) => (surface = element)}
      class="absolute inset-0 touch-none overflow-hidden select-none"
      style={{ width: `${props.size.width}px`, height: `${props.size.height}px` }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => {
        pressed = null;
        setEdit(null);
      }}
    >
      <For each={props.outlines.filter((outline) => outline.id !== props.selected)}>
        {outlineView}
      </For>
      <Show when={props.outlines.find((outline) => outline.id === props.selected)} keyed>
        {outlineView}
      </Show>
      <svg
        class="pointer-events-none absolute inset-0"
        width={props.size.width}
        height={props.size.height}
      >
        <Show when={edit()}>
          {(current) => (
            <>
              <Show when={current().guideX !== undefined}>
                {guideLine(
                  { x: current().guideX!, y: 0 },
                  { x: current().guideX!, y: props.canvas.height },
                )}
              </Show>
              <Show when={current().guideY !== undefined}>
                {guideLine(
                  { x: 0, y: current().guideY! },
                  { x: props.canvas.width, y: current().guideY! },
                )}
              </Show>
              <Show
                when={current().mode === "move" && movesDiagonally(current().outline.sceneWidget)}
              >
                {diagonalGuideLine(current().outline.sceneWidget.alignment)}
              </Show>
            </>
          )}
        </Show>
      </svg>
      <Show when={handlePoint()}>
        {(point) => (
          <div
            class="absolute grid size-[44px] -translate-1/2 cursor-nwse-resize place-items-center"
            style={{ left: `${point().x}px`, top: `${point().y}px` }}
            onPointerDown={(event) => {
              event.stopPropagation();
              if (event.button !== 0) {
                return;
              }
              (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
              handlePressed = localPoint(event);
            }}
            onPointerMove={(event) => {
              const outline = selectedOutline();
              if (!handlePressed || !outline) {
                return;
              }
              const local = localPoint(event);
              handleChanged(outline, {
                width: local.x - handlePressed.x,
                height: local.y - handlePressed.y,
              });
            }}
            onPointerUp={() => {
              handlePressed = null;
              setEdit(null);
            }}
            onPointerCancel={() => {
              handlePressed = null;
              setEdit(null);
            }}
          >
            <span class="size-[20px] rounded-full border-2 border-white bg-phone-tint" />
          </div>
        )}
      </Show>
    </div>
  );
}
