import { createEffect, createMemo, createSignal, For, Index, on, Show, type JSX } from "solid-js";
import Icon from "./Icon";
import { ask } from "./Modal";
import { cardClass, ghostButtonClass, Notice, Spinner } from "./PageParts";
import { controlDevice, deviceScenes, type Control } from "../data/account";
import { BackendError } from "../data/backend";
import {
  alignmentNames,
  alignments,
  boxPosition,
  boxSize,
  cameraOf,
  creatableWidgetTypes,
  hasPosition,
  hasSize,
  moved,
  movedTo,
  newId,
  resizeCorner,
  resizedBy,
  newSceneWidget,
  sameId,
  sceneErrorText,
  withCamera,
  widgetTypeNames,
  type Alignment,
  type Scene,
  type SceneConfig,
  type SceneWidget,
  type Widget,
  type WidgetType,
} from "../data/scenes";
import { t } from "../i18n";

const smallInputClass =
  "h-10 w-full rounded-xl border-[1.5px] border-line bg-well px-3 text-[15px] text-ink placeholder:text-muted focus:border-leaf focus:outline-none disabled:opacity-60";

const smallButtonClass =
  "inline-flex h-10 items-center justify-center gap-2 rounded-full bg-moss px-4 font-display text-[15px] font-semibold whitespace-nowrap text-white transition-colors hover:bg-moss-bright disabled:opacity-60";

const smallGhostButtonClass =
  "inline-flex h-10 items-center justify-center gap-2 rounded-full border-[1.5px] border-ghost-line bg-ghost px-4 font-display text-[15px] font-semibold whitespace-nowrap text-ink transition-colors hover:bg-ghost-hover disabled:opacity-60";

const labelClass = "flex min-w-0 flex-col gap-1 text-[13px] text-muted";

const checkboxLabelClass = "flex h-10 items-center gap-2 text-[15px] whitespace-nowrap text-ink-2";

const iconButtonClass =
  "grid size-8 shrink-0 place-items-center rounded-full text-ink-2 hover:bg-ghost-hover hover:text-ink disabled:opacity-40 disabled:hover:bg-transparent";

const copy = <T,>(value: T): T => JSON.parse(JSON.stringify(value));

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const readBase64 = (file: File) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });

function MoveButtons(props: {
  index: number;
  count: number;
  disabled: boolean;
  onMove: (by: number) => void;
}) {
  return (
    <>
      <button
        type="button"
        class={iconButtonClass}
        aria-label={t("Move up")}
        title={t("Move up")}
        disabled={props.disabled || props.index === 0}
        onClick={() => props.onMove(-1)}
      >
        <Icon name="chevron-down" class="size-[18px] rotate-180" />
      </button>
      <button
        type="button"
        class={iconButtonClass}
        aria-label={t("Move down")}
        title={t("Move down")}
        disabled={props.disabled || props.index === props.count - 1}
        onClick={() => props.onMove(1)}
      >
        <Icon name="chevron-down" class="size-[18px]" />
      </button>
    </>
  );
}

function SceneCanvas(props: {
  scene: Scene;
  widgets: Widget[];
  selected: string | null;
  disabled: boolean;
  onSelect: (id: string) => void;
  onMove: (index: number, layout: SceneWidget) => void;
}) {
  let canvas: HTMLDivElement | undefined;
  const widgetOf = (layout: SceneWidget) =>
    props.widgets.find((widget) => sameId(widget.id, layout.widgetId));

  const startDrag = (
    event: PointerEvent,
    index: number,
    layout: SceneWidget,
    dragged: (size: [number, number], dx: number, dy: number) => SceneWidget,
  ) => {
    props.onSelect(layout.id);
    if (props.disabled || layout.positioningLock || !canvas) {
      return;
    }
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const bounds = canvas.getBoundingClientRect();
    const size = boxSize(widgetOf(layout), layout);
    const startX = event.clientX;
    const startY = event.clientY;
    const move = (moveEvent: PointerEvent) => {
      props.onMove(
        index,
        dragged(
          size,
          (moveEvent.clientX - startX) / bounds.width,
          (moveEvent.clientY - startY) / bounds.height,
        ),
      );
    };
    const stop = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", stop);
      target.removeEventListener("pointercancel", stop);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", stop);
    target.addEventListener("pointercancel", stop);
  };

  const startMove = (event: PointerEvent, index: number, layout: SceneWidget) =>
    startDrag(event, index, layout, (size, dx, dy) => {
      const [left, top] = boxPosition(layout, size);
      return movedTo(layout, size, [
        Math.max(0, Math.min(1 - size[0], left! + dx)),
        Math.max(0, Math.min(1 - size[1], top! + dy)),
      ]);
    });

  const startResize = (event: PointerEvent, index: number, layout: SceneWidget) =>
    startDrag(event, index, layout, (size, dx, dy) => resizedBy(layout, size, [dx, dy]));

  return (
    <div
      ref={(element) => (canvas = element)}
      class="relative aspect-video w-full touch-none overflow-hidden rounded-xl border-[1.5px] border-line bg-well select-none"
      role="group"
      aria-label={t("Layout")}
    >
      <Index each={props.scene.widgets}>
        {(layout, index) => {
          const widget = () => widgetOf(layout());
          const size = () => boxSize(widget(), layout());
          const position = () => boxPosition(layout(), size());
          const selected = () => props.selected === layout().id;
          const corner = () => resizeCorner(layout().alignment);
          return (
            <Show when={widget() && hasPosition(widget()!.type)}>
              <div
                class="absolute"
                classList={{ "z-10": selected() }}
                style={{
                  left: `${position()[0]! * 100}%`,
                  top: `${position()[1]! * 100}%`,
                  width: `${size()[0] * 100}%`,
                  height: `${size()[1] * 100}%`,
                }}
              >
                <button
                  type="button"
                  class="flex size-full items-start overflow-hidden rounded-md border-2 px-1.5 py-0.5 text-left text-[11px] leading-tight font-semibold"
                  classList={{
                    "border-leaf bg-leaf/25 text-ink": selected(),
                    "border-ink-2/60 bg-ink/10 text-ink-2": !selected(),
                    "border-dashed": !widget()!.enabled,
                    "cursor-move": !props.disabled && !layout().positioningLock,
                  }}
                  onPointerDown={(event) => startMove(event, index, layout())}
                >
                  <span class="truncate">{widget()!.name}</span>
                </button>
                <Show
                  when={
                    selected() &&
                    hasSize(widget()!.type) &&
                    !props.disabled &&
                    !layout().positioningLock
                  }
                >
                  <span
                    aria-hidden="true"
                    class="absolute size-3.5 rounded-sm border-2 border-card bg-leaf"
                    classList={{
                      "right-0.5": corner().right,
                      "left-0.5": !corner().right,
                      "bottom-0.5": corner().bottom,
                      "top-0.5": !corner().bottom,
                      "cursor-nwse-resize": corner().right === corner().bottom,
                      "cursor-nesw-resize": corner().right !== corner().bottom,
                    }}
                    onPointerDown={(event) => startResize(event, index, layout())}
                  />
                </Show>
              </div>
            </Show>
          );
        }}
      </Index>
    </div>
  );
}

function NumberField(props: {
  label: string;
  value: number;
  disabled: boolean;
  min?: number;
  onChange: (value: number) => void;
}) {
  return (
    <label class={labelClass}>
      {props.label}
      <input
        type="number"
        step="0.1"
        min={props.min ?? 0}
        max="100"
        value={props.value}
        disabled={props.disabled}
        onChange={(event) => {
          const value = Number(event.currentTarget.value);
          if (Number.isFinite(value)) {
            props.onChange(Math.max(props.min ?? 0, Math.min(100, value)));
          }
        }}
        class={smallInputClass}
      />
    </label>
  );
}

function SceneWidgetRow(props: {
  layout: SceneWidget;
  widget: Widget | undefined;
  index: number;
  count: number;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
  onChange: (layout: SceneWidget) => void;
  onMove: (by: number) => void;
  onRemove: () => void;
}) {
  const type = () => props.widget?.type ?? "Image";
  return (
    <li
      class="rounded-xl border-[1.5px]"
      classList={{ "border-leaf bg-leaf/10": props.selected, "border-line": !props.selected }}
      onFocusIn={() => props.onSelect()}
      onClick={() => props.onSelect()}
    >
      <div class="flex items-center gap-1 py-0.5 pr-0.5 pl-3">
        <button
          type="button"
          class="min-w-0 flex-1 truncate py-1.5 text-left"
          aria-pressed={props.selected}
        >
          <span class="font-semibold text-ink">{props.widget?.name ?? t("Unknown widget")}</span>
          <span class="ml-2 text-[13px] text-muted">
            {props.widget ? widgetTypeNames()[props.widget.type] : ""}
            <Show when={props.widget && !props.widget.enabled}> · {t("Off")}</Show>
          </span>
        </button>
        <MoveButtons
          index={props.index}
          count={props.count}
          disabled={props.disabled}
          onMove={props.onMove}
        />
        <button
          type="button"
          class={iconButtonClass}
          aria-label={t("Remove from scene")}
          title={t("Remove from scene")}
          disabled={props.disabled}
          onClick={() => props.onRemove()}
        >
          <Icon name="close" class="size-[18px]" />
        </button>
      </div>
      <Show when={props.selected && hasPosition(type())}>
        <div class="grid grid-cols-2 gap-2 px-3 pb-3 @md:grid-cols-4">
          <NumberField
            label={t("X (%)")}
            value={props.layout.x}
            disabled={props.disabled}
            onChange={(x) => props.onChange({ ...props.layout, x })}
          />
          <NumberField
            label={t("Y (%)")}
            value={props.layout.y}
            disabled={props.disabled}
            onChange={(y) => props.onChange({ ...props.layout, y })}
          />
          <Show when={hasSize(type())}>
            <NumberField
              label={t("Size (%)")}
              value={props.layout.size}
              min={1}
              disabled={props.disabled}
              onChange={(size) => props.onChange({ ...props.layout, size })}
            />
          </Show>
          <label class={labelClass}>
            {t("Alignment")}
            <select
              value={props.layout.alignment}
              disabled={props.disabled}
              onChange={(event) =>
                props.onChange({
                  ...props.layout,
                  alignment: event.currentTarget.value as Alignment,
                })
              }
              class={smallInputClass}
            >
              <For each={alignments}>
                {(alignment) => <option value={alignment}>{alignmentNames()[alignment]}</option>}
              </For>
            </select>
          </label>
          <label class={`${checkboxLabelClass} col-span-full`}>
            <input
              type="checkbox"
              checked={props.layout.positioningLock ?? false}
              disabled={props.disabled}
              onChange={(event) =>
                props.onChange({ ...props.layout, positioningLock: event.currentTarget.checked })
              }
              class="size-4 accent-moss"
            />
            {t("Lock position")}
          </label>
        </div>
      </Show>
    </li>
  );
}

function SaveButtons(props: {
  label: string;
  dirty: boolean;
  disabled: boolean;
  valid: boolean;
  onDiscard: () => void;
}) {
  return (
    <div class="flex flex-wrap gap-2">
      <button
        type="submit"
        class={smallButtonClass}
        disabled={props.disabled || !props.dirty || !props.valid}
      >
        {props.label}
      </button>
      <button
        type="button"
        class={smallGhostButtonClass}
        disabled={props.disabled || !props.dirty}
        onClick={() => props.onDiscard()}
      >
        {t("Discard changes")}
      </button>
    </div>
  );
}

function SceneForm(props: {
  scene: Scene;
  config: SceneConfig;
  dirty: boolean;
  disabled: boolean;
  onChange: (scene: Scene) => void;
  onSave: () => void;
  onDiscard: () => void;
  children: JSX.Element;
}) {
  const [selected, setSelected] = createSignal<string | null>(null);
  const [adding, setAdding] = createSignal("");
  const camera = () => cameraOf(props.scene, props.config.cameras);
  const widgetOf = (layout: SceneWidget) =>
    props.config.widgets.find((widget) => sameId(widget.id, layout.widgetId));
  const addable = () =>
    props.config.widgets.filter(
      (widget) => !props.scene.widgets.some((layout) => sameId(layout.widgetId, widget.id)),
    );
  const setWidgets = (widgets: SceneWidget[]) => props.onChange({ ...props.scene, widgets });

  return (
    <form
      class="@container flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        props.onSave();
      }}
    >
      <div class="flex flex-col gap-2">
        <SceneCanvas
          scene={props.scene}
          widgets={props.config.widgets}
          selected={selected()}
          disabled={props.disabled}
          onSelect={setSelected}
          onMove={(index, layout) =>
            setWidgets(props.scene.widgets.map((old, at) => (at === index ? layout : old)))
          }
        />
        <p class="text-[13px] text-muted">
          {t(
            "Drag a widget to move it, or its corner handle to resize it. The boxes show where widgets are on the stream, not their exact shape.",
          )}
        </p>
      </div>
      <div class="grid gap-4 @2xl:grid-cols-2 @2xl:items-start">
        {props.children}
        <div class="@container flex min-w-0 flex-col gap-2">
          <h3 class="text-[13px] font-semibold text-muted">{t("Widgets in this scene")}</h3>
          <Show
            when={props.scene.widgets.length > 0}
            fallback={<p class="text-[15px] text-muted">{t("No widgets in this scene yet.")}</p>}
          >
            <ul class="flex flex-col gap-1.5">
              <Index each={props.scene.widgets}>
                {(layout, index) => (
                  <SceneWidgetRow
                    layout={layout()}
                    widget={widgetOf(layout())}
                    index={index}
                    count={props.scene.widgets.length}
                    selected={selected() === layout().id}
                    disabled={props.disabled}
                    onSelect={() => setSelected(layout().id)}
                    onChange={(changed) =>
                      setWidgets(
                        props.scene.widgets.map((old, at) => (at === index ? changed : old)),
                      )
                    }
                    onMove={(by) => setWidgets(moved(props.scene.widgets, index, by))}
                    onRemove={() => setWidgets(props.scene.widgets.filter((_, at) => at !== index))}
                  />
                )}
              </Index>
            </ul>
          </Show>
          <Show when={addable().length > 0}>
            <div class="flex gap-2">
              <select
                aria-label={t("Widget to add")}
                value={adding()}
                disabled={props.disabled}
                onChange={(event) => setAdding(event.currentTarget.value)}
                class={`${smallInputClass} min-w-0 flex-1`}
              >
                <option value="">{t("Pick a widget to add")}</option>
                <For each={addable()}>
                  {(widget) => (
                    <option value={widget.id}>
                      {widget.name} ({widgetTypeNames()[widget.type] ?? widget.type})
                    </option>
                  )}
                </For>
              </select>
              <button
                type="button"
                class={smallGhostButtonClass}
                disabled={props.disabled || !adding()}
                onClick={() => {
                  const widget = addable().find((candidate) => candidate.id === adding());
                  if (widget) {
                    const layout = newSceneWidget(widget);
                    setWidgets([...props.scene.widgets, layout]);
                    setSelected(layout.id);
                    setAdding("");
                  }
                }}
              >
                {t("Add")}
              </button>
            </div>
          </Show>
        </div>
      </div>
      <div class="grid items-end gap-3 border-t-[1.5px] border-line pt-4 @xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
        <label class={labelClass}>
          {t("Name")}
          <input
            type="text"
            value={props.scene.name}
            maxLength={100}
            disabled={props.disabled}
            onInput={(event) => props.onChange({ ...props.scene, name: event.currentTarget.value })}
            class={smallInputClass}
          />
        </label>
        <label class={labelClass}>
          {t("Camera")}
          <select
            value={camera()?.id ?? ""}
            disabled={props.disabled}
            onChange={(event) => {
              const picked = props.config.cameras.find(
                (candidate) => candidate.id === event.currentTarget.value,
              );
              if (picked) {
                props.onChange(withCamera(props.scene, picked));
              }
            }}
            class={smallInputClass}
          >
            <Show when={!camera()}>
              <option value="">{props.scene.cameraPosition ?? t("Unknown camera")}</option>
            </Show>
            <For each={props.config.cameras}>
              {(candidate) => <option value={candidate.id}>{candidate.name}</option>}
            </For>
          </select>
        </label>
        <label class={checkboxLabelClass}>
          <input
            type="checkbox"
            checked={props.scene.enabled}
            disabled={props.disabled}
            onChange={(event) =>
              props.onChange({ ...props.scene, enabled: event.currentTarget.checked })
            }
            class="size-4 accent-moss"
          />
          {t("Shown in Moblin's scene picker")}
        </label>
      </div>
      <SaveButtons
        label={t("Save scene")}
        dirty={props.dirty}
        disabled={props.disabled}
        valid={!!props.scene.name.trim()}
        onDiscard={props.onDiscard}
      />
    </form>
  );
}

function WidgetForm(props: {
  widget: Widget;
  config: SceneConfig;
  dirty: boolean;
  disabled: boolean;
  onChange: (widget: Widget) => void;
  onSave: () => void;
  onDiscard: () => void;
  onImage: (file: File) => void;
}) {
  const usedIn = () =>
    props.config.scenes.filter((scene) =>
      scene.widgets.some((layout) => sameId(layout.widgetId, props.widget.id)),
    );
  const part = <K extends "text" | "browser" | "qrCode">(key: K): NonNullable<Widget[K]> =>
    props.widget[key] ?? ({} as NonNullable<Widget[K]>);
  const setPart = <K extends "text" | "browser" | "qrCode">(key: K, changes: Widget[K]) =>
    props.onChange({ ...props.widget, [key]: { ...part(key), ...changes } });

  return (
    <form
      class="@container flex flex-col gap-4 border-t-[1.5px] border-line pt-4"
      onSubmit={(event) => {
        event.preventDefault();
        props.onSave();
      }}
    >
      <div class="grid items-end gap-3 @xl:grid-cols-[minmax(0,1fr)_auto]">
        <label class={labelClass}>
          {t("Name")}
          <input
            type="text"
            value={props.widget.name}
            maxLength={100}
            disabled={props.disabled}
            onInput={(event) =>
              props.onChange({ ...props.widget, name: event.currentTarget.value })
            }
            class={smallInputClass}
          />
        </label>
        <label class={checkboxLabelClass}>
          <input
            type="checkbox"
            checked={props.widget.enabled}
            disabled={props.disabled}
            onChange={(event) =>
              props.onChange({ ...props.widget, enabled: event.currentTarget.checked })
            }
            class="size-4 accent-moss"
          />
          {t("Shown on the stream")}
        </label>
      </div>
      <p class="-mt-2 text-[13px] text-muted">
        {usedIn().length === 0
          ? t("Not in any scene.")
          : t("In {scenes}.", {
              scenes: usedIn()
                .map((scene) => scene.name)
                .join(", "),
            })}
      </p>
      <Show when={props.widget.type === "Text"}>
        <div class="grid gap-3 @xl:grid-cols-[minmax(0,1fr)_7rem]">
          <label class={labelClass}>
            {t("Text")}
            <textarea
              rows="2"
              value={part("text").formatString ?? ""}
              disabled={props.disabled}
              onInput={(event) => setPart("text", { formatString: event.currentTarget.value })}
              class={smallInputClass.replace("h-10", "py-2")}
            />
            <span>
              {t("Moblin replaces placeholders like {time}, {date}, {speed} and {bitrate}.", {
                time: "{time}",
                date: "{date}",
                speed: "{speed}",
                bitrate: "{bitrate}",
              })}
            </span>
          </label>
          <label class={`${labelClass} max-w-28 self-start`}>
            {t("Font size")}
            <input
              type="number"
              min="10"
              max="200"
              value={part("text").fontSize ?? 30}
              disabled={props.disabled}
              onChange={(event) =>
                setPart("text", {
                  fontSize: Math.max(
                    10,
                    Math.min(200, Math.round(Number(event.currentTarget.value) || 30)),
                  ),
                })
              }
              class={smallInputClass}
            />
          </label>
        </div>
      </Show>
      <Show when={props.widget.type === "Browser"}>
        <div class="grid grid-cols-2 gap-3 @xl:grid-cols-[minmax(0,1fr)_7rem_7rem]">
          <label class={`${labelClass} col-span-2 @xl:col-span-1`}>
            {t("URL")}
            <input
              type="url"
              value={part("browser").url ?? ""}
              disabled={props.disabled}
              placeholder="https://"
              onInput={(event) => setPart("browser", { url: event.currentTarget.value })}
              class={smallInputClass}
            />
          </label>
          <For each={["width", "height"] as const}>
            {(key) => (
              <label class={labelClass}>
                {key === "width" ? t("Width") : t("Height")}
                <input
                  type="number"
                  min="1"
                  max="4000"
                  value={part("browser")[key] ?? 500}
                  disabled={props.disabled}
                  onChange={(event) =>
                    setPart("browser", {
                      [key]: Math.max(
                        1,
                        Math.min(4000, Math.round(Number(event.currentTarget.value) || 500)),
                      ),
                    })
                  }
                  class={smallInputClass}
                />
              </label>
            )}
          </For>
        </div>
      </Show>
      <Show when={props.widget.type === "QR code"}>
        <label class={labelClass}>
          {t("Message")}
          <input
            type="text"
            value={part("qrCode").message ?? ""}
            disabled={props.disabled}
            onInput={(event) => setPart("qrCode", { message: event.currentTarget.value })}
            class={smallInputClass}
          />
        </label>
      </Show>
      <Show when={props.widget.type === "Image"}>
        <label class={labelClass}>
          {t("Image")}
          <input
            type="file"
            accept="image/png,image/jpeg,image/gif,image/webp,image/heic"
            disabled={props.disabled}
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (file) {
                props.onImage(file);
              }
            }}
            class="text-[15px] text-ink-2 file:mr-3 file:h-10 file:rounded-full file:border-0 file:bg-ghost file:px-4 file:font-semibold file:text-ink disabled:opacity-60"
          />
          <span>{t("At most 4 MB. It replaces the image at once.")}</span>
        </label>
      </Show>
      <Show when={!["Text", "Browser", "QR code", "Image"].includes(props.widget.type)}>
        <p class="text-[13px] text-muted">
          {t("The other settings of this widget are changed in Moblin.")}
        </p>
      </Show>
      <SaveButtons
        label={t("Save widget")}
        dirty={props.dirty}
        disabled={props.disabled}
        valid={!!props.widget.name.trim()}
        onDiscard={props.onDiscard}
      />
    </form>
  );
}

function ItemRow(props: {
  name: string;
  detail: string;
  index: number;
  count: number;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
  onMove: (by: number) => void;
  onDelete: () => void;
}) {
  return (
    <li
      class="flex items-center gap-1 rounded-xl border-[1.5px] py-0.5 pr-0.5 pl-3"
      classList={{ "border-leaf bg-leaf/10": props.selected, "border-line": !props.selected }}
    >
      <button
        type="button"
        class="min-w-0 flex-1 truncate py-1.5 text-left"
        aria-pressed={props.selected}
        onClick={() => props.onSelect()}
      >
        <span class="font-semibold text-ink">{props.name}</span>
        <span class="ml-2 text-[13px] text-muted">{props.detail}</span>
      </button>
      <MoveButtons
        index={props.index}
        count={props.count}
        disabled={props.disabled}
        onMove={props.onMove}
      />
      <button
        type="button"
        class={iconButtonClass}
        aria-label={t("Delete")}
        title={t("Delete")}
        disabled={props.disabled}
        onClick={() => props.onDelete()}
      >
        <Icon name="trash" class="size-[18px]" />
      </button>
    </li>
  );
}

function createDraft<T extends { id: string }>(items: () => T[] | undefined) {
  const [id, setId] = createSignal<string | null>(null);
  const [original, setOriginal] = createSignal<T | null>(null);
  const [draft, setDraft] = createSignal<T | null>(null);
  const dirty = () => draft() !== null && !same(draft(), original());
  const select = (item: T | null) => {
    setId(item?.id ?? null);
    setOriginal(() => item && copy(item));
    setDraft(() => item && copy(item));
  };
  createEffect(
    on(items, (fresh) => {
      const selected = id();
      if (!fresh || selected === null) {
        return;
      }
      const item = fresh.find((candidate) => sameId(candidate.id, selected));
      if (!item) {
        if (!dirty()) {
          select(null);
        }
      } else if (!dirty()) {
        select(item);
      } else {
        setOriginal(() => copy(item));
      }
    }),
  );
  return {
    id,
    draft,
    setDraft,
    dirty,
    select,
    saved: () => setOriginal(() => draft() && copy(draft()!)),
    discard: () => setDraft(() => original() && copy(original()!)),
  };
}

export default function SceneEditor(props: {
  session: string;
  page: string;
  device: string;
  connected: boolean;
  version: number;
  enabled: boolean;
  current: string | undefined;
  onSignOut: () => void;
}) {
  const [config, setConfig] = createSignal<SceneConfig | null>(null);
  const [loadError, setLoadError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [newName, setNewName] = createSignal("");
  const [newType, setNewType] = createSignal<WidgetType>("Text");
  const scene = createDraft(() => config()?.scenes);
  const widget = createDraft(() => config()?.widgets);
  let loads = 0;

  const failed = (error: unknown) => {
    if (error instanceof BackendError && error.status === 401) {
      props.onSignOut();
      return null;
    }
    return sceneErrorText(error);
  };

  const load = async () => {
    const load = ++loads;
    try {
      const fresh = await deviceScenes(props.session, props.page, props.device);
      if (load === loads) {
        setConfig(fresh);
        setLoadError(null);
      }
    } catch (error) {
      if (load === loads) {
        setLoadError(failed(error));
      }
    }
  };

  createEffect(
    on(
      () => [props.connected, props.enabled, props.version, props.device] as const,
      ([connected, enabled]) => {
        if (connected && enabled) {
          void load();
        }
      },
    ),
  );

  createEffect(
    on(
      () => props.device,
      () => {
        setConfig(null);
        scene.select(null);
        widget.select(null);
      },
      { defer: true },
    ),
  );

  const run = async (control: Control, after?: () => void) => {
    setBusy(true);
    setError(null);
    try {
      await controlDevice(props.session, props.page, props.device, control);
      after?.();
      await load();
      return true;
    } catch (error) {
      setError(failed(error));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const disabled = () => busy() || !props.connected || !props.enabled;

  const newScene = () => {
    const cameras = config()?.cameras ?? [];
    const base: Scene = {
      id: newId(),
      name: t("My scene"),
      enabled: true,
      cameraPosition: "Back",
      widgets: [],
    };
    const back = cameras.find((camera) => camera.position === "Back") ?? cameras[0];
    const created = back ? withCamera(base, back) : base;
    void run({ createScene: { scene: created } }, () => scene.select(created));
  };

  const createWidget = (event: SubmitEvent) => {
    event.preventDefault();
    const name = newName().trim();
    if (!name) {
      return;
    }
    const created: Widget = { id: newId(), name, type: newType(), enabled: true };
    void run({ createWidget: { widget: created } }, () => {
      setNewName("");
      widget.select(created);
    });
  };

  const deleteScene = async (item: Scene) => {
    if (await ask(t("Delete the scene {name}?", { name: item.name }), t("Delete"))) {
      void run({ deleteScene: { id: item.id } });
    }
  };

  const deleteWidget = async (item: Widget) => {
    if (
      await ask(
        t("Delete the widget {name}? It is removed from every scene.", { name: item.name }),
        t("Delete"),
      )
    ) {
      void run({ deleteWidget: { id: item.id } });
    }
  };

  const uploadImage = async (file: File) => {
    if (file.size > 4 * 1024 * 1024) {
      setError(t("The image is too big. It can be at most 4 MB."));
      return;
    }
    const id = widget.id();
    if (id) {
      void run({ setWidgetImage: { id, image: await readBase64(file) } });
    }
  };

  createEffect(() => {
    const scenes = config()?.scenes;
    if (scenes && scenes.length > 0 && scene.id() === null) {
      scene.select(
        scenes.find((item) => props.current && sameId(props.current, item.id)) ?? scenes[0]!,
      );
    }
  });

  const widgetCounts = createMemo(() => {
    const counts = new Map<string, number>();
    for (const item of config()?.scenes ?? []) {
      counts.set(item.id, item.widgets.length);
    }
    return counts;
  });

  return (
    <>
      <Show when={!props.enabled}>
        <div class={cardClass}>
          <h2 class="text-[23px] font-semibold">{t("Scenes")}</h2>
          <p class="text-[15px] text-muted">{t("The streamer has not let you control Moblin.")}</p>
        </div>
      </Show>
      <Show when={props.enabled}>
        <Show when={error()}>
          {(error) => (
            <div class="mb-6">
              <Notice error onClose={() => setError(null)}>
                {error()}
              </Notice>
            </div>
          )}
        </Show>
        <Show
          when={config()}
          fallback={
            <div class={cardClass}>
              <h2 class="text-[23px] font-semibold">{t("Scenes")}</h2>
              <Show
                when={loadError()}
                fallback={
                  <Show
                    when={props.connected}
                    fallback={
                      <p class="text-[15px] text-muted">
                        {t("Scenes can be edited while Moblin is connected.")}
                      </p>
                    }
                  >
                    <Spinner />
                  </Show>
                }
              >
                {(loadError) => (
                  <div class="flex flex-col items-start gap-4">
                    <Notice error>{loadError()}</Notice>
                    <button
                      type="button"
                      class={ghostButtonClass}
                      disabled={!props.connected}
                      onClick={() => void load()}
                    >
                      <Icon name="reload" class="size-5" />
                      {t("Try again")}
                    </button>
                  </div>
                )}
              </Show>
            </div>
          }
        >
          {(config) => {
            const sceneList = () => (
              <ul class="flex flex-col gap-1.5">
                <For each={config().scenes}>
                  {(item, index) => (
                    <ItemRow
                      name={item.name}
                      detail={[
                        props.current && sameId(props.current, item.id) ? t("Shown now") : null,
                        item.enabled ? null : t("Off"),
                        cameraOf(item, config().cameras)?.name ?? item.cameraPosition,
                        widgetCounts().get(item.id) === 1
                          ? t("1 widget")
                          : t("{count} widgets", {
                              count: String(widgetCounts().get(item.id) ?? 0),
                            }),
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                      index={index()}
                      count={config().scenes.length}
                      selected={scene.id() !== null && sameId(scene.id()!, item.id)}
                      disabled={disabled()}
                      onSelect={() => {
                        if (!sameId(scene.id() ?? "", item.id)) {
                          scene.select(item);
                        }
                      }}
                      onMove={(by) =>
                        void run({
                          reorderScenes: {
                            ids: moved(config().scenes, index(), by).map((scene) => scene.id),
                          },
                        })
                      }
                      onDelete={() => void deleteScene(item)}
                    />
                  )}
                </For>
              </ul>
            );
            return (
              <div class="flex flex-col gap-6">
                <div class={cardClass}>
                  <div class="flex flex-wrap items-center justify-between gap-3">
                    <h2 class="text-[23px] font-semibold">{t("Scenes")}</h2>
                    <button
                      type="button"
                      class={smallGhostButtonClass}
                      disabled={disabled()}
                      onClick={newScene}
                    >
                      <Icon name="screen" class="size-[18px]" />
                      {t("New scene")}
                    </button>
                  </div>
                  <p class="-mt-2 text-[15px] text-ink-2">
                    {t(
                      "The scenes on the device, in the order Moblin shows them. Changes are saved on the device right away.",
                    )}
                  </p>
                  <Show
                    when={scene.draft()}
                    fallback={
                      <div class="@container flex flex-col gap-4">
                        <div class="aspect-video w-full rounded-xl border-[1.5px] border-line bg-well" />
                        <div class="grid gap-4 @2xl:grid-cols-2">{sceneList()}</div>
                      </div>
                    }
                  >
                    {(draft) => (
                      <SceneForm
                        scene={draft()}
                        config={config()}
                        dirty={scene.dirty()}
                        disabled={disabled()}
                        onChange={scene.setDraft}
                        onSave={() =>
                          void run(
                            { updateScene: { scene: { ...draft(), name: draft().name.trim() } } },
                            scene.saved,
                          )
                        }
                        onDiscard={scene.discard}
                      >
                        {sceneList()}
                      </SceneForm>
                    )}
                  </Show>
                </div>
                <div class={cardClass}>
                  <h2 class="text-[23px] font-semibold">{t("Widgets")}</h2>
                  <p class="-mt-2 text-[15px] text-ink-2">
                    {t(
                      "Every widget on the device. A widget is shown in the scenes it is added to.",
                    )}
                  </p>
                  <ul class="flex flex-col gap-1.5">
                    <For each={config().widgets}>
                      {(item, index) => (
                        <ItemRow
                          name={item.name}
                          detail={[
                            widgetTypeNames()[item.type] ?? item.type,
                            item.enabled ? null : t("Off"),
                          ]
                            .filter(Boolean)
                            .join(" · ")}
                          index={index()}
                          count={config().widgets.length}
                          selected={widget.id() !== null && sameId(widget.id()!, item.id)}
                          disabled={disabled()}
                          onSelect={() =>
                            widget.select(sameId(widget.id() ?? "", item.id) ? null : item)
                          }
                          onMove={(by) =>
                            void run({
                              reorderWidgets: {
                                ids: moved(config().widgets, index(), by).map(
                                  (widget) => widget.id,
                                ),
                              },
                            })
                          }
                          onDelete={() => void deleteWidget(item)}
                        />
                      )}
                    </For>
                  </ul>
                  <Show when={widget.draft()}>
                    {(draft) => (
                      <WidgetForm
                        widget={draft()}
                        config={config()}
                        dirty={widget.dirty()}
                        disabled={disabled()}
                        onChange={widget.setDraft}
                        onSave={() =>
                          void run(
                            { updateWidget: { widget: { ...draft(), name: draft().name.trim() } } },
                            widget.saved,
                          )
                        }
                        onDiscard={widget.discard}
                        onImage={(file) => void uploadImage(file)}
                      />
                    )}
                  </Show>
                  <form
                    class="flex flex-col gap-2 border-t-[1.5px] border-line pt-4"
                    onSubmit={createWidget}
                  >
                    <h3 class="text-[13px] font-semibold text-muted">{t("New widget")}</h3>
                    <div class="flex flex-wrap gap-2">
                      <input
                        type="text"
                        value={newName()}
                        maxLength={100}
                        placeholder={t("Name")}
                        aria-label={t("Name")}
                        disabled={disabled()}
                        onInput={(event) => setNewName(event.currentTarget.value)}
                        class={`${smallInputClass} min-w-0 flex-1 basis-48`}
                      />
                      <select
                        aria-label={t("Type")}
                        value={newType()}
                        disabled={disabled()}
                        onChange={(event) => setNewType(event.currentTarget.value as WidgetType)}
                        class={`${smallInputClass} min-w-0 flex-1 basis-40`}
                      >
                        <For each={creatableWidgetTypes}>
                          {(type) => <option value={type}>{widgetTypeNames()[type]}</option>}
                        </For>
                      </select>
                      <button
                        type="submit"
                        class={smallButtonClass}
                        disabled={disabled() || !newName().trim()}
                      >
                        {t("Create")}
                      </button>
                    </div>
                  </form>
                </div>
              </div>
            );
          }}
        </Show>
      </Show>
    </>
  );
}
