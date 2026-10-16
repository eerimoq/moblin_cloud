import { createSignal, For, Index, onCleanup, onMount, Show } from "solid-js";
import { asset } from "../asset";
import { createFlash } from "../flash";
import Icon from "../components/Icon";
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
  newStreamerUrl,
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
  type Page,
  type Paywall,
  type Settings,
  type Spin,
  type Tips,
} from "../data/remoteControl";

const pageUrl = (account: Account) =>
  new URL(asset(`/twitch/${encodeURIComponent(account.login)}`), location.origin).href;

function Steps() {
  return (
    <ol class="flex list-decimal flex-col gap-2 pl-6 text-ink-2">
      <li>
        In Moblin, go to Settings, Remote control, Streamer, Assistant and paste the streamer URL as
        URL. Then turn Enabled on.
      </li>
      <li>Make sure Moblin may use Apple Music.</li>
    </ol>
  );
}

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
      !confirm(
        "Viewers can't pay for songs, tips or wheel spins until you connect a PayPal account again.",
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
            Connect your PayPal account to let viewers pay for songs, tips and wheel spins. You sign
            in or sign up on PayPal with a personal or business account (in some countries PayPal
            asks for a business account), and the money goes straight to your PayPal account. Moblin
            takes nothing, PayPal charges its usual fees. By connecting PayPal you agree to the{" "}
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
        Share it with your viewers so they can tip you, spin your wheel and request songs.
      </p>
      <Value
        label="Your page"
        text={pageUrl(props.account)}
        href={pageUrl(props.account)}
        hideLabel
      />
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">Headline</span>
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

function Payments(props: {
  paywall: Paywall | null;
  busy: boolean;
  onSave: (changes: { paywall: Paywall | null }) => Promise<void>;
}) {
  const [required, setRequired] = createSignal(props.paywall !== null);
  const [price, setPrice] = createSignal(
    props.paywall ? (props.paywall.price / 100).toFixed(2) : "1.00",
  );
  const [currency, setCurrency] = createSignal(props.paywall?.currency ?? "USD");
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
        paywall: required()
          ? {
              price: Math.round(Number.parseFloat(price()) * 100),
              currency: currency(),
            }
          : null,
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
      <h2 class="text-[23px] font-semibold">Paid song requests</h2>
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
          <CurrencySelect value={currency()} onChange={setCurrency} />
        </div>
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

function CurrencySelect(props: { value: string; onChange: (value: string) => void }) {
  return (
    <label class="flex w-32 flex-col gap-2">
      <span class="text-[15px] font-semibold text-ink-2">Currency</span>
      <select
        value={props.value}
        onChange={(event) => props.onChange(event.currentTarget.value)}
        class={inputClass}
      >
        <For each={currencies}>{(code) => <option value={code}>{code}</option>}</For>
      </select>
    </label>
  );
}

function AmountInputs(props: { amounts: string[]; onAmounts: (amounts: string[]) => void }) {
  return (
    <Index each={props.amounts}>
      {(amount, index) => (
        <input
          type="number"
          required={index === 0}
          min="0.5"
          max="1000"
          step="0.01"
          inputmode="decimal"
          aria-label={`Amount ${index + 1}`}
          value={amount()}
          onInput={(event) =>
            props.onAmounts(
              props.amounts.map((old, at) => (at === index ? event.currentTarget.value : old)),
            )
          }
          class={`${inputClass} max-w-32`}
        />
      )}
    </Index>
  );
}

const amountsOf = (amounts: string[]) =>
  amounts
    .filter((amount) => amount.trim())
    .map((amount) => Math.round(Number.parseFloat(amount) * 100));

function TipSettings(props: {
  tips: Tips | null;
  paywall: Paywall | null;
  busy: boolean;
  onSave: (changes: { tips: Tips | null }) => Promise<void>;
}) {
  const [enabled, setEnabled] = createSignal(props.tips !== null);
  const [currency, setCurrency] = createSignal(
    props.tips?.currency ?? props.paywall?.currency ?? "USD",
  );
  const [webhookUrl, setWebhookUrl] = createSignal(props.tips?.webhookUrl ?? "");
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
        tips: enabled()
          ? {
              currency: currency(),
              amounts: amountsOf(amounts()),
              webhookUrl: webhookUrl().trim() || null,
            }
          : null,
      });
      flashSaved();
    } catch (error) {
      setError(
        paymentErrorText(
          error,
          "Enter amounts from 0.50 to 1000 and a webhook URL starting with https://.",
        ),
      );
      setSignInAgain(error instanceof BackendError && error.status === 403);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form class={cardClass} onSubmit={submit}>
      <h2 class="text-[23px] font-semibold">Tips</h2>
      <p class="text-ink-2">
        Let viewers tip you with PayPal. Viewers pick one of your amounts or enter their own, and
        can add a message. The money goes straight to your PayPal account.
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
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">Suggested amounts</legend>
          <div class="flex flex-wrap items-end gap-4">
            <AmountInputs amounts={amounts()} onAmounts={setAmounts} />
            <CurrencySelect value={currency()} onChange={setCurrency} />
          </div>
        </fieldset>
        <label class="flex flex-col gap-2">
          <span class="text-[15px] font-semibold text-ink-2">Webhook URL (optional)</span>
          <input
            type="url"
            maxLength={500}
            value={webhookUrl()}
            onInput={(event) => setWebhookUrl(event.currentTarget.value)}
            placeholder="https://example.com/tips?token=…"
            autocomplete="off"
            spellcheck={false}
            class={inputClass}
          />
          <span class="text-[15px] text-muted">
            Every tip is posted to this URL as JSON, with the name, message and amount. Put a secret
            in the URL so nobody else can post fake tips to it.
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
  const [currency, setCurrency] = createSignal(props.spin?.currency ?? props.currency);

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
                  price: Math.round(Number.parseFloat(option.price) * 100),
                })),
              currency: currency(),
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
      <h2 class="text-[23px] font-semibold">Spin the wheel</h2>
      <p class="text-ink-2">
        Let viewers pay with PayPal to spin your wheel, with a lower price per spin when they buy
        more. Moblin spins the wheel you pick in its remote control settings, showing the viewer's
        name and message. The money goes straight to your PayPal account.
      </p>
      <label class="flex items-center gap-3 text-ink-2">
        <input
          type="checkbox"
          checked={enabled()}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
          class="size-5 accent-moss"
        />
        Viewers can spin the wheel
      </label>
      <Show when={enabled()}>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-[15px] font-semibold text-ink-2">Prices</legend>
          <div class="flex flex-col gap-3">
            <Index each={options()}>
              {(option, index) => (
                <div class="flex items-center gap-3 text-ink-2">
                  <input
                    type="number"
                    required={index === 0}
                    min="1"
                    max="10"
                    step="1"
                    inputmode="numeric"
                    aria-label={`Spins ${index + 1}`}
                    value={option().spins}
                    onInput={(event) => changeOption(index, { spins: event.currentTarget.value })}
                    class={`${inputClass} max-w-20`}
                  />
                  <span class="shrink-0">spins for</span>
                  <input
                    type="number"
                    required={index === 0}
                    min="0.5"
                    max="1000"
                    step="0.01"
                    inputmode="decimal"
                    aria-label={`Price ${index + 1}`}
                    value={option().price}
                    onInput={(event) => changeOption(index, { price: event.currentTarget.value })}
                    class={`${inputClass} max-w-32`}
                  />
                </div>
              )}
            </Index>
          </div>
        </fieldset>
        <CurrencySelect value={currency()} onChange={setCurrency} />
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
  streamerUrl: string;
  settings: Settings;
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const [settings, setSettings] = createSignal(props.settings);
  const [busy, setBusy] = createSignal(false);
  const [newUrlFailed, setNewUrlFailed] = createSignal(false);

  const newUrl = async () => {
    if (
      !confirm(
        "Moblin disconnects and the current streamer URL stops working. Paste the new one in Moblin to reconnect.",
      )
    ) {
      return;
    }
    setBusy(true);
    setNewUrlFailed(false);
    try {
      props.onAccount(await newStreamerUrl(props.session));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      } else {
        setNewUrlFailed(true);
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

  return (
    <>
      <YourPage account={props.account} page={settings()} busy={busy()} onSave={save} />
      <Show when={settings().paypal}>
        <PayPalAccount
          session={props.session}
          settings={settings()}
          onSettings={setSettings}
          onSignOut={props.onSignOut}
        />
      </Show>
      <Show when={settings().merchantId || settings().paywall}>
        <Payments paywall={settings().paywall} busy={busy()} onSave={save} />
      </Show>
      <Show when={settings().merchantId || settings().tips}>
        <TipSettings
          tips={settings().tips}
          paywall={settings().paywall}
          busy={busy()}
          onSave={save}
        />
      </Show>
      <Show when={settings().merchantId || settings().spin}>
        <SpinSettings
          spin={settings().spin}
          currency={settings().tips?.currency ?? settings().paywall?.currency ?? "USD"}
          busy={busy()}
          onSave={save}
        />
      </Show>
      <div class={cardClass}>
        <h2 class="text-[23px] font-semibold">Connect Moblin for song requests</h2>
        <p class="flex items-center gap-2.5 text-[15px] text-muted">
          <span
            class={`size-2.5 shrink-0 rounded-full ${settings().connected ? "bg-leaf" : "bg-muted"}`}
          />
          {settings().connected ? "Moblin is connected." : "Moblin is not connected right now."}
        </p>
        <Steps />
        <Value label="Streamer URL for Moblin" text={props.streamerUrl} secret />
        <p class="flex items-start gap-3 rounded-2xl border-[1.5px] border-amber-line bg-amber-bg px-4 py-3 text-amber-ink">
          <Icon name="warning" class="mt-0.5 size-5 shrink-0 text-amber" />
          Keep the streamer URL to yourself. Anyone who has it can connect as your Moblin and get
          your song requests. Reset it if it leaked.
        </p>
        <div class="flex flex-wrap items-center gap-4">
          <button type="button" class={ghostButtonClass} disabled={busy()} onClick={newUrl}>
            Reset streamer URL
          </button>
          <Show when={newUrlFailed()}>
            <p role="status" class="text-amber">
              Something went wrong. Try again.
            </p>
          </Show>
        </div>
      </div>
      <div>
        <button
          type="button"
          class={ghostButtonClass}
          onClick={async () => {
            try {
              await signOutEverywhere(props.session);
            } catch {}
            props.onSignOut();
          }}
        >
          Sign out everywhere
        </button>
      </div>
    </>
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
    if (!props.account.streamerUrl) {
      void create();
    }
  });

  return (
    <Show
      when={props.account.streamerUrl && props.account.settings ? props.account : null}
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
          streamerUrl={account.streamerUrl!}
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

  return (
    <>
      <Hero title="Let viewers tip, spin and pick the music.">
        <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
          Sign in with Twitch to get your own page at moblin.app/twitch/your-name. Viewers add songs
          from Apple Music to the queue of Moblin's music player, for free or for a price you set,
          and can send you tips or pay to spin your wheel.
        </p>
        <p class="text-[15px] text-muted">
          Using the dashboard and your page means you agree to the{" "}
          <a href={asset(pages.terms)} class="text-leaf hover:text-leaf-bright">
            terms
          </a>
          .
        </p>
      </Hero>
      <Band>
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
