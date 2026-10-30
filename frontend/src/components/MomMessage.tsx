import { createSignal, Show } from "solid-js";
import Icon from "./Icon";
import { buttonClass, inputClass, Notice } from "./PageParts";
import { controlDevice } from "../data/account";
import { BackendError } from "../data/backend";
import { t } from "../i18n";

const messageErrorText = (error: unknown) =>
  error instanceof BackendError && [403, 409, 502, 504].includes(error.status) && error.message
    ? error.message
    : t("Something went wrong. Try again.");

export default function MomMessage(props: {
  session: string;
  page: string;
  device: string;
  enabled: boolean;
  compact?: boolean;
  onSignOut: () => void;
}) {
  const [text, setText] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const send = async (event: SubmitEvent) => {
    event.preventDefault();
    const message = text().trim();
    if (!message) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await controlDevice(props.session, props.page, props.device, {
        sendMessage: { text: message },
      });
      setText("");
    } catch (error) {
      if (error instanceof BackendError && error.status === 401) {
        props.onSignOut();
        return;
      }
      setError(messageErrorText(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      class={
        props.compact
          ? "flex flex-col gap-2"
          : "flex flex-col gap-3 border-t-[1.5px] border-line pt-5"
      }
    >
      <form
        class={`flex ${props.compact ? "gap-2" : "flex-wrap gap-3"}`}
        onSubmit={(event) => void send(event)}
      >
        <input
          type="text"
          value={text()}
          onInput={(event) => setText(event.currentTarget.value)}
          placeholder={t("Message to the streamer")}
          aria-label={t("Message to the streamer")}
          maxLength={300}
          autocomplete="off"
          disabled={!props.enabled}
          class={`${inputClass} min-w-0 flex-1 disabled:opacity-60 ${props.compact ? "h-9! rounded-xl! px-3! text-[15px]!" : ""}`}
        />
        <button
          type="submit"
          class={`${buttonClass} ${props.compact ? "h-9! gap-1.5! px-4! text-[15px]!" : ""}`}
          disabled={!props.enabled || busy() || !text().trim()}
        >
          <Icon name="chat" class={props.compact ? "size-4" : "size-5"} />
          {t("Send")}
        </button>
      </form>
      <Show when={error()}>
        {(error) => (
          <Notice error onClose={() => setError(null)}>
            {error()}
          </Notice>
        )}
      </Show>
    </div>
  );
}
