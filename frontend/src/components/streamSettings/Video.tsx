import { createSignal, Show } from "solid-js";
import { MenuRow, Section, ToggleRow } from "../sceneEditor/Form";
import {
  CommitSlider,
  EditRow,
  liveDisabled,
  liveOrRecordingDisabled,
  Lines,
  parseInt32,
  showAllSettings,
  ToggleLinkRow,
} from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import { formatBits } from "../../data/moblin";
import {
  streamFpss,
  streamProtocol,
  streamResolutions,
  type StreamCodec,
  type StreamColorRange,
  type StreamH264Profile,
  type StreamRateControl,
  type StreamResolution,
} from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

function AdaptiveResolutionSettings(props: StreamFormProps) {
  const [dragging, setDragging] = createSignal<number | null>(null);
  const factor = () => dragging() ?? props.stream.adaptiveEncoderResolutionThreashold;
  const threshold = (bitrate: number, name: string) =>
    `< ${formatBits(Math.trunc(bitrate * factor()))} → ${name}`;
  return (
    <Section
      header={t("Thresholds")}
      footer={
        <Lines
          lines={[
            threshold(100_000, "160p"),
            threshold(250_000, "360p"),
            threshold(500_000, "480p"),
            threshold(750_000, "720p"),
            threshold(1_500_000, "1080p"),
          ]}
        />
      }
    >
      <CommitSlider
        value={props.stream.adaptiveEncoderResolutionThreashold}
        minimum={1}
        maximum={3}
        step={0.1}
        disabled={liveDisabled(props)}
        onChange={setDragging}
        onSubmit={(value) => {
          props.edit((draft) => {
            draft.adaptiveEncoderResolutionThreashold = value;
          });
          setDragging(null);
        }}
      />
    </Section>
  );
}

function TimecodesSettings(props: StreamFormProps) {
  return (
    <Section>
      <EditRow
        stack={props.stack}
        label={t("NTP pool address")}
        value={props.stream.ntpPoolAddress}
        disabled={props.stream.codec !== "H.265/HEVC" || liveDisabled(props)}
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.ntpPoolAddress = value;
          })
        }
      />
    </Section>
  );
}

export function VideoSettings(props: StreamFormProps) {
  const bitrateItems = () => {
    const items = (props.remote.settings?.bitratePresets ?? []).map((preset) => ({
      id: String(preset.bitrate),
      name: formatBits(preset.bitrate),
    }));
    return items.some((item) => item.id === String(props.stream.bitrate))
      ? items
      : [...items, { id: String(props.stream.bitrate), name: formatBits(props.stream.bitrate) }];
  };
  const timecodesDisabled = () =>
    !["SRT", "RIST"].includes(streamProtocol(props.stream)) ||
    props.stream.codec !== "H.265/HEVC" ||
    liveDisabled(props);
  return (
    <>
      <Section>
        <MenuRow
          label={t("Resolution")}
          items={streamResolutions}
          selected={props.stream.resolution}
          disabled={liveOrRecordingDisabled(props)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.resolution = id as StreamResolution;
            })
          }
        />
      </Section>
      <Section footer={t("Lower FPS generally gives brighter image in low light conditions.")}>
        <MenuRow
          label={t("FPS")}
          items={streamFpss.map((fps) => ({ id: String(fps), name: String(fps) }))}
          selected={String(props.stream.fps)}
          disabled={liveOrRecordingDisabled(props)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.fps = Number(id);
            })
          }
        />
      </Section>
      <Section
        footer={t(
          "Enable low light boost to make builtin cameras automatically lower the selected FPS for brighter image when dark (if supported).",
        )}
      >
        <ToggleRow
          label={t("Low light boost (LLB)")}
          on={props.stream.autoFps}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.autoFps = on;
            })
          }
        />
      </Section>
      <Section
        footer={t(
          "H.265/HEVC generally requires less bandwidth for same image quality. RTMP generally only supports H.264/AVC.",
        )}
      >
        <MenuRow
          label={t("Codec")}
          items={[
            { id: "H.265/HEVC", name: "H.265/HEVC" },
            { id: "H.264/AVC", name: "H.264/AVC" },
          ]}
          selected={props.stream.codec}
          disabled={liveDisabled(props)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.codec = id as StreamCodec;
            })
          }
        />
        <Show when={props.stream.codec === "H.264/AVC"}>
          <MenuRow
            label={t("Profile")}
            items={[
              { id: "Baseline", name: "Baseline" },
              { id: "Main", name: "Main" },
              { id: "High", name: "High" },
            ]}
            selected={props.stream.h264Profile}
            disabled={liveDisabled(props)}
            onPick={(id) =>
              props.edit((draft) => {
                draft.h264Profile = id as StreamH264Profile;
              })
            }
          />
        </Show>
      </Section>
      <Show when={showAllSettings(props)}>
        <Section>
          <MenuRow
            label={t("Rate control")}
            items={[
              { id: "ABR", name: t("ABR (Average)") },
              { id: "CBR", name: t("CBR (Constant)") },
              { id: "VBR", name: t("VBR (Variable)") },
            ]}
            selected={props.stream.bitrateRateControl}
            disabled={liveDisabled(props)}
            onPick={(id) =>
              props.edit((draft) => {
                draft.bitrateRateControl = id as StreamRateControl;
              })
            }
          />
        </Section>
        <Section footer={t("About 5-8 Mbps is usually enough for decent image quality.")}>
          <MenuRow
            label={t("Bitrate")}
            items={bitrateItems()}
            selected={String(props.stream.bitrate)}
            onPick={(id) =>
              props.edit((draft) => {
                draft.bitrate = Number(id);
              })
            }
          />
        </Section>
        <Section>
          <EditRow
            stack={props.stack}
            label={t("Key frame interval")}
            value={String(props.stream.maxKeyFrameInterval)}
            display={
              props.stream.maxKeyFrameInterval === 0
                ? t("Auto")
                : t("{count} sec", { count: props.stream.maxKeyFrameInterval })
            }
            footers={[t("Maximum key frame interval in seconds. Set to 0 for automatic.")]}
            disabled={liveDisabled(props)}
            onSubmit={(value) => {
              const interval = parseInt32(value);
              if (interval === null || interval < 0 || interval > 10) {
                return;
              }
              props.edit((draft) => {
                draft.maxKeyFrameInterval = interval;
              });
            }}
          />
        </Section>
        <Section>
          <ToggleRow
            label={t("B-frames")}
            on={props.stream.bFrames}
            disabled={liveDisabled(props)}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.bFrames = on;
              })
            }
          />
        </Section>
        <Section
          footer={t(
            "Color range of streams and recordings. Full range uses all levels, while limited range is the broadcast standard expected by some servers and players. Color space is configured in Settings → Camera.",
          )}
        >
          <MenuRow
            label={t("Color range")}
            items={[
              { id: "Full", name: t("Full") },
              { id: "Limited", name: t("Limited") },
            ]}
            selected={props.stream.colorRange}
            disabled={liveOrRecordingDisabled(props)}
            onPick={(id) =>
              props.edit((draft) => {
                draft.colorRange = id as StreamColorRange;
              })
            }
          />
        </Section>
        <Section
          footer={
            <Lines
              lines={[
                t(
                  "Automatically lower resolution when the available bandwidth is low. Generally gives better image quality at low (< 750 Kbps) bitrates.",
                ),
                "",
                t("Warning: OBS typically requires hardware decoding not to crash when enabled."),
              ]}
            />
          }
        >
          <ToggleLinkRow
            label={t("Adaptive resolution")}
            on={props.stream.adaptiveEncoderResolution}
            disabled={liveDisabled(props)}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.adaptiveEncoderResolution = on;
              })
            }
            onClick={() =>
              props.stack.push({
                title: t("Adaptive resolution"),
                render: () => <AdaptiveResolutionSettings {...props} />,
              })
            }
          />
        </Section>
        <Section
          footer={t(
            "Synchronize multiple streams on your server using timecodes. Timecodes are in UTC and requires H.265/HEVC codec and SRT(LA) or RIST.",
          )}
        >
          <ToggleLinkRow
            label={t("Timecodes")}
            on={props.stream.timecodesEnabled}
            disabled={timecodesDisabled()}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.timecodesEnabled = on;
              })
            }
            onClick={() =>
              props.stack.push({
                title: t("Timecodes"),
                render: () => <TimecodesSettings {...props} />,
              })
            }
          />
        </Section>
      </Show>
    </>
  );
}
