import { Show } from "solid-js";
import { useReadOnly } from "../../Form";
import { clamped } from "../../../../data/sceneEditorGeometry";

export type Fraction = { x: number; y: number; width: number; height: number };

type Anchor = "topLeft" | "topRight" | "bottomLeft" | "bottomRight" | "center";

const anchorAt = (
  location: { x: number; y: number },
  size: { width: number; height: number },
  rect: Fraction,
): [Anchor | null, { width: number; height: number }] => {
  const x = location.x / size.width;
  const y = location.y / size.height;
  const right = rect.x + rect.width;
  const bottom = rect.y + rect.height;
  const centerX = rect.x + rect.width / 2;
  const centerY = rect.y + rect.height / 2;
  const innerLeft = rect.x + rect.width / 4;
  const innerTop = rect.y + rect.height / 4;
  const innerRight = right - rect.width / 4;
  const innerBottom = bottom - rect.height / 4;
  const centerOffset = { width: centerX - x, height: centerY - y };
  if (x > innerLeft && x < innerRight && y > innerTop && y < innerBottom) {
    return ["center", centerOffset];
  }
  if (x + 0.1 < rect.x || x > right + 0.1 || y + 0.1 < rect.y || y > bottom + 0.1) {
    return ["center", centerOffset];
  }
  if (x < innerLeft && y < innerTop) {
    return ["topLeft", { width: rect.x - x, height: rect.y - y }];
  }
  if (x > innerRight && y < innerTop) {
    return ["topRight", { width: right - x, height: rect.y - y }];
  }
  if (x < innerLeft && y > innerBottom) {
    return ["bottomLeft", { width: rect.x - x, height: bottom - y }];
  }
  if (x > innerRight && y > innerBottom) {
    return ["bottomRight", { width: right - x, height: bottom - y }];
  }
  return [null, { width: 0, height: 0 }];
};

const moved = (
  anchor: Anchor | null,
  rect: Fraction,
  position: { x: number; y: number },
  size: { width: number; height: number },
  offset: { width: number; height: number },
): Fraction => {
  let left = rect.x;
  let top = rect.y;
  let right = rect.x + rect.width;
  let bottom = rect.y + rect.height;
  const x = clamped(position.x / size.width + offset.width, 0, 1);
  const y = clamped(position.y / size.height + offset.height, 0, 1);
  const minimumWidth = 0.05;
  const minimumHeight = 0.04;
  switch (anchor) {
    case "topLeft":
      if (x + minimumWidth < right) left = x;
      if (y + minimumHeight < bottom) top = y;
      break;
    case "topRight":
      if (x > left + minimumWidth) right = x;
      if (y + minimumHeight < bottom) top = y;
      break;
    case "bottomLeft":
      if (x + minimumWidth < right) left = x;
      if (y > top + minimumHeight) bottom = y;
      break;
    case "bottomRight":
      if (x > left + minimumWidth) right = x;
      if (y > top + minimumHeight) bottom = y;
      break;
    case "center": {
      const halfWidth = rect.width / 2;
      const halfHeight = rect.height / 2;
      let newX = rect.x;
      let newY = rect.y;
      if (x - halfWidth >= 0 && x + halfWidth <= 1) newX = x - halfWidth;
      if (y - halfHeight >= 0 && y + halfHeight <= 1) newY = y - halfHeight;
      left = newX;
      top = newY;
      right = newX + rect.width;
      bottom = newY + rect.height;
      break;
    }
    default:
      break;
  }
  return { x: left, y: top, width: right - left, height: bottom - top };
};

export function PositioningRectangle(props: {
  image: string | null;
  portrait?: boolean;
  aspect?: string;
  rect: Fraction;
  onChange: (rect: Fraction) => void;
}) {
  const readOnly = useReadOnly();
  let surface: HTMLDivElement | undefined;
  let drag: { anchor: Anchor | null; offset: { width: number; height: number } } | null = null;
  const size = () => ({ width: surface?.clientWidth ?? 1, height: surface?.clientHeight ?? 1 });
  const local = (event: PointerEvent) => {
    const bounds = surface!.getBoundingClientRect();
    const scale = bounds.width / size().width;
    return { x: (event.clientX - bounds.left) / scale, y: (event.clientY - bounds.top) / scale };
  };
  const corners = () => {
    const rect = props.rect;
    return [
      [rect.x, rect.y],
      [rect.x + rect.width, rect.y],
      [rect.x + rect.width, rect.y + rect.height],
      [rect.x, rect.y + rect.height],
    ];
  };
  return (
    <div
      ref={(element) => (surface = element)}
      class={`relative w-full overflow-hidden bg-phone-preview-dark select-none ${readOnly() ? "" : "touch-none"}`}
      style={{ "aspect-ratio": props.aspect ?? (props.portrait ? "9 / 16" : "16 / 9") }}
      onPointerDown={(event) => {
        if (readOnly()) {
          return;
        }
        event.currentTarget.setPointerCapture(event.pointerId);
        const [anchor, offset] = anchorAt(local(event), size(), props.rect);
        drag = { anchor, offset };
      }}
      onPointerMove={(event) => {
        if (drag) {
          props.onChange(moved(drag.anchor, props.rect, local(event), size(), drag.offset));
        }
      }}
      onPointerUp={() => (drag = null)}
      onPointerCancel={() => (drag = null)}
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
        <rect
          x={props.rect.x * 100}
          y={props.rect.y * 100}
          width={props.rect.width * 100}
          height={props.rect.height * 100}
          fill="rgb(255 255 255 / 0.25)"
          stroke="white"
          stroke-width="1.5"
          vector-effect="non-scaling-stroke"
        />
      </svg>
      {corners().map(([x, y]) => (
        <span
          class="pointer-events-none absolute size-[12px] -translate-1/2 rounded-full border border-black bg-white"
          style={{ left: `${x! * 100}%`, top: `${y! * 100}%` }}
        />
      ))}
    </div>
  );
}
