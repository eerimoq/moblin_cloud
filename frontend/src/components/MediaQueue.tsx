import { createEffect, createSignal, For, on, onMount, Show, type JSX } from "solid-js";
import Icon from "./Icon";
import { timeAgo, VideoLink } from "./Latest";
import { ask } from "./Modal";
import { buttonClass, cardClass, ghostButtonClass, Notice, Spinner } from "./PageParts";
import {
  controlDevice,
  decideMedia,
  fetchMediaQueue,
  pauseMedia,
  type Control,
  type MediaQueue as Queue,
  type QueuedMedia,
} from "../data/account";
import { BackendError, isSignedOut } from "../data/backend";
import { formatPrice } from "../data/remoteControl";
import { timeText, youTubeThumbnail } from "../data/youtube";
import { t } from "../i18n";
import { sameData } from "../same";

const smallButtonClass = "h-9! px-4! text-[15px]!";

const fromText = (video: QueuedMedia) =>
  video.kickSubscriber
    ? t("Kick subscriber")
    : video.tier
      ? t("Tier {tier} subscriber", { tier: video.tier })
      : video.amount !== undefined && video.currency
        ? formatPrice({ price: video.amount, currency: video.currency })
        : null;

function Video(props: { video: QueuedMedia; children?: JSX.Element }) {
  const video = () => props.video;
  return (
    <li class="flex flex-wrap items-start gap-x-4 gap-y-2 py-3 first:pt-0 last:pb-0">
      <Show
        when={
          video().thumbnail ??
          (video().platform === "youtube" ? youTubeThumbnail(video().video) : undefined)
        }
        fallback={
          <span class="grid aspect-video w-28 shrink-0 place-items-center rounded-lg border border-line bg-well text-muted">
            <Icon name="play-rectangle" class="size-6" />
          </span>
        }
      >
        {(thumbnail) => (
          <img
            src={thumbnail()}
            alt=""
            loading="lazy"
            referrerpolicy="no-referrer"
            class="aspect-video w-28 shrink-0 rounded-lg border border-line bg-well object-cover"
          />
        )}
      </Show>
      <div class="flex min-w-0 flex-1 basis-48 flex-col gap-0.5">
        <VideoLink
          video={video().video}
          start={video().start}
          link={video().link}
          title={video().title}
          class="font-semibold wrap-anywhere text-ink"
        />
        <span class="text-[15px] text-ink-2">
          {video().platform === "youtube" ? "YouTube" : t("Twitch clip")}
          {" · "}
          {timeText(video().start)}–{timeText(video().start + video().seconds)}
        </span>
        <span class="flex flex-wrap items-baseline gap-x-2 text-[15px]">
          <span class="font-semibold">{video().user}</span>
          <Show when={fromText(video())}>{(text) => <span class="text-ink-2">{text()}</span>}</Show>
          <span class="text-muted">{timeAgo(video().at)}</span>
        </span>
        <Show when={video().message}>
          {(message) => <span class="text-ink-2 wrap-anywhere">{message()}</span>}
        </Show>
      </div>
      {props.children}
    </li>
  );
}

export default function MediaQueue(props: {
  session: string;
  page: string;
  changes: number;
  control: boolean;
  onSignOut: () => void;
}) {
  const [queue, setQueue] = createSignal<Queue | null>(null, { equals: sameData });
  const [working, setWorking] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const load = async () => {
    try {
      setQueue(await fetchMediaQueue(props.session, props.page));
    } catch (error) {
      if (isSignedOut(error)) {
        props.onSignOut();
      }
    }
  };

  onMount(() => void load());
  createEffect(
    on(
      () => props.changes,
      () => void load(),
      { defer: true },
    ),
  );

  const run = async (id: string, action: () => Promise<unknown>) => {
    if (working()) {
      return;
    }
    setError(null);
    setWorking(id);
    try {
      await action();
      await load();
    } catch (error) {
      if (isSignedOut(error)) {
        props.onSignOut();
      }
      setError(
        error instanceof BackendError &&
          error.message &&
          (error.status < 500 || error.status === 502)
          ? error.message
          : t("Something went wrong. Try again."),
      );
    } finally {
      setWorking(null);
    }
  };

  const approve = (video: QueuedMedia) =>
    run(video.id, () => decideMedia(props.session, video.id, true, props.page));

  const reject = async (video: QueuedMedia) => {
    const paid = video.amount !== undefined;
    if (
      !(await ask(
        paid
          ? t("The video is not played and the tip from {name} is refunded.", { name: video.user })
          : t("The video from {name} is not played.", { name: video.user }),
        paid ? t("Reject and refund") : t("Reject"),
      ))
    ) {
      return;
    }
    await run(video.id, () => decideMedia(props.session, video.id, false, props.page));
  };

  const control = (name: string, control: Control) => {
    const device = queue()?.device;
    if (device) {
      void run(name, () => controlDevice(props.session, props.page, device, control));
    }
  };

  return (
    <section class={cardClass}>
      <div class="flex flex-col gap-1">
        <h2 class="text-[23px] font-semibold">{t("Media share queue")}</h2>
        <p class="text-[15px] text-muted">
          {t("Approved videos play one at a time in the browser widget picked in Moblin.")}
        </p>
      </div>
      <Show when={queue()} fallback={<Spinner />}>
        {(queue) => (
          <>
            <Show when={queue().device === null}>
              <Notice>
                {t(
                  "No Moblin is taking videos. Pick a browser widget under Media share in Moblin's " +
                    "Moblin Cloud settings and add it to the scenes you stream with.",
                )}
              </Notice>
            </Show>
            <Show when={error()}>
              {(error) => (
                <Notice error onClose={() => setError(null)}>
                  {error()}
                </Notice>
              )}
            </Show>
            <div class="flex flex-col gap-3">
              <h3 class="font-title text-sm font-semibold tracking-wider text-muted uppercase">
                {t("Waiting for approval")}
              </h3>
              <Show
                when={queue().held.length > 0}
                fallback={<p class="text-ink-2">{t("No videos are waiting.")}</p>}
              >
                <ul class="flex flex-col divide-y divide-line">
                  <For each={queue().held}>
                    {(video) => (
                      <Video video={video}>
                        <div class="flex gap-2">
                          <button
                            type="button"
                            class={`${buttonClass} ${smallButtonClass}`}
                            disabled={working() !== null}
                            onClick={() => void approve(video)}
                          >
                            {t("Approve")}
                          </button>
                          <button
                            type="button"
                            class={`${ghostButtonClass} ${smallButtonClass}`}
                            disabled={working() !== null}
                            onClick={() => void reject(video)}
                          >
                            {t("Reject")}
                          </button>
                        </div>
                      </Video>
                    )}
                  </For>
                </ul>
              </Show>
            </div>
            <Show when={queue().device !== null || queue().queued.length > 0}>
              <div class="flex flex-col gap-3 border-t border-line pt-5">
                <div class="flex flex-wrap items-center justify-between gap-3">
                  <h3 class="font-title text-sm font-semibold tracking-wider text-muted uppercase">
                    {queue().paused ? t("In Moblin · paused") : t("In Moblin")}
                  </h3>
                  <Show when={props.control}>
                    <div class="flex gap-2">
                      <button
                        type="button"
                        class={`${ghostButtonClass} ${smallButtonClass}`}
                        disabled={working() !== null}
                        onClick={() =>
                          void run("pause", () =>
                            pauseMedia(props.session, !queue().paused, props.page),
                          )
                        }
                      >
                        <Icon name={queue().paused ? "play" : "pause"} class="size-4" />
                        {queue().paused ? t("Resume") : t("Pause")}
                      </button>
                      <button
                        type="button"
                        class={`${ghostButtonClass} ${smallButtonClass}`}
                        disabled={working() !== null || queue().playing === null}
                        onClick={() => control("skip", { skipMediaShare: {} })}
                      >
                        {t("Skip")}
                      </button>
                    </div>
                  </Show>
                </div>
                <Show
                  when={queue().playing !== null || queue().queued.length > 0}
                  fallback={<p class="text-ink-2">{t("Nothing is playing.")}</p>}
                >
                  <ul class="flex flex-col divide-y divide-line">
                    <Show when={queue().playing}>
                      {(playing) => (
                        <Video video={playing()}>
                          <span class="flex items-center gap-1 text-[15px] font-semibold text-leaf">
                            <Icon name="play" class="size-4" />
                            {t("Playing")}
                          </span>
                        </Video>
                      )}
                    </Show>
                    <For each={queue().queued}>
                      {(video) => (
                        <Video video={video}>
                          <span class="text-[15px] text-muted">{t("Up next")}</span>
                        </Video>
                      )}
                    </For>
                  </ul>
                </Show>
                <Show when={queue().paused}>
                  <p class="text-[15px] text-muted">
                    {t("While paused, the video that plays finishes and no new one starts.")}
                  </p>
                </Show>
              </div>
            </Show>
          </>
        )}
      </Show>
    </section>
  );
}
