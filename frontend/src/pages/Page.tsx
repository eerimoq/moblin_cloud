import { loadCoreSdkScript, type OneTimePaymentSession } from "@paypal/paypal-js/sdk-v6";
import {
  loadStripe,
  type Stripe,
  type StripeElements,
  type StripePaymentElement,
} from "@stripe/stripe-js";
import {
  batch,
  createEffect,
  createMemo,
  createSignal,
  For,
  Match,
  on,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";
import { asset } from "../asset";
import { fetchAccount, readSession, type Account } from "../data/account";
import Icon from "../components/Icon";
import Queue, { type Status } from "../components/Queue";
import { fireConfetti } from "../confetti";
import { cardClass, ghostButtonClass, inputClass, Notice } from "../components/PageParts";
import { links, pages } from "../data/links";
import {
  accentAttributes,
  BackendError,
  captureSong,
  captureSpin,
  captureTip,
  formatPrice,
  fetchSongAllowance,
  fetchSpinAllowance,
  hundredths,
  idOfTwitchLogin,
  loadFonts,
  musicStatus,
  orderSong,
  orderSpin,
  orderTip,
  pageImageUrl,
  requestSubscriberSong,
  requestSubscriberSpin,
  streamerProfile,
  streamerSongs,
  streamerSpin,
  streamerTips,
  tierCount,
  tierNumbers,
  viewerSignInUrl,
  type Allowance,
  type Order,
  type PayPalConfig,
  type Profile,
  type Provider,
  type PublicPayment,
  type PublicPaywall,
  type PublicSpin,
  type PublicTips,
  type StripeConfig,
  type Tiers,
  type Wheel,
} from "../data/remoteControl";
import { t, tRich } from "../i18n";

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

const viewerKey = (id: string) => `viewerSession:${id}`;

function readViewer(id: string) {
  try {
    return localStorage.getItem(viewerKey(id));
  } catch {
    return null;
  }
}

function saveViewer(id: string, viewer: string | null) {
  try {
    if (viewer) {
      localStorage.setItem(viewerKey(id), viewer);
    } else {
      localStorage.removeItem(viewerKey(id));
    }
  } catch {}
}

const sectionKey = "viewerSignInSection";

function takeSection(): Section {
  try {
    const section = sessionStorage.getItem(sectionKey);
    sessionStorage.removeItem(sectionKey);
    return section === "tip" || section === "spin" ? section : "songs";
  } catch {
    return "songs";
  }
}

function takeViewerSignIn(): { viewer: string | null; failed: boolean; section: Section } {
  const hash = location.hash.slice(1);
  const viewer = new URLSearchParams(hash).get("viewer");
  const failed = hash === "viewerSignInFailed";
  const section = viewer || failed ? takeSection() : "songs";
  if (viewer || failed) {
    history.replaceState(null, "", `${location.pathname}${location.search}#${section}`);
  }
  return { viewer, failed, section };
}

function signInAsViewer(id: string, section: Section) {
  const login = import.meta.env.DEV
    ? prompt("Fake Twitch login (sub1, sub2, sub3 or any other)", "sub1")
    : null;
  try {
    sessionStorage.setItem(sectionKey, section);
  } catch {}
  location.href = viewerSignInUrl(id, login ?? undefined);
}

function errorText(error: unknown) {
  return error instanceof BackendError && error.message
    ? error.message
    : t("Something went wrong. Try again.");
}

type Payment = {
  paypal: PayPalConfig | null;
  stripe: StripeConfig | null;
};

function paymentOf(current: PublicPayment): Payment | null {
  const paypal =
    current.clientId && current.merchantId && current.environment
      ? {
          clientId: current.clientId,
          merchantId: current.merchantId,
          environment: current.environment,
        }
      : null;
  const stripe = current.stripe ?? null;
  return paypal || stripe ? { paypal, stripe } : null;
}

const sameData = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const providerName = (provider: Provider) => (provider === "stripe" ? "Stripe" : "PayPal");

type PayOptions = {
  start: () => boolean;
  createOrder: (provider: Provider) => Promise<Order>;
  complete: (orderId: string) => Promise<void>;
  onCancel: () => void;
  onError: (provider: Provider) => void;
};

function PayPalButton(props: { config: PayPalConfig; options: PayOptions; disabled: boolean }) {
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
      await session.start({ presentationMode: "auto" }, props.options.createOrder("paypal"));
    } catch {
      props.options.onError("paypal");
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
        onApprove: (data) => props.options.complete(data.orderId),
        onCancel: props.options.onCancel,
        onError: () => props.options.onError("paypal"),
      });
      button = document.createElement("paypal-button");
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
        class={`text-center transition-opacity [color-scheme:light] [&>paypal-button]:mx-auto ${
          props.disabled ? "pointer-events-none opacity-40" : ""
        }`}
      />
      <Show when={props.config.environment === "sandbox"}>
        <Notice status>
          {t("PayPal sandbox. This is a test payment and no real money is moved.")}
        </Notice>
      </Show>
      <Show when={failed()}>
        <Notice error>{t("PayPal could not be loaded. Reload the page to try again.")}</Notice>
      </Show>
    </>
  );
}

const stripeButtonClass =
  "inline-flex items-center justify-center gap-2.5 bg-stripe font-display text-lg font-semibold whitespace-nowrap text-white transition-colors not-disabled:hover:bg-stripe-bright disabled:opacity-60";

type CardPayment = {
  order: Order;
  stripe: Stripe;
  elements: StripeElements;
  element: StripePaymentElement;
};

function StripeCard(props: {
  config: StripeConfig;
  options: PayOptions;
  disabled: boolean;
  resetKey: string;
}) {
  let container: HTMLDivElement | undefined;
  const [payment, setPayment] = createSignal<CardPayment | null>(null);
  const [working, setWorking] = createSignal(false);
  const [cardError, setCardError] = createSignal<string | null>(null);
  const [failed, setFailed] = createSignal(false);

  const close = () => {
    payment()?.element.destroy();
    setPayment(null);
    setCardError(null);
  };
  onCleanup(close);
  createEffect(on(() => props.resetKey, close, { defer: true }));

  const appearance = () => {
    const style = getComputedStyle(container!);
    const color = (name: string) => style.getPropertyValue(`--color-${name}`).trim();
    return {
      theme: matchMedia("(prefers-color-scheme: light)").matches
        ? ("stripe" as const)
        : ("night" as const),
      variables: {
        colorPrimary: color("moss"),
        colorBackground: color("well"),
        colorText: color("ink"),
        colorTextSecondary: color("muted"),
        borderRadius: "16px",
      },
    };
  };

  const open = async () => {
    if (props.disabled || working() || !props.options.start()) {
      return;
    }
    setWorking(true);
    setFailed(false);
    let order: Order;
    try {
      order = await props.options.createOrder("stripe");
    } catch {
      props.options.onError("stripe");
      setWorking(false);
      return;
    }
    try {
      const stripe = await loadStripe(props.config.publishableKey, {
        stripeAccount: props.config.accountId,
      });
      if (!stripe || !container || !order.clientSecret) {
        throw new Error("no Stripe");
      }
      const elements = stripe.elements({
        clientSecret: order.clientSecret,
        appearance: appearance(),
      });
      const element = elements.create("payment", { layout: "tabs" });
      element.mount(container);
      setPayment({ order, stripe, elements, element });
    } catch {
      setFailed(true);
    } finally {
      setWorking(false);
    }
  };

  const pay = async () => {
    const current = payment();
    if (!current || working()) {
      return;
    }
    setWorking(true);
    setCardError(null);
    try {
      const { error } = await current.stripe.confirmPayment({
        elements: current.elements,
        redirect: "if_required",
        confirmParams: { return_url: location.href },
      });
      if (error?.type === "card_error" || error?.type === "validation_error") {
        setCardError(error.message ?? t("The payment did not go through. Try another card."));
        return;
      }
      close();
      if (error) {
        props.options.onError("stripe");
        return;
      }
      await props.options.complete(current.order.orderId);
    } finally {
      setWorking(false);
    }
  };

  const cancel = () => {
    close();
    props.options.onCancel();
  };

  return (
    <div class="flex flex-col gap-3">
      <Show when={!payment()}>
        <button
          type="button"
          class={`${stripeButtonClass} h-[45px] w-[135px] self-center rounded-[4px] px-3`}
          disabled={props.disabled || working()}
          onClick={() => void open()}
        >
          <Show when={!working()} fallback={t("Loading…")}>
            <svg viewBox="54 36 360.02 149.84" class="h-6" fill="currentColor" role="img">
              <title>Stripe</title>
              <path d="M414,113.4c0-25.6-12.4-45.8-36.1-45.8c-23.8,0-38.2,20.2-38.2,45.6c0,30.1,17,45.3,41.4,45.3c11.9,0,20.9-2.7,27.7-6.5v-20c-6.8,3.4-14.6,5.5-24.5,5.5c-9.7,0-18.3-3.4-19.4-15.2h48.9C413.8,121,414,115.8,414,113.4z M364.6,103.9c0-11.3,6.9-16,13.2-16c6.1,0,12.6,4.7,12.6,16H364.6z" />
              <path d="M301.1,67.6c-9.8,0-16.1,4.6-19.6,7.8l-1.3-6.2h-22v116.6l25-5.3l0.1-28.3c3.6,2.6,8.9,6.3,17.7,6.3c17.9,0,34.2-14.4,34.2-46.1C335.1,83.4,318.6,67.6,301.1,67.6z M295.1,136.5c-5.9,0-9.4-2.1-11.8-4.7l-0.1-37.1c2.6-2.9,6.2-4.9,11.9-4.9c9.1,0,15.4,10.2,15.4,23.3C310.5,126.5,304.3,136.5,295.1,136.5z" />
              <path d="M223.8,61.7l25.1-5.4V36l-25.1,5.3z M223.8,69.3h25.1v87.5h-25.1z" />
              <path d="M196.9,76.7l-1.6-7.4h-21.6v87.5h25V97.5c5.9-7.7,15.9-6.3,19-5.2v-23C214.5,68.1,202.8,65.9,196.9,76.7z" />
              <path d="M146.9,47.6l-24.4,5.2l-0.1,80.1c0,14.8,11.1,25.7,25.9,25.7c8.2,0,14.2-1.5,17.5-3.3V135c-3.2,1.3-19,5.9-19-8.9V90.6h19V69.3h-19L146.9,47.6z" />
              <path d="M79.3,94.7c0-3.9,3.2-5.4,8.5-5.4c7.6,0,17.2,2.3,24.8,6.4V72.2c-8.3-3.3-16.5-4.6-24.8-4.6C67.5,67.6,54,78.2,54,95.9c0,27.6,38,23.2,38,35.1c0,4.6-4,6.1-9.6,6.1c-8.3,0-18.9-3.4-27.3-8v23.8c9.3,4,18.7,5.7,27.3,5.7c20.8,0,35.1-10.3,35.1-28.2C117.4,100.6,79.3,105.9,79.3,94.7z" />
            </svg>
          </Show>
        </button>
      </Show>
      <div ref={(element) => (container = element)} class={payment() ? "" : "hidden"} />
      <Show when={payment()}>
        {(current) => (
          <>
            <Show when={cardError()}>{(error) => <Notice error>{error()}</Notice>}</Show>
            <div class="flex flex-wrap items-center gap-3">
              <button
                type="button"
                class={`${stripeButtonClass} h-12 rounded-lg px-6`}
                disabled={working()}
                onClick={() => void pay()}
              >
                {current().order.price !== undefined && current().order.currency
                  ? t("Tip {price}", {
                      price: formatPrice({
                        price: current().order.price!,
                        currency: current().order.currency!,
                      }),
                    })
                  : t("Tip")}
              </button>
              <button type="button" class={ghostButtonClass} disabled={working()} onClick={cancel}>
                {t("Cancel")}
              </button>
            </div>
          </>
        )}
      </Show>
      <Show when={props.config.publishableKey.startsWith("pk_test_")}>
        <Notice status>
          {t("Stripe test mode. This is a test payment and no real money is moved.")}
        </Notice>
      </Show>
      <Show when={failed()}>
        <Notice error>{t("Stripe could not be loaded. Reload the page to try again.")}</Notice>
      </Show>
    </div>
  );
}

function PayButtons(props: {
  payment: Payment;
  options: PayOptions;
  disabled: boolean;
  resetKey: string;
}) {
  return (
    <div class="flex flex-col gap-3">
      <Show when={props.payment.paypal}>
        {(paypal) => (
          <PayPalButton config={paypal()} options={props.options} disabled={props.disabled} />
        )}
      </Show>
      <Show when={props.payment.paypal && props.payment.stripe}>
        <p class="text-center text-xs font-semibold tracking-widest text-muted uppercase">
          {t("or")}
        </p>
      </Show>
      <Show when={props.payment.stripe}>
        {(stripe) => (
          <StripeCard
            config={stripe()}
            options={props.options}
            disabled={props.disabled}
            resetKey={props.resetKey}
          />
        )}
      </Show>
    </div>
  );
}

function TermsLink() {
  return (
    <a href={asset(pages.terms)} class="underline hover:text-ink">
      {t("Terms")}
    </a>
  );
}

type Result = { text: string };

function CheerCard(props: { title: string; text: string; onClose: () => void }) {
  let overlay: HTMLDivElement | undefined;
  let close: HTMLButtonElement | undefined;
  const onKey = (event: KeyboardEvent) => event.key === "Escape" && props.onClose();
  onMount(() => {
    if (overlay) {
      fireConfetti(overlay);
    }
    close?.focus();
    addEventListener("keydown", onKey);
  });
  onCleanup(() => removeEventListener("keydown", onKey));

  return (
    <div
      ref={(element) => (overlay = element)}
      role="dialog"
      aria-modal="true"
      onClick={(event) => event.target === event.currentTarget && props.onClose()}
      class="cheer-fade fixed inset-0 z-50 flex items-center justify-center bg-page/60 px-4 backdrop-blur-md"
    >
      <button
        ref={(element) => (close = element)}
        type="button"
        aria-label={t("Close")}
        onClick={() => props.onClose()}
        class="absolute top-4 right-4 z-10 grid size-11 place-items-center rounded-full border-[1.5px] border-ghost-line bg-ghost text-ink hover:bg-ghost-hover"
      >
        <Icon name="close" class="size-5" />
      </button>
      <div class="cheer-pop relative z-10 flex max-w-md flex-col items-center gap-2 rounded-3xl border-[1.5px] border-moss bg-card px-10 pt-24 pb-8 text-center shadow-2xl">
        <img
          src={asset("logos/logo-party.png")}
          alt=""
          class="absolute -top-20 left-1/2 h-40 w-auto -translate-x-1/2 drop-shadow-xl"
        />
        <span class="font-display text-3xl font-semibold text-ink">{props.title}</span>
        <span class="text-lg text-ink-2">{props.text}</span>
      </div>
    </div>
  );
}

function TwitchBox(props: {
  kind: "song" | "spin";
  tiers: Tiers;
  allowance: Allowance | null;
  signedIn: boolean;
  paid: boolean;
  disabled: boolean;
  result: Result | null;
  onSignIn: () => void;
  onSignOut: () => void;
  onRequest: () => void;
}) {
  const tiers = () =>
    tierNumbers
      .map((tier) => ({ tier, count: tierCount(props.tiers, tier) }))
      .filter((tier) => tier.count > 0);
  const song = () => props.kind === "song";
  const signOut = () => (
    <button type="button" class="underline hover:text-ink" onClick={() => props.onSignOut()}>
      {t("Sign out")}
    </button>
  );
  const amount = (count: number) => (song() ? count * 5 : count);
  const offer = () => {
    const values = {
      counts: tiers()
        .map((tier) => amount(tier.count))
        .join(" / "),
      tiers: tiers()
        .map((tier) => tier.tier)
        .join(" / "),
    };
    return song()
      ? t("{counts} minutes a month for tier {tiers}", values)
      : t("{counts} spins a month for tier {tiers}", values);
  };

  return (
    <div class="flex w-full flex-col gap-4 border-t border-line pt-6">
      <p class="flex flex-col items-center gap-1 text-center">
        <span class="text-ink-2">
          <Show when={props.allowance} fallback={offer()}>
            {(current) => (
              <Switch fallback={offer()}>
                <Match when={current().tier === null}>
                  {t("You are not subscribed to this streamer on Twitch.")}
                </Match>
                <Match when={current().allowance === 0}>
                  {song()
                    ? t("Tier {tier} subscribers can't request songs here.", {
                        tier: current().tier ?? 0,
                      })
                    : t("Tier {tier} subscribers can't spin the wheel here.", {
                        tier: current().tier ?? 0,
                      })}
                </Match>
                <Match when={current().left === 0}>
                  {song()
                    ? t(
                        "You have used all your minutes this month. New ones come on the 1st of the month.",
                      )
                    : t(
                        "You have used all your spins this month. New ones come on the 1st of the month.",
                      )}
                </Match>
                <Match when={current().left > 0}>
                  {song()
                    ? t("{left} of {allowance} minutes left this month", {
                        left: amount(current().left),
                        allowance: amount(current().allowance),
                      })
                    : t("{left} of {allowance} spins left this month", {
                        left: current().left,
                        allowance: current().allowance,
                      })}
                </Match>
              </Switch>
            )}
          </Show>
        </span>
        <Show when={song() && !props.paid}>
          <span class="text-sm text-muted">
            {t("Every started 5 minutes of the song counts, and it always plays in full.")}
          </span>
        </Show>
      </p>
      <Show when={!props.paid}>
        <ResultLine result={props.result} />
      </Show>
      <Show when={!props.signedIn}>
        <button
          type="button"
          class={`${ghostButtonClass} self-center`}
          onClick={() => props.onSignIn()}
        >
          <Icon name="twitch" class="size-5 text-twitch" />
          {t("Sign in with Twitch")}
        </button>
      </Show>
      <Show when={props.allowance}>
        {(current) => (
          <>
            <Show when={current().left > 0}>
              <button
                type="button"
                class={`${ghostButtonClass} self-center`}
                disabled={props.disabled}
                onClick={() => props.onRequest()}
              >
                <Icon name="twitch" class="size-5 text-twitch" />
                {song() ? t("Request song") : t("Spin the wheel")}
              </button>
            </Show>
            <p class="text-center text-sm text-muted">
              {current().tier === null
                ? tRich("Signed in as {name}. {signOut} and in again if your sub changed.", {
                    name: current().name,
                    signOut: signOut(),
                  })
                : tRich(
                    "Signed in as {name}, tier {tier}. {signOut} and in again if your sub changed.",
                    { name: current().name, tier: String(current().tier), signOut: signOut() },
                  )}
            </p>
          </>
        )}
      </Show>
    </div>
  );
}

function ResultLine(props: { result: Result | null }) {
  return (
    <Show when={props.result}>
      {(result) => (
        <p
          role="alert"
          class="flex items-start gap-3 rounded-2xl border-[1.5px] border-danger-line bg-danger-bg px-4 py-3 text-[17px] font-semibold text-danger-ink"
        >
          <Icon name="warning" class="mt-0.5 size-5 shrink-0" />
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
          {t("Other")}
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
        {t("Message (optional)")}
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
  tips: PublicTips & { payment: Payment };
  name: string;
  onName: (name: string) => void;
  onError: (error: unknown) => void;
  onPaid: (text: string) => void;
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
    return cents >= Math.min(...props.tips.amounts) ? cents : null;
  };

  const showError = (error: unknown) => {
    setResult({ text: errorText(error) });
    props.onError(error);
  };

  const options: PayOptions = {
    start: () => {
      setResult(null);
      if (busy() || amount() === null || !form?.reportValidity()) {
        return false;
      }
      saveName(props.name.trim());
      return true;
    },
    createOrder: async (provider) => {
      orderError = null;
      try {
        return await orderTip(
          props.id,
          amount() ?? 0,
          props.name.trim(),
          message().trim(),
          provider,
        );
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    complete: async (orderId) => {
      setBusy(true);
      setResult(null);
      try {
        await captureTip(props.id, orderId);
        props.onPaid(t("Your tip is on its way to the streamer."));
        setMessage("");
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
      }
    },
    onCancel: () => {
      setResult({ text: t("Payment cancelled. No tip was sent.") });
    },
    onError: (provider) => {
      if (orderError) {
        showError(orderError);
      } else {
        setResult({
          text: t("{provider} could not complete the payment. Try again.", {
            provider: providerName(provider),
          }),
        });
      }
    },
  };

  return (
    <form
      ref={(element) => (form = element)}
      class={cardClass}
      onSubmit={(event) => event.preventDefault()}
    >
      <fieldset class="flex flex-col gap-3">
        <legend class="mb-2 text-[15px] font-semibold text-ink-2">{t("Amount")}</legend>
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
              {t("Your amount in {currency}", { currency: props.tips.currency })}
            </span>
            <input
              type="text"
              required
              pattern="\d+([.,]\d{1,2})?"
              inputmode="decimal"
              value={other()}
              onInput={(event) => setOther(event.currentTarget.value)}
              placeholder={t("At least {minimum}", {
                minimum: (Math.min(...props.tips.amounts) / 100).toFixed(2),
              })}
              class={inputClass}
            />
          </label>
        </Show>
      </fieldset>
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">{t("Your name (optional)")}</span>
        <input
          type="text"
          maxLength={25}
          value={props.name}
          onInput={(event) => props.onName(event.currentTarget.value)}
          placeholder={t("Shown with your message")}
          autocomplete="nickname"
          class={inputClass}
        />
      </label>
      <MessageField
        message={message()}
        onMessage={setMessage}
        placeholder={t("Love the stream!")}
      />
      <div class="flex w-full flex-col gap-4 border-t border-line pt-6">
        <p class="flex flex-col items-center gap-1 text-center">
          <span class="font-title text-sm font-semibold tracking-wider text-muted uppercase">
            {t("Your tip")}
          </span>
          <span class="font-display text-4xl font-semibold">
            {amount() === null
              ? "–"
              : formatPrice({ price: amount()!, currency: props.tips.currency })}
          </span>
        </p>
        <ResultLine result={result()} />
        <PayButtons
          payment={props.tips.payment}
          options={options}
          disabled={busy() || amount() === null}
          resetKey={`${amount()}\n${props.name}\n${message()}`}
        />
        <p class="text-center text-sm text-muted">
          {tRich("Your tip goes straight to the streamer. {terms}", { terms: <TermsLink /> })}
        </p>
      </div>
    </form>
  );
}

function Avatar(props: { profile: Profile; class: string; cutout?: boolean }) {
  const ring = (color: string) =>
    props.cutout ? "ring-4 ring-page" : `ring-4 ${color} ring-offset-4 ring-offset-page`;
  return (
    <Show
      when={props.profile.avatar}
      fallback={
        <span
          class={`flex shrink-0 items-center justify-center rounded-full bg-moss font-display font-semibold text-white ${ring("ring-moss/40")} ${props.class}`}
        >
          {props.profile.name.slice(0, 1).toUpperCase()}
        </span>
      }
    >
      {(avatar) => (
        <img
          src={avatar()}
          alt=""
          class={`shrink-0 rounded-full bg-ghost object-cover ${ring("ring-moss")} ${props.class}`}
        />
      )}
    </Show>
  );
}

function TwitchLink(props: { profile: Profile }) {
  return (
    <a
      href={`https://www.twitch.tv/${encodeURIComponent(props.profile.login)}`}
      target="_blank"
      rel="noopener"
      class="inline-flex min-w-0 items-center gap-1.5 text-[17px] font-semibold text-ink-2 transition-colors hover:text-ink"
    >
      <Icon name="twitch" class="size-5 shrink-0 text-twitch" />
      <span class="truncate">twitch.tv/{props.profile.login}</span>
    </a>
  );
}

function Greeting(props: { profile: Profile }) {
  return (
    <Show when={props.profile.title || props.profile.message}>
      <div class="mt-2 flex flex-col gap-4">
        <Show when={props.profile.title}>
          {(title) => <p class="text-[23px] font-semibold text-balance">{title()}</p>}
        </Show>
        <Show when={props.profile.message}>
          {(message) => (
            <p class="text-lg whitespace-pre-line text-ink-2 text-pretty sm:text-[21px]">
              {message()}
            </p>
          )}
        </Show>
      </div>
    </Show>
  );
}

type DashboardLink = { href: string; own: boolean };

function DashboardLink(props: { link: DashboardLink | null }) {
  return (
    <Show when={props.link}>
      {(link) => (
        <a
          href={link().href}
          class="inline-flex min-w-0 items-center gap-1.5 text-[15px] font-semibold text-leaf transition-colors hover:text-ink"
        >
          <Icon name="remote" class="size-4 shrink-0" />
          <span class="truncate">
            {link().own ? t("Open your dashboard") : t("Moderate on the dashboard")}
          </span>
        </a>
      )}
    </Show>
  );
}

async function signedInAccount(): Promise<Account | null> {
  const session = readSession();
  if (!session) {
    return null;
  }
  try {
    return await fetchAccount(session);
  } catch {
    return null;
  }
}

function dashboardLink(profile: Profile, account: Account | null): DashboardLink | null {
  if (!account) {
    return null;
  }
  const login = profile.login.toLowerCase();
  if (account.login.toLowerCase() === login) {
    return { href: asset(pages.dashboard), own: true };
  }
  const channel = account.moderating.find((channel) => channel.login.toLowerCase() === login);
  return channel
    ? {
        href: asset(`${pages.dashboard}?streamer=twitch:${encodeURIComponent(channel.login)}`),
        own: false,
      }
    : null;
}

function StreamerHeader(props: {
  id: string | null;
  profile: Profile | null;
  unknown: boolean;
  dashboard: DashboardLink | null;
}) {
  const banner = () => {
    const profile = props.profile;
    return props.id && profile?.image
      ? { profile, src: pageImageUrl(props.id, profile.image) }
      : null;
  };

  return (
    <header>
      <Show
        when={banner()}
        fallback={
          <div class="mx-auto flex max-w-[720px] flex-col items-center gap-4 px-5 pt-8 pb-6 text-center sm:px-8 lg:pt-12">
            <Show
              when={props.profile}
              fallback={
                <Show when={props.unknown}>
                  <h1 class="text-[40px] leading-[1.05] font-bold sm:text-6xl">
                    {t("Page not found.")}
                  </h1>
                </Show>
              }
            >
              {(profile) => (
                <>
                  <Avatar profile={profile()} class="size-28 text-5xl sm:size-32" />
                  <TwitchLink profile={profile()} />
                  <DashboardLink link={props.dashboard} />
                  <Greeting profile={profile()} />
                </>
              )}
            </Show>
          </div>
        }
      >
        {(banner) => (
          <div class="mx-auto flex max-w-[720px] flex-col px-5 pt-6 pb-2 sm:px-8 lg:pt-10">
            <img
              src={banner().src}
              alt=""
              class="aspect-[3/1] w-full rounded-xl bg-ghost object-cover"
            />
            <div class="flex items-start justify-between gap-4 px-4 sm:px-6">
              <Avatar
                profile={banner().profile}
                cutout
                class="relative -mt-10 size-20 text-3xl sm:-mt-12 sm:size-24 sm:text-4xl"
              />
              <div class="mt-3 flex min-w-0 flex-col items-end gap-1.5">
                <TwitchLink profile={banner().profile} />
                <DashboardLink link={props.dashboard} />
              </div>
            </div>
            <div class="px-4 sm:px-6">
              <Greeting profile={banner().profile} />
            </div>
          </div>
        )}
      </Show>
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
        Moblin Cloud
      </a>
    </footer>
  );
}

const spinsText = (spins: number) =>
  spins === 1 ? t("1 spin") : t("{count} spins", { count: spins });

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
      aria-label={t("The wheel: {options}", {
        options: props.wheel.options.map((option) => option.text).join(", "),
      })}
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
  spin: (PublicSpin & { payment: Payment }) | null;
  subSpins: Tiers | null;
  allowance: Allowance | null;
  viewer: string | null;
  wheel: Wheel | null;
  connected: boolean;
  name: string;
  onName: (name: string) => void;
  onError: (error: unknown) => void;
  onPaid: (text: string, title?: string) => void;
  onSignIn: () => void;
  onSignOut: () => void;
  onAllowance: () => void;
  signInFailed: boolean;
}) {
  const [choice, setChoice] = createSignal(props.spin?.options[0] ?? { spins: 1, price: 0 });
  const [message, setMessage] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(
    props.signInFailed ? { text: t("Signing in with Twitch failed. Try again.") } : null,
  );
  let orderError: unknown = null;

  const showError = (error: unknown) => {
    setResult({ text: errorText(error) });
    props.onError(error);
  };

  const options: PayOptions = {
    start: () => {
      setResult(null);
      if (busy()) {
        return false;
      }
      saveName(props.name.trim());
      return true;
    },
    createOrder: async (provider) => {
      orderError = null;
      try {
        return await orderSpin(props.id, choice(), props.name.trim(), message().trim(), provider);
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    complete: async (orderId) => {
      setBusy(true);
      setResult(null);
      try {
        await captureSpin(props.id, orderId);
        props.onPaid(
          choice().spins === 1
            ? t("Your spin starts on stream in a moment.")
            : t("Your spins start on stream in a moment."),
        );
        setMessage("");
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
      }
    },
    onCancel: () => {
      setResult({ text: t("Payment cancelled. No tip was sent and the wheel was not spun.") });
    },
    onError: (provider) => {
      if (orderError) {
        showError(orderError);
      } else {
        setResult({
          text: t("{provider} could not complete the payment. Try again.", {
            provider: providerName(provider),
          }),
        });
      }
    },
  };

  const spinAsSubscriber = async () => {
    if (busy() || !props.viewer) {
      return;
    }
    setBusy(true);
    setResult(null);
    try {
      await requestSubscriberSpin(props.id, props.viewer, message().trim());
      props.onPaid(t("Your spin starts on stream in a moment."), t("Enjoy the spin!"));
      setMessage("");
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
      props.onAllowance();
    }
  };

  return (
    <form class={cardClass} onSubmit={(event) => event.preventDefault()}>
      <p class="text-ink-2">
        {t("The wheel spins live on stream, so watch the stream to see where it lands.")}
      </p>
      <Show
        when={props.wheel}
        keyed
        fallback={
          <Notice>
            {props.connected
              ? t("The streamer is not taking wheel spins right now.")
              : t("The wheel is not available right now. Try again when the streamer is live.")}
          </Notice>
        }
      >
        {(wheel) => (
          <figure class="flex flex-col gap-2">
            <WheelPicture wheel={wheel} />
            <figcaption class="text-center text-sm text-muted">
              {t("This is what the wheel looks like. It only spins on stream, not here.")}
            </figcaption>
          </figure>
        )}
      </Show>
      <Show when={props.spin && props.spin.options.length > 1}>
        <fieldset class="flex flex-col gap-3">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">{t("Spins")}</legend>
          <div class="flex flex-wrap gap-3">
            <For each={props.spin?.options}>
              {(option) => (
                <label class={chipClass}>
                  <input
                    type="radio"
                    name="spin"
                    checked={choice() === option}
                    onChange={() => setChoice(option)}
                    class="sr-only"
                  />
                  {spinsText(option.spins)} · {formatTip(option.price, props.spin?.currency ?? "")}
                </label>
              )}
            </For>
          </div>
        </fieldset>
      </Show>
      <label class={`flex flex-col gap-2 ${props.spin ? "" : "hidden"}`}>
        <span class="text-[15px] font-semibold text-ink-2">{t("Your name (optional)")}</span>
        <input
          type="text"
          maxLength={25}
          value={props.name}
          onInput={(event) => props.onName(event.currentTarget.value)}
          placeholder={t("Shown with your spin")}
          autocomplete="nickname"
          class={inputClass}
        />
      </label>
      <MessageField
        message={message()}
        onMessage={setMessage}
        placeholder={t("Land on the big one!")}
      />
      <Show when={props.spin} keyed>
        {(spin) => (
          <div class="flex w-full flex-col gap-4 border-t border-line pt-6">
            <p class="flex flex-col items-center gap-1 text-center">
              <span class="font-title text-sm font-semibold tracking-wider text-muted uppercase">
                {t("Your tip · {spins}", { spins: spinsText(choice().spins) })}
              </span>
              <span class="font-display text-4xl font-semibold">
                {formatPrice({ price: choice().price, currency: spin.currency })}
              </span>
            </p>
            <ResultLine result={result()} />
            <PayButtons
              payment={spin.payment}
              options={options}
              disabled={busy() || !props.wheel}
              resetKey={`${choice().spins}\n${choice().price}\n${props.name}\n${message()}`}
            />
            <p class="text-center text-sm text-muted">
              {tRich("Your tip goes straight to the streamer. {terms}", { terms: <TermsLink /> })}
            </p>
          </div>
        )}
      </Show>
      <Show when={props.subSpins} keyed>
        {(tiers) => (
          <TwitchBox
            kind="spin"
            tiers={tiers}
            allowance={props.allowance}
            signedIn={props.viewer !== null}
            paid={props.spin !== null}
            disabled={busy() || !props.wheel}
            result={result()}
            onSignIn={props.onSignIn}
            onSignOut={props.onSignOut}
            onRequest={() => void spinAsSubscriber()}
          />
        )}
      </Show>
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
      <div class="flex flex-col items-center gap-4 text-center">
        <h2 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-5xl">
          {props.streamer ? t("Tip {name}", { name: props.streamer }) : t("Tip the streamer")}
        </h2>
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          {enabled() === 0
            ? props.streamer
              ? t("{name} isn't taking tips right now.", { name: props.streamer })
              : t("The streamer isn't taking tips right now.")
            : enabled() > 1
              ? props.streamer
                ? t("Every tip goes straight to {name}. Pick how you want to send yours.", {
                    name: props.streamer,
                  })
                : t("Every tip goes straight to the streamer. Pick how you want to send yours.")
              : props.streamer
                ? t("Every tip goes straight to {name}.", { name: props.streamer })
                : t("Every tip goes straight to the streamer.")}
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
                  <span class="font-title text-xl font-semibold text-ink">{way.title}</span>
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
  const [message, setMessage] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(null);
  const [paywall, setPaywall] = createSignal<PublicPaywall | null>(null);
  const [subSongs, setSubSongs] = createSignal<Tiers | null>(null, { equals: sameData });
  const [maxSongMinutes, setMaxSongMinutes] = createSignal<number | null>(null);
  const [subSpins, setSubSpins] = createSignal<Tiers | null>(null, { equals: sameData });
  const [viewer, setViewer] = createSignal<string | null>(null);
  const [songAllowance, setSongAllowance] = createSignal<Allowance | null>(null);
  const [spinAllowance, setSpinAllowance] = createSignal<Allowance | null>(null);
  const [spinSignInFailed, setSpinSignInFailed] = createSignal(false);
  const [tips, setTips] = createSignal<PublicTips | null>(null);
  const [spin, setSpin] = createSignal<PublicSpin | null>(null);
  const [wheelOfLuck, setWheelOfLuck] = createSignal<Wheel | null>(null, {
    equals: (a, b) => JSON.stringify(a) === JSON.stringify(b),
  });
  const [streamerConnected, setStreamerConnected] = createSignal(false);
  const [profile, setProfile] = createSignal<Profile | null>(null);
  const [dashboard, setDashboard] = createSignal<DashboardLink | null>(null);
  const [styled, setStyled] = createSignal(false);
  const [section, setSection] = createSignal<Section | null>(
    (["tip", "spin", "songs"] as const).find((name) => location.hash === `#${name}`) ?? null,
  );
  const sections = (): Section[] => [
    ...(tipJar() ? (["tip"] as const) : []),
    ...(spinnable() ? (["spin"] as const) : []),
    ...(songs() ? (["songs"] as const) : []),
  ];
  const songs = () => payable() !== null || subSongs() !== null;
  const closed = () => {
    const current = status();
    return typeof current !== "string" && !current.songRequests;
  };
  const shown = () => {
    const current = section();
    return current && sections().includes(current) ? current : sections()[0];
  };

  const ways = (): Way[] => {
    return [
      {
        section: "tip",
        icon: "heart",
        title: t("Just a tip"),
        text: t("Send a tip with a message if you like."),
        enabled: tipJar() !== null,
      },
      {
        section: "spin",
        icon: "wheel",
        title: t("Spin the wheel"),
        text: wheel()
          ? t("Your tip spins the wheel live on stream.")
          : t("Twitch subscribers spin the wheel live on stream."),
        enabled: spinnable(),
      },
      {
        section: "songs",
        icon: "music",
        title: t("Request a song"),
        text: payable()
          ? t("Your tip puts a song in the queue.")
          : t("Twitch subscribers add songs to the queue."),
        enabled: songs(),
      },
    ];
  };

  const changeSection = (next: Section) => {
    setSection(next);
    history.replaceState(null, "", `${location.pathname}${location.search}#${next}`);
  };
  let form: HTMLFormElement | undefined;
  const withPayment = <T extends PublicPayment>(current: T | null) => {
    const payment = current && paymentOf(current);
    return current && payment ? { ...current, payment } : null;
  };
  const payable = createMemo(() => withPayment(paywall()), null, { equals: sameData });
  const tipJar = createMemo(() => withPayment(tips()), null, { equals: sameData });
  const wheel = createMemo(() => withPayment(spin()), null, { equals: sameData });
  const spinnable = () => wheel() !== null || subSpins() !== null;
  let orderError: unknown = null;
  const [cheer, setCheer] = createSignal<{ title: string; text: string } | null>(null);

  const celebrate = (text: string, title = t("Thanks for the tip!")) => setCheer({ title, text });

  const refreshPaywall = async () => {
    const current = id();
    if (current) {
      try {
        const found = await streamerSongs(current);
        setPaywall(found.paywall);
        setSubSongs(found.subSongs);
        setMaxSongMinutes(found.maxSongMinutes);
      } catch {}
    }
  };

  const signOutViewer = () => {
    const current = id();
    if (current) {
      saveViewer(current, null);
    }
    setViewer(null);
    setSongAllowance(null);
    setSpinAllowance(null);
  };

  const refreshAllowances = async () => {
    const current = id();
    const session = viewer();
    if (!current || !session) {
      return;
    }
    try {
      await Promise.all([
        subSongs() && fetchSongAllowance(current, session).then(setSongAllowance),
        subSpins() && fetchSpinAllowance(current, session).then(setSpinAllowance),
      ]);
    } catch (error) {
      if (error instanceof BackendError && error.status === 401) {
        signOutViewer();
      }
    }
  };

  const allowancesMissing = () =>
    (subSongs() !== null && songAllowance() === null) ||
    (subSpins() !== null && spinAllowance() === null);

  createEffect(() => {
    if (viewer() && allowancesMissing()) {
      void refreshAllowances();
    }
  });

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
        setSubSpins(found.subSpins);
        setWheelOfLuck(found.wheel);
        setStreamerConnected(found.connected);
      } catch {}
    }
  };

  const spinError = (error: unknown) => {
    if (error instanceof BackendError && [400, 404, 409].includes(error.status)) {
      void refreshSpin();
    }
  };

  const requested = (title = t("Thanks for the tip!")) => {
    setCheer({ title, text: t("Your song will be added to the queue in a moment.") });
    setSong("");
    setMessage("");
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
    setResult({ text: errorText(error) });
    if (error instanceof BackendError && error.status === 404) {
      setStatus("offline");
    }
    if (error instanceof BackendError && [400, 402].includes(error.status)) {
      void refreshPaywall();
    }
    if (error instanceof BackendError && error.status === 409) {
      void refresh();
    }
  };

  const requestAsSubscriber = async () => {
    const current = id();
    const session = viewer();
    if (!current || !session || !start()) {
      return;
    }
    setBusy(true);
    try {
      const answer = await requestSubscriberSong(current, session, song().trim(), message().trim());
      setSongAllowance((found) => found && { ...found, left: answer.left });
      requested(t("Enjoy the song!"));
    } catch (error) {
      showError(error);
      if (error instanceof BackendError && error.status === 401) {
        signOutViewer();
      } else {
        void refreshAllowances();
      }
    } finally {
      setBusy(false);
    }
  };

  const payOptions: PayOptions = {
    start,
    createOrder: async (provider) => {
      orderError = null;
      try {
        return await orderSong(
          id() ?? "",
          song().trim(),
          name().trim(),
          message().trim(),
          provider,
        );
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    complete: async (orderId) => {
      setBusy(true);
      setResult(null);
      try {
        await captureSong(id() ?? "", orderId);
        requested();
      } catch (error) {
        showError(error);
      } finally {
        setBusy(false);
      }
    },
    onCancel: () => {
      setResult({ text: t("Payment cancelled. No tip was sent and no song was added.") });
    },
    onError: (provider) => {
      if (orderError) {
        showError(orderError);
      } else {
        setResult({
          text: t("{provider} could not complete the payment. Try again.", {
            provider: providerName(provider),
          }),
        });
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
      const signIn = takeViewerSignIn();
      if (signIn.viewer) {
        saveViewer(current, signIn.viewer);
      }
      setViewer(readViewer(current));
      if (signIn.viewer || signIn.failed) {
        setSection(signIn.section);
      }
      if (signIn.failed && signIn.section === "spin") {
        setSpinSignInFailed(true);
      } else if (signIn.failed) {
        setResult({ text: t("Signing in with Twitch failed. Try again.") });
      }
      Promise.all([streamerProfile(current), signedInAccount()])
        .then(([found, account]) => {
          batch(() => {
            setDashboard(dashboardLink(found, account));
            setProfile(found);
          });
          loadFonts(found.font ? [found.font] : []);
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
      if (viewer() && allowancesMissing()) {
        void refreshAllowances();
      }
    }, 30_000);
    onCleanup(() => clearInterval(timer));
  });

  return (
    <div
      {...accentAttributes(profile()?.accent)}
      data-font={profile()?.font ?? undefined}
      class="isolate flex min-h-svh flex-col"
    >
      <main class="flex-1">
        <StreamerHeader
          id={id()}
          profile={profile()}
          unknown={status() === "unknown"}
          dashboard={dashboard()}
        />
        <Show
          when={status() !== "unknown"}
          fallback={
            <p class="mx-auto max-w-[720px] px-5 text-center text-lg text-ink-2 sm:px-8">
              {t("There is no page with this name.")}
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
              <Show when={status() === "offline"}>
                <Notice>
                  {t(
                    "Song requests are not available right now. Try again when the streamer is live.",
                  )}
                </Notice>
              </Show>
              <Show when={closed()}>
                <Notice>{t("The streamer is not taking song requests right now.")}</Notice>
              </Show>
              <label class="flex flex-col gap-2">
                <span class="text-[15px] font-semibold text-ink-2">
                  {t("Apple Music song link")}
                </span>
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
                  {tRich("Find the song on {appleMusic}, then use Share → Copy Link.", {
                    appleMusic: (
                      <a
                        href={links.appleMusic}
                        target="_blank"
                        rel="noopener"
                        class="text-leaf hover:text-leaf-bright"
                      >
                        Apple Music
                      </a>
                    ),
                  })}
                </span>
                <Show when={maxSongMinutes()}>
                  {(minutes) => (
                    <span class="text-[15px] text-muted">
                      {t("Songs can be at most {minutes} minutes long.", { minutes: minutes() })}
                    </span>
                  )}
                </Show>
              </label>
              <label class={`flex flex-col gap-2 ${payable() ? "" : "hidden"}`}>
                <span class="text-[15px] font-semibold text-ink-2">
                  {t("Your name (optional)")}
                </span>
                <input
                  type="text"
                  maxLength={25}
                  value={name()}
                  onInput={(event) => setName(event.currentTarget.value)}
                  placeholder={t("Shown next to the song")}
                  autocomplete="nickname"
                  class={inputClass}
                />
              </label>
              <MessageField
                message={message()}
                onMessage={setMessage}
                placeholder={t("This one is for the drive home!")}
              />
              <div class="flex flex-wrap items-center gap-4">
                <Show when={payable()} keyed>
                  {(current) => (
                    <div class="flex w-full flex-col gap-4 border-t border-line pt-6">
                      <p class="flex flex-col items-center gap-1 text-center">
                        <span class="font-title text-sm font-semibold tracking-wider text-muted uppercase">
                          {t("Your tip")}
                        </span>
                        <span class="text-ink-2">
                          {tRich("{price} per 5 minutes", {
                            price: (
                              <span class="font-display text-4xl font-semibold text-ink">
                                {formatPrice(current)}
                              </span>
                            ),
                          })}
                        </span>
                      </p>
                      <ResultLine result={result()} />
                      <PayButtons
                        payment={current.payment}
                        options={payOptions}
                        disabled={
                          busy() || typeof status() === "string" || closed() || !song().trim()
                        }
                        resetKey={`${song()}\n${name()}\n${message()}`}
                      />
                      <p class="text-center text-sm text-muted">
                        {tRich(
                          "Every started 5 minutes of the song counts, and it always plays in full. " +
                            "Your tip goes straight to the streamer. {terms}",
                          { terms: <TermsLink /> },
                        )}
                      </p>
                    </div>
                  )}
                </Show>
                <Show when={subSongs()} keyed>
                  {(current) => (
                    <TwitchBox
                      kind="song"
                      tiers={current}
                      allowance={songAllowance()}
                      signedIn={viewer() !== null}
                      paid={payable() !== null}
                      disabled={
                        busy() || typeof status() === "string" || closed() || !song().trim()
                      }
                      result={result()}
                      onSignIn={() => signInAsViewer(id() ?? "", "songs")}
                      onSignOut={signOutViewer}
                      onRequest={() => void requestAsSubscriber()}
                    />
                  )}
                </Show>
              </div>
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
                    onPaid={celebrate}
                  />
                </div>
              )}
            </Show>
            <Show when={wheel() ?? subSpins()} keyed>
              {(_current) => (
                <div class={shown() === "spin" ? "" : "hidden"}>
                  <SpinWheel
                    id={id() ?? ""}
                    spin={wheel()}
                    subSpins={subSpins()}
                    allowance={spinAllowance()}
                    viewer={viewer()}
                    wheel={wheelOfLuck()}
                    connected={streamerConnected()}
                    name={name()}
                    onName={setName}
                    onError={spinError}
                    onPaid={celebrate}
                    onSignIn={() => signInAsViewer(id() ?? "", "spin")}
                    onSignOut={signOutViewer}
                    onAllowance={() => void refreshAllowances()}
                    signInFailed={spinSignInFailed()}
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
      <Show when={styled() || status() === "unknown"}>
        <PoweredBy />
      </Show>
      <Show when={cheer()} keyed>
        {(current) => (
          <CheerCard title={current.title} text={current.text} onClose={() => setCheer(null)} />
        )}
      </Show>
    </div>
  );
}

export default function Page(props: { twitch?: string }) {
  const id = new URLSearchParams(location.search).get("streamer");
  const twitch = props.twitch ?? null;

  return <Viewer id={id} twitch={twitch} />;
}
