import { createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { cardClass } from "./PageParts";
import type { Sample } from "../data/account";
import { formatBits } from "../data/moblin";
import { t } from "../i18n";

const historyWindow = 60 * 1000;

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
  fill?: boolean;
}) {
  const [width, setWidth] = createSignal(600);
  const [height, setHeight] = createSignal(180);
  const [hover, setHover] = createSignal<number | null>(null);
  let container: HTMLDivElement | undefined;
  const left = 56;
  const right = 12;
  const top = 12;
  const bottom = 24;

  onMount(() => {
    if (!container) {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const measured = entries[0]?.contentRect;
      if (measured?.width) {
        setWidth(Math.max(240, measured.width));
      }
      if (props.fill && measured?.height) {
        setHeight(Math.max(80, measured.height));
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
  const y = (value: number) => top + (1 - value / max()) * (height() - top - bottom);
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
    <div class={`flex flex-col gap-2 ${props.fill ? "min-h-0 flex-1" : ""}`}>
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
        class={`relative w-full ${props.fill ? "min-h-0 flex-1" : ""}`}
        onPointerMove={onPointer}
        onPointerLeave={() => setHover(null)}
      >
        <Show
          when={hasData()}
          fallback={<p class="py-6 text-center text-[15px] text-muted">{props.empty}</p>}
        >
          <svg
            width={width()}
            height={height()}
            class={`block overflow-visible ${props.fill ? "absolute top-0 left-0" : ""}`}
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
                <text
                  x={x(at)}
                  y={height() - 6}
                  text-anchor="middle"
                  class="fill-muted text-[11px]"
                >
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
                y2={height() - bottom}
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

export default function Graphs(props: { history: Sample[]; compact?: boolean }) {
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
    <div class={props.compact ? "flex h-full min-h-0 flex-col gap-3 p-3" : cardClass}>
      <Show when={!props.compact}>
        <h2 class="text-[23px] font-semibold">{t("Last minute")}</h2>
      </Show>
      <TimeChart
        title={t("Bitrate")}
        times={times()}
        series={bitrate()}
        format={formatBits}
        fill={props.compact}
        empty={t("No bitrate yet. It is shown while the stream is live.")}
      />
      <Show when={names().length > 0}>
        <TimeChart
          title={t("Round trip time")}
          times={times()}
          series={rtts()}
          format={formatMilliseconds}
          fill={props.compact}
          empty={t("No round trip times yet.")}
        />
      </Show>
    </div>
  );
}
