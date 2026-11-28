import { createSignal, For, Show } from "solid-js";
import Icon from "./Icon";
import { cardClass, ghostButtonClass, Spinner } from "./PageParts";
import { recentKinds, type Recent, type RecentKind, type RecentList } from "../data/account";
import { formatPrice, providerName, spinsText } from "../data/remoteControl";
import { effectName } from "../data/effects";
import { youTubeUrl } from "../data/youtube";
import { language, t } from "../i18n";
import { sameData } from "../same";

const relativeTime = new Intl.RelativeTimeFormat(language, { numeric: "auto" });

const [now, setNow] = createSignal(Date.now());
export { now };
setInterval(() => setNow(Date.now()), 60_000);

export function timeAgo(at: number) {
  const seconds = Math.max(0, Math.round(now() / 1000 - at));
  if (seconds < 60) {
    return t("Just now");
  }
  if (seconds < 3600) {
    return relativeTime.format(-Math.round(seconds / 60), "minute");
  }
  if (seconds < 86_400) {
    return relativeTime.format(-Math.round(seconds / 3600), "hour");
  }
  return relativeTime.format(-Math.round(seconds / 86_400), "day");
}

export const amountLabel = (item: Recent) =>
  item.kickSubscriber
    ? t("Kick subscriber")
    : item.tier
      ? t("Tier {tier} subscriber", { tier: item.tier })
      : formatPrice({ price: item.amount, currency: item.currency });

export function VideoLink(props: {
  video: string;
  start?: number;
  link?: string;
  title: string;
  class?: string;
}) {
  return (
    <a
      href={props.link ?? youTubeUrl(props.video, props.start ?? 0)}
      target="_blank"
      rel="noopener noreferrer"
      class={["underline decoration-line underline-offset-2 hover:text-ink", props.class]}
    >
      {props.title}
    </a>
  );
}

export function ReceivedDevices(props: {
  devices: NonNullable<Recent["devices"]>;
  compact?: boolean;
}) {
  return (
    <ul
      class={[
        "flex flex-wrap",
        props.compact ? "gap-x-2.5 text-[12px]" : "gap-x-3 gap-y-0.5 text-[15px]",
      ]}
    >
      <For keyed={false} each={props.devices}>
        {(device) => (
          <li
            class={[
              "flex items-center",
              props.compact ? "gap-0.5" : "gap-1",
              device().received ? "text-leaf" : "text-muted",
            ]}
          >
            <Icon
              name={device().received ? "check" : "clock"}
              class={`${props.compact ? "size-3" : "size-4"} shrink-0`}
            />
            <span class="sr-only">
              {device().received ? t("Received:") : t("Not received yet:")}
            </span>
            {device().name}
          </li>
        )}
      </For>
    </ul>
  );
}

export function Status(props: { item: Recent; compact?: boolean }) {
  const status = (): {
    text: string;
    icon: "check" | "clock" | "warning" | "close";
    color: string;
  } | null => {
    const item = props.item;
    if (item.status === "held") {
      return { text: t("Waiting for approval"), icon: "clock", color: "bg-well text-muted" };
    }
    switch (item.outcome) {
      case "finished":
        return { text: t("Played"), icon: "check", color: "bg-well text-leaf" };
      case "skipped":
        return { text: t("Skipped"), icon: "close", color: "bg-well text-muted" };
      case "failedToStart":
        return {
          text: t("Did not start playing"),
          icon: "warning",
          color: "bg-danger-bg text-danger",
        };
      case "timedOut":
        return {
          text: t("Stopped, as it did not end in time"),
          icon: "warning",
          color: "bg-danger-bg text-danger",
        };
    }
    if (item.status === "waiting" && !item.devices?.length) {
      return { text: t("Waiting for Moblin"), icon: "clock", color: "bg-well text-muted" };
    }
    return item.video
      ? { text: t("Not played yet"), icon: "clock", color: "bg-amber-bg text-amber" }
      : null;
  };
  return (
    <Show when={status()}>
      {(status) => (
        <span
          class={[
            "inline-flex items-center self-start rounded-full font-semibold",
            props.compact ? "gap-1 px-2 py-0.5 text-[12px]" : "gap-1.5 px-3 py-1 text-[14px]",
            status().color,
          ]}
        >
          <Icon name={status().icon} class={`${props.compact ? "size-3" : "size-4"} shrink-0`} />
          {status().text}
        </span>
      )}
    </Show>
  );
}

export function createResend(
  send: (item: Recent) => Promise<void>,
  onError: (error: unknown) => void = () => {},
) {
  const [resending, setResending] = createSignal<string | null>(null);
  const [failed, setFailed] = createSignal<string | null>(null);
  const resend = async (item: Recent) => {
    if (resending()) {
      return;
    }
    setFailed(null);
    setResending(item.id);
    try {
      await send(item);
    } catch (error) {
      onError(error);
      setFailed(item.id);
    } finally {
      setResending(null);
    }
  };
  return { resending, failed, resend };
}

export function Latest(props: {
  kind: RecentKind;
  list: RecentList | undefined;
  onMore: () => Promise<void>;
  onResend: (item: Recent) => Promise<void>;
}) {
  const [loading, setLoading] = createSignal(false);
  const { resending, failed, resend } = createResend((item) => props.onResend(item));
  const title = () =>
    ({
      tip: t("Received messages"),
      spin: t("Received spins"),
      effect: t("Received effects"),
      media: t("Received videos"),
    })[props.kind];
  const empty = () =>
    ({
      tip: t("No messages yet."),
      spin: t("No spins yet."),
      effect: t("No effects yet."),
      media: t("No videos yet."),
    })[props.kind];
  const more = async () => {
    setLoading(true);
    try {
      await props.onMore();
    } finally {
      setLoading(false);
    }
  };
  return (
    <section class={cardClass}>
      <div class="flex flex-col gap-1">
        <h2 class="text-[23px] font-semibold">{title()}</h2>
        <p class="text-[15px] text-muted">{t("From the last 30 days.")}</p>
      </div>
      <Show when={props.list} fallback={<Spinner />}>
        {(list) => (
          <Show when={list().items.length > 0} fallback={<p class="text-ink-2">{empty()}</p>}>
            <ul class="flex flex-col divide-y divide-line">
              <For each={list().items}>
                {(item) => (
                  <li class="flex flex-col gap-2 py-4 first:pt-0 last:pb-0">
                    <div class="flex items-baseline justify-between gap-4">
                      <span class="flex min-w-0 flex-wrap items-baseline gap-x-2">
                        <span class="font-semibold wrap-anywhere">{item.user}</span>
                        <span class="text-ink-2">
                          {amountLabel(item)}
                          <Show when={item.provider}>
                            {(provider) => (
                              <span class="text-muted">
                                {" · "}
                                {providerName(provider())}
                              </span>
                            )}
                          </Show>
                        </span>
                      </span>
                      <span class="shrink-0 text-[15px] text-muted">{timeAgo(item.at)}</span>
                    </div>
                    <Show when={item.spins}>
                      {(spins) => <span class="text-ink-2">{spinsText(spins())}</span>}
                    </Show>
                    <Show when={item.effect}>
                      {(effect) => (
                        <span class="flex items-center gap-1.5 text-ink-2">
                          <Icon name="play" class="size-4 shrink-0" />
                          {effectName(effect())}
                        </span>
                      )}
                    </Show>
                    <Show when={item.video}>
                      {(video) => (
                        <span class="flex items-start gap-1.5 text-ink">
                          <Icon name="play" class="mt-1 size-4 shrink-0 text-ink-2" />
                          <VideoLink
                            video={video()}
                            link={item.link}
                            title={item.title ?? video()}
                            class="line-clamp-2 wrap-anywhere"
                          />
                        </span>
                      )}
                    </Show>
                    <Show when={item.message}>
                      {(message) => <span class="text-ink-2 wrap-anywhere">{message()}</span>}
                    </Show>
                    <div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
                      <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
                        <Status item={item} />
                        <Show when={item.devices && item.devices.length > 0 && item.devices}>
                          {(devices) => <ReceivedDevices devices={devices()} />}
                        </Show>
                      </div>
                      <Show when={item.status !== "held"}>
                        <button
                          type="button"
                          class={[ghostButtonClass, "ml-auto h-9! px-4! text-[15px]!"]}
                          disabled={resending() !== null}
                          onClick={() => resend(item)}
                        >
                          {resending() === item.id ? t("Sending…") : t("Send again")}
                        </button>
                      </Show>
                    </div>
                    <Show when={failed() === item.id}>
                      <span class="text-[15px] text-danger">
                        {t("Sending did not work. Try again.")}
                      </span>
                    </Show>
                  </li>
                )}
              </For>
            </ul>
            <Show when={list().more}>
              <button
                type="button"
                class={[ghostButtonClass, "self-start"]}
                disabled={loading()}
                onClick={more}
              >
                {loading() ? t("Loading…") : t("Show more")}
              </button>
            </Show>
          </Show>
        )}
      </Show>
    </section>
  );
}

export function createRecent(
  fetchAll: (limit: number) => Promise<Record<RecentKind, RecentList>>,
  fetchList: (kind: RecentKind, limit: number, before?: string) => Promise<RecentList>,
  onError: (error: unknown) => void,
) {
  const [recent, setRecent] = createSignal<Record<RecentKind, RecentList> | null>(null);

  const loadRecent = async () => {
    const shown = recent();
    try {
      const fetched = await fetchAll(
        Math.min(Math.max(...recentKinds.map((kind) => shown?.[kind]?.items.length ?? 0), 5), 50),
      );
      const merge = (fetched: RecentList, old: RecentList | undefined) => {
        const kept = new Map(old?.items.map((item) => [item.id, item]));
        const page = {
          ...fetched,
          items: fetched.items.map((item) => {
            const known = kept.get(item.id);
            return known && sameData(known, item) ? known : item;
          }),
        };
        const last = page.items[page.items.length - 1]?.id;
        const index = old && page.more ? old.items.findIndex((item) => item.id === last) : -1;
        if (!old || index < 0 || index === old.items.length - 1) {
          return page;
        }
        return { items: [...page.items, ...old.items.slice(index + 1)], more: old.more };
      };
      const empty: RecentList = { items: [], more: false };
      setRecent((old) => ({
        tip: merge(fetched.tip, old?.tip),
        spin: merge(fetched.spin, old?.spin),
        effect: merge(fetched.effect, old?.effect),
        media: merge(fetched.media ?? empty, old?.media),
      }));
    } catch (error) {
      onError(error);
    }
  };

  const loadMore = async (kind: RecentKind) => {
    const items = recent()?.[kind].items ?? [];
    try {
      const page = await fetchList(kind, 20, items[items.length - 1]?.id);
      setRecent((old) => {
        if (!old) {
          return old;
        }
        const ids = new Set(old[kind].items.map((item) => item.id));
        return {
          ...old,
          [kind]: {
            items: [...old[kind].items, ...page.items.filter((item) => !ids.has(item.id))],
            more: page.more,
          },
        };
      });
    } catch (error) {
      onError(error);
    }
  };

  return { recent, loadRecent, loadMore };
}
