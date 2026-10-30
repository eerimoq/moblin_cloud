import {
  createEffect,
  createMemo,
  createSignal,
  createUniqueId,
  For,
  Index,
  type JSX,
  lazy,
  on,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { encode } from "uqr";
import { asset } from "../asset";
import { createFlash } from "../flash";
import { subscribeChanges } from "../sse";
import { copyText } from "../components/CopyButton";
import Icon, { kindIcons } from "../components/Icon";
import Modal, { ask } from "../components/Modal";
import { createRecent, Latest, timeAgo } from "../components/Latest";
import SectionMenu, {
  connectedOrNot,
  createSectionChoice,
  type Section,
} from "../components/SectionMenu";
import {
  Avatar,
  channelKey,
  assistedChannelKey,
  accountPage,
  adminPage,
  monitorPage,
  ownPages,
  PagePicker,
  rememberChannel,
  StreamerPicker,
} from "../components/HeaderPickers";
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
  TermsLink,
  Value,
} from "../components/PageParts";
import {
  addAssistant,
  createAccountSession,
  type Assistant,
  removeAssistant,
  setAssistantEditTips,
  setAssistantGoLive,
  setAssistantPreview,
  setAssistantRemoteControl,
  type Assisting,
  checkPayPal,
  checkStripe,
  connectPayPal,
  connectStripe,
  disconnectPayPal,
  disconnectStripe,
  fetchAccount,
  pageEventsUrl,
  fetchRecent,
  newMoblinKey,
  removeDevice,
  renameDevice,
  removeImage,
  saveAccountSettings,
  resendRecent,
  sendTest,
  stripeCountries,
  uploadImage,
  type Access,
  type Account,
  type LinkResult,
  type Device,
  type Recent,
  type RecentKind,
  type SettingsChanges,
} from "../data/account";
import { BackendError, isForbidden, isSignedOut } from "../data/backend";
import { deviceUrl, links, monitorUrl, pages } from "../data/links";
import {
  accentAttributes,
  accents,
  amountText,
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
  type Effects,
  type Feature,
  type Perk,
  type Page,
  type Settings,
  type Spin,
  type Tiers,
  type Tips,
} from "../data/remoteControl";
import { effectName, effects } from "../data/effects";
import { language, t, tRich } from "../i18n";
import { sameData } from "../same";
import CloudLanding from "../components/CloudLanding";

const DeviceView = lazy(() => import("./Device"));
const Monitor = lazy(() => import("./Monitor"));
const AdminView = lazy(() => import("./Admin"));
const AccountView = lazy(() => import("./Account"));

export const [fullWindow, setFullWindow] = createSignal(false);

const urlOf = (page: string, device: string) =>
  device === monitorPage
    ? monitorUrl(page)
    : device === adminPage
      ? asset(pages.admin)
      : device === accountPage
        ? asset(pages.account)
        : deviceUrl(page, device);

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

function createFeatureSave<Form extends { enabled: boolean }, Value>(options: {
  initial: () => Form;
  value: (form: Form) => Value;
  busy: () => boolean;
  save: (value: Value) => Promise<void>;
  errorText: (error: unknown) => string;
}) {
  const [form, setForm] = createSignal(options.initial());
  const change = (changes: Partial<Form>) => setForm(() => ({ ...form(), ...changes }));
  const current = () => options.value(form());
  const save = createSave(
    options.busy,
    () => !sameData(current(), options.value(options.initial())),
    () => options.save(current()),
    options.errorText,
  );
  return { form, change, save };
}

function EnableToggle(props: {
  checked: boolean;
  onChange: (on: boolean) => void;
  children: JSX.Element;
}) {
  return (
    <label class="flex items-center gap-3 text-ink-2">
      <input
        type="checkbox"
        checked={props.checked}
        onChange={(event) => props.onChange(event.currentTarget.checked)}
        class="size-5 accent-moss"
      />
      {props.children}
    </label>
  );
}

function RemoveRowButton(props: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      class="flex size-10 shrink-0 items-center justify-center rounded-xl text-muted hover:bg-ghost hover:text-ink"
      aria-label={props.label}
      onClick={() => props.onClick()}
    >
      <Icon name="close" class="size-5" />
    </button>
  );
}

function AddRowButton(props: { rows: number; onClick: () => void; children: JSX.Element }) {
  return (
    <div class="mt-1">
      <button
        type="button"
        class={ghostButtonClass}
        disabled={props.rows >= 5}
        onClick={() => props.onClick()}
      >
        {props.children}
      </button>
    </div>
  );
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
      <Show when={props.signInAgain && isForbidden(props.save.failure()?.error)}>
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
        ? t("Turn off all tips to change the currency.")
        : paymentErrorText(error, t("Pick one of the currencies.")),
  );

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">{t("Currency")}</h2>
      <p class="text-ink-2">{t("All tips are in this currency.")}</p>
      <Show when={props.locked}>
        <p class="text-ink-2">{t("Turn off all tips to change the currency.")}</p>
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
    if (isSignedOut(error)) {
      props.onSignOut();
      return;
    }
    setError(provider.errorText(error));
    setSignInAgain(isForbidden(error));
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
    if (await ask(provider.confirmText, t("Disconnect"))) {
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
      confirmText: t("Viewers can't tip you with PayPal until you connect a PayPal account again."),
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
          {t("PayPal is in sandbox mode. Tips are for testing only and no real money is moved.")}
        </Notice>
      </Show>
      <Show
        when={props.settings.merchantId}
        fallback={
          <p class="text-ink-2">
            {tRich(
              "Connect your PayPal account to let viewers send you tips. You sign in or sign up " +
                "on PayPal with a personal or business " +
                "account (in some countries PayPal asks for a business account). By connecting PayPal you " +
                "agree to the {terms}.",
              {
                terms: <TermsLink leaf />,
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
      confirmText: t("Viewers can't tip you with Stripe until you connect a Stripe account again."),
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
          {t("Stripe is in test mode. Tips are for testing only and no real money is moved.")}
        </Notice>
      </Show>
      <Show
        when={props.settings.stripeAccountId}
        fallback={
          <p class="text-ink-2">
            {tRich(
              "Connect a Stripe account to let viewers tip with a card, Apple Pay or Google Pay. You " +
                "sign up or sign in on Stripe, which asks about you and your bank account so it can send " +
                "your tips to you. By connecting Stripe you agree to the {terms}.",
              {
                terms: <TermsLink leaf />,
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

const setupGuide = `${links.docs}#guides/moblin-cloud`;

function GuideLink(props: { text: string }) {
  return (
    <a href={setupGuide} class="text-leaf hover:text-leaf-bright">
      {props.text}
    </a>
  );
}

function GetStarted(props: {
  account: Account;
  settings: Settings;
  payments: boolean;
  onSelect: (section: string) => void;
}) {
  const spin = () => isOn(props.settings.spin);
  const steps = () =>
    [
      {
        shown: props.payments,
        done: props.settings.payable,
        title: t("Connect PayPal or Stripe"),
        text: t("The money goes straight to your account."),
        section: "payments",
      },
      {
        shown: true,
        done:
          isOn(props.settings.tips) ||
          isOn(props.settings.spin) ||
          isOn(props.settings.effects) ||
          isOn(props.settings.subSpins) ||
          isOn(props.settings.subEffects),
        title: t("Choose how viewers tip you"),
        text: t("Turn on at least one way to tip and set its amounts."),
        section: props.payments ? "tips" : "wheel",
      },
      {
        shown: true,
        done: props.account.devices.length > 0,
        title: t("Connect Moblin"),
        text: t("Scan the QR code with the device running Moblin."),
        section: "moblin",
      },
      {
        shown: spin(),
        done: props.account.devices.some((device) => device.wheel),
        title: t("Pick your wheel in Moblin"),
        text: t(
          "Create a wheel of luck widget and pick it under Settings → Moblin Cloud → Wheel of luck.",
        ),
        section: "moblin",
      },
    ].filter((step) => step.shown);

  return (
    <Show when={steps().some((step) => !step.done)}>
      <div class={cardClass}>
        <h2 class="text-[23px] font-semibold">{t("Get started")}</h2>
        <ol class="flex flex-col gap-3">
          <For each={steps()}>
            {(step, index) => (
              <li class="flex items-start gap-3">
                <span
                  class={`flex size-8 shrink-0 items-center justify-center rounded-full font-display font-bold ${
                    step.done ? "bg-moss text-white" : "bg-well text-leaf"
                  }`}
                >
                  <Show when={step.done} fallback={index() + 1}>
                    <Icon name="check" class="size-4" />
                  </Show>
                </span>
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class={`font-semibold ${step.done ? "text-muted line-through" : ""}`}>
                    {step.title}
                  </span>
                  <Show when={!step.done}>
                    <span class="text-[15px] text-ink-2">{step.text}</span>
                  </Show>
                </span>
                <Show when={!step.done}>
                  <button
                    type="button"
                    class={`${smallButtonClass} shrink-0`}
                    aria-label={step.title}
                    onClick={() => props.onSelect(step.section)}
                  >
                    {t("Go")}
                  </button>
                </Show>
              </li>
            )}
          </For>
        </ol>
        <p class="text-[15px] text-ink-2">
          {tRich("To show tips as alerts on stream, see the {guide}.", {
            guide: <GuideLink text={t("setup guide")} />,
          })}
        </p>
      </div>
    </Show>
  );
}

function YourPage(props: {
  account: Account;
  page: Settings;
  busy: boolean;
  assisting: boolean;
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
    () => !sameData(current(), initial()),
    () => props.onSave(current()),
    () => t("Something went wrong. Try again."),
  );

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">
        {props.assisting
          ? t("The tip page of {name}", { name: props.account.name })
          : t("Your tip page")}
      </h2>
      <p class="text-ink-2">{t("Share it with your viewers so they can tip you.")}</p>
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
  perk: Perk;
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
  const { form, change, save } = createFeatureSave({
    initial,
    value: (form): Feature<Tiers> => {
      const [tier1, tier2, tier3] = form.amounts.map((count) => Number.parseInt(count, 10) || 0);
      return { enabled: form.enabled, tier1, tier2, tier3 };
    },
    busy: () => props.busy,
    save: (tiers) => props.onSave(tiers),
    errorText: (error) =>
      paymentErrorText(
        error,
        props.perk === "spin"
          ? t("Enter 0 to 100 spins per tier, and more than 0 for at least one tier.")
          : t("Enter 0 to 100 effects per tier, and more than 0 for at least one tier."),
      ),
  });
  const enabled = () => form().enabled;
  const amounts = () => form().amounts;

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">
        {props.perk === "spin"
          ? props.twitch && !props.kick
            ? t("Spins for Twitch subscribers")
            : t("Spins for subscribers")
          : props.twitch && !props.kick
            ? t("Effects for Twitch subscribers")
            : t("Effects for subscribers")}
      </h2>
      <p class="text-ink-2">
        {props.perk === "spin"
          ? t(
              "Give your subscribers spins of the wheel every month, without a tip. Subscribers sign " +
                "in on your tip page, and their spins come back on the 1st of every " +
                "month. This works with or without tips to spin the wheel.",
            )
          : t(
              "Give your subscribers effects every month, without a tip. Subscribers sign in on " +
                "your tip page, and their effects come back on the 1st of every month. This " +
                "works with or without tips for effects.",
            )}
        <Show when={props.twitch && props.kick}>
          {" "}
          {t("Kick subscribers get the tier 1 amount.")}
        </Show>
      </p>
      <EnableToggle checked={enabled()} onChange={(enabled) => change({ enabled })}>
        {props.perk === "spin"
          ? t("Subscribers can spin the wheel without a tip")
          : t("Subscribers can play effects without a tip")}
      </EnableToggle>
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
                    change({
                      amounts: amounts().map((old, at) => (at === tier - 1 ? value : old)),
                    });
                  }}
                  class={`${inputClass} max-w-28`}
                />
              </label>
            )}
          </For>
        </div>
        <span class="text-[15px] text-muted">
          {props.perk === "spin"
            ? t("Spins per month for each tier.")
            : t("Effects per month for each tier.")}
        </span>
      </Show>
      <SaveFooter save={save} />
    </form>
  );
}

function AssistantSettings(props: {
  session: string;
  account: Account;
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const linked = () => platforms.filter((platform) => props.account[platform] !== null);
  return (
    <section class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Assistants")}</h2>
      <p class="text-ink-2">
        {t(
          "Assistants can see your dashboard, send tests and use the remote control. Turn on more below.",
        )}
      </p>
      <For each={linked()}>
        {(platform) => (
          <AssistantList
            session={props.session}
            platform={platform}
            titled={linked().length > 1}
            assistants={
              platform === "twitch" ? props.account.twitchAssistants : props.account.kickAssistants
            }
            onAccount={props.onAccount}
            onSignOut={props.onSignOut}
          />
        )}
      </For>
    </section>
  );
}

const tooltipClass =
  "pointer-events-none invisible absolute bottom-full left-0 z-10 mb-1 w-max max-w-[min(100%,18rem)] lg:max-w-72 rounded-xl border-[1.5px] border-line bg-band px-3 py-2 text-[14px] whitespace-normal opacity-0 shadow-lg transition-opacity group-hover:visible group-hover:opacity-100 group-has-[:focus-visible]:visible group-has-[:focus-visible]:opacity-100";

function AccessPicker(props: {
  access: Access;
  label: string;
  onChange: (access: Access) => Promise<boolean>;
}) {
  const help = createUniqueId();
  const [pending, setPending] = createSignal(false);
  const options: { value: Access; text: string }[] = [
    { value: "none", text: t("No access") },
    { value: "view", text: t("View only") },
    { value: "control", text: t("View and control") },
  ];
  return (
    <span class="group flex items-center lg:relative">
      <select
        value={props.access}
        disabled={pending()}
        aria-label={props.label}
        aria-describedby={help}
        onChange={async (event) => {
          const select = event.currentTarget;
          setPending(true);
          try {
            if (!(await props.onChange(select.value as Access))) {
              select.value = props.access;
            }
          } finally {
            setPending(false);
          }
        }}
        class="h-9 rounded-xl border-[1.5px] border-line bg-well px-3 text-[15px] text-ink focus:border-leaf focus:outline-none disabled:opacity-50"
      >
        <For each={options}>{(option) => <option value={option.value}>{option.text}</option>}</For>
      </select>
      <span id={help} role="tooltip" class={tooltipClass}>
        {t(
          "View only shows the device page, with the phone and the graphs, and lets them send the streamer messages. View and control also lets them use its buttons.",
        )}
      </span>
    </span>
  );
}

function PermissionBox(props: {
  checked: boolean;
  disabled?: boolean;
  text: string;
  label: string;
  help: string;
  onChange: (on: boolean) => Promise<boolean>;
}) {
  const help = createUniqueId();
  const [pending, setPending] = createSignal(false);
  return (
    <label class="group flex items-center gap-2 whitespace-nowrap has-[:disabled]:text-muted lg:relative">
      <input
        type="checkbox"
        checked={props.checked}
        disabled={pending() || props.disabled}
        aria-label={props.label}
        aria-describedby={help}
        onChange={async (event) => {
          const input = event.currentTarget;
          setPending(true);
          try {
            if (!(await props.onChange(input.checked))) {
              input.checked = props.checked;
            }
          } finally {
            setPending(false);
          }
        }}
        class="size-5 accent-moss"
      />
      {props.text}
      <span id={help} role="tooltip" class={tooltipClass}>
        {props.help}
      </span>
    </label>
  );
}

function PermissionRow(props: { title: string; children: JSX.Element }) {
  return (
    <div
      role="group"
      aria-label={props.title}
      class="relative flex flex-wrap items-center gap-x-5 gap-y-2"
    >
      <span class="w-36 text-[12px] font-semibold tracking-[0.06em] text-muted uppercase">
        {props.title}
      </span>
      {props.children}
    </div>
  );
}

function AssistantList(props: {
  session: string;
  platform: Platform;
  titled: boolean;
  assistants: Assistant[];
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const [username, setUsername] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  let latest = 0;
  const call = async (action: () => Promise<Account>) => {
    const request = ++latest;
    setError(null);
    try {
      const account = await action();
      if (request === latest) {
        props.onAccount(account);
      }
      return true;
    } catch (error) {
      if (isSignedOut(error)) {
        props.onSignOut();
      }
      setError(assistantErrorText(error));
      return false;
    }
  };
  const run = async (action: () => Promise<Account>) => {
    if (busy()) {
      return false;
    }
    setBusy(true);
    try {
      return await call(action);
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
    if (await run(() => addAssistant(props.session, props.platform, name))) {
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
      <Show when={props.assistants.length > 0}>
        <ul class="rounded-2xl border-[1.5px] border-line">
          <Index each={props.assistants}>
            {(assistant) => (
              <li class="flex items-start gap-3 border-line px-4 py-3 not-first:border-t-[1.5px] first:rounded-t-[14px] last:rounded-b-[14px] odd:bg-well even:bg-card">
                <div class="flex min-w-0 flex-1 flex-col gap-3">
                  <div class="flex min-w-0 items-center gap-3">
                    <Avatar picked={assistant()} class="size-10 shrink-0 text-base" />
                    <div class="min-w-0">
                      <div class="truncate font-semibold">{assistant().name}</div>
                      <div class="truncate text-[14px] text-ink-2">{assistant().login}</div>
                    </div>
                  </div>
                  <PermissionRow title={t("Dashboard")}>
                    <PermissionBox
                      checked={assistant().editTips}
                      text={t("Edit tips")}
                      label={t("Let {name} edit your settings", { name: assistant().name })}
                      help={t("Change the tip page, message, wheel of luck and effect settings.")}
                      onChange={(on) =>
                        call(() =>
                          setAssistantEditTips(props.session, props.platform, assistant().id, on),
                        )
                      }
                    />
                  </PermissionRow>
                  <PermissionRow title={t("Remote control")}>
                    <AccessPicker
                      access={assistant().remoteControl}
                      label={t("What {name} may do with the remote control", {
                        name: assistant().name,
                      })}
                      onChange={(access) =>
                        call(() =>
                          setAssistantRemoteControl(
                            props.session,
                            props.platform,
                            assistant().id,
                            access,
                          ),
                        )
                      }
                    />
                    <PermissionBox
                      checked={assistant().preview}
                      disabled={assistant().remoteControl === "none"}
                      text={t("Preview")}
                      label={t("Let {name} see the stream preview", { name: assistant().name })}
                      help={t("See the stream preview.")}
                      onChange={(on) =>
                        call(() =>
                          setAssistantPreview(props.session, props.platform, assistant().id, on),
                        )
                      }
                    />
                    <PermissionBox
                      checked={assistant().goLive}
                      disabled={assistant().remoteControl !== "control"}
                      text={t("Go live")}
                      label={t("Let {name} go live", { name: assistant().name })}
                      help={t("Start the stream. Ending it is always allowed.")}
                      onChange={(on) =>
                        call(() =>
                          setAssistantGoLive(props.session, props.platform, assistant().id, on),
                        )
                      }
                    />
                  </PermissionRow>
                </div>
                <button
                  type="button"
                  class="flex size-10 shrink-0 items-center justify-center rounded-xl text-danger transition-colors hover:bg-danger-bg disabled:opacity-50"
                  disabled={busy()}
                  aria-label={t("Remove {name}", { name: assistant().name })}
                  title={t("Remove")}
                  onClick={() =>
                    void run(() => removeAssistant(props.session, props.platform, assistant().id))
                  }
                >
                  <Icon name="trash" class="size-5" />
                </button>
              </li>
            )}
          </Index>
        </ul>
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

const assistantErrorText = (error: unknown) =>
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
    amounts: (props.tips?.amounts ?? [200, 500, 1000]).map(amountText),
  });
  const { form, change, save } = createFeatureSave({
    initial,
    value: (form): Feature<Tips> => ({ enabled: form.enabled, amounts: amountsOf(form.amounts) }),
    busy: () => props.busy,
    save: (tips) => props.onSave({ tips }),
    errorText: (error) => paymentErrorText(error, t("Enter amounts from 0.50 to 100000.")),
  });
  const enabled = () => form().enabled;
  const amounts = () => form().amounts;
  const setAmounts = (amounts: string[]) => change({ amounts });

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">{t("Message")}</h2>
      <p class="text-ink-2">
        {t(
          "Let viewers send you a tip with a message. Viewers pick one of your amounts or enter " +
            "their own, and the message is optional. Messages show up in Moblin's chat, also those sent " +
            "while Moblin is not connected.",
        )}
      </p>
      <p class="text-ink-2">
        {tRich(
          "To also show them as alerts on stream, pick an alerts widget in Moblin under " +
            "Settings → Moblin Cloud → Message. See the {guide}.",
          { guide: <GuideLink text={t("setup guide")} /> },
        )}
      </p>
      <EnableToggle checked={enabled()} onChange={(enabled) => change({ enabled })}>
        {t("Viewers can send messages")}
      </EnableToggle>
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
                    <RemoveRowButton
                      label={t("Remove amount {number}", { number: index + 1 })}
                      onClick={() => setAmounts(amounts().filter((_, at) => at !== index))}
                    />
                  </Show>
                </div>
              )}
            </Index>
          </div>
          <AddRowButton rows={amounts().length} onClick={() => setAmounts([...amounts(), ""])}>
            {t("Add amount")}
          </AddRowButton>
        </fieldset>
      </Show>
      <SaveFooter save={save} signInAgain />
    </form>
  );
}

function EffectSettings(props: {
  effects: Feature<Effects> | null;
  currency: string;
  busy: boolean;
  onSave: (changes: { effects: Feature<Effects> }) => Promise<void>;
}) {
  const initial = () => ({
    enabled: isOn(props.effects),
    price: amountText(props.effects?.price ?? 200),
  });
  const { form, change, save } = createFeatureSave({
    initial,
    value: (form): Feature<Effects> => ({ enabled: form.enabled, price: hundredths(form.price) }),
    busy: () => props.busy,
    save: (effects) => props.onSave({ effects }),
    errorText: (error) => paymentErrorText(error, t("Enter a price from 0.50 to 100000.")),
  });
  const enabled = () => form().enabled;

  return (
    <form class={cardClass} onSubmit={save.submit}>
      <h2 class="text-[23px] font-semibold">{t("Effect")}</h2>
      <p class="text-ink-2">
        {t(
          "Let viewers tip to play an effect on stream. Viewers pick one of Moblin's fart sounds, " +
            "listen to it first if they like, and add an optional message.",
        )}
      </p>
      <EnableToggle checked={enabled()} onChange={(enabled) => change({ enabled })}>
        {t("Viewers tip to play effects")}
      </EnableToggle>
      <Show when={enabled()}>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">
            {t("Price per effect ({currency})", { currency: props.currency })}
          </span>
          <MoneyInput
            value={form().price}
            onValue={(price) => change({ price })}
            required
            label={t("Price per effect")}
          />
        </label>
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
    ).map((option) => ({ spins: String(option.spins), price: amountText(option.price) })),
  });
  const { form, change, save } = createFeatureSave({
    initial,
    value: (form): Feature<Spin> => ({
      enabled: form.enabled,
      options: form.options
        .filter((option) => option.spins.trim() && option.price.trim())
        .map((option) => ({
          spins: Number.parseInt(option.spins),
          price: hundredths(option.price),
        })),
    }),
    busy: () => props.busy,
    save: (spin) => props.onSave({ spin }),
    errorText: (error) =>
      paymentErrorText(error, t("Enter 1 to 10 spins for 0.50 to 100000 per price.")),
  });
  const enabled = () => form().enabled;
  const options = () => form().options;
  const setOptions = (options: ReturnType<typeof initial>["options"]) => change({ options });
  const changeOption = (index: number, changes: Partial<{ spins: string; price: string }>) =>
    setOptions(options().map((old, at) => (at === index ? { ...old, ...changes } : old)));

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
      <p class="text-ink-2">
        {tRich(
          "Create a wheel of luck widget in Moblin under Settings → Scenes, then pick it under " +
            "Settings → Moblin Cloud → Wheel of luck. See the {guide}.",
          { guide: <GuideLink text={t("setup guide")} /> },
        )}
      </p>
      <EnableToggle checked={enabled()} onChange={(enabled) => change({ enabled })}>
        {t("Viewers tip to spin the wheel")}
      </EnableToggle>
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
                    <RemoveRowButton
                      label={t("Remove option {number}", { number: index + 1 })}
                      onClick={() => setOptions(options().filter((_, at) => at !== index))}
                    />
                  </Show>
                </div>
              )}
            </Index>
          </div>
          <AddRowButton
            rows={options().length}
            onClick={() => setOptions([...options(), { spins: "", price: "" }])}
          >
            {t("Add option")}
          </AddRowButton>
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
  effects?: string[];
  send: (message: string, effect: string | undefined) => Promise<unknown>;
  onSignOut: () => void;
}) {
  const [message, setMessage] = createSignal("");
  const [effect, setEffect] = createSignal(props.effects?.[0]);
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
      await props.send(message(), effect());
      flashSent();
    } catch (error) {
      if (isSignedOut(error)) {
        props.onSignOut();
      }
      setError(assistantErrorText(error));
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
      <Show when={props.effects}>
        {(effects) => (
          <select
            aria-label={t("Effect")}
            value={effect()}
            onChange={(event) => setEffect(event.currentTarget.value)}
            class={`${inputClass} max-w-60`}
          >
            <For each={effects()}>{(id) => <option value={id}>{effectName(id)}</option>}</For>
          </select>
        )}
      </Show>
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
    description: t("Send yourself a message, to check that messages show up in Moblin."),
    label: t("Send test message"),
  },
  spin: {
    description: t("Spin your wheel in Moblin, to check that it works."),
    label: t("Send test spin"),
  },
  effect: {
    description: t("Play an effect in Moblin, to check that it works."),
    label: t("Send test effect"),
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
      effects={props.kind === "effect" ? effects : undefined}
      onSignOut={props.onSignOut}
      send={(message, effect) =>
        sendTest(props.session, props.kind, { message, effect }, props.page)
      }
    />
  );
}

function Editable(props: { readOnly: boolean; children: JSX.Element }) {
  return (
    <fieldset disabled={props.readOnly} class="contents">
      {props.children}
    </fieldset>
  );
}

function Admin(props: {
  session: string;
  account: Account;
  page?: string;
  recentChanges: number;
  onAccount: (account: Account) => void;
  onOpenDevice: (device: string) => void;
  onSignOut: () => void;
}) {
  const account = () => props.account;
  const settings = () => props.account.settings;
  const setAccount = (account: Account) => props.onAccount(account);
  const setSettings = (settings: Account["settings"]) =>
    props.onAccount({ ...props.account, settings });
  const [busy, setBusy] = createSignal(false);
  const assisting = () => props.page !== undefined;
  const { recent, loadRecent, loadMore } = createRecent(
    (kind, limit, before) => fetchRecent(props.session, props.page, kind, limit, before),
    (error) => {
      if (isSignedOut(error)) {
        props.onSignOut();
      }
    },
  );

  const guard = async <T,>(promise: Promise<T>) => {
    try {
      return await promise;
    } catch (error) {
      if (isSignedOut(error)) {
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

  onMount(() => void loadRecent());
  createEffect(
    on(
      () => props.recentChanges,
      () => void loadRecent(),
      { defer: true },
    ),
  );

  const save = (changes: Partial<SettingsChanges>) =>
    busyWith(
      () => saveAccountSettings(props.session, { ...settings(), ...changes }, props.page),
      setSettings,
    );

  const payments = () => settings().paypal || settings().stripe;
  const sections: (Section & { shown: () => boolean })[] = [
    {
      id: "page",
      label: t("Tip page"),
      icon: "tip-jar",
      shown: () => true,
      status: () => null,
    },
    {
      id: "tips",
      label: t("Message"),
      icon: kindIcons.tip,
      shown: () => assisting() || payments() || isOn(settings().tips),
      status: () => onOff(isOn(settings().tips)),
    },
    {
      id: "wheel",
      label: t("Wheel of luck"),
      icon: kindIcons.spin,
      shown: () => true,
      status: () => onOff(isOn(settings().spin) || isOn(settings().subSpins)),
    },
    {
      id: "effects",
      label: t("Effect"),
      icon: kindIcons.effect,
      shown: () => true,
      status: () => onOff(isOn(settings().effects) || isOn(settings().subEffects)),
    },
    {
      id: "assistants",
      label: t("Assistants"),
      icon: "users",
      shown: () => !assisting() && (account().twitch !== null || account().kick !== null),
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
  const { active, select } = createSectionChoice(visible, initial);
  const paid = (configured: boolean) => settings().payable || configured;

  return (
    <div class="grid gap-6 lg:grid-cols-[260px_minmax(0,1fr)] lg:items-start">
      <SectionMenu
        label={t("Dashboard")}
        sections={visible()}
        active={active()}
        onSelect={select}
      />
      <div class="min-w-0">
        <Show when={!account().editTips && ["page", "tips", "wheel", "effects"].includes(active())}>
          <div class="mb-6">
            <Notice>{t("The streamer has not let you edit tips.")}</Notice>
          </div>
        </Show>
        <div hidden={active() !== "page"} class="flex flex-col gap-6">
          <Show when={!assisting()}>
            <GetStarted
              account={account()}
              settings={settings()}
              payments={payments()}
              onSelect={select}
            />
          </Show>
          <Editable readOnly={!account().editTips}>
            <YourPage
              account={account()}
              page={settings()}
              busy={busy()}
              assisting={assisting()}
              onSave={save}
              onImage={changeImage}
            />
          </Editable>
        </div>
        <div hidden={active() !== "tips"} class="flex flex-col gap-6">
          <Show
            when={paid(isOn(settings().tips))}
            fallback={
              <NeedsPayments
                title={t("Message")}
                available={payments()}
                streamer={assisting() ? account().name : null}
                onConnect={() => select("payments")}
              />
            }
          >
            <Editable readOnly={!account().editTips}>
              <TipSettings
                tips={settings().tips}
                currency={settings().currency}
                busy={busy()}
                onSave={save}
              />
            </Editable>
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
          <Show when={!assisting() && isOn(settings().spin) && !settings().connected}>
            <Notice>
              {t("Viewers can't spin the wheel right now, as Moblin is not connected.")}
            </Notice>
          </Show>
          <Show
            when={
              !assisting() &&
              isOn(settings().spin) &&
              settings().connected &&
              !account().devices.some((device) => device.wheel)
            }
          >
            <Notice>
              {t(
                "Viewers can't spin the wheel right now, as no wheel is picked under Settings → " +
                  "Moblin Cloud → Wheel of luck in Moblin.",
              )}
            </Notice>
          </Show>
          <Show
            when={paid(isOn(settings().spin))}
            fallback={
              <NeedsPayments
                title={t("Wheel of luck")}
                available={payments()}
                streamer={assisting() ? account().name : null}
                onConnect={() => select("payments")}
              />
            }
          >
            <Editable readOnly={!account().editTips}>
              <SpinSettings
                spin={settings().spin}
                currency={settings().currency}
                busy={busy()}
                onSave={save}
              />
            </Editable>
          </Show>
          <Show when={account().twitch || account().kick}>
            <Editable readOnly={!account().editTips}>
              <SubscriberSettings
                perk="spin"
                tiers={settings().subSpins}
                twitch={account().twitch !== null}
                kick={account().kick !== null}
                busy={busy()}
                onSave={(subSpins) => save({ subSpins })}
              />
            </Editable>
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
        <div hidden={active() !== "effects"} class="flex flex-col gap-6">
          <Show
            when={paid(isOn(settings().effects))}
            fallback={
              <NeedsPayments
                title={t("Effect")}
                available={payments()}
                streamer={assisting() ? account().name : null}
                onConnect={() => select("payments")}
              />
            }
          >
            <Editable readOnly={!account().editTips}>
              <EffectSettings
                effects={settings().effects}
                currency={settings().currency}
                busy={busy()}
                onSave={save}
              />
            </Editable>
          </Show>
          <Show when={account().twitch || account().kick}>
            <Editable readOnly={!account().editTips}>
              <SubscriberSettings
                perk="effect"
                tiers={settings().subEffects}
                twitch={account().twitch !== null}
                kick={account().kick !== null}
                busy={busy()}
                onSave={(subEffects) => save({ subEffects })}
              />
            </Editable>
          </Show>
          <TestSend
            kind="effect"
            session={props.session}
            page={props.page}
            onSignOut={props.onSignOut}
          />
          <Latest
            kind="effect"
            list={recent()?.effect}
            onMore={() => loadMore("effect")}
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
                locked={isOn(settings().tips) || isOn(settings().spin) || isOn(settings().effects)}
                busy={busy()}
                onSave={save}
              />
            </Show>
          </div>
        </Show>
        <Show when={!assisting() && (account().twitch || account().kick)}>
          <div hidden={active() !== "assistants"}>
            <AssistantSettings
              session={props.session}
              account={account()}
              onAccount={setAccount}
              onSignOut={props.onSignOut}
            />
          </div>
        </Show>
        <div hidden={active() !== "moblin"}>
          <Show
            when={assisting()}
            fallback={
              <MoblinSettings
                account={account()}
                busy={busy()}
                onOpen={props.onOpenDevice}
                onNewKey={() => busyWith(() => newMoblinKey(props.session), setAccount)}
                onRename={(device, name) =>
                  busyWith(() => renameDevice(props.session, device, name), setAccount)
                }
                onRemove={(device) =>
                  busyWith(() => removeDevice(props.session, device), setAccount)
                }
              />
            }
          >
            <AssistedDevices account={account()} onOpen={props.onOpenDevice} />
          </Show>
        </div>
      </div>
    </div>
  );
}

type Target = { channel: string; page: string | null; device: string };

function rememberedChannel() {
  let stored = "";
  try {
    stored = sessionStorage.getItem(assistedChannelKey) ?? "";
  } catch {}
  return stored && !stored.includes("/") ? `twitch/${stored}` : stored;
}

function readTarget(remembered: boolean): Target {
  const path = location.pathname.slice(import.meta.env.BASE_URL.length);
  const own = path.match(/^(admin|account)\/?$/);
  if (own) {
    return {
      channel: rememberedChannel(),
      page: null,
      device: own[1] === "admin" ? adminPage : accountPage,
    };
  }
  const monitor = path.match(/^monitor\/([^/]+)\/?$/);
  if (monitor) {
    return { channel: "", page: decodeURIComponent(monitor[1]), device: monitorPage };
  }
  const device = path.match(/^device\/([^/]+)(?:\/([^/]+))?\/?$/);
  if (device) {
    return {
      channel: "",
      page: decodeURIComponent(device[1]),
      device: decodeURIComponent(device[2] ?? ""),
    };
  }
  const channel = path.match(/^dashboard\/(twitch|kick)\/([^/]+)\/?$/);
  if (channel) {
    return { channel: `${channel[1]}/${decodeURIComponent(channel[2])}`, page: null, device: "" };
  }
  return { channel: remembered ? rememberedChannel() : "", page: null, device: "" };
}

function resolve(account: Account, target: Target) {
  const [platform, login] = target.channel.split("/", 2) as [Platform, string | undefined];
  const channel =
    target.page !== null
      ? (account.assisting.find((channel) => channel.id === target.page) ?? null)
      : ((login ? account.assisting.find((channel) => channel[platform]?.login === login) : null) ??
        account.assisting.find((channel) => channelKey(channel) === target.channel) ??
        null);
  const page = channel?.id ?? account.settings.id;
  const device =
    target.device === adminPage
      ? account.admin
        ? adminPage
        : ""
      : target.device === accountPage
        ? accountPage
        : target.page === page
          ? target.device
          : "";
  return { channel, page, device };
}

const dashboardUrl = (channel: Assisting | null) =>
  asset(
    channel
      ? `${pages.assisted}${channel.platform}/${encodeURIComponent(channel.login)}`
      : pages.dashboard,
  );

const onOff = (on: boolean) => ({ on, text: on ? t("On") : t("Off") });

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
  onOpen: (device: string) => void;
  onNewKey: () => Promise<void>;
  onRename: (device: string, name: string) => Promise<void>;
  onRemove: (device: string) => Promise<void>;
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
      !(await ask(
        t(
          "Every device disconnects and the current Moblin Cloud key stops working. Paste the new one in Moblin on each device to reconnect.",
        ),
        t("Reset key"),
      ))
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
      if (!isSignedOut(error)) {
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
  const remove = async (device: Device) => {
    if (
      !(await ask(
        t(
          "Remove {name} from the list? Tips stop waiting for it. If it connects again, it is added back.",
          { name: device.name },
        ),
        t("Remove"),
      ))
    ) {
      return;
    }
    await run(() => props.onRemove(device.id));
  };
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">Moblin</h2>
      <p class="text-ink-2">
        {t(
          "Use the same key on up to 5 devices running Moblin. In Moblin, go to Settings → Moblin Cloud, paste the key and turn on Enabled.",
        )}
      </p>
      <p class="text-ink-2">
        {tRich(
          "Then pick the alerts widget that shows messages and the wheel of luck widget that viewers spin in the same " +
            "settings. See the {guide}.",
          { guide: <GuideLink text={t("setup guide")} /> },
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
          <DeviceList devices={props.account.devices}>
            {(device) => (
              <Show
                when={renaming() === device.id}
                fallback={
                  <DeviceRow device={device} page={props.account.settings.id} onOpen={props.onOpen}>
                    <button
                      type="button"
                      class="text-[15px] font-semibold text-leaf hover:underline disabled:opacity-60"
                      disabled={props.busy}
                      onClick={() => startRenaming(device)}
                    >
                      {t("Rename")}
                    </button>
                    <Show when={!device.connected && !device.reconnecting}>
                      <button
                        type="button"
                        class="text-[15px] font-semibold text-danger hover:underline disabled:opacity-60"
                        disabled={props.busy}
                        onClick={() => void remove(device)}
                      >
                        {t("Remove")}
                      </button>
                    </Show>
                  </DeviceRow>
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
                  <button type="button" class={smallButtonClass} onClick={() => setRenaming(null)}>
                    {t("Cancel")}
                  </button>
                </form>
              </Show>
            )}
          </DeviceList>
        </Show>
        <p class="text-[15px] text-muted">
          {t(
            "Wheel spins go to the first device with a wheel picked in Moblin's Moblin Cloud settings. Other tips go to every device.",
          )}
        </p>
        <p class="text-[15px] text-muted">
          {t(
            "Open a device's status to see how it is doing and mute it. Your assistants can too, from their dashboard.",
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

const deviceGets = (device: Device) =>
  device.connected || device.reconnecting
    ? [
        device.reconnecting ? t("Reconnecting") : null,
        t("Message"),
        device.wheel ? t("Wheel of luck") : null,
        t("Effect"),
      ]
        .filter((text) => text !== null)
        .join(" · ")
    : t("Last seen {time}", { time: timeAgo(device.lastSeen ?? 0) });

function DeviceList(props: { devices: Device[]; children: (device: Device) => JSX.Element }) {
  return (
    <ul class="flex flex-col divide-y-[1.5px] divide-line rounded-2xl border-[1.5px] border-line">
      <For each={props.devices}>
        {(device) => (
          <li class="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3">
            {props.children(device)}
          </li>
        )}
      </For>
    </ul>
  );
}

function DeviceRow(props: {
  device: Device;
  page: string;
  remote?: boolean;
  onOpen: (device: string) => void;
  children?: JSX.Element;
}) {
  return (
    <>
      <span
        class={`size-2.5 shrink-0 rounded-full ${props.device.connected ? "bg-leaf" : props.device.reconnecting ? "bg-amber" : "bg-muted"}`}
        title={
          props.device.connected
            ? t("Connected")
            : props.device.reconnecting
              ? t("Reconnecting")
              : t("Not connected")
        }
      />
      <span class="min-w-0 flex-1 font-semibold break-words sm:truncate">{props.device.name}</span>
      <span class="basis-full pl-[22px] text-[15px] text-muted sm:basis-auto sm:pl-0">
        {deviceGets(props.device)}
      </span>
      <span class="flex basis-full gap-3 pl-[22px] sm:basis-auto sm:pl-0">
        <Show when={props.remote ?? true}>
          <a
            href={deviceUrl(props.page, props.device.id)}
            onClick={(event) => {
              if (!event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey) {
                event.preventDefault();
                props.onOpen(props.device.id);
              }
            }}
            class="text-[15px] font-semibold text-leaf hover:underline"
          >
            {t("Remote control")}
          </a>
        </Show>
        {props.children}
      </span>
    </>
  );
}

function AssistedDevices(props: { account: Account; onOpen: (device: string) => void }) {
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">Moblin</h2>
      <p class="text-ink-2">
        {t("The devices running Moblin that this streamer has connected to Moblin Cloud.")}
      </p>
      <div class="flex flex-col gap-2">
        <h3 class="text-[15px] font-semibold text-ink-2">{t("Devices")}</h3>
        <Show
          when={props.account.devices.length > 0}
          fallback={<p class="text-muted">{t("No device has connected yet.")}</p>}
        >
          <DeviceList devices={props.account.devices}>
            {(device) => (
              <DeviceRow
                device={device}
                page={props.account.settings.id}
                remote={props.account.remoteControl !== "none"}
                onOpen={props.onOpen}
              />
            )}
          </DeviceList>
        </Show>
        <p class="text-[15px] text-muted">
          {t("Open a device's status to see how it is doing and mute it.")}
        </p>
      </div>
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

function SignedIn(props: {
  session: string;
  account: Account;
  linkResult: LinkResult | null;
  onSignOut: () => void;
  onSignOutEverywhere: () => Promise<void>;
}) {
  const [target, setTarget] = createSignal(readTarget(true));
  const shown = createMemo(() => resolve(props.account, target()), undefined, {
    equals: (a, b) => a.channel === b.channel && a.page === b.page && a.device === b.device,
  });
  const channel = () => shown().channel;
  const page = () => shown().page;
  const device = () => shown().device;

  const known = new Map([[props.account.settings.id, props.account]]);
  const [pageAccount, setPageAccount] = createSignal<Account | null>(null);
  const current = createMemo(() => {
    const account = pageAccount();
    return account?.settings.id === page() ? account : null;
  });
  const update = (account: Account) => {
    known.set(account.settings.id, account);
    if (account.settings.id === page()) {
      setPageAccount(account);
    }
  };
  const [failure, setFailure] = createSignal<{ page: string; message: string } | null>(null);
  const [recentChanges, setRecentChanges] = createSignal(0);

  createEffect(
    on(page, (id) => {
      const assisted = channel();
      setPageAccount(known.get(id) ?? null);
      setFailure(null);
      const refresh = async () => {
        try {
          update(await fetchAccount(props.session, assisted ? id : undefined));
        } catch (error) {
          if (isSignedOut(error)) {
            props.onSignOut();
          } else if (!known.has(id)) {
            setFailure({
              page: id,
              message:
                assisted && isForbidden(error)
                  ? t("{name} has removed you as an assistant.", { name: assisted.name })
                  : t("Something went wrong. Try again."),
            });
          }
        }
      };
      void refresh();
      onCleanup(
        subscribeChanges(
          pageEventsUrl(id),
          props.session,
          {
            account: () => void refresh(),
            recent: () => setRecentChanges((count) => count + 1),
          },
          (status) => {
            if (status === 401) {
              props.onSignOut();
            }
          },
        ),
      );
    }),
  );

  createEffect(() => {
    const picked = channel();
    rememberChannel(picked ? channelKey(picked) : "");
  });

  createEffect(() => {
    const { channel, page, device } = shown();
    const there = resolve(props.account, readTarget(false));
    if (
      there.page !== page ||
      there.device !== device ||
      (!device &&
        (location.pathname.startsWith(asset(pages.device)) ||
          location.pathname.startsWith(asset(pages.monitor)) ||
          location.pathname.startsWith(asset(pages.admin)) ||
          location.pathname.startsWith(asset(pages.account))))
    ) {
      history.replaceState(
        null,
        "",
        `${device ? urlOf(page, device) : dashboardUrl(channel)}${location.search}${location.hash}`,
      );
    }
  });

  const onPopState = () => setTarget(readTarget(false));
  addEventListener("popstate", onPopState);
  onCleanup(() => removeEventListener("popstate", onPopState));

  const hashes = { dashboard: "", device: "", monitor: "", admin: "", account: "" };
  const kindOf = (to: string) =>
    to === monitorPage
      ? "monitor"
      : to === adminPage
        ? "admin"
        : to === accountPage
          ? "account"
          : to
            ? "device"
            : ("dashboard" as const);
  const show = (picked: Assisting | null, to: string) => {
    const id = picked?.id ?? props.account.settings.id;
    hashes[kindOf(device())] = location.hash;
    scrollTo({ top: 0, behavior: "instant" });
    history.pushState(
      null,
      "",
      `${to ? urlOf(id, to) : dashboardUrl(picked)}${hashes[kindOf(to)]}`,
    );
    setTarget(
      to
        ? { channel: "", page: id, device: to }
        : { channel: picked ? channelKey(picked) : "", page: null, device: "" },
    );
  };

  const pickStreamer = async (key: string) => {
    const picked = props.account.assisting.find((channel) => channelKey(channel) === key) ?? null;
    if (!device() || device() === monitorPage || ownPages.includes(device())) {
      show(picked, ownPages.includes(device()) ? "" : device());
      return;
    }
    const id = picked?.id ?? props.account.settings.id;
    const account =
      known.get(id) ??
      (await fetchAccount(props.session, picked ? id : undefined).catch(() => null));
    show(picked, account && account.remoteControl !== "none" ? (account.devices[0]?.id ?? "") : "");
  };

  let premiumDialog: HTMLDialogElement | undefined;

  const monitor = () => device() === monitorPage;
  const copyTargets = () => [
    { id: props.account.settings.id, label: t("You") },
    ...props.account.assisting
      .filter((channel, index, all) => all.findIndex((other) => other.id === channel.id) === index)
      .map((channel) => ({
        id: channel.id,
        label:
          channel.platform === "kick" ? t("{name} on Kick", { name: channel.name }) : channel.name,
      })),
  ];
  createEffect(() => setFullWindow(monitor()));
  onCleanup(() => setFullWindow(false));

  const content = () => (
    <Show
      when={!ownPages.includes(device())}
      fallback={
        <Show
          when={device() === adminPage}
          fallback={
            <AccountView
              session={props.session}
              account={known.get(props.account.settings.id) ?? props.account}
              linkResult={props.linkResult}
              onAccount={update}
              onSignOut={props.onSignOut}
              onSignOutEverywhere={props.onSignOutEverywhere}
            />
          }
        >
          <AdminView session={props.session} onSignOut={props.onSignOut} />
        </Show>
      }
    >
      <Show when={page()} keyed>
        {(id) => (
          <Show
            when={current()}
            fallback={
              <Show
                when={failure()?.page === id ? failure()?.message : null}
                fallback={<Spinner />}
              >
                {(message) => <Notice status>{message()}</Notice>}
              </Show>
            }
          >
            {(account) => (
              <Show
                when={device()}
                fallback={
                  <Admin
                    session={props.session}
                    account={account()}
                    page={channel() ? id : undefined}
                    recentChanges={recentChanges()}
                    onAccount={update}
                    onOpenDevice={(to) => show(channel(), to)}
                    onSignOut={props.onSignOut}
                  />
                }
              >
                {(picked) => (
                  <Show
                    when={picked() !== monitorPage}
                    fallback={
                      <Monitor
                        session={props.session}
                        page={id}
                        account={account()}
                        assisted={channel() !== null}
                        copyTargets={copyTargets()}
                        recentChanges={recentChanges()}
                        onSignOut={props.onSignOut}
                      />
                    }
                  >
                    <Show
                      when={account().remoteControl !== "none"}
                      fallback={
                        <div class={cardClass}>
                          <p class="text-ink-2">
                            {t("The streamer has not let you use the remote control.")}
                          </p>
                        </div>
                      }
                    >
                      <DeviceView
                        session={props.session}
                        page={id}
                        device={picked()}
                        preview={account().preview}
                        goLive={account().goLive}
                        remoteControl={account().remoteControl === "control"}
                        nameOf={(device) =>
                          account().devices.find((candidate) => candidate.id === device)?.name
                        }
                        onSignOut={props.onSignOut}
                      />
                    </Show>
                  </Show>
                )}
              </Show>
            )}
          </Show>
        )}
      </Show>
    </Show>
  );

  return (
    <div class={monitor() ? "flex h-dvh flex-col" : "contents"}>
      <CloudHeader premium={props.account.premium} wide={monitor()}>
        <div class="ml-auto flex min-w-0 items-center gap-3">
          <Show when={!props.account.premium}>
            <GoPremium ref={(element) => (premiumDialog = element)} />
          </Show>
          <PagePicker
            devices={current()?.devices ?? []}
            admin={props.account.admin}
            picked={device()}
            onPick={(to) => show(channel(), to)}
          />
          <Show when={props.account.assisting.length > 0}>
            <StreamerPicker
              account={props.account}
              picked={channel()}
              onPick={(key) => void pickStreamer(key)}
            />
          </Show>
          <AccountMenu
            name={props.account.name}
            avatar={props.account.avatar}
            items={[
              ...(props.account.premium
                ? []
                : [
                    {
                      label: t("Go premium"),
                      icon: "star" as const,
                      onClick: () => premiumDialog?.showModal(),
                    },
                  ]),
              {
                label: t("Account"),
                icon: "user",
                onClick: () => show(channel(), accountPage),
              },
            ]}
            onSignOut={props.onSignOut}
          />
        </div>
      </CloudHeader>
      <main class={monitor() ? "min-h-0 flex-1 bg-band" : undefined}>
        <Show when={monitor()} fallback={<Band tight>{content()}</Band>}>
          {content()}
        </Show>
      </main>
    </div>
  );
}

export default function Dashboard() {
  const { session, loading, signInFailed, linkResult, signOut, signOutAll } =
    createAccountSession();

  createEffect(() => {
    if (!loading() && !session() && location.pathname !== asset(pages.dashboard)) {
      history.replaceState(null, "", `${asset(pages.dashboard)}${location.search}${location.hash}`);
    }
  });

  return (
    <Show
      when={session()}
      keyed
      fallback={
        <>
          <CloudHeader>
            <Show when={!loading()}>
              <span class="ml-auto flex items-center gap-2">
                <PlatformSignIn platform="twitch" />
                <PlatformSignIn platform="kick" />
              </span>
            </Show>
          </CloudHeader>
          <main>
            <Show when={loading()} fallback={<CloudLanding failed={signInFailed()} />}>
              <Band>
                <Spinner />
              </Band>
            </Show>
          </main>
        </>
      }
    >
      {(current) => (
        <SignedIn
          session={current.session}
          account={current.account}
          linkResult={linkResult()}
          onSignOut={signOut}
          onSignOutEverywhere={signOutAll}
        />
      )}
    </Show>
  );
}
