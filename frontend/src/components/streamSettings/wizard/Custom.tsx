import { LinkRow, Section } from "../../sceneEditor/Form";
import { NextRow } from "../../sceneEditor/wizards/Sheet";
import { General } from "./General";
import { Field, pushPage, SkipRow, type Wizard, type WizardPageProps } from "./shared";
import { extractSrtStreamId } from "../../../data/streamWizard";
import { cleanUrl, isValidUrl } from "../../../data/streamUrls";
import { t } from "../../../i18n";

export const urlError = (url: string, allowedSchemes: string[], rtmpStreamKeyRequired = true) => {
  const cleaned = cleanUrl(url);
  return cleaned === "" ? "" : (isValidUrl(cleaned, allowedSchemes, rtmpStreamKeyRequired) ?? "");
};

export const srtUrlError = (wizard: Wizard) =>
  urlError(wizard.state.customSrtUrl, ["srt", "srtla"]);

export const srtNextDisabled = (wizard: Wizard) =>
  wizard.state.customSrtUrl === "" ||
  wizard.state.customSrtStreamId === "" ||
  srtUrlError(wizard) !== "";

export const rtmpUrlError = (wizard: Wizard) =>
  urlError(wizard.state.customRtmpUrl, ["rtmp", "rtmps"], false);

export const rtmpNextDisabled = (wizard: Wizard) =>
  wizard.state.customRtmpUrl === "" ||
  wizard.state.customRtmpStreamKey === "" ||
  rtmpUrlError(wizard) !== "";

export function SrtUrlFields(props: WizardPageProps) {
  const wizard = props.wizard;
  return (
    <>
      <Field
        header={t("URL")}
        value={wizard.state.customSrtUrl}
        placeholder="srt://107.32.12.132:5000?streamid=1234"
        error={srtUrlError(wizard)}
        onInput={(url) =>
          wizard.set({ customSrtUrl: url, customSrtStreamId: extractSrtStreamId(url) ?? "" })
        }
      />
      <Field
        header={t("Stream id")}
        value={wizard.state.customSrtStreamId}
        placeholder="#!::r=stream/-NDZ1WPA4zjMBTJTyNwU,m=publish,..."
        footer={t("Replaces or adds the stream id to the URL.")}
        onInput={(streamId) => wizard.set("customSrtStreamId", streamId)}
      />
    </>
  );
}

export function RtmpUrlFields(props: WizardPageProps) {
  const wizard = props.wizard;
  return (
    <>
      <Field
        header={t("URL")}
        value={wizard.state.customRtmpUrl}
        placeholder="rtmp://arn03.contribute.live-video.net/app/"
        error={rtmpUrlError(wizard)}
        onInput={(url) => wizard.set("customRtmpUrl", url)}
      />
      <Field
        header={t("Stream key")}
        value={wizard.state.customRtmpStreamKey}
        placeholder="live_48950233_okF4f455GRWEF443fFr23GRbt5rEv"
        onInput={(streamKey) => wizard.set("customRtmpStreamKey", streamKey)}
      />
    </>
  );
}

const pushGeneral = (wizard: Wizard) => pushPage(wizard, t("General"), General);

function CustomSrt(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ customProtocol: "srt", name: wizard.uniqueName(t("Custom SRT")) });
  return (
    <>
      <SrtUrlFields wizard={wizard} />
      <NextRow disabled={srtNextDisabled(wizard)} onClick={() => pushGeneral(wizard)} />
    </>
  );
}

function CustomRtmp(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ customProtocol: "rtmp", name: wizard.uniqueName(t("Custom RTMP")) });
  return (
    <>
      <RtmpUrlFields wizard={wizard} />
      <NextRow disabled={rtmpNextDisabled(wizard)} onClick={() => pushGeneral(wizard)} />
    </>
  );
}

function CustomRist(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ customProtocol: "rist", name: wizard.uniqueName(t("Custom RIST")) });
  const error = () => urlError(wizard.state.customRistUrl, ["rist"]);
  return (
    <>
      <Field
        header={t("URL")}
        value={wizard.state.customRistUrl}
        placeholder="rist://120.35.234.2:2030"
        error={error()}
        onInput={(url) => wizard.set("customRistUrl", url)}
      />
      <NextRow
        disabled={wizard.state.customRistUrl === "" || error() !== ""}
        onClick={() => pushGeneral(wizard)}
      />
    </>
  );
}

function CustomWhip(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ customProtocol: "whip", name: wizard.uniqueName(t("Custom WHIP")) });
  const error = () => urlError(wizard.state.customWhipUrl, ["whip", "whips"]);
  return (
    <>
      <Field
        header={t("URL")}
        value={wizard.state.customWhipUrl}
        placeholder="whip://120.12.32.12:8889/mystream/whip"
        error={error()}
        onInput={(url) => wizard.set("customWhipUrl", url)}
      />
      <NextRow
        disabled={wizard.state.customWhipUrl === "" || error() !== ""}
        onClick={() => pushGeneral(wizard)}
      />
    </>
  );
}

export function Custom(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ platform: "custom", customProtocol: "none", name: wizard.uniqueName(t("Custom")) });
  return (
    <>
      <Section header={t("Protocol")}>
        <LinkRow label={t("SRT(LA)")} onClick={() => pushPage(wizard, t("SRT(LA)"), CustomSrt)} />
        <LinkRow label={t("RTMP(S)")} onClick={() => pushPage(wizard, t("RTMP(S)"), CustomRtmp)} />
        <LinkRow label={t("RIST")} onClick={() => pushPage(wizard, t("RIST"), CustomRist)} />
        <LinkRow label={t("WHIP")} onClick={() => pushPage(wizard, t("WHIP"), CustomWhip)} />
      </Section>
      <SkipRow onClick={() => pushGeneral(wizard)} />
    </>
  );
}
