import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { asset } from "../asset";
import Footer from "../components/Footer";
import Icon from "../components/Icon";
import Nav from "../components/Nav";
import {
  Band,
  buttonClass,
  cardClass,
  ghostButtonClass,
  Hero,
  inputClass,
  Value,
} from "../components/SongRequestParts";
import { pages } from "../data/links";
import {
  BackendError,
  createStreamer,
  keyOf,
  saveStreamerSettings,
  streamerSettings,
  streamerUrl,
  type Settings,
} from "../data/remoteControl";

const keyKey = "songRequestKey";

function readKey() {
  try {
    return localStorage.getItem(keyKey);
  } catch {
    return null;
  }
}

function saveKey(key: string | null) {
  try {
    if (key) {
      localStorage.setItem(keyKey, key);
    } else {
      localStorage.removeItem(keyKey);
    }
  } catch {}
}

function errorText(error: unknown) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 400:
        return "That name can't be used. Use 3 to 25 letters, digits and dashes, not starting or ending with a dash.";
      case 409:
        return "That name is taken. Pick another one.";
    }
  }
  return "Something went wrong. Try again.";
}

const pageUrl = (settings: Settings) =>
  new URL(
    settings.name
      ? `${asset(pages.songRequest)}${settings.name}`
      : `${asset(pages.songRequest)}?streamer=${encodeURIComponent(settings.id)}`,
    location.origin,
  ).href;

function Steps() {
  return (
    <ol class="flex list-decimal flex-col gap-2 pl-6 text-ink-2">
      <li>
        In Moblin, go to Settings, Remote control, Streamer, Assistant and paste the streamer URL as
        URL. Then turn Enabled on.
      </li>
      <li>Make sure Moblin may use Apple Music, and share your page link with your viewers.</li>
    </ol>
  );
}

function SignIn(props: { onKey: (key: string, settings: Settings, isNew: boolean) => void }) {
  const [busy, setBusy] = createSignal(false);
  const [failed, setFailed] = createSignal(false);
  const [url, setUrl] = createSignal("");
  const [signInError, setSignInError] = createSignal<string | null>(null);

  const create = async () => {
    setBusy(true);
    setFailed(false);
    try {
      const streamer = await createStreamer();
      props.onKey(
        streamer.key,
        { id: streamer.id, connected: false, enabled: true, name: null },
        true,
      );
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };

  const signIn = async (event: SubmitEvent) => {
    event.preventDefault();
    const key = keyOf(url());
    if (!key || busy()) {
      return;
    }
    setBusy(true);
    setSignInError(null);
    try {
      props.onKey(key, await streamerSettings(key), false);
    } catch (error) {
      setSignInError(
        error instanceof BackendError && error.status === 404
          ? "That is not a streamer URL from this page."
          : "Something went wrong. Try again.",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div class={cardClass}>
        <h2 class="text-[23px] font-semibold">Create your page</h2>
        <p class="text-ink-2">
          You get a secret streamer URL for Moblin and a page your viewers request songs on.
        </p>
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
      </div>
      <form class={cardClass} onSubmit={signIn}>
        <h2 class="text-[23px] font-semibold">Already have a page?</h2>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">Streamer URL</span>
          <input
            type="password"
            required
            value={url()}
            onInput={(event) => setUrl(event.currentTarget.value)}
            placeholder="wss://api.moblin.app/remote-control/streamer/…"
            autocomplete="off"
            spellcheck={false}
            class={inputClass}
          />
          <span class="text-[15px] text-muted">
            Copy it from Settings, Remote control, Streamer, Assistant in Moblin.
          </span>
        </label>
        <div class="flex flex-wrap items-center gap-4">
          <button type="submit" class={ghostButtonClass} disabled={busy()}>
            Manage my page
          </button>
          <Show when={signInError()}>
            {(text) => (
              <p role="status" class="text-amber">
                {text()}
              </p>
            )}
          </Show>
        </div>
      </form>
    </>
  );
}

function Admin(props: {
  streamerKey: string;
  settings: Settings;
  isNew: boolean;
  onForget: () => void;
}) {
  const [settings, setSettings] = createSignal(props.settings);
  const [name, setName] = createSignal(props.settings.name ?? "");
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<{ ok: boolean; text: string } | null>(null);
  const [toggleFailed, setToggleFailed] = createSignal(false);

  onMount(() => {
    const timer = setInterval(async () => {
      try {
        setSettings(await streamerSettings(props.streamerKey));
      } catch {}
    }, 10_000);
    onCleanup(() => clearInterval(timer));
  });

  const save = async (enabled: boolean, name: string | null) => {
    setBusy(true);
    try {
      setSettings(await saveStreamerSettings(props.streamerKey, enabled, name));
    } finally {
      setBusy(false);
    }
  };

  const toggle = async () => {
    if (busy()) {
      return;
    }
    setToggleFailed(false);
    try {
      await save(!settings().enabled, settings().name);
    } catch {
      setToggleFailed(true);
    }
  };

  const saveName = async (event: SubmitEvent) => {
    event.preventDefault();
    if (busy()) {
      return;
    }
    setResult(null);
    try {
      await save(settings().enabled, name().trim() || null);
      setName(settings().name ?? "");
      setResult({ ok: true, text: settings().name ? "Saved." : "Name removed." });
    } catch (error) {
      setResult({ ok: false, text: errorText(error) });
    }
  };

  return (
    <>
      <div class={cardClass}>
        <div class="flex flex-wrap items-center justify-between gap-4">
          <h2 class="text-[23px] font-semibold">Song requests</h2>
          <button
            type="button"
            role="switch"
            aria-checked={settings().enabled}
            aria-label="Song requests"
            disabled={busy()}
            onClick={toggle}
            class={`relative h-8 w-14 shrink-0 rounded-full transition-colors disabled:opacity-60 ${
              settings().enabled ? "bg-moss" : "bg-ghost-line"
            }`}
          >
            <span
              class={`absolute top-1 left-1 size-6 rounded-full bg-white transition-transform ${
                settings().enabled ? "translate-x-6" : ""
              }`}
            />
          </button>
        </div>
        <p class="text-ink-2">
          {settings().enabled
            ? "On. Viewers can add songs to your queue."
            : "Off. Your page tells viewers that song requests are turned off."}
        </p>
        <p class="flex items-center gap-2.5 text-[15px] text-muted">
          <span
            class={`size-2.5 shrink-0 rounded-full ${settings().connected ? "bg-leaf" : "bg-muted"}`}
          />
          {settings().connected ? "Moblin is connected." : "Moblin is not connected right now."}
        </p>
        <Show when={toggleFailed()}>
          <p role="status" class="text-amber">
            Could not save. Try again.
          </p>
        </Show>
      </div>
      <form class={cardClass} onSubmit={saveName}>
        <h2 class="text-[23px] font-semibold">Page name</h2>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">
            A short name for your page link, easy to remember
          </span>
          <input
            type="text"
            minLength={3}
            maxLength={25}
            pattern="[A-Za-z0-9]([A-Za-z0-9\-]*[A-Za-z0-9])?"
            value={name()}
            onInput={(event) => setName(event.currentTarget.value.toLowerCase())}
            placeholder="your-name"
            autocomplete="off"
            spellcheck={false}
            class={`${inputClass} font-mono`}
          />
          <span class="text-[15px] text-muted">
            Letters, digits and dashes. Leave it empty to only use the random link.
          </span>
        </label>
        <div class="flex flex-wrap items-center gap-4">
          <button type="submit" class={buttonClass} disabled={busy()}>
            Save name
          </button>
          <Show when={result()}>
            {(result) => (
              <p role="status" class={result().ok ? "text-leaf" : "text-amber"}>
                {result().text}
              </p>
            )}
          </Show>
        </div>
        <Value
          label="Your page, for viewers"
          text={pageUrl(settings())}
          href={pageUrl(settings())}
        />
      </form>
      <div class={cardClass}>
        <h2 class="text-[23px] font-semibold">Connect Moblin</h2>
        <Steps />
        <Value label="Streamer URL for Moblin" text={streamerUrl(props.streamerKey)} />
        <p class="flex items-start gap-3 rounded-2xl border-[1.5px] border-amber-line bg-amber-bg px-4 py-3 text-amber-ink">
          <Icon name="warning" class="mt-0.5 size-5 shrink-0 text-amber" />
          {props.isNew
            ? "Keep the streamer URL to yourself and save it now. Anyone who has it can manage your page, and it is the only way back here from another browser."
            : "Keep the streamer URL to yourself. Anyone who has it can manage your page."}
        </p>
        <div>
          <button type="button" class={ghostButtonClass} onClick={props.onForget}>
            Forget on this browser
          </button>
        </div>
      </div>
    </>
  );
}

export default function SongRequestAdmin() {
  const [session, setSession] = createSignal<{
    key: string;
    settings: Settings;
    isNew: boolean;
  } | null>(null);
  const [loading, setLoading] = createSignal(true);

  onMount(async () => {
    const key = readKey();
    if (key) {
      try {
        setSession({ key, settings: await streamerSettings(key), isNew: false });
      } catch (error) {
        if (error instanceof BackendError && error.status === 404) {
          saveKey(null);
        }
      }
    }
    setLoading(false);
  });

  const onKey = (key: string, settings: Settings, isNew: boolean) => {
    saveKey(key);
    setSession({ key, settings, isNew });
  };

  const forget = () => {
    saveKey(null);
    setSession(null);
  };

  return (
    <>
      <Nav />
      <main>
        <Hero eyebrow="Song requests admin" title="Your song request page.">
          <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
            Turn song requests on and off, and pick a name for your page so viewers can find it
            easily.
          </p>
        </Hero>
        <Band>
          <Show when={!loading()} fallback={<p class="text-muted">Loading…</p>}>
            <Show when={session()} fallback={<SignIn onKey={onKey} />}>
              {(session) => (
                <Admin
                  streamerKey={session().key}
                  settings={session().settings}
                  isNew={session().isNew}
                  onForget={forget}
                />
              )}
            </Show>
          </Show>
        </Band>
      </main>
      <Footer />
    </>
  );
}
