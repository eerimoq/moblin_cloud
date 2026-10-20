import { createSignal, For, Show } from "solid-js";
import Icon from "./Icon";
import { cardClass, ghostButtonClass, Spinner } from "./PageParts";
import type { Recent, RecentKind, RecentList } from "../data/account";
import { formatPrice } from "../data/remoteControl";
import { language, t } from "../i18n";

const relativeTime = new Intl.RelativeTimeFormat(language, { numeric: "auto" });

function timeAgo(at: number) {
  const seconds = Math.max(0, Math.round(Date.now() / 1000 - at));
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

export function Latest(props: {
  kind: RecentKind;
  list: RecentList | undefined;
  onMore: () => Promise<void>;
  onResend: (item: Recent) => Promise<void>;
}) {
  const [loading, setLoading] = createSignal(false);
  const [resending, setResending] = createSignal<string | null>(null);
  const [failed, setFailed] = createSignal<string | null>(null);
  const title = () =>
    props.kind === "song"
      ? t("Received song requests")
      : props.kind === "tip"
        ? t("Received tips")
        : t("Received spins");
  const empty = () =>
    props.kind === "song"
      ? t("No song requests yet.")
      : props.kind === "tip"
        ? t("No tips yet.")
        : t("No spins yet.");
  const status = (item: Recent) =>
    item.status === "delivered"
      ? { text: t("In Moblin"), class: "text-leaf" }
      : item.status === "waiting"
        ? { text: t("Waiting for Moblin"), class: "text-muted" }
        : { text: t("Not added by Moblin"), class: "text-amber-ink" };
  const more = async () => {
    setLoading(true);
    try {
      await props.onMore();
    } finally {
      setLoading(false);
    }
  };
  const resend = async (item: Recent) => {
    if (resending()) {
      return;
    }
    setFailed(null);
    setResending(item.id);
    try {
      await props.onResend(item);
    } catch {
      setFailed(item.id);
    } finally {
      setResending(null);
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
                  <li class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 py-3 first:pt-0 last:pb-0">
                    <div class="flex min-w-0 flex-col gap-0.5">
                      <span class="flex flex-wrap items-baseline gap-x-2">
                        <span class="font-semibold">{item.user}</span>
                        <span class="text-ink-2">
                          {item.moderator
                            ? t("Moderator")
                            : item.tier
                              ? t("Tier {tier} subscriber", { tier: item.tier })
                              : formatPrice({ price: item.amount, currency: item.currency })}
                          <Show when={item.provider}>
                            {(provider) => (
                              <span class="text-muted">
                                {" · "}
                                {provider() === "paypal" ? "PayPal" : "Stripe"}
                              </span>
                            )}
                          </Show>
                        </span>
                      </span>
                      <Show when={item.name}>
                        {(name) => <span class="truncate text-ink-2">{name()}</span>}
                      </Show>
                      <Show when={item.spins}>
                        {(spins) => (
                          <span class="text-ink-2">
                            {spins() === 1 ? t("1 spin") : t("{count} spins", { count: spins() })}
                          </span>
                        )}
                      </Show>
                      <Show when={item.message}>
                        {(message) => <span class="text-ink-2 wrap-anywhere">{message()}</span>}
                      </Show>
                      <Show when={item.devices && item.devices.length > 1 && item.devices}>
                        {(devices) => (
                          <ul class="flex flex-wrap gap-x-3 gap-y-0.5 text-[15px]">
                            <For each={devices()}>
                              {(device) => (
                                <li
                                  class={`flex items-center gap-1 ${device.received ? "text-leaf" : "text-muted"}`}
                                >
                                  <Icon
                                    name={device.received ? "check" : "clock"}
                                    class="size-4 shrink-0"
                                  />
                                  <span class="sr-only">
                                    {device.received ? t("Received:") : t("Not received yet:")}
                                  </span>
                                  {device.name}
                                </li>
                              )}
                            </For>
                          </ul>
                        )}
                      </Show>
                    </div>
                    <div class="flex flex-col items-end gap-1 text-[15px]">
                      <div class="flex items-baseline gap-3">
                        <span class="text-muted">{timeAgo(item.at)}</span>
                        <span class={`font-semibold ${status(item).class}`}>
                          {status(item).text}
                        </span>
                      </div>
                      <button
                        type="button"
                        class={`${ghostButtonClass} h-9! px-4! text-[15px]!`}
                        disabled={resending() !== null}
                        onClick={() => resend(item)}
                      >
                        {resending() === item.id ? t("Sending…") : t("Send again")}
                      </button>
                      <Show when={failed() === item.id}>
                        <span class="text-danger">{t("Sending did not work. Try again.")}</span>
                      </Show>
                    </div>
                  </li>
                )}
              </For>
            </ul>
            <Show when={list().more}>
              <button
                type="button"
                class={`${ghostButtonClass} self-start`}
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
  fetchList: (kind: RecentKind, limit: number, before?: string) => Promise<RecentList>,
  onError: (error: unknown) => void,
  preview?: boolean,
) {
  const empty = { items: [], more: false };
  const [recent, setRecent] = createSignal<Record<RecentKind, RecentList> | null>(
    preview ? { song: empty, tip: empty, spin: empty } : null,
  );

  const loadRecent = async () => {
    const shown = recent();
    try {
      const [song, tip, spin] = await Promise.all(
        (["song", "tip", "spin"] as const).map((kind) =>
          fetchList(kind, Math.min(Math.max(shown?.[kind].items.length ?? 0, 5), 50)),
        ),
      );
      const merge = (page: RecentList, old: RecentList | undefined) => {
        const last = page.items[page.items.length - 1]?.id;
        const index = old && page.more ? old.items.findIndex((item) => item.id === last) : -1;
        if (!old || index < 0 || index === old.items.length - 1) {
          return page;
        }
        return { items: [...page.items, ...old.items.slice(index + 1)], more: old.more };
      };
      setRecent((old) => ({
        song: merge(song, old?.song),
        tip: merge(tip, old?.tip),
        spin: merge(spin, old?.spin),
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
