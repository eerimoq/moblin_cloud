import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { asset } from "../asset";
import {
  channelKey,
  DevicePicker,
  rememberChannel,
  StreamerPicker,
} from "../components/HeaderPickers";
import Icon from "../components/Icon";
import { ask } from "../components/Modal";
import SectionMenu, { createSectionChoice, type Section } from "../components/SectionMenu";
import {
  AccountMenu,
  Band,
  cardClass,
  CloudHeader,
  ghostButtonClass,
  inputClass,
  Spinner,
} from "../components/PageParts";
import {
  controlDevice,
  fetchAccount,
  pageEventsUrl,
  createAccountSession,
  deviceEventsUrl,
  type Account,
  type Device,
  type Moderating,
  type AudioLevel,
  type Control,
  type DeviceSettings,
  type DeviceSnapshot,
  type DeviceState,
  type DeviceStatus,
  type Sample,
  type StatusItem,
  type StatusUpdate,
  type TopLeftItem,
  type TopRightItem,
} from "../data/account";
import { BackendError } from "../data/backend";
import { deviceUrl, pages } from "../data/links";
import { t } from "../i18n";
import { subscribe, subscribeChanges } from "../sse";

const topLeftItems: [TopLeftItem, string][] = [
  ["stream", t("Stream")],
  ["camera", t("Camera")],
  ["mic", t("Mic")],
  ["zoom", t("Zoom")],
  ["obs", t("OBS")],
  ["events", t("Events")],
  ["chat", t("Chat")],
  ["viewers", t("Viewers")],
];

const topRightItems: [TopRightItem, string][] = [
  ["bitrate", t("Bitrate")],
  ["uptime", t("Uptime")],
  ["srtla", t("SRTLA")],
  ["srtlaRtts", t("SRTLA RTTs")],
  ["audioLevel", t("Audio level")],
  ["recording", t("Recording")],
  ["replay", t("Replay")],
  ["location", t("Location")],
  ["rtmpServer", t("RTMP server")],
  ["remoteControl", t("Remote control")],
  ["gameController", t("Game controller")],
  ["browserWidgets", t("Browser widgets")],
  ["moblink", t("Moblink")],
  ["djiDevices", t("DJI devices")],
  ["systemMonitor", t("System monitor")],
];

const emptyStatus = (): DeviceStatus => ({ general: {}, topLeft: {}, topRight: {} });

const emptySettings: DeviceSettings = {
  streams: [],
  scenes: [],
  bitratePresets: [],
  mics: [],
  srt: { connectionPrioritiesEnabled: false, connectionPriorities: [] },
};

const filled = <T extends object>(part: T) => (Object.keys(part).length > 0 ? part : undefined);

const historyWindow = 60 * 1000;

const formatBits = (bitsPerSecond: number) =>
  bitsPerSecond >= 1_000_000
    ? `${(bitsPerSecond / 1_000_000).toFixed(1)} Mbps`
    : `${Math.round(bitsPerSecond / 1000)} kbps`;

const formatMilliseconds = (milliseconds: number) => `${Math.round(milliseconds)} ms`;

const formatTime = (at: number) =>
  new Date(at).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

const connectionColor = (index: number) => `var(--color-chart-${(index % 4) + 1})`;

type Series = { name: string; color: string; points: (number | null)[]; wide?: boolean };

function TimeChart(props: {
  title: string;
  times: number[];
  series: Series[];
  format: (value: number) => string;
  empty: string;
}) {
  const [width, setWidth] = createSignal(600);
  const [hover, setHover] = createSignal<number | null>(null);
  let container: HTMLDivElement | undefined;
  const height = 180;
  const left = 56;
  const right = 12;
  const top = 12;
  const bottom = 24;

  onMount(() => {
    if (!container) {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const measured = entries[0]?.contentRect.width;
      if (measured) {
        setWidth(Math.max(240, measured));
      }
    });
    observer.observe(container);
    onCleanup(() => observer.disconnect());
  });

  const end = () => props.times[props.times.length - 1] ?? Date.now();
  const start = () => end() - historyWindow;
  const max = createMemo(() => {
    let highest = 0;
    for (const series of props.series) {
      for (const value of series.points) {
        if (value !== null && value > highest) {
          highest = value;
        }
      }
    }
    return highest > 0 ? highest * 1.1 : 1;
  });
  const x = (at: number) => left + ((at - start()) / historyWindow) * (width() - left - right);
  const y = (value: number) => top + (1 - value / max()) * (height - top - bottom);
  const hasData = () =>
    props.series.some((series) => series.points.some((value) => value !== null));

  const path = (series: Series) => {
    let drawn = "";
    let open = false;
    let previous = 0;
    props.times.forEach((at, index) => {
      const value = series.points[index];
      if (value === null || value === undefined || at < start()) {
        open = false;
        return;
      }
      if (at - previous > 5000) {
        open = false;
      }
      drawn += `${open ? "L" : "M"}${x(at).toFixed(1)},${y(value).toFixed(1)}`;
      open = true;
      previous = at;
    });
    return drawn;
  };

  const gridValues = () => [0.25, 0.5, 0.75, 1].map((fraction) => (max() / 1.1) * fraction);
  const xTicks = () =>
    (width() < 480 ? [1, 2] : [1, 2, 3, 4]).map(
      (step) => start() + (step * historyWindow) / (width() < 480 ? 3 : 5),
    );

  const onPointer = (event: PointerEvent) => {
    if (!container || props.times.length === 0) {
      return;
    }
    const x = event.clientX - container.getBoundingClientRect().left;
    const at = start() + ((x - left) / (width() - left - right)) * historyWindow;
    let nearest = 0;
    let distance = Number.POSITIVE_INFINITY;
    props.times.forEach((time, index) => {
      const delta = Math.abs(time - at);
      if (delta < distance) {
        distance = delta;
        nearest = index;
      }
    });
    setHover(nearest);
  };

  const latest = (series: Series) => {
    for (let index = series.points.length - 1; index >= 0; index -= 1) {
      const value = series.points[index];
      if (value !== null && value !== undefined) {
        return value;
      }
    }
    return null;
  };

  return (
    <div class="flex flex-col gap-2">
      <div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h3 class="text-[15px] font-semibold text-ink-2">{props.title}</h3>
        <Show when={props.series.length === 1 && latest(props.series[0]!) !== null}>
          <span class="font-mono text-[15px] text-ink">
            {props.format(latest(props.series[0]!)!)}
          </span>
        </Show>
      </div>
      <div
        ref={(element) => (container = element)}
        class="relative w-full"
        onPointerMove={onPointer}
        onPointerLeave={() => setHover(null)}
      >
        <Show
          when={hasData()}
          fallback={<p class="py-6 text-center text-[15px] text-muted">{props.empty}</p>}
        >
          <svg
            width={width()}
            height={height}
            class="block overflow-visible"
            role="img"
            aria-label={props.title}
          >
            <For each={gridValues()}>
              {(value) => (
                <>
                  <line
                    x1={left}
                    x2={width() - right}
                    y1={y(value)}
                    y2={y(value)}
                    class="stroke-line"
                    stroke-width="1"
                  />
                  <text
                    x={left - 8}
                    y={y(value) + 4}
                    text-anchor="end"
                    class="fill-muted text-[11px]"
                  >
                    {props.format(value)}
                  </text>
                </>
              )}
            </For>
            <For each={xTicks()}>
              {(at) => (
                <text x={x(at)} y={height - 6} text-anchor="middle" class="fill-muted text-[11px]">
                  {formatTime(at)}
                </text>
              )}
            </For>
            <line
              x1={left}
              x2={width() - right}
              y1={y(0)}
              y2={y(0)}
              class="stroke-line"
              stroke-width="1"
            />
            <For each={props.series}>
              {(series) => (
                <path
                  d={path(series)}
                  fill="none"
                  stroke={series.color}
                  stroke-width={series.wide ? 3 : 2}
                  stroke-linejoin="round"
                  stroke-linecap="round"
                />
              )}
            </For>
            <Show when={hover() !== null && props.times[hover()!] !== undefined}>
              <line
                x1={x(props.times[hover()!]!)}
                x2={x(props.times[hover()!]!)}
                y1={top}
                y2={height - bottom}
                class="stroke-muted"
                stroke-width="1"
              />
              <For each={props.series}>
                {(series) => (
                  <Show
                    when={series.points[hover()!] !== null && series.points[hover()!] !== undefined}
                  >
                    <circle
                      cx={x(props.times[hover()!]!)}
                      cy={y(series.points[hover()!]!)}
                      r="4"
                      fill={series.color}
                      class="stroke-card"
                      stroke-width="2"
                    />
                  </Show>
                )}
              </For>
            </Show>
          </svg>
          <Show when={hover() !== null && props.times[hover()!] !== undefined}>
            <div
              class="pointer-events-none absolute top-2 rounded-xl border-[1.5px] border-line bg-card px-3 py-2 text-[13px] shadow-lg"
              style={{
                left: `${Math.min(x(props.times[hover()!]!) + 12, width() - 170)}px`,
              }}
            >
              <div class="text-muted">{formatTime(props.times[hover()!]!)}</div>
              <For each={props.series}>
                {(series) => (
                  <div class="flex items-center gap-2">
                    <span class="size-2 rounded-full" style={{ background: series.color }} />
                    <span class="font-semibold text-ink">
                      {series.points[hover()!] === null || series.points[hover()!] === undefined
                        ? "–"
                        : props.format(series.points[hover()!]!)}
                    </span>
                    <Show when={props.series.length > 1}>
                      <span class="text-ink-2">{series.name}</span>
                    </Show>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </div>
      <Show when={props.series.length > 1}>
        <ul class="flex flex-wrap gap-x-5 gap-y-1 text-[13px]">
          <For each={props.series}>
            {(series) => (
              <li class="flex items-center gap-2">
                <span class="size-2.5 rounded-full" style={{ background: series.color }} />
                <span class="text-ink-2">{series.name}</span>
                <span class="font-mono text-ink">
                  {latest(series) === null ? "–" : props.format(latest(series)!)}
                </span>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}

function Graphs(props: { history: Sample[] }) {
  const recent = createMemo(() => {
    const samples = props.history;
    const last = samples[samples.length - 1]?.at ?? Date.now();
    return samples.filter((sample) => sample.at >= last - historyWindow);
  });
  const times = createMemo(() => recent().map((sample) => sample.at));
  const names = createMemo(() => {
    const seen: string[] = [];
    for (const sample of recent()) {
      for (const connection of sample.bonding) {
        if (!seen.includes(connection.name)) {
          seen.push(connection.name);
        }
      }
    }
    return seen;
  });
  const bitrate = createMemo<Series[]>(() => [
    {
      name: t("Total"),
      color: "var(--color-ink)",
      wide: true,
      points: recent().map((sample) => sample.bitrate),
    },
    ...names().map((name, index) => ({
      name,
      color: connectionColor(index),
      points: recent().map((sample) => {
        const connection = sample.bonding.find((candidate) => candidate.name === name);
        return connection === undefined ? null : connection.bytesPerSecond * 8;
      }),
    })),
  ]);
  const rtts = createMemo<Series[]>(() =>
    names().map((name, index) => ({
      name,
      color: connectionColor(index),
      points: recent().map((sample) => {
        const connection = sample.bonding.find((candidate) => candidate.name === name);
        return connection?.rtt ?? null;
      }),
    })),
  );

  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Last minute")}</h2>
      <TimeChart
        title={t("Bitrate")}
        times={times()}
        series={bitrate()}
        format={formatBits}
        empty={t("No bitrate yet. It is shown while the stream is live.")}
      />
      <Show when={names().length > 0}>
        <TimeChart
          title={t("Round trip time")}
          times={times()}
          series={rtts()}
          format={formatMilliseconds}
          empty={t("No round trip times yet.")}
        />
      </Show>
    </div>
  );
}

function Tile(props: { label: string; value: string; tone?: "on" | "warn" | "bad" }) {
  return (
    <div class="flex flex-col gap-1 rounded-2xl border-[1.5px] border-line bg-well px-4 py-3">
      <span class="text-[13px] font-semibold tracking-[0.04em] text-muted uppercase">
        {props.label}
      </span>
      <span
        class={`font-display text-[22px] font-bold ${
          props.tone === "on"
            ? "text-leaf"
            : props.tone === "warn"
              ? "text-amber"
              : props.tone === "bad"
                ? "text-danger"
                : "text-ink"
        }`}
      >
        {props.value}
      </span>
    </div>
  );
}

function AudioMeter(props: { level: AudioLevel }) {
  const decibels = () => ("value" in props.level ? props.level.value._0 : null);
  const width = () => Math.max(0, Math.min(100, ((decibels() ?? -60) + 60) / 0.6));
  return (
    <div class="flex items-center gap-3">
      <div class="h-2.5 flex-1 overflow-hidden rounded-full bg-line">
        <div
          class="h-full rounded-full bg-leaf transition-[width]"
          style={{ width: `${width()}%` }}
        />
      </div>
      <span class="w-16 text-right font-mono text-[15px] text-ink-2">
        {"muted" in props.level
          ? t("Muted")
          : decibels() === null
            ? t("Unknown")
            : `${decibels()!.toFixed(0)} dB`}
      </span>
    </div>
  );
}

type ItemKey = TopLeftItem | TopRightItem;

function Items(props: { items: [ItemKey, string][]; of: Partial<Record<ItemKey, StatusItem>> }) {
  const present = () => props.items.filter(([key]) => props.of[key] !== undefined);
  return (
    <Show when={present().length > 0}>
      <ul class="flex flex-col divide-y-[1.5px] divide-line rounded-2xl border-[1.5px] border-line">
        <For each={present()}>
          {([key, label]) => (
            <li class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 px-4 py-3">
              <span class="text-[15px] font-semibold text-ink-2">{label}</span>
              <span
                class={`font-mono text-[15px] break-words ${props.of[key]?.ok === false ? "text-danger" : "text-ink"}`}
              >
                {props.of[key]?.message}
              </span>
            </li>
          )}
        </For>
      </ul>
    </Show>
  );
}

const onOff = (on: boolean | undefined) =>
  on === undefined ? null : on ? { text: t("On"), tone: "on" as const } : { text: t("Off") };

function StatusCard(props: { status: DeviceStatus; connected: boolean }) {
  const general = () => props.status.general;
  const flame = () => {
    switch (general().flame) {
      case "White":
        return { text: t("Normal"), tone: "on" as const };
      case "Yellow":
        return { text: t("Warm"), tone: "warn" as const };
      case "Red":
        return { text: t("Hot"), tone: "bad" as const };
      default:
        return null;
    }
  };
  const battery = () => {
    const level = general().batteryLevel;
    if (level === undefined) {
      return null;
    }
    return {
      text: general().batteryCharging ? t("{level}%, charging", { level }) : `${level}%`,
      tone: level <= 20 ? ("bad" as const) : undefined,
    };
  };
  const tiles = () =>
    [
      [t("Live"), onOff(general().isLive)],
      [t("Recording"), onOff(general().isRecording)],
      [
        t("Muted"),
        general().isMuted === undefined
          ? null
          : {
              text: general().isMuted ? t("Yes") : t("No"),
              tone: general().isMuted ? ("bad" as const) : undefined,
            },
      ],
      [t("Battery"), battery()],
      [t("Thermal"), flame()],
      [t("Wi-Fi"), general().wiFiSsid === undefined ? null : { text: general().wiFiSsid }],
    ].filter(([, value]) => value !== null) as [
      string,
      { text: string; tone?: "on" | "warn" | "bad" },
    ][];
  const empty = () =>
    Object.keys(props.status.general).length === 0 &&
    Object.keys(props.status.topLeft).length === 0 &&
    Object.keys(props.status.topRight).length === 0;

  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Status")}</h2>
      <Show
        when={!empty()}
        fallback={
          props.connected ? (
            <Spinner />
          ) : (
            <p class="py-6 text-center text-[15px] text-muted">
              {t("This Moblin is not connected.")}
            </p>
          )
        }
      >
        <div class="grid gap-3 sm:grid-cols-3">
          <For each={tiles()}>
            {([label, value]) => <Tile label={label} value={value.text} tone={value.tone} />}
          </For>
        </div>
        <Show when={props.status.topRight.audioInfo}>
          {(audio) => (
            <div class="flex flex-col gap-2">
              <span class="text-[15px] font-semibold text-ink-2">{t("Audio level")}</span>
              <AudioMeter level={audio().audioLevel} />
            </div>
          )}
        </Show>
        <Items items={topRightItems} of={props.status.topRight} />
        <Items items={topLeftItems} of={props.status.topLeft} />
      </Show>
    </div>
  );
}

const toggleClass = (on: boolean | undefined, live?: boolean) =>
  `inline-flex h-11 items-center justify-center gap-2 rounded-full border-[1.5px] px-4 font-display text-[15px] font-semibold whitespace-nowrap transition-colors disabled:opacity-60 ${
    on === true
      ? live
        ? "border-live bg-live text-white"
        : "border-leaf bg-leaf text-page"
      : "border-ghost-line bg-ghost text-ink hover:bg-ghost-hover"
  }`;

function Toggle(props: {
  label: string;
  on: boolean | undefined;
  live?: boolean;
  disabled: boolean;
  onToggle: (on: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={props.on === true}
      class={toggleClass(props.on, props.live)}
      disabled={props.disabled || props.on === undefined}
      onClick={() => props.onToggle(!props.on)}
    >
      <span class={`size-2.5 rounded-full ${props.on ? "bg-current" : "bg-muted"}`} />
      {props.label}
    </button>
  );
}

function Picker(props: {
  label: string;
  value: string | undefined;
  options: { id: string; name: string }[];
  disabled: boolean;
  onPick: (id: string) => Promise<void>;
}) {
  return (
    <label class="flex flex-col gap-2">
      <span class="text-[15px] font-semibold text-ink-2">{props.label}</span>
      <select
        class={inputClass}
        disabled={props.disabled}
        value={props.value ?? ""}
        onChange={async (event) => {
          const select = event.currentTarget;
          await props.onPick(select.value);
          select.value = props.value ?? "";
        }}
      >
        <Show when={props.value === undefined}>
          <option value="" selected disabled>
            –
          </option>
        </Show>
        <For each={props.options}>
          {(option) => (
            <option value={option.id} selected={option.id === props.value}>
              {option.name}
            </option>
          )}
        </For>
      </select>
    </label>
  );
}

function Controls(props: {
  state: DeviceState;
  settings: DeviceSettings | null;
  connected: boolean;
  pending: string[];
  onControl: (control: Control) => Promise<void>;
}) {
  const [zoom, setZoom] = createSignal("");
  const disabled = (key: string) => !props.connected || props.pending.includes(key);
  const confirmed = async (question: string, action: string, control: Control) => {
    if (await ask(question, action)) {
      await props.onControl(control);
    }
  };
  const presets = () =>
    (props.settings?.bitratePresets ?? []).map((preset) => ({
      id: preset.id,
      name: formatBits(preset.bitrate),
    }));
  const submitZoom = (event: SubmitEvent) => {
    event.preventDefault();
    const x = Number(zoom());
    if (Number.isFinite(x) && x > 0) {
      void props.onControl({ setZoom: { x } });
    }
  };

  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Controls")}</h2>
      <div class="flex flex-wrap gap-2">
        <Toggle
          label={t("Live")}
          on={props.state.streaming}
          live
          disabled={disabled("setLive")}
          onToggle={(on) =>
            void confirmed(
              on ? t("Go live on this device?") : t("End the stream on this device?"),
              on ? t("Go live") : t("End stream"),
              { setLive: { on } },
            )
          }
        />
        <Toggle
          label={t("Recording")}
          on={props.state.recording}
          disabled={disabled("setRecord")}
          onToggle={(on) =>
            void confirmed(
              on ? t("Start recording on this device?") : t("Stop recording on this device?"),
              on ? t("Start recording") : t("Stop recording"),
              { setRecord: { on } },
            )
          }
        />
        <Toggle
          label={t("Muted")}
          on={props.state.muted}
          disabled={disabled("setMute")}
          onToggle={(on) => void props.onControl({ setMute: { on } })}
        />
        <Toggle
          label={t("Stealth mode")}
          on={props.state.stealthMode}
          disabled={disabled("setStealthMode")}
          onToggle={(on) => void props.onControl({ setStealthMode: { on } })}
        />
        <Toggle
          label={t("Torch")}
          on={props.state.torchOn}
          disabled={disabled("setTorch")}
          onToggle={(on) => void props.onControl({ setTorch: { on } })}
        />
      </div>
      <Show
        when={props.settings ?? (props.connected ? null : emptySettings)}
        fallback={<Spinner />}
      >
        {(settings) => (
          <>
            <div class="grid gap-4 sm:grid-cols-2">
              <Picker
                label={t("Scene")}
                value={props.state.scene}
                options={settings().scenes}
                disabled={disabled("setScene")}
                onPick={(id) => props.onControl({ setScene: { id } })}
              />
              <Picker
                label={t("Mic")}
                value={props.state.mic}
                options={settings().mics}
                disabled={disabled("setMic")}
                onPick={(id) => props.onControl({ setMic: { id } })}
              />
              <Picker
                label={t("Bitrate")}
                value={props.state.bitrate}
                options={presets()}
                disabled={disabled("setBitratePreset")}
                onPick={(id) => props.onControl({ setBitratePreset: { id } })}
              />
              <Picker
                label={t("Zoom preset")}
                value={props.state.zoomPreset}
                options={props.state.zoomPresets ?? []}
                disabled={disabled("setZoomPreset")}
                onPick={(id) => props.onControl({ setZoomPreset: { id } })}
              />
            </div>
            <form class="flex flex-wrap items-end gap-2" onSubmit={submitZoom}>
              <label class="flex flex-col gap-2">
                <span class="text-[15px] font-semibold text-ink-2">
                  {props.state.zoom === undefined
                    ? t("Zoom")
                    : t("Zoom (now {zoom}x)", { zoom: props.state.zoom.toFixed(1) })}
                </span>
                <input
                  type="number"
                  inputmode="decimal"
                  min="0.5"
                  max="20"
                  step="0.1"
                  placeholder={props.state.zoom?.toFixed(1) ?? "1.0"}
                  value={zoom()}
                  onInput={(event) => setZoom(event.currentTarget.value)}
                  class={`${inputClass} w-32`}
                />
              </label>
              <button
                type="submit"
                class={ghostButtonClass}
                disabled={disabled("setZoom") || zoom() === ""}
              >
                {t("Set zoom")}
              </button>
            </form>
            <div class="flex flex-col gap-3">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <h3 class="text-[15px] font-semibold text-ink-2">
                  {t("SRTLA connection priorities")}
                </h3>
                <Toggle
                  label={settings().srt.connectionPrioritiesEnabled ? t("On") : t("Off")}
                  on={settings().srt.connectionPrioritiesEnabled}
                  disabled={disabled("setSrtConnectionPrioritiesEnabled")}
                  onToggle={(enabled) =>
                    void props.onControl({ setSrtConnectionPrioritiesEnabled: { enabled } })
                  }
                />
              </div>
              <Show
                when={settings().srt.connectionPriorities.length > 0}
                fallback={
                  <p class="text-[15px] text-muted">
                    {t("The stream has no connection priorities.")}
                  </p>
                }
              >
                <ul class="flex flex-col divide-y-[1.5px] divide-line rounded-2xl border-[1.5px] border-line">
                  <For each={settings().srt.connectionPriorities}>
                    {(connection) => (
                      <li class="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
                        <label class="flex min-w-32 flex-1 items-center gap-3">
                          <input
                            type="checkbox"
                            class="size-5 accent-leaf"
                            checked={connection.enabled}
                            disabled={disabled(`setSrtConnectionPriority:${connection.id}`)}
                            onChange={async (event) => {
                              const checkbox = event.currentTarget;
                              await props.onControl({
                                setSrtConnectionPriority: {
                                  id: connection.id,
                                  priority: connection.priority,
                                  enabled: checkbox.checked,
                                },
                              });
                              checkbox.checked = connection.enabled;
                            }}
                          />
                          <span class="font-semibold">{connection.name}</span>
                        </label>
                        <label class="flex items-center gap-3">
                          <span class="text-[15px] text-muted">{t("Priority")}</span>
                          <input
                            type="range"
                            min="1"
                            max="10"
                            step="1"
                            class="w-32 accent-leaf"
                            value={connection.priority}
                            disabled={
                              disabled(`setSrtConnectionPriority:${connection.id}`) ||
                              !connection.enabled
                            }
                            onChange={async (event) => {
                              const slider = event.currentTarget;
                              await props.onControl({
                                setSrtConnectionPriority: {
                                  id: connection.id,
                                  priority: Number(slider.value),
                                  enabled: connection.enabled,
                                },
                              });
                              slider.value = String(connection.priority);
                            }}
                          />
                          <span class="w-5 font-mono text-[15px]">{connection.priority}</span>
                        </label>
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
            </div>
          </>
        )}
      </Show>
      <div class="flex flex-wrap gap-2">
        <button
          type="button"
          class={ghostButtonClass}
          disabled={disabled("reloadBrowserWidgets")}
          onClick={() => void props.onControl({ reloadBrowserWidgets: {} })}
        >
          {t("Reload browser widgets")}
        </button>
      </div>
    </div>
  );
}

function DeviceView(props: {
  session: string;
  page: string;
  device: string;
  nameOf: (device: string) => string | undefined;
  onSignOut: () => void;
}) {
  const [loaded, setLoaded] = createSignal(false);
  const [shown, setShown] = createSignal({ device: "", name: "" });
  const [connected, setConnected] = createSignal(false);
  const [streaming, setStreaming] = createSignal(false);
  const [status, setStatus] = createSignal<DeviceStatus>(emptyStatus());
  const [state, setState] = createSignal<DeviceState>({});
  const [settings, setSettings] = createSignal<DeviceSettings | null>(null);
  const [history, setHistory] = createSignal<Sample[]>([]);
  const [pending, setPending] = createSignal<string[]>([]);

  createEffect(() => {
    let fresh = true;
    const device = props.device;
    const stop = subscribe(deviceEventsUrl(props.page, device), props.session, {
      onOpen: () => setStreaming(true),
      onEvent: (kind, data) => {
        switch (kind) {
          case "snapshot": {
            const snapshot = data as DeviceSnapshot;
            if (fresh) {
              fresh = false;
              setStatus(emptyStatus());
              setState({});
              setSettings(null);
              setHistory([]);
            }
            setLoaded(true);
            setShown({ device, name: snapshot.name });
            setConnected(snapshot.connected);
            if (snapshot.connected) {
              setStatus((current) => ({
                general: filled(snapshot.status.general) ?? current.general,
                topLeft: filled(snapshot.status.topLeft) ?? current.topLeft,
                topRight: filled(snapshot.status.topRight) ?? current.topRight,
              }));
              setState((current) => ({ ...current, ...snapshot.state }));
              setSettings((current) => snapshot.settings ?? current);
            } else {
              setStatus(emptyStatus());
              setState({});
              setSettings(snapshot.settings);
            }
            break;
          }
          case "status": {
            const update = data as StatusUpdate;
            setStatus((current) => ({
              general: update.general ?? current.general,
              topLeft: update.topLeft ?? current.topLeft,
              topRight: update.topRight ?? current.topRight,
            }));
            if (update.sample) {
              const sample = update.sample;
              setHistory((current) => [...current.slice(-59), sample]);
            }
            break;
          }
          case "state":
            setState((current) => ({ ...current, ...(data as { data: DeviceState }).data }));
            break;
          case "settings":
            setSettings((data as { data: DeviceSettings }).data);
            break;
          case "connection": {
            const connected = (data as { connected: boolean }).connected;
            setConnected(connected);
            if (!connected) {
              setStatus(emptyStatus());
              setState({});
            }
            break;
          }
          default:
            break;
        }
      },
      onError: (code) => {
        setStreaming(false);
        if (code === 401) {
          props.onSignOut();
        }
      },
    });
    onCleanup(stop);
  });

  const control = async (control: Control) => {
    const kind = Object.keys(control)[0] ?? "";
    const key =
      "setSrtConnectionPriority" in control
        ? `${kind}:${control.setSrtConnectionPriority.id}`
        : kind;
    setPending((current) => [...current, key]);
    try {
      await controlDevice(props.session, props.page, props.device, control);
    } catch (error) {
      if (error instanceof BackendError && error.status === 401) {
        props.onSignOut();
      }
    } finally {
      setPending((current) => current.filter((pending) => pending !== key));
    }
  };

  const online = () => connected() && streaming();
  const sections: Section[] = [
    {
      id: "controls",
      label: t("Controls"),
      icon: "remote",
      status: () =>
        state().streaming === undefined
          ? null
          : { on: state().streaming === true, text: state().streaming ? t("Live") : t("Not live") },
    },
    {
      id: "status",
      label: t("Status"),
      icon: "smartphone",
      status: () => ({ on: online(), text: online() ? t("Connected") : t("Not connected") }),
    },
    {
      id: "graphs",
      label: t("Graphs"),
      icon: "chart",
      status: () => {
        const bitrate = history().at(-1)?.bitrate;
        return bitrate === null || bitrate === undefined
          ? null
          : { on: online(), text: formatBits(bitrate) };
      },
    },
  ];
  const { active, select } = createSectionChoice(() => sections);

  return (
    <>
      <Show when={loaded() && streaming() && !connected()}>
        <div class="cheer-fade fixed inset-0 z-10 flex items-center justify-center bg-page/60 px-4 backdrop-blur-md">
          <div
            role="status"
            class="cheer-pop flex max-w-md flex-col items-center gap-4 rounded-3xl border-[1.5px] border-line bg-card px-10 py-8 text-center shadow-2xl"
          >
            <span class="grid size-16 place-items-center rounded-full bg-ghost text-muted">
              <Icon name="smartphone" class="size-8" />
            </span>
            <span class="font-display text-2xl font-semibold text-ink">
              {t("{name} is not connected", { name: props.nameOf(shown().device) ?? shown().name })}
            </span>
            <span class="text-[15px] text-ink-2">
              {t("This page updates as soon as it connects again.")}
            </span>
          </div>
        </div>
      </Show>
      <Show when={loaded()}>
        <div class="grid gap-6 lg:grid-cols-[260px_minmax(0,1fr)] lg:items-start">
          <SectionMenu
            label={t("Device page")}
            sections={sections}
            active={active()}
            onSelect={select}
          />
          <div class="min-w-0">
            <div hidden={active() !== "controls"}>
              <Controls
                state={state()}
                settings={settings()}
                connected={online()}
                pending={pending()}
                onControl={control}
              />
            </div>
            <div hidden={active() !== "status"}>
              <StatusCard status={status()} connected={online()} />
            </div>
            <div hidden={active() !== "graphs"}>
              <Graphs history={history()} />
            </div>
          </div>
        </div>
      </Show>
    </>
  );
}

export default function DevicePage(props: { page: string; device: string }) {
  const { session, loading, signOut } = createAccountSession();

  createEffect(() => {
    if (!loading() && !session()) {
      location.replace(asset(pages.dashboard));
    }
  });

  const [device, setDevice] = createSignal(props.device);
  const onPopState = () => {
    const match = location.pathname.match(/\/device\/[^/]+\/([^/]+)\/?$/);
    if (match) {
      setDevice(decodeURIComponent(match[1]!));
    }
  };
  addEventListener("popstate", onPopState);
  onCleanup(() => removeEventListener("popstate", onPopState));
  const pick = (picked: string) => {
    if (picked !== device()) {
      history.pushState(null, "", `${deviceUrl(props.page, picked)}${location.hash}`);
      setDevice(picked);
    }
  };

  const [devices, setDevices] = createSignal<Device[]>([]);
  createEffect(() => {
    const current = session();
    if (current) {
      const refresh = () =>
        fetchAccount(current.session, props.page)
          .then((account) => setDevices(account.devices))
          .catch(() => {});
      void refresh();
      onCleanup(subscribeChanges(pageEventsUrl(props.page), current.session, { account: refresh }));
    }
  });

  const channel = (account: Account) =>
    account.moderating.find((channel) => channel.id === props.page) ?? null;
  const dashboardOf = (picked: Moderating | null) =>
    asset(picked ? `${pages.dashboard}${channelKey(picked)}` : pages.dashboard);
  const dashboard = (account: Account) => dashboardOf(channel(account));

  createEffect(() => {
    const current = session();
    if (current) {
      const picked = channel(current.account);
      rememberChannel(picked ? channelKey(picked) : "");
    }
  });

  return (
    <>
      <CloudHeader premium={session()?.account.premium}>
        <Show when={session()}>
          {(current) => (
            <div class="ml-auto flex min-w-0 items-center gap-3">
              <DevicePicker devices={devices()} picked={device()} onPick={pick} />
              <Show when={current().account.moderating.length > 0}>
                <StreamerPicker
                  account={current().account}
                  picked={channel(current().account)}
                  onPick={(key) => {
                    rememberChannel(key);
                    location.assign(
                      `${dashboardOf(
                        current().account.moderating.find(
                          (channel) => channelKey(channel) === key,
                        ) ?? null,
                      )}#moblin`,
                    );
                  }}
                />
              </Show>
              <AccountMenu
                name={current().account.name}
                avatar={current().account.avatar}
                items={[
                  { label: t("Dashboard"), icon: "chart", href: dashboard(current().account) },
                ]}
                onSignOut={signOut}
              />
            </div>
          )}
        </Show>
      </CloudHeader>
      <main>
        <Band tight>
          <Show when={!loading()} fallback={<Spinner />}>
            <Show when={session()} keyed>
              {(current) => (
                <DeviceView
                  session={current.session}
                  page={props.page}
                  device={device()}
                  nameOf={(id) => devices().find((candidate) => candidate.id === id)?.name}
                  onSignOut={signOut}
                />
              )}
            </Show>
          </Show>
        </Band>
      </main>
    </>
  );
}
