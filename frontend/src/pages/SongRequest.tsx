import { createSignal, For, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import { asset } from "../asset";
import Footer from "../components/Footer";
import Icon from "../components/Icon";
import Nav from "../components/Nav";
import { Band, buttonClass, cardClass, Hero, inputClass } from "../components/SongRequestParts";
import { links, pages } from "../data/links";
import {
  addSong,
  BackendError,
  idOfName,
  musicStatus,
  type MusicStatus,
  type Song,
} from "../data/remoteControl";

type Status = MusicStatus | "loading" | "offline" | "off" | "unknown";

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
      case 403:
        return "The streamer turned song requests off.";
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

function Queue(props: { status: Status }) {
  return (
    <div class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8">
      <h2 class="text-[23px] font-semibold">Queue</h2>
      <Switch>
        <Match when={props.status === "loading"}>
          <p class="text-muted">Loading…</p>
        </Match>
        <Match when={props.status === "off"}>
          <p class="text-ink-2">The streamer turned song requests off. Check back later.</p>
        </Match>
        <Match when={props.status === "unknown"}>
          <p class="text-ink-2">There is no song request page with this name.</p>
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

function statusOfError(error: unknown): Status {
  return error instanceof BackendError && error.status === 403 ? "off" : "offline";
}

function Viewer(props: { id: string | null; name: string | null }) {
  const [id, setId] = createSignal(props.id);
  const [status, setStatus] = createSignal<Status>("loading");
  const [song, setSong] = createSignal("");
  const [name, setName] = createSignal(readName());
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<{ ok: boolean; text: string } | null>(null);

  const refresh = async () => {
    const current = id();
    if (!current) {
      return;
    }
    try {
      setStatus(await musicStatus(current));
    } catch (error) {
      setStatus(statusOfError(error));
    }
  };

  onMount(async () => {
    if (props.name) {
      try {
        setId(await idOfName(props.name));
      } catch {
        setStatus("unknown");
      }
    }
    void refresh();
    const timer = setInterval(refresh, 30_000);
    onCleanup(() => clearInterval(timer));
  });

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    const text = song().trim();
    const current = id();
    if (!text || !current || busy()) {
      return;
    }
    saveName(name().trim());
    setBusy(true);
    setResult(null);
    try {
      const added = await addSong(current, text, name().trim());
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
      if (error instanceof BackendError && [403, 404].includes(error.status)) {
        setStatus(statusOfError(error));
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
      <Band>
        <form class={cardClass} onSubmit={submit}>
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
            <span class="text-[15px] text-muted">
              Search for songs on{" "}
              <a
                href={links.appleMusic}
                target="_blank"
                rel="noopener"
                class="text-leaf hover:text-leaf-bright"
              >
                Apple Music
              </a>{" "}
              and copy the share link of the one you want.
            </span>
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
            <button
              type="submit"
              class={buttonClass}
              disabled={busy() || typeof status() === "string"}
            >
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
      </Band>
    </>
  );
}

function Landing() {
  return (
    <>
      <Hero title="Let viewers pick the music.">
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          Get your own song request page. Viewers add songs from Apple Music to the queue of
          Moblin's music player, without typing commands in chat.
        </p>
      </Hero>
      <Band>
        <div class={cardClass}>
          <h2 class="text-[23px] font-semibold">Are you a streamer?</h2>
          <p class="text-ink-2">
            Create your page, pick a name that is easy to remember and turn song requests on and off
            on the admin page.
          </p>
          <div>
            <a href={asset(pages.songRequestAdmin)} class={buttonClass}>
              <Icon name="music" class="size-5" />
              Set up my page
            </a>
          </div>
        </div>
      </Band>
    </>
  );
}

export default function SongRequest(props: { name?: string }) {
  const id = new URLSearchParams(location.search).get("streamer");
  const name = props.name ?? null;

  return (
    <>
      <Nav />
      <main>
        <Show when={id || name} fallback={<Landing />}>
          <Viewer id={id} name={name} />
        </Show>
      </main>
      <Footer />
    </>
  );
}
