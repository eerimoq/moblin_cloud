import { createMemo, createSignal, For, Index, onCleanup, onMount, Show, type JSX } from "solid-js";
import { encode } from "uqr";
import { asset } from "../asset";
import { createFlash } from "../flash";
import { poll } from "../poll";
import { copyText } from "../components/CopyButton";
import Icon, { type IconName } from "../components/Icon";
import { createRecent, Latest, timeAgo } from "../components/Latest";
import {
  AccountMenu,
  Band,
  buttonClass,
  cardClass,
  CloudHeader,
  ghostButtonClass,
  inputClass,
  Notice,
  PlatformSignIn,
  SaveButton,
  Spinner,
  Value,
} from "../components/PageParts";
import {
  addModerator,
  createAccountSession,
  type Moderator,
  removeModerator,
  type Moderating,
  checkPayPal,
  checkStripe,
  connectPayPal,
  connectStripe,
  disconnectPayPal,
  disconnectStripe,
  fetchAccount,
  fetchRecent,
  newMoblinKey,
  renameDevice,
  removeImage,
  saveAccountSettings,
  resendRecent,
  sendTest,
  stripeCountries,
  uploadImage,
  type Account,
  type Device,
  type Recent,
  type RecentKind,
  type SettingsChanges,
} from "../data/account";
import { BackendError } from "../data/backend";
import { links, pages } from "../data/links";
import {
  accentAttributes,
  accents,
  currencies,
  fonts,
  hundredths,
  isColor,
  isOn,
  loadFonts,
  pageImageUrl,
  type Platform,
  platformNames,
  platforms,
  tierCount,
  tierNumbers,
  type Feature,
  type Page,
  type Settings,
  type Spin,
  type Tiers,
  type Tips,
} from "../data/remoteControl";
import { language, t, tRich } from "../i18n";
import CloudLanding from "../components/CloudLanding";

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

const pageUrl = (platform: Platform, login: string) =>
  new URL(asset(`/${platform}/${encodeURIComponent(login)}`), location.origin).href;

const pageUrls = (account: Account) =>
  platforms.flatMap((platform) => {
    const identity = account[platform];
    return identity ? [{ platform, url: pageUrl(platform, identity.login) }] : [];
  });

const signedOut = (error: unknown) => error instanceof BackendError && error.status === 401;

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

const forbidden = (error: unknown) => error instanceof BackendError && error.status === 403;

function SignInAgain() {
  return (
    <span class="flex flex-wrap gap-3">
      <For each={platforms}>
        {(platform) => (
          <PlatformSignIn platform={platform} large>
            {t("Sign in again")}
          </PlatformSignIn>
        )}
      </For>
    </span>
  );
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

function createSave(
  busy: () => boolean,
  changed: () => boolean,
  run: () => Promise<void>,
  errorText: (error: unknown) => string,
) {
  const [saving, setSaving] = createSignal(false);
  const [saved, flashSaved] = createFlash(2000);
  const [failure, setFailure] = createSignal<{ error: unknown } | null>(null);
  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (busy()) {
      return;
    }
    setFailure(null);
    setSaving(true);
    try {
      await run();
      flashSaved();
    } catch (error) {
      setFailure({ error });
    } finally {
      setSaving(false);
    }
  };
  const dismiss = () => setFailure(null);
  return { busy, changed, saving, saved, failure, errorText, submit, dismiss };
}

function SaveFooter(props: { save: ReturnType<typeof createSave>; signInAgain?: boolean }) {
  return (
    <div class="flex flex-wrap items-center gap-4">
      <SaveButton
        saving={props.save.saving()}
        saved={props.save.saved()}
        disabled={props.save.busy() || !props.save.changed()}
      />
      <Show when={props.save.failure()}>
        {(failure) => (
          <Notice status onClose={props.save.dismiss}>
            {props.save.errorText(failure().error)}
          </Notice>
        )}
      </Show>
      <Show when={props.signInAgain && forbidden(props.save.failure()?.error)}>
        <SignInAgain />
      </Show>
    </div>
  );
}

function CurrencySettings(props: {
  currency: string;
  locked: boolean;
  busy: boolean;
  onSave: (changes: { currency: string }) => Promise<void>;
}) {
  const [currency, setCurrency] = createSignal(props.currency);
  const save = createSave(
    () => props.busy,
    () => currency() !== props.currency,
    () => props.onSave({ currency: currency() }),
    (error) =>
      error instanceof BackendError && error.status === 409
        ? t("Turn tips and the wheel off to change the currency.")
        : paymentErrorText(error, t("Pick one of the currencies.")),
  );

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">{t("Currency")}</h2>
      <p class="text-ink-2">{t("Tips and wheel spins are all in this currency.")}</p>
      <Show when={props.locked}>
        <p class="text-ink-2">{t("Turn tips and the wheel off to change the currency.")}</p>
      </Show>
      <select
        aria-label={t("Currency")}
        disabled={props.locked}
        value={currency()}
        onChange={(event) => setCurrency(event.currentTarget.value)}
        class={`${inputClass} max-w-32`}
      >
        <For each={currencies}>{(code) => <option value={code}>{code}</option>}</For>
      </select>
      <SaveFooter save={save} />
    </form>
  );
}

function createProviderActions(
  props: { session: string; onSettings: (settings: Settings) => void; onSignOut: () => void },
  provider: {
    errorText: (error: unknown) => string;
    connect: () => Promise<string>;
    check: (session: string) => Promise<Settings>;
    disconnect: (session: string) => Promise<Settings>;
    confirmText: string;
  },
) {
  const [working, setWorking] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [signInAgain, setSignInAgain] = createSignal(false);

  const begin = () => {
    setError(null);
    setSignInAgain(false);
    setWorking(true);
  };

  const failed = (error: unknown) => {
    if (signedOut(error)) {
      props.onSignOut();
      return;
    }
    setError(provider.errorText(error));
    setSignInAgain(forbidden(error));
  };

  const run = async (action: () => Promise<Settings>, report = true) => {
    begin();
    try {
      props.onSettings(await action());
    } catch (error) {
      if (report) {
        failed(error);
      }
    } finally {
      setWorking(false);
    }
  };

  const connect = async () => {
    begin();
    try {
      location.href = await provider.connect();
    } catch (error) {
      failed(error);
      setWorking(false);
    }
  };

  const check = (quiet: boolean) => run(() => provider.check(props.session), !quiet);

  const disconnect = async () => {
    if (confirm(provider.confirmText)) {
      await run(() => provider.disconnect(props.session));
    }
  };

  return { working, error, setError, signInAgain, connect, check, disconnect };
}

function PayPalAccount(props: {
  session: string;
  settings: Settings;
  onSettings: (settings: Settings) => void;
  onSignOut: () => void;
}) {
  const { working, error, setError, signInAgain, connect, check, disconnect } =
    createProviderActions(props, {
      errorText: payPalErrorText,
      connect: () => connectPayPal(props.session),
      check: checkPayPal,
      disconnect: disconnectPayPal,
      confirmText: t("Viewers can't pay you with PayPal until you connect a PayPal account again."),
    });

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
              "Connect your PayPal account to let viewers send you tips, on their own or with a " +
                "wheel spin. You sign in or sign up on PayPal with a personal or business " +
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
        <Show when={error()}>
          {(error) => (
            <Notice status onClose={() => setError(null)}>
              {error()}
            </Notice>
          )}
        </Show>
        <Show when={signInAgain()}>
          <SignInAgain />
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
  const { working, error, setError, signInAgain, connect, check, disconnect } =
    createProviderActions(props, {
      errorText: stripeErrorText,
      connect: () => connectStripe(props.session, country()),
      check: checkStripe,
      disconnect: disconnectStripe,
      confirmText: t("Viewers can't pay you with Stripe until you connect a Stripe account again."),
    });
  const [waiting, setWaiting] = createSignal(false);
  let stopped = false;
  onCleanup(() => (stopped = true));

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
        <Show when={error()}>
          {(error) => (
            <Notice status onClose={() => setError(null)}>
              {error()}
            </Notice>
          )}
        </Show>
        <Show when={signInAgain()}>
          <SignInAgain />
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
        <Show when={error()}>
          {(error) => (
            <Notice status onClose={() => setError(null)}>
              {error()}
            </Notice>
          )}
        </Show>
      </div>
    </div>
  );
}

function YourPage(props: {
  account: Account;
  page: Settings;
  busy: boolean;
  moderating: boolean;
  onSave: (changes: Page) => Promise<void>;
  onImage: (image: File | null) => Promise<void>;
}) {
  const initial = (): Page => ({
    message: props.page.message ?? "",
    accent: props.page.accent ?? accents[0],
    font: props.page.font ?? fonts[0],
  });
  const [message, setMessage] = createSignal(initial().message!);
  const [accent, setAccent] = createSignal(initial().accent!);
  const [color, setColor] = createSignal(
    isColor(props.page.accent) ? props.page.accent! : "#22b33a",
  );
  const [font, setFont] = createSignal(initial().font!);
  const current = (): Page => ({ message: message(), accent: accent(), font: font() });
  onMount(() => loadFonts(fonts));
  const save = createSave(
    () => props.busy,
    () => !same(current(), initial()),
    () => props.onSave(current()),
    () => t("Something went wrong. Try again."),
  );

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">
        {props.moderating
          ? t("The tip page of {name}", { name: props.account.name })
          : t("Your tip page")}
      </h2>
      <p class="text-ink-2">
        {t("Share it with your viewers so they can send you tips and spin your wheel.")}
      </p>
      <For each={pageUrls(props.account)}>
        {(address) => (
          <div class="flex flex-col gap-3 sm:flex-row sm:items-end">
            <div class="min-w-0 flex-1">
              <Value
                label={
                  pageUrls(props.account).length > 1
                    ? t("Address for {platform} viewers", {
                        platform: platformNames[address.platform],
                      })
                    : t("Address")
                }
                text={address.url}
                href={address.url}
              />
            </div>
            <a
              href={address.url}
              target="_blank"
              rel="noopener"
              class={`${ghostButtonClass} shrink-0`}
            >
              {t("Open tip page")}
            </a>
          </div>
        )}
      </For>
      <Show when={props.account.premium}>
        <BannerImage settings={props.page} busy={props.busy} onImage={props.onImage} />
      </Show>
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
          <Show when={props.account.premium || isColor(props.page.accent)}>
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
              <Show when={props.account.premium}>
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
      <Show when={props.account.premium || props.page.font}>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">{t("Font")}</legend>
          <div class="flex flex-wrap gap-3">
            <For
              each={
                props.account.premium
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
      <Show when={!props.account.premium}>
        <p class="text-[15px] text-ink-2">
          {t(
            "With a premium account you can also add a banner image, pick any color and choose a font.",
          )}
        </p>
      </Show>
      <SaveFooter save={save} />
    </form>
  );
}

function SubscriberSettings(props: {
  tiers: Feature<Tiers> | null;
  twitch: boolean;
  kick: boolean;
  busy: boolean;
  onSave: (tiers: Feature<Tiers>) => Promise<void>;
}) {
  const shownTiers = () => (props.twitch ? tierNumbers : ([1] as const));
  const initial = () => ({
    enabled: isOn(props.tiers),
    amounts: tierNumbers.map((tier) =>
      String(props.tiers ? tierCount(props.tiers, tier) : [1, 5, 10][tier - 1]),
    ),
  });
  const [enabled, setEnabled] = createSignal(initial().enabled);
  const [amounts, setAmounts] = createSignal(initial().amounts);
  const tiers = (form: ReturnType<typeof initial>): Feature<Tiers> => {
    const [tier1, tier2, tier3] = form.amounts.map((count) => Number.parseInt(count, 10) || 0);
    return { enabled: form.enabled, tier1, tier2, tier3 };
  };
  const current = () => tiers({ enabled: enabled(), amounts: amounts() });
  const save = createSave(
    () => props.busy,
    () => !same(current(), tiers(initial())),
    () => props.onSave(current()),
    (error) =>
      paymentErrorText(
        error,
        t("Enter 0 to 100 spins per tier, and more than 0 for at least one tier."),
      ),
  );

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">
        {props.twitch && !props.kick
          ? t("Spins for Twitch subscribers")
          : t("Spins for subscribers")}
      </h2>
      <p class="text-ink-2">
        {t(
          "Give your subscribers spins of the wheel every month, without a tip. Subscribers sign " +
            "in on your tip page, and their spins come back on the 1st of every " +
            "month. This works with or without paid wheel spins.",
        )}
        <Show when={props.twitch && props.kick}>
          {" "}
          {t("Kick subscribers get the tier 1 amount.")}
        </Show>
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        {t("Subscribers can spin the wheel without a tip")}
      </label>
      <Show when={enabled()}>
        <div class="flex flex-wrap gap-4">
          <For each={shownTiers()}>
            {(tier) => (
              <label class="flex flex-col gap-2">
                <span class="text-[15px] font-semibold text-ink-2">
                  {props.twitch ? t("Tier {tier}", { tier }) : t("Kick subscribers")}
                </span>
                <input
                  type="number"
                  required
                  min={0}
                  max={100}
                  inputmode="numeric"
                  value={amounts()[tier - 1]}
                  onInput={(event) => {
                    const value = event.currentTarget.value;
                    setAmounts(amounts().map((old, at) => (at === tier - 1 ? value : old)));
                  }}
                  class={`${inputClass} max-w-28`}
                />
              </label>
            )}
          </For>
        </div>
        <span class="text-[15px] text-muted">{t("Spins per month for each tier.")}</span>
      </Show>
      <SaveFooter save={save} />
    </form>
  );
}

function ModeratorSettings(props: {
  session: string;
  account: Account;
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const linked = () => platforms.filter((platform) => props.account[platform] !== null);
  return (
    <section class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Moderators")}</h2>
      <p class="text-ink-2">
        {t("Moderators can manage your dashboard, except payments, moderators and Moblin.")}
      </p>
      <For each={linked()}>
        {(platform) => (
          <ModeratorList
            session={props.session}
            platform={platform}
            titled={linked().length > 1}
            moderators={
              platform === "twitch" ? props.account.twitchModerators : props.account.kickModerators
            }
            onAccount={props.onAccount}
            onSignOut={props.onSignOut}
          />
        )}
      </For>
    </section>
  );
}

function ModeratorList(props: {
  session: string;
  platform: Platform;
  titled: boolean;
  moderators: Moderator[];
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const [username, setUsername] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const run = async (action: () => Promise<Account>) => {
    if (busy()) {
      return false;
    }
    setBusy(true);
    setError(null);
    try {
      props.onAccount(await action());
      return true;
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      setError(moderationErrorText(error));
      return false;
    } finally {
      setBusy(false);
    }
  };
  const add = async (event: Event) => {
    event.preventDefault();
    const name = username().trim();
    if (!name) {
      return;
    }
    if (await run(() => addModerator(props.session, props.platform, name))) {
      setUsername("");
    }
  };
  const placeholder = () =>
    props.platform === "twitch" ? t("Twitch username") : t("Kick username");
  return (
    <>
      <Show when={props.titled}>
        <h3 class="flex items-center gap-2 text-lg font-semibold">
          <Icon
            name={props.platform}
            class={props.platform === "twitch" ? "size-5 text-twitch" : "size-5 text-kick"}
          />
          {platformNames[props.platform]}
        </h3>
      </Show>
      <Show when={props.moderators.length > 0}>
        <div class="overflow-hidden rounded-2xl border-[1.5px] border-line">
          <table class="w-full table-fixed">
            <thead class="bg-band">
              <tr class="text-left text-[12px] font-semibold tracking-[0.06em] text-muted uppercase">
                <th scope="col" class="px-4 py-2.5">
                  {t("Name")}
                </th>
                <th scope="col" class="px-4 py-2.5">
                  {t("Username")}
                </th>
                <th scope="col" class="w-14">
                  <span class="sr-only">{t("Remove")}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              <For each={props.moderators}>
                {(moderator) => (
                  <tr class="border-t-[1.5px] border-line odd:bg-well even:bg-card">
                    <td class="truncate px-4 py-2 font-semibold">{moderator.name}</td>
                    <td class="truncate px-4 py-2 text-ink-2">{moderator.login}</td>
                    <td class="py-1 pr-2">
                      <button
                        type="button"
                        class="ml-auto flex size-10 items-center justify-center rounded-xl text-danger transition-colors hover:bg-danger-bg disabled:opacity-50"
                        disabled={busy()}
                        aria-label={t("Remove {name}", { name: moderator.name })}
                        title={t("Remove")}
                        onClick={() =>
                          void run(() =>
                            removeModerator(props.session, props.platform, moderator.id),
                          )
                        }
                      >
                        <Icon name="trash" class="size-5" />
                      </button>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
      <form class="flex flex-wrap gap-3" onSubmit={(event) => void add(event)}>
        <input
          type="text"
          value={username()}
          onInput={(event) => setUsername(event.currentTarget.value)}
          placeholder={placeholder()}
          aria-label={placeholder()}
          maxLength={50}
          autocomplete="off"
          autocapitalize="off"
          spellcheck={false}
          class={`${inputClass} min-w-0 flex-1`}
        />
        <button type="submit" class={buttonClass} disabled={busy() || !username().trim()}>
          {t("Add")}
        </button>
      </form>
      <Show when={error()}>
        {(error) => (
          <Notice status onClose={() => setError(null)}>
            {error()}
          </Notice>
        )}
      </Show>
    </>
  );
}

const moderationErrorText = (error: unknown) =>
  error instanceof BackendError && error.status < 500 && error.message
    ? error.message
    : t("Something went wrong. Try again.");

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
  tips: Feature<Tips> | null;
  currency: string;
  busy: boolean;
  onSave: (changes: { tips: Feature<Tips> }) => Promise<void>;
}) {
  const initial = () => ({
    enabled: isOn(props.tips),
    amounts: (props.tips?.amounts ?? [200, 500, 1000]).map((amount) => (amount / 100).toFixed(2)),
  });
  const [enabled, setEnabled] = createSignal(initial().enabled);
  const [amounts, setAmounts] = createSignal(initial().amounts);
  const tips = (form: ReturnType<typeof initial>): Feature<Tips> => ({
    enabled: form.enabled,
    amounts: amountsOf(form.amounts),
  });
  const current = () => tips({ enabled: enabled(), amounts: amounts() });
  const save = createSave(
    () => props.busy,
    () => !same(current(), tips(initial())),
    () => props.onSave({ tips: current() }),
    (error) => paymentErrorText(error, t("Enter amounts from 0.50 to 100000.")),
  );

  return (
    <form class={cardClass} onSubmit={save.submit}>
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
      <SaveFooter save={save} signInAgain />
    </form>
  );
}

function SpinSettings(props: {
  spin: Feature<Spin> | null;
  currency: string;
  busy: boolean;
  onSave: (changes: { spin: Feature<Spin> }) => Promise<void>;
}) {
  const initial = () => ({
    enabled: isOn(props.spin),
    options: (
      props.spin?.options ?? [
        { spins: 1, price: 500 },
        { spins: 2, price: 800 },
        { spins: 5, price: 1500 },
      ]
    ).map((option) => ({ spins: String(option.spins), price: (option.price / 100).toFixed(2) })),
  });
  const [enabled, setEnabled] = createSignal(initial().enabled);
  const [options, setOptions] = createSignal(initial().options);

  const changeOption = (index: number, change: Partial<{ spins: string; price: string }>) =>
    setOptions(options().map((old, at) => (at === index ? { ...old, ...change } : old)));
  const spin = (form: ReturnType<typeof initial>): Feature<Spin> => ({
    enabled: form.enabled,
    options: form.options
      .filter((option) => option.spins.trim() && option.price.trim())
      .map((option) => ({
        spins: Number.parseInt(option.spins),
        price: hundredths(option.price),
      })),
  });
  const current = () => spin({ enabled: enabled(), options: options() });
  const save = createSave(
    () => props.busy,
    () => !same(current(), spin(initial())),
    () => props.onSave({ spin: current() }),
    (error) => paymentErrorText(error, t("Enter 1 to 10 spins for 0.50 to 100000 per price.")),
  );

  return (
    <form class={cardClass} onSubmit={save.submit}>
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
      <SaveFooter save={save} signInAgain />
    </form>
  );
}

function SendForm(props: {
  title: string;
  description?: string;
  label: string;
  send: (message: string) => Promise<unknown>;
  onSignOut: () => void;
}) {
  const [message, setMessage] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [sent, flashSent] = createFlash(3000);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (sending()) {
      return;
    }
    setError(null);
    setSending(true);
    try {
      await props.send(message());
      flashSent();
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      setError(moderationErrorText(error));
    } finally {
      setSending(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">{props.title}</h2>
      <Show when={props.description}>
        <p class="text-ink-2">{props.description}</p>
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
            {sending() ? t("Sending…") : sent() ? t("Queued for Moblin") : props.label}
          </span>
        </button>
        <Show when={error()}>
          {(error) => (
            <Notice status onClose={() => setError(null)}>
              {error()}
            </Notice>
          )}
        </Show>
      </div>
    </form>
  );
}

const testTexts: Record<RecentKind, { description: string; label: string }> = {
  tip: {
    description: t("Send yourself a tip, to check that tips show up in Moblin."),
    label: t("Send test tip"),
  },
  spin: {
    description: t("Spin your wheel in Moblin, to check that it works."),
    label: t("Send test spin"),
  },
};

function TestSend(props: {
  kind: RecentKind;
  session: string;
  page?: string;
  onSignOut: () => void;
}) {
  return (
    <SendForm
      title={t("Test it")}
      description={testTexts[props.kind].description}
      label={testTexts[props.kind].label}
      onSignOut={props.onSignOut}
      send={(message) => sendTest(props.session, props.kind, { message }, props.page)}
    />
  );
}

function Admin(props: { session: string; account: Account; page?: string; onSignOut: () => void }) {
  const [account, setAccount] = createSignal(props.account);
  const [settings, setSettings] = createSignal(props.account.settings);
  const [busy, setBusy] = createSignal(false);
  const moderating = () => props.page !== undefined;
  const { recent, loadRecent, loadMore } = createRecent(
    (kind, limit, before) => fetchRecent(props.session, props.page, kind, limit, before),
    (error) => {
      if (signedOut(error)) {
        props.onSignOut();
      }
    },
  );

  const guard = async <T,>(promise: Promise<T>) => {
    try {
      return await promise;
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      throw error;
    }
  };

  const busyWith = async <T,>(call: () => Promise<T>, apply: (value: T) => void) => {
    setBusy(true);
    try {
      apply(await guard(call()));
    } finally {
      setBusy(false);
    }
  };

  const resend = async (item: Recent) => {
    await guard(resendRecent(props.session, item.id, props.page));
    await loadRecent();
  };

  const changeImage = (image: File | null) =>
    busyWith(
      () =>
        image
          ? uploadImage(props.session, image, props.page)
          : removeImage(props.session, props.page),
      setSettings,
    );

  onMount(() => {
    void loadRecent();
    onCleanup(
      poll(async () => {
        void loadRecent();
        try {
          const account = await fetchAccount(props.session, props.page);
          setAccount(account);
          setSettings(account.settings);
        } catch (error) {
          if (signedOut(error)) {
            props.onSignOut();
          }
        }
      }, 30_000),
    );
  });

  const save = (changes: Partial<SettingsChanges>) =>
    busyWith(
      () => saveAccountSettings(props.session, { ...settings(), ...changes }, props.page),
      setSettings,
    );

  const payments = () => settings().paypal || settings().stripe;
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
      shown: () => moderating() || payments() || isOn(settings().tips),
      status: () => onOff(isOn(settings().tips)),
    },
    {
      id: "wheel",
      label: t("Wheel of luck"),
      icon: "wheel",
      shown: () => true,
      status: () => onOff(isOn(settings().spin) || isOn(settings().subSpins)),
    },
    {
      id: "moderators",
      label: t("Moderators"),
      icon: "shield",
      shown: () => !moderating() && (account().twitch !== null || account().kick !== null),
      status: () => null,
    },
    {
      id: "payments",
      label: t("Payments"),
      icon: "wallet",
      shown: payments,
      status: () => connectedOrNot(settings().payable),
    },
    {
      id: "moblin",
      label: "Moblin",
      icon: "smartphone",
      shown: () => !moderating(),
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
    if (id !== active()) {
      scrollTo({ top: 0, behavior: "instant" });
    }
    setChosen(id);
    history.replaceState(null, "", `${location.pathname}${location.search}#${id}`);
  };
  const current = () => visible().find((section) => section.id === active())!;
  const [menuOpen, setMenuOpen] = createSignal(false);
  const paid = (configured: boolean) => settings().payable || configured;

  return (
    <div class="grid gap-6 lg:grid-cols-[260px_minmax(0,1fr)] lg:items-start">
      <nav
        aria-label={t("Dashboard")}
        class="overflow-hidden rounded-2xl border border-line bg-card lg:sticky lg:top-[calc(4.5rem+1px+3rem)] lg:flex lg:max-h-[calc(100dvh-4.5rem-1px-6rem)] lg:flex-col lg:overflow-y-auto lg:rounded-none lg:border-0 lg:bg-transparent"
      >
        <button
          type="button"
          aria-expanded={menuOpen()}
          onClick={() => setMenuOpen(!menuOpen())}
          class="flex w-full items-center gap-3 px-4 py-3 text-left text-ink lg:hidden"
        >
          <Icon name={current().icon} class="size-5 shrink-0" />
          <span class="font-display text-[17px] font-semibold">{current().label}</span>
          <Show when={current().status()}>
            {(status) => (
              <span class="ml-auto flex items-center gap-1.5 pl-1 text-[13px] text-muted">
                <span
                  class={`size-2 shrink-0 rounded-full ${status().on ? "bg-leaf" : "bg-muted/50"}`}
                />
                {status().text}
              </span>
            )}
          </Show>
          <Icon
            name="chevron-down"
            class={`size-5 shrink-0 text-muted transition-transform ${menuOpen() ? "rotate-180" : ""} ${current().status() ? "" : "ml-auto"}`}
          />
        </button>
        <div
          class="divide-line flex flex-col divide-y border-t border-line lg:contents"
          classList={{ hidden: !menuOpen() }}
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
                class="flex items-center gap-3 px-4 py-2.5 text-left transition-colors lg:rounded-2xl lg:border-[1.5px] lg:py-3 lg:whitespace-nowrap"
                classList={{
                  "bg-ghost text-ink lg:border-line lg:bg-card": active() === section.id,
                  "text-ink-2 hover:bg-ghost-hover hover:text-ink lg:border-transparent lg:hover:bg-ghost":
                    active() !== section.id,
                }}
              >
                <Icon name={section.icon} class="size-5 shrink-0" />
                <span class="font-display text-[16px] font-semibold lg:text-[17px]">
                  {section.label}
                </span>
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
        </div>
      </nav>
      <div class="min-w-0">
        <div hidden={active() !== "page"}>
          <YourPage
            account={account()}
            page={settings()}
            busy={busy()}
            moderating={moderating()}
            onSave={save}
            onImage={changeImage}
          />
        </div>
        <div hidden={active() !== "tips"} class="flex flex-col gap-6">
          <Show
            when={paid(isOn(settings().tips))}
            fallback={
              <NeedsPayments
                title={t("Tips")}
                available={payments()}
                streamer={moderating() ? account().name : null}
                onConnect={() => select("payments")}
              />
            }
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
            page={props.page}
            onSignOut={props.onSignOut}
          />
          <Latest
            kind="tip"
            list={recent()?.tip}
            onMore={() => loadMore("tip")}
            onResend={resend}
          />
        </div>
        <div hidden={active() !== "wheel"} class="flex flex-col gap-6">
          <Show
            when={paid(isOn(settings().spin))}
            fallback={
              <NeedsPayments
                title={t("Wheel of luck")}
                available={payments()}
                streamer={moderating() ? account().name : null}
                onConnect={() => select("payments")}
              />
            }
          >
            <SpinSettings
              spin={settings().spin}
              currency={settings().currency}
              busy={busy()}
              onSave={save}
            />
          </Show>
          <Show when={account().twitch || account().kick}>
            <SubscriberSettings
              tiers={settings().subSpins}
              twitch={account().twitch !== null}
              kick={account().kick !== null}
              busy={busy()}
              onSave={(subSpins) => save({ subSpins })}
            />
          </Show>
          <TestSend
            kind="spin"
            session={props.session}
            page={props.page}
            onSignOut={props.onSignOut}
          />
          <Latest
            kind="spin"
            list={recent()?.spin}
            onMore={() => loadMore("spin")}
            onResend={resend}
          />
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
            <Show when={settings().payable}>
              <CurrencySettings
                currency={settings().currency}
                locked={isOn(settings().tips) || isOn(settings().spin)}
                busy={busy()}
                onSave={save}
              />
            </Show>
          </div>
        </Show>
        <Show when={!moderating() && (account().twitch || account().kick)}>
          <div hidden={active() !== "moderators"}>
            <ModeratorSettings
              session={props.session}
              account={account()}
              onAccount={setAccount}
              onSignOut={props.onSignOut}
            />
          </div>
        </Show>
        <div hidden={active() !== "moblin"}>
          <MoblinSettings
            account={account()}
            busy={busy()}
            onNewKey={() => busyWith(() => newMoblinKey(props.session), setAccount)}
            onRename={(device, name) =>
              busyWith(() => renameDevice(props.session, device, name), setAccount)
            }
          />
        </div>
      </div>
    </div>
  );
}

function Modal(props: { ref: (dialog: HTMLDialogElement) => void; children: JSX.Element }) {
  let dialog: HTMLDialogElement | undefined;
  return (
    <dialog
      ref={(element) => {
        dialog = element;
        props.ref(element);
      }}
      class="m-auto w-[calc(100%-2rem)] max-w-md rounded-3xl border-[1.5px] border-line bg-card p-6 text-ink backdrop:bg-black/60 sm:p-8"
      onClick={(event) => {
        if (event.target === dialog) {
          dialog?.close();
        }
      }}
    >
      {props.children}
    </dialog>
  );
}

function Moderated(props: { session: string; channel: Moderating; onSignOut: () => void }) {
  const [account, setAccount] = createSignal<Account | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    try {
      setAccount(await fetchAccount(props.session, props.channel.id));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      setError(
        error instanceof BackendError && error.status === 403
          ? t("{name} has removed you as a moderator.", { name: props.channel.name })
          : t("Something went wrong. Try again."),
      );
    }
  });

  return (
    <Show
      when={account()}
      fallback={
        <Show when={error()} fallback={<Spinner />}>
          {(error) => <Notice status>{error()}</Notice>}
        </Show>
      }
    >
      {(account) => (
        <Admin
          session={props.session}
          account={account()}
          page={props.channel.id}
          onSignOut={props.onSignOut}
        />
      )}
    </Show>
  );
}

const moderatedChannelKey = "moderatedChannel";

const channelKey = (channel: Moderating) => `${channel.platform}/${channel.login}`;

function readModeratedChannel() {
  const fromUrl = location.pathname
    .slice(import.meta.env.BASE_URL.length)
    .match(/^dashboard\/(twitch|kick)\/([^/]+)\/?$/);
  if (fromUrl) {
    return `${fromUrl[1]}/${decodeURIComponent(fromUrl[2])}`;
  }
  try {
    const stored = sessionStorage.getItem(moderatedChannelKey) ?? "";
    return stored && !stored.includes("/") ? `twitch/${stored}` : stored;
  } catch {
    return "";
  }
}

const onOff = (on: boolean) => ({ on, text: on ? t("On") : t("Off") });

const connectedOrNot = (on: boolean) => ({ on, text: on ? t("Connected") : t("Not connected") });

function NeedsPayments(props: {
  title: string;
  available: boolean;
  streamer: string | null;
  onConnect: () => void;
}) {
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{props.title}</h2>
      <Show
        when={props.streamer === null}
        fallback={
          <p class="text-ink-2">
            {t(
              "{name} has to connect PayPal or Stripe first, so the money goes straight to them.",
              {
                name: props.streamer!,
              },
            )}
          </p>
        }
      >
        <Show
          when={props.available}
          fallback={<p class="text-ink-2">{t("Payments are not available right now.")}</p>}
        >
          <p class="text-ink-2">
            {t("Connect PayPal or Stripe first, so the money goes straight to you.")}
          </p>
          <div>
            <button type="button" class={buttonClass} onClick={() => props.onConnect()}>
              {t("Go to payments")}
            </button>
          </div>
        </Show>
      </Show>
    </div>
  );
}

function QrCode(props: { text: string; label: string }) {
  const qr = createMemo(() => encode(props.text, { border: 4 }));
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
  onNewKey: () => Promise<void>;
  onRename: (device: string, name: string) => Promise<void>;
}) {
  let qrDialog: HTMLDialogElement | undefined;
  const [error, setError] = createSignal<string | null>(null);
  const [renaming, setRenaming] = createSignal<string | null>(null);
  const [name, setName] = createSignal("");
  const [copied, flashCopied] = createFlash(1500);
  const key = () => props.account.key ?? "";
  const deepLink = () => {
    const settings = { moblinCloud: { enabled: true, key: key() } };
    return `moblin://?${encodeURIComponent(JSON.stringify(settings))}`;
  };
  const copyDeepLink = async () => {
    if (await copyText(deepLink())) {
      flashCopied();
    }
  };
  const newKey = async () => {
    if (
      !confirm(
        t(
          "Every device disconnects and the current Moblin Cloud key stops working. Paste the new one in Moblin on each device to reconnect.",
        ),
      )
    ) {
      return;
    }
    await run(props.onNewKey);
  };
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
  const startRenaming = (device: Device) => {
    setName(device.name);
    setRenaming(device.id);
  };
  const rename = (event: SubmitEvent, device: string) => {
    event.preventDefault();
    void run(async () => {
      await props.onRename(device, name().trim());
      setRenaming(null);
    });
  };
  const gets = (device: Device) =>
    device.connected
      ? [device.wheel ? t("Wheel of luck") : null, t("Tips")]
          .filter((text) => text !== null)
          .join(" · ")
      : t("Last seen {time}", { time: timeAgo(device.lastSeen ?? 0) });

  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">Moblin</h2>
      <p class="text-ink-2">
        {t(
          "Use the same key on up to 5 devices running Moblin. In Moblin, go to Settings → Moblin Cloud, paste the key and turn on Enabled.",
        )}
      </p>
      <div class="flex flex-col gap-2">
        <Value label={t("Moblin Cloud key")} text={key()} secret />
        <p class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 text-[15px] text-muted">
          <span>{t("Keep it to yourself. Anyone with it can connect as your Moblin.")}</span>
          <button
            type="button"
            class="font-semibold text-danger hover:underline disabled:opacity-60"
            disabled={props.busy}
            onClick={() => void newKey()}
          >
            {t("Reset key")}
          </button>
        </p>
      </div>
      <p class="text-[15px] text-ink-2">
        {t(
          "Or set it all up at once on the device running Moblin: scan the QR code with its camera or open the deep link there.",
        )}
      </p>
      <div class="flex flex-wrap items-center gap-2">
        <button type="button" class={smallButtonClass} onClick={() => qrDialog?.showModal()}>
          <Icon name="eye" class="size-4" />
          {t("Show QR code")}
        </button>
        <button type="button" class={smallButtonClass} onClick={copyDeepLink}>
          <Icon name={copied() ? "check" : "copy"} class="size-4" />
          {copied() ? t("Copied") : t("Copy deep link")}
        </button>
      </div>
      <Show when={error()}>
        {(error) => (
          <Notice status onClose={() => setError(null)}>
            {error()}
          </Notice>
        )}
      </Show>
      <div class="flex flex-col gap-2">
        <h3 class="text-[15px] font-semibold text-ink-2">{t("Devices")}</h3>
        <Show
          when={props.account.devices.length > 0}
          fallback={<p class="text-muted">{t("No device has connected yet.")}</p>}
        >
          <ul class="flex flex-col divide-y-[1.5px] divide-line rounded-2xl border-[1.5px] border-line">
            <For each={props.account.devices}>
              {(device) => (
                <li class="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3">
                  <span
                    class={`size-2.5 shrink-0 rounded-full ${device.connected ? "bg-leaf" : "bg-muted"}`}
                    title={device.connected ? t("Connected") : t("Not connected")}
                  />
                  <Show
                    when={renaming() === device.id}
                    fallback={
                      <>
                        <span class="min-w-0 flex-1 truncate font-semibold">{device.name}</span>
                        <span class="text-[15px] text-muted">{gets(device)}</span>
                        <button
                          type="button"
                          class="text-[15px] font-semibold text-leaf hover:underline disabled:opacity-60"
                          disabled={props.busy}
                          onClick={() => startRenaming(device)}
                        >
                          {t("Rename")}
                        </button>
                      </>
                    }
                  >
                    <form
                      class="flex min-w-0 flex-1 flex-wrap items-center gap-2"
                      onSubmit={(event) => rename(event, device.id)}
                    >
                      <input
                        type="text"
                        maxLength={30}
                        aria-label={t("Device name")}
                        placeholder={t("Device name")}
                        value={name()}
                        onInput={(event) => setName(event.currentTarget.value)}
                        class={`${inputClass} max-w-64`}
                      />
                      <button
                        type="submit"
                        class={smallButtonClass}
                        disabled={props.busy || name().trim() === ""}
                      >
                        {t("Save")}
                      </button>
                      <button
                        type="button"
                        class={smallButtonClass}
                        onClick={() => setRenaming(null)}
                      >
                        {t("Cancel")}
                      </button>
                    </form>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </Show>
        <p class="text-[15px] text-muted">
          {t(
            "Wheel spins go to the first device with a wheel picked in Moblin's Moblin Cloud settings. Tips go to every device.",
          )}
        </p>
      </div>
      <Modal ref={(element) => (qrDialog = element)}>
        <div class="flex flex-col items-center gap-5">
          <h2 class="font-display text-2xl font-bold">Moblin Cloud</h2>
          <QrCode text={deepLink()} label={t("QR code that sets up Moblin Cloud in Moblin")} />
          <p class="text-center text-[17px] text-ink-2">
            {t("Scan it with the camera of the device running Moblin to set up Moblin Cloud.")}
          </p>
          <button type="button" class={ghostButtonClass} onClick={() => qrDialog?.close()}>
            {t("Close")}
          </button>
        </div>
      </Modal>
    </div>
  );
}

function GoPremium(props: { ref: (dialog: HTMLDialogElement) => void }) {
  let dialog: HTMLDialogElement | undefined;

  return (
    <>
      <Modal
        ref={(element) => {
          dialog = element;
          props.ref(element);
        }}
      >
        <div class="flex flex-col gap-5">
          <h2 class="font-display text-2xl font-bold">{t("Go premium")}</h2>
          <p class="text-[17px] text-ink-2">
            {t(
              "With a premium account you can add a banner image, pick any color and choose a font for your tip page.",
            )}
          </p>
          <p class="text-[17px] text-ink-2">
            {tRich("To go premium, join the {discord} and request it in the {channel} channel.", {
              discord: (
                <a
                  href={links.discordPremium}
                  target="_blank"
                  rel="noopener"
                  class="text-leaf hover:text-leaf-bright"
                >
                  {t("Moblin Discord")}
                </a>
              ),
              channel: <span class="font-semibold text-ink">#dev-cave</span>,
            })}
          </p>
          <div class="flex flex-wrap gap-3">
            <a href={links.discordPremium} target="_blank" rel="noopener" class={buttonClass}>
              <Icon name="discord" class="size-5" />
              {t("Join Discord")}
            </a>
            <button type="button" class={ghostButtonClass} onClick={() => dialog?.close()}>
              {t("Close")}
            </button>
          </div>
        </div>
      </Modal>
    </>
  );
}

const smallButtonClass =
  "inline-flex h-10 items-center justify-center gap-2 rounded-full border-[1.5px] border-ghost-line bg-ghost px-4 font-display text-[15px] font-semibold whitespace-nowrap text-ink transition-colors hover:bg-ghost-hover disabled:opacity-60";

function PickedAvatar(props: { picked: { name: string; avatar: string | null } }) {
  return (
    <span aria-hidden="true" class="pointer-events-none absolute top-1/2 left-1.5 -translate-y-1/2">
      <Show
        when={props.picked.avatar}
        fallback={
          <span class="flex size-8 items-center justify-center rounded-full bg-moss font-display text-sm font-semibold text-white">
            {props.picked.name.slice(0, 1).toUpperCase()}
          </span>
        }
      >
        {(avatar) => (
          <img
            src={avatar()}
            alt=""
            referrerpolicy="no-referrer"
            class="size-8 rounded-full bg-ghost object-cover"
          />
        )}
      </Show>
    </span>
  );
}

export default function Dashboard() {
  const { session, loading, signInFailed, signOut } = createAccountSession();

  const [chosenChannel, setChosenChannel] = createSignal(readModeratedChannel());
  const chooseChannel = (key: string) => {
    setChosenChannel(key);
    const [platform, login] = key.split("/", 2);
    const path = login
      ? `${pages.dashboard}${platform}/${encodeURIComponent(login)}`
      : pages.dashboard;
    history.replaceState(null, "", `${asset(path)}${location.search}${location.hash}`);
    try {
      sessionStorage.setItem(moderatedChannelKey, key);
    } catch {}
  };
  const channel = (account: Account) => {
    const [platform, login] = chosenChannel().split("/", 2) as [Platform, string | undefined];
    return (
      (login ? account.moderating.find((channel) => channel[platform]?.login === login) : null) ??
      account.moderating.find((channel) => channelKey(channel) === chosenChannel()) ??
      null
    );
  };

  let premiumDialog: HTMLDialogElement | undefined;

  return (
    <>
      <CloudHeader premium={session()?.account.premium}>
        <Show
          when={session()}
          fallback={
            <Show when={!loading()}>
              <span class="ml-auto flex items-center gap-2">
                <PlatformSignIn platform="twitch" />
                <PlatformSignIn platform="kick" />
              </span>
            </Show>
          }
        >
          {(current) => (
            <div class="ml-auto flex min-w-0 items-center gap-3">
              <Show when={!current().account.premium}>
                <GoPremium ref={(element) => (premiumDialog = element)} />
              </Show>
              <Show when={current().account.moderating.length > 0}>
                <div class="relative min-w-0">
                  <PickedAvatar picked={channel(current().account) ?? current().account} />
                  <select
                    aria-label={t("Streamer")}
                    class={`${ghostButtonClass} h-11! w-full cursor-pointer appearance-none truncate pr-10! pl-12! text-base! max-sm:w-11 max-sm:px-0! max-sm:text-transparent max-sm:[&>option]:text-ink`}
                    value={(() => {
                      const picked = channel(current().account);
                      return picked ? channelKey(picked) : "";
                    })()}
                    onChange={(event) => chooseChannel(event.currentTarget.value)}
                  >
                    <option value="">{t("You")}</option>
                    <For each={current().account.moderating}>
                      {(channel) => (
                        <option value={channelKey(channel)}>
                          {channel.platform === "kick"
                            ? t("{name} on Kick", { name: channel.name })
                            : channel.name}
                        </option>
                      )}
                    </For>
                  </select>
                  <Icon
                    name="chevron-down"
                    class="pointer-events-none absolute top-1/2 right-4 size-4 -translate-y-1/2 text-ink-2 max-sm:hidden"
                  />
                </div>
              </Show>
              <Show when={current().account.admin}>
                <a
                  href={asset(pages.admin)}
                  class={`${ghostButtonClass} h-11! px-5! text-base! max-sm:w-11 max-sm:px-0!`}
                  title={t("Admin")}
                >
                  <Icon name="chart" class="size-5 sm:hidden" />
                  <span class="max-sm:sr-only">{t("Admin")}</span>
                </a>
              </Show>
              <AccountMenu
                name={current().account.name}
                avatar={current().account.avatar}
                items={[
                  ...(current().account.premium
                    ? []
                    : [
                        {
                          label: t("Go premium"),
                          icon: "star" as const,
                          onClick: () => premiumDialog?.showModal(),
                        },
                      ]),
                  { label: t("Account"), icon: "user", href: asset(pages.account) },
                ]}
                onSignOut={signOut}
              />
            </div>
          )}
        </Show>
      </CloudHeader>
      <main>
        <Show when={loading() || session()} fallback={<CloudLanding failed={signInFailed()} />}>
          <Band tight={session() !== null}>
            <Show when={!loading()} fallback={<Spinner />}>
              <Show when={session()} keyed>
                {(current) => (
                  <Show
                    when={channel(current.account)}
                    keyed
                    fallback={
                      <Admin
                        session={current.session}
                        account={current.account}
                        onSignOut={signOut}
                      />
                    }
                  >
                    {(channel) => (
                      <Moderated session={current.session} channel={channel} onSignOut={signOut} />
                    )}
                  </Show>
                )}
              </Show>
            </Show>
          </Band>
        </Show>
      </main>
    </>
  );
}
