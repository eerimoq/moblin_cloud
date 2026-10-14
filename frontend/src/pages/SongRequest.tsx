import { createSignal, For, Match, onCleanup, onMount, Show, Switch, type JSX } from "solid-js";
import { asset } from "../asset";
import CopyButton from "../components/CopyButton";
import Footer from "../components/Footer";
import Icon from "../components/Icon";
import Mascot from "../components/Mascot";
import Nav from "../components/Nav";
import { pages } from "../data/links";
import {
  addSong,
  BackendError,
  createStreamer,
  musicStatus,
  streamerUrl,
  type MusicStatus,
  type Song,
  type Streamer,
} from "../data/remoteControl";

const nameKey = "songRequestName";

function readName() {
  try {
    return localStorage.getItem(nameKey) ?? "";
  } catch {
    return "";
  }
}

function saveName(name: string) {
  try {
    localStorage.setItem(nameKey, name);
  } catch {}
}

function errorText(error: unknown) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 400:
        return "Enter an Apple Music link, a song ID or a search of at most 200 characters.";
      case 404:
        return "The streamer is offline right now. Try again when they are live.";
      case 429:
        return "A song was just added. Try again in a few seconds.";
      case 504:
        return "The streamer did not answer. Try again in a moment.";
    }
  }
  return "Something went wrong. Try again.";
}

function songText(song: Song) {
  return song.artist ? `${song.artist} – ${song.title}` : song.title;
}

const inputClass =
  "h-13 w-full rounded-2xl border-[1.5px] border-line bg-well px-4 text-[17px] text-ink placeholder:text-muted focus:border-leaf focus:outline-none";

const buttonClass =
  "inline-flex h-13 items-center justify-center gap-2.5 rounded-full bg-moss px-6 font-display text-lg font-semibold whitespace-nowrap text-white transition-colors hover:bg-moss-bright disabled:opacity-60";

function Hero(props: { title: string; children: JSX.Element }) {
  return (
    <section class="mx-auto flex max-w-[1120px] flex-col gap-6 px-5 pt-12 pb-10 sm:px-8 lg:pt-18">
      <div class="flex items-start justify-between gap-6">
        <div class="flex max-w-[640px] flex-col gap-4">
          <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
            Song requests
          </span>
          <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
            {props.title}
          </h1>
          {props.children}
        </div>
        <Mascot
          variant="looking"
          class="hidden w-[150px] shrink-0 -rotate-[6deg] sm:block lg:w-[190px]"
        />
      </div>
    </section>
  );
}

function Queue(props: { status: MusicStatus | "offline" | "loading" }) {
  return (
    <div class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8">
      <h2 class="text-[23px] font-semibold">Queue</h2>
      <Switch>
        <Match when={props.status === "loading"}>
          <p class="text-muted">Loading…</p>
        </Match>
        <Match when={props.status === "offline"}>
          <p class="text-ink-2">
            The streamer is offline. The queue shows up here when Moblin is connected.
          </p>
        </Match>
        <Match when={typeof props.status === "object" && props.status}>
          {(status) => (
            <Show
              when={status().songs.length > 0}
              fallback={<p class="text-ink-2">The queue is empty. Add the first song!</p>}
            >
              <ol class="flex flex-col">
                <For each={status().songs}>
                  {(song, index) => {
                    const current = () => index() === status().currentSongIndex;
                    return (
                      <li
                        class={`flex items-center gap-4 border-t border-line py-3 first:border-t-0 ${
                          current() ? "text-leaf" : ""
                        }`}
                      >
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
                          <span class="truncate text-[15px] text-muted">
                            {[song.artist, song.user && `added by ${song.user}`]
                              .filter(Boolean)
                              .join(" · ")}
                          </span>
                        </span>
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

function Viewer(props: { id: string }) {
  const [status, setStatus] = createSignal<MusicStatus | "offline" | "loading">("loading");
  const [song, setSong] = createSignal("");
  const [name, setName] = createSignal(readName());
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<{ ok: boolean; text: string } | null>(null);

  const refresh = async () => {
    try {
      setStatus(await musicStatus(props.id));
    } catch {
      setStatus("offline");
    }
  };

  onMount(() => {
    void refresh();
    const timer = setInterval(refresh, 30_000);
    onCleanup(() => clearInterval(timer));
  });

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    const text = song().trim();
    if (!text || busy()) {
      return;
    }
    saveName(name().trim());
    setBusy(true);
    setResult(null);
    try {
      const added = await addSong(props.id, text, name().trim());
      if (added.songNotFound) {
        setResult({ ok: false, text: `Could not find “${text}” on Apple Music.` });
      } else {
        const last = added.songs.at(-1);
        setResult({
          ok: true,
          text: last ? `Added ${songText(last)} to the queue.` : "Added to the queue.",
        });
        setSong("");
      }
      setStatus({ ...added, songNotFound: null });
    } catch (error) {
      setResult({ ok: false, text: errorText(error) });
      if (error instanceof BackendError && error.status === 404) {
        setStatus("offline");
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Hero title="Pick the next song.">
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          Paste an Apple Music share link or song ID, or just type what you want to hear. It goes
          straight into the streamer's queue in Moblin.
        </p>
      </Hero>
      <section class="bg-band py-16 lg:py-20">
        <div class="mx-auto flex max-w-[1120px] flex-col gap-6 px-5 sm:px-8">
          <form
            class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8"
            onSubmit={submit}
          >
            <h2 class="text-[23px] font-semibold">Add a song</h2>
            <label class="flex flex-col gap-2">
              <span class="text-[15px] font-semibold text-ink-2">Song</span>
              <input
                type="text"
                required
                maxLength={200}
                value={song()}
                onInput={(event) => setSong(event.currentTarget.value)}
                placeholder="https://music.apple.com/… or artist and title"
                autocomplete="off"
                spellcheck={false}
                class={inputClass}
              />
            </label>
            <label class="flex flex-col gap-2">
              <span class="text-[15px] font-semibold text-ink-2">Your name (optional)</span>
              <input
                type="text"
                maxLength={25}
                value={name()}
                onInput={(event) => setName(event.currentTarget.value)}
                placeholder="Shown next to the song"
                autocomplete="nickname"
                class={inputClass}
              />
            </label>
            <div class="flex flex-wrap items-center gap-4">
              <button type="submit" class={buttonClass} disabled={busy()}>
                <Icon name="music" class="size-5" />
                {busy() ? "Adding…" : "Add to queue"}
              </button>
              <Show when={result()}>
                {(result) => (
                  <p role="status" class={result().ok ? "text-leaf" : "text-amber"}>
                    {result().text}
                  </p>
                )}
              </Show>
            </div>
          </form>
          <Queue status={status()} />
        </div>
      </section>
    </>
  );
}

function Value(props: { label: string; text: string; href?: string }) {
  return (
    <div class="flex flex-col gap-2">
      <span class="text-[15px] font-semibold text-ink-2">{props.label}</span>
      <span class="flex items-start gap-2 rounded-2xl border-[1.5px] border-line bg-well px-4 py-3">
        <Show
          when={props.href}
          fallback={
            <code class="min-w-0 flex-1 font-mono text-[15px] break-all">{props.text}</code>
          }
        >
          {(href) => (
            <a
              href={href()}
              class="min-w-0 flex-1 font-mono text-[15px] break-all text-leaf hover:text-leaf-bright"
            >
              {props.text}
            </a>
          )}
        </Show>
        <CopyButton text={props.text} label={props.label} />
      </span>
    </div>
  );
}

function Setup() {
  const [streamer, setStreamer] = createSignal<Streamer | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [failed, setFailed] = createSignal(false);

  const create = async () => {
    setBusy(true);
    setFailed(false);
    try {
      setStreamer(await createStreamer());
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };

  const pageUrl = (id: string) =>
    new URL(`${asset(pages.songRequest)}?streamer=${encodeURIComponent(id)}`, location.origin).href;

  return (
    <>
      <Hero title="Let viewers pick the music.">
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          Get your own song request page. Viewers add songs from Apple Music to the queue of
          Moblin's music player, without typing commands in chat.
        </p>
      </Hero>
      <section class="bg-band py-16 lg:py-20">
        <div class="mx-auto flex max-w-[1120px] flex-col gap-6 px-5 sm:px-8">
          <div class="flex flex-col gap-5 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8">
            <h2 class="text-[23px] font-semibold">Set it up</h2>
            <ol class="flex list-decimal flex-col gap-2 pl-6 text-ink-2">
              <li>Create your page below.</li>
              <li>
                In Moblin, go to Settings, Remote control, Streamer, Assistant and paste the
                streamer URL as URL. Then turn Enabled on.
              </li>
              <li>
                Make sure Moblin may use Apple Music, and share your page link with your viewers.
              </li>
            </ol>
            <Show
              when={streamer()}
              fallback={
                <div class="flex flex-wrap items-center gap-4">
                  <button type="button" class={buttonClass} disabled={busy()} onClick={create}>
                    <Icon name="music" class="size-5" />
                    {busy() ? "Creating…" : "Create my page"}
                  </button>
                  <Show when={failed()}>
                    <p role="status" class="text-amber">
                      Could not create a page. Try again.
                    </p>
                  </Show>
                </div>
              }
            >
              {(streamer) => (
                <div class="flex flex-col gap-5">
                  <Value label="Streamer URL for Moblin" text={streamerUrl(streamer().key)} />
                  <p class="flex items-start gap-3 rounded-2xl border-[1.5px] border-amber-line bg-amber-bg px-4 py-3 text-amber-ink">
                    <Icon name="warning" class="mt-0.5 size-5 shrink-0 text-amber" />
                    Keep the streamer URL to yourself. Anyone who has it can take over your page. It
                    is not shown again, so save it now.
                  </p>
                  <Value
                    label="Your page, for viewers"
                    text={pageUrl(streamer().id)}
                    href={pageUrl(streamer().id)}
                  />
                </div>
              )}
            </Show>
          </div>
        </div>
      </section>
    </>
  );
}

export default function SongRequest() {
  const id = new URLSearchParams(location.search).get("streamer");

  return (
    <>
      <Nav />
      <main>
        <Show when={id} fallback={<Setup />}>
          {(id) => <Viewer id={id()} />}
        </Show>
      </main>
      <Footer />
    </>
  );
}
