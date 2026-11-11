import { LinkRow, Section } from "../../sceneEditor/Form";
import { NextRow } from "../../sceneEditor/wizards/Sheet";
import { RtmpUrlFields, rtmpNextDisabled, SrtUrlFields, srtNextDisabled } from "./Custom";
import { ObsRemoteControl } from "./ObsRemoteControl";
import { pushPage, type Wizard, type WizardPageProps } from "./shared";
import { t } from "../../../i18n";

const pushObsRemoteControl = (wizard: Wizard) =>
  pushPage(wizard, t("OBS remote control"), ObsRemoteControl);

function MyServersSrt(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set("customProtocol", "srt");
  return (
    <>
      <SrtUrlFields wizard={wizard} />
      <NextRow disabled={srtNextDisabled(wizard)} onClick={() => pushObsRemoteControl(wizard)} />
    </>
  );
}

function MyServersRtmp(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set("customProtocol", "rtmp");
  return (
    <>
      <RtmpUrlFields wizard={wizard} />
      <NextRow disabled={rtmpNextDisabled(wizard)} onClick={() => pushObsRemoteControl(wizard)} />
    </>
  );
}

export function MyServers(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set("networkSetup", "myServers");
  return (
    <Section header={t("Protocol")}>
      <LinkRow label={t("SRT(LA)")} onClick={() => pushPage(wizard, t("SRT(LA)"), MyServersSrt)} />
      <LinkRow label={t("RTMP(S)")} onClick={() => pushPage(wizard, t("RTMP(S)"), MyServersRtmp)} />
    </Section>
  );
}
