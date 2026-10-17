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
import Icon from "../components/Icon";
import { cardClass, inputClass } from "../components/PageParts";
import { links, pages } from "../data/links";
import {
  BackendError,
  captureSong,
  captureSpin,
  captureTip,
  formatDuration,
  formatPrice,
  hundredths,
  idOfTwitchLogin,
  musicStatus,
  orderSong,
  orderSpin,
  orderTip,
  streamerPaywall,
  streamerProfile,
  streamerSpin,
  streamerTips,
  type MusicStatus,
  type PayPalConfig,
  type Profile,
  type PublicPaywall,
  type PublicSpin,
  type PublicTips,
  type Wheel,
} from "../data/remoteControl";

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

function PayPalButton(props: { config: PayPalConfig; options: PayPalOptions; disabled: boolean }) {
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
        environment: props.config.environment,
      });
      const instance = await paypal?.createInstance({
        clientId: props.config.clientId,
        merchantId: props.config.merchantId,
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
      <div
        ref={(element) => (container = element)}
        class={`transition-opacity [color-scheme:light] ${
          props.disabled ? "pointer-events-none opacity-40" : ""
        }`}
      />
      <Show when={props.config.environment === "sandbox"}>
        <p role="status" class="text-amber">
          PayPal sandbox. This is a test payment and no real money is moved.
        </p>
      </Show>
      <Show when={failed()}>
        <p role="status" class="text-amber">
          PayPal could not be loaded. Reload the page to try again.
        </p>
      </Show>
    </>
  );
}

function TermsLink() {
  return (
    <a href={asset(pages.terms)} class="underline hover:text-ink">
      Terms
    </a>
  );
}

type Result = { ok: boolean; text: string };

function ResultLine(props: { result: Result | null }) {
  return (
    <Show when={props.result}>
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
  );
}

const formatTip = (amount: number, currency: string) =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    minimumFractionDigits: amount % 100 === 0 ? 0 : 2,
  }).format(amount / 100);

const chipClass =
  "relative inline-flex h-12 min-w-20 cursor-pointer items-center justify-center rounded-full border-[1.5px] border-line bg-well px-5 font-display text-lg font-semibold text-ink transition-colors hover:border-leaf has-[:checked]:border-moss has-[:checked]:bg-moss has-[:checked]:text-white has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-ink";

function AmountChips(props: {
  group: string;
  amounts: number[];
  currency: string;
  choice: number | "other";
  onChoice: (choice: number | "other") => void;
  other?: boolean;
}) {
  return (
    <div class="flex flex-wrap gap-3">
      <For each={props.amounts}>
        {(amount) => (
          <label class={chipClass}>
            <input
              type="radio"
              name={props.group}
              checked={props.choice === amount}
              onChange={() => props.onChoice(amount)}
              class="sr-only"
            />
            {formatTip(amount, props.currency)}
          </label>
        )}
      </For>
      <Show when={props.other}>
        <label class={chipClass}>
          <input
            type="radio"
            name={props.group}
            checked={props.choice === "other"}
            onChange={() => props.onChoice("other")}
            class="sr-only"
          />
          Other
        </label>
      </Show>
    </div>
  );
}

function MessageField(props: {
  message: string;
  onMessage: (message: string) => void;
  placeholder: string;
}) {
  return (
    <label class="flex flex-col gap-2">
      <span class="flex items-baseline justify-between gap-4 text-[15px] font-semibold text-ink-2">
        Message (optional)
        <span class="font-mono text-sm font-normal text-muted tabular-nums">
          {props.message.length}/200
        </span>
      </span>
      <textarea
        maxLength={200}
        rows={3}
        value={props.message}
        onInput={(event) => props.onMessage(event.currentTarget.value)}
        placeholder={props.placeholder}
        class={`${inputClass} h-auto resize-y py-3`}
      />
    </label>
  );
}

function TipJar(props: {
  id: string;
  tips: PublicTips & PayPalConfig;
  name: string;
  onName: (name: string) => void;
  onError: (error: unknown) => void;
}) {
  const [choice, setChoice] = createSignal<number | "other">(
    props.tips.amounts[Math.min(1, props.tips.amounts.length - 1)],
  );
  const [other, setOther] = createSignal("");
  const [message, setMessage] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(null);
  let form: HTMLFormElement | undefined;
  let orderError: unknown = null;

  const amount = () => {
    const current = choice();
    if (current !== "other") {
      return current;
    }
    const cents = hundredths(other());
    return cents >= 50 && cents <= 100_000 ? cents : null;
  };

  const showError = (error: unknown) => {
    setResult({ ok: false, text: errorText(error) });
    props.onError(error);
  };

  const options: PayPalOptions = {
    start: () => {
      setResult(null);
      if (busy() || amount() === null || !form?.reportValidity()) {
        return false;
      }
      saveName(props.name.trim());
      return true;
    },
    createOrder: async () => {
      orderError = null;
      try {
        return {
          orderId: await orderTip(props.id, amount() ?? 0, props.name.trim(), message().trim()),
        };
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    onApprove: async (data) => {
      setBusy(true);
      setResult({ ok: true, text: "Payment approved. Completing it…" });
      try {
        await captureTip(props.id, data.orderId);
        setResult({ ok: true, text: "Thanks for the tip!" });
        setMessage("");
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
      }
    },
    onCancel: () => {
      setResult({ ok: false, text: "Payment cancelled. No tip was sent." });
    },
    onError: () => {
      if (orderError) {
        showError(orderError);
      } else {
        setResult({ ok: false, text: "PayPal could not complete the payment. Try again." });
      }
    },
  };

  return (
    <form
      ref={(element) => (form = element)}
      class={cardClass}
      onSubmit={(event) => event.preventDefault()}
    >
      <p class="text-ink-2">Pick an amount and add a message if you like.</p>
      <fieldset class="flex flex-col gap-3">
        <legend class="mb-2 text-[15px] font-semibold text-ink-2">Amount</legend>
        <AmountChips
          group="tip"
          amounts={props.tips.amounts}
          currency={props.tips.currency}
          choice={choice()}
          onChoice={setChoice}
          other
        />
        <Show when={choice() === "other"}>
          <label class="flex max-w-60 flex-col gap-2">
            <span class="text-[15px] font-semibold text-ink-2">
              Your amount in {props.tips.currency}
            </span>
            <input
              type="text"
              required
              pattern="\d+([.,]\d{1,2})?"
              inputmode="decimal"
              value={other()}
              onInput={(event) => setOther(event.currentTarget.value)}
              placeholder="0.50 to 1000"
              class={inputClass}
            />
          </label>
        </Show>
      </fieldset>
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">Your name (optional)</span>
        <input
          type="text"
          maxLength={25}
          value={props.name}
          onInput={(event) => props.onName(event.currentTarget.value)}
          placeholder="Shown with your message"
          autocomplete="nickname"
          class={inputClass}
        />
      </label>
      <MessageField message={message()} onMessage={setMessage} placeholder="Love the stream!" />
      <div class="flex w-full max-w-sm flex-col gap-3">
        <p class="flex items-baseline justify-between gap-4 border-b border-line pb-3">
          <span class="text-ink-2">Your tip</span>
          <span class="font-display text-2xl font-semibold">
            {amount() === null
              ? "–"
              : formatPrice({ price: amount()!, currency: props.tips.currency })}
          </span>
        </p>
        <PayPalButton
          config={props.tips}
          options={options}
          disabled={busy() || amount() === null}
        />
        <p class="text-[15px] text-muted">
          Your tip goes straight to the streamer. <TermsLink />
        </p>
      </div>
      <ResultLine result={result()} />
    </form>
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
      <div class="mx-auto flex max-w-[720px] flex-col items-center gap-4 px-5 pt-8 pb-6 text-center sm:px-8 lg:pt-12">
        <Show
          when={props.profile}
          fallback={
            <>
              <span class="size-28 rounded-full bg-ghost sm:size-32" />
              <Show when={props.unknown}>
                <h1 class="text-[40px] leading-[1.05] font-bold sm:text-6xl">Page not found.</h1>
              </Show>
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
              <a
                href={`https://www.twitch.tv/${encodeURIComponent(profile().login)}`}
                target="_blank"
                rel="noopener"
                class="inline-flex items-center gap-1.5 text-[17px] font-semibold text-ink-2 transition-colors hover:text-ink"
              >
                <Icon name="twitch" class="size-5 shrink-0 text-twitch" />
                twitch.tv/{profile().login}
              </a>
              <Show when={profile().title || profile().message}>
                <div class="mt-2 flex flex-col gap-4">
                  <Show when={profile().title}>
                    {(title) => <p class="text-[23px] font-semibold text-balance">{title()}</p>}
                  </Show>
                  <Show when={profile().message}>
                    {(message) => (
                      <p class="text-lg whitespace-pre-line text-ink-2 text-pretty sm:text-[21px]">
                        {message()}
                      </p>
                    )}
                  </Show>
                </div>
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
        href={asset(pages.dashboard)}
        class="mx-auto flex w-fit items-center gap-2 text-[15px] font-semibold text-muted transition-colors hover:text-ink"
      >
        <img src={asset("logos/logo-default.png")} alt="" class="h-5 w-auto" />
        Made with Moblin
      </a>
    </footer>
  );
}

const spinsText = (spins: number) => (spins === 1 ? "1 spin" : `${spins} spins`);

const wheelColors = [
  "fill-wheel-1",
  "fill-wheel-2",
  "fill-wheel-3",
  "fill-wheel-4",
  "fill-wheel-5",
  "fill-wheel-6",
];

function WheelPicture(props: { wheel: Wheel }) {
  const sectors = () => {
    const total = props.wheel.options.reduce((sum, option) => sum + option.weight, 0);
    let start = 0;
    return props.wheel.options.map((option, index) => {
      const sweep = (option.weight / total) * 360;
      const middle = start + sweep / 2;
      const point = (angle: number) => {
        const radians = (angle * Math.PI) / 180;
        return `${100 * Math.sin(radians)} ${-100 * Math.cos(radians)}`;
      };
      const path =
        sweep >= 360
          ? "M 0 -100 A 100 100 0 1 1 0 100 A 100 100 0 1 1 0 -100 Z"
          : `M 0 0 L ${point(start)} A 100 100 0 ${sweep > 180 ? 1 : 0} 1 ${point(start + sweep)} Z`;
      start += sweep;
      const fontSize = Math.min(
        20,
        75 / Math.max(option.text.length * 0.55, 1),
        sweep >= 180 ? 20 : 59 * Math.sin((sweep * Math.PI) / 180) * 0.8,
      );
      return { option, path, middle, color: wheelColors[index % wheelColors.length], fontSize };
    });
  };

  return (
    <svg
      viewBox="-110 -110 220 220"
      role="img"
      aria-label={`The wheel: ${props.wheel.options.map((option) => option.text).join(", ")}`}
      class="mx-auto w-full max-w-72"
    >
      <For each={sectors()}>
        {(sector) => (
          <g>
            <path d={sector.path} class={sector.color} />
            <text
              transform={`rotate(${sector.middle - 90}) translate(59 0)`}
              text-anchor="middle"
              dominant-baseline="central"
              font-size={String(sector.fontSize)}
              class="fill-black font-body"
            >
              {sector.option.text}
            </text>
          </g>
        )}
      </For>
      <circle r="20" class="fill-white" />
      <path d="M 92 0 L 110 -9 L 110 9 Z" class="fill-white stroke-black" stroke-width="1.5" />
    </svg>
  );
}

function SpinWheel(props: {
  id: string;
  spin: PublicSpin & PayPalConfig;
  wheel: Wheel | null;
  name: string;
  onName: (name: string) => void;
  onError: (error: unknown) => void;
}) {
  const [choice, setChoice] = createSignal(props.spin.options[0]);
  const [message, setMessage] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(null);
  let orderError: unknown = null;

  const showError = (error: unknown) => {
    setResult({ ok: false, text: errorText(error) });
    props.onError(error);
  };

  const options: PayPalOptions = {
    start: () => {
      setResult(null);
      if (busy()) {
        return false;
      }
      saveName(props.name.trim());
      return true;
    },
    createOrder: async () => {
      orderError = null;
      try {
        return {
          orderId: await orderSpin(props.id, choice().spins, props.name.trim(), message().trim()),
        };
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    onApprove: async (data) => {
      setBusy(true);
      setResult({ ok: true, text: "Payment approved. Completing it…" });
      try {
        await captureSpin(props.id, data.orderId);
        setResult({
          ok: true,
          text: `Thanks for the tip! ${choice().spins === 1 ? "Your spin starts" : "Your spins start"} on stream in a moment.`,
        });
        setMessage("");
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
      }
    },
    onCancel: () => {
      setResult({
        ok: false,
        text: "Payment cancelled. No tip was sent and the wheel was not spun.",
      });
    },
    onError: () => {
      if (orderError) {
        showError(orderError);
      } else {
        setResult({ ok: false, text: "PayPal could not complete the payment. Try again." });
      }
    },
  };

  return (
    <form class={cardClass} onSubmit={(event) => event.preventDefault()}>
      <p class="text-ink-2">
        Pick how many spins you want and add a message if you like. The wheel spins live on stream,
        so watch the stream to see where it lands.
      </p>
      <Show
        when={props.wheel}
        keyed
        fallback={
          <p class="text-amber">
            The wheel is not available right now. Try again when the streamer is live.
          </p>
        }
      >
        {(wheel) => (
          <figure class="flex flex-col gap-2">
            <WheelPicture wheel={wheel} />
            <figcaption class="text-center text-sm text-muted">
              This is what the wheel looks like. It only spins on stream, not here.
            </figcaption>
          </figure>
        )}
      </Show>
      <Show when={props.spin.options.length > 1}>
        <fieldset class="flex flex-col gap-3">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">Spins</legend>
          <div class="flex flex-wrap gap-3">
            <For each={props.spin.options}>
              {(option) => (
                <label class={chipClass}>
                  <input
                    type="radio"
                    name="spin"
                    checked={choice().spins === option.spins}
                    onChange={() => setChoice(option)}
                    class="sr-only"
                  />
                  {spinsText(option.spins)} · {formatTip(option.price, props.spin.currency)}
                </label>
              )}
            </For>
          </div>
        </fieldset>
      </Show>
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">Your name (optional)</span>
        <input
          type="text"
          maxLength={25}
          value={props.name}
          onInput={(event) => props.onName(event.currentTarget.value)}
          placeholder="Shown with your spin"
          autocomplete="nickname"
          class={inputClass}
        />
      </label>
      <MessageField message={message()} onMessage={setMessage} placeholder="Land on the big one!" />
      <div class="flex w-full max-w-sm flex-col gap-3">
        <p class="flex items-baseline justify-between gap-4 border-b border-line pb-3">
          <span class="text-ink-2">Your tip · {spinsText(choice().spins)}</span>
          <span class="font-display text-2xl font-semibold">
            {formatPrice({ price: choice().price, currency: props.spin.currency })}
          </span>
        </p>
        <PayPalButton config={props.spin} options={options} disabled={busy() || !props.wheel} />
        <p class="text-[15px] text-muted">
          Your tip goes straight to the streamer. <TermsLink />
        </p>
      </div>
      <ResultLine result={result()} />
    </form>
  );
}

type Section = "tip" | "spin" | "songs";

type Way = {
  section: Section;
  icon: "heart" | "wheel" | "music";
  title: string;
  text: string;
  enabled: boolean;
};

function WaysToTip(props: {
  streamer: string | undefined;
  ways: Way[];
  section: Section | undefined;
  onSection: (section: Section) => void;
}) {
  const enabled = () => props.ways.filter((way) => way.enabled).length;

  return (
    <div class="flex flex-col gap-6">
      <div class="flex flex-col items-center gap-6 text-center">
        <h2 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-5xl">
          Tip {props.streamer ?? "the streamer"}
        </h2>
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          {enabled() === 0
            ? `${props.streamer ?? "The streamer"} isn't taking tips right now.`
            : `Every tip goes straight to ${props.streamer ?? "the streamer"}.${
                enabled() > 1 ? " Pick how you want to send yours." : ""
              }`}
        </p>
      </div>
      <div role="tablist" class="grid gap-3 sm:grid-cols-3">
        <For each={props.ways}>
          {(way) => {
            const selected = () => props.section === way.section;
            return (
              <button
                type="button"
                role="tab"
                aria-selected={selected()}
                disabled={!way.enabled}
                onClick={() => props.onSection(way.section)}
                class={`flex items-center gap-4 rounded-3xl border-[1.5px] p-4 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-45 sm:flex-col sm:items-start sm:gap-3 sm:p-5 ${
                  selected()
                    ? "border-moss bg-well"
                    : "border-line bg-card enabled:hover:border-leaf"
                }`}
              >
                <span
                  class={`flex size-12 shrink-0 items-center justify-center rounded-full transition-colors ${
                    selected()
                      ? "bg-moss text-white"
                      : way.enabled
                        ? "bg-well text-leaf"
                        : "bg-well text-muted"
                  }`}
                >
                  <Icon name={way.icon} class="size-6" />
                </span>
                <span class="flex min-w-0 flex-col gap-0.5">
                  <span class="font-display text-xl font-semibold text-ink">{way.title}</span>
                  <span class="text-[15px] text-ink-2">{way.text}</span>
                </span>
              </button>
            );
          }}
        </For>
      </div>
    </div>
  );
}

function Viewer(props: { id: string | null; twitch: string | null }) {
  const [id, setId] = createSignal(props.id);
  const [status, setStatus] = createSignal<Status>("loading");
  const [song, setSong] = createSignal("");
  const [name, setName] = createSignal(readName());
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(null);
  const [paywall, setPaywall] = createSignal<PublicPaywall | null>(null);
  const [tips, setTips] = createSignal<PublicTips | null>(null);
  const [spin, setSpin] = createSignal<PublicSpin | null>(null);
  const [wheelOfLuck, setWheelOfLuck] = createSignal<Wheel | null>(null, {
    equals: (a, b) => JSON.stringify(a) === JSON.stringify(b),
  });
  const [profile, setProfile] = createSignal<Profile | null>(null);
  const [styled, setStyled] = createSignal(false);
  const [section, setSection] = createSignal<Section | null>(
    (["tip", "spin", "songs"] as const).find((name) => location.hash === `#${name}`) ?? null,
  );
  const sections = (): Section[] => [
    ...(tipJar() ? (["tip"] as const) : []),
    ...(wheel() ? (["spin"] as const) : []),
    ...(songs() ? (["songs"] as const) : []),
  ];
  const songs = () => payable() !== null;
  const shown = () => {
    const current = section();
    return current && sections().includes(current) ? current : sections()[0];
  };

  const ways = (): Way[] => {
    return [
      {
        section: "tip",
        icon: "heart",
        title: "Just a tip",
        text: "Send a tip with a message if you like.",
        enabled: tipJar() !== null,
      },
      {
        section: "spin",
        icon: "wheel",
        title: "Spin the wheel",
        text: "Your tip spins the wheel live on stream.",
        enabled: wheel() !== null,
      },
      {
        section: "songs",
        icon: "music",
        title: "Request a song",
        text: "Your tip puts a song in the queue.",
        enabled: songs(),
      },
    ];
  };

  const changeSection = (next: Section) => {
    setSection(next);
    history.replaceState(null, "", `${location.pathname}${location.search}#${next}`);
  };
  let form: HTMLFormElement | undefined;
  const payable = createMemo(
    () => {
      const current = paywall();
      return current?.clientId && current.merchantId && current.environment
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
  const tipJar = createMemo(
    () => {
      const current = tips();
      return current?.clientId && current.environment
        ? { ...current, clientId: current.clientId, environment: current.environment }
        : null;
    },
    null,
    {
      equals: (a, b) =>
        a?.clientId === b?.clientId &&
        a?.merchantId === b?.merchantId &&
        a?.environment === b?.environment &&
        a?.currency === b?.currency &&
        a?.amounts.join() === b?.amounts.join(),
    },
  );
  const wheel = createMemo(
    () => {
      const current = spin();
      return current?.clientId && current.environment
        ? { ...current, clientId: current.clientId, environment: current.environment }
        : null;
    },
    null,
    {
      equals: (a, b) =>
        a?.clientId === b?.clientId &&
        a?.merchantId === b?.merchantId &&
        a?.environment === b?.environment &&
        a?.currency === b?.currency &&
        JSON.stringify(a?.options) === JSON.stringify(b?.options),
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

  const refreshTips = async () => {
    const current = id();
    if (current) {
      try {
        setTips(await streamerTips(current));
      } catch {}
    }
  };

  const tipError = (error: unknown) => {
    if (error instanceof BackendError && error.status === 400) {
      void refreshTips();
    }
  };

  const refreshSpin = async () => {
    const current = id();
    if (current) {
      try {
        const found = await streamerSpin(current);
        setSpin(found.spin);
        setWheelOfLuck(found.wheel);
      } catch {}
    }
  };

  const spinError = (error: unknown) => {
    if (error instanceof BackendError && error.status === 400) {
      void refreshSpin();
    }
  };

  const requested = () => {
    setResult({
      ok: true,
      text: "Thanks for the tip! Your song will be added to the queue in a moment.",
    });
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
        requested();
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
      }
    },
    onCancel: () => {
      setResult({ ok: false, text: "Payment cancelled. No tip was sent and no song was added." });
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
    if (!current || !songs()) {
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
          document.title = `${found.name} – Moblin`;
        })
        .catch(() => {})
        .finally(() => setStyled(true));
    }
    void refreshPaywall().then(refresh);
    void refreshTips();
    void refreshSpin();
    const timer = setInterval(() => {
      void refresh();
      void refreshSpin();
    }, 30_000);
    onCleanup(() => clearInterval(timer));
  });

  return (
    <div data-accent={profile()?.accent ?? undefined} class="isolate flex min-h-svh flex-col">
      <main class="flex-1">
        <StreamerHeader profile={profile()} unknown={status() === "unknown"} />
        <Show
          when={status() !== "unknown"}
          fallback={
            <p class="mx-auto max-w-[720px] px-5 text-center text-lg text-ink-2 sm:px-8">
              There is no page with this name.
            </p>
          }
        >
          <div
            class={`mx-auto flex max-w-[720px] flex-col gap-6 px-5 sm:px-8 ${styled() ? "" : "invisible"}`}
          >
            <WaysToTip
              streamer={profile()?.name}
              ways={ways()}
              section={shown()}
              onSection={changeSection}
            />
            <form
              ref={(element) => (form = element)}
              class={`${cardClass} ${shown() === "songs" ? "" : "hidden"}`}
              onSubmit={(event) => event.preventDefault()}
            >
              <p class="text-ink-2">
                Paste the Apple Music link of a song and it goes straight into the queue.
              </p>
              <Show when={status() === "offline"}>
                <p class="text-amber">
                  Song requests are not available right now. Try again when the streamer is live.
                </p>
              </Show>
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
                <Show when={payable()} keyed>
                  {(current) => (
                    <div class="flex w-full max-w-sm flex-col gap-3">
                      <p class="flex items-baseline justify-between gap-4 border-b border-line pb-3">
                        <span class="text-ink-2">Your tip</span>
                        <span class="font-display text-2xl font-semibold">
                          {formatPrice(current)}
                        </span>
                      </p>
                      <PayPalButton
                        config={current}
                        options={payPalOptions}
                        disabled={busy() || typeof status() === "string" || !song().trim()}
                      />
                      <p class="text-[15px] text-muted">
                        Your tip goes straight to the streamer. <TermsLink />
                      </p>
                    </div>
                  )}
                </Show>
              </div>
              <ResultLine result={result()} />
            </form>
            <Show when={tipJar()} keyed>
              {(current) => (
                <div class={shown() === "tip" ? "" : "hidden"}>
                  <TipJar
                    id={id() ?? ""}
                    tips={current}
                    name={name()}
                    onName={setName}
                    onError={tipError}
                  />
                </div>
              )}
            </Show>
            <Show when={wheel()} keyed>
              {(current) => (
                <div class={shown() === "spin" ? "" : "hidden"}>
                  <SpinWheel
                    id={id() ?? ""}
                    spin={current}
                    wheel={wheelOfLuck()}
                    name={name()}
                    onName={setName}
                    onError={spinError}
                  />
                </div>
              )}
            </Show>
            <div class={shown() === "songs" && status() !== "offline" ? "" : "hidden"}>
              <Queue status={status()} />
            </div>
          </div>
        </Show>
      </main>
      <PoweredBy />
    </div>
  );
}

export default function Page(props: { twitch?: string }) {
  const id = new URLSearchParams(location.search).get("streamer");
  const twitch = props.twitch ?? null;

  return <Viewer id={id} twitch={twitch} />;
}
