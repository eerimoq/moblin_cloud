import { createSignal, For, type JSX, Match, Show, Switch } from "solid-js";
import Icon from "./Icon";
import { ghostButtonClass, Notice } from "./PageParts";
import {
  formatDuration,
  type MusicCommand,
  type MusicStatus,
  type Song,
} from "../data/remoteControl";
import { t, tRich } from "../i18n";

export type Status = MusicStatus | "loading" | "offline" | "unknown";

export type Moderate = (command: MusicCommand, song?: string) => void;

const iconButtonClass =
  "flex size-10 shrink-0 items-center justify-center rounded-xl text-muted transition-colors hover:bg-ghost hover:text-ink disabled:opacity-50";

const dangerIconButtonClass =
  "flex size-10 shrink-0 items-center justify-center rounded-xl text-danger transition-colors hover:bg-danger-bg disabled:opacity-50";

const cardClass = "flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8";

function SongRow(props: {
  song: Song;
  marker: JSX.Element;
  current?: boolean;
  played?: boolean;
  moderate?: Moderate;
  busy?: boolean;
  error: JSX.Element;
}) {
  const song = () => props.song;
  return (
    <li
      class={`flex flex-col gap-3 border-t border-line py-3 first:border-t-0 ${
        props.current ? "text-leaf" : ""
      }`}
    >
      <div class="flex items-center gap-4">
        <span class="w-6 shrink-0 text-right font-mono text-sm text-muted">{props.marker}</span>
        <span class="flex min-w-0 flex-col">
          <span
            class={`truncate font-semibold ${
              props.current ? "text-leaf" : props.played ? "text-ink-2" : "text-ink"
            }`}
          >
            {song().title}
          </span>
          <Show when={song().artist}>
            <span class="truncate text-[15px] text-muted">{song().artist}</span>
          </Show>
          <Show when={song().user}>
            <span class="truncate text-[15px] text-muted">
              {tRich("added by {name}", {
                name: <span class="font-medium text-ink">{song().user}</span>,
              })}
            </span>
          </Show>
        </span>
        <Show when={song().duration}>
          {(duration) => (
            <span class="ml-auto shrink-0 font-mono text-sm text-muted tabular-nums">
              {formatDuration(duration())}
            </span>
          )}
        </Show>
        <Show when={props.moderate}>
          {(moderate) => (
            <span class={`flex shrink-0 gap-1 ${song().duration ? "" : "ml-auto"}`}>
              <button
                type="button"
                class={iconButtonClass}
                disabled={props.busy || props.current}
                aria-label={t("Play {song} now", { song: song().title })}
                title={t("Play now")}
                onClick={() => moderate()("select", song().id)}
              >
                <Icon name="play" class="size-5" />
              </button>
              <button
                type="button"
                class={dangerIconButtonClass}
                disabled={props.busy}
                aria-label={t("Remove {song}", { song: song().title })}
                title={t("Remove")}
                onClick={() => moderate()("remove", song().id)}
              >
                <Icon name="trash" class="size-5" />
              </button>
            </span>
          )}
        </Show>
      </div>
      {props.error}
    </li>
  );
}

export default function Queue(props: {
  status: Status;
  moderate?: Moderate;
  busy?: boolean;
  error?: string | null;
}) {
  const music = () => (typeof props.status === "object" ? props.status : null);
  const firstQueued = () => music()?.currentSongIndex ?? 0;
  const queued = () => music()?.songs.slice(firstQueued()) ?? [];
  const played = () => (music()?.songs.slice(0, firstQueued()) ?? []).reverse();
  const [target, setTarget] = createSignal<string | null>(null);
  const moderate = props.moderate
    ? (command: MusicCommand, song?: string) => {
        setTarget(song ?? "controls");
        props.moderate?.(command, song);
      }
    : undefined;
  const clear = () => {
    if (confirm(t("Remove every song from the queue?"))) {
      moderate?.("clear");
    }
  };
  const errorFor = (at: string) => (
    <Show when={props.error && target() === at}>{(error) => <Notice error>{error()}</Notice>}</Show>
  );

  return (
    <>
      <div class={cardClass}>
        <div class="flex flex-wrap items-center justify-between gap-3">
          <h2 class="text-[23px] font-semibold">{t("Queue")}</h2>
          <Show when={props.moderate && music()}>
            {(status) => (
              <div class="flex flex-wrap gap-2">
                <button
                  type="button"
                  class={ghostButtonClass}
                  disabled={props.busy}
                  onClick={() => moderate?.(status().playing ? "pause" : "play")}
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
          <Match when={music()}>
            {(status) => (
              <Show
                when={queued().length > 0}
                fallback={<p class="text-ink-2">{t("The queue is empty. Add the first song!")}</p>}
              >
                <ol class="flex flex-col">
                  <For each={queued()}>
                    {(song, index) => {
                      const current = () => index() === 0 && status().currentSongIndex !== null;
                      return (
                        <SongRow
                          song={song}
                          current={current()}
                          marker={
                            <Show when={current()} fallback={index() + 1}>
                              <Icon
                                name={status().playing ? "music" : "pause"}
                                class="size-5 text-leaf"
                              />
                            </Show>
                          }
                          moderate={moderate}
                          busy={props.busy}
                          error={errorFor(song.id)}
                        />
                      );
                    }}
                  </For>
                </ol>
              </Show>
            )}
          </Match>
        </Switch>
      </div>
      <Show when={played().length > 0}>
        <div class={cardClass}>
          <h2 class="text-[23px] font-semibold">{t("Played")}</h2>
          <ol class="flex flex-col">
            <For each={played()}>
              {(song) => (
                <SongRow
                  song={song}
                  played
                  marker={<Icon name="check" class="size-5" />}
                  moderate={moderate}
                  busy={props.busy}
                  error={errorFor(song.id)}
                />
              )}
            </For>
          </ol>
        </div>
      </Show>
    </>
  );
}
