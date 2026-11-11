import { createSignal } from "solid-js";
import {
  LinkRow,
  MenuRow,
  Section,
  SliderRow,
  TextEditPage,
  ToggleRow,
  ValueRow,
} from "../sceneEditor/Form";
import type { Stack } from "../sceneEditor/Stack";
import { formatShortDuration } from "../sceneEditor/settings/widgets/shared";
import { inUse, isLive, isRecording, type StreamFormProps } from "./StreamsSettings";
import {
  streamResolutions,
  type StreamCodec,
  type StreamResolution,
} from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

export const formatBytesPerSecond = (speed: number) => {
  if (speed < 1000) {
    return `${speed} bps`;
  }
  if (speed < 1_000_000) {
    return `${Math.round(speed / 1000)} Kbps`;
  }
  if (speed < 1_000_000_000) {
    return `${Number((speed / 1_000_000).toFixed(1))} Mbps`;
  }
  return `${Number((speed / 1_000_000_000).toFixed(2))} Gbps`;
};

const codecs: StreamCodec[] = ["H.265/HEVC", "H.264/AVC"];

function EditRow(props: {
  stack: Stack;
  label: string;
  display: string;
  value: string;
  footers: string[];
  disabled: boolean;
  onSubmit: (value: string) => void;
}) {
  return (
    <LinkRow
      label={props.label}
      value={props.display}
      disabled={props.disabled}
      onClick={() =>
        props.stack.push({
          title: props.label,
          render: () => (
            <TextEditPage
              value={props.value}
              footers={props.footers}
              onSubmit={props.onSubmit}
              onDone={() => props.stack.pop()}
            />
          ),
        })
      }
    />
  );
}

function RecordingAudioSettings(props: StreamFormProps) {
  const [bitrate, setBitrate] = createSignal(
    Math.floor(props.stream.recording.audioBitrate / 1000),
  );
  const calcBitrate = (value: number) => Math.ceil(value * 1000);
  return (
    <Section
      header={t("Bitrate")}
      footer={
        <span class="flex flex-col">
          <span>{t("128 Kbps or higher is recommended. Set to 0 for automatic.")}</span>
          <span>&nbsp;</span>
          <span>{t("320 Kbps typically requires stereo mic.")}</span>
        </span>
      }
    >
      <fieldset class="disabled:opacity-50" disabled={inUse(props) && isRecording(props.remote)}>
        <SliderRow
          value={bitrate()}
          minimum={0}
          maximum={320}
          step={32}
          format={(value) => formatBytesPerSecond(calcBitrate(value))}
          onChange={setBitrate}
          onCommit={(value) => {
            setBitrate(value);
            props.edit((draft) => {
              draft.recording.audioBitrate = calcBitrate(value);
            });
          }}
        />
      </fieldset>
    </Section>
  );
}

export function RecordingSettings(props: StreamFormProps) {
  const recording = () => props.stream.recording;
  const overrideDisabled = () =>
    inUse(props) && (isLive(props.remote) || isRecording(props.remote));
  const recordingDisabled = () => inUse(props) && isRecording(props.remote);
  const videoBitrateString = () =>
    recording().videoBitrate !== 0 ? formatBytesPerSecond(recording().videoBitrate) : t("Auto");
  const maxKeyFrameIntervalString = () =>
    recording().maxKeyFrameInterval !== 0
      ? formatShortDuration(recording().maxKeyFrameInterval)
      : t("Auto");
  const audioBitrateString = () =>
    recording().audioBitrate !== 0 ? formatBytesPerSecond(recording().audioBitrate) : t("Auto");
  return (
    <>
      <Section
        footer={
          <span class="flex flex-col">
            <span>{t("Resolution and FPS are same as for live stream if not overridden.")}</span>
            <span>&nbsp;</span>
            <span>
              {t(
                "The overall energy consumption will be higher and the live stream image quality will be worse when the override is enabled, regardless of if you are recording or not.",
              )}
            </span>
          </span>
        }
      >
        <ToggleRow
          label={t("Override")}
          on={recording().overrideStream}
          disabled={overrideDisabled()}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.recording.overrideStream = on;
            })
          }
        />
        <MenuRow
          label={t("Resolution")}
          items={streamResolutions}
          selected={recording().resolution}
          disabled={overrideDisabled()}
          onPick={(id) =>
            props.edit((draft) => {
              draft.recording.resolution = id as StreamResolution;
            })
          }
        />
      </Section>
      <Section>
        <MenuRow
          label={t("Video codec")}
          items={codecs.map((codec) => ({ id: codec, name: codec }))}
          selected={recording().videoCodec}
          disabled={recordingDisabled()}
          onPick={(id) =>
            props.edit((draft) => {
              draft.recording.videoCodec = id as StreamCodec;
            })
          }
        />
        <EditRow
          stack={props.stack}
          label={t("Video bitrate")}
          display={videoBitrateString()}
          value={String(recording().videoBitrate / 1_000_000)}
          footers={[t("Up to 50 Mbps. Set to 0 for automatic.")]}
          disabled={recordingDisabled()}
          onSubmit={(value) => {
            const bitrate = Number(value);
            if (value === "" || Number.isNaN(bitrate)) {
              return;
            }
            props.edit((draft) => {
              draft.recording.videoBitrate = Math.floor(
                Math.min(50, Math.max(0, bitrate)) * 1_000_000,
              );
            });
          }}
        />
        <EditRow
          stack={props.stack}
          label={t("Key frame interval")}
          display={maxKeyFrameIntervalString()}
          value={String(recording().maxKeyFrameInterval)}
          footers={[t("Maximum key frame interval in seconds. Set to 0 for automatic.")]}
          disabled={recordingDisabled()}
          onSubmit={(value) => {
            if (!/^[+-]?\d+$/.test(value)) {
              return;
            }
            const interval = Number(value);
            if (interval < 0 || interval > 10) {
              return;
            }
            props.edit((draft) => {
              draft.recording.maxKeyFrameInterval = interval;
            });
          }}
        />
        <LinkRow
          label={t("Audio bitrate")}
          value={audioBitrateString()}
          disabled={recordingDisabled()}
          onClick={() =>
            props.stack.push({
              title: t("Audio"),
              render: () => <RecordingAudioSettings {...props} />,
            })
          }
        />
      </Section>
      <Section>
        <ValueRow
          label={t("Recording path")}
          value={recording().recordingPath ? t("Set on the phone.") : ""}
        />
      </Section>
      <Section footer={t("Do not show widgets in recordings.")}>
        <ToggleRow
          label={t("Clean recordings")}
          on={recording().cleanRecordings}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.recording.cleanRecordings = on;
            })
          }
        />
      </Section>
      <Section>
        <ToggleRow
          label={t("Auto start recording when going live")}
          on={recording().autoStartRecording}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.recording.autoStartRecording = on;
            })
          }
        />
        <ToggleRow
          label={t("Auto stop recording when ending stream")}
          on={recording().autoStopRecording}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.recording.autoStopRecording = on;
            })
          }
        />
      </Section>
    </>
  );
}
