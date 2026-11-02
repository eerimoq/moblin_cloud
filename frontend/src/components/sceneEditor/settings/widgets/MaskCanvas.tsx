import { createSignal, Show } from "solid-js";
import Icon from "../../../Icon";
import { clamped } from "../../../../data/sceneEditorGeometry";
import { t } from "../../../../i18n";

export type MaskPoint = { x: number; y: number };

const handleRadius = 12;
const edgeHitWidth = 20;
const dragThreshold = 6;
const tapThreshold = 4;
export const maskMinimumPoints = 3;

export const catmullRomPath = (points: { x: number; y: number }[], tension: number) => {
  if (points.length === 0) {
    return "";
  }
  const count = points.length;
  let path = `M ${points[0]!.x} ${points[0]!.y}`;
  for (let index = 0; index < count; index += 1) {
    const point0 = points[(index - 1 + count) % count]!;
    const point1 = points[index]!;
    const point2 = points[(index + 1) % count]!;
    const point3 = points[(index + 2) % count]!;
    const control1 = {
      x: point1.x + (point2.x - point0.x) * tension,
      y: point1.y + (point2.y - point0.y) * tension,
    };
    const control2 = {
      x: point2.x - (point3.x - point1.x) * tension,
      y: point2.y - (point3.y - point1.y) * tension,
    };
    path += ` C ${control1.x} ${control1.y} ${control2.x} ${control2.y} ${point2.x} ${point2.y}`;
  }
  return `${path} Z`;
};

const segmentDistance = (point: MaskPoint, a: MaskPoint, b: MaskPoint) => {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lengthSquared = dx * dx + dy * dy;
  if (lengthSquared <= 0) {
    return Math.hypot(point.x - a.x, point.y - a.y);
  }
  const along = Math.max(
    0,
    Math.min(1, ((point.x - a.x) * dx + (point.y - a.y) * dy) / lengthSquared),
  );
  return Math.hypot(point.x - (a.x + along * dx), point.y - (a.y + along * dy));
};

export function MaskCanvas(props: {
  image: string | null;
  portrait?: boolean;
  points: MaskPoint[];
  tension: number;
  selectedPoint: number | null;
  selectedEdge: number | null;
  onSelect: (point: number | null, edge: number | null) => void;
  onChange: (points: MaskPoint[]) => void;
  onRefresh: () => void;
}) {
  let surface: HTMLDivElement | undefined;
  const [size, setSize] = createSignal({ width: 1, height: 1 });
  let gesture: {
    start: MaskPoint;
    pendingDrag: number | null;
    drag: number | null;
    panStart: MaskPoint[] | null;
  } | null = null;
  const toCanvas = (point: MaskPoint) => ({
    x: (point.x / 100) * size().width,
    y: (point.y / 100) * size().height,
  });
  const normalized = (location: MaskPoint): MaskPoint => ({
    x: clamped((location.x / size().width) * 100, 0, 100),
    y: clamped((location.y / size().height) * 100, 0, 100),
  });
  const local = (event: PointerEvent) => {
    const bounds = surface!.getBoundingClientRect();
    setSize({ width: surface!.clientWidth, height: surface!.clientHeight });
    const scale = bounds.width / surface!.clientWidth;
    return { x: (event.clientX - bounds.left) / scale, y: (event.clientY - bounds.top) / scale };
  };
  const closestPoint = (location: MaskPoint) => {
    let closest: number | null = null;
    let closestDistance = handleRadius * 1.2;
    props.points.forEach((point, index) => {
      const distance = Math.hypot(location.x - toCanvas(point).x, location.y - toCanvas(point).y);
      if (distance < closestDistance) {
        closestDistance = distance;
        closest = index;
      }
    });
    return closest;
  };
  const closestEdge = (location: MaskPoint) => {
    if (props.points.length < 2) {
      return null;
    }
    let closest: number | null = null;
    let closestDistance = edgeHitWidth;
    const canvasPoints = props.points.map(toCanvas);
    canvasPoints.forEach((point, index) => {
      const distance = segmentDistance(
        location,
        point,
        canvasPoints[(index + 1) % canvasPoints.length]!,
      );
      if (distance < closestDistance) {
        closestDistance = distance;
        closest = index;
      }
    });
    return closest;
  };
  const resize = (scale: number) => {
    const centerX = props.points.reduce((sum, point) => sum + point.x, 0) / props.points.length;
    const centerY = props.points.reduce((sum, point) => sum + point.y, 0) / props.points.length;
    props.onChange(
      props.points.map((point) => ({
        x: clamped(centerX + (point.x - centerX) * scale, 0, 100),
        y: clamped(centerY + (point.y - centerY) * scale, 0, 100),
      })),
    );
  };
  const highlightedEdge = () => {
    const edge = props.selectedEdge;
    if (edge === null || props.points.length < 2) {
      return "";
    }
    const count = props.points.length;
    const point0 = props.points[(edge - 1 + count) % count]!;
    const point1 = props.points[edge]!;
    const point2 = props.points[(edge + 1) % count]!;
    const point3 = props.points[(edge + 2) % count]!;
    const control1 = {
      x: point1.x + (point2.x - point0.x) * props.tension,
      y: point1.y + (point2.y - point0.y) * props.tension,
    };
    const control2 = {
      x: point2.x - (point3.x - point1.x) * props.tension,
      y: point2.y - (point3.y - point1.y) * props.tension,
    };
    return `M ${point1.x} ${point1.y} C ${control1.x} ${control1.y} ${control2.x} ${control2.y} ${point2.x} ${point2.y}`;
  };
  return (
    <div class="flex flex-col gap-2">
      <div
        ref={(element) => (surface = element)}
        class={`relative w-full touch-none overflow-hidden bg-phone-preview-dark select-none ${
          props.portrait ? "aspect-9/16" : "aspect-video"
        }`}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          const start = local(event);
          const index = closestPoint(start);
          gesture = {
            start,
            pendingDrag: index,
            drag: null,
            panStart: index === null ? [...props.points] : null,
          };
          if (index !== null) {
            props.onSelect(index, null);
          }
        }}
        onPointerMove={(event) => {
          if (!gesture) {
            return;
          }
          const location = local(event);
          if (gesture.pendingDrag !== null && gesture.drag === null) {
            if (
              Math.hypot(location.x - gesture.start.x, location.y - gesture.start.y) >=
              dragThreshold
            ) {
              gesture.drag = gesture.pendingDrag;
              gesture.pendingDrag = null;
            }
          }
          if (gesture.drag !== null) {
            const points = [...props.points];
            points[gesture.drag] = normalized(location);
            props.onChange(points);
          } else if (gesture.panStart) {
            const dx = ((location.x - gesture.start.x) / size().width) * 100;
            const dy = ((location.y - gesture.start.y) / size().height) * 100;
            props.onChange(
              gesture.panStart.map((point) => ({
                x: clamped(point.x + dx, 0, 100),
                y: clamped(point.y + dy, 0, 100),
              })),
            );
          }
        }}
        onPointerUp={(event) => {
          if (!gesture) {
            return;
          }
          const location = local(event);
          const wasDragging = gesture.drag !== null;
          const wasPanning = gesture.panStart !== null;
          const distance = Math.hypot(location.x - gesture.start.x, location.y - gesture.start.y);
          const start = gesture.start;
          gesture = null;
          if (!wasDragging && (!wasPanning || distance < tapThreshold)) {
            const point = closestPoint(start);
            if (point !== null) {
              props.onSelect(point, null);
            } else {
              const edge = closestEdge(start);
              props.onSelect(null, edge);
            }
          }
        }}
        onPointerCancel={() => (gesture = null)}
      >
        <Show when={props.image}>
          {(image) => (
            <img
              src={image()}
              alt=""
              class="pointer-events-none absolute inset-0 size-full object-cover"
            />
          )}
        </Show>
        <svg
          class="pointer-events-none absolute inset-0 size-full"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
        >
          <path
            d={catmullRomPath(props.points, props.tension)}
            fill="rgb(255 255 255 / 0.25)"
            stroke="white"
            stroke-width="1.5"
            vector-effect="non-scaling-stroke"
          />
          <path
            d={highlightedEdge()}
            fill="none"
            stroke="var(--color-phone-warn)"
            stroke-width="3"
            vector-effect="non-scaling-stroke"
          />
        </svg>
        {props.points.map((point, index) => (
          <span
            class={`pointer-events-none absolute -translate-1/2 rounded-full border border-black ${
              index === props.selectedPoint ? "size-[17px] bg-phone-warn" : "size-[12px] bg-white"
            }`}
            style={{ left: `${point.x}%`, top: `${point.y}%` }}
          />
        ))}
      </div>
      <div class="flex items-center gap-3 px-1 text-phone-tint">
        <button type="button" aria-label={t("Smaller")} onClick={() => resize(1 / 1.1)}>
          <Icon name="resize-down" class="size-[26px]" />
        </button>
        <button type="button" aria-label={t("Bigger")} onClick={() => resize(1.1)}>
          <Icon name="resize-up" class="size-[26px]" />
        </button>
        <button type="button" aria-label={t("Refresh")} onClick={() => props.onRefresh()}>
          <Icon name="refresh" class="size-[26px]" />
        </button>
      </div>
    </div>
  );
}
