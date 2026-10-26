import { type JSX } from "solid-js";
import { render } from "solid-js/web";
import { t } from "../i18n";
import { ghostButtonClass } from "./PageParts";

export default function Modal(props: {
  ref: (dialog: HTMLDialogElement) => void;
  onClose?: () => void;
  children: JSX.Element;
}) {
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
      onClose={() => props.onClose?.()}
    >
      {props.children}
    </dialog>
  );
}

export function ask(text: string, action: string): Promise<boolean> {
  return new Promise((resolve) => {
    const container = document.createElement("div");
    document.body.append(container);
    let dialog: HTMLDialogElement | undefined;
    let confirmed = false;
    const dispose = render(
      () => (
        <Modal
          ref={(element) => (dialog = element)}
          onClose={() => {
            dispose();
            container.remove();
            resolve(confirmed);
          }}
        >
          <div class="flex flex-col gap-6">
            <p class="text-[17px] text-ink-2">{text}</p>
            <div class="flex flex-wrap justify-end gap-3">
              <button type="button" class={ghostButtonClass} onClick={() => dialog?.close()}>
                {t("Cancel")}
              </button>
              <button
                type="button"
                class={`${ghostButtonClass} border-danger-line! bg-danger-bg! text-danger! hover:bg-danger-line!`}
                onClick={() => {
                  confirmed = true;
                  dialog?.close();
                }}
              >
                {action}
              </button>
            </div>
          </div>
        </Modal>
      ),
      container,
    );
    dialog?.showModal();
  });
}
