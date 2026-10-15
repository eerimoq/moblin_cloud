import {
  loadCoreSdkScript,
  type OnApproveDataOneTimePayments,
  type OneTimePaymentSession,
} from "@paypal/paypal-js/sdk-v6";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Match,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";
import { asset } from "../asset";
import Footer from "../components/Footer";
import Icon from "../components/Icon";
import Nav from "../components/Nav";
import { buttonClass, cardClass, inputClass } from "../components/SongRequestParts";
import { links, pages } from "../data/links";
import {
  BackendError,
  captureSong,
  defaultTitle,
  formatDuration,
  formatPrice,
  idOfTwitchLogin,
  musicStatus,
  orderSong,
  requestSong,
  streamerPaywall,
  streamerProfile,
  type MusicStatus,
  type Profile,
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

type PayPalOptions = {
  start: () => boolean;
  createOrder: () => Promise<{ orderId: string }>;
  onApprove: (data: OnApproveDataOneTimePayments) => Promise<void>;
  onCancel: () => void;
  onError: () => void;
};

function PayPalButtons(props: {
  paywall: PublicPaywall & { clientId: string; merchantId: string; environment: string };
  options: PayPalOptions;
  disabled: boolean;
}) {
  let container: HTMLDivElement | undefined;
  let button: HTMLElement | undefined;
  let session: OneTimePaymentSession | undefined;
  const [failed, setFailed] = createSignal(false);

  createEffect(() => {
    const disabled = props.disabled;
    button?.toggleAttribute("disabled", disabled);
  });
  onCleanup(() => session?.destroy());

  const pay = async () => {
    if (!session || props.disabled || !props.options.start()) {
      return;
    }
    try {
      await session.start({ presentationMode: "auto" }, props.options.createOrder());
    } catch {
      props.options.onError();
    }
  };

  onMount(async () => {
    try {
      const paypal = await loadCoreSdkScript({
        environment: props.paywall.environment === "sandbox" ? "sandbox" : "production",
      });
      const instance = await paypal?.createInstance({
        clientId: props.paywall.clientId,
        merchantId: props.paywall.merchantId,
        components: ["paypal-payments"],
        pageType: "checkout",
      });
      if (!instance || !container) {
        throw new Error("no PayPal");
      }
      session = instance.createPayPalOneTimePaymentSession({
        onApprove: props.options.onApprove,
        onCancel: props.options.onCancel,
        onError: props.options.onError,
      });
      button = document.createElement("paypal-button");
      button.setAttribute("type", "pay");
      button.toggleAttribute("disabled", props.disabled);
      button.addEventListener("click", () => void pay());
      container.append(button);
    } catch {
      setFailed(true);
    }
  });

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
        <p class="text-[15px] text-muted">Your payment goes straight to the streamer.</p>
      </div>
      <Show when={failed()}>
        <p role="status" class="text-amber">
          PayPal could not be loaded. Reload the page to try again.
        </p>
      </Show>
    </>
  );
}

function Queue(props: { status: Status; name: string | undefined }) {
  return (
    <div class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8">
      <h2 class="text-[23px] font-semibold">Queue</h2>
      <Switch>
        <Match when={props.status === "loading"}>
          <p class="text-muted">Loading…</p>
        </Match>
        <Match when={props.status === "offline"}>
          <p class="text-ink-2">
            {props.name ?? "The streamer"} is offline. The queue shows up here when they go live
            with Moblin.
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
                          <Show when={song.artist}>
                            <span class="truncate text-[15px] text-muted">{song.artist}</span>
                          </Show>
                          <Show when={song.user}>
                            <span class="truncate text-[15px] text-muted">
                              added by <span class="font-medium text-ink">{song.user}</span>
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

function StreamerHeader(props: { profile: Profile | null; unknown: boolean }) {
  return (
    <header>
      <div class="mx-auto flex max-w-[720px] flex-col items-center gap-4 px-5 pt-14 pb-10 text-center sm:px-8 lg:pt-20">
        <Show
          when={props.profile}
          fallback={
            <>
              <span class="size-28 rounded-full bg-ghost sm:size-32" />
              <h1 class="text-[40px] leading-[1.05] font-bold sm:text-6xl">
                {props.unknown ? "Page not found." : "\u00a0"}
              </h1>
            </>
          }
        >
          {(profile) => (
            <>
              <Show
                when={profile().avatar}
                fallback={
                  <span class="flex size-28 shrink-0 items-center justify-center rounded-full bg-moss font-display text-5xl font-semibold text-white ring-4 ring-moss/40 ring-offset-4 ring-offset-page sm:size-32">
                    {profile().name.slice(0, 1).toUpperCase()}
                  </span>
                }
              >
                {(avatar) => (
                  <img
                    src={avatar()}
                    alt=""
                    class="size-28 shrink-0 rounded-full bg-ghost object-cover ring-4 ring-moss ring-offset-4 ring-offset-page sm:size-32"
                  />
                )}
              </Show>
              <h1 class="max-w-full text-[40px] leading-[1.05] font-bold break-words sm:text-6xl">
                {profile().name}
              </h1>
              <a
                href={`https://www.twitch.tv/${encodeURIComponent(profile().login)}`}
                target="_blank"
                rel="noopener"
                class="inline-flex items-center gap-1.5 text-[17px] font-semibold text-ink-2 transition-colors hover:text-ink"
              >
                <Icon name="twitch" class="size-5 shrink-0 text-twitch" />
                twitch.tv/{profile().login}
              </a>
              <Show when={profile().message}>
                {(message) => (
                  <p class="mt-2 text-lg whitespace-pre-line text-ink-2 text-pretty sm:text-[21px]">
                    {message()}
                  </p>
                )}
              </Show>
            </>
          )}
        </Show>
      </div>
    </header>
  );
}

function PoweredBy() {
  return (
    <footer class="py-10">
      <a
        href={asset(pages.songRequests)}
        class="mx-auto flex w-fit items-center gap-2 text-[15px] font-semibold text-muted transition-colors hover:text-ink"
      >
        <img src={asset("logos/logo-default.png")} alt="" class="h-5 w-auto" />
        Song requests by Moblin
      </a>
    </footer>
  );
}

function Viewer(props: { id: string | null; twitch: string | null }) {
  const [id, setId] = createSignal(props.id);
  const [status, setStatus] = createSignal<Status>("loading");
  const [song, setSong] = createSignal("");
  const [name, setName] = createSignal(readName());
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<{ ok: boolean; text: string } | null>(null);
  const [paywall, setPaywall] = createSignal<PublicPaywall | null>(null);
  const [profile, setProfile] = createSignal<Profile | null>(null);
  const [styled, setStyled] = createSignal(false);
  let form: HTMLFormElement | undefined;
  const payable = createMemo(
    () => {
      const current = paywall();
      return current?.clientId &&
        current.merchantId &&
        current.environment &&
        typeof status() === "object"
        ? {
            ...current,
            clientId: current.clientId,
            merchantId: current.merchantId,
            environment: current.environment,
          }
        : null;
    },
    null,
    {
      equals: (a, b) =>
        a?.clientId === b?.clientId &&
        a?.merchantId === b?.merchantId &&
        a?.environment === b?.environment &&
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

  const payPalOptions: PayPalOptions = {
    start,
    createOrder: async () => {
      orderError = null;
      try {
        return { orderId: await orderSong(id() ?? "", song().trim(), name().trim()) };
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    onApprove: async (data) => {
      setBusy(true);
      setResult({ ok: true, text: "Payment approved. Completing it…" });
      try {
        await captureSong(id() ?? "", data.orderId);
        requested(true);
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
      }
    },
    onCancel: () => {
      setResult({ ok: false, text: "Payment cancelled. No song was added." });
    },
    onError: () => {
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
    if (props.twitch) {
      try {
        setId(await idOfTwitchLogin(props.twitch));
      } catch {
        setStatus("unknown");
      }
    }
    const current = id();
    if (current) {
      streamerProfile(current)
        .then((found) => {
          setProfile(found);
          document.title = `${found.name} song requests`;
        })
        .catch(() => {})
        .finally(() => setStyled(true));
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
    <div data-accent={profile()?.accent ?? undefined} class="isolate flex min-h-svh flex-col">
      <main class="flex-1">
        <StreamerHeader profile={profile()} unknown={status() === "unknown"} />
        <Show
          when={status() !== "unknown"}
          fallback={
            <p class="mx-auto max-w-[720px] px-5 text-center text-lg text-ink-2 sm:px-8">
              There is no song request page with this name.
            </p>
          }
        >
          <div
            class={`mx-auto flex max-w-[720px] flex-col gap-6 px-5 sm:px-8 ${styled() ? "" : "invisible"}`}
          >
            <form ref={(element) => (form = element)} class={cardClass} onSubmit={submit}>
              <div class="flex flex-col gap-1">
                <h2 class="text-[28px] leading-tight font-semibold text-balance">
                  {profile()?.title ?? defaultTitle}
                </h2>
                <p class="text-ink-2">
                  Paste the Apple Music link of a song and it goes straight into the queue.
                </p>
              </div>
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
            <Queue status={status()} name={profile()?.name} />
          </div>
        </Show>
      </main>
      <PoweredBy />
    </div>
  );
}

export default function SongRequest(props: { twitch?: string }) {
  const id = new URLSearchParams(location.search).get("streamer");
  const twitch = props.twitch ?? null;

  return (
    <Show
      when={id || twitch}
      fallback={
        <>
          <Nav />
          <main>
            <SongRequestAdmin />
          </main>
          <Footer />
        </>
      }
    >
      <Viewer id={id} twitch={twitch} />
    </Show>
  );
}
