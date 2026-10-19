import { createSignal, For, Index, onCleanup, onMount, Show } from "solid-js";
import { encode } from "uqr";
import { asset } from "../asset";
import { createFlash } from "../flash";
import { copyText } from "../components/CopyButton";
import Icon, { type IconName } from "../components/Icon";
import {
  Band,
  buttonClass,
  cardClass,
  CloudHeader,
  ghostButtonClass,
  Hero,
  inputClass,
  Notice,
  SaveButton,
  Spinner,
  Value,
} from "../components/PageParts";
import {
  addDevice,
  checkPayPal,
  chooseDevices,
  checkStripe,
  connectPayPal,
  connectStripe,
  disconnectPayPal,
  disconnectStripe,
  fetchAccount,
  fetchRecent,
  createStreamer,
  newDeviceKey,
  readSession,
  removeDevice,
  removeImage,
  saveAccountSettings,
  saveSession,
  sendTest,
  signInUrl,
  signOutEverywhere,
  stripeCountries,
  takeSignInResult,
  uploadImage,
  type Account,
  type Device,
  type Recent,
  type RecentKind,
  type RecentList,
  type SettingsChanges,
} from "../data/account";
import { pages } from "../data/links";
import {
  accentAttributes,
  accents,
  BackendError,
  currencies,
  defaultTitle,
  fonts,
  formatPrice,
  hundredths,
  isColor,
  loadFonts,
  pageImageUrl,
  type Page,
  type Paywall,
  type Settings,
  type Spin,
  type Tips,
} from "../data/remoteControl";
import { language, t, tRich } from "../i18n";

const accentNames: Record<string, string> = {
  green: t("Green"),
  purple: t("Purple"),
  blue: t("Blue"),
  pink: t("Pink"),
  orange: t("Orange"),
  red: t("Red"),
};

const fontNames: Record<string, string> = {
  rounded: t("Rounded"),
  modern: t("Modern"),
  serif: t("Serif"),
  mono: t("Mono"),
  handwritten: t("Handwritten"),
  pixel: t("Pixel"),
};

const pageUrl = (account: Account) =>
  new URL(asset(`/twitch/${encodeURIComponent(account.login)}`), location.origin).href;

const signedOut = (error: unknown) => error instanceof BackendError && error.status === 401;

const relativeTime = new Intl.RelativeTimeFormat(language, { numeric: "auto" });

function timeAgo(at: number) {
  const seconds = Math.max(0, Math.round(Date.now() / 1000 - at));
  if (seconds < 60) {
    return t("Just now");
  }
  if (seconds < 3600) {
    return relativeTime.format(-Math.round(seconds / 60), "minute");
  }
  if (seconds < 86_400) {
    return relativeTime.format(-Math.round(seconds / 3600), "hour");
  }
  return relativeTime.format(-Math.round(seconds / 86_400), "day");
}

function paymentErrorText(error: unknown, invalid: string) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 400:
        return invalid;
      case 409:
        return t("Connect PayPal or Stripe first.");
      case 503:
        return t("Payments are unavailable right now.");
    }
  }
  return t("Something went wrong. Try again.");
}

function payPalErrorText(error: unknown) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 403:
        return t("To connect PayPal, sign in again first.");
      case 404:
        return t(
          "PayPal has not connected your account yet. Finish signing up on PayPal, then check again.",
        );
      case 409:
        return t(
          "You did not give Moblin permission to take payments for you on PayPal. " +
            "Connect PayPal again and accept the permissions.",
        );
      case 422:
        return t(
          "PayPal can't send payments to your account yet. Confirm your email address on PayPal, then check again.",
        );
      case 502:
        return t("PayPal could not be reached. Try again.");
      case 503:
        return t("Payments are unavailable right now.");
    }
  }
  return t("Something went wrong. Try again.");
}

function stripeErrorText(error: unknown) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 400:
        return t("Pick your country.");
      case 403:
        return t("To connect Stripe, sign in again first.");
      case 422:
        return t(
          "Stripe can't take payments for your account yet. Finish setting it up on Stripe, then check again.",
        );
      case 502:
        return t("Something went wrong on Stripe. Try again.");
      case 503:
        return t("Payments are unavailable right now.");
    }
  }
  return t("Something went wrong. Try again.");
}

function CurrencySettings(props: {
  currency: string;
  busy: boolean;
  onSave: (changes: { currency: string }) => Promise<void>;
}) {
  const [currency, setCurrency] = createSignal(props.currency);
  const [saving, setSaving] = createSignal(false);
  const [saved, flashSaved] = createFlash(2000);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.busy) {
      return;
    }
    setError(null);
    setSaving(true);
    try {
      await props.onSave({ currency: currency() });
      flashSaved();
    } catch (error) {
      setError(paymentErrorText(error, t("Pick one of the currencies.")));
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">{t("Currency")}</h2>
      <p class="text-ink-2">
        {t("Viewers pay all tips, song requests and wheel spins in this currency.")}
      </p>
      <select
        aria-label={t("Currency")}
        value={currency()}
        onChange={(event) => setCurrency(event.currentTarget.value)}
        class={`${inputClass} max-w-32`}
      >
        <For each={currencies}>{(code) => <option value={code}>{code}</option>}</For>
      </select>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
      </div>
    </form>
  );
}

function PayPalAccount(props: {
  session: string;
  settings: Settings;
  onSettings: (settings: Settings) => void;
  onSignOut: () => void;
}) {
  const [working, setWorking] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [signInAgain, setSignInAgain] = createSignal(false);

  const failed = (error: unknown) => {
    if (signedOut(error)) {
      props.onSignOut();
      return;
    }
    setError(payPalErrorText(error));
    setSignInAgain(error instanceof BackendError && error.status === 403);
  };

  const connect = async () => {
    setError(null);
    setSignInAgain(false);
    setWorking(true);
    try {
      location.href = await connectPayPal(props.session);
    } catch (error) {
      failed(error);
      setWorking(false);
    }
  };

  const check = async (quiet: boolean) => {
    setError(null);
    setSignInAgain(false);
    setWorking(true);
    try {
      props.onSettings(await checkPayPal(props.session));
    } catch (error) {
      if (!quiet) {
        failed(error);
      }
    } finally {
      setWorking(false);
    }
  };

  const disconnect = async () => {
    if (
      !confirm(t("Viewers can't pay you with PayPal until you connect a PayPal account again."))
    ) {
      return;
    }
    setError(null);
    setSignInAgain(false);
    setWorking(true);
    try {
      props.onSettings(await disconnectPayPal(props.session));
    } catch (error) {
      failed(error);
    } finally {
      setWorking(false);
    }
  };

  onMount(() => {
    const search = new URLSearchParams(location.search);
    const returned = search.has("merchantIdInPayPal");
    if (returned) {
      history.replaceState(null, "", location.pathname + location.hash);
    }
    if (props.settings.paypalPending) {
      void check(!returned);
    }
  });

  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">PayPal</h2>
      <Show when={props.settings.paypalSandbox}>
        <Notice status>
          {t(
            "PayPal is in sandbox mode. Payments are for testing only and no real money is moved.",
          )}
        </Notice>
      </Show>
      <Show
        when={props.settings.merchantId}
        fallback={
          <p class="text-ink-2">
            {tRich(
              "Connect your PayPal account to let viewers send you tips, on their own or with a song " +
                "request or wheel spin. You sign in or sign up on PayPal with a personal or business " +
                "account (in some countries PayPal asks for a business account). By connecting PayPal you " +
                "agree to the {terms}.",
              {
                terms: (
                  <a href={asset(pages.terms)} class="text-leaf hover:text-leaf-bright">
                    {t("terms")}
                  </a>
                ),
              },
            )}
          </p>
        }
      >
        {(merchantId) => (
          <p class="flex items-center gap-2.5 text-ink-2">
            <span class="size-2.5 shrink-0 rounded-full bg-leaf" />
            <span>
              {tRich("Connected to the PayPal account with merchant ID {merchantId}.", {
                merchantId: <span class="font-mono">{merchantId()}</span>,
              })}
            </span>
          </p>
        )}
      </Show>
      <Show when={props.settings.paypalPending}>
        <p class="text-ink-2">
          {t("Finish connecting your account on PayPal, then reload this page.")}
        </p>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <Show when={!props.settings.merchantId}>
          <button type="button" class={buttonClass} disabled={working()} onClick={connect}>
            {t("Connect")}
          </button>
        </Show>
        <Show when={props.settings.merchantId || props.settings.paypalPending}>
          <button type="button" class={ghostButtonClass} disabled={working()} onClick={disconnect}>
            {t("Disconnect")}
          </button>
        </Show>
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            {t("Sign in again")}
          </a>
        </Show>
      </div>
    </div>
  );
}

const countryNames = new Intl.DisplayNames([language, "en"], { type: "region" });

const sortedStripeCountries = stripeCountries
  .map((code) => ({ code, name: countryNames.of(code) ?? code }))
  .sort((a, b) => a.name.localeCompare(b.name, language));

function guessStripeCountry() {
  for (const tag of navigator.languages) {
    try {
      const region = new Intl.Locale(tag).maximize().region;
      if (region && stripeCountries.includes(region)) {
        return region;
      }
    } catch {}
  }
  return "";
}

function StripeAccount(props: {
  session: string;
  settings: Settings;
  onSettings: (settings: Settings) => void;
  onSignOut: () => void;
}) {
  const [country, setCountry] = createSignal(guessStripeCountry());
  const [working, setWorking] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [signInAgain, setSignInAgain] = createSignal(false);
  const [waiting, setWaiting] = createSignal(false);
  let stopped = false;
  onCleanup(() => (stopped = true));

  const failed = (error: unknown) => {
    if (signedOut(error)) {
      props.onSignOut();
      return;
    }
    setError(stripeErrorText(error));
    setSignInAgain(error instanceof BackendError && error.status === 403);
  };

  const connect = async () => {
    setError(null);
    setSignInAgain(false);
    setWorking(true);
    try {
      location.href = await connectStripe(props.session, country());
    } catch (error) {
      failed(error);
      setWorking(false);
    }
  };

  const check = async (quiet: boolean) => {
    setError(null);
    setSignInAgain(false);
    setWorking(true);
    try {
      props.onSettings(await checkStripe(props.session));
    } catch (error) {
      if (!quiet) {
        failed(error);
      }
    } finally {
      setWorking(false);
    }
  };

  const disconnect = async () => {
    if (
      !confirm(t("Viewers can't pay you with Stripe until you connect a Stripe account again."))
    ) {
      return;
    }
    setError(null);
    setSignInAgain(false);
    setWorking(true);
    try {
      props.onSettings(await disconnectStripe(props.session));
    } catch (error) {
      failed(error);
    } finally {
      setWorking(false);
    }
  };

  const wait = async () => {
    setWaiting(true);
    for (let attempt = 0; attempt < 20 && !stopped; attempt++) {
      if (attempt > 0) {
        await new Promise((resolve) => setTimeout(resolve, 3000));
      }
      await check(attempt < 19);
      if (!props.settings.stripePending) {
        break;
      }
    }
    setWaiting(false);
  };

  onMount(() => {
    const search = new URLSearchParams(location.search);
    const returned = search.has("stripe");
    if (returned) {
      history.replaceState(null, "", location.pathname + location.hash);
    }
    if (props.settings.stripePending) {
      void (returned ? wait() : check(true));
    }
  });

  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">Stripe</h2>
      <Show when={props.settings.stripeTest}>
        <Notice status>
          {t("Stripe is in test mode. Payments are for testing only and no real money is moved.")}
        </Notice>
      </Show>
      <Show
        when={props.settings.stripeAccountId}
        fallback={
          <p class="text-ink-2">
            {tRich(
              "Connect a Stripe account to let viewers pay with a card, Apple Pay or Google Pay. You " +
                "sign up or sign in on Stripe, which asks about you and your bank account so it can pay " +
                "you out. By connecting Stripe you agree to the {terms}.",
              {
                terms: (
                  <a href={asset(pages.terms)} class="text-leaf hover:text-leaf-bright">
                    {t("terms")}
                  </a>
                ),
              },
            )}
          </p>
        }
      >
        {(accountId) => (
          <p class="flex items-center gap-2.5 text-ink-2">
            <span class="size-2.5 shrink-0 rounded-full bg-leaf" />
            <span>
              {tRich("Connected to the Stripe account {accountId}.", {
                accountId: <span class="font-mono">{accountId()}</span>,
              })}
            </span>
          </p>
        )}
      </Show>
      <Show when={props.settings.stripePending}>
        <Show
          when={waiting()}
          fallback={
            <p class="text-ink-2">
              {t("Finish setting up your account on Stripe, then check again.")}
            </p>
          }
        >
          <p role="status" class="flex items-center gap-3 text-ink-2">
            <span class="size-5 shrink-0 animate-spin rounded-full border-2 border-line border-t-leaf" />
            {t("Stripe is checking your account. This usually takes a few seconds.")}
          </p>
        </Show>
      </Show>
      <Show when={!props.settings.stripeAccountId && !props.settings.stripePending}>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">{t("Country")}</span>
          <select
            value={country()}
            onChange={(event) => setCountry(event.currentTarget.value)}
            class={`${inputClass} max-w-80`}
          >
            <option value="" disabled>
              {t("Pick your country")}
            </option>
            <For each={sortedStripeCountries}>
              {(item) => <option value={item.code}>{item.name}</option>}
            </For>
          </select>
          <span class="text-[15px] text-muted">
            {t(
              "Where you or your business are based. Stripe can't change it later, and isn't available in other countries yet.",
            )}
          </span>
        </label>
      </Show>
      <div class={`flex flex-wrap items-center gap-4 ${waiting() ? "hidden" : ""}`}>
        <Show when={!props.settings.stripeAccountId}>
          <button
            type="button"
            class={buttonClass}
            disabled={working() || (!props.settings.stripePending && !country())}
            onClick={connect}
          >
            {props.settings.stripePending ? t("Continue on Stripe") : t("Connect")}
          </button>
        </Show>
        <Show when={props.settings.stripePending}>
          <button
            type="button"
            class={ghostButtonClass}
            disabled={working()}
            onClick={() => void check(false)}
          >
            {t("Check again")}
          </button>
        </Show>
        <Show when={props.settings.stripeAccountId || props.settings.stripePending}>
          <button type="button" class={ghostButtonClass} disabled={working()} onClick={disconnect}>
            {t("Disconnect")}
          </button>
        </Show>
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            {t("Sign in again")}
          </a>
        </Show>
      </div>
    </div>
  );
}

function BannerImage(props: {
  settings: Settings;
  busy: boolean;
  onImage: (image: File | null) => Promise<void>;
}) {
  const [working, setWorking] = createSignal<"upload" | "remove" | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const change = async (image: File | null) => {
    setError(null);
    if (image && image.size > 2 * 1024 * 1024) {
      setError(t("That image is too large. Use one under 2 MB."));
      return;
    }
    setWorking(image ? "upload" : "remove");
    try {
      await props.onImage(image);
    } catch (error) {
      setError(
        error instanceof BackendError && error.status === 400
          ? t(
              "That image could not be read. Use a JPEG, PNG or WebP at least 600 pixels wide and at most 4000 pixels on each side.",
            )
          : error instanceof BackendError && error.status === 413
            ? t("That image is too large. Use one under 2 MB.")
            : t("Something went wrong. Try again."),
      );
    } finally {
      setWorking(null);
    }
  };

  return (
    <div class="flex flex-col gap-2">
      <span class="text-[15px] font-semibold text-ink-2">{t("Banner image (optional)")}</span>
      <Show
        when={props.settings.image}
        fallback={
          <span class="flex aspect-[3/1] w-full items-center justify-center rounded-2xl border-[1.5px] border-dashed border-line bg-well text-muted">
            <Icon name="image" class="size-8" />
          </span>
        }
      >
        {(version) => (
          <img
            src={pageImageUrl(props.settings.id, version())}
            alt=""
            class="aspect-[3/1] w-full rounded-2xl bg-ghost object-cover"
          />
        )}
      </Show>
      <span class="text-sm text-muted">
        {t("Shown at the top of your tip page. Wide images work best, about 3:1.")}
      </span>
      <div class="flex flex-wrap items-center gap-3">
        <label
          class={`${smallButtonClass} cursor-pointer has-[:disabled]:cursor-default has-[:disabled]:opacity-60 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-ink`}
        >
          <input
            type="file"
            accept="image/jpeg,image/png,image/webp"
            disabled={props.busy || working() !== null}
            class="sr-only"
            onChange={(event) => {
              const image = event.currentTarget.files?.[0] ?? null;
              event.currentTarget.value = "";
              if (image) {
                void change(image);
              }
            }}
          />
          <span aria-live="polite">
            {working() === "upload"
              ? t("Uploading…")
              : props.settings.image
                ? t("Change image")
                : t("Upload image")}
          </span>
        </label>
        <Show when={props.settings.image}>
          <button
            type="button"
            class={smallButtonClass}
            disabled={props.busy || working() !== null}
            onClick={() => void change(null)}
          >
            {t("Remove")}
          </button>
        </Show>
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
      </div>
    </div>
  );
}

function YourPage(props: {
  account: Account;
  page: Settings;
  busy: boolean;
  onSave: (changes: Page) => Promise<void>;
  onImage: (image: File | null) => Promise<void>;
}) {
  const [title, setTitle] = createSignal(props.page.title ?? "");
  const [message, setMessage] = createSignal(props.page.message ?? "");
  const [accent, setAccent] = createSignal(props.page.accent ?? accents[0]);
  const [color, setColor] = createSignal(
    isColor(props.page.accent) ? props.page.accent! : "#22b33a",
  );
  const [font, setFont] = createSignal(props.page.font ?? fonts[0]);
  onMount(() => loadFonts(fonts));
  const [saving, setSaving] = createSignal(false);
  const [saved, flashSaved] = createFlash(2000);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.busy) {
      return;
    }
    setError(null);
    setSaving(true);
    try {
      await props.onSave({ title: title(), message: message(), accent: accent(), font: font() });
      flashSaved();
    } catch {
      setError(t("Something went wrong. Try again."));
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">{t("Your tip page")}</h2>
      <p class="text-ink-2">
        {t(
          "Share it with your viewers so they can send you tips, request songs and spin your wheel.",
        )}
      </p>
      <Show when={props.account.pro}>
        <BannerImage settings={props.page} busy={props.busy} onImage={props.onImage} />
      </Show>
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">{t("Headline (optional)")}</span>
        <input
          type="text"
          maxLength={60}
          value={title()}
          onInput={(event) => setTitle(event.currentTarget.value)}
          placeholder={defaultTitle}
          class={inputClass}
        />
      </label>
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">
          {t("Message to your viewers (optional)")}
        </span>
        <textarea
          maxLength={300}
          rows={3}
          value={message()}
          onInput={(event) => setMessage(event.currentTarget.value)}
          placeholder={t("Thanks for hanging out! Say hi in chat!")}
          class={`${inputClass} h-auto resize-y py-3`}
        />
      </label>
      <fieldset class="flex flex-col gap-2">
        <legend class="mb-2 text-[15px] font-semibold text-ink-2">{t("Color")}</legend>
        <div class="flex flex-wrap gap-3">
          <For each={accents}>
            {(name) => (
              <label
                data-accent={name}
                class="relative flex size-11 cursor-pointer items-center justify-center rounded-full bg-moss text-white has-[:checked]:ring-3 has-[:checked]:ring-ink has-[:checked]:ring-offset-3 has-[:checked]:ring-offset-card has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-6 has-[:focus-visible]:outline-ink"
                title={accentNames[name]}
              >
                <input
                  type="radio"
                  name="accent"
                  value={name}
                  checked={accent() === name}
                  onChange={() => setAccent(name)}
                  class="sr-only"
                />
                <span class="sr-only">{accentNames[name]}</span>
                <Show when={accent() === name}>
                  <Icon name="check" class="size-5" />
                </Show>
              </label>
            )}
          </For>
          <Show when={props.account.pro || isColor(props.page.accent)}>
            <label
              {...accentAttributes(color())}
              class="relative flex size-11 cursor-pointer items-center justify-center rounded-full bg-moss text-white has-[:checked]:ring-3 has-[:checked]:ring-ink has-[:checked]:ring-offset-3 has-[:checked]:ring-offset-card has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-6 has-[:focus-visible]:outline-ink"
              title={t("Any color")}
            >
              <input
                type="radio"
                name="accent"
                value={color()}
                checked={isColor(accent())}
                onChange={() => setAccent(color())}
                class="sr-only"
              />
              <span class="sr-only">{t("Any color")}</span>
              <Show
                when={isColor(accent())}
                fallback={<span class="text-2xl leading-none font-semibold">+</span>}
              >
                <Icon name="check" class="size-5" />
              </Show>
              <Show when={props.account.pro}>
                <input
                  type="color"
                  value={color()}
                  onInput={(event) => {
                    setColor(event.currentTarget.value);
                    setAccent(event.currentTarget.value);
                  }}
                  aria-label={t("Any color")}
                  class="absolute inset-0 cursor-pointer rounded-full opacity-0"
                />
              </Show>
            </label>
          </Show>
        </div>
      </fieldset>
      <Show when={props.account.pro || props.page.font}>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">{t("Font")}</legend>
          <div class="flex flex-wrap gap-3">
            <For
              each={
                props.account.pro
                  ? fonts
                  : fonts.filter((name) => name === fonts[0] || name === props.page.font)
              }
            >
              {(name) => (
                <label
                  data-font={name}
                  class="flex h-11 cursor-pointer items-center rounded-full border-[1.5px] border-line bg-well px-5 font-title text-lg font-semibold text-ink transition-colors hover:border-leaf has-[:checked]:border-moss has-[:checked]:bg-moss has-[:checked]:text-white has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-ink"
                >
                  <input
                    type="radio"
                    name="font"
                    value={name}
                    checked={font() === name}
                    onChange={() => setFont(name)}
                    class="sr-only"
                  />
                  {fontNames[name]}
                </label>
              )}
            </For>
          </div>
        </fieldset>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <a href={pageUrl(props.account)} target="_blank" rel="noopener" class={ghostButtonClass}>
          {t("Open tip page")}
        </a>
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
      </div>
    </form>
  );
}

function SongSettings(props: {
  paywall: Paywall | null;
  currency: string;
  busy: boolean;
  onSave: (changes: { paywall: Paywall | null }) => Promise<void>;
}) {
  const [enabled, setEnabled] = createSignal(props.paywall !== null);
  const [price, setPrice] = createSignal(
    props.paywall ? (props.paywall.price / 100).toFixed(2) : "1.00",
  );
  const [saving, setSaving] = createSignal(false);
  const [saved, flashSaved] = createFlash(2000);
  const [error, setError] = createSignal<string | null>(null);
  const [signInAgain, setSignInAgain] = createSignal(false);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.busy) {
      return;
    }
    setError(null);
    setSignInAgain(false);
    setSaving(true);
    try {
      await props.onSave({
        paywall: enabled() ? { price: hundredths(price()) } : null,
      });
      flashSaved();
    } catch (error) {
      setError(paymentErrorText(error, t("Enter a price from 0.50 to 1000.")));
      setSignInAgain(error instanceof BackendError && error.status === 403);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">{t("Song requests")}</h2>
      <p class="text-ink-2">
        {t(
          "Let viewers add songs from Apple Music to the queue of Moblin's music player with a tip.",
        )}
      </p>
      <p class="text-ink-2">
        {t(
          "Moblin plays the songs with Apple Music, so you need an Apple Music subscription and must " +
            "allow Moblin to use Apple Music. Viewers do not need an Apple Music subscription.",
        )}
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        {t("Viewers can request songs")}
      </label>
      <Show when={enabled()}>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">
            {t("Tip per 5 minutes of song ({currency})", { currency: props.currency })}
          </span>
          <span class="flex items-center gap-3">
            <MoneyInput value={price()} onValue={setPrice} required />
            <span class="text-ink-2">{t("per started 5 minutes")}</span>
          </span>
          <span class="text-[15px] text-muted">
            {t(
              "A song up to 5 minutes long costs this once, a song of 5 to 10 minutes twice, and so on. " +
                "Songs always play in full.",
            )}
          </span>
        </label>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            {t("Sign in again")}
          </a>
        </Show>
      </div>
    </form>
  );
}

function MoneyInput(props: {
  value: string;
  onValue: (value: string) => void;
  required?: boolean;
  label?: string;
}) {
  return (
    <input
      type="text"
      required={props.required}
      pattern="\d+([.,]\d{1,2})?"
      inputmode="decimal"
      aria-label={props.label}
      value={props.value}
      onInput={(event) => props.onValue(event.currentTarget.value)}
      class={`${inputClass} max-w-32`}
    />
  );
}

const amountsOf = (amounts: string[]) => amounts.filter((amount) => amount.trim()).map(hundredths);

function TipSettings(props: {
  tips: Tips | null;
  currency: string;
  busy: boolean;
  onSave: (changes: { tips: Tips | null }) => Promise<void>;
}) {
  const [enabled, setEnabled] = createSignal(props.tips !== null);
  const [amounts, setAmounts] = createSignal(
    (props.tips?.amounts ?? [200, 500, 1000]).map((amount) => (amount / 100).toFixed(2)),
  );
  const [saving, setSaving] = createSignal(false);
  const [saved, flashSaved] = createFlash(2000);
  const [error, setError] = createSignal<string | null>(null);
  const [signInAgain, setSignInAgain] = createSignal(false);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.busy) {
      return;
    }
    setError(null);
    setSignInAgain(false);
    setSaving(true);
    try {
      await props.onSave({
        tips: enabled() ? { amounts: amountsOf(amounts()) } : null,
      });
      flashSaved();
    } catch (error) {
      setError(paymentErrorText(error, t("Enter amounts from 0.50 to 1000.")));
      setSignInAgain(error instanceof BackendError && error.status === 403);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">{t("Tips")}</h2>
      <p class="text-ink-2">
        {t(
          "Let viewers send you just a tip. Viewers pick one of your amounts or enter their " +
            "own, and can add a message. Tips show up in Moblin's chat, also those sent while Moblin is " +
            "not connected.",
        )}
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        {t("Viewers can send tips")}
      </label>
      <Show when={enabled()}>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">
            {t("Suggested amounts ({currency})", { currency: props.currency })}
          </legend>
          <div class="flex flex-col items-start gap-3">
            <Index each={amounts()}>
              {(amount, index) => (
                <div class="flex items-center gap-3">
                  <MoneyInput
                    value={amount()}
                    onValue={(value) =>
                      setAmounts(amounts().map((old, at) => (at === index ? value : old)))
                    }
                    required
                    label={t("Amount {number}", { number: index + 1 })}
                  />
                  <Show when={amounts().length > 1}>
                    <button
                      type="button"
                      class="flex size-10 shrink-0 items-center justify-center rounded-xl text-muted hover:bg-ghost hover:text-ink"
                      aria-label={t("Remove amount {number}", { number: index + 1 })}
                      onClick={() => setAmounts(amounts().filter((_, at) => at !== index))}
                    >
                      <Icon name="close" class="size-5" />
                    </button>
                  </Show>
                </div>
              )}
            </Index>
          </div>
          <div class="mt-1">
            <button
              type="button"
              class={ghostButtonClass}
              disabled={amounts().length >= 5}
              onClick={() => setAmounts([...amounts(), ""])}
            >
              {t("Add amount")}
            </button>
          </div>
        </fieldset>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            {t("Sign in again")}
          </a>
        </Show>
      </div>
    </form>
  );
}

function SpinSettings(props: {
  spin: Spin | null;
  currency: string;
  busy: boolean;
  onSave: (changes: { spin: Spin | null }) => Promise<void>;
}) {
  const [enabled, setEnabled] = createSignal(props.spin !== null);
  const [options, setOptions] = createSignal(
    (
      props.spin?.options ?? [
        { spins: 1, price: 500 },
        { spins: 2, price: 800 },
        { spins: 5, price: 1500 },
      ]
    ).map((option) => ({ spins: String(option.spins), price: (option.price / 100).toFixed(2) })),
  );

  const changeOption = (index: number, change: Partial<{ spins: string; price: string }>) =>
    setOptions(options().map((old, at) => (at === index ? { ...old, ...change } : old)));
  const [saving, setSaving] = createSignal(false);
  const [saved, flashSaved] = createFlash(2000);
  const [error, setError] = createSignal<string | null>(null);
  const [signInAgain, setSignInAgain] = createSignal(false);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.busy) {
      return;
    }
    setError(null);
    setSignInAgain(false);
    setSaving(true);
    try {
      await props.onSave({
        spin: enabled()
          ? {
              options: options()
                .filter((option) => option.spins.trim() && option.price.trim())
                .map((option) => ({
                  spins: Number.parseInt(option.spins),
                  price: hundredths(option.price),
                })),
            }
          : null,
      });
      flashSaved();
    } catch (error) {
      setError(paymentErrorText(error, t("Enter 1 to 10 spins for 0.50 to 1000 per price.")));
      setSignInAgain(error instanceof BackendError && error.status === 403);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">{t("Wheel of luck")}</h2>
      <p class="text-ink-2">
        {t(
          "Let viewers spin your wheel by sending you a tip, with a smaller tip per spin " +
            "when they buy more. Moblin spins the wheel you pick in its Moblin Cloud settings, showing " +
            "the viewer's name and message.",
        )}
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        {t("Viewers tip to spin the wheel")}
      </label>
      <Show when={enabled()}>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">
            {t("Tips ({currency})", { currency: props.currency })}
          </legend>
          <div class="flex flex-col gap-3">
            <Index each={options()}>
              {(option, index) => (
                <div class="flex items-center gap-3 text-ink-2">
                  <input
                    type="text"
                    required
                    pattern="\d+"
                    inputmode="numeric"
                    aria-label={t("Spins {number}", { number: index + 1 })}
                    value={option().spins}
                    onInput={(event) => changeOption(index, { spins: event.currentTarget.value })}
                    class={`${inputClass} max-w-20`}
                  />
                  <span class="shrink-0">{t("spins for")}</span>
                  <MoneyInput
                    value={option().price}
                    onValue={(price) => changeOption(index, { price })}
                    required
                    label={t("Tip {number}", { number: index + 1 })}
                  />
                  <Show when={options().length > 1}>
                    <button
                      type="button"
                      class="flex size-10 shrink-0 items-center justify-center rounded-xl text-muted hover:bg-ghost hover:text-ink"
                      aria-label={t("Remove option {number}", { number: index + 1 })}
                      onClick={() => setOptions(options().filter((_, at) => at !== index))}
                    >
                      <Icon name="close" class="size-5" />
                    </button>
                  </Show>
                </div>
              )}
            </Index>
          </div>
          <div class="mt-1">
            <button
              type="button"
              class={ghostButtonClass}
              disabled={options().length >= 5}
              onClick={() => setOptions([...options(), { spins: "", price: "" }])}
            >
              {t("Add option")}
            </button>
          </div>
        </fieldset>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            {t("Sign in again")}
          </a>
        </Show>
      </div>
    </form>
  );
}

function TestSend(props: {
  kind: "song" | "tip" | "spin";
  session: string;
  preview?: boolean;
  onSignOut: () => void;
}) {
  const [song, setSong] = createSignal("");
  const [message, setMessage] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [sent, flashSent] = createFlash(3000);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.preview || sending()) {
      return;
    }
    setError(null);
    setSending(true);
    try {
      await sendTest(
        props.session,
        props.kind,
        props.kind === "song" ? { song: song(), message: message() } : { message: message() },
      );
      flashSent();
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      setError(
        error instanceof BackendError && error.status >= 400 && error.status < 500
          ? error.message
          : t("Something went wrong. Try again."),
      );
    } finally {
      setSending(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">{t("Test it")}</h2>
      <p class="text-ink-2">
        {props.kind === "song"
          ? t("Add a song to Moblin's queue for free, to check that song requests work.")
          : props.kind === "tip"
            ? t("Send yourself a tip for free, to check that tips show up in Moblin.")
            : t("Spin your wheel in Moblin for free, to check that it works.")}
      </p>
      <Show when={props.kind === "song"}>
        <input
          type="url"
          required
          aria-label={t("Apple Music song link")}
          placeholder="https://music.apple.com/…"
          value={song()}
          onInput={(event) => setSong(event.currentTarget.value)}
          class={inputClass}
        />
      </Show>
      <input
        type="text"
        maxLength={200}
        aria-label={t("Message (optional)")}
        placeholder={t("Message (optional)")}
        value={message()}
        onInput={(event) => setMessage(event.currentTarget.value)}
        class={inputClass}
      />
      <div class="flex flex-wrap items-center gap-4">
        <button type="submit" class={buttonClass} disabled={sending()}>
          <Show when={sent()}>
            <Icon name="check" class="size-5" />
          </Show>
          <span aria-live="polite">
            {sending()
              ? t("Sending…")
              : sent()
                ? t("Queued for Moblin")
                : props.kind === "song"
                  ? t("Send test song")
                  : props.kind === "tip"
                    ? t("Send test tip")
                    : t("Send test spin")}
          </span>
        </button>
        <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
      </div>
    </form>
  );
}

function Latest(props: {
  kind: RecentKind;
  list: RecentList | undefined;
  onMore: () => Promise<void>;
}) {
  const [loading, setLoading] = createSignal(false);
  const title = () =>
    props.kind === "song"
      ? t("Received song requests")
      : props.kind === "tip"
        ? t("Received tips")
        : t("Received spins");
  const empty = () =>
    props.kind === "song"
      ? t("No paid song requests yet.")
      : props.kind === "tip"
        ? t("No tips yet.")
        : t("No paid spins yet.");
  const status = (item: Recent) =>
    item.status === "delivered"
      ? { text: t("In Moblin"), class: "text-leaf" }
      : item.status === "waiting"
        ? { text: t("Waiting for Moblin"), class: "text-muted" }
        : { text: t("Not added by Moblin"), class: "text-amber-ink" };
  const more = async () => {
    setLoading(true);
    try {
      await props.onMore();
    } finally {
      setLoading(false);
    }
  };

  return (
    <section class={cardClass}>
      <div class="flex flex-col gap-1">
        <h2 class="text-[23px] font-semibold">{title()}</h2>
        <p class="text-[15px] text-muted">{t("From the last 30 days.")}</p>
      </div>
      <Show when={props.list} fallback={<Spinner />}>
        {(list) => (
          <Show when={list().items.length > 0} fallback={<p class="text-ink-2">{empty()}</p>}>
            <ul class="flex flex-col divide-y divide-line">
              <For each={list().items}>
                {(item) => (
                  <li class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 py-3 first:pt-0 last:pb-0">
                    <div class="flex min-w-0 flex-col gap-0.5">
                      <span class="flex flex-wrap items-baseline gap-x-2">
                        <span class="font-semibold">{item.user}</span>
                        <span class="text-ink-2">
                          {formatPrice({ price: item.amount, currency: item.currency })}
                        </span>
                      </span>
                      <Show when={item.name}>
                        {(name) => <span class="truncate text-ink-2">{name()}</span>}
                      </Show>
                      <Show when={item.spins}>
                        {(spins) => (
                          <span class="text-ink-2">
                            {spins() === 1 ? t("1 spin") : t("{count} spins", { count: spins() })}
                          </span>
                        )}
                      </Show>
                      <Show when={item.message}>
                        {(message) => <span class="text-ink-2 wrap-anywhere">{message()}</span>}
                      </Show>
                      <Show when={item.devices && item.devices.length > 1 && item.devices}>
                        {(devices) => (
                          <ul class="flex flex-wrap gap-x-3 gap-y-0.5 text-[15px]">
                            <For each={devices()}>
                              {(device) => (
                                <li
                                  class={`flex items-center gap-1 ${device.received ? "text-leaf" : "text-muted"}`}
                                >
                                  <Icon
                                    name={device.received ? "check" : "clock"}
                                    class="size-4 shrink-0"
                                  />
                                  <span class="sr-only">
                                    {device.received ? t("Received:") : t("Not received yet:")}
                                  </span>
                                  {device.name}
                                </li>
                              )}
                            </For>
                          </ul>
                        )}
                      </Show>
                    </div>
                    <div class="flex items-baseline gap-3 text-[15px]">
                      <span class="text-muted">{timeAgo(item.at)}</span>
                      <span class={`font-semibold ${status(item).class}`}>{status(item).text}</span>
                    </div>
                  </li>
                )}
              </For>
            </ul>
            <Show when={list().more}>
              <button
                type="button"
                class={`${ghostButtonClass} self-start`}
                disabled={loading()}
                onClick={more}
              >
                {loading() ? t("Loading…") : t("Show more")}
              </button>
            </Show>
          </Show>
        )}
      </Show>
    </section>
  );
}

function Admin(props: {
  session: string;
  account: Account;
  settings: Settings;
  onSignOut: () => void;
  preview?: boolean;
}) {
  const [account, setAccount] = createSignal(props.account);
  const [settings, setSettings] = createSignal(props.settings);
  const [busy, setBusy] = createSignal(false);
  const empty = { items: [], more: false };
  const [recent, setRecent] = createSignal<Record<RecentKind, RecentList> | null>(
    props.preview ? { song: empty, tip: empty, spin: empty } : null,
  );

  const loadRecent = async () => {
    const shown = recent();
    try {
      const [song, tip, spin] = await Promise.all(
        (["song", "tip", "spin"] as const).map((kind) =>
          fetchRecent(
            props.session,
            kind,
            Math.min(Math.max(shown?.[kind].items.length ?? 0, 5), 50),
          ),
        ),
      );
      const merge = (page: RecentList, old: RecentList | undefined) => {
        const last = page.items[page.items.length - 1]?.id;
        const index = old && page.more ? old.items.findIndex((item) => item.id === last) : -1;
        if (!old || index < 0 || index === old.items.length - 1) {
          return page;
        }
        return { items: [...page.items, ...old.items.slice(index + 1)], more: old.more };
      };
      setRecent((old) => ({
        song: merge(song, old?.song),
        tip: merge(tip, old?.tip),
        spin: merge(spin, old?.spin),
      }));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
    }
  };

  const loadMore = async (kind: RecentKind) => {
    const items = recent()?.[kind].items ?? [];
    try {
      const page = await fetchRecent(props.session, kind, 20, items[items.length - 1]?.id);
      setRecent((old) => {
        if (!old) {
          return old;
        }
        const ids = new Set(old[kind].items.map((item) => item.id));
        return {
          ...old,
          [kind]: {
            items: [...old[kind].items, ...page.items.filter((item) => !ids.has(item.id))],
            more: page.more,
          },
        };
      });
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
    }
  };

  const changeImage = async (image: File | null) => {
    setBusy(true);
    try {
      setSettings(await (image ? uploadImage(props.session, image) : removeImage(props.session)));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const changeDevices = async (change: () => Promise<Account>) => {
    setBusy(true);
    try {
      setAccount(await change());
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      throw error;
    } finally {
      setBusy(false);
    }
  };

  onMount(() => {
    if (props.preview) {
      return;
    }
    void loadRecent();
    const timer = setInterval(async () => {
      void loadRecent();
      try {
        const account = await fetchAccount(props.session);
        setAccount(account);
        if (account.settings) {
          setSettings(account.settings);
        }
      } catch (error) {
        if (signedOut(error)) {
          props.onSignOut();
        }
      }
    }, 10_000);
    onCleanup(() => clearInterval(timer));
  });

  const save = async (changes: Partial<SettingsChanges>) => {
    setBusy(true);
    try {
      setSettings(await saveAccountSettings(props.session, { ...settings(), ...changes }));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const payments = () => settings().paypal || settings().stripe;
  const connected = () => settings().merchantId !== null || settings().stripeAccountId !== null;
  const sections: {
    id: string;
    label: string;
    icon: IconName;
    shown: () => boolean;
    status: () => { on: boolean; text: string } | null;
  }[] = [
    {
      id: "page",
      label: t("Tip page"),
      icon: "tip-jar",
      shown: () => true,
      status: () => null,
    },
    {
      id: "tips",
      label: t("Tips"),
      icon: "heart",
      shown: () => payments() || settings().tips !== null,
      status: () => onOff(settings().tips !== null),
    },
    {
      id: "wheel",
      label: t("Wheel of luck"),
      icon: "wheel",
      shown: () => payments() || settings().spin !== null,
      status: () => onOff(settings().spin !== null),
    },
    {
      id: "songs",
      label: t("Song requests"),
      icon: "music",
      shown: () => payments() || settings().paywall !== null,
      status: () => onOff(settings().paywall !== null),
    },
    {
      id: "payments",
      label: t("Payments"),
      icon: "wallet",
      shown: payments,
      status: () => connectedOrNot(connected()),
    },
    {
      id: "moblin",
      label: "Moblin",
      icon: "smartphone",
      shown: () => true,
      status: () => connectedOrNot(settings().connected),
    },
  ];
  const visible = () => sections.filter((section) => section.shown());
  const search = new URLSearchParams(location.search);
  const initial =
    search.has("merchantIdInPayPal") || search.has("stripe") || location.hash === "#paypal"
      ? "payments"
      : location.hash.slice(1);
  const [chosen, setChosen] = createSignal(initial);
  const active = () =>
    visible().some((section) => section.id === chosen()) ? chosen() : visible()[0].id;
  const select = (id: string) => {
    setChosen(id);
    history.replaceState(null, "", `${location.pathname}${location.search}#${id}`);
  };
  const paid = (configured: boolean) => connected() || configured;
  const [menuOpen, setMenuOpen] = createSignal(false);

  return (
    <div class="grid gap-6 lg:grid-cols-[260px_minmax(0,1fr)] lg:items-start">
      <div>
        <button
          type="button"
          aria-expanded={menuOpen()}
          aria-controls="dashboard-menu"
          onClick={() => setMenuOpen(!menuOpen())}
          class="flex w-full items-center gap-3 rounded-2xl border-[1.5px] border-line bg-card px-4 py-3 text-left text-ink lg:hidden"
        >
          <Show when={visible().find((section) => section.id === active())}>
            {(section) => (
              <>
                <Icon name={section().icon} class="size-5 shrink-0" />
                <span class="font-display text-[17px] font-semibold">{section().label}</span>
              </>
            )}
          </Show>
          <Icon
            name="chevron-down"
            class={`ml-auto size-5 shrink-0 transition-transform ${menuOpen() ? "rotate-180" : ""}`}
          />
        </button>
        <nav
          id="dashboard-menu"
          aria-label={t("Dashboard")}
          class="mt-2 flex-col gap-2 lg:sticky lg:top-24 lg:mt-0 lg:flex"
          classList={{ flex: menuOpen(), hidden: !menuOpen() }}
        >
          <For each={visible()}>
            {(section) => (
              <button
                type="button"
                aria-current={active() === section.id ? "page" : undefined}
                onClick={() => {
                  select(section.id);
                  setMenuOpen(false);
                }}
                class="flex items-center gap-3 rounded-2xl border-[1.5px] px-4 py-3 text-left whitespace-nowrap transition-colors"
                classList={{
                  "border-line bg-card text-ink": active() === section.id,
                  "border-transparent text-ink-2 hover:bg-ghost hover:text-ink":
                    active() !== section.id,
                }}
              >
                <Icon name={section.icon} class="size-5 shrink-0" />
                <span class="font-display text-[17px] font-semibold">{section.label}</span>
                <Show when={section.status()}>
                  {(status) => (
                    <span class="ml-auto flex items-center gap-1.5 pl-1 text-[13px] text-muted">
                      <span
                        class={`size-2 shrink-0 rounded-full ${status().on ? "bg-leaf" : "bg-muted/50"}`}
                      />
                      {status().text}
                    </span>
                  )}
                </Show>
              </button>
            )}
          </For>
        </nav>
      </div>
      <div class="min-w-0">
        <div hidden={active() !== "page"}>
          <YourPage
            account={account()}
            page={settings()}
            busy={busy()}
            onSave={save}
            onImage={changeImage}
          />
        </div>
        <div hidden={active() !== "tips"} class="flex flex-col gap-6">
          <Show
            when={paid(settings().tips !== null)}
            fallback={<NeedsPayments title={t("Tips")} onConnect={() => select("payments")} />}
          >
            <TipSettings
              tips={settings().tips}
              currency={settings().currency}
              busy={busy()}
              onSave={save}
            />
          </Show>
          <TestSend
            kind="tip"
            session={props.session}
            preview={props.preview}
            onSignOut={props.onSignOut}
          />
          <Latest kind="tip" list={recent()?.tip} onMore={() => loadMore("tip")} />
        </div>
        <div hidden={active() !== "wheel"} class="flex flex-col gap-6">
          <Show
            when={paid(settings().spin !== null)}
            fallback={
              <NeedsPayments title={t("Wheel of luck")} onConnect={() => select("payments")} />
            }
          >
            <SpinSettings
              spin={settings().spin}
              currency={settings().currency}
              busy={busy()}
              onSave={save}
            />
          </Show>
          <TestSend
            kind="spin"
            session={props.session}
            preview={props.preview}
            onSignOut={props.onSignOut}
          />
          <Latest kind="spin" list={recent()?.spin} onMore={() => loadMore("spin")} />
        </div>
        <div hidden={active() !== "songs"} class="flex flex-col gap-6">
          <Show
            when={paid(settings().paywall !== null)}
            fallback={
              <NeedsPayments title={t("Song requests")} onConnect={() => select("payments")} />
            }
          >
            <SongSettings
              paywall={settings().paywall}
              currency={settings().currency}
              busy={busy()}
              onSave={save}
            />
          </Show>
          <TestSend
            kind="song"
            session={props.session}
            preview={props.preview}
            onSignOut={props.onSignOut}
          />
          <Latest kind="song" list={recent()?.song} onMore={() => loadMore("song")} />
        </div>
        <Show when={payments()}>
          <div hidden={active() !== "payments"} class="flex flex-col gap-6">
            <Show when={settings().paypal}>
              <PayPalAccount
                session={props.session}
                settings={settings()}
                onSettings={setSettings}
                onSignOut={props.onSignOut}
              />
            </Show>
            <Show when={settings().stripe}>
              <StripeAccount
                session={props.session}
                settings={settings()}
                onSettings={setSettings}
                onSignOut={props.onSignOut}
              />
            </Show>
            <Show when={connected()}>
              <CurrencySettings currency={settings().currency} busy={busy()} onSave={save} />
            </Show>
          </div>
        </Show>
        <div hidden={active() !== "moblin"}>
          <MoblinSettings
            account={account()}
            busy={busy()}
            onAdd={(name) => changeDevices(() => addDevice(props.session, name))}
            onChoose={(music, wheel) =>
              changeDevices(() => chooseDevices(props.session, music, wheel))
            }
            onNewKey={(device) => changeDevices(() => newDeviceKey(props.session, device))}
            onRemove={(device) => changeDevices(() => removeDevice(props.session, device))}
          />
        </div>
      </div>
    </div>
  );
}

const onOff = (on: boolean) => ({ on, text: on ? t("On") : t("Off") });

const connectedOrNot = (on: boolean) => ({ on, text: on ? t("Connected") : t("Not connected") });

function NeedsPayments(props: { title: string; onConnect: () => void }) {
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{props.title}</h2>
      <p class="text-ink-2">
        {t("Connect PayPal or Stripe first, so the money goes straight to you.")}
      </p>
      <div>
        <button type="button" class={buttonClass} onClick={() => props.onConnect()}>
          {t("Go to payments")}
        </button>
      </div>
    </div>
  );
}

function QrCode(props: { text: string; label: string }) {
  const qr = () => encode(props.text, { border: 4 });
  const path = () =>
    qr()
      .data.flatMap((row, y) => row.map((dark, x) => (dark ? `M${x} ${y}h1v1h-1z` : "")))
      .join("");

  return (
    <svg
      viewBox={`0 0 ${qr().size} ${qr().size}`}
      role="img"
      aria-label={props.label}
      shape-rendering="crispEdges"
      class="size-64 rounded-2xl"
    >
      <rect width="100%" height="100%" fill="#fff" />
      <path d={path()} fill="#000" />
    </svg>
  );
}

function MoblinSettings(props: {
  account: Account;
  busy: boolean;
  onAdd: (name: string) => Promise<void>;
  onChoose: (music: string, wheel: string) => Promise<void>;
  onNewKey: (device: string) => Promise<void>;
  onRemove: (device: string) => Promise<void>;
}) {
  const [name, setName] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const [opened, setOpened] = createSignal<string | null>(null);
  const devices = () => props.account.devices;
  const full = () => devices().length >= 5;

  const run = async (action: () => Promise<void>) => {
    setError(null);
    try {
      await action();
    } catch (error) {
      if (!signedOut(error)) {
        setError(t("Something went wrong. Try again."));
      }
    }
  };

  const add = (event: SubmitEvent) => {
    event.preventDefault();
    void run(async () => {
      const before = devices().map((device) => device.id);
      await props.onAdd(name().trim());
      setName("");
      setOpened(devices().find((device) => !before.includes(device.id))?.id ?? null);
    });
  };

  const choose = (music: string, wheel: string) => void run(() => props.onChoose(music, wheel));

  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">Moblin</h2>
      <p class="text-ink-2">
        {t(
          "Connect up to 5 devices running Moblin. Each device has its own Moblin Cloud key. In Moblin, go to Settings → Moblin Cloud, paste the key and turn on Enabled.",
        )}
      </p>
      <Show when={devices().length > 0}>
        <ul class="flex flex-col divide-y-[1.5px] divide-line rounded-2xl border-[1.5px] border-line">
          <For each={devices().map((device) => device.id)}>
            {(id) => (
              <Show when={devices().find((device) => device.id === id)}>
                {(device) => (
                  <DeviceSettings
                    device={device()}
                    busy={props.busy}
                    open={opened() === id}
                    onToggle={() => setOpened(opened() === id ? null : id)}
                    onNewKey={() => run(() => props.onNewKey(id))}
                    onRemove={() => run(() => props.onRemove(id))}
                  />
                )}
              </Show>
            )}
          </For>
        </ul>
      </Show>
      <form class="flex flex-wrap items-center gap-4" onSubmit={add}>
        <input
          type="text"
          maxLength={30}
          required
          aria-label={t("Device name")}
          placeholder={t("Device name")}
          value={name()}
          onInput={(event) => setName(event.currentTarget.value)}
          disabled={full()}
          class={`${inputClass} max-w-64`}
        />
        <button
          type="submit"
          class={ghostButtonClass}
          disabled={props.busy || full() || name().trim() === ""}
        >
          {t("Add device")}
        </button>
      </form>
      <Show when={error()}>{(error) => <Notice status>{error()}</Notice>}</Show>
      <Show when={devices().length > 1}>
        <div class="grid gap-4 sm:grid-cols-2">
          <label class="flex flex-col gap-2">
            <span class="text-[15px] font-semibold text-ink-2">{t("Song requests")}</span>
            <select
              disabled={props.busy}
              onChange={(event) =>
                choose(event.currentTarget.value, props.account.wheelDevice ?? "")
              }
              class={inputClass}
            >
              <For each={devices()}>
                {(device) => (
                  <option value={device.id} selected={device.id === props.account.musicDevice}>
                    {device.name}
                  </option>
                )}
              </For>
            </select>
          </label>
          <label class="flex flex-col gap-2">
            <span class="text-[15px] font-semibold text-ink-2">{t("Wheel of luck")}</span>
            <select
              disabled={props.busy}
              onChange={(event) =>
                choose(props.account.musicDevice ?? "", event.currentTarget.value)
              }
              class={inputClass}
            >
              <For each={devices()}>
                {(device) => (
                  <option value={device.id} selected={device.id === props.account.wheelDevice}>
                    {device.name}
                  </option>
                )}
              </For>
            </select>
          </label>
        </div>
        <p class="text-[15px] text-muted">
          {t(
            "Song requests and wheel spins go to the device picked for them. Tips go to every device.",
          )}
        </p>
      </Show>
    </div>
  );
}

const smallButtonClass =
  "inline-flex h-10 items-center justify-center gap-2 rounded-full border-[1.5px] border-ghost-line bg-ghost px-4 font-display text-[15px] font-semibold whitespace-nowrap text-ink transition-colors hover:bg-ghost-hover disabled:opacity-60";

function DeviceSettings(props: {
  device: Device;
  busy: boolean;
  open: boolean;
  onToggle: () => void;
  onNewKey: () => void;
  onRemove: () => void;
}) {
  const [qrShown, setQrShown] = createSignal(false);
  const [copied, flashCopied] = createFlash(1500);
  const deepLink = () => {
    const settings = { moblinCloud: { enabled: true, key: props.device.key } };
    return `moblin://?${encodeURIComponent(JSON.stringify(settings))}`;
  };
  const copyDeepLink = async () => {
    if (await copyText(deepLink())) {
      flashCopied();
    }
  };
  const newKey = () => {
    if (
      confirm(
        t(
          "{name} disconnects and its current Moblin Cloud key stops working. Paste the new one in Moblin to reconnect.",
          { name: props.device.name },
        ),
      )
    ) {
      props.onNewKey();
    }
  };
  const remove = () => {
    if (
      confirm(
        t("{name} disconnects and its Moblin Cloud key stops working.", {
          name: props.device.name,
        }),
      )
    ) {
      props.onRemove();
    }
  };

  return (
    <li>
      <button
        type="button"
        aria-expanded={props.open}
        onClick={props.onToggle}
        class="flex w-full items-center gap-3 px-4 py-3 text-left"
      >
        <span
          class={`size-2.5 shrink-0 rounded-full ${props.device.connected ? "bg-leaf" : "bg-muted"}`}
        />
        <span class="min-w-0 flex-1 truncate font-semibold">{props.device.name}</span>
        <span class="text-[15px] text-muted">
          {props.device.connected ? t("Connected") : t("Not connected")}
        </span>
        <Icon
          name="chevron-down"
          class={`size-5 shrink-0 text-muted transition-transform ${props.open ? "rotate-180" : ""}`}
        />
      </button>
      <Show when={props.open}>
        <div class="flex flex-col gap-4 px-4 pb-4">
          <Value label={t("Moblin Cloud key")} text={props.device.key} secret />
          <Notice>
            {t(
              "Keep it to yourself. Anyone with it can connect as your Moblin. Reset it if it leaks.",
            )}
          </Notice>
          <p class="text-[15px] text-ink-2">
            {t(
              "Or set it all up at once on the device running Moblin: scan the QR code with its camera or open the deep link there.",
            )}
          </p>
          <div class="flex flex-wrap items-center gap-2">
            <button type="button" class={smallButtonClass} onClick={() => setQrShown(!qrShown())}>
              <Icon name={qrShown() ? "eye-off" : "eye"} class="size-4" />
              {qrShown() ? t("Hide QR code") : t("Show QR code")}
            </button>
            <button type="button" class={smallButtonClass} onClick={copyDeepLink}>
              <Icon name={copied() ? "check" : "copy"} class="size-4" />
              {copied() ? t("Copied") : t("Copy deep link")}
            </button>
            <button type="button" class={smallButtonClass} disabled={props.busy} onClick={newKey}>
              {t("Reset Moblin Cloud key")}
            </button>
            <button type="button" class={smallButtonClass} disabled={props.busy} onClick={remove}>
              {t("Remove device")}
            </button>
          </div>
          <Show when={qrShown()}>
            <QrCode text={deepLink()} label={t("QR code that sets up Moblin Cloud in Moblin")} />
          </Show>
        </div>
      </Show>
    </li>
  );
}

function SignedIn(props: {
  session: string;
  account: Account;
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const [failed, setFailed] = createSignal(false);

  const create = async () => {
    setFailed(false);
    try {
      const account = await createStreamer(props.session);
      if (account.settings) {
        props.onAccount(account);
      } else {
        setFailed(true);
      }
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      } else {
        setFailed(true);
      }
    }
  };

  onMount(() => {
    if (!props.account.settings) {
      void create();
    }
  });

  return (
    <Show
      when={props.account.settings ? props.account : null}
      keyed
      fallback={
        <Show when={failed()} fallback={<Spinner />}>
          <div class="flex flex-wrap items-center gap-4">
            <Notice status>{t("Could not create your tip page.")}</Notice>
            <button type="button" class={ghostButtonClass} onClick={create}>
              {t("Try again")}
            </button>
          </div>
        </Show>
      }
    >
      {(account) => (
        <Admin
          session={props.session}
          account={account}
          settings={account.settings!}
          onSignOut={props.onSignOut}
        />
      )}
    </Show>
  );
}

function Preview(props: { failed: boolean }) {
  return (
    <div class="flex flex-col gap-6">
      <Show when={props.failed}>
        <Notice status>{t("Signing in did not work. Try again.")}</Notice>
      </Show>
      <div class="relative rounded-3xl border-[1.5px] border-dashed border-line p-4 sm:p-6">
        <span class="absolute -top-3.5 left-6 rounded-full border-[1.5px] border-line bg-band px-3 py-0.5 font-display text-[13px] font-semibold uppercase tracking-[0.08em] text-muted">
          {t("Preview")}
        </span>
        <div
          inert
          aria-hidden="true"
          class="max-h-[520px] overflow-hidden opacity-50 select-none [&_nav]:static [mask-image:linear-gradient(to_bottom,black_30%,transparent_80%)]"
        >
          <Admin
            preview
            session=""
            account={{
              name: "",
              login: "",
              devices: [
                { id: "", name: "Moblin", key: "moblin-cloud-key-preview", connected: true },
              ],
              musicDevice: null,
              wheelDevice: null,
              settings: null,
              admin: false,
              pro: false,
            }}
            settings={{
              id: "",
              title: null,
              message: null,
              accent: null,
              font: null,
              image: null,
              connected: true,
              currency: "USD",
              paywall: { price: 100 },
              tips: { amounts: [300, 500, 1000] },
              spin: {
                options: [
                  { spins: 1, price: 200 },
                  { spins: 5, price: 800 },
                ],
              },
              paypal: true,
              merchantId: "preview",
              paypalPending: false,
              paypalSandbox: false,
              stripe: true,
              stripeAccountId: null,
              stripePending: false,
              stripeTest: false,
            }}
            onSignOut={() => {}}
          />
        </div>
        <div class="absolute inset-x-0 bottom-0 flex flex-col items-center gap-4 px-6 pb-10 text-center">
          <p class="text-lg font-semibold text-balance">
            {t("Sign in with Twitch to create or manage your tip page.")}
          </p>
          <a
            href={signInUrl}
            class="inline-flex h-13 items-center justify-center gap-2.5 rounded-full bg-twitch px-6 font-display text-lg font-semibold whitespace-nowrap text-white transition-colors hover:bg-twitch-bright"
          >
            <Icon name="twitch" class="size-5" />
            {t("Sign in with Twitch")}
          </a>
        </div>
      </div>
    </div>
  );
}

export default function Dashboard() {
  const [session, setSession] = createSignal<{ session: string; account: Account } | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [signInFailed, setSignInFailed] = createSignal(false);

  onMount(async () => {
    const result = takeSignInResult();
    if (result.session) {
      saveSession(result.session);
    }
    setSignInFailed(result.failed);
    const stored = readSession();
    if (stored) {
      try {
        setSession({ session: stored, account: await fetchAccount(stored) });
      } catch (error) {
        if (signedOut(error)) {
          saveSession(null);
        }
      }
    }
    setLoading(false);
  });

  const signOut = () => {
    saveSession(null);
    setSession(null);
  };

  const terms = () => (
    <p class="text-[15px] text-muted">
      {tRich("Using Moblin Cloud and your tip page means you agree to the {terms}.", {
        terms: (
          <a href={asset(pages.terms)} class="text-leaf hover:text-leaf-bright">
            {t("terms")}
          </a>
        ),
      })}
    </p>
  );

  return (
    <>
      <CloudHeader>
        <Show
          when={session()}
          fallback={
            <Show when={!loading()}>
              <a
                href={signInUrl}
                class="inline-flex h-11 items-center justify-center gap-2 rounded-full bg-twitch px-5 font-display text-base font-semibold whitespace-nowrap text-white transition-colors hover:bg-twitch-bright"
              >
                <Icon name="twitch" class="size-5" />
                {t("Sign in")}
              </a>
            </Show>
          }
        >
          {(current) => (
            <div class="flex items-center gap-3">
              <Show when={current().account.admin}>
                <a href={asset(pages.admin)} class={`${ghostButtonClass} h-11! px-5! text-base!`}>
                  {t("Admin")}
                </a>
              </Show>
              <button
                type="button"
                class={`${ghostButtonClass} h-11! px-5! text-base!`}
                onClick={async () => {
                  try {
                    await signOutEverywhere(current().session);
                  } catch {}
                  signOut();
                }}
              >
                {t("Sign out")}
              </button>
            </div>
          )}
        </Show>
      </CloudHeader>
      <main>
        <Show
          when={session()}
          fallback={
            <Hero title={t("Let viewers tip you.")}>
              <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
                {t(
                  "Tips go straight to your PayPal or Stripe account. Moblin takes nothing, PayPal and " +
                    "Stripe charge their usual fees.",
                )}
              </p>
              {terms()}
            </Hero>
          }
        >
          {(current) => (
            <section class="mx-auto flex max-w-[1120px] flex-col gap-2 px-5 pt-10 pb-8 sm:px-8">
              <h1 class="text-[32px] leading-[1.1] font-bold text-balance sm:text-[40px]">
                {t("Hi, {name}", { name: current().account.name })}
              </h1>
              {terms()}
            </section>
          )}
        </Show>
        <Band tight={session() !== null}>
          <Show when={!loading()} fallback={<Spinner />}>
            <Show when={session()} keyed fallback={<Preview failed={signInFailed()} />}>
              {(current) => (
                <SignedIn
                  session={current.session}
                  account={current.account}
                  onAccount={(account) => setSession({ ...current, account })}
                  onSignOut={signOut}
                />
              )}
            </Show>
          </Show>
        </Band>
      </main>
    </>
  );
}
