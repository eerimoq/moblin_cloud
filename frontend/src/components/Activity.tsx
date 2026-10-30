import { createEffect, createMemo, createSignal, For, on, onMount, Show } from "solid-js";
import Icon, { kindIcons } from "./Icon";
import { amountLabel, createRecent, createResend, ReceivedDevices, timeAgo } from "./Latest";
import { Spinner } from "./PageParts";
import { fetchRecent, resendRecent, type Recent, type RecentKind } from "../data/account";
import { isSignedOut } from "../data/backend";
import { effectName } from "../data/effects";
import { providerName, spinsText } from "../data/remoteControl";
import { t } from "../i18n";

const recentKinds: RecentKind[] = ["tip", "spin", "effect"];

const kindNames = (): Record<RecentKind, string> => ({
  tip: t("Message"),
  spin: t("Wheel of luck"),
  effect: t("Effect"),
});

type Item = Recent & { kind: RecentKind };

function ActivityRow(props: {
  item: Item;
  fresh: boolean;
  resending: boolean;
  disabled: boolean;
  failed: boolean;
  onResend: () => void;
}) {
  const [expanded, setExpanded] = createSignal(false);
  const item = () => props.item;
  return (
    <li class={`flex gap-2.5 px-3 py-2 ${props.fresh ? "arrival-flash" : ""}`}>
      <Icon
        name={kindIcons[item().kind]}
        class="mt-0.5 size-4 shrink-0 text-leaf"
        aria-label={kindNames()[item().kind]}
      />
      <div class="flex min-w-0 flex-1 flex-col gap-0.5 text-[14px] leading-snug">
        <div class="flex items-baseline gap-2">
          <span class="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-1.5">
            <span class="truncate font-semibold text-ink">{item().user}</span>
            <span class="text-ink-2">{amountLabel(item())}</span>
            <Show when={item().spins}>
              {(spins) => <span class="text-ink-2">· {spinsText(spins())}</span>}
            </Show>
            <Show when={item().effect}>
              {(effect) => <span class="text-ink-2">· {effectName(effect())}</span>}
            </Show>
            <Show when={item().provider}>
              {(provider) => <span class="text-muted">· {providerName(provider())}</span>}
            </Show>
          </span>
          <span class="shrink-0 text-[12px] text-muted">{timeAgo(item().at)}</span>
          <button
            type="button"
            title={t("Send again")}
            aria-label={t("Send again")}
            disabled={props.disabled}
            onClick={() => props.onResend()}
            class="-my-1 grid size-7 shrink-0 place-items-center rounded-full text-ink-2 transition-colors hover:bg-ghost-hover hover:text-ink disabled:opacity-40"
          >
            <Icon name="reload" class={`size-4 ${props.resending ? "animate-spin" : ""}`} />
          </button>
        </div>
        <Show when={item().message}>
          {(message) => (
            <button
              type="button"
              onClick={() => setExpanded(!expanded())}
              class={`text-left text-ink wrap-anywhere ${expanded() ? "" : "line-clamp-2"}`}
            >
              {message()}
            </button>
          )}
        </Show>
        <Show
          when={item().devices && item().devices!.length > 0 && item().devices}
          fallback={
            <Show when={item().status === "waiting"}>
              <span class="text-[12px] text-muted">{t("Waiting for Moblin")}</span>
            </Show>
          }
        >
          {(devices) => <ReceivedDevices devices={devices()} compact />}
        </Show>
        <Show when={props.failed}>
          <span class="text-[12px] text-danger">{t("Sending did not work. Try again.")}</span>
        </Show>
      </div>
    </li>
  );
}

export default function Activity(props: {
  session: string;
  page: string | undefined;
  kinds: RecentKind[] | undefined;
  recentChanges: number;
  onKinds: (kinds: RecentKind[] | undefined) => void;
  onSignOut: () => void;
}) {
  const guard = (error: unknown) => {
    if (isSignedOut(error)) {
      props.onSignOut();
    }
  };
  const { recent, loadRecent, loadMore } = createRecent(
    (kind, limit, before) => fetchRecent(props.session, props.page, kind, limit, before),
    guard,
  );
  const [loading, setLoading] = createSignal(false);
  let latest: number | null = null;
  const arrivals = new Map<string, number>();
  const stable = new Map<string, { json: string; item: Item }>();

  onMount(() => void loadRecent());
  createEffect(
    on(
      () => props.recentChanges,
      () => void loadRecent(),
      { defer: true },
    ),
  );

  const shownKinds = () => props.kinds ?? recentKinds;

  const items = createMemo(() => {
    const lists = recent();
    if (!lists) {
      return null;
    }
    const all = recentKinds.flatMap((kind) => lists[kind].items);
    const newest = Math.max(latest ?? 0, ...all.map((item) => item.at));
    if (latest !== null) {
      const since = latest;
      all
        .filter((item) => item.at > since && !arrivals.has(item.id))
        .forEach((item) => arrivals.set(item.id, Date.now()));
    }
    latest = newest;
    const kinds = shownKinds();
    let oldest = 0;
    for (const kind of kinds) {
      const list = lists[kind];
      const last = list.items[list.items.length - 1];
      if (list.more && last) {
        oldest = Math.max(oldest, last.at);
      }
    }
    return kinds
      .flatMap((kind) => lists[kind].items.map((item) => ({ ...item, kind })))
      .filter((item) => item.at >= oldest)
      .sort((a, b) => b.at - a.at || b.id.localeCompare(a.id))
      .map((item) => {
        const json = JSON.stringify(item);
        const kept = stable.get(item.id);
        if (kept?.json === json) {
          return kept.item;
        }
        stable.set(item.id, { json, item });
        return item;
      });
  });

  const isFresh = (id: string) => Date.now() - (arrivals.get(id) ?? 0) < 8000;

  const more = () => {
    const lists = recent();
    return lists ? shownKinds().filter((kind) => lists[kind].more) : [];
  };

  const showMore = async () => {
    setLoading(true);
    try {
      await Promise.all(more().map((kind) => loadMore(kind)));
    } finally {
      setLoading(false);
    }
  };

  const { resending, failed, resend } = createResend(async (item) => {
    await resendRecent(props.session, item.id, props.page);
    await loadRecent();
  }, guard);

  const toggle = (kind: RecentKind) => {
    const shown = shownKinds();
    const next = shown.includes(kind) ? shown.filter((other) => other !== kind) : [...shown, kind];
    props.onKinds(next.length === recentKinds.length ? undefined : next);
  };

  return (
    <div class="flex flex-col">
      <div class="sticky top-0 flex flex-wrap gap-1.5 border-b border-line bg-card px-3 py-1.5">
        <For each={recentKinds}>
          {(kind) => (
            <button
              type="button"
              aria-pressed={shownKinds().includes(kind)}
              onClick={() => toggle(kind)}
              class="inline-flex h-7 items-center gap-1 rounded-full border border-line px-2.5 text-[13px] font-semibold text-muted transition-colors hover:bg-ghost aria-pressed:border-leaf/50 aria-pressed:bg-ghost aria-pressed:text-ink"
            >
              <Icon name={kindIcons[kind]} class="size-3.5" />
              {kindNames()[kind]}
            </button>
          )}
        </For>
      </div>
      <Show when={items()} fallback={<Spinner />}>
        {(shown) => (
          <Show
            when={shown().length > 0}
            fallback={<p class="px-3 py-4 text-[14px] text-muted">{t("Nothing yet.")}</p>}
          >
            <ul class="flex flex-col divide-y divide-line">
              <For each={shown()}>
                {(item) => (
                  <ActivityRow
                    item={item}
                    fresh={isFresh(item.id)}
                    resending={resending() === item.id}
                    disabled={resending() !== null}
                    failed={failed() === item.id}
                    onResend={() => void resend(item)}
                  />
                )}
              </For>
            </ul>
            <Show when={more().length > 0}>
              <button
                type="button"
                disabled={loading()}
                onClick={() => void showMore()}
                class="w-full py-2 text-[13px] font-semibold text-leaf hover:bg-ghost disabled:opacity-60"
              >
                {loading() ? t("Loading…") : t("Show more")}
              </button>
            </Show>
          </Show>
        )}
      </Show>
    </div>
  );
}
