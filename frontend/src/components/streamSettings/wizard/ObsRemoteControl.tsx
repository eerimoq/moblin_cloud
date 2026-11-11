import { Show } from "solid-js";
import { Section, ToggleRow } from "../../sceneEditor/Form";
import { NextRow } from "../../sceneEditor/wizards/Sheet";
import { General } from "./General";
import { Field, pushPage, type WizardPageProps } from "./shared";
import { cleanUrl, isValidWebSocketUrl } from "../../../data/streamUrls";
import { t } from "../../../i18n";

export function ObsRemoteControl(props: WizardPageProps) {
  const wizard = props.wizard;
  const urlError = () => isValidWebSocketUrl(cleanUrl(wizard.state.obsRemoteControlUrl)) ?? "";
  const nextDisabled = () =>
    wizard.state.obsRemoteControlEnabled &&
    (wizard.state.obsRemoteControlUrl === "" ||
      wizard.state.obsRemoteControlPassword === "" ||
      urlError() !== "");
  return (
    <>
      <Section>
        <ToggleRow
          label={t("Enabled")}
          on={wizard.state.obsRemoteControlEnabled}
          onToggle={(on) => wizard.set("obsRemoteControlEnabled", on)}
        />
      </Section>
      <Show when={wizard.state.obsRemoteControlEnabled}>
        <Field
          header={t("URL")}
          value={wizard.state.obsRemoteControlUrl}
          placeholder="ws://213.33.45.132:4567"
          error={urlError()}
          footer={
            <>
              <span>{t("Use your public IP address if streaming over the internet.")}</span>
              <span>
                {t("Configure port forwarding in your router to forward incoming traffic to OBS.")}
              </span>
            </>
          }
          onInput={(url) => wizard.set("obsRemoteControlUrl", url)}
        />
        <Field
          header={t("Password")}
          value={wizard.state.obsRemoteControlPassword}
          placeholder="po3Gg4pflp3s"
          footer={t(
            "Copy from OBS Show Connect Info as seen in the screenshot below. Tools → WebSocket Server Settings → Show Connect Info → Server Password.",
          )}
          onInput={(password) => wizard.set("obsRemoteControlPassword", password)}
        />
        <Field
          header={t("Main scene")}
          value={wizard.state.obsRemoteControlMainScene}
          placeholder={t("Main scene")}
          footer={t(
            "The name of your main scene in OBS. Moblin will periodically try to switch to this scene from your BRB scene if the stream is likely working.",
          )}
          onInput={(scene) => wizard.set("obsRemoteControlMainScene", scene)}
        />
        <Field
          header={t("BRB scene")}
          value={wizard.state.obsRemoteControlBrbScene}
          placeholder={t("My BRB scene")}
          footer={t(
            "The name of your BRB scene in OBS. Moblin will periodically try to switch from your main scene to this scene if the stream is likely broken.",
          )}
          onInput={(scene) => wizard.set("obsRemoteControlBrbScene", scene)}
        />
        <Field
          header={t("Source name")}
          value={wizard.state.obsRemoteControlSourceName}
          placeholder={t("My source")}
          footer={t("The name of the Source in OBS that receives the stream from Moblin.")}
          onInput={(source) => wizard.set("obsRemoteControlSourceName", source)}
        />
      </Show>
      <NextRow disabled={nextDisabled()} onClick={() => pushPage(wizard, t("General"), General)} />
    </>
  );
}
