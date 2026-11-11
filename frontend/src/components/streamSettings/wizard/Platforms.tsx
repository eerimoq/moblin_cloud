import { NextRow } from "../../sceneEditor/wizards/Sheet";
import { NetworkSetupObs, pushNetworkSetup } from "./NetworkSetup";
import { Field, LoginFooter, type WizardPageProps } from "./shared";
import { t } from "../../../i18n";

export function Twitch(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({
    platform: "twitch",
    name: wizard.uniqueName(t("Twitch")),
    directIngest: "rtmp://ingest.global-contribute.live-video.net/app",
  });
  return (
    <>
      <LoginFooter />
      <Field
        header={t("Channel name")}
        value={wizard.state.twitchChannelName}
        placeholder={t("MyChannel")}
        onInput={(name) => wizard.set("twitchChannelName", name)}
      />
      <Field
        header={t("Channel id")}
        value={wizard.state.twitchChannelId}
        placeholder="908123903"
        onInput={(id) => wizard.set("twitchChannelId", id)}
      />
      <NextRow
        disabled={wizard.state.twitchChannelName.trim() === ""}
        onClick={() => pushNetworkSetup(wizard, t("Twitch"))}
      />
    </>
  );
}

export function Kick(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ platform: "kick", name: wizard.uniqueName(t("Kick")), directIngest: "" });
  return (
    <>
      <LoginFooter />
      <Field
        header={t("Channel name")}
        value={wizard.state.kickChannelName}
        placeholder={t("MyChannel")}
        onInput={(name) => wizard.set("kickChannelName", name)}
      />
      <NextRow
        disabled={wizard.state.kickChannelName.trim() === ""}
        onClick={() => pushNetworkSetup(wizard, t("Kick"))}
      />
    </>
  );
}

export function YouTube(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({
    platform: "youTube",
    name: wizard.uniqueName(t("YouTube")),
    directIngest: "rtmp://a.rtmp.youtube.com/live2",
  });
  return (
    <>
      <LoginFooter />
      <Field
        header={t("Channel handle")}
        value={wizard.state.youTubeHandle}
        placeholder="@erimo144"
        footer={t("Only needed for chat.")}
        onInput={(handle) => wizard.set("youTubeHandle", handle)}
      />
      <NextRow onClick={() => pushNetworkSetup(wizard, t("YouTube"))} />
    </>
  );
}

export function Soop(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ platform: "soop", name: wizard.uniqueName(t("SOOP")), directIngest: "" });
  return (
    <>
      <Field
        header={t("Channel name")}
        value={wizard.state.soopChannelName}
        placeholder={t("MyChannel")}
        onInput={(name) => wizard.set("soopChannelName", name)}
      />
      <Field
        header={t("Video id")}
        value={wizard.state.soopStreamId}
        placeholder="908123903"
        onInput={(id) => wizard.set("soopStreamId", id)}
      />
      <NextRow onClick={() => pushNetworkSetup(wizard, t("SOOP"))} />
    </>
  );
}

export function Obs(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ platform: "obs", name: wizard.uniqueName(t("OBS")) });
  return <NetworkSetupObs wizard={wizard} />;
}
