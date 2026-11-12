import {
  createEffect,
  createResource,
  createSignal,
  For,
  on,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import Icon, { type IconName } from "./Icon";
import { ask, rickRoll } from "./Modal";
import { PanelButtons, panelWidth, type Remote } from "./MoblinParts";
import Panels, { type Panel } from "./MoblinPanels";
import { ReadOnly } from "./sceneEditor/Form";
import SceneEditor from "./sceneEditor/SceneEditor";
import { createEditor, type Editor } from "./sceneEditor/store";
import {
  type AudioLevel,
  type Control,
  type DeviceSettings,
  type DeviceState,
  type DeviceStatus,
  type MoblinFilter,
  type Named,
  type StatusItem,
  type TopLeftItem,
  type TopRightItem,
} from "../data/account";
import { moblinIconUrl } from "../data/backend";
import { filtersOf } from "../data/moblin";
import { streamerProfile } from "../data/remoteControl";
import { t } from "../i18n";
import { readStored, store } from "../storage";

const screenWidth = 852;
const screenHeight = 393;
const bezel = 12;

const icons = new Map<string, Promise<string | null>>();

export const topLeftIcons: [TopLeftItem, IconName][] = [
  ["stream", "broadcast"],
  ["camera", "camera"],
  ["mic", "music-mic"],
  ["zoom", "search"],
  ["obs", "server"],
  ["events", "megaphone"],
  ["chat", "chat"],
  ["viewers", "eye"],
];

export const topRightIcons: [TopRightItem, IconName][] = [
  ["rtmpServer", "server"],
  ["moblink", "moblink"],
  ["remoteControl", "remote"],
  ["djiDevices", "remote"],
  ["gameController", "gamepad"],
  ["bitrate", "speedometer"],
  ["srtla", "connection"],
  ["srtlaRtts", "connection"],
  ["replay", "play"],
  ["uptime", "clock"],
  ["location", "location"],
  ["recording", "record"],
  ["browserWidgets", "globe"],
  ["systemMonitor", "cpu"],
];

type QuickButton = { type: string; name: string; page: number };

const quickButtons = (): QuickButton[] => [
  { type: "Torch", name: t("Torch"), page: 1 },
  { type: "Mute", name: t("Mute"), page: 1 },
  { type: "Mic", name: t("Mic"), page: 1 },
  { type: "Record", name: t("Record"), page: 1 },
  { type: "Widgets", name: t("Scene widgets"), page: 1 },
  { type: "Scene editor", name: t("Scene editor"), page: 1 },
  { type: "Bitrate", name: t("Bitrate"), page: 1 },
  { type: "Connection priorities", name: t("Connection priorities"), page: 1 },
  { type: "Reload browser widgets", name: t("Reload browser widgets"), page: 1 },
  { type: "Black screen", name: t("Stealth mode"), page: 1 },
  { type: "Poll", name: t("Poll"), page: 2 },
  { type: "Pinch", name: t("Pinch"), page: 2 },
  { type: "Whirlpool", name: t("Whirlpool"), page: 2 },
  { type: "Blur faces", name: t("Blur faces"), page: 2 },
  { type: "Privacy", name: t("Privacy"), page: 2 },
  { type: "Movie", name: t("Movie"), page: 2 },
  { type: "4:3", name: "4:3", page: 2 },
  { type: "CRT", name: "CRT", page: 2 },
  { type: "Pixellate", name: t("Pixellate"), page: 2 },
  { type: "Gray scale", name: t("Gray scale"), page: 2 },
  { type: "Sepia", name: t("Sepia"), page: 2 },
  { type: "Triple", name: t("Triple"), page: 2 },
  { type: "Twin", name: t("Twin"), page: 2 },
  { type: "Moblin in mouth", name: t("Moblin in mouth"), page: 2 },
  { type: "Camera man", name: t("Camera man"), page: 2 },
  { type: "Beauty filter", name: t("Beauty"), page: 2 },
  { type: "Instant replay", name: t("Instant replay"), page: 3 },
  { type: "Auto scene switcher", name: t("Auto scene switcher"), page: 3 },
  { type: "Macros", name: t("Macros"), page: 3 },
  { type: "Gimbal tracking", name: t("Gimbal tracking"), page: 3 },
  { type: "Preview stream", name: t("Preview stream"), page: 3 },
];

const buttonIcons: Record<string, IconName> = {
  Torch: "flashlight",
  Mute: "mic",
  Mic: "music-mic",
  Record: "record",
  "Black screen": "stealth",
  Bitrate: "speedometer",
  "Connection priorities": "phone-connection",
  "Reload browser widgets": "reload",
  "Scene editor": "rectangle-group",
  Widgets: "photo-on-rectangle",
  "Instant replay": "memories",
  Movie: "film",
  "Gray scale": "moon",
  Sepia: "moonphase",
  Triple: "person-3",
  Twin: "person-2",
  Pixellate: "split-2x2",
  CRT: "tv",
  "4:3": "square",
  "Blur faces": "face-dashed",
  Privacy: "privacy",
  "Beauty filter": "wand-stars",
  Poll: "chart-bar",
  Pinch: "hand-pinch",
  Whirlpool: "tornado",
  "Auto scene switcher": "autostartstop",
  Macros: "increase-indent",
  "Gimbal tracking": "gimbal",
  "Preview stream": "video-circle",
  "Camera man": "video",
  "Moblin in mouth": "mouth",
};

const filters: Record<string, MoblinFilter> = {
  Pixellate: "pixellate",
  Movie: "movie",
  "Gray scale": "grayScale",
  Sepia: "sepia",
  Triple: "triple",
  Twin: "twin",
  "4:3": "fourThree",
  CRT: "crt",
  Pinch: "pinch",
  Whirlpool: "whirlpool",
  Poll: "poll",
  "Blur faces": "blurFaces",
  Privacy: "privacy",
  "Beauty filter": "beauty",
  "Moblin in mouth": "moblinInMouth",
  "Camera man": "cameraMan",
};

const panels: Record<string, Panel> = {
  Mic: "mic",
  Bitrate: "bitrate",
  "Connection priorities": "priorities",
  "Auto scene switcher": "autoSceneSwitcher",
  Macros: "macros",
  Widgets: "sceneWidgets",
};

function Chip(props: { icon: IconName; text?: string; ok?: boolean; before?: boolean }) {
  const text = () => (
    <Show when={props.text}>
      <span class="rounded-[5px] bg-phone-chip px-[2px] text-[13px] leading-[17px] whitespace-nowrap">
        {props.text}
      </span>
    </Show>
  );
  return (
    <div class="flex items-center gap-px">
      <Show when={props.before}>{text()}</Show>
      <span
        class={`grid h-[17px] w-[21px] shrink-0 place-items-center rounded-[5px] bg-phone-chip ${
          props.ok === false ? "text-phone-live" : "text-white"
        }`}
      >
        <Icon name={props.icon} class="size-[13px]" />
      </span>
      <Show when={!props.before}>{text()}</Show>
    </div>
  );
}

function AudioBar(props: { level: AudioLevel }) {
  const level = () => ("value" in props.level ? props.level.value._0 : null);
  const bars = () => {
    const db = level();
    if (db === null || db <= -60) {
      return [];
    }
    if (db > -0.2) {
      return [{ width: 60, color: "bg-phone-live" }];
    }
    return [
      { width: Math.max(0, db + 8.5), color: "bg-phone-live" },
      { width: Math.max(0, Math.min(db + 20, 11.5)), color: "bg-phone-warn" },
      { width: Math.min(db + 60, 40), color: "bg-phone-switch" },
    ].filter((bar) => bar.width > 0);
  };
  return (
    <div class="flex items-center gap-px">
      <span class="flex min-h-[17px] items-center rounded-[5px] bg-phone-chip px-[2px] text-[13px]">
        <Show when={!("muted" in props.level)} fallback={t("Muted")}>
          <span class="flex py-[2px]">
            <For each={bars()}>
              {(bar) => <span class={`h-[5px] ${bar.color}`} style={{ width: `${bar.width}px` }} />}
            </For>
          </span>
        </Show>
      </span>
      <span class="grid h-[17px] w-[21px] place-items-center rounded-[5px] bg-phone-chip">
        <Icon name="waveform" class="size-[13px]" />
      </span>
    </div>
  );
}

function Statuses(props: { status: DeviceStatus; verbose: boolean; onToggle: () => void }) {
  const left = () => topLeftIcons.filter(([key]) => props.status.topLeft[key] !== undefined);
  const right = () => topRightIcons.filter(([key]) => props.status.topRight[key] !== undefined);
  const leftItem = (key: TopLeftItem): StatusItem => props.status.topLeft[key]!;
  const rightItem = (key: TopRightItem): StatusItem => props.status.topRight[key]!;
  return (
    <>
      <button
        type="button"
        class={`absolute top-[10px] left-[54px] z-10 flex items-start gap-px text-left ${
          props.verbose ? "flex-col" : "flex-row"
        }`}
        onClick={() => props.onToggle()}
      >
        <For each={left()}>
          {([key, icon]) => (
            <Chip
              icon={icon}
              text={props.verbose ? leftItem(key).message : undefined}
              ok={leftItem(key).ok}
            />
          )}
        </For>
      </button>
      <button
        type="button"
        class={`absolute top-[10px] right-[16px] z-10 flex items-end gap-px ${
          props.verbose ? "flex-col" : "flex-row"
        }`}
        onClick={() => props.onToggle()}
      >
        <Show when={props.verbose && props.status.topRight.audioInfo}>
          {(audio) => <AudioBar level={audio().audioLevel} />}
        </Show>
        <For each={right()}>
          {([key, icon]) => (
            <Chip
              icon={icon}
              text={props.verbose ? rightItem(key).message : undefined}
              ok={rightItem(key).ok}
              before
            />
          )}
        </For>
        <Show when={!props.verbose && props.status.topRight.audioInfo}>
          {(audio) => <Chip icon={"muted" in audio().audioLevel ? "mic-off" : "waveform"} />}
        </Show>
      </button>
    </>
  );
}

function Segments(props: {
  label: string;
  items: Named[];
  selected: string | undefined;
  width: number;
  disabled: boolean;
  onPick: (id: string) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={props.label}
      class="flex overflow-hidden rounded-[7px] border border-phone-border/30 bg-phone-chip"
    >
      <For each={props.items}>
        {(item, index) => (
          <button
            type="button"
            role="radio"
            aria-checked={item.id === props.selected}
            disabled={props.disabled}
            class={`h-[40px] truncate border-phone-border/30 px-1 text-[15px] ${
              index() < props.items.length - 1 ? "border-r" : ""
            } ${item.id === props.selected ? "bg-phone-selected" : ""}`}
            style={{ width: `${props.width}px` }}
            onClick={() => item.id !== props.selected && props.onPick(item.id)}
          >
            {item.name}
          </button>
        )}
      </For>
    </div>
  );
}

function Battery(props: { level: number | undefined; charging: boolean | undefined }) {
  return (
    <Show when={props.level !== undefined} fallback={<span class="w-[38px]" />}>
      <span class="flex items-center">
        <Icon
          name="bolt"
          class={`size-[10px] ${props.charging ? "text-white" : "text-transparent"}`}
        />
        <span class="grid h-[12px] w-[24px] place-items-center rounded-[2px] bg-white text-[12px] leading-none font-bold text-black">
          {props.level}
        </span>
        <span class="h-[4px] w-[2px] rounded-r-full bg-phone-border" />
      </span>
    </Show>
  );
}

function QuickButtonView(props: {
  button: QuickButton;
  on: boolean;
  selected: boolean;
  disabled: boolean;
  nameWidth: number;
  onPress: () => void;
}) {
  const icon = () =>
    props.button.type === "Mute" && props.on
      ? "mic-off"
      : (buttonIcons[props.button.type] ?? "sparkles");
  const nameSize = () => {
    const longest = Math.max(...props.button.name.split(/\s+/).map((word) => word.length));
    const twoLines = (2 * props.nameWidth) / (props.button.name.length * 0.62);
    return Math.max(5, Math.min(10, props.nameWidth / (longest * 0.58), twoLines));
  };
  return (
    <div class="flex flex-col items-center" style={{ width: `${props.nameWidth}px` }}>
      <button
        type="button"
        aria-label={props.button.name}
        aria-pressed={props.on}
        title={props.button.name}
        disabled={props.disabled}
        class="relative grid size-[40px] shrink-0 place-items-center rounded-full bg-phone-button text-white"
        onClick={() => props.onPress()}
      >
        <Icon name={icon()} class="size-[21px] [stroke-width:1.7]" />
        <Show when={props.on || props.selected}>
          <span class="pointer-events-none absolute inset-[0.5px] rounded-full border border-white" />
        </Show>
      </button>
      <span
        class="line-clamp-2 w-full text-center leading-[1.2]"
        style={{ "font-size": `${nameSize()}px` }}
      >
        {props.button.name}
      </span>
    </div>
  );
}

export default function MoblinScreen(props: {
  page: string;
  state: DeviceState;
  status: DeviceStatus;
  settings: DeviceSettings | null;
  preview: string | null;
  goLive: boolean;
  secrets: boolean;
  remoteControl: boolean;
  connected: boolean;
  pending: string[];
  fit?: boolean;
  window?: boolean;
  onControl: (control: Control) => Promise<void>;
  onRequest?: <T>(control: Control) => Promise<T>;
}) {
  const [width, setWidth] = createSignal(screenWidth + 2 * bezel);
  const [editor, setEditor] = createSignal<Editor | null>(null);
  const [sceneEditorOpen, setSceneEditorOpen] = createSignal(false);
  const [panel, setPanel] = createSignal<Panel | null>(null);
  createEffect(
    on(
      () => sceneEditorOpen() || panel() === "sceneWidgets",
      (needed) => {
        const current = editor();
        if (needed && !current && props.onRequest) {
          setEditor(
            createEditor(
              props.onRequest,
              props.onControl,
              () => props.state.scene,
              () => props.secrets,
              () => !props.remoteControl,
            ),
          );
        } else if (!needed && current) {
          setEditor(null);
          void current.flush().finally(() => current.dispose());
        }
      },
    ),
  );
  onCleanup(() => editor()?.dispose());
  const openSceneEditor = () => {
    if (props.onRequest) {
      setSceneEditorOpen(true);
    }
  };
  const [height, setHeight] = createSignal(screenHeight + 2 * bezel);
  const [verbose, setVerbose] = createSignal(readStored("statusCollapsed") !== "1");
  const [panelHidden, setPanelHidden] = createSignal(false);
  const [page, setPage] = createSignal(1);
  const [allPages, setAllPages] = createSignal(readStored("quickButtonsExpanded") === "1");
  const [icon] = createResource(
    () => props.page,
    (id) => {
      let found = icons.get(id);
      if (!found) {
        found = streamerProfile(id)
          .then((profile) => profile.icon)
          .catch(() => {
            icons.delete(id);
            return null;
          });
        icons.set(id, found);
      }
      return found;
    },
  );
  let container: HTMLDivElement | undefined;

  onMount(() => {
    if (!container) {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const measured = entries[0]?.contentRect;
      if (measured?.width) {
        setWidth(measured.width);
      }
      if (measured?.height) {
        setHeight(measured.height);
      }
    });
    observer.observe(container);
    onCleanup(() => observer.disconnect());
  });

  const frame = () => (props.window ? 0 : bezel);
  const scale = () =>
    props.window
      ? Math.min(width() / 640, height() / screenHeight, 1)
      : Math.min(
          width() / (screenWidth + 2 * bezel),
          props.fit ? height() / (screenHeight + 2 * bezel) : 1.2,
        );
  const shownWidth = () => (props.window ? width() / scale() : screenWidth);
  const shownHeight = () => (props.window ? height() / scale() : screenHeight);
  const outerWidth = () => shownWidth() + 2 * frame();
  const outerHeight = () => shownHeight() + 2 * frame();
  const disabled = (key: string) =>
    !props.connected || !props.remoteControl || props.pending.includes(key);
  const confirmed = async (question: string, action: string, control: Control) => {
    if (await ask(question, action)) {
      await props.onControl(control);
    }
  };
  const toggle = (next: Panel | null) => {
    setPanel((current) => (current === next ? null : next));
    setPanelHidden(false);
  };
  const remote: Remote = {
    get state() {
      return props.state;
    },
    get settings() {
      return props.settings;
    },
    get secrets() {
      return props.secrets;
    },
    get readOnly() {
      return !props.remoteControl;
    },
    disabled,
    control: (control: Control) => props.onControl(control),
    get request() {
      return props.onRequest;
    },
  };

  const filterOn = () => filtersOf(props.state.filters);
  const pages = () =>
    [1, 2, 3].filter((number) => quickButtons().some((button) => button.page === number));
  const quickButtonsWidth = () =>
    allPages() ? pages().length * 100 + (pages().length - 1) * 8 : 100;
  const controlBarWidth = () => (props.window ? 106 : 150) + quickButtonsWidth() - 100;
  let pager: HTMLDivElement | undefined;
  const toggleAllPages = () => {
    const next = !allPages();
    setAllPages(next);
    store("quickButtonsExpanded", next ? "1" : null);
    if (!next) {
      setPage(pages()[0] ?? 1);
      pager?.scrollTo({ left: 0 });
    }
  };
  const pageStride = (element: HTMLElement) => element.clientWidth + 8;
  const rows = (page: number) => {
    const buttons = quickButtons().filter((button) => button.page === page);
    const pairs: (QuickButton | null)[][] = [];
    for (let index = 0; index < buttons.length; index += 2) {
      pairs.push(
        index + 1 < buttons.length
          ? [buttons[index + 1]!, buttons[index]!]
          : [buttons[index]!, null],
      );
    }
    return pairs;
  };

  const isOn = (button: QuickButton) => {
    switch (button.type) {
      case "Torch":
        return props.state.torchOn === true;
      case "Mute":
        return props.state.muted === true;
      case "Record":
        return props.state.recording === true;
      case "Black screen":
        return props.state.stealthMode === true;
      case "Gimbal tracking":
        return props.state.gimbalTracking === true;
      case "Preview stream":
        return props.state.previewStream === true;
      default: {
        const filter = filters[button.type];
        return filter !== undefined && filterOn()[filter] === true;
      }
    }
  };

  const actionOf = (button: QuickButton): { key: string; run: () => void } | undefined => {
    const on = isOn(button);
    const send = (key: string, control: Control) => ({
      key,
      run: () => void props.onControl(control),
    });
    switch (button.type) {
      case "Torch":
        return send("setTorch", { setTorch: { on: !on } });
      case "Mute":
        return send("setMute", { setMute: { on: !on } });
      case "Black screen":
        return send("setStealthMode", { setStealthMode: { on: !on } });
      case "Gimbal tracking":
        return send("setGimbalTracking", { setGimbalTracking: { on: !on } });
      case "Preview stream":
        return send("setPreviewStream", { setPreviewStream: { on: !on } });
      case "Instant replay":
        return send("instantReplay", { instantReplay: {} });
      case "Reload browser widgets":
        return send("reloadBrowserWidgets", { reloadBrowserWidgets: {} });
      case "Scene editor":
        return { key: "", run: openSceneEditor };
      case "Record":
        return {
          key: "setRecord",
          run: () =>
            void (on
              ? confirmed(t("Stop recording on this device?"), t("Stop recording"), {
                  setRecord: { on: false },
                })
              : confirmed(t("Start recording on this device?"), t("Start recording"), {
                  setRecord: { on: true },
                })),
        };
    }
    const filter = filters[button.type];
    if (filter) {
      return send("setFilter", {
        setFilter: { filter: { [filter]: {} } as Record<MoblinFilter, object>, on: !on },
      });
    }
    const opened = panels[button.type];
    if (opened) {
      return { key: "", run: () => toggle(opened) };
    }
    return undefined;
  };

  const quickButton = (button: QuickButton, nameWidth: number) => {
    const action = () => actionOf(button);
    return (
      <QuickButtonView
        button={button}
        on={isOn(button)}
        selected={panel() !== null && panel() === panels[button.type]}
        disabled={
          !props.connected ||
          (action()?.key ? disabled(action()!.key) : false) ||
          ((button.type === "Scene editor" || button.type === "Widgets") && !props.onRequest)
        }
        nameWidth={nameWidth}
        onPress={() => action()?.run()}
      />
    );
  };

  const shownPanelWidth = () => (panel() ? (panelHidden() ? 1 : panelWidth) : 0);
  const areaWidth = () => shownWidth() - controlBarWidth() - shownPanelWidth();
  const panelButtons = () => (
    <PanelButtons
      hidden={panelHidden()}
      onToggleHidden={() => setPanelHidden((hidden) => !hidden)}
      onClose={() => toggle(null)}
    />
  );
  const scenes = () => props.settings?.scenes ?? [];
  const zoomPresets = () => {
    const camera = props.state.zoomCamera;
    if (camera?.position !== "front" && camera?.position !== "back") {
      return [];
    }
    return (props.settings?.zoomPresets?.[camera.position] ?? []).filter(
      (preset) => preset.x >= camera.minX && preset.x <= camera.maxX,
    );
  };
  const previewSource = () =>
    props.preview
      ? `data:image/${props.preview.startsWith("iVBOR") ? "png" : "jpeg"};base64,${props.preview}`
      : undefined;

  const zoomPicker = () => (
    <Show when={zoomPresets().length > 0}>
      <Segments
        label={t("Zoom")}
        items={zoomPresets()}
        selected={props.state.zoomPreset}
        width={Math.min(50, (areaWidth() - 20) / zoomPresets().length)}
        disabled={disabled("setZoomPreset")}
        onPick={(id) => void props.onControl({ setZoomPreset: { id } })}
      />
    </Show>
  );
  const scenePicker = () => (
    <Show when={scenes().length > 0}>
      <Segments
        label={t("Scene")}
        items={scenes()}
        selected={props.state.scene}
        width={Math.min(70, (areaWidth() - 20) / scenes().length)}
        disabled={disabled("setScene")}
        onPick={(id) => void props.onControl({ setScene: { id } })}
      />
    </Show>
  );

  return (
    <div
      ref={(element) => (container = element)}
      class={props.fit ? "min-h-0 w-full flex-1 overflow-hidden" : "w-full"}
    >
      <div
        class={props.fit ? "mx-auto" : ""}
        style={{
          height: `${outerHeight() * scale()}px`,
          width: props.fit ? `${outerWidth() * scale()}px` : undefined,
        }}
      >
        <div
          class={
            props.window
              ? "overflow-hidden rounded-[10px] ring-1 ring-phone-frame"
              : "rounded-[62px] bg-phone-bezel shadow-2xl ring-2 ring-phone-frame"
          }
          style={{
            width: `${outerWidth()}px`,
            height: `${outerHeight()}px`,
            padding: `${frame()}px`,
            transform: `scale(${scale()})`,
            "transform-origin": "top left",
            "font-family": "system-ui, -apple-system, sans-serif",
          }}
        >
          <div
            class={`relative flex h-full overflow-hidden bg-phone-screen text-white select-none ${
              props.window ? "" : "rounded-[50px]"
            }`}
          >
            <Show when={sceneEditorOpen() && editor()}>
              {(editor) => (
                <ReadOnly.Provider value={() => !props.remoteControl}>
                  <SceneEditor
                    editor={editor()}
                    preview={props.preview}
                    state={props.state}
                    settings={props.settings}
                    width={shownWidth()}
                    height={shownHeight()}
                    onClose={() => setSceneEditorOpen(false)}
                  />
                </ReadOnly.Provider>
              )}
            </Show>
            <Show when={!sceneEditorOpen()}>
              <div class="relative min-w-0 flex-1 overflow-hidden bg-radial-[at_40%_40%] from-phone-preview to-phone-preview-dark">
                <Show when={previewSource()}>
                  {(source) => (
                    <img
                      src={source()}
                      alt={t("Stream preview")}
                      class="absolute inset-0 size-full bg-phone-screen object-contain object-right"
                    />
                  )}
                </Show>
                <Statuses
                  status={props.status}
                  verbose={verbose()}
                  onToggle={() => {
                    setVerbose((current) => !current);
                    store("statusCollapsed", verbose() ? null : "1");
                  }}
                />
                <Show when={props.state.muted}>
                  <div class="pointer-events-none absolute inset-0 grid place-items-center">
                    <span class="rounded-[10px] bg-black/75 p-[10px] text-phone-live">
                      <Icon name="mic-off" class="size-[80px]" />
                    </span>
                  </div>
                </Show>
                <div class="absolute right-[16px] bottom-[18px] flex flex-col items-end gap-[6px]">
                  {zoomPicker()}
                  {scenePicker()}
                </div>
                <Show when={!props.window}>
                  <span class="absolute top-1/2 left-[12px] h-[118px] w-[34px] -translate-y-1/2 rounded-full bg-phone-screen" />
                </Show>
                <Show when={panel() && panelHidden()}>{panelButtons()}</Show>
              </div>
              <Show when={panel()}>
                {(shown) => (
                  <div
                    class="relative h-full shrink-0 overflow-hidden bg-phone-screen"
                    style={{ width: `${shownPanelWidth()}px` }}
                  >
                    <div class={`h-full ${panelHidden() ? "opacity-0" : ""}`} inert={panelHidden()}>
                      <ReadOnly.Provider value={() => !props.remoteControl}>
                        <Panels
                          panel={shown()}
                          remote={remote}
                          editor={editor()}
                          onOpenSceneEditor={openSceneEditor}
                        />
                      </ReadOnly.Provider>
                    </div>
                    <Show when={!panelHidden()}>{panelButtons()}</Show>
                  </div>
                )}
              </Show>
              <div
                class="flex h-full shrink-0 bg-phone-screen"
                style={{ width: `${controlBarWidth()}px` }}
              >
                <div
                  class="ml-[6px] flex h-full flex-col pt-[6px] pb-[14px]"
                  style={{ width: `${quickButtonsWidth()}px` }}
                >
                  <div class="flex h-[16px] w-[100px] items-center justify-between pr-[18px] pl-[4px]">
                    <Battery
                      level={props.status.general.batteryLevel}
                      charging={props.status.general.batteryCharging}
                    />
                    <Show when={props.status.general.flame}>
                      {(flame) => (
                        <Icon
                          name="flame"
                          class={`size-[14px] ${
                            flame() === "Red"
                              ? "text-phone-live"
                              : flame() === "Yellow"
                                ? "text-phone-warn"
                                : "text-white"
                          }`}
                        />
                      )}
                    </Show>
                    <span class="w-[10px]" />
                  </div>
                  <div
                    ref={(element) => (pager = element)}
                    class={`flex min-h-0 flex-1 gap-[8px] ${
                      allPages()
                        ? "overflow-hidden"
                        : "snap-x snap-mandatory overflow-x-auto overscroll-x-contain [scrollbar-width:none]"
                    }`}
                    onScroll={(event) =>
                      !allPages() &&
                      setPage(
                        pages()[
                          Math.round(
                            event.currentTarget.scrollLeft / pageStride(event.currentTarget),
                          )
                        ] ?? 1,
                      )
                    }
                  >
                    <For each={pages()}>
                      {(number) => (
                        <div
                          class={`flex h-full shrink-0 flex-col ${
                            allPages() ? "w-[100px]" : "w-full snap-start snap-always"
                          }`}
                        >
                          <Show when={number === 1}>
                            <div class="flex items-center justify-center gap-[4px] py-[2px] pr-[10px]">
                              <button
                                type="button"
                                aria-label="Moblin"
                                onClick={() => rickRoll(props.page)}
                              >
                                <img
                                  src={moblinIconUrl(icon() ?? "plain")}
                                  alt=""
                                  class="size-[40px] object-contain"
                                />
                              </button>
                              <button
                                type="button"
                                aria-label={t("Settings")}
                                class={`grid size-[40px] place-items-center rounded-full border ${
                                  panel() === "settings" ? "border-white" : "border-phone-border"
                                }`}
                                onClick={() => toggle("settings")}
                              >
                                <Icon name="gear" class="size-[20px]" />
                              </button>
                            </div>
                          </Show>
                          <div class="flex min-h-0 flex-1 flex-col-reverse gap-[8px] overflow-y-auto pt-[4px] pr-[10px] pl-[5px] [scrollbar-width:none]">
                            <For each={rows(number)}>
                              {(row) => (
                                <div class="flex items-start justify-end gap-[8px]">
                                  <For each={row}>
                                    {(button) =>
                                      button ? (
                                        quickButton(button, 40)
                                      ) : (
                                        <span style={{ width: `${40}px` }} />
                                      )
                                    }
                                  </For>
                                </div>
                              )}
                            </For>
                          </div>
                          <Show when={number === 1}>
                            <div class="flex justify-center pt-[5px] pr-[10px]">
                              <button
                                type="button"
                                disabled={
                                  (!props.goLive && !props.state.streaming) ||
                                  disabled("setLive") ||
                                  props.state.streaming === undefined
                                }
                                title={
                                  props.goLive || props.state.streaming || !props.remoteControl
                                    ? undefined
                                    : t("The streamer has not let you go live.")
                                }
                                class={`min-w-[60px] rounded-[10px] bg-phone-live p-[5px] text-[17px] leading-tight whitespace-nowrap disabled:opacity-50 ${
                                  props.state.streaming ? "border border-white" : ""
                                }`}
                                onClick={() =>
                                  void (props.state.streaming
                                    ? confirmed(
                                        t("End the stream on this device?"),
                                        t("End stream"),
                                        { setLive: { on: false } },
                                      )
                                    : confirmed(t("Go live on this device?"), t("Go live"), {
                                        setLive: { on: true },
                                      }))
                                }
                              >
                                {props.state.streaming ? t("End") : t("Go Live")}
                              </button>
                            </div>
                          </Show>
                        </div>
                      )}
                    </For>
                  </div>
                  <Show when={pages().length > 1}>
                    <div class="flex items-center justify-center gap-[3px] pt-[6px] pr-[10px]">
                      <For each={allPages() ? [] : pages()}>
                        {(number, index) => (
                          <button
                            type="button"
                            aria-label={t("Page {page}", { page: number })}
                            class={`size-[6px] rounded-full border border-white ${
                              number === page() ? "bg-white" : ""
                            }`}
                            onClick={() =>
                              pager?.scrollTo({
                                left: index() * pageStride(pager),
                                behavior: "smooth",
                              })
                            }
                          />
                        )}
                      </For>
                      <button
                        type="button"
                        aria-label={allPages() ? t("Show one page") : t("Show all pages")}
                        title={allPages() ? t("Show one page") : t("Show all pages")}
                        class="ml-[3px] grid size-[10px] place-items-center"
                        onClick={toggleAllPages}
                      >
                        <Icon
                          name={allPages() ? "chevron-right" : "chevron-left"}
                          class="size-[10px]"
                        />
                      </button>
                    </div>
                  </Show>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </div>
  );
}

export function NotConnected(props: { name: string; pane?: boolean }) {
  return (
    <div
      class={`cheer-fade inset-0 z-10 flex items-center justify-center bg-page/60 backdrop-blur-md ${
        props.pane ? "absolute overflow-hidden rounded-xl p-3" : "fixed px-4"
      }`}
    >
      <div
        role="status"
        class={`cheer-pop flex flex-col items-center border-[1.5px] border-line bg-card text-center shadow-2xl ${
          props.pane
            ? "max-w-sm gap-3 rounded-2xl px-6 py-5"
            : "max-w-md gap-4 rounded-3xl px-10 py-8"
        }`}
      >
        <span
          class={`grid place-items-center rounded-full bg-ghost text-muted ${props.pane ? "size-12" : "size-16"}`}
        >
          <Icon name="smartphone" class={props.pane ? "size-6" : "size-8"} />
        </span>
        <span class={`font-display font-semibold text-ink ${props.pane ? "text-lg" : "text-2xl"}`}>
          {t("{name} is not connected", { name: props.name })}
        </span>
        <span class={`text-ink-2 ${props.pane ? "text-[13px]" : "text-[15px]"}`}>
          {t("This page updates as soon as it connects again.")}
        </span>
      </div>
    </div>
  );
}
