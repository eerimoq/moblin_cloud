import { For, Match, Switch, type JSX } from "solid-js";
import Icon from "../../Icon";
import { rowClass } from "../../MoblinParts";
import { Section } from "../../sceneEditor/Form";
import { NextRow } from "../../sceneEditor/wizards/Sheet";
import { urlError } from "./Custom";
import { General } from "./General";
import { MyServers } from "./MyServers";
import { ObsRemoteControl } from "./ObsRemoteControl";
import { ExternalLink, Field, pushPage, type Wizard, type WizardPageProps } from "./shared";
import { cleanUrl, isValidUrl } from "../../../data/streamUrls";
import { t, tRich } from "../../../i18n";

const pushObsRemoteControl = (wizard: Wizard) =>
  pushPage(wizard, t("OBS remote control"), ObsRemoteControl);

export function NetworkSetupObs(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set("networkSetup", "obs");
  const portError = () => {
    const port = wizard.state.obsPort.trim();
    if (port === "" || (/^\d+$/.test(port) && Number(port) > 0 && Number(port) <= 65535)) {
      return "";
    }
    return t("Must be a number between 1 and 65535.");
  };
  const nextDisabled = () =>
    wizard.state.obsAddress.trim() === "" ||
    wizard.state.obsPort.trim() === "" ||
    portError() !== "";
  return (
    <>
      <Field
        header={t("IP address or domain name")}
        value={wizard.state.obsAddress}
        placeholder="213.33.45.132"
        footer={t("Your public IP address if streaming over the internet.")}
        onInput={(address) => wizard.set("obsAddress", address)}
      />
      <Field
        header={t("Port")}
        value={wizard.state.obsPort}
        placeholder="7654"
        error={portError()}
        footer={t("Configure port forwarding in your router to forward incoming traffic to OBS.")}
        onInput={(port) => wizard.set("obsPort", port)}
      />
      <Section header={t("Configure OBS on your computer")}>
        <div class="flex flex-col gap-4 px-4 py-2 text-[17px]">
          <span>
            {t("1. Create a Media Source in OBS and configure it as shown in the image below.")}
          </span>
          <span>{t("2. Replace 7654 with your port.")}</span>
        </div>
      </Section>
      <NextRow disabled={nextDisabled()} onClick={() => pushObsRemoteControl(wizard)} />
    </>
  );
}

function NetworkSetupBelabox(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set("networkSetup", "belaboxCloudObs");
  const error = () => urlError(wizard.state.belaboxUrl, ["srt", "srtla"]);
  return (
    <>
      <Field
        header={t("Ingest URL")}
        value={wizard.state.belaboxUrl}
        placeholder="srtla://uk.srt.belabox.net:5000?streamid=jO4ijfFgrlpv4m2375msdoG3DDr2"
        error={error()}
        footer={t(
          'Press "Add automatically to Moblin" on https://cloud.belabox.net SRT(LA) relays (requires login). See screenshot below.',
        )}
        onInput={(url) => wizard.set("belaboxUrl", url)}
      />
      <NextRow
        disabled={wizard.state.belaboxUrl.trim() === "" || error() !== ""}
        onClick={() => pushObsRemoteControl(wizard)}
      />
    </>
  );
}

function NetworkSetupDirect(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set("networkSetup", "direct");
  const ingestError = () => {
    const url = cleanUrl(wizard.state.directIngest);
    return url === "" ? "" : (isValidUrl(url, undefined, false) ?? "");
  };
  const nextDisabled = () =>
    wizard.state.directIngest === "" || wizard.state.directStreamKey === "" || ingestError() !== "";
  const twitchStreamKeyUrl = () =>
    `https://dashboard.twitch.tv/u/${wizard.state.twitchChannelName.trim()}/settings/stream`;
  const ingest = (header: string, placeholder: string, footer: string) => (
    <Field
      header={header}
      value={wizard.state.directIngest}
      placeholder={placeholder}
      error={ingestError()}
      footer={footer}
      onInput={(url) => wizard.set("directIngest", url)}
    />
  );
  const streamKey = (placeholder: string, footer: JSX.Element) => (
    <Field
      header={t("Stream key")}
      value={wizard.state.directStreamKey}
      placeholder={placeholder}
      footer={footer}
      onInput={(key) => wizard.set("directStreamKey", key)}
    />
  );
  return (
    <>
      <Switch>
        <Match when={wizard.state.platform === "twitch"}>
          {ingest(
            t("Nearby ingest endpoint"),
            "rtmp://arn03.contribute.live-video.net/app",
            t(
              "Copy from https://help.twitch.tv/s/twitch-ingest-recommendation. Remove {stream_key}.",
            ),
          )}
          {streamKey(
            "live_48950233_okF4f455GRWEF443fFr23GRbt5rEv",
            tRich("Copy from {link} (requires login).", {
              link: <ExternalLink href={twitchStreamKeyUrl()} />,
            }),
          )}
        </Match>
        <Match when={wizard.state.platform === "kick"}>
          {ingest(
            t("Stream URL"),
            "rtmps://fa723fc1b171.global-contribute.live-video.net",
            t("Copy from https://kick.com/dashboard/settings/stream (requires login)."),
          )}
          {streamKey(
            "sk_us-west-2_okfef49k34k_34g59gGDDHGHSREj754gYJYTJERH",
            t("Copy from https://kick.com/dashboard/settings/stream (requires login)."),
          )}
        </Match>
        <Match when={wizard.state.platform === "youTube"}>
          {ingest(
            t("Stream URL"),
            "rtmp://a.rtmp.youtube.com/live2",
            t("Copy from https://youtube.com (requires login)."),
          )}
          {streamKey(
            "4bkf-8d03-g6w3-ekjh-emdc",
            t("Copy from https://youtube.com (requires login)."),
          )}
        </Match>
        <Match when={wizard.state.platform === "soop"}>
          {ingest(t("Stream URL"), "???", t("Copy from ??? (requires login)."))}
          {streamKey("???", t("Copy from ??? (requires login)."))}
        </Match>
      </Switch>
      <NextRow disabled={nextDisabled()} onClick={() => pushPage(wizard, t("General"), General)} />
    </>
  );
}

function FlowRow(props: { steps: string[]; footer: string; onClick: () => void }) {
  return (
    <Section footer={props.footer}>
      <button type="button" class={rowClass} onClick={() => props.onClick()}>
        <span class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
          <For each={props.steps}>
            {(step, index) => (
              <>
                {index() > 0 ? (
                  <Icon name="arrow-right" class="size-[14px] shrink-0 text-phone-border" />
                ) : null}
                <span>{step}</span>
              </>
            )}
          </For>
        </span>
        <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
      </button>
    </Section>
  );
}

export function NetworkSetup(props: WizardPageProps & { platform: string }) {
  const wizard = props.wizard;
  return (
    <>
      <FlowRow
        steps={[t("Moblin"), t("OBS"), props.platform]}
        footer={t("Good stability in most network conditions.")}
        onClick={() => pushPage(wizard, t("OBS"), NetworkSetupObs)}
      />
      <FlowRow
        steps={[t("Moblin"), t("BELABOX cloud"), t("OBS"), props.platform]}
        footer={t("Best possible stability. Uses bonding. Paid third-party service.")}
        onClick={() => pushPage(wizard, t("BELABOX cloud and OBS"), NetworkSetupBelabox)}
      />
      <FlowRow
        steps={[t("Moblin"), props.platform]}
        footer={t(
          "Often bad stability if network connection is unstable. No server side disconnection protection possible.",
        )}
        onClick={() => pushPage(wizard, t("Direct"), NetworkSetupDirect)}
      />
      <FlowRow
        steps={[t("Moblin"), t("My server(s)"), props.platform]}
        footer={t("Best possible stability. May use bonding. Most flexible setup.")}
        onClick={() => pushPage(wizard, t("My server(s)"), MyServers)}
      />
    </>
  );
}

export const pushNetworkSetup = (wizard: Wizard, platform: string) =>
  wizard.stack.push({
    title: t("Network setup"),
    render: () => <NetworkSetup wizard={wizard} platform={platform} />,
  });
