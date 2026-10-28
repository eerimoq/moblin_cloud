import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { asset } from "../asset";
import {
  channelKey,
  DevicePicker,
  rememberChannel,
  StreamerPicker,
} from "../components/HeaderPickers";
import Icon from "../components/Icon";
import MoblinScreen from "../components/MoblinScreen";
import SectionMenu, { createSectionChoice, type Section } from "../components/SectionMenu";
import { AccountMenu, Band, cardClass, CloudHeader, Spinner } from "../components/PageParts";
import {
  controlDevice,
  fetchAccount,
  pageEventsUrl,
  createAccountSession,
  deviceEventsUrl,
  devicePreviewUrl,
  type Account,
  type Device,
  type Control,
  type DeviceSettings,
  type DeviceSnapshot,
  type DeviceState,
  type DeviceStatus,
  type Sample,
  type StatusUpdate,
} from "../data/account";
import { BackendError } from "../data/backend";
import { formatBits } from "../data/moblin";
import { deviceUrl, pages } from "../data/links";
import { t } from "../i18n";
import { subscribe, subscribeChanges } from "../sse";

const emptyStatus = (): DeviceStatus => ({ general: {}, topLeft: {}, topRight: {} });

const filled = <T extends object>(part: T) => (Object.keys(part).length > 0 ? part : undefined);

const historyWindow = 60 * 1000;

const perItem = ["setSrtConnectionPriority", "startMacro", "stopMacro"];

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

function DeviceView(props: {
  session: string;
  page: string;
  device: string;
  preview: boolean | null;
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
  const [preview, setPreview] = createSignal<string | null>(null);
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
              setPreview(null);
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
    const body = Object.values(control)[0] as { id?: string };
    const key = perItem.includes(kind) ? `${kind}:${body.id}` : kind;
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
      id: "phone",
      label: t("Phone"),
      icon: "screen",
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

  createEffect(() => {
    if (active() !== "phone" || props.preview !== true) {
      return;
    }
    const stop = subscribe(devicePreviewUrl(props.page, props.device), props.session, {
      onEvent: (kind, data) => {
        if (kind === "frame") {
          setPreview((data as { image: string }).image);
        }
      },
    });
    onCleanup(() => {
      stop();
      setPreview(null);
    });
  });

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
            <div hidden={active() !== "phone"}>
              <div class={cardClass}>
                <h2 class="text-[23px] font-semibold">{t("Phone")}</h2>
                <p class="text-[15px] text-ink-2">
                  {t(
                    "What the Moblin app shows on the device, with the same buttons to control it.",
                  )}
                </p>
                <Show when={props.preview === false}>
                  <p class="text-[15px] text-muted">
                    {t("The streamer has not let you see the stream preview.")}
                  </p>
                </Show>
                <MoblinScreen
                  page={props.page}
                  state={state()}
                  status={status()}
                  settings={settings()}
                  preview={preview()}
                  connected={online()}
                  pending={pending()}
                  onControl={control}
                />
              </div>
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
    const match = location.pathname.match(/\/device\/[^/]+(?:\/([^/]+))?\/?$/);
    if (match) {
      setDevice(decodeURIComponent(match[1] ?? ""));
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
  const [preview, setPreview] = createSignal<boolean | null>(null);
  createEffect(() => {
    const current = session();
    if (current) {
      const refresh = () =>
        fetchAccount(current.session, props.page)
          .then((account) => {
            setDevices(account.devices);
            setPreview(account.preview);
          })
          .catch(() => {});
      void refresh();
      onCleanup(subscribeChanges(pageEventsUrl(props.page), current.session, { account: refresh }));
    }
  });

  const channel = (account: Account) =>
    account.moderating.find((channel) => channel.id === props.page) ?? null;
  const dashboard = (account: Account) => {
    const picked = channel(account);
    return asset(picked ? `${pages.dashboard}${channelKey(picked)}` : pages.dashboard);
  };

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
              <Show when={devices().length > 0}>
                <DevicePicker devices={devices()} picked={device()} onPick={pick} />
              </Show>
              <Show when={current().account.moderating.length > 0}>
                <StreamerPicker
                  account={current().account}
                  picked={channel(current().account)}
                  onPick={async (key) => {
                    rememberChannel(key);
                    const picked =
                      current().account.moderating.find((channel) => channelKey(channel) === key) ??
                      null;
                    const page = picked ? picked.id : current().account.settings.id;
                    const first = await fetchAccount(current().session, page)
                      .then((account) => account.devices[0]?.id ?? "")
                      .catch(() => "");
                    location.assign(`${deviceUrl(page, first)}${location.hash}`);
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
            <Show when={device() ? session() : undefined} keyed>
              {(current) => (
                <DeviceView
                  session={current.session}
                  page={props.page}
                  device={device()}
                  preview={preview()}
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
