import { createSignal, Show } from "solid-js";
import {
  buttonClass,
  cardClass,
  ghostButtonClass,
  inputClass,
  Notice,
} from "../components/PageParts";
import { connectPendingKey, decideConnect } from "../data/account";
import { BackendError, isSignedOut } from "../data/backend";
import { t } from "../i18n";

export default function ConnectView(props: { session: string; onSignOut: () => void }) {
  try {
    sessionStorage.removeItem(connectPendingKey);
  } catch {}
  const [code, setCode] = createSignal("");
  const [working, setWorking] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [decided, setDecided] = createSignal<"approved" | "denied" | null>(null);

  const decide = async (approve: boolean) => {
    setError(null);
    setWorking(true);
    try {
      await decideConnect(props.session, code().trim(), approve);
      setDecided(approve ? "approved" : "denied");
    } catch (error) {
      if (isSignedOut(error)) {
        props.onSignOut();
        return;
      }
      setError(
        error instanceof BackendError && error.status < 500 && error.message
          ? error.message
          : t("Something went wrong. Try again."),
      );
    } finally {
      setWorking(false);
    }
  };

  return (
    <div class="flex flex-col gap-8">
      <h1 class="font-display text-[34px] font-bold">{t("Connect mobtop")}</h1>
      <div class={cardClass}>
        <Show
          when={decided()}
          fallback={
            <>
              <p class="text-ink-2">
                {t(
                  "mobtop, a terminal app, asks to use your Moblin Cloud account with full access. " +
                    "Only enter a code that mobtop shows you after you started signing in yourself.",
                )}
              </p>
              <label class="flex flex-col gap-2">
                <span class="font-semibold">{t("Code")}</span>
                <input
                  class={`${inputClass} max-w-60 font-mono text-[21px] tracking-[0.12em] uppercase`}
                  value={code()}
                  placeholder="XXXX-XXXX"
                  autofocus
                  autocomplete="off"
                  spellcheck={false}
                  onInput={(event) => setCode(event.currentTarget.value)}
                />
              </label>
              <div class="flex flex-wrap items-center gap-4">
                <button
                  type="button"
                  class={buttonClass}
                  disabled={working() || !code().trim()}
                  onClick={() => decide(true)}
                >
                  {t("Approve")}
                </button>
                <button
                  type="button"
                  class={ghostButtonClass}
                  disabled={working() || !code().trim()}
                  onClick={() => decide(false)}
                >
                  {t("Deny")}
                </button>
                <Show when={error()}>
                  {(error) => (
                    <Notice error onClose={() => setError(null)}>
                      {error()}
                    </Notice>
                  )}
                </Show>
              </div>
            </>
          }
        >
          {(decided) => (
            <Notice status>
              {decided() === "approved"
                ? t("mobtop is signed in. You can close this page.")
                : t("Denied. mobtop was not signed in.")}
            </Notice>
          )}
        </Show>
      </div>
    </div>
  );
}
