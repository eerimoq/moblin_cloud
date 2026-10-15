import {
  loadScript,
  type PayPalButtonsComponent,
  type PayPalButtonsComponentOptions,
} from "@paypal/paypal-js";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  on,
  Match,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";
import Footer from "../components/Footer";
import Icon from "../components/Icon";
import Nav from "../components/Nav";
import { Band, buttonClass, cardClass, Hero, inputClass } from "../components/SongRequestParts";
import { links } from "../data/links";
import {
  BackendError,
  captureSong,
  formatPrice,
  idOfName,
  musicStatus,
  orderSong,
  requestSong,
  streamerPaywall,
  type MusicStatus,
  type PublicPaywall,
} from "../data/remoteControl";
import SongRequestAdmin from "./SongRequestAdmin";

type Status = MusicStatus | "loading" | "offline" | "unknown";

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
  return error instanceof BackendError && error.message
    ? error.message
    : "Something went wrong. Try again.";
}

function PayPalButtons(props: {
  paywall: PublicPaywall & { clientId: string };
  options: PayPalButtonsComponentOptions;
  disabled: boolean;
  round: number;
}) {
  let container: HTMLDivElement | undefined;
  let buttons: PayPalButtonsComponent | undefined;
  const [failed, setFailed] = createSignal(false);
  const [actions, setActions] = createSignal<{
    enable: () => Promise<void>;
    disable: () => Promise<void>;
  }>();

  createEffect(() => {
    void (props.disabled ? actions()?.disable() : actions()?.enable());
  });
  onCleanup(() => void buttons?.close());

  const render = async () => {
    try {
      const paypal = await loadScript({
        clientId: props.paywall.clientId,
        merchantId: props.paywall.merchantId,
        currency: props.paywall.currency,
        intent: "capture",
        components: "buttons",
      });
      const old = buttons;
      if (container && old) {
        container.style.minHeight = `${container.offsetHeight}px`;
        await old.close();
      }
      setActions(undefined);
      buttons = paypal?.Buttons?.({
        style: { layout: "vertical", shape: "pill", height: 52, label: "pay" },
        onInit: (_, actions) => setActions(actions),
        ...props.options,
      });
      if (!buttons) {
        throw new Error("no PayPal buttons");
      }
      if (container) {
        await buttons.render(container);
        container.style.minHeight = "";
      }
    } catch {
      setFailed(true);
    }
  };

  createEffect(on(() => props.round, render));

  return (
    <>
      <div class="flex w-full max-w-sm flex-col gap-3">
        <p class="flex items-baseline justify-between gap-4 border-b border-line pb-3">
          <span class="text-ink-2">Price per song</span>
          <span class="font-display text-2xl font-semibold">{formatPrice(props.paywall)}</span>
        </p>
        <div
          ref={(element) => (container = element)}
          class={`transition-opacity [color-scheme:light] ${
            props.disabled ? "pointer-events-none opacity-40" : ""
          }`}
        />
        <p class="text-center text-[15px] text-muted">
          Your payment goes straight to the streamer.
        </p>
      </div>
      <Show when={failed()}>
        <p role="status" class="text-amber">
          PayPal could not be loaded. Reload the page to try again.
        </p>
      </Show>
    </>
  );
}

function Queue(props: { status: Status }) {
  return (
    <div class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8">
      <h2 class="text-[23px] font-semibold">Queue</h2>
      <Switch>
        <Match when={props.status === "loading"}>
          <p class="text-muted">Loading…</p>
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

function Viewer(props: { name: string }) {
  const [id, setId] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<Status>("loading");
  const [song, setSong] = createSignal("");
  const [name, setName] = createSignal(readName());
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<{ ok: boolean; text: string } | null>(null);
  const [paywall, setPaywall] = createSignal<PublicPaywall | null>(null);
  let form: HTMLFormElement | undefined;
  const [round, setRound] = createSignal(0);
  const nextRound = () => setRound(round() + 1);
  const payable = createMemo(
    () => {
      const current = paywall();
      return current?.clientId && typeof status() === "object"
        ? { ...current, clientId: current.clientId }
        : null;
    },
    null,
    {
      equals: (a, b) =>
        a?.clientId === b?.clientId &&
        a?.merchantId === b?.merchantId &&
        a?.currency === b?.currency &&
        a?.price === b?.price,
    },
  );
  let orderError: unknown = null;

  const refreshPaywall = async () => {
    const current = id();
    if (current) {
      try {
        setPaywall(await streamerPaywall(current));
      } catch {}
    }
  };

  const requested = (paid: boolean) => {
    const text = "Your song will be added to the queue in a moment.";
    setResult({ ok: true, text: paid ? `Thanks! ${text}` : text });
    setSong("");
    setTimeout(refresh, 3_000);
  };

  const start = () => {
    setResult(null);
    if (busy() || !id() || !song().trim() || !form?.reportValidity()) {
      return false;
    }
    saveName(name().trim());
    return true;
  };

  const showError = (error: unknown) => {
    setResult({ ok: false, text: errorText(error) });
    if (error instanceof BackendError && error.status === 404) {
      setStatus("offline");
    }
    if (error instanceof BackendError && [400, 402].includes(error.status)) {
      void refreshPaywall();
    }
  };

  const payPalOptions: PayPalButtonsComponentOptions = {
    onClick: (_, actions) => (start() ? actions.resolve() : actions.reject()),
    createOrder: async () => {
      orderError = null;
      try {
        return await orderSong(id() ?? "", song().trim(), name().trim());
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    onApprove: async (data) => {
      setBusy(true);
      setResult({ ok: true, text: "Payment approved. Completing it…" });
      try {
        await captureSong(id() ?? "", data.orderID);
        requested(true);
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
        nextRound();
      }
    },
    onCancel: () => {
      setResult({ ok: false, text: "Payment cancelled. No song was added." });
      nextRound();
    },
    onError: () => {
      nextRound();
      if (orderError) {
        showError(orderError);
      } else {
        setResult({ ok: false, text: "PayPal could not complete the payment. Try again." });
      }
    },
  };

  const refresh = async () => {
    const current = id();
    if (!current) {
      return;
    }
    try {
      setStatus(await musicStatus(current));
    } catch {
      setStatus("offline");
    }
  };

  onMount(async () => {
    try {
      setId(await idOfName(props.name));
    } catch {
      setStatus("unknown");
    }
    void refreshPaywall();
    void refresh();
    const timer = setInterval(refresh, 30_000);
    onCleanup(() => clearInterval(timer));
  });

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (paywall() || !start()) {
      return;
    }
    setBusy(true);
    try {
      await requestSong(id() ?? "", song().trim(), name().trim());
      requested(false);
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Hero title="Pick the next song.">
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          Paste the Apple Music share link of the song you want to hear. It goes straight into the
          streamer's queue in Moblin.
        </p>
      </Hero>
      <Band>
        <form ref={(element) => (form = element)} class={cardClass} onSubmit={submit}>
          <h2 class="text-[23px] font-semibold">Add a song</h2>
          <label class="flex flex-col gap-2">
            <span class="text-[15px] font-semibold text-ink-2">Apple Music song link</span>
            <input
              type="url"
              required
              maxLength={500}
              value={song()}
              onInput={(event) => setSong(event.currentTarget.value)}
              placeholder="https://music.apple.com/…"
              autocomplete="off"
              spellcheck={false}
              class={inputClass}
            />
            <span class="text-[15px] text-muted">
              Find the song on{" "}
              <a
                href={links.appleMusic}
                target="_blank"
                rel="noopener"
                class="text-leaf hover:text-leaf-bright"
              >
                Apple Music
              </a>
              , then use Share → Copy Link.
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
            <Show
              when={payable()}
              keyed
              fallback={
                <button
                  type="submit"
                  class={buttonClass}
                  disabled={busy() || typeof status() === "string" || paywall() !== null}
                >
                  <Icon name="music" class="size-5" />
                  {busy() ? "Adding…" : "Add to queue"}
                </button>
              }
            >
              {(current) => (
                <PayPalButtons
                  paywall={current}
                  options={payPalOptions}
                  disabled={busy() || !song().trim()}
                  round={round()}
                />
              )}
            </Show>
          </div>
          <Show when={result()}>
            {(result) => (
              <p
                role="status"
                class={`flex items-start gap-3 rounded-2xl border-[1.5px] px-4 py-3 text-[17px] font-semibold ${
                  result().ok
                    ? "border-moss bg-well text-leaf"
                    : "border-amber-line bg-amber-bg text-amber-ink"
                }`}
              >
                <Icon
                  name={result().ok ? "check" : "warning"}
                  class={`mt-0.5 size-5 shrink-0 ${result().ok ? "text-leaf" : "text-amber"}`}
                />
                {result().text}
              </p>
            )}
          </Show>
        </form>
        <Queue status={status()} />
      </Band>
    </>
  );
}

export default function SongRequest(props: { name?: string }) {
  return (
    <>
      <Nav />
      <main>
        <Show when={props.name} fallback={<SongRequestAdmin />}>
          {(name) => <Viewer name={name()} />}
        </Show>
      </main>
      <Footer />
    </>
  );
}
