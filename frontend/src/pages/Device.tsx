import { createMemo, createSignal, Show } from "solid-js";
import Graphs from "../components/Graphs";
import MoblinScreen, { NotConnected } from "../components/MoblinScreen";
import MomMessage from "../components/MomMessage";
import SectionMenu, {
  connectedOrNot,
  createSectionChoice,
  type Section,
} from "../components/SectionMenu";
import Icon from "../components/Icon";
import { cardClass } from "../components/PageParts";
import { useDeviceFeed, usePreviewFeed, type DeviceFeed } from "../components/deviceFeed";
import { formatBits } from "../data/moblin";
import { t } from "../i18n";
import { readStored, store } from "../storage";

const previewHiddenKey = "previewHidden";

export default function DeviceView(props: {
  session: string;
  page: string;
  device: string;
  preview: boolean | null;
  goLive: boolean;
  remoteControl: boolean;
  nameOf: (device: string) => string | undefined;
  onSignOut: () => void;
}) {
  const current = createMemo(() => ({
    device: props.device,
    feed: useDeviceFeed(props.session, props.page, props.device, props.onSignOut),
  }));
  const shown = createMemo<{ device: string; feed: DeviceFeed } | undefined>((previous) =>
    current().feed.loaded() || !previous ? current() : previous,
  );
  const feed = () => shown()!.feed;
  const loaded = () => feed().loaded();
  const connected = () => feed().connected();
  const streaming = () => feed().streaming();
  const history = () => feed().history();

  const online = () => connected() && streaming();
  const sections: Section[] = [
    {
      id: "phone",
      label: t("Phone"),
      icon: "screen",
      status: () => connectedOrNot(online()),
    },
    {
      id: "graphs",
      label: t("Graphs"),
      icon: "chart",
      status: () => {
        const bitrate = history().at(-1)?.bitrate;
        return bitrate === null || bitrate === undefined
          ? null
          : { on: online(), text: formatBits(bitrate) };
      },
    },
  ];
  const { active, select } = createSectionChoice(() => sections);

  const [previewHidden, setPreviewHidden] = createSignal(readStored(previewHiddenKey) === "1");
  const togglePreview = () => {
    setPreviewHidden(!previewHidden());
    store(previewHiddenKey, previewHidden() ? "1" : null);
  };
  const previewFeed = createMemo(() =>
    active() === "phone" && props.preview === true && !previewHidden()
      ? usePreviewFeed(props.session, props.page, props.device)
      : null,
  );
  const preview = () => (connected() ? (previewFeed()?.() ?? null) : null);

  return (
    <>
      <Show when={loaded() && streaming() && !connected()}>
        <NotConnected name={props.nameOf(shown()!.device) ?? feed().name()} />
      </Show>
      <Show when={loaded()}>
        <div class="grid gap-6 lg:grid-cols-[260px_minmax(0,1fr)] lg:items-start">
          <SectionMenu
            label={t("Device page")}
            sections={sections}
            active={active()}
            onSelect={select}
          />
          <div class="min-w-0">
            <div hidden={active() !== "phone"}>
              <div class={cardClass}>
                <div class="flex items-center justify-between gap-3">
                  <h2 class="text-[23px] font-semibold">{t("Phone")}</h2>
                  <Show when={props.preview === true}>
                    <button
                      type="button"
                      aria-pressed={previewHidden()}
                      title={t("Hiding the stream preview saves bandwidth on the phone.")}
                      onClick={togglePreview}
                      class="inline-flex h-8 items-center gap-1.5 rounded-full border border-line px-3 text-[13px] font-semibold text-ink-2 transition-colors hover:bg-ghost hover:text-ink"
                    >
                      <Icon name={previewHidden() ? "eye" : "eye-off"} class="size-4" />
                      {previewHidden() ? t("Show stream preview") : t("Hide stream preview")}
                    </button>
                  </Show>
                </div>
                <p class="text-[15px] text-ink-2">
                  {t(
                    "What the Moblin app shows on the device, with the same buttons to control it.",
                  )}
                </p>
                <Show when={props.preview === false}>
                  <p class="text-[15px] text-muted">
                    {t("The streamer has not let you see the stream preview.")}
                  </p>
                </Show>
                <Show when={!props.remoteControl}>
                  <p class="text-[15px] text-muted">
                    {t("The streamer has not let you control Moblin.")}
                  </p>
                </Show>
                <MoblinScreen
                  page={props.page}
                  state={feed().state()}
                  status={feed().status()}
                  settings={feed().settings()}
                  preview={preview()}
                  goLive={props.goLive}
                  remoteControl={props.remoteControl}
                  connected={online()}
                  pending={feed().pending()}
                  onControl={(control) => feed().control(control)}
                />
                <MomMessage
                  session={props.session}
                  page={props.page}
                  device={props.device}
                  enabled={online()}
                  onSignOut={props.onSignOut}
                />
              </div>
            </div>
            <div hidden={active() !== "graphs"}>
              <Graphs history={history()} />
            </div>
          </div>
        </div>
      </Show>
    </>
  );
}
