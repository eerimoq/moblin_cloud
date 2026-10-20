import { createSignal, For, Match, Show, Switch } from "solid-js";
import Icon from "./Icon";
import { ghostButtonClass, Notice } from "./PageParts";
import { formatDuration, type MusicCommand, type MusicStatus } from "../data/remoteControl";
import { t, tRich } from "../i18n";

export type Status = MusicStatus | "loading" | "offline" | "unknown";

export type Moderate = (command: MusicCommand, song?: string) => void;

const iconButtonClass =
  "flex size-10 shrink-0 items-center justify-center rounded-xl text-muted transition-colors hover:bg-ghost hover:text-ink disabled:opacity-50";

const dangerIconButtonClass =
  "flex size-10 shrink-0 items-center justify-center rounded-xl text-danger transition-colors hover:bg-danger-bg disabled:opacity-50";

export default function Queue(props: {
  status: Status;
  moderate?: Moderate;
  busy?: boolean;
  error?: string | null;
}) {
  const music = () => (typeof props.status === "object" ? props.status : null);
  const [target, setTarget] = createSignal<string | null>(null);
  const moderate = (target: string, command: MusicCommand, song?: string) => {
    setTarget(target);
    props.moderate?.(command, song);
  };
  const clear = () => {
    if (confirm(t("Remove every song from the queue?"))) {
      moderate("controls", "clear");
    }
  };
  const errorFor = (at: string) => (
    <Show when={props.error && target() === at}>{(error) => <Notice error>{error()}</Notice>}</Show>
  );

  return (
    <div class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <h2 class="text-[23px] font-semibold">{t("Queue")}</h2>
        <Show when={props.moderate && music()}>
          {(status) => (
            <div class="flex flex-wrap gap-2">
              <button
                type="button"
                class={ghostButtonClass}
                disabled={props.busy}
                onClick={() => moderate("controls", status().playing ? "pause" : "play")}
              >
                <Icon name={status().playing ? "pause" : "play"} class="size-5" />
                {status().playing ? t("Pause") : t("Play")}
              </button>
              <button
                type="button"
                class={`${ghostButtonClass} border-danger-line! bg-danger-bg! text-danger! hover:bg-danger-line!`}
                disabled={props.busy || status().songs.length === 0}
                onClick={clear}
              >
                <Icon name="trash" class="size-5" />
                {t("Clear")}
              </button>
            </div>
          )}
        </Show>
      </div>
      {errorFor("controls")}
      <Switch>
        <Match when={props.status === "loading"}>
          <p class="text-muted">{t("Loading…")}</p>
        </Match>
        <Match when={typeof props.status === "object" && props.status}>
          {(status) => (
            <Show
              when={status().songs.length > 0}
              fallback={<p class="text-ink-2">{t("The queue is empty. Add the first song!")}</p>}
            >
              <ol class="flex flex-col">
                <For each={status().songs}>
                  {(song, index) => {
                    const current = () => index() === status().currentSongIndex;
                    return (
                      <li
                        class={`flex flex-col gap-3 border-t border-line py-3 first:border-t-0 ${
                          current() ? "text-leaf" : ""
                        }`}
                      >
                        <div class="flex items-center gap-4">
                          <span class="w-6 shrink-0 text-right font-mono text-sm text-muted">
                            <Show when={current()} fallback={index() + 1}>
                              <Icon name="music" class="size-5 text-leaf" />
                            </Show>
                          </span>
                          <span class="flex min-w-0 flex-col">
                            <span
                              class={`truncate font-semibold ${current() ? "text-leaf" : "text-ink"}`}
                            >
                              {song.title}
                            </span>
                            <Show when={song.artist}>
                              <span class="truncate text-[15px] text-muted">{song.artist}</span>
                            </Show>
                            <Show when={song.user}>
                              <span class="truncate text-[15px] text-muted">
                                {tRich("added by {name}", {
                                  name: <span class="font-medium text-ink">{song.user}</span>,
                                })}
                              </span>
                            </Show>
                          </span>
                          <Show when={song.duration}>
                            {(duration) => (
                              <span class="ml-auto shrink-0 font-mono text-sm text-muted tabular-nums">
                                {formatDuration(duration())}
                              </span>
                            )}
                          </Show>
                          <Show when={props.moderate}>
                            <span class={`flex shrink-0 gap-1 ${song.duration ? "" : "ml-auto"}`}>
                              <button
                                type="button"
                                class={iconButtonClass}
                                disabled={props.busy || current()}
                                aria-label={t("Play {song} now", { song: song.title })}
                                title={t("Play now")}
                                onClick={() => moderate(song.id, "select", song.id)}
                              >
                                <Icon name="play" class="size-5" />
                              </button>
                              <button
                                type="button"
                                class={dangerIconButtonClass}
                                disabled={props.busy}
                                aria-label={t("Remove {song}", { song: song.title })}
                                title={t("Remove")}
                                onClick={() => moderate(song.id, "remove", song.id)}
                              >
                                <Icon name="trash" class="size-5" />
                              </button>
                            </span>
                          </Show>
                        </div>
                        {errorFor(song.id)}
                      </li>
                    );
                  }}
                </For>
              </ol>
            </Show>
          )}
        </Match>
      </Switch>
    </div>
  );
}
