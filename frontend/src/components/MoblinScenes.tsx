import { createEffect, createSignal, For, Match, on, Show, Switch } from "solid-js";
import Icon, { type IconName } from "./Icon";
import {
  Empty,
  Group,
  Loading,
  NavRow,
  PhoneSwitch,
  rowClass,
  SwipeRows,
  SwitchNavRow,
  TextButton,
} from "./MoblinParts";
import { type Control } from "../data/account";
import {
  alignments,
  cameraOf,
  canvasHeight,
  canvasWidth,
  creatableWidgetTypes,
  hasAlignment,
  hasPosition,
  hasSize,
  isHorizontalCenter,
  isLeft,
  isVerticalCenter,
  mirrorsHorizontally,
  mirrorsVertically,
  nestedSceneId,
  newId,
  newSceneWidget,
  clonedScene,
  sameId,
  sceneErrorText,
  sceneWidgetsOf,
  uniqueName,
  widgetIcons,
  widgetTypeDescriptions,
  widgetTypeNames,
  withCamera,
  type Alignment,
  type Scene,
  type SceneConfig,
  type SceneWidget,
  type Widget,
  type WidgetOutline,
  type WidgetType,
} from "../data/scenes";
import { t } from "../i18n";

export type ScenePage =
  | { kind: "scenes" }
  | { kind: "sceneWidgets" }
  | { kind: "scene"; id: string }
  | { kind: "sceneWidget"; sceneId: string; id: string }
  | { kind: "widget"; id: string }
  | { kind: "camera"; sceneId: string }
  | { kind: "wizard" }
  | { kind: "wizardScenes" }
  | {
      kind: "text";
      title: string;
      value: string;
      multiline?: boolean;
      placeholder?: string;
      check?: (value: string) => string | null;
      onSubmit: (value: string) => void;
    };

type Layout = Pick<SceneWidget, "x" | "y" | "size" | "alignment" | "positioningLock">;

type Wizard = { type: WidgetType; name: string; scenes: string[] };

const reordered = <T,>(items: T[], from: number, to: number) => {
  const result = [...items];
  const [item] = result.splice(from, 1);
  result.splice(to, 0, item!);
  return result;
};

const replaced = <T extends { id: string }>(items: T[], item: T) =>
  items.map((old) => (sameId(old.id, item.id) ? item : old));

export function createScenes(options: {
  device: () => string;
  active: () => boolean;
  allowed: () => boolean;
  connected: () => boolean;
  current: () => string | undefined;
  version: () => number;
  load: () => Promise<SceneConfig>;
  control: (control: Control) => Promise<void>;
}) {
  const [config, setConfig] = createSignal<SceneConfig | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [clipboard, setClipboard] = createSignal<Layout | null>(null);
  const [wizard, setWizard] = createSignal<Wizard>({ type: "Text", name: "", scenes: [] });
  let loads = 0;
  let edits = 0;
  let sending = 0;

  const load = async () => {
    const load = ++loads;
    const at = edits;
    try {
      const fresh = await options.load();
      if (load === loads && at === edits && sending === 0) {
        setConfig(fresh);
        setError(null);
      }
    } catch (error) {
      if (load === loads) {
        setError(sceneErrorText(error));
      }
    }
  };

  createEffect(
    on(options.device, () => {
      setConfig(null);
      setError(null);
    }),
  );

  createEffect(
    on(
      () => [options.active() && options.allowed() && options.connected(), options.version()],
      ([active]) => {
        if (active) {
          void load();
        }
      },
    ),
  );

  const edit = (change: (config: SceneConfig) => SceneConfig) => {
    edits += 1;
    setConfig((current) => current && change(current));
  };

  const send = async (control: Control, change?: (config: SceneConfig) => SceneConfig) => {
    if (change) {
      edit(change);
    }
    sending += 1;
    try {
      await options.control(control);
    } finally {
      sending -= 1;
      if (sending === 0) {
        void load();
      }
    }
  };

  return {
    config,
    error,
    clipboard,
    setClipboard,
    wizard,
    setWizard,
    allowed: options.allowed,
    current: options.current,
    disabled: () => !options.connected() || !options.allowed(),
    scene: (id: string) => config()?.scenes.find((scene) => sameId(scene.id, id)),
    widget: (id: string) => config()?.widgets.find((widget) => sameId(widget.id, id)),
    previewScene: (scene: Scene) =>
      edit((current) => ({ ...current, scenes: replaced(current.scenes, scene) })),
    updateScene: (scene: Scene) =>
      send({ updateScene: { scene } }, (current) => ({
        ...current,
        scenes: replaced(current.scenes, scene),
      })),
    updateWidget: (widget: Widget) =>
      send({ updateWidget: { widget } }, (current) => ({
        ...current,
        widgets: replaced(current.widgets, widget),
      })),
    createScene: (scene: Scene) =>
      send({ createScene: { scene } }, (current) => ({
        ...current,
        scenes: [...current.scenes, scene],
      })),
    deleteScene: (id: string) =>
      send({ deleteScene: { id } }, (current) => ({
        ...current,
        scenes: current.scenes.filter((scene) => !sameId(scene.id, id)),
      })),
    moveScene: (from: number, to: number) => {
      const scenes = reordered(config()?.scenes ?? [], from, to);
      return send({ reorderScenes: { ids: scenes.map((scene) => scene.id) } }, (current) => ({
        ...current,
        scenes,
      }));
    },
    createWidget: (widget: Widget) =>
      send({ createWidget: { widget } }, (current) => ({
        ...current,
        widgets: [...current.widgets, widget],
      })),
    deleteWidget: (id: string) =>
      send({ deleteWidget: { id } }, (current) => ({
        ...current,
        widgets: current.widgets.filter((widget) => !sameId(widget.id, id)),
        scenes: current.scenes.map((scene) => ({
          ...scene,
          widgets: scene.widgets.filter((layout) => !sameId(layout.widgetId, id)),
        })),
      })),
    moveWidget: (from: number, to: number) => {
      const widgets = reordered(config()?.widgets ?? [], from, to);
      return send({ reorderWidgets: { ids: widgets.map((widget) => widget.id) } }, (current) => ({
        ...current,
        widgets,
      }));
    },
    setImage: (id: string, image: string) => send({ setWidgetImage: { id, image } }),
  };
}

export type Scenes = ReturnType<typeof createScenes>;

export const scenePageTitle = (page: ScenePage, scenes: Scenes): string => {
  switch (page.kind) {
    case "scenes":
      return t("Scenes");
    case "sceneWidgets":
      return t("Scene widgets");
    case "scene":
      return t("Scene");
    case "sceneWidget": {
      const layout = scenes.scene(page.sceneId)?.widgets.find((item) => sameId(item.id, page.id));
      return (layout && scenes.widget(layout.widgetId)?.name) ?? t("Widget");
    }
    case "widget": {
      const widget = scenes.widget(page.id);
      return widget
        ? t("{type} widget", { type: widgetTypeNames()[widget.type] ?? widget.type })
        : t("Widget");
    }
    case "camera":
      return t("Name");
    case "wizard":
      return t("Create widget wizard");
    case "wizardScenes":
      return t("Scenes to add the widget to");
    case "text":
      return page.title;
  }
};

const nameCheck = (existing: () => { name: string }[], current: string) => (value: string) =>
  value === ""
    ? t("Empty names are not allowed.")
    : value !== current && existing().some((item) => item.name === value)
      ? t("The name '{name}' is already in use.", { name: value })
      : null;

const readBase64 = (file: File) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });

function PickerRow(props: {
  label: string;
  value: string;
  options: { id: string; name: string }[];
  disabled: boolean;
  onPick: (id: string) => void;
}) {
  return (
    <label class={`${rowClass} ${props.disabled ? "opacity-50" : ""}`}>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <span class="relative flex max-w-[60%] items-center gap-1 text-phone-border">
        <span class="truncate">
          {props.options.find((option) => option.id === props.value)?.name ?? ""}
        </span>
        <Icon name="chevron-down" class="size-[14px] shrink-0" />
        <select
          aria-label={props.label}
          class="absolute inset-0 cursor-pointer opacity-0"
          value={props.value}
          disabled={props.disabled}
          onChange={(event) => props.onPick(event.currentTarget.value)}
        >
          <For each={props.options}>
            {(option) => <option value={option.id}>{option.name}</option>}
          </For>
        </select>
      </span>
    </label>
  );
}

function TextPage(props: { page: Extract<ScenePage, { kind: "text" }>; onDone: () => void }) {
  const [value, setValue] = createSignal(props.page.value);
  let submitted = props.page.value;
  const problem = () => props.page.check?.(value().trim()) ?? null;
  const submit = () => {
    const trimmed = value().trim();
    if (problem() === null && trimmed !== submitted) {
      submitted = trimmed;
      props.page.onSubmit(trimmed);
    }
  };
  const field = "w-full bg-transparent px-4 py-2.5 text-[17px] outline-none";
  return (
    <Group error={problem()}>
      <Show
        when={props.page.multiline}
        fallback={
          <input
            type="text"
            ref={(element) => setTimeout(() => element.focus())}
            class={field}
            value={value()}
            placeholder={props.page.placeholder}
            onInput={(event) => setValue(event.currentTarget.value)}
            onChange={submit}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                submit();
                props.onDone();
              }
            }}
          />
        }
      >
        <textarea
          rows="6"
          ref={(element) => setTimeout(() => element.focus())}
          class={`${field} resize-none`}
          value={value()}
          placeholder={props.page.placeholder}
          onInput={(event) => setValue(event.currentTarget.value)}
          onChange={submit}
        />
      </Show>
    </Group>
  );
}

function ValueEdit(props: {
  label: string;
  value: number;
  min: number;
  max: number;
  increment: number;
  mirror: boolean;
  numeric: boolean;
  decrementIcon: IconName;
  incrementIcon: IconName;
  disabled: boolean;
  onInput: (value: number) => void;
  onChange: (value: number) => void;
}) {
  const clamp = (value: number) =>
    Math.round(Math.max(props.min, Math.min(props.max, value)) * 1e6) / 1e6;
  const step = (by: number) => props.onChange(clamp(props.value + (props.mirror ? -by : by)));
  const iconButton =
    "grid size-[30px] shrink-0 place-items-center text-phone-tint disabled:opacity-40";
  return (
    <div class="flex flex-1 items-center gap-2">
      <Show
        when={props.numeric}
        fallback={
          <input
            type="range"
            aria-label={props.label}
            class="min-w-0 flex-1 accent-phone-tint"
            style={{ transform: props.mirror ? "rotate(180deg)" : undefined }}
            min={props.min}
            max={props.max}
            step={15 * props.increment}
            value={props.value}
            disabled={props.disabled}
            onInput={(event) => props.onInput(clamp(Number(event.currentTarget.value)))}
            onChange={(event) => props.onChange(clamp(Number(event.currentTarget.value)))}
          />
        }
      >
        <input
          type="text"
          inputMode="decimal"
          aria-label={props.label}
          class="min-w-0 flex-1 bg-transparent text-[17px] outline-none"
          value={String(props.value)}
          disabled={props.disabled}
          onChange={(event) => {
            const value = Number(event.currentTarget.value.trim());
            if (Number.isFinite(value) && event.currentTarget.value.trim() !== "") {
              props.onChange(clamp(value));
            } else {
              event.currentTarget.value = String(props.value);
            }
          }}
        />
      </Show>
      <button
        type="button"
        aria-label={t("Decrease")}
        class={iconButton}
        disabled={props.disabled || props.value === (props.mirror ? props.max : props.min)}
        onClick={() => step(-props.increment)}
      >
        <Icon name={props.decrementIcon} class="size-[26px]" />
      </button>
      <button
        type="button"
        aria-label={t("Increase")}
        class={iconButton}
        disabled={props.disabled || props.value === (props.mirror ? props.min : props.max)}
        onClick={() => step(props.increment)}
      >
        <Icon name={props.incrementIcon} class="size-[26px]" />
      </button>
    </div>
  );
}

function LayoutGroup(props: { scenes: Scenes; scene: Scene; layout: SceneWidget; widget: Widget }) {
  const [numeric, setNumeric] = createSignal(false);
  const horizontal = 100 / canvasWidth;
  const vertical = 100 / canvasHeight;
  const disabled = () => props.scenes.disabled();
  const withLayout = (changes: Partial<Layout>) => ({
    ...props.scene,
    widgets: props.scene.widgets.map((layout) =>
      sameId(layout.id, props.layout.id) ? { ...layout, ...changes } : layout,
    ),
  });
  const locked = (changes: Partial<Layout>): Partial<Layout> => {
    if (!props.layout.positioningLock) {
      return changes;
    }
    if (changes.x !== undefined) {
      return { ...changes, y: (changes.x * vertical) / horizontal };
    }
    if (changes.y !== undefined) {
      return { ...changes, x: (changes.y * horizontal) / vertical };
    }
    return changes;
  };
  const preview = (changes: Partial<Layout>) =>
    props.scenes.previewScene(withLayout(locked(changes)));
  const save = (changes: Partial<Layout>) =>
    void props.scenes.updateScene(withLayout(locked(changes)));
  const alignment = () => props.layout.alignment;
  const position = (axis: "x" | "y") => (
    <ValueEdit
      label={axis === "x" ? t("Horizontal position") : t("Vertical position")}
      value={props.layout[axis]}
      min={0}
      max={100}
      increment={axis === "x" ? horizontal : vertical}
      mirror={axis === "x" ? mirrorsHorizontally(alignment()) : mirrorsVertically(alignment())}
      numeric={numeric()}
      decrementIcon={axis === "x" ? "arrow-left-circle" : "arrow-up-circle"}
      incrementIcon={axis === "x" ? "arrow-right-circle" : "arrow-down-circle"}
      disabled={disabled()}
      onInput={(value) => preview({ [axis]: value })}
      onChange={(value) => save({ [axis]: value })}
    />
  );
  const button =
    "rounded-[8px] bg-phone-separator px-3 py-1.5 text-[15px] text-phone-tint disabled:opacity-40";
  return (
    <Group
      header={t("Layout")}
      footer={t(
        "Use save/load layout to position a widget in the same place in multiple scenes. Alternatively, use a Scene widget to easily show the same widgets in multiple scenes.",
      )}
    >
      <div class="flex items-center gap-3 px-4 py-2">
        <div class="flex flex-1 flex-col gap-2">
          <button
            type="button"
            class={button}
            onClick={() => {
              const { x, y, size, alignment, positioningLock } = props.layout;
              props.scenes.setClipboard({ x, y, size, alignment, positioningLock });
            }}
          >
            {t("Save layout")}
          </button>
          <button
            type="button"
            class={button}
            disabled={disabled() || props.scenes.clipboard() === null}
            onClick={() => void props.scenes.updateScene(withLayout(props.scenes.clipboard()!))}
          >
            {t("Load layout")}
          </button>
        </div>
        <Show when={hasAlignment(props.widget.type)}>
          <span class="self-stretch border-l border-phone-separator" />
          <div class="grid grid-cols-3 gap-[3px]" role="radiogroup" aria-label={t("Alignment")}>
            <For each={alignments}>
              {(option: Alignment) => (
                <button
                  type="button"
                  role="radio"
                  aria-checked={alignment() === option}
                  aria-label={option}
                  class="text-phone-tint disabled:opacity-50"
                  disabled={disabled()}
                  onClick={() => option !== alignment() && save({ alignment: option })}
                >
                  <Icon
                    name={alignment() === option ? "square-fill" : "square"}
                    class="size-[30px]"
                  />
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
      <Show when={hasPosition(props.widget.type)}>
        <Switch>
          <Match when={!isHorizontalCenter(alignment()) && !isVerticalCenter(alignment())}>
            <div class="flex items-center gap-2 px-4 py-2">
              <div class="flex min-w-0 flex-1 flex-col gap-3">
                {position("x")}
                {position("y")}
              </div>
              <button
                type="button"
                aria-label={t("Lock position")}
                aria-pressed={props.layout.positioningLock ?? false}
                class="grid w-[35px] place-items-center text-phone-tint disabled:opacity-50"
                disabled={disabled()}
                onClick={() => {
                  const positioningLock = !props.layout.positioningLock;
                  void props.scenes.updateScene(
                    withLayout(
                      positioningLock
                        ? { positioningLock, y: (props.layout.x * vertical) / horizontal }
                        : { positioningLock },
                    ),
                  );
                }}
              >
                <Icon
                  name={props.layout.positioningLock ? "lock" : "lock-open"}
                  class="size-[28px]"
                />
              </button>
            </div>
          </Match>
          <Match when={!isHorizontalCenter(alignment())}>
            <div class="flex px-4 py-2">{position("x")}</div>
          </Match>
          <Match when={!isVerticalCenter(alignment())}>
            <div class="flex px-4 py-2">{position("y")}</div>
          </Match>
        </Switch>
      </Show>
      <Show when={hasSize(props.widget.type)}>
        <div class="flex px-4 py-2">
          <ValueEdit
            label={t("Size")}
            value={props.layout.size}
            min={1}
            max={100}
            increment={1 / 15}
            mirror={false}
            numeric={numeric()}
            decrementIcon="resize-down"
            incrementIcon="resize-up"
            disabled={disabled()}
            onInput={(size) => preview({ size })}
            onChange={(size) => save({ size })}
          />
        </div>
      </Show>
      <PhoneSwitch
        label={t("Numeric input")}
        on={numeric()}
        disabled={false}
        onToggle={setNumeric}
      />
    </Group>
  );
}

function ScenesPage(props: {
  scenes: Scenes;
  config: SceneConfig;
  push: (page: ScenePage) => void;
}) {
  const newScene = () => {
    const base: Scene = {
      id: newId(),
      name: uniqueName(t("My scene"), props.config.scenes),
      enabled: true,
      cameraPosition: "Back",
      widgets: [],
    };
    const back =
      props.config.cameras.find((camera) => camera.position === "Back") ?? props.config.cameras[0];
    void props.scenes.createScene(back ? withCamera(base, back) : base);
  };
  return (
    <>
      <Group header={t("Scenes")} footer={t("Swipe left on a scene to duplicate or delete it.")}>
        <SwipeRows
          items={props.config.scenes}
          key={(scene) => scene.id}
          disabled={props.scenes.disabled()}
          actions={(scene) => [
            { label: t("Delete"), tone: "red", run: () => void props.scenes.deleteScene(scene.id) },
            {
              label: t("Duplicate"),
              tone: "gray",
              run: () =>
                void props.scenes.createScene(
                  clonedScene(scene, uniqueName(scene.name, props.config.scenes)),
                ),
            },
          ]}
          onMove={(from, to) => void props.scenes.moveScene(from, to)}
        >
          {(scene, handle) => (
            <SwitchNavRow
              label={scene.name}
              on={scene.enabled}
              disabled={props.scenes.disabled()}
              handle={handle}
              onToggle={(enabled) => void props.scenes.updateScene({ ...scene, enabled })}
              onClick={() => props.push({ kind: "scene", id: scene.id })}
            />
          )}
        </SwipeRows>
        <TextButton label={t("Create")} disabled={props.scenes.disabled()} onClick={newScene} />
      </Group>
      <WidgetsGroup scenes={props.scenes} config={props.config} push={props.push} />
    </>
  );
}

function WidgetsGroup(props: {
  scenes: Scenes;
  config: SceneConfig;
  push: (page: ScenePage) => void;
}) {
  return (
    <Group
      header={t("Widgets")}
      footer={`${t("A widget can be used in zero or more scenes.")}\n\n${t(
        "Swipe left on a widget to delete it.",
      )}`}
    >
      <SwipeRows
        items={props.config.widgets}
        key={(widget) => widget.id}
        disabled={props.scenes.disabled()}
        actions={(widget) => [
          { label: t("Delete"), tone: "red", run: () => void props.scenes.deleteWidget(widget.id) },
        ]}
        onMove={(from, to) => void props.scenes.moveWidget(from, to)}
      >
        {(widget, handle) => (
          <SwitchNavRow
            label={widget.name}
            icon={widgetIcons[widget.type] ?? "sparkles"}
            on={widget.enabled}
            disabled={props.scenes.disabled()}
            handle={handle}
            onToggle={(enabled) => void props.scenes.updateWidget({ ...widget, enabled })}
            onClick={() => props.push({ kind: "widget", id: widget.id })}
          />
        )}
      </SwipeRows>
      <TextButton
        label={t("Create")}
        disabled={props.scenes.disabled()}
        onClick={() => {
          const current = props.scenes.current();
          props.scenes.setWizard({ type: "Text", name: "", scenes: current ? [current] : [] });
          props.push({ kind: "wizard" });
        }}
      />
    </Group>
  );
}

function ScenePageView(props: {
  scenes: Scenes;
  config: SceneConfig;
  scene: Scene;
  push: (page: ScenePage) => void;
}) {
  const [adding, setAdding] = createSignal(false);
  const camera = () => cameraOf(props.scene, props.config.cameras);
  const update = (changes: Partial<Scene>) =>
    void props.scenes.updateScene({ ...props.scene, ...changes });
  return (
    <>
      <Group>
        <NavRow
          label={t("Name")}
          value={props.scene.name}
          onClick={() =>
            props.push({
              kind: "text",
              title: t("Name"),
              value: props.scene.name,
              check: nameCheck(() => props.config.scenes, props.scene.name),
              onSubmit: (name) => update({ name }),
            })
          }
        />
      </Group>
      <Group header={t("Widgets")} footer={t("Swipe left on a widget to remove it.")}>
        <SwipeRows
          items={props.scene.widgets}
          key={(layout) => layout.id}
          disabled={props.scenes.disabled()}
          actions={(layout) => [
            {
              label: t("Delete"),
              tone: "red",
              run: () =>
                update({
                  widgets: props.scene.widgets.filter((item) => !sameId(item.id, layout.id)),
                }),
            },
          ]}
          onMove={(from, to) => update({ widgets: reordered(props.scene.widgets, from, to) })}
        >
          {(layout, handle) => {
            const widget = () => props.scenes.widget(layout.widgetId);
            return (
              <Show when={widget()}>
                {(widget) => (
                  <SwitchNavRow
                    label={widget().name}
                    icon={widgetIcons[widget().type] ?? "sparkles"}
                    on={widget().enabled}
                    disabled={props.scenes.disabled()}
                    handle={handle}
                    onToggle={(enabled) => void props.scenes.updateWidget({ ...widget(), enabled })}
                    onClick={() =>
                      props.push({ kind: "sceneWidget", sceneId: props.scene.id, id: layout.id })
                    }
                  />
                )}
              </Show>
            );
          }}
        </SwipeRows>
        <div class="relative">
          <TextButton
            label={t("Add")}
            disabled={props.scenes.disabled() || props.config.widgets.length === 0}
            onClick={() => setAdding(true)}
          />
          <Show when={adding()}>
            <div class="fixed inset-0 z-20" onClick={() => setAdding(false)} />
            <div class="absolute top-full left-1/2 z-30 max-h-[220px] min-w-[220px] -translate-x-1/2 overflow-y-auto rounded-[12px] bg-phone-row p-[5px] shadow-2xl ring-1 ring-phone-separator [scrollbar-width:none]">
              <For each={props.config.widgets}>
                {(widget) => (
                  <button
                    type="button"
                    class="flex w-full items-center gap-3 p-[11px] text-left text-[17px]"
                    onClick={() => {
                      setAdding(false);
                      update({ widgets: [...props.scene.widgets, newSceneWidget(widget)] });
                    }}
                  >
                    <Icon
                      name={widgetIcons[widget.type] ?? "sparkles"}
                      class="size-[18px] shrink-0 text-phone-tint"
                    />
                    <span class="truncate">{widget.name}</span>
                  </button>
                )}
              </For>
            </div>
          </Show>
        </div>
        <TextButton
          label={t("Create")}
          disabled={props.scenes.disabled()}
          onClick={() => {
            props.scenes.setWizard({ type: "Text", name: "", scenes: [props.scene.id] });
            props.push({ kind: "wizard" });
          }}
        />
      </Group>
      <Group header={t("Video source")}>
        <NavRow
          icon="camera"
          label={t("Name")}
          value={camera()?.name ?? props.scene.cameraPosition ?? ""}
          onClick={() => props.push({ kind: "camera", sceneId: props.scene.id })}
        />
      </Group>
      <Group footer={t("Switching between scenes in the same group may be instant.")}>
        <PickerRow
          label={t("Quick switch group")}
          value={String(props.scene.quickSwitchGroup ?? "")}
          options={[
            { id: "", name: t("-- None --") },
            ...[1, 2, 3, 4].map((group) => ({ id: String(group), name: String(group) })),
          ]}
          disabled={props.scenes.disabled()}
          onPick={(group) => update({ quickSwitchGroup: group === "" ? undefined : Number(group) })}
        />
      </Group>
    </>
  );
}

function WidgetPage(props: {
  scenes: Scenes;
  config: SceneConfig;
  widget: Widget;
  push: (page: ScenePage) => void;
}) {
  const [image, setImage] = createSignal<string | null>(null);
  let picker: HTMLInputElement | undefined;
  const part = <K extends "text" | "browser" | "qrCode">(key: K): NonNullable<Widget[K]> =>
    props.widget[key] ?? ({} as NonNullable<Widget[K]>);
  const update = (changes: Partial<Widget>) =>
    void props.scenes.updateWidget({ ...props.widget, ...changes });
  const textRow = (
    label: string,
    value: string,
    onSubmit: (value: string) => void,
    options: {
      multiline?: boolean;
      placeholder?: string;
      check?: (value: string) => string | null;
    } = {},
  ) => (
    <NavRow
      label={label}
      value={value}
      onClick={() => props.push({ kind: "text", title: label, value, onSubmit, ...options })}
    />
  );
  const sizeCheck = (value: string) => {
    const number = Number(value);
    return !/^-?\d+$/.test(value)
      ? t("Not a number")
      : number <= 0
        ? t("Too small")
        : number >= 4000
          ? t("Too big")
          : null;
  };
  const urlCheck = (value: string) => {
    if (value === "") {
      return null;
    }
    if (!/^[a-z][a-z0-9+.-]*:/i.test(value)) {
      return t("Scheme missing");
    }
    try {
      const url = new URL(value);
      if (url.protocol !== "http:" && url.protocol !== "https:") {
        return t("Unsupported scheme {scheme}", { scheme: url.protocol.slice(0, -1) });
      }
      return url.host ? null : t("Host missing");
    } catch {
      return t("Malformed URL");
    }
  };
  return (
    <>
      <Group>
        {textRow(t("Name"), props.widget.name, (name) => update({ name }), {
          check: nameCheck(() => props.config.widgets, props.widget.name),
        })}
      </Group>
      <Switch
        fallback={<Empty text={t("The other settings of this widget are changed in Moblin.")} />}
      >
        <Match when={props.widget.type === "Text"}>
          <Group>
            {textRow(
              t("Text"),
              part("text").formatString ?? "",
              (formatString) => update({ text: { ...part("text"), formatString } }),
              { multiline: true },
            )}
          </Group>
        </Match>
        <Match when={props.widget.type === "Browser"}>
          <Group>
            {textRow(
              t("URL"),
              part("browser").url ?? "",
              (url) => update({ browser: { ...part("browser"), url } }),
              { check: urlCheck, placeholder: "https://" },
            )}
            {textRow(
              t("Width"),
              String(part("browser").width ?? 500),
              (width) => update({ browser: { ...part("browser"), width: Number(width) } }),
              { check: sizeCheck },
            )}
            {textRow(
              t("Height"),
              String(part("browser").height ?? 500),
              (height) => update({ browser: { ...part("browser"), height: Number(height) } }),
              { check: sizeCheck },
            )}
          </Group>
        </Match>
        <Match when={props.widget.type === "QR code"}>
          <Group>
            {textRow(t("Message"), part("qrCode").message ?? "", (message) =>
              update({ qrCode: { ...part("qrCode"), message } }),
            )}
          </Group>
        </Match>
        <Match when={props.widget.type === "Image"}>
          <Group footer={t("At most 4 MB. It replaces the image at once.")}>
            <input
              ref={(element) => (picker = element)}
              type="file"
              accept="image/png,image/jpeg,image/gif,image/webp,image/heic"
              class="hidden"
              onChange={async (event) => {
                const file = event.currentTarget.files?.[0];
                event.currentTarget.value = "";
                if (!file || file.size > 4 * 1024 * 1024) {
                  return;
                }
                const data = await readBase64(file);
                setImage(`data:${file.type};base64,${data}`);
                void props.scenes.setImage(props.widget.id, data);
              }}
            />
            <button
              type="button"
              class={`${rowClass} justify-center text-phone-tint`}
              disabled={props.scenes.disabled()}
              onClick={() => picker?.click()}
            >
              <Show when={image()} fallback={t("Select image")}>
                {(source) => (
                  <img src={source()} alt="" class="aspect-video w-[192px] object-contain" />
                )}
              </Show>
            </button>
          </Group>
        </Match>
      </Switch>
    </>
  );
}

function WizardPage(props: {
  scenes: Scenes;
  config: SceneConfig;
  push: (page: ScenePage) => void;
}) {
  const wizard = () => props.scenes.wizard();
  const taken = () => props.config.widgets.some((widget) => widget.name === wizard().name);
  return (
    <>
      <Group footer={widgetTypeDescriptions()[wizard().type]}>
        <PickerRow
          label={t("Type")}
          value={wizard().type}
          options={creatableWidgetTypes.map((type) => ({
            id: type,
            name: widgetTypeNames()[type],
          }))}
          disabled={false}
          onPick={(type) => props.scenes.setWizard({ ...wizard(), type: type as WidgetType })}
        />
      </Group>
      <Group
        header={t("Name")}
        error={taken() ? t("The name '{name}' is already in use.", { name: wizard().name }) : null}
      >
        <input
          type="text"
          class="w-full bg-transparent px-4 py-2.5 text-[17px] outline-none placeholder:text-phone-border"
          placeholder={t("My widget")}
          value={wizard().name}
          onInput={(event) =>
            props.scenes.setWizard({ ...wizard(), name: event.currentTarget.value })
          }
        />
      </Group>
      <Group>
        <button
          type="button"
          class={rowClass}
          disabled={taken()}
          onClick={() => props.push({ kind: "wizardScenes" })}
        >
          <span class="min-w-0 flex-1 text-center">{t("Next")}</span>
          <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
        </button>
      </Group>
    </>
  );
}

function WizardScenesPage(props: { scenes: Scenes; config: SceneConfig; onCreated: () => void }) {
  const wizard = () => props.scenes.wizard();
  const picked = (id: string) => wizard().scenes.some((scene) => sameId(scene, id));
  const create = async () => {
    const name = wizard().name.trim() || uniqueName(t("My widget"), props.config.widgets);
    const widget: Widget = { id: newId(), name, type: wizard().type, enabled: true };
    const scenes = props.config.scenes.filter((scene) => picked(scene.id));
    props.onCreated();
    await props.scenes.createWidget(widget);
    for (const scene of scenes) {
      const fresh = props.scenes.scene(scene.id) ?? scene;
      await props.scenes.updateScene({
        ...fresh,
        widgets: [...fresh.widgets, newSceneWidget(widget)],
      });
    }
  };
  return (
    <>
      <Group>
        <For each={props.config.scenes}>
          {(scene) => (
            <button
              type="button"
              class={rowClass}
              onClick={() =>
                props.scenes.setWizard({
                  ...wizard(),
                  scenes: picked(scene.id)
                    ? wizard().scenes.filter((id) => !sameId(id, scene.id))
                    : [...wizard().scenes, scene.id],
                })
              }
            >
              <span class="min-w-0 flex-1 truncate">{scene.name}</span>
              <Show when={picked(scene.id)}>
                <Icon name="check" class="size-[20px] text-phone-tint" />
              </Show>
            </button>
          )}
        </For>
      </Group>
      <Group>
        <TextButton
          label={t("Create")}
          disabled={props.scenes.disabled()}
          onClick={() => void create()}
        />
      </Group>
    </>
  );
}

function SceneWidgetsPage(props: {
  scenes: Scenes;
  config: SceneConfig;
  push: (page: ScenePage) => void;
}) {
  const current = () => {
    const id = props.scenes.current();
    return id ? props.scenes.scene(id) : undefined;
  };
  const items = () => {
    const scene = current();
    return scene ? sceneWidgetsOf(scene, props.config) : [];
  };
  return (
    <>
      <Show when={items().length > 0}>
        <Group>
          <For each={items()}>
            {(item) => (
              <SwitchNavRow
                label={item.widget.name}
                icon={widgetIcons[item.widget.type] ?? "sparkles"}
                on={item.widget.enabled}
                disabled={props.scenes.disabled()}
                onToggle={(enabled) => void props.scenes.updateWidget({ ...item.widget, enabled })}
                onClick={() =>
                  props.push({ kind: "sceneWidget", sceneId: item.scene.id, id: item.layout.id })
                }
              />
            )}
          </For>
        </Group>
      </Show>
      <Group header={t("Shortcut")}>
        <NavRow icon="scenes" label={t("Scenes")} onClick={() => props.push({ kind: "scenes" })} />
      </Group>
    </>
  );
}

export function ScenePageContent(props: {
  page: ScenePage;
  scenes: Scenes;
  push: (page: ScenePage) => void;
  pop: (count: number) => void;
}) {
  return (
    <Show
      when={props.scenes.allowed()}
      fallback={<Empty text={t("The streamer has not let you control Moblin.")} />}
    >
      <Switch>
        <Match when={props.page.kind === "text" && props.page}>
          {(page) => (
            <TextPage
              page={page() as Extract<ScenePage, { kind: "text" }>}
              onDone={() => props.pop(1)}
            />
          )}
        </Match>
        <Match when={props.scenes.config()}>
          {(config) => (
            <ConfigPage
              page={props.page}
              scenes={props.scenes}
              config={config()}
              push={props.push}
              pop={props.pop}
            />
          )}
        </Match>
        <Match when={props.scenes.error()}>{(error) => <Empty text={error()} />}</Match>
        <Match when={true}>
          <Loading />
        </Match>
      </Switch>
    </Show>
  );
}

function ConfigPage(props: {
  page: ScenePage;
  scenes: Scenes;
  config: SceneConfig;
  push: (page: ScenePage) => void;
  pop: (count: number) => void;
}) {
  const gone = <Empty text={t("Nothing here.")} />;
  const page = () => props.page;
  return (
    <Switch>
      <Match when={page().kind === "scenes"}>
        <ScenesPage scenes={props.scenes} config={props.config} push={props.push} />
      </Match>
      <Match when={page().kind === "sceneWidgets"}>
        <SceneWidgetsPage scenes={props.scenes} config={props.config} push={props.push} />
      </Match>
      <Match when={page().kind === "scene" && props.scenes.scene((page() as { id: string }).id)}>
        {(scene) => (
          <ScenePageView
            scenes={props.scenes}
            config={props.config}
            scene={scene()}
            push={props.push}
          />
        )}
      </Match>
      <Match when={page().kind === "sceneWidget" && (page() as { sceneId: string; id: string })}>
        {(at) => {
          const scene = () => props.scenes.scene(at().sceneId);
          const layout = () => scene()?.widgets.find((item) => sameId(item.id, at().id));
          const widget = () => {
            const found = layout();
            return found ? props.scenes.widget(found.widgetId) : undefined;
          };
          const nested = () => {
            const found = widget();
            const id = found?.type === "Scene" ? nestedSceneId(found) : undefined;
            return id ? props.scenes.scene(id) : undefined;
          };
          return (
            <Show when={scene() && layout() && widget()} fallback={gone}>
              <LayoutGroup
                scenes={props.scenes}
                scene={scene()!}
                layout={layout()!}
                widget={widget()!}
              />
              <Group header={t("Shortcut")}>
                <NavRow
                  label={t("Widget")}
                  onClick={() => props.push({ kind: "widget", id: widget()!.id })}
                />
                <Show when={nested()}>
                  {(inner) => (
                    <NavRow
                      label={t("Scene")}
                      onClick={() => props.push({ kind: "scene", id: inner().id })}
                    />
                  )}
                </Show>
              </Group>
            </Show>
          );
        }}
      </Match>
      <Match when={page().kind === "widget" && props.scenes.widget((page() as { id: string }).id)}>
        {(widget) => (
          <WidgetPage
            scenes={props.scenes}
            config={props.config}
            widget={widget()}
            push={props.push}
          />
        )}
      </Match>
      <Match
        when={
          page().kind === "camera" && props.scenes.scene((page() as { sceneId: string }).sceneId)
        }
      >
        {(scene) => (
          <Group>
            <For each={props.config.cameras}>
              {(camera) => (
                <button
                  type="button"
                  class={rowClass}
                  disabled={props.scenes.disabled()}
                  onClick={() => void props.scenes.updateScene(withCamera(scene(), camera))}
                >
                  <span class="min-w-0 flex-1 truncate">{camera.name}</span>
                  <Show when={cameraOf(scene(), props.config.cameras)?.id === camera.id}>
                    <Icon name="check" class="size-[18px] text-phone-tint" />
                  </Show>
                </button>
              )}
            </For>
          </Group>
        )}
      </Match>
      <Match when={page().kind === "wizard"}>
        <WizardPage scenes={props.scenes} config={props.config} push={props.push} />
      </Match>
      <Match when={page().kind === "wizardScenes"}>
        <WizardScenesPage
          scenes={props.scenes}
          config={props.config}
          onCreated={() => props.pop(2)}
        />
      </Match>
      <Match when={true}>{gone}</Match>
    </Switch>
  );
}

function OutlineLabel(props: { widget: Widget }) {
  return (
    <span class="flex items-center gap-[3px] rounded-[3px] bg-black/60 px-[4px] py-px text-[11px] leading-[13px] whitespace-nowrap">
      <Icon name={widgetIcons[props.widget.type] ?? "sparkles"} class="size-[10px] shrink-0" />
      {props.widget.name}
    </span>
  );
}

export function WidgetOutlines(props: { outlines: WidgetOutline[]; width: number }) {
  const scale = () => props.width / canvasWidth;
  return (
    <For each={props.outlines}>
      {(outline) => {
        const x = () => outline.rect[0] * scale();
        const y = () => outline.rect[1] * scale();
        const width = () => outline.rect[2] * scale();
        const height = () => outline.rect[3] * scale();
        const above = () => y() > 20;
        const outside = (leading: boolean) => ({
          position: "absolute" as const,
          ...(above() ? { bottom: "calc(100% + 4px)" } : { top: "calc(100% + 4px)" }),
          ...(leading ? { left: "0" } : { right: "0" }),
        });
        return (
          <Show
            when={outline.sized}
            fallback={
              <div class="absolute" style={{ left: `${x()}px`, top: `${y()}px` }}>
                <span class="absolute -top-[4px] -left-[4px] size-[8px] rounded-full bg-white" />
                <div class="absolute size-0" style={{ left: "0", top: "0" }}>
                  <div style={outside(isLeft(outline.layout.alignment))}>
                    <OutlineLabel widget={outline.widget} />
                  </div>
                </div>
              </div>
            }
          >
            <div
              class="absolute border border-white"
              classList={{ "border-dashed": !outline.inScene }}
              style={{
                left: `${x()}px`,
                top: `${y()}px`,
                width: `${width()}px`,
                height: `${height()}px`,
              }}
            >
              <Show
                when={width() >= 120 && height() >= 40}
                fallback={
                  <div style={outside(true)}>
                    <OutlineLabel widget={outline.widget} />
                  </div>
                }
              >
                <div class="absolute top-[4px] left-[4px]">
                  <OutlineLabel widget={outline.widget} />
                </div>
              </Show>
            </div>
          </Show>
        );
      }}
    </For>
  );
}
