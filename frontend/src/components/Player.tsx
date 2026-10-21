import { createSignal, For, type JSX, Match, Show, Switch } from "solid-js";
import Icon from "./Icon";
import { Notice } from "./PageParts";
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

export const playerCardClass =
  "flex flex-col gap-5 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8";

function SongText(props: { song: Song; played?: boolean; large?: boolean }) {
  const song = () => props.song;
  return (
    <span class="flex min-w-0 flex-col">
      <span
        class={`truncate font-semibold ${props.large ? "text-lg" : ""} ${
          props.played ? "text-ink-2" : "text-ink"
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
  );
}

function Duration(props: { song: Song; class?: string }) {
  return (
    <Show when={props.song.duration}>
      {(duration) => (
        <span class={`shrink-0 font-mono text-sm text-muted tabular-nums ${props.class ?? ""}`}>
          {formatDuration(duration())}
        </span>
      )}
    </Show>
  );
}

function RemoveButton(props: { song: Song; moderate: Moderate; busy?: boolean }) {
  return (
    <button
      type="button"
      class={dangerIconButtonClass}
      disabled={props.busy}
      aria-label={t("Remove {song}", { song: props.song.title })}
      title={t("Remove")}
      onClick={() => props.moderate("remove", props.song.id)}
    >
      <Icon name="trash" class="size-5" />
    </button>
  );
}

function Equalizer() {
  return (
    <span class="flex h-5 items-end gap-[3px]" aria-hidden="true">
      <For each={[0, 0.3, 0.15]}>
        {(delay) => (
          <span
            class="equalizer-bar block w-1 rounded-full bg-current"
            style={{ "animation-delay": `${delay}s` }}
          />
        )}
      </For>
    </span>
  );
}

function NowPlaying(props: {
  status: MusicStatus;
  song: Song | null;
  moderate?: Moderate;
  busy?: boolean;
  error: JSX.Element;
}) {
  const playing = () => props.song !== null && props.status.playing;
  return (
    <div class="flex flex-col gap-3 rounded-2xl bg-well p-4">
      <div class="flex items-center gap-4">
        <span
          class={`flex size-12 shrink-0 items-center justify-center rounded-full ${
            props.song ? "bg-moss text-white" : "bg-ghost text-muted"
          }`}
        >
          <Show when={props.song} fallback={<Icon name="music" class="size-6" />}>
            <Show when={playing()} fallback={<Icon name="pause" class="size-6" />}>
              <Equalizer />
            </Show>
          </Show>
        </span>
        <Show
          when={props.song}
          fallback={<span class="text-lg font-semibold text-ink-2">{t("Nothing is playing")}</span>}
        >
          {(song) => (
            <span class="flex min-w-0 flex-col">
              <span class="text-xs font-semibold tracking-wide text-leaf uppercase">
                {playing() ? t("Now playing") : t("Paused")}
              </span>
              <SongText song={song()} large />
            </span>
          )}
        </Show>
        <Show when={props.song}>{(song) => <Duration song={song()} class="ml-auto" />}</Show>
        <Show when={props.moderate}>
          {(moderate) => (
            <span class={`flex shrink-0 gap-1 ${props.song?.duration ? "" : "ml-auto"}`}>
              <button
                type="button"
                class={iconButtonClass}
                disabled={props.busy || props.status.songs.length === 0}
                aria-label={playing() ? t("Pause") : t("Play")}
                title={playing() ? t("Pause") : t("Play")}
                onClick={() => moderate()(playing() ? "pause" : "play")}
              >
                <Icon name={playing() ? "pause" : "play"} class="size-5" />
              </button>
              <Show when={props.song}>
                {(song) => <RemoveButton song={song()} moderate={moderate()} busy={props.busy} />}
              </Show>
            </span>
          )}
        </Show>
      </div>
      {props.error}
    </div>
  );
}

function SongRow(props: {
  song: Song;
  marker: JSX.Element;
  played?: boolean;
  moderate?: Moderate;
  busy?: boolean;
  error: JSX.Element;
}) {
  const song = () => props.song;
  return (
    <li class="flex flex-col gap-3 border-t border-line py-3 first:border-t-0">
      <div class="flex items-center gap-4">
        <span class="w-7 shrink-0 text-right font-mono text-sm text-muted tabular-nums">
          {props.marker}
        </span>
        <SongText song={song()} played={props.played} />
        <Duration song={song()} class="ml-auto" />
        <Show when={props.moderate}>
          {(moderate) => (
            <span class={`flex shrink-0 gap-1 ${song().duration ? "" : "ml-auto"}`}>
              <button
                type="button"
                class={iconButtonClass}
                disabled={props.busy}
                aria-label={t("Play {song} now", { song: song().title })}
                title={t("Play now")}
                onClick={() => moderate()("select", song().id)}
              >
                <Icon name="play" class="size-5" />
              </button>
              <RemoveButton song={song()} moderate={moderate()} busy={props.busy} />
            </span>
          )}
        </Show>
      </div>
      {props.error}
    </li>
  );
}

type View = "next" | "played";

function ViewButton(props: {
  view: View;
  shown: View;
  count: number;
  onShow: (view: View) => void;
  children: JSX.Element;
}) {
  const selected = () => props.view === props.shown;
  return (
    <button
      type="button"
      role="tab"
      aria-selected={selected()}
      class={`flex h-9 items-center gap-2 rounded-full px-4 text-[15px] font-semibold whitespace-nowrap transition-colors ${
        selected() ? "bg-moss text-white" : "text-muted hover:text-ink"
      }`}
      onClick={() => props.onShow(props.view)}
    >
      {props.children}
      <span class={`tabular-nums ${selected() ? "text-white/75" : ""}`}>{props.count}</span>
    </button>
  );
}

export default function Player(props: {
  status: Status;
  moderate?: Moderate;
  busy?: boolean;
  error?: string | null;
}) {
  const music = () => (typeof props.status === "object" ? props.status : null);
  const currentIndex = () => music()?.currentSongIndex ?? null;
  const current = () => {
    const index = currentIndex();
    return index === null ? null : (music()?.songs[index] ?? null);
  };
  const next = () => music()?.songs.slice(currentIndex() === null ? 0 : currentIndex()! + 1) ?? [];
  const played = () => (music()?.songs.slice(0, currentIndex() ?? 0) ?? []).reverse();
  const [shown, setShown] = createSignal<View>("next");
  const [target, setTarget] = createSignal<string | null>(null);
  const moderate = props.moderate
    ? (command: MusicCommand, song?: string) => {
        setTarget(song ?? (command === "clear" ? "clear" : (current()?.id ?? "controls")));
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
    <div class={playerCardClass}>
      <Switch>
        <Match when={props.status === "loading"}>
          <p class="text-muted">{t("Loading…")}</p>
        </Match>
        <Match when={music()}>
          {(status) => (
            <>
              <NowPlaying
                status={status()}
                song={current()}
                moderate={moderate}
                busy={props.busy}
                error={errorFor(current()?.id ?? "controls")}
              />
              <div class="flex flex-wrap items-center justify-between gap-3">
                <div
                  role="tablist"
                  class="flex gap-1 rounded-full border-[1.5px] border-line bg-well p-1"
                >
                  <ViewButton view="next" shown={shown()} count={next().length} onShow={setShown}>
                    {t("Up next")}
                  </ViewButton>
                  <ViewButton
                    view="played"
                    shown={shown()}
                    count={played().length}
                    onShow={setShown}
                  >
                    {t("History")}
                  </ViewButton>
                </div>
                <Show when={moderate}>
                  <button
                    type="button"
                    class={`${dangerIconButtonClass} w-auto gap-2 px-3 text-[15px] font-semibold`}
                    disabled={props.busy || status().songs.length === 0}
                    onClick={clear}
                  >
                    <Icon name="trash" class="size-5" />
                    {t("Clear")}
                  </button>
                </Show>
              </div>
              {errorFor("clear")}
              <Show
                when={shown() === "next"}
                fallback={
                  <Show
                    when={played().length > 0}
                    fallback={<p class="text-ink-2">{t("No songs have been played yet.")}</p>}
                  >
                    <ol class="flex flex-col">
                      <For each={played()}>
                        {(song, index) => (
                          <SongRow
                            song={song}
                            played
                            marker={-(index() + 1)}
                            moderate={moderate}
                            busy={props.busy}
                            error={errorFor(song.id)}
                          />
                        )}
                      </For>
                    </ol>
                  </Show>
                }
              >
                <Show
                  when={next().length > 0}
                  fallback={
                    <p class="text-ink-2">
                      {current()
                        ? t("No more songs are queued. Add the next one!")
                        : t("The queue is empty. Add the first song!")}
                    </p>
                  }
                >
                  <ol class="flex flex-col">
                    <For each={next()}>
                      {(song, index) => (
                        <SongRow
                          song={song}
                          marker={index() + 1}
                          moderate={moderate}
                          busy={props.busy}
                          error={errorFor(song.id)}
                        />
                      )}
                    </For>
                  </ol>
                </Show>
              </Show>
            </>
          )}
        </Match>
      </Switch>
    </div>
  );
}
