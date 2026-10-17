import { createSignal, For, Index, onCleanup, onMount, Show } from "solid-js";
import { asset } from "../asset";
import { createFlash } from "../flash";
import { copyText } from "../components/CopyButton";
import Icon, { type IconName } from "../components/Icon";
import {
  Band,
  buttonClass,
  cardClass,
  ghostButtonClass,
  Hero,
  inputClass,
  SaveButton,
  Spinner,
  Value,
} from "../components/PageParts";
import {
  checkPayPal,
  connectPayPal,
  disconnectPayPal,
  fetchAccount,
  createStreamer,
  newSecretKey,
  readSession,
  saveAccountSettings,
  saveSession,
  signInUrl,
  signOutEverywhere,
  takeSignInResult,
  type Account,
  type SettingsChanges,
} from "../data/account";
import { pages } from "../data/links";
import {
  accents,
  BackendError,
  currencies,
  defaultTitle,
  hundredths,
  type Page,
  type Paywall,
  type Settings,
  type Spin,
  type Tips,
} from "../data/remoteControl";

const pageUrl = (account: Account) =>
  new URL(asset(`/twitch/${encodeURIComponent(account.login)}`), location.origin).href;

function TwitchSignIn(props: { failed: boolean }) {
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">Sign in</h2>
      <p class="text-ink-2">Sign in with Twitch to create or manage your page.</p>
      <div>
        <a
          href={signInUrl}
          class="inline-flex h-13 items-center justify-center gap-2.5 rounded-full bg-twitch px-6 font-display text-lg font-semibold whitespace-nowrap text-white transition-colors hover:bg-twitch-bright"
        >
          <Icon name="twitch" class="size-5" />
          Sign in with Twitch
        </a>
      </div>
      <Show when={props.failed}>
        <p role="status" class="text-amber">
          Signing in did not work. Try again.
        </p>
      </Show>
    </div>
  );
}

const signedOut = (error: unknown) => error instanceof BackendError && error.status === 401;

function paymentErrorText(error: unknown, invalid: string) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 400:
        return invalid;
      case 409:
        return "Connect your PayPal account first.";
      case 502:
        return "PayPal could not be reached. Try again.";
      case 503:
        return "Payments are unavailable right now.";
    }
  }
  return "Something went wrong. Try again.";
}

function payPalErrorText(error: unknown) {
  if (error instanceof BackendError) {
    switch (error.status) {
      case 403:
        return "To connect PayPal, sign in again first.";
      case 404:
        return "PayPal has not connected your account yet. Finish signing up on PayPal, then check again.";
      case 409:
        return "You did not give Moblin permission to take payments for you on PayPal. Connect PayPal again and accept the permissions.";
      case 422:
        return "PayPal can't send payments to your account yet. Confirm your email address on PayPal, then check again.";
      case 502:
        return "PayPal could not be reached. Try again.";
      case 503:
        return "Payments are unavailable right now.";
    }
  }
  return "Something went wrong. Try again.";
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
      setError(paymentErrorText(error, "Pick one of the currencies."));
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class="flex flex-col gap-4" onSubmit={submit}>
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
      <p class="text-[15px] text-muted">
        Viewers pay all tips, song requests and wheel spins in this currency.
      </p>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>
          {(error) => (
            <p role="status" class="text-amber">
              {error()}
            </p>
          )}
        </Show>
      </div>
    </form>
  );
}

function PayPalAccount(props: {
  session: string;
  settings: Settings;
  busy: boolean;
  onSave: (changes: { currency: string }) => Promise<void>;
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
      !confirm(
        "Viewers can't send you tips, with or without a song or wheel spin, until you connect a PayPal account again.",
      )
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
        <p role="status" class="text-amber">
          PayPal is in sandbox mode. Payments are for testing only and no real money is moved.
        </p>
      </Show>
      <Show
        when={props.settings.merchantId}
        fallback={
          <p class="text-ink-2">
            Connect your PayPal account to let viewers send you tips, on their own or with a song
            request or wheel spin. You sign in or sign up on PayPal with a personal or business
            account (in some countries PayPal asks for a business account). By connecting PayPal you
            agree to the{" "}
            <a href={asset(pages.terms)} class="text-leaf hover:text-leaf-bright">
              terms
            </a>
            .
          </p>
        }
      >
        {(merchantId) => (
          <p class="flex items-center gap-2.5 text-ink-2">
            <span class="size-2.5 shrink-0 rounded-full bg-leaf" />
            <span>
              Connected to the PayPal account with merchant ID{" "}
              <span class="font-mono">{merchantId()}</span>.
            </span>
          </p>
        )}
      </Show>
      <Show when={props.settings.paypalPending}>
        <p class="text-ink-2">Finish connecting your account on PayPal, then reload this page.</p>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <Show when={!props.settings.merchantId}>
          <button type="button" class={buttonClass} disabled={working()} onClick={connect}>
            Connect PayPal
          </button>
        </Show>
        <Show when={props.settings.merchantId || props.settings.paypalPending}>
          <button type="button" class={ghostButtonClass} disabled={working()} onClick={disconnect}>
            Disconnect PayPal
          </button>
        </Show>
        <Show when={error()}>
          {(error) => (
            <p role="status" class="text-amber">
              {error()}
            </p>
          )}
        </Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            Sign in again
          </a>
        </Show>
      </div>
      <Show when={props.settings.merchantId}>
        <CurrencySettings
          currency={props.settings.currency}
          busy={props.busy}
          onSave={props.onSave}
        />
      </Show>
    </div>
  );
}

function YourPage(props: {
  account: Account;
  page: Page;
  busy: boolean;
  onSave: (changes: Page) => Promise<void>;
}) {
  const [title, setTitle] = createSignal(props.page.title ?? "");
  const [message, setMessage] = createSignal(props.page.message ?? "");
  const [accent, setAccent] = createSignal(props.page.accent ?? accents[0]);
  const [saving, setSaving] = createSignal(false);
  const [saved, flashSaved] = createFlash(2000);
  const [error, setError] = createSignal<string | null>(null);
  const [copied, flashCopied] = createFlash(1500);

  const copyLink = async () => {
    if (await copyText(pageUrl(props.account))) {
      flashCopied();
    }
  };

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (props.busy) {
      return;
    }
    setError(null);
    setSaving(true);
    try {
      await props.onSave({ title: title(), message: message(), accent: accent() });
      flashSaved();
    } catch {
      setError("Something went wrong. Try again.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">Your page</h2>
      <p class="text-ink-2">
        Share it with your viewers so they can send you tips, request songs and spin your wheel.
      </p>
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">Headline (optional)</span>
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
        <span class="text-[15px] font-semibold text-ink-2">Message to your viewers (optional)</span>
        <textarea
          maxLength={300}
          rows={3}
          value={message()}
          onInput={(event) => setMessage(event.currentTarget.value)}
          placeholder="Thanks for hanging out! Say hi in chat!"
          class={`${inputClass} h-auto resize-y py-3`}
        />
      </label>
      <fieldset class="flex flex-col gap-2">
        <legend class="mb-2 text-[15px] font-semibold text-ink-2">Color</legend>
        <div class="flex flex-wrap gap-3">
          <For each={accents}>
            {(name) => (
              <label
                data-accent={name}
                class="relative flex size-11 cursor-pointer items-center justify-center rounded-full bg-moss text-white has-[:checked]:ring-3 has-[:checked]:ring-ink has-[:checked]:ring-offset-3 has-[:checked]:ring-offset-card has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-6 has-[:focus-visible]:outline-ink"
                title={name[0].toUpperCase() + name.slice(1)}
              >
                <input
                  type="radio"
                  name="accent"
                  value={name}
                  checked={accent() === name}
                  onChange={() => setAccent(name)}
                  class="sr-only"
                />
                <span class="sr-only">{name}</span>
                <Show when={accent() === name}>
                  <Icon name="check" class="size-5" />
                </Show>
              </label>
            )}
          </For>
        </div>
      </fieldset>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <a href={pageUrl(props.account)} target="_blank" rel="noopener" class={ghostButtonClass}>
          Open your page
        </a>
        <button type="button" class={ghostButtonClass} onClick={copyLink}>
          <Icon name={copied() ? "check" : "copy"} class="size-4" />
          {copied() ? "Copied" : "Copy link"}
        </button>
        <Show when={error()}>
          {(error) => (
            <p role="status" class="text-amber">
              {error()}
            </p>
          )}
        </Show>
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
      setError(paymentErrorText(error, "Enter a price from 0.50 to 1000."));
      setSignInAgain(error instanceof BackendError && error.status === 403);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">Song requests</h2>
      <p class="text-ink-2">
        Let viewers add songs from Apple Music to the queue of Moblin's music player with a tip.
      </p>
      <p class="text-ink-2">
        Moblin plays the songs with Apple Music, so you need an Apple Music subscription and must
        allow Moblin to use Apple Music. Viewers do not need an Apple Music subscription.
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        Viewers can request songs
      </label>
      <Show when={enabled()}>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">
            Tip per 5 minutes of song ({props.currency})
          </span>
          <span class="flex items-center gap-3">
            <MoneyInput value={price()} onValue={setPrice} required />
            <span class="text-ink-2">per started 5 minutes</span>
          </span>
          <span class="text-[15px] text-muted">
            A song up to 5 minutes long costs this once, a song of 5 to 10 minutes twice, and so on.
            Songs always play in full.
          </span>
        </label>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>
          {(error) => (
            <p role="status" class="text-amber">
              {error()}
            </p>
          )}
        </Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            Sign in again
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
      setError(paymentErrorText(error, "Enter amounts from 0.50 to 1000."));
      setSignInAgain(error instanceof BackendError && error.status === 403);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">Tips</h2>
      <p class="text-ink-2">
        Let viewers send you just a tip with PayPal. Viewers pick one of your amounts or enter their
        own, and can add a message. Tips show up in Moblin's chat, also those sent while Moblin is
        not connected.
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        Viewers can send tips
      </label>
      <Show when={enabled()}>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">
            Suggested amounts ({props.currency})
          </legend>
          <div class="flex flex-wrap gap-3">
            <Index each={amounts()}>
              {(amount, index) => (
                <div class="flex items-center gap-3">
                  <MoneyInput
                    value={amount()}
                    onValue={(value) =>
                      setAmounts(amounts().map((old, at) => (at === index ? value : old)))
                    }
                    required
                    label={`Amount ${index + 1}`}
                  />
                  <Show when={amounts().length > 1}>
                    <button
                      type="button"
                      class="flex size-10 shrink-0 items-center justify-center rounded-xl text-muted hover:bg-ghost hover:text-ink"
                      aria-label={`Remove amount ${index + 1}`}
                      onClick={() => setAmounts(amounts().filter((_, at) => at !== index))}
                    >
                      <Icon name="close" class="size-5" />
                    </button>
                  </Show>
                </div>
              )}
            </Index>
          </div>
          <Show when={amounts().length < 4}>
            <div class="mt-1">
              <button
                type="button"
                class={ghostButtonClass}
                onClick={() => setAmounts([...amounts(), ""])}
              >
                Add amount
              </button>
            </div>
          </Show>
        </fieldset>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>
          {(error) => (
            <p role="status" class="text-amber">
              {error()}
            </p>
          )}
        </Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            Sign in again
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
      setError(paymentErrorText(error, "Enter 1 to 10 spins for 0.50 to 1000 per price."));
      setSignInAgain(error instanceof BackendError && error.status === 403);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">Wheel of luck</h2>
      <p class="text-ink-2">
        Let viewers spin your wheel by sending you a tip with PayPal, with a smaller tip per spin
        when they buy more. Moblin spins the wheel you pick in its Moblin Cloud settings, showing
        the viewer's name and message.
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        Viewers tip to spin the wheel
      </label>
      <Show when={enabled()}>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">Tips ({props.currency})</legend>
          <div class="flex flex-col gap-3">
            <Index each={options()}>
              {(option, index) => (
                <div class="flex items-center gap-3 text-ink-2">
                  <input
                    type="text"
                    required
                    pattern="\d+"
                    inputmode="numeric"
                    aria-label={`Spins ${index + 1}`}
                    value={option().spins}
                    onInput={(event) => changeOption(index, { spins: event.currentTarget.value })}
                    class={`${inputClass} max-w-20`}
                  />
                  <span class="shrink-0">spins for</span>
                  <MoneyInput
                    value={option().price}
                    onValue={(price) => changeOption(index, { price })}
                    required
                    label={`Tip ${index + 1}`}
                  />
                  <Show when={options().length > 1}>
                    <button
                      type="button"
                      class="flex size-10 shrink-0 items-center justify-center rounded-xl text-muted hover:bg-ghost hover:text-ink"
                      aria-label={`Remove option ${index + 1}`}
                      onClick={() => setOptions(options().filter((_, at) => at !== index))}
                    >
                      <Icon name="close" class="size-5" />
                    </button>
                  </Show>
                </div>
              )}
            </Index>
          </div>
          <Show when={options().length < 4}>
            <div class="mt-1">
              <button
                type="button"
                class={ghostButtonClass}
                onClick={() => setOptions([...options(), { spins: "", price: "" }])}
              >
                Add option
              </button>
            </div>
          </Show>
        </fieldset>
      </Show>
      <div class="flex flex-wrap items-center gap-4">
        <SaveButton saving={saving()} saved={saved()} disabled={props.busy} />
        <Show when={error()}>
          {(error) => (
            <p role="status" class="text-amber">
              {error()}
            </p>
          )}
        </Show>
        <Show when={signInAgain()}>
          <a href={signInUrl} class={ghostButtonClass}>
            Sign in again
          </a>
        </Show>
      </div>
    </form>
  );
}

function Admin(props: {
  session: string;
  account: Account;
  secretKey: string;
  settings: Settings;
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const [settings, setSettings] = createSignal(props.settings);
  const [busy, setBusy] = createSignal(false);
  const [newKeyFailed, setNewKeyFailed] = createSignal(false);

  const newKey = async () => {
    if (
      !confirm(
        "Moblin disconnects and the current secret key stops working. Paste the new one in Moblin to reconnect.",
      )
    ) {
      return;
    }
    setBusy(true);
    setNewKeyFailed(false);
    try {
      props.onAccount(await newSecretKey(props.session));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      } else {
        setNewKeyFailed(true);
      }
    } finally {
      setBusy(false);
    }
  };

  onMount(() => {
    const timer = setInterval(async () => {
      try {
        const account = await fetchAccount(props.session);
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

  const sections: {
    id: string;
    label: string;
    icon: IconName;
    shown: () => boolean;
    status: () => { on: boolean; text: string } | null;
  }[] = [
    {
      id: "page",
      label: "Your page",
      icon: "person",
      shown: () => true,
      status: () => null,
    },
    {
      id: "tips",
      label: "Tips",
      icon: "heart",
      shown: () => settings().paypal || settings().tips !== null,
      status: () => onOff(settings().tips !== null),
    },
    {
      id: "wheel",
      label: "Wheel of luck",
      icon: "wheel",
      shown: () => settings().paypal || settings().spin !== null,
      status: () => onOff(settings().spin !== null),
    },
    {
      id: "songs",
      label: "Song requests",
      icon: "music",
      shown: () => settings().paypal || settings().paywall !== null,
      status: () => onOff(settings().paywall !== null),
    },
    {
      id: "paypal",
      label: "PayPal",
      icon: "wallet",
      shown: () => settings().paypal,
      status: () => connectedOrNot(settings().merchantId !== null),
    },
    {
      id: "moblin",
      label: "Moblin",
      icon: "remote",
      shown: () => true,
      status: () => connectedOrNot(settings().connected),
    },
  ];
  const visible = () => sections.filter((section) => section.shown());
  const initial = new URLSearchParams(location.search).has("merchantIdInPayPal")
    ? "paypal"
    : location.hash.slice(1);
  const [chosen, setChosen] = createSignal(initial);
  const active = () =>
    visible().some((section) => section.id === chosen()) ? chosen() : visible()[0].id;
  const select = (id: string) => {
    setChosen(id);
    history.replaceState(null, "", `${location.pathname}${location.search}#${id}`);
  };
  const paid = (configured: boolean) => settings().merchantId !== null || configured;

  return (
    <div class="grid gap-6 lg:grid-cols-[260px_minmax(0,1fr)] lg:items-start">
      <nav
        aria-label="Dashboard"
        class="-mx-5 flex gap-2 overflow-x-auto px-5 pb-1 sm:-mx-8 sm:px-8 lg:sticky lg:top-24 lg:mx-0 lg:flex-col lg:overflow-visible lg:px-0 lg:pb-0"
      >
        <For each={visible()}>
          {(section) => (
            <button
              type="button"
              aria-current={active() === section.id ? "page" : undefined}
              onClick={() => select(section.id)}
              class="flex shrink-0 items-center gap-3 rounded-2xl border-[1.5px] px-4 py-3 text-left whitespace-nowrap transition-colors"
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
                    <span class="hidden lg:inline">{status().text}</span>
                  </span>
                )}
              </Show>
            </button>
          )}
        </For>
      </nav>
      <div class="min-w-0">
        <div hidden={active() !== "page"}>
          <YourPage account={props.account} page={settings()} busy={busy()} onSave={save} />
        </div>
        <div hidden={active() !== "tips"}>
          <Show
            when={paid(settings().tips !== null)}
            fallback={<NeedsPayPal title="Tips" onConnect={() => select("paypal")} />}
          >
            <TipSettings
              tips={settings().tips}
              currency={settings().currency}
              busy={busy()}
              onSave={save}
            />
          </Show>
        </div>
        <div hidden={active() !== "wheel"}>
          <Show
            when={paid(settings().spin !== null)}
            fallback={<NeedsPayPal title="Wheel of luck" onConnect={() => select("paypal")} />}
          >
            <SpinSettings
              spin={settings().spin}
              currency={settings().currency}
              busy={busy()}
              onSave={save}
            />
          </Show>
        </div>
        <div hidden={active() !== "songs"}>
          <Show
            when={paid(settings().paywall !== null)}
            fallback={<NeedsPayPal title="Song requests" onConnect={() => select("paypal")} />}
          >
            <SongSettings
              paywall={settings().paywall}
              currency={settings().currency}
              busy={busy()}
              onSave={save}
            />
          </Show>
        </div>
        <Show when={settings().paypal}>
          <div hidden={active() !== "paypal"}>
            <PayPalAccount
              session={props.session}
              settings={settings()}
              busy={busy()}
              onSave={save}
              onSettings={setSettings}
              onSignOut={props.onSignOut}
            />
          </div>
        </Show>
        <div hidden={active() !== "moblin"}>
          <MoblinSettings
            secretKey={props.secretKey}
            connected={settings().connected}
            busy={busy()}
            newKeyFailed={newKeyFailed()}
            onNewKey={newKey}
          />
        </div>
      </div>
    </div>
  );
}

const onOff = (on: boolean) => ({ on, text: on ? "On" : "Off" });

const connectedOrNot = (on: boolean) => ({ on, text: on ? "Connected" : "Not connected" });

function NeedsPayPal(props: { title: string; onConnect: () => void }) {
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{props.title}</h2>
      <p class="text-ink-2">
        Connect your PayPal account first, so the money goes straight to you.
      </p>
      <div>
        <button type="button" class={buttonClass} onClick={() => props.onConnect()}>
          Go to PayPal
        </button>
      </div>
    </div>
  );
}

function MoblinSettings(props: {
  secretKey: string;
  connected: boolean;
  busy: boolean;
  newKeyFailed: boolean;
  onNewKey: () => void;
}) {
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">Configure Moblin</h2>
      <p class="flex items-center gap-2.5 text-ink-2">
        <span
          class={`size-2.5 shrink-0 rounded-full ${props.connected ? "bg-leaf" : "bg-muted"}`}
        />
        {props.connected ? "Moblin is connected." : "Moblin is not connected."}
      </p>
      <p class="text-ink-2">
        In Moblin, go to Settings → Moblin Cloud, paste the key below and turn on Enabled.
      </p>
      <Value label="Secret key" text={props.secretKey} secret />
      <p class="flex items-start gap-3 rounded-2xl border-[1.5px] border-amber-line bg-amber-bg px-4 py-3 text-amber-ink">
        <Icon name="warning" class="mt-0.5 size-5 shrink-0 text-amber" />
        Keep it to yourself. Anyone with it can connect as your Moblin. Reset it if it leaks.
      </p>
      <div class="flex flex-wrap items-center gap-4">
        <button
          type="button"
          class={ghostButtonClass}
          disabled={props.busy}
          onClick={() => props.onNewKey()}
        >
          Reset secret key
        </button>
        <Show when={props.newKeyFailed}>
          <p role="status" class="text-amber">
            Something went wrong. Try again.
          </p>
        </Show>
      </div>
    </div>
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
      props.onAccount(await createStreamer(props.session));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      } else {
        setFailed(true);
      }
    }
  };

  onMount(() => {
    if (!props.account.secretKey) {
      void create();
    }
  });

  return (
    <Show
      when={props.account.secretKey && props.account.settings ? props.account : null}
      keyed
      fallback={
        <Show when={failed()} fallback={<Spinner />}>
          <div class="flex flex-wrap items-center gap-4">
            <p role="status" class="text-amber">
              Could not create your page.
            </p>
            <button type="button" class={ghostButtonClass} onClick={create}>
              Try again
            </button>
          </div>
        </Show>
      }
    >
      {(account) => (
        <Admin
          session={props.session}
          account={account}
          secretKey={account.secretKey!}
          settings={account.settings!}
          onAccount={props.onAccount}
          onSignOut={props.onSignOut}
        />
      )}
    </Show>
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
      Using Moblin Cloud and your page means you agree to the{" "}
      <a href={asset(pages.terms)} class="text-leaf hover:text-leaf-bright">
        terms
      </a>
      .
    </p>
  );

  return (
    <>
      <Show
        when={session()}
        fallback={
          <Hero title="Let viewers tip you.">
            <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
              Tips go straight to your PayPal account. Moblin takes nothing, PayPal charges its
              usual fees.
            </p>
            {terms()}
          </Hero>
        }
      >
        {(current) => (
          <section class="mx-auto flex max-w-[1120px] flex-wrap items-end justify-between gap-4 px-5 pt-10 pb-8 sm:px-8">
            <div class="flex min-w-0 flex-col gap-2">
              <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
                Moblin Cloud
              </span>
              <h1 class="text-[32px] leading-[1.1] font-bold text-balance sm:text-[40px]">
                Hi, {current().account.name}
              </h1>
              {terms()}
            </div>
            <button
              type="button"
              class={ghostButtonClass}
              onClick={async () => {
                try {
                  await signOutEverywhere(current().session);
                } catch {}
                signOut();
              }}
            >
              Sign out everywhere
            </button>
          </section>
        )}
      </Show>
      <Band tight={session() !== null}>
        <Show when={!loading()} fallback={<Spinner />}>
          <Show when={session()} keyed fallback={<TwitchSignIn failed={signInFailed()} />}>
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
    </>
  );
}
