import { untrack } from "solid-js";
import { createStore } from "solid-js/store";
import { LinkRow, Section } from "../../sceneEditor/Form";
import type { Page, Stack } from "../../sceneEditor/Stack";
import { Custom } from "./Custom";
import { Mobcam } from "./Mobcam";
import { Kick, Obs, Soop, Twitch, YouTube } from "./Platforms";
import { pushPage, type Wizard } from "./shared";
import type { Streams } from "../store";
import { makeUniqueName } from "../../../data/moblinSettings";
import type { SettingsStream } from "../../../data/moblinStreamSettings";
import { createStreamFromWizard, defaultWizard } from "../../../data/streamWizard";
import { t } from "../../../i18n";

const wizards = new WeakMap<Page, Wizard>();

export function StreamWizard(props: {
  streams: Streams;
  stack: Stack;
  highestBitrate?: number;
  onCreated: (stream: SettingsStream) => void;
}) {
  const page = untrack(() => props.stack.pages().at(-1));
  const makeWizard = (): Wizard => {
    const [state, set] = createStore(defaultWizard());
    return {
      state,
      set,
      stack: props.stack,
      uniqueName: (name) => makeUniqueName(name, props.streams.names()),
      create: () => props.onCreated(createStreamFromWizard(state, props.highestBitrate)),
    };
  };
  const existing = page ? wizards.get(page) : undefined;
  const wizard = existing ?? makeWizard();
  if (page && !existing) {
    wizards.set(page, wizard);
  }
  return (
    <>
      <Section header={t("Platform to stream to")}>
        <LinkRow label={t("Twitch")} onClick={() => pushPage(wizard, t("Twitch"), Twitch)} />
        <LinkRow label={t("Kick")} onClick={() => pushPage(wizard, t("Kick"), Kick)} />
        <LinkRow label={t("YouTube")} onClick={() => pushPage(wizard, t("YouTube"), YouTube)} />
        <LinkRow label={t("SOOP")} onClick={() => pushPage(wizard, t("SOOP"), Soop)} />
        <LinkRow label={t("OBS")} onClick={() => pushPage(wizard, t("OBS"), Obs)} />
      </Section>
      <Section>
        <LinkRow label={t("Mobcam")} onClick={() => pushPage(wizard, t("Mobcam"), Mobcam)} />
      </Section>
      <Section footer={t("For advanced users or if your platform is not in the list above.")}>
        <LinkRow label={t("Custom")} onClick={() => pushPage(wizard, t("Custom"), Custom)} />
      </Section>
    </>
  );
}
