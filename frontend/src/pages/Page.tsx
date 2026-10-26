import { loadCoreSdkScript, type OneTimePaymentSession } from "@paypal/paypal-js/sdk-v6";
import type { Stripe, StripeElements, StripePaymentElement } from "@stripe/stripe-js";
import { loadStripe } from "@stripe/stripe-js/pure";
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
import { BackendError, moblinIconUrl } from "../data/backend";
import { fetchAccount, readSession, type Account } from "../data/account";
import Icon from "../components/Icon";
import { fireConfetti } from "../confetti";
import { poll } from "../poll";
import {
  cardClass,
  CloseNotice,
  ghostButtonClass,
  inputClass,
  Notice,
} from "../components/PageParts";
import { pages } from "../data/links";
import {
  accentAttributes,
  captureOrder,
  channelText,
  channelUrl,
  formatPrice,
  fetchAllowance,
  hundredths,
  identityOf,
  idOfLogin,
  loadFonts,
  orderEffect,
  orderSpin,
  orderTip,
  pageImageUrl,
  platformNames,
  platforms,
  providerName,
  spinsText,
  requestSubscriberEffect,
  requestSubscriberSpin,
  streamerEffects,
  streamerProfile,
  streamerSpin,
  streamerTips,
  tierCount,
  tierNumbers,
  viewerSignInUrl,
  type Allowance,
  type EffectOption,
  type Perk,
  type Identity,
  type Order,
  type PayPalConfig,
  type Platform,
  type Profile,
  type Provider,
  type PublicEffects,
  type PublicPayment,
  type PublicSpin,
  type PublicTips,
  type StripeConfig,
  type Tiers,
  type Wheel,
} from "../data/remoteControl";
import { effectCredit, effectName, playEffect, playingEffect, stopEffect } from "../data/effects";
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
    return section === "tip" || section === "effect" ? section : "spin";
  } catch {
    return "spin";
  }
}

function takeViewerSignIn(): { viewer: string | null; failed: string | null; section: Section } {
  const hash = location.hash.slice(1);
  const params = new URLSearchParams(hash);
  const viewer = params.get("viewer");
  const failedWith = params.get("viewerSignInFailed");
  const failed = params.has("viewerSignInFailed")
    ? t("Signing in with {platform} failed. Try again.", {
        platform: platformNames[failedWith === "kick" ? "kick" : "twitch"],
      })
    : hash === "viewerCheckFailed"
      ? t("Couldn't check your Kick subscription, try again.")
      : null;
  const section = viewer || failed ? takeSection() : "spin";
  if (viewer || failed) {
    history.replaceState(null, "", `${location.pathname}${location.search}#${section}`);
  }
  return { viewer, failed, section };
}

const draftKey = "viewerSignInDraft";

type Draft = {
  page?: string;
  name?: string;
  tip?: { choice: number | "other"; other: string; message: string };
  spin?: { spins: number; price: number; message: string };
  effect?: { effect: string | null; message: string };
};

function takeDraft(): Draft {
  try {
    const saved = sessionStorage.getItem(draftKey);
    sessionStorage.removeItem(draftKey);
    return saved ? JSON.parse(saved) : {};
  } catch {
    return {};
  }
}

const draft = takeDraft();

const restored = (id: string): Draft => (draft.page === id ? draft : {});

function signInAsViewer(platform: Platform, id: string, section: Section) {
  try {
    sessionStorage.setItem(sectionKey, section);
    sessionStorage.setItem(draftKey, JSON.stringify({ ...draft, page: id }));
  } catch {}
  location.href = viewerSignInUrl(platform, id);
}

const platformIconClass = (platform: Platform) =>
  platform === "twitch" ? "size-5 text-twitch" : "size-5 text-kick";

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

type PayOptions = {
  start: () => boolean;
  createOrder: (provider: Provider) => Promise<Order>;
  complete: (orderId: string) => Promise<void>;
  onCancel: () => void;
  onError: (provider: Provider) => void;
};

function payOptions(props: {
  start: () => boolean;
  order: (provider: Provider) => Promise<Order>;
  capture: (orderId: string) => Promise<unknown>;
  paid: () => void;
  cancelled: string;
  showError: (error: unknown) => void;
  setBusy: (busy: boolean) => void;
  setResult: (result: Result | null) => void;
}): PayOptions {
  let orderError: unknown = null;
  return {
    start: props.start,
    createOrder: async (provider) => {
      orderError = null;
      try {
        return await props.order(provider);
      } catch (error) {
        orderError = error;
        throw error;
      }
    },
    complete: async (orderId) => {
      props.setBusy(true);
      props.setResult(null);
      try {
        await props.capture(orderId);
        props.paid();
      } catch (error) {
        props.showError(error);
      } finally {
        props.setBusy(false);
      }
    },
    onCancel: () => props.setResult({ text: props.cancelled }),
    onError: (provider) => {
      if (orderError) {
        props.showError(orderError);
      } else {
        props.setResult({
          text: t("{provider} could not complete the tip. Try again.", {
            provider: providerName(provider),
          }),
        });
      }
    },
  };
}

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
          {t("PayPal sandbox. This is a test tip and no real money is moved.")}
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
        setCardError(error.message ?? t("The tip did not go through. Try another card."));
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
            <Show when={cardError()}>
              {(error) => (
                <Notice error onClose={() => setCardError(null)}>
                  {error()}
                </Notice>
              )}
            </Show>
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
          {t("Stripe test mode. This is a test tip and no real money is moved.")}
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

function CheerCard(props: {
  title: string;
  text: string;
  streamer: Streamer | null;
  onClose: () => void;
}) {
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
        <Show
          when={props.streamer}
          fallback={
            <img
              src={asset("logos/logo-party.png")}
              alt=""
              class="absolute -top-20 left-1/2 h-40 w-auto -translate-x-1/2 drop-shadow-xl"
            />
          }
        >
          {(streamer) => (
            <Avatar
              streamer={streamer()}
              class="absolute -top-16 left-1/2 size-32 -translate-x-1/2 text-5xl shadow-xl"
            />
          )}
        </Show>
        <span class="font-display text-3xl font-semibold text-ink">{props.title}</span>
        <span class="text-lg text-ink-2">{props.text}</span>
      </div>
    </div>
  );
}

function SubscriberBox(props: {
  perk: Perk;
  tiers: Tiers;
  platforms: Platform[];
  allowance: Allowance | null;
  signedIn: boolean;
  paid: boolean;
  disabled: boolean;
  result: Result | null;
  onCloseResult: () => void;
  onSignIn: (platform: Platform) => void;
  onSignOut: () => void;
  onRequest: () => void;
}) {
  const tiers = () =>
    tierNumbers
      .map((tier) => ({ tier, count: tierCount(props.tiers, tier) }))
      .filter((tier) => tier.count > 0);
  const signOut = () => (
    <button type="button" class="underline hover:text-ink" onClick={() => props.onSignOut()}>
      {t("Sign out")}
    </button>
  );
  const offer = () => {
    const lines: string[] = [];
    if (props.platforms.includes("twitch")) {
      const values = {
        counts: tiers()
          .map((tier) => tier.count)
          .join(" / "),
        tiers: tiers()
          .map((tier) => tier.tier)
          .join(" / "),
      };
      lines.push(
        props.perk === "spin"
          ? t("{counts} spins a month for tier {tiers}", values)
          : t("{counts} effects a month for tier {tiers}", values),
      );
    }
    if (props.platforms.includes("kick") && props.tiers.tier1 > 0) {
      const values = { count: props.tiers.tier1 };
      lines.push(
        props.perk === "spin"
          ? t("{count} spins a month for Kick subscribers", values)
          : t("{count} effects a month for Kick subscribers", values),
      );
    }
    return (
      <For each={lines}>
        {(line, index) => (
          <>
            <Show when={index() > 0}>
              <br />
            </Show>
            {line}
          </>
        )}
      </For>
    );
  };

  return (
    <div class="flex w-full flex-col gap-4 border-t border-line pt-6">
      <p class="flex flex-col items-center gap-1 text-center">
        <span class="text-ink-2">
          <Show when={props.allowance} fallback={offer()}>
            {(current) => (
              <Switch fallback={offer()}>
                <Match when={current().tier === null}>
                  {current().platform === "kick"
                    ? t("You are not subscribed to this streamer on Kick.")
                    : t("You are not subscribed to this streamer on Twitch.")}
                </Match>
                <Match when={current().allowance === 0 && current().platform === "kick"}>
                  {props.perk === "spin"
                    ? t("Kick subscribers can't spin the wheel here.")
                    : t("Kick subscribers can't play effects here.")}
                </Match>
                <Match when={current().allowance === 0}>
                  {props.perk === "spin"
                    ? t("Tier {tier} subscribers can't spin the wheel here.", {
                        tier: current().tier ?? 0,
                      })
                    : t("Tier {tier} subscribers can't play effects here.", {
                        tier: current().tier ?? 0,
                      })}
                </Match>
                <Match when={current().left === 0}>
                  {props.perk === "spin"
                    ? t(
                        "You have used all your spins this month. New ones come on the 1st of the month.",
                      )
                    : t(
                        "You have used all your effects this month. New ones come on the 1st of the month.",
                      )}
                </Match>
                <Match when={current().left > 0}>
                  {props.perk === "spin"
                    ? t("{left} of {allowance} spins left this month", {
                        left: current().left,
                        allowance: current().allowance,
                      })
                    : t("{left} of {allowance} effects left this month", {
                        left: current().left,
                        allowance: current().allowance,
                      })}
                </Match>
              </Switch>
            )}
          </Show>
        </span>
      </p>
      <Show when={!props.paid}>
        <ResultLine result={props.result} onClose={props.onCloseResult} />
      </Show>
      <Show when={!props.signedIn}>
        <div class="flex flex-wrap justify-center gap-3">
          <For each={props.platforms}>
            {(platform) => (
              <button
                type="button"
                class={ghostButtonClass}
                onClick={() => props.onSignIn(platform)}
              >
                <Icon name={platform} class={platformIconClass(platform)} />
                {t("Sign in with {platform}", { platform: platformNames[platform] })}
              </button>
            )}
          </For>
        </div>
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
                <Icon name={current().platform} class={platformIconClass(current().platform)} />
                {props.perk === "spin" ? t("Spin the wheel") : t("Play the effect")}
              </button>
            </Show>
            <p class="text-center text-sm text-muted">
              {current().tier === null
                ? tRich("Signed in as {name}. {signOut} and in again if your sub changed.", {
                    name: current().name,
                    signOut: signOut(),
                  })
                : current().platform === "kick"
                  ? tRich(
                      "Signed in as {name}, a Kick subscriber. {signOut} and in again if your sub changed.",
                      { name: current().name, signOut: signOut() },
                    )
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

function ResultLine(props: { result: Result | null; onClose: () => void }) {
  return (
    <Show when={props.result}>
      {(result) => (
        <p
          role="alert"
          class="flex items-start gap-3 rounded-2xl border-[1.5px] border-danger-line bg-danger-bg px-4 py-3 text-[17px] font-semibold text-danger-ink"
        >
          <Icon name="warning" class="mt-0.5 size-5 shrink-0" />
          <span>{result().text}</span>
          <CloseNotice onClose={props.onClose} />
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

function NameField(props: {
  name: string;
  onName: (name: string) => void;
  placeholder: string;
  hidden?: boolean;
}) {
  return (
    <label class={`flex flex-col gap-2 ${props.hidden ? "hidden" : ""}`}>
      <span class="text-[15px] font-semibold text-ink-2">{t("Your name (optional)")}</span>
      <input
        type="text"
        maxLength={25}
        value={props.name}
        onInput={(event) => props.onName(event.currentTarget.value)}
        placeholder={props.placeholder}
        autocomplete="nickname"
        class={inputClass}
      />
    </label>
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
  const saved = restored(props.id).tip;
  const [choice, setChoice] = createSignal<number | "other">(
    saved && (saved.choice === "other" || props.tips.amounts.includes(saved.choice))
      ? saved.choice
      : props.tips.amounts[Math.min(1, props.tips.amounts.length - 1)],
  );
  const [other, setOther] = createSignal(saved?.other ?? "");
  const [message, setMessage] = createSignal(saved?.message ?? "");
  createEffect(() => {
    draft.tip = { choice: choice(), other: other(), message: message() };
  });
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(null);
  let form: HTMLFormElement | undefined;

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

  const options = payOptions({
    start: () => {
      setResult(null);
      if (busy() || amount() === null || !form?.reportValidity()) {
        return false;
      }
      saveName(props.name.trim());
      return true;
    },
    order: (provider) =>
      orderTip(props.id, amount() ?? 0, props.name.trim(), message().trim(), provider),
    capture: (orderId) => captureOrder("tip", props.id, orderId),
    paid: () => {
      props.onPaid(t("Your tip is on its way to the streamer."));
      setMessage("");
    },
    cancelled: t("Tip cancelled."),
    showError,
    setBusy,
    setResult,
  });

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
      <NameField
        name={props.name}
        onName={props.onName}
        placeholder={t("Shown with your message")}
      />
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
        <ResultLine result={result()} onClose={() => setResult(null)} />
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

function EffectJar(props: {
  id: string;
  options: EffectOption[];
  effects: (PublicEffects & { payment: Payment }) | null;
  subEffects: Tiers | null;
  platforms: Platform[];
  allowance: Allowance | null;
  viewer: string | null;
  name: string;
  onName: (name: string) => void;
  onError: (error: unknown) => void;
  onPaid: (text: string, title?: string) => void;
  onSignIn: (platform: Platform) => void;
  onSignOut: () => void;
  onAllowance: () => void;
  signInFailed: string | null;
}) {
  const saved = restored(props.id).effect;
  const [chosen, setChosen] = createSignal<string | null>(saved?.effect ?? null);
  const [message, setMessage] = createSignal(saved?.message ?? "");
  createEffect(() => {
    draft.effect = { effect: chosen(), message: message() };
  });
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(
    props.signInFailed ? { text: props.signInFailed } : null,
  );
  let form: HTMLFormElement | undefined;

  const effect = () => {
    const current = chosen();
    return props.options.some((option) => option.id === current) ? current : null;
  };

  const showError = (error: unknown) => {
    setResult({ text: errorText(error) });
    props.onError(error);
  };

  const options = payOptions({
    start: () => {
      setResult(null);
      if (busy() || effect() === null || !form?.reportValidity()) {
        return false;
      }
      saveName(props.name.trim());
      return true;
    },
    order: (provider) =>
      orderEffect(
        props.id,
        effect() ?? "",
        props.effects?.price ?? 0,
        props.name.trim(),
        message().trim(),
        provider,
      ),
    capture: (orderId) => captureOrder("effect", props.id, orderId),
    paid: () => {
      props.onPaid(t("Your effect is on its way to the streamer."));
      setMessage("");
    },
    cancelled: t("Tip cancelled. No effect was played."),
    showError,
    setBusy,
    setResult,
  });

  const playAsSubscriber = async () => {
    const current = effect();
    if (busy() || !props.viewer || current === null) {
      return;
    }
    setBusy(true);
    setResult(null);
    try {
      await requestSubscriberEffect(props.id, props.viewer, current, message().trim());
      props.onPaid(t("Your effect is on its way to the streamer."), t("Enjoy the effect!"));
      setMessage("");
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
      props.onAllowance();
    }
  };

  return (
    <form
      ref={(element) => (form = element)}
      class={cardClass}
      onSubmit={(event) => event.preventDefault()}
    >
      <fieldset class="flex flex-col gap-3">
        <legend class="mb-2 text-[15px] font-semibold text-ink-2">{t("Sound")}</legend>
        <div class="divide-y divide-line rounded-2xl border-[1.5px] border-line bg-well">
          <For each={props.options}>
            {(option) => (
              <div class="flex items-center first:rounded-t-[14px] last:rounded-b-[14px] has-[:checked]:bg-moss/15">
                <label class="flex min-h-14 flex-1 cursor-pointer items-center gap-3 px-4 font-display text-lg font-semibold text-ink has-[:focus-visible]:outline-2 has-[:focus-visible]:-outline-offset-2 has-[:focus-visible]:outline-ink">
                  <input
                    type="radio"
                    name="effect"
                    checked={effect() === option.id}
                    onChange={() => setChosen(option.id)}
                    class="peer sr-only"
                  />
                  <span class="flex size-5 shrink-0 items-center justify-center rounded-full border-2 border-muted peer-checked:border-moss peer-checked:bg-moss peer-checked:[&>span]:bg-white">
                    <span class="size-2 rounded-full" />
                  </span>
                  {effectName(option.id)}
                </label>
                <Show when={effectCredit(option.id)}>
                  {(credit) => <SoundInfo name={effectName(option.id)} credit={credit()} />}
                </Show>
                <button
                  type="button"
                  class="mr-2 flex size-10 shrink-0 cursor-pointer items-center justify-center rounded-full border-[1.5px] border-line bg-card text-ink transition-colors hover:border-leaf"
                  aria-label={
                    playingEffect() === option.id
                      ? t("Stop {name}", { name: effectName(option.id) })
                      : t("Play {name}", { name: effectName(option.id) })
                  }
                  onClick={() =>
                    playingEffect() === option.id ? stopEffect() : playEffect(option.id)
                  }
                >
                  <Icon name={playingEffect() === option.id ? "pause" : "play"} class="size-5" />
                </button>
              </div>
            )}
          </For>
        </div>
      </fieldset>
      <NameField
        name={props.name}
        onName={props.onName}
        placeholder={t("Shown with your message")}
        hidden={!props.effects}
      />
      <MessageField
        message={message()}
        onMessage={setMessage}
        placeholder={t("Love the stream!")}
      />
      <Show when={props.effects} keyed>
        {(effects) => (
          <div class="flex w-full flex-col gap-4 border-t border-line pt-6">
            <p class="flex flex-col items-center gap-1 text-center">
              <span class="font-title text-sm font-semibold tracking-wider text-muted uppercase">
                {t("Your tip")}
              </span>
              <span class="font-display text-4xl font-semibold">
                {formatPrice({ price: effects.price, currency: effects.currency })}
              </span>
              <span class="text-ink-2">
                {effect() === null ? t("Pick an effect") : effectName(effect()!)}
              </span>
            </p>
            <ResultLine result={result()} onClose={() => setResult(null)} />
            <PayButtons
              payment={effects.payment}
              options={options}
              disabled={busy() || effect() === null}
              resetKey={`${effect()}\n${effects.price}\n${props.name}\n${message()}`}
            />
            <p class="text-center text-sm text-muted">
              {tRich("Your tip goes straight to the streamer. {terms}", { terms: <TermsLink /> })}
            </p>
          </div>
        )}
      </Show>
      <Show when={props.subEffects} keyed>
        {(tiers) => (
          <SubscriberBox
            perk="effect"
            tiers={tiers}
            platforms={props.platforms}
            allowance={props.allowance}
            signedIn={props.viewer !== null}
            paid={props.effects !== null}
            disabled={busy() || effect() === null}
            result={result()}
            onCloseResult={() => setResult(null)}
            onSignIn={props.onSignIn}
            onSignOut={props.onSignOut}
            onRequest={() => void playAsSubscriber()}
          />
        )}
      </Show>
    </form>
  );
}

function SoundInfo(props: {
  name: string;
  credit: { title: string; author: string; url: string };
}) {
  const [open, setOpen] = createSignal(false);
  return (
    <span
      class="group relative mr-1"
      onFocusOut={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          setOpen(false);
        }
      }}
    >
      <button
        type="button"
        aria-label={t("About {name}", { name: props.name })}
        aria-expanded={open()}
        class="flex size-10 cursor-pointer items-center justify-center rounded-full text-muted transition-colors hover:text-ink aria-expanded:text-ink"
        onClick={() => setOpen(!open())}
      >
        <Icon name="info" class="size-5" />
      </button>
      <span
        role="tooltip"
        class={`absolute right-0 bottom-full z-10 w-max max-w-[min(18rem,calc(100vw-5rem))] rounded-2xl border-[1.5px] border-line bg-card px-4 py-3 text-[14px] leading-snug text-ink-2 shadow-xl ${
          open() ? "block" : "hidden group-hover:block group-has-[:focus-visible]:block"
        }`}
      >
        <span class="block font-semibold text-ink">
          {t("{name} by {author}", { name: props.credit.title, author: props.credit.author })}
        </span>
        <span class="block">
          {tRich("From {freesound}, license CC0.", {
            freesound: (
              <a
                href={props.credit.url}
                target="_blank"
                rel="noopener"
                class="underline hover:text-ink"
              >
                Freesound
              </a>
            ),
          })}
        </span>
      </span>
    </span>
  );
}

type Streamer = Identity & { platform: Platform };

function Avatar(props: { streamer: Streamer; class: string; cutout?: boolean }) {
  const ring = () =>
    props.cutout ? "ring-4 ring-page" : "ring-4 ring-moss ring-offset-4 ring-offset-page";
  return (
    <Show
      when={props.streamer.avatar}
      fallback={
        <span
          class={`flex shrink-0 items-center justify-center rounded-full bg-moss font-display font-semibold text-white ${ring()} ${props.class}`}
        >
          {props.streamer.name.slice(0, 1).toUpperCase()}
        </span>
      }
    >
      {(avatar) => (
        <img
          src={avatar()}
          alt=""
          class={`shrink-0 rounded-full bg-ghost object-cover ${ring()} ${props.class}`}
        />
      )}
    </Show>
  );
}

function MoblinIcon(props: { icon: string | null; class: string }) {
  return (
    <Show when={props.icon}>
      {(icon) => (
        <img src={moblinIconUrl(icon())} alt="" class={`w-auto shrink-0 ${props.class}`} />
      )}
    </Show>
  );
}

function ChannelLink(props: { streamer: Streamer }) {
  return (
    <a
      href={channelUrl(props.streamer.platform, props.streamer.login)}
      target="_blank"
      rel="noopener"
      class="inline-flex max-w-full min-w-0 items-center gap-1.5 text-[17px] font-semibold text-ink-2 transition-colors hover:text-ink"
    >
      <Icon
        name={props.streamer.platform}
        class={`size-5 shrink-0 ${props.streamer.platform === "twitch" ? "text-twitch" : "text-kick"}`}
      />
      <span class="truncate">{channelText(props.streamer.platform, props.streamer.login)}</span>
    </a>
  );
}

function Greeting(props: { profile: Profile }) {
  return (
    <Show when={props.profile.message}>
      {(message) => (
        <p class="mx-auto max-w-[560px] text-center text-lg leading-relaxed whitespace-pre-line text-ink-2 text-pretty sm:text-xl">
          {message()}
        </p>
      )}
    </Show>
  );
}

type DashboardLink = { href: string; own: boolean };

function DashboardLink(props: { link: DashboardLink }) {
  return (
    <a
      href={props.link.href}
      class="inline-flex max-w-full min-w-0 items-center gap-1.5 text-[17px] font-semibold text-ink-2 transition-colors hover:text-ink"
    >
      <img src={asset("logos/logo-cloud.png")} alt="" class="h-5 w-auto shrink-0" />
      <span class="truncate">
        {props.link.own ? t("Open your dashboard") : t("Moderate on the dashboard")}
      </span>
    </a>
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

function dashboardLink(
  id: string,
  platform: Platform | null,
  account: Account | null,
): DashboardLink | null {
  if (!account) {
    return null;
  }
  if (account.settings.id === id) {
    return { href: asset(pages.dashboard), own: true };
  }
  const channel = account.moderating.find((channel) => channel.id === id);
  if (!channel) {
    return null;
  }
  const address = identityOf(channel, platform) ?? channel;
  return {
    href: asset(`${pages.dashboard}${address.platform}/${encodeURIComponent(address.login)}`),
    own: false,
  };
}

function StreamerHeader(props: {
  id: string | null;
  profile: Profile | null;
  platform: Platform | null;
  unknown: boolean;
  dashboard: DashboardLink | null;
}) {
  const shown = () => {
    const profile = props.profile;
    const streamer = profile && identityOf(profile, props.platform);
    return profile && streamer ? { profile, streamer } : null;
  };
  const banner = () => {
    const current = shown();
    return props.id && current?.profile.image
      ? { ...current, src: pageImageUrl(props.id, current.profile.image) }
      : null;
  };

  return (
    <header>
      <Show when={props.dashboard}>
        {(link) => (
          <div class="mx-auto flex max-w-[720px] justify-center px-5 pt-5 sm:px-8">
            <DashboardLink link={link()} />
          </div>
        )}
      </Show>
      <Show
        when={banner()}
        fallback={
          <div class="mx-auto flex max-w-[720px] flex-col items-center gap-4 px-5 pt-8 pb-4 text-center sm:px-8 lg:pt-12">
            <Show
              when={shown()}
              fallback={
                <Show when={props.unknown}>
                  <h1 class="text-[40px] leading-[1.05] font-bold sm:text-6xl">
                    {t("Page not found.")}
                  </h1>
                </Show>
              }
            >
              {(current) => (
                <>
                  <Avatar streamer={current().streamer} class="size-28 text-5xl sm:size-32" />
                  <ChannelLink streamer={current().streamer} />
                  <MoblinIcon icon={current().profile.icon} class="h-8" />
                  <Greeting profile={current().profile} />
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
              <div class="flex shrink-0 items-start gap-1">
                <Avatar
                  streamer={banner().streamer}
                  cutout
                  class="relative -mt-10 size-20 text-3xl sm:-mt-12 sm:size-24 sm:text-4xl"
                />
                <MoblinIcon
                  icon={banner().profile.icon}
                  class="relative -mt-3.5 ml-3 h-7 cutout sm:-mt-4 sm:h-8"
                />
              </div>
              <div class="mt-3 min-w-0">
                <ChannelLink streamer={banner().streamer} />
              </div>
            </div>
            <Show when={banner().profile.message}>
              <div class="mt-2 px-4 sm:px-6">
                <Greeting profile={banner().profile} />
              </div>
            </Show>
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
        <img src={asset("logos/logo-cloud.png")} alt="" class="h-5 w-auto" />
        Moblin Cloud
      </a>
    </footer>
  );
}

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
  platforms: Platform[];
  allowance: Allowance | null;
  viewer: string | null;
  wheel: Wheel | null;
  connected: boolean;
  name: string;
  onName: (name: string) => void;
  onError: (error: unknown) => void;
  onPaid: (text: string, title?: string) => void;
  onSignIn: (platform: Platform) => void;
  onSignOut: () => void;
  onAllowance: () => void;
  signInFailed: string | null;
}) {
  const saved = restored(props.id).spin;
  const [choice, setChoice] = createSignal(
    props.spin?.options.find(
      (option) => option.spins === saved?.spins && option.price === saved.price,
    ) ??
      props.spin?.options[0] ?? { spins: 1, price: 0 },
  );
  const [message, setMessage] = createSignal(saved?.message ?? "");
  createEffect(() => {
    draft.spin = { spins: choice().spins, price: choice().price, message: message() };
  });
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Result | null>(
    props.signInFailed ? { text: props.signInFailed } : null,
  );

  const showError = (error: unknown) => {
    setResult({ text: errorText(error) });
    props.onError(error);
  };

  const options = payOptions({
    start: () => {
      setResult(null);
      if (busy()) {
        return false;
      }
      saveName(props.name.trim());
      return true;
    },
    order: (provider) =>
      orderSpin(props.id, choice(), props.name.trim(), message().trim(), provider),
    capture: (orderId) => captureOrder("spin", props.id, orderId),
    paid: () => {
      props.onPaid(
        choice().spins === 1
          ? t("Your spin starts on stream in a moment.")
          : t("Your spins start on stream in a moment."),
      );
      setMessage("");
    },
    cancelled: t("Tip cancelled. The wheel was not spun."),
    showError,
    setBusy,
    setResult,
  });

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
      <NameField
        name={props.name}
        onName={props.onName}
        placeholder={t("Shown with your spin")}
        hidden={!props.spin}
      />
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
                {tRich("Your tip · {spins}", {
                  spins: <span class="font-body">{spinsText(choice().spins)}</span>,
                })}
              </span>
              <span class="font-display text-4xl font-semibold">
                {formatPrice({ price: choice().price, currency: spin.currency })}
              </span>
            </p>
            <ResultLine result={result()} onClose={() => setResult(null)} />
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
          <SubscriberBox
            perk="spin"
            tiers={tiers}
            platforms={props.platforms}
            allowance={props.allowance}
            signedIn={props.viewer !== null}
            paid={props.spin !== null}
            disabled={busy() || !props.wheel}
            result={result()}
            onCloseResult={() => setResult(null)}
            onSignIn={props.onSignIn}
            onSignOut={props.onSignOut}
            onRequest={() => void spinAsSubscriber()}
          />
        )}
      </Show>
    </form>
  );
}

type Section = "tip" | "spin" | "effect";

type Way = {
  section: Section;
  icon: "heart" | "wheel" | "sparkles";
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
      <div
        role="tablist"
        class={`grid gap-3 ${props.ways.length > 2 ? "sm:grid-cols-3" : "sm:grid-cols-2"}`}
      >
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

function Viewer(props: { id: string | null; platform: Platform | null; login: string | null }) {
  const [id, setId] = createSignal(props.id);
  const [unknown, setUnknown] = createSignal(false);
  const [name, setName] = createSignal(draft.name ?? readName());
  createEffect(() => {
    draft.name = name();
  });
  const [subSpins, setSubSpins] = createSignal<Tiers | null>(null, { equals: sameData });
  const [viewer, setViewer] = createSignal<string | null>(null);
  const [spinAllowance, setSpinAllowance] = createSignal<Allowance | null>(null);
  const [spinSignInFailed, setSpinSignInFailed] = createSignal<string | null>(null);
  const [subEffects, setSubEffects] = createSignal<Tiers | null>(null, { equals: sameData });
  const [effectOptions, setEffectOptions] = createSignal<EffectOption[]>([], { equals: sameData });
  const [effectAllowance, setEffectAllowance] = createSignal<Allowance | null>(null);
  const [effectSignInFailed, setEffectSignInFailed] = createSignal<string | null>(null);
  const [tips, setTips] = createSignal<PublicTips | null>(null);
  const [spin, setSpin] = createSignal<PublicSpin | null>(null);
  const [effects, setEffects] = createSignal<PublicEffects | null>(null);
  const [wheelOfLuck, setWheelOfLuck] = createSignal<Wheel | null>(null, { equals: sameData });
  const [streamerConnected, setStreamerConnected] = createSignal(false);
  const [profile, setProfile] = createSignal<Profile | null>(null);
  const streamerOf = () => {
    const current = profile();
    return current && identityOf(current, props.platform);
  };
  const streamerName = () => streamerOf()?.name;
  const subPlatforms = () => {
    const current = profile();
    return current ? platforms.filter((platform) => current[platform] !== null) : [];
  };
  const subscribersText = () => {
    const linked = subPlatforms();
    if (linked.includes("kick") && !linked.includes("twitch")) {
      return t("Kick subscribers spin the wheel live on stream.");
    }
    if (linked.includes("kick")) {
      return t("Twitch and Kick subscribers spin the wheel live on stream.");
    }
    return t("Twitch subscribers spin the wheel live on stream.");
  };
  const [dashboard, setDashboard] = createSignal<DashboardLink | null>(null);
  const [styled, setStyled] = createSignal(false);
  const [section, setSection] = createSignal<Section | null>(
    (["tip", "spin", "effect"] as const).find((name) => location.hash === `#${name}`) ?? null,
  );
  const sections = (): Section[] => [
    ...(tipJar() ? (["tip"] as const) : []),
    ...(spinnable() ? (["spin"] as const) : []),
    ...(effectsShown() ? (["effect"] as const) : []),
  ];
  const shown = () => {
    const current = section();
    return current && sections().includes(current) ? current : sections()[0];
  };

  const ways = (): Way[] => {
    return [
      {
        section: "tip",
        icon: "heart",
        title: t("Message"),
        text: t("Send a tip with a message if you like."),
        enabled: tipJar() !== null,
      },
      {
        section: "spin",
        icon: "wheel",
        title: t("Spin the wheel"),
        text: wheel() ? t("Your tip spins the wheel live on stream.") : subscribersText(),
        enabled: spinnable(),
      },
      ...(effectsShown()
        ? [
            {
              section: "effect" as const,
              icon: "sparkles" as const,
              title: t("Effect"),
              text: effectJar()
                ? t("Your tip plays a sound live on stream.")
                : t("Subscribers play a sound live on stream."),
              enabled: true,
            },
          ]
        : []),
    ];
  };

  const changeSection = (next: Section) => {
    setSection(next);
    history.replaceState(null, "", `${location.pathname}${location.search}#${next}`);
  };
  const withPayment = <T extends PublicPayment>(current: T | null) => {
    const payment = current && paymentOf(current);
    return current && payment ? { ...current, payment } : null;
  };
  const tipJar = createMemo(() => withPayment(tips()), null, { equals: sameData });
  const wheel = createMemo(() => withPayment(spin()), null, { equals: sameData });
  const effectJar = createMemo(() => withPayment(effects()), null, { equals: sameData });
  const effectsShown = () =>
    effectOptions().length > 0 && (effectJar() !== null || subEffects() !== null);
  const spinnable = () => wheel() !== null || subSpins() !== null;
  const [cheer, setCheer] = createSignal<{ title: string; text: string } | null>(null);

  const celebrate = (text: string, title = t("Thanks for the tip!")) => setCheer({ title, text });

  const signOutViewer = () => {
    const current = id();
    if (current) {
      saveViewer(current, null);
    }
    setViewer(null);
    setSpinAllowance(null);
    setEffectAllowance(null);
  };

  const refreshAllowances = async () => {
    const current = id();
    const session = viewer();
    if (!current || !session) {
      return;
    }
    try {
      if (subSpins()) {
        setSpinAllowance(await fetchAllowance(current, "spin", session));
      }
      if (subEffects()) {
        setEffectAllowance(await fetchAllowance(current, "effect", session));
      }
    } catch (error) {
      if (error instanceof BackendError && error.status === 401) {
        signOutViewer();
      }
    }
  };

  const allowancesMissing = () =>
    (subSpins() !== null && spinAllowance() === null) ||
    (subEffects() !== null && effectAllowance() === null);

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

  const refreshEffects = async () => {
    const current = id();
    if (current) {
      try {
        const found = await streamerEffects(current);
        setEffects(found.effects);
        setSubEffects(found.subEffects);
        setEffectOptions(found.options);
      } catch {}
    }
  };

  const effectError = (error: unknown) => {
    if (error instanceof BackendError && [400, 409].includes(error.status)) {
      void refreshEffects();
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

  onMount(async () => {
    if (props.platform && props.login) {
      try {
        setId(await idOfLogin(props.platform, props.login));
      } catch {
        setUnknown(true);
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
        setSpinSignInFailed(signIn.failed);
      }
      if (signIn.failed && signIn.section === "effect") {
        setEffectSignInFailed(signIn.failed);
      }
      Promise.all([streamerProfile(current), signedInAccount()])
        .then(([found, account]) => {
          batch(() => {
            setDashboard(dashboardLink(current, props.platform, account));
            setProfile(found);
          });
          loadFonts(found.font ? [found.font] : []);
          const streamer = identityOf(found, props.platform);
          if (streamer) {
            document.title = `${streamer.name} – Moblin`;
          }
        })
        .catch(() => {})
        .finally(() => setStyled(true));
    }
    void refreshTips();
    void refreshSpin();
    void refreshEffects();
    onCleanup(
      poll(() => {
        void refreshSpin();
        if (viewer() && allowancesMissing()) {
          void refreshAllowances();
        }
      }, 30_000),
    );
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
          platform={props.platform}
          unknown={unknown()}
          dashboard={dashboard()}
        />
        <Show
          when={!unknown()}
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
              streamer={streamerName()}
              ways={ways()}
              section={shown()}
              onSection={changeSection}
            />
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
                    platforms={subPlatforms()}
                    allowance={spinAllowance()}
                    viewer={viewer()}
                    wheel={wheelOfLuck()}
                    connected={streamerConnected()}
                    name={name()}
                    onName={setName}
                    onError={spinError}
                    onPaid={celebrate}
                    onSignIn={(platform) => signInAsViewer(platform, id() ?? "", "spin")}
                    onSignOut={signOutViewer}
                    onAllowance={() => void refreshAllowances()}
                    signInFailed={spinSignInFailed()}
                  />
                </div>
              )}
            </Show>
            <Show when={effectsShown()}>
              <div class={shown() === "effect" ? "" : "hidden"}>
                <EffectJar
                  id={id() ?? ""}
                  options={effectOptions()}
                  effects={effectJar()}
                  subEffects={subEffects()}
                  platforms={subPlatforms()}
                  allowance={effectAllowance()}
                  viewer={viewer()}
                  name={name()}
                  onName={setName}
                  onError={effectError}
                  onPaid={celebrate}
                  onSignIn={(platform) => signInAsViewer(platform, id() ?? "", "effect")}
                  onSignOut={signOutViewer}
                  onAllowance={() => void refreshAllowances()}
                  signInFailed={effectSignInFailed()}
                />
              </div>
            </Show>
          </div>
        </Show>
      </main>
      <Show when={styled() || unknown()}>
        <PoweredBy />
      </Show>
      <Show when={cheer()} keyed>
        {(current) => (
          <CheerCard
            title={current.title}
            text={current.text}
            streamer={streamerOf()}
            onClose={() => setCheer(null)}
          />
        )}
      </Show>
    </div>
  );
}

export default function Page(props: { platform?: Platform; login?: string }) {
  const id = new URLSearchParams(location.search).get("streamer");

  return <Viewer id={id} platform={props.platform ?? null} login={props.login ?? null} />;
}
