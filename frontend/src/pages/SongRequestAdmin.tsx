import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { asset } from "../asset";
import Icon from "../components/Icon";
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
  currencies,
  keyOf,
  saveStreamerSettings,
  streamerSettings,
  streamerUrl,
  type Paywall,
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

const pageUrl = (name: string) =>
  new URL(`${asset(pages.songRequests)}${name}`, location.origin).href;

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
      props.onKey(streamer.key, await streamerSettings(streamer.key), true);
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

function paywallErrorText(error: unknown) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 400:
        return "Enter your PayPal email and a price from 0.50 to 1000.";
      case 422:
        return "PayPal can't send payments to that email. Use the email of your PayPal business account.";
      case 502:
        return "PayPal could not be reached. Try again.";
      case 503:
        return "Payments are unavailable right now.";
    }
  }
  return "Something went wrong. Try again.";
}

function Payments(props: {
  paywall: Paywall | null;
  busy: boolean;
  onSave: (changes: { paywall: Paywall | null }) => Promise<void>;
}) {
  const [required, setRequired] = createSignal(props.paywall !== null);
  const [email, setEmail] = createSignal(props.paywall?.email ?? "");
  const [price, setPrice] = createSignal(
    props.paywall ? (props.paywall.price / 100).toFixed(2) : "1.00",
  );
  const [currency, setCurrency] = createSignal(props.paywall?.currency ?? "USD");
  const [result, setResult] = createSignal<{ ok: boolean; text: string } | null>(null);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.busy) {
      return;
    }
    setResult(null);
    try {
      await props.onSave({
        paywall: required()
          ? {
              email: email().trim(),
              price: Math.round(Number.parseFloat(price()) * 100),
              currency: currency(),
            }
          : null,
      });
      setResult({
        ok: true,
        text: required() ? "Saved. Viewers now pay to add songs." : "Song requests are free.",
      });
    } catch (error) {
      setResult({ ok: false, text: paywallErrorText(error) });
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">Paid requests</h2>
      <p class="text-ink-2">
        Let viewers pay with PayPal to add a song. The money goes straight to your PayPal account.
        Moblin takes nothing, PayPal charges its usual fees.
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={required()}
          onChange={(event) => setRequired(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        Viewers pay to add songs
      </label>
      <Show when={required()}>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">Your PayPal email</span>
          <input
            type="email"
            required
            maxLength={254}
            value={email()}
            onInput={(event) => setEmail(event.currentTarget.value)}
            placeholder="you@example.com"
            autocomplete="email"
            spellcheck={false}
            class={inputClass}
          />
        </label>
        <div class="flex flex-wrap gap-4">
          <label class="flex min-w-40 flex-1 flex-col gap-2">
            <span class="text-[15px] font-semibold text-ink-2">Price per song</span>
            <input
              type="number"
              required
              min="0.5"
              max="1000"
              step="0.01"
              inputmode="decimal"
              value={price()}
              onInput={(event) => setPrice(event.currentTarget.value)}
              class={inputClass}
            />
          </label>
          <label class="flex w-32 flex-col gap-2">
            <span class="text-[15px] font-semibold text-ink-2">Currency</span>
            <select
              value={currency()}
              onChange={(event) => setCurrency(event.currentTarget.value)}
              class={inputClass}
            >
              <For each={currencies}>{(code) => <option value={code}>{code}</option>}</For>
            </select>
          </label>
        </div>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <button type="submit" class={buttonClass} disabled={props.busy}>
          Save
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

  onMount(() => {
    const timer = setInterval(async () => {
      try {
        setSettings(await streamerSettings(props.streamerKey));
      } catch {}
    }, 10_000);
    onCleanup(() => clearInterval(timer));
  });

  const save = async (changes: Partial<Pick<Settings, "name" | "paywall">>) => {
    setBusy(true);
    try {
      setSettings(await saveStreamerSettings(props.streamerKey, { ...settings(), ...changes }));
    } finally {
      setBusy(false);
    }
  };

  const saveName = async (event: SubmitEvent) => {
    event.preventDefault();
    if (busy()) {
      return;
    }
    setResult(null);
    try {
      await save({ name: name().trim() });
      setName(settings().name ?? "");
      setResult({ ok: true, text: "Saved." });
    } catch (error) {
      setResult({ ok: false, text: errorText(error) });
    }
  };

  return (
    <>
      <form class={cardClass} onSubmit={saveName}>
        <h2 class="text-[23px] font-semibold">Your song request link</h2>
        <p class="text-ink-2">
          Viewers open this link to request songs. Share it in your chat, bio or stream overlay.
        </p>
        <Show when={settings().name}>
          {(name) => (
            <Value
              label="Link for viewers to request songs"
              text={pageUrl(name())}
              href={pageUrl(name())}
            />
          )}
        </Show>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">Name in the link</span>
          <input
            type="text"
            required
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
            Short and easy to remember, like your channel name. Letters, digits and dashes.
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
      </form>
      <Show when={settings().name && (settings().paypal || settings().paywall)}>
        <Payments paywall={settings().paywall} busy={busy()} onSave={save} />
      </Show>
      <div class={cardClass}>
        <h2 class="text-[23px] font-semibold">Connect Moblin</h2>
        <p class="flex items-center gap-2.5 text-[15px] text-muted">
          <span
            class={`size-2.5 shrink-0 rounded-full ${settings().connected ? "bg-leaf" : "bg-muted"}`}
          />
          {settings().connected ? "Moblin is connected." : "Moblin is not connected right now."}
        </p>
        <Steps />
        <Value label="Streamer URL for Moblin" text={streamerUrl(props.streamerKey)} secret />
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
      <Hero title="Let viewers pick the music.">
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          Get your own song request page. Viewers add songs from Apple Music to the queue of
          Moblin's music player, and you turn requests on and off here.
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
    </>
  );
}
