import { createEffect, createSignal, For, Show } from "solid-js";
import { asset } from "../asset";
import BrandIcon from "../components/BrandIcon";
import { ask } from "../components/Modal";
import {
  AccountMenu,
  Band,
  cardClass,
  CloudHeader,
  ghostButtonClass,
  Hero,
  Notice,
  Spinner,
} from "../components/PageParts";
import {
  createAccountSession,
  deleteAccount,
  linkUrl,
  type LinkResult,
  unlinkAccount,
  type Account,
} from "../data/account";
import { BackendError } from "../data/backend";
import { pages } from "../data/links";
import { channelText, type Platform, platformNames, platforms } from "../data/remoteControl";
import { t } from "../i18n";

const smallButtonClass =
  "inline-flex h-10 items-center justify-center gap-2 rounded-full border-[1.5px] border-ghost-line bg-ghost px-4 font-display text-[15px] font-semibold whitespace-nowrap text-ink transition-colors hover:bg-ghost-hover disabled:opacity-60";

const signedOut = (error: unknown) => error instanceof BackendError && error.status === 401;

const errorText = (error: unknown) =>
  error instanceof BackendError && error.status < 500 && error.message
    ? error.message
    : t("Something went wrong. Try again.");

const linkResultText = (result: LinkResult) =>
  result.linked ? t("Linked.") : (result.error ?? t("Linking did not work. Try again."));

function LinkedAccounts(props: {
  session: string;
  account: Account;
  result: LinkResult | null;
  onAccount: (account: Account) => void;
  onSignOut: () => void;
}) {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [resultDismissed, setResultDismissed] = createSignal(false);
  const linked = () => platforms.filter((platform) => props.account[platform] !== null);
  const unlink = async (platform: Platform) => {
    setBusy(true);
    setError(null);
    try {
      props.onAccount(await unlinkAccount(props.session, platform));
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
      }
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Linked accounts")}</h2>
      <p class="text-ink-2">
        {t(
          "Sign in with any linked account to get this dashboard. Your tip page gets an address for each of them.",
        )}
      </p>
      <Show when={!resultDismissed() && props.result}>
        {(result) => (
          <Notice
            status={result().linked}
            error={!result().linked}
            onClose={() => setResultDismissed(true)}
          >
            {linkResultText(result())}
          </Notice>
        )}
      </Show>
      <Show when={error()}>
        {(error) => (
          <Notice error onClose={() => setError(null)}>
            {error()}
          </Notice>
        )}
      </Show>
      <ul class="flex flex-col gap-3">
        <For each={platforms}>
          {(platform) => (
            <li class="flex items-center gap-3 rounded-2xl border-[1.5px] border-line bg-well px-4 py-3">
              <BrandIcon name={platform} class="size-9" />
              <div class="min-w-0 flex-1">
                <div class="font-semibold">{platformNames[platform]}</div>
                <div class="truncate text-[15px] text-muted">
                  <Show when={props.account[platform]} fallback={t("Not linked")}>
                    {(identity) => channelText(platform, identity().login)}
                  </Show>
                </div>
              </div>
              <Show
                when={props.account[platform]}
                fallback={
                  <a href={linkUrl(platform)} class={smallButtonClass}>
                    {t("Link")}
                  </a>
                }
              >
                <button
                  type="button"
                  class={smallButtonClass}
                  disabled={busy() || linked().length < 2}
                  title={linked().length < 2 ? t("Link another account first.") : undefined}
                  onClick={() => void unlink(platform)}
                >
                  {t("Unlink")}
                </button>
              </Show>
            </li>
          )}
        </For>
      </ul>
      <p class="text-[15px] text-muted">
        {t(
          "Free tips for subscribers and moderators need a linked Twitch or Kick account. An account that already has its own page can't be linked. Delete that page first.",
        )}
      </p>
    </section>
  );
}

function SignOutEverywhere(props: { onSignOutEverywhere: () => Promise<void> }) {
  const [working, setWorking] = createSignal(false);
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Sign out everywhere")}</h2>
      <p class="text-ink-2">{t("Signs you out in every browser, including this one.")}</p>
      <div>
        <button
          type="button"
          class={ghostButtonClass}
          disabled={working()}
          onClick={async () => {
            setWorking(true);
            await props.onSignOutEverywhere();
          }}
        >
          {t("Sign out everywhere")}
        </button>
      </div>
    </div>
  );
}

function DeleteAccount(props: { session: string; onSignOut: () => void }) {
  const [working, setWorking] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const remove = async () => {
    if (
      !(await ask(
        t(
          "Your account is deleted with its tip page, Moblin devices, settings and history. This can't be undone.",
        ),
        t("Delete account"),
      ))
    ) {
      return;
    }
    setError(null);
    setWorking(true);
    try {
      await deleteAccount(props.session);
      props.onSignOut();
    } catch (error) {
      if (signedOut(error)) {
        props.onSignOut();
        return;
      }
      setError(t("Deleting did not work. Try again."));
    } finally {
      setWorking(false);
    }
  };
  return (
    <div class={cardClass}>
      <h2 class="text-[23px] font-semibold">{t("Delete account")}</h2>
      <p class="text-ink-2">
        {t(
          "Deletes your account with its tip page, Moblin devices, settings and history. Your PayPal " +
            "and Stripe accounts stay as they are, and tips you got stay with you. Signing in " +
            "again starts over with a new account.",
        )}
      </p>
      <div class="flex flex-wrap items-center gap-4">
        <button
          type="button"
          class={`${ghostButtonClass} border-danger-line! bg-danger-bg! text-danger! hover:bg-danger-line!`}
          disabled={working()}
          onClick={remove}
        >
          {t("Delete account")}
        </button>
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

export default function AccountPage() {
  const { session, loading, linkResult, signOut, signOutAll } = createAccountSession();
  const [account, setAccount] = createSignal<Account | null>(null);

  createEffect(() => {
    if (!loading() && !session()) {
      location.replace(asset(pages.dashboard));
    }
  });

  return (
    <>
      <CloudHeader premium={session()?.account.premium}>
        <Show when={session()}>
          {(current) => (
            <AccountMenu
              name={current().account.name}
              avatar={current().account.avatar}
              items={[{ label: t("Dashboard"), icon: "chart", href: asset(pages.dashboard) }]}
              onSignOut={signOut}
            />
          )}
        </Show>
      </CloudHeader>
      <main>
        <Hero title={t("Account")}>
          <a href={asset(pages.dashboard)} class="text-lg text-leaf hover:text-leaf-bright">
            {t("Back to the dashboard")}
          </a>
        </Hero>
        <Band tight>
          <Show when={!loading()} fallback={<Spinner />}>
            <Show when={session()} keyed>
              {(current) => (
                <>
                  <LinkedAccounts
                    session={current.session}
                    account={account() ?? current.account}
                    result={linkResult()}
                    onAccount={setAccount}
                    onSignOut={signOut}
                  />
                  <SignOutEverywhere onSignOutEverywhere={signOutAll} />
                  <DeleteAccount session={current.session} onSignOut={signOut} />
                </>
              )}
            </Show>
          </Show>
        </Band>
      </main>
    </>
  );
}
