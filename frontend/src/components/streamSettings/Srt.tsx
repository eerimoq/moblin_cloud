import { Show } from "solid-js";
import { Toggle } from "../MoblinParts";
import {
  LinkRow,
  MenuRow,
  ReorderableList,
  Section,
  ToggleRow,
  useReadOnly,
} from "../sceneEditor/Form";
import {
  CommitSlider,
  EditRow,
  liveDisabled,
  Lines,
  parseInt32,
  TextBlock,
  ToggleLinkRow,
} from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import { formatBits } from "../../data/moblin";
import { sameId } from "../../data/moblinSettings";
import {
  isSrtla,
  srtAlgorithmName,
  streamProtocol,
  urlScheme,
  type DnsLookupStrategy,
  type SettingsStream,
  type SrtAdaptiveBitrateAlgorithm,
  type SrtConnectionPriority,
  type SrtImplementation,
} from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

const integerRange = (minimum: number, maximum: number) => (value: string) => {
  const number = parseInt32(value);
  if (number === null) {
    return t("Not a number");
  }
  if (number < minimum) {
    return t("Too small");
  }
  if (number > maximum) {
    return t("Too big");
  }
  return null;
};

export function SrtSettings(props: StreamFormProps) {
  const srt = () => props.stream.srt;
  const edit = (change: (srt: SettingsStream["srt"]) => void) =>
    props.edit((draft) => change(draft.srt));
  return (
    <>
      <Section
        footer={t(
          "Big packets means 7 MPEG-TS packets per SRT packet, 6 otherwise. Sometimes Android hotspots does not work with big packets.",
        )}
      >
        <EditRow
          stack={props.stack}
          label={t("Latency")}
          value={String(srt().latency)}
          display={`${srt().latency} ms`}
          disabled={liveDisabled(props)}
          validate={integerRange(0, 65535)}
          onSubmit={(value) =>
            edit((draft) => {
              draft.latency = Number(value);
            })
          }
        />
        <Show when={srt().implementation === "Moblin" && srt().latency < 1000}>
          <TextBlock>
            {t(
              '⚠️ The "Moblin" implementation does not perform well with low latency. Select the "Official" implementation at the bottom of this page.',
            )}
          </TextBlock>
        </Show>
        <ToggleLinkRow
          label={t("Adaptive bitrate")}
          on={srt().adaptiveBitrateEnabled}
          disabled={liveDisabled(props)}
          onToggle={(on) =>
            edit((draft) => {
              draft.adaptiveBitrateEnabled = on;
            })
          }
          onClick={() =>
            props.stack.push({
              title: t("Adaptive bitrate"),
              render: () => <SrtAdaptiveBitrateSettings {...props} />,
            })
          }
        />
        <LinkRow
          label={t("Connection priorities")}
          onClick={() =>
            props.stack.push({
              title: t("Connection priorities"),
              render: () => <SrtConnectionPrioritiesSettings {...props} />,
            })
          }
        />
        <Show when={srt().implementation === "Official"}>
          <ToggleRow
            label={t("Max bandwidth follows input")}
            on={srt().maximumBandwidthFollowInput}
            disabled={liveDisabled(props)}
            onToggle={(on) =>
              edit((draft) => {
                draft.maximumBandwidthFollowInput = on;
              })
            }
          />
          <EditRow
            stack={props.stack}
            label={t("Overhead bandwidth")}
            value={String(srt().overheadBandwidth)}
            display={`${srt().overheadBandwidth}%`}
            disabled={liveDisabled(props)}
            validate={integerRange(5, 100)}
            onSubmit={(value) =>
              edit((draft) => {
                draft.overheadBandwidth = Number(value);
              })
            }
          />
        </Show>
        <ToggleRow
          label={t("Big packets")}
          on={srt().bigPackets}
          disabled={liveDisabled(props)}
          onToggle={(on) =>
            edit((draft) => {
              draft.bigPackets = on;
            })
          }
        />
      </Section>
      <Section
        footer={t("System seems to work best for TMobile. IPv4 probably best for IRLToolkit.")}
      >
        <MenuRow
          label={t("DNS lookup strategy")}
          items={[
            { id: "System", name: t("System") },
            { id: "IPv4", name: "IPv4" },
            { id: "IPv6", name: "IPv6" },
            { id: "IPv4 and IPv6", name: t("IPv4 and IPv6") },
          ]}
          selected={srt().dnsLookupStrategy}
          disabled={liveDisabled(props)}
          onPick={(id) =>
            edit((draft) => {
              draft.dnsLookupStrategy = id as DnsLookupStrategy;
            })
          }
        />
      </Section>
      <Section
        footer={t(
          '"Official" uses the widely supported libSRT (version 1.5.3) and "Moblin" uses a more energy efficient custom implementation.',
        )}
      >
        <MenuRow
          label={t("Implementation")}
          items={[
            { id: "Moblin", name: t("Moblin") },
            { id: "Official", name: t("Official") },
          ]}
          selected={srt().implementation}
          disabled={liveDisabled(props)}
          onPick={(id) =>
            edit((draft) => {
              draft.implementation = id as SrtImplementation;
            })
          }
        />
      </Section>
    </>
  );
}

const minimumBitrateFormat = (value: number) => formatBits(Math.trunc(value));

const packetsFormat = (value: number) => String(Math.trunc(value));

export function SrtAdaptiveBitrateSettings(props: StreamFormProps) {
  const adaptiveBitrate = () => props.stream.srt.adaptiveBitrate;
  const algorithm = () => srtAlgorithmName(adaptiveBitrate().algorithm);
  const edit = (change: (adaptiveBitrate: SettingsStream["srt"]["adaptiveBitrate"]) => void) =>
    props.edit((draft) => change(draft.srt.adaptiveBitrate));
  const custom = () => adaptiveBitrate().customSettings;
  return (
    <>
      <Section
        footer={t(
          "BELABOX and Fast IRL are the safest options. Choose the others only if you know what you are doing!",
        )}
      >
        <MenuRow
          label={t("Algorithm")}
          items={[
            { id: "belabox", name: t("BELABOX") },
            { id: "fastIrl", name: t("Fast IRL") },
            { id: "slowIrl", name: t("Slow IRL") },
            { id: "customIrl", name: t("Custom IRL") },
          ]}
          selected={algorithm()}
          onPick={(id) =>
            edit((draft) => {
              draft.algorithm = { [id]: {} } as SrtAdaptiveBitrateAlgorithm;
            })
          }
        />
      </Section>
      <Show when={algorithm() === "fastIrl"}>
        <Section
          header={t("Packets in flight decrease threshold")}
          footer={
            <Lines
              lines={[
                t(
                  "The bitrate will decrease quickly when the number of packets in flight are above this value.",
                ),
                t("200 by default."),
              ]}
            />
          }
        >
          <CommitSlider
            value={adaptiveBitrate().fastIrlSettings.packetsInFlight}
            minimum={200}
            maximum={700}
            step={10}
            format={packetsFormat}
            onSubmit={(value) =>
              edit((draft) => {
                draft.fastIrlSettings.packetsInFlight = Math.trunc(value);
              })
            }
          />
        </Section>
        <Section
          header={t("Minimum bitrate")}
          footer={<Lines lines={[t("The minimum encoder bitrate."), t("250 Kbps by default.")]} />}
        >
          <CommitSlider
            value={1000 * adaptiveBitrate().fastIrlSettings.minimumBitrate}
            minimum={50000}
            maximum={2_000_000}
            step={50000}
            format={minimumBitrateFormat}
            onSubmit={(value) =>
              edit((draft) => {
                draft.fastIrlSettings.minimumBitrate = value / 1000;
              })
            }
          />
        </Section>
      </Show>
      <Show when={algorithm() === "customIrl"}>
        <Section>
          <TextBlock>
            {"⚠️ "}
            {t("Finding good parameters is hard. You are on you own! =)")}
          </TextBlock>
        </Section>
        <Section header={t("Bitrate increase speed")}>
          <CommitSlider
            value={custom().pifDiffIncreaseFactor}
            minimum={5}
            maximum={500}
            step={5}
            format={(value) => `${formatBits(Math.trunc(value * 1000))}/sec`}
            onSubmit={(value) =>
              edit((draft) => {
                draft.customSettings.pifDiffIncreaseFactor = value;
              })
            }
          />
        </Section>
        <Section
          header={t("Bitrate decrease speed")}
          footer={t("The bitrate decrease speed when RTT is too high.")}
        >
          <CommitSlider
            value={100 * (1 - Math.fround(Math.pow(custom().rttDiffHighDecreaseFactor, 5)))}
            minimum={10}
            maximum={50}
            step={1}
            format={(value) => `${Math.round(value)} %/sec`}
            onSubmit={(value) =>
              edit((draft) => {
                draft.customSettings.rttDiffHighDecreaseFactor = Math.fround(
                  Math.pow(1 - value / 100, 0.2),
                );
              })
            }
          />
        </Section>
        <Section
          header={t("Minimum bitrate decrease speed")}
          footer={t("The minimum bitrate decrease speed when RTT is too high.")}
        >
          <CommitSlider
            value={5 * 1000 * custom().rttDiffHighMinimumDecrease}
            minimum={25000}
            maximum={2_000_000}
            step={5000}
            format={(value) => `${formatBits(Math.trunc(value))}/sec`}
            onSubmit={(value) =>
              edit((draft) => {
                draft.customSettings.rttDiffHighMinimumDecrease = value / 5 / 1000;
              })
            }
          />
        </Section>
        <Section
          header={t("Packets in flight decrease threshold")}
          footer={t(
            "The bitrate will decrease quickly when the number of packets in flight are above this value.",
          )}
        >
          <CommitSlider
            value={custom().packetsInFlight}
            minimum={50}
            maximum={5000}
            step={5}
            format={packetsFormat}
            onSubmit={(value) =>
              edit((draft) => {
                draft.customSettings.packetsInFlight = Math.trunc(value);
              })
            }
          />
        </Section>
        <Section
          header={t("Allowed RTT spike")}
          footer={t("The maximum allowed RTT spike before decreasing the bitrate")}
        >
          <CommitSlider
            value={custom().rttDiffHighAllowedSpike}
            minimum={25}
            maximum={1000}
            step={5}
            format={packetsFormat}
            onSubmit={(value) =>
              edit((draft) => {
                draft.customSettings.rttDiffHighAllowedSpike = value;
              })
            }
          />
        </Section>
        <Section header={t("Minimum bitrate")} footer={t("The minimum encoder bitrate.")}>
          <CommitSlider
            value={1000 * custom().minimumBitrate}
            minimum={50000}
            maximum={2_000_000}
            step={50000}
            format={minimumBitrateFormat}
            onSubmit={(value) =>
              edit((draft) => {
                draft.customSettings.minimumBitrate = value / 1000;
              })
            }
          />
        </Section>
      </Show>
      <Show when={algorithm() === "belabox"}>
        <Section
          header={t("Minimum bitrate")}
          footer={<Lines lines={[t("The minimum encoder bitrate."), t("250 Kbps by default.")]} />}
        >
          <CommitSlider
            value={1000 * adaptiveBitrate().belaboxSettings.minimumBitrate}
            minimum={50000}
            maximum={2_000_000}
            step={50000}
            format={minimumBitrateFormat}
            onSubmit={(value) =>
              edit((draft) => {
                draft.belaboxSettings.minimumBitrate = value / 1000;
              })
            }
          />
        </Section>
      </Show>
    </>
  );
}

const protocolName = (stream: SettingsStream) => {
  const protocol = streamProtocol(stream);
  if (protocol === "SRT" && isSrtla(stream)) {
    return "SRTLA";
  }
  if (protocol === "RTMP" && urlScheme(stream.url) === "rtmps") {
    return "RTMPS";
  }
  return protocol;
};

function PriorityRow(props: {
  priority: SrtConnectionPriority;
  onEnabled: (enabled: boolean) => void;
  onPriority: (priority: number) => void;
}) {
  const readOnly = useReadOnly();
  return (
    <div class="flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-[17px]">
      <span class="w-[90px] shrink-0 truncate text-center">{props.priority.name}</span>
      <input
        type="range"
        class="min-w-0 flex-1 accent-white"
        min={1}
        max={10}
        step={1}
        value={props.priority.priority}
        disabled={readOnly()}
        onChange={(event) => props.onPriority(Number(event.currentTarget.value))}
      />
      <Toggle
        label={props.priority.name}
        on={props.priority.enabled}
        disabled={readOnly()}
        onToggle={props.onEnabled}
      />
    </div>
  );
}

export function SrtConnectionPrioritiesSettings(props: StreamFormProps) {
  const priorities = () => props.stream.srt.connectionPriorities;
  const editPriority = (id: string, change: (priority: SrtConnectionPriority) => void) =>
    props.edit((draft) => {
      const priority = draft.srt.connectionPriorities.priorities.find((candidate) =>
        sameId(candidate.id, id),
      );
      if (priority) {
        change(priority);
      }
    });
  return (
    <Show
      when={isSrtla(props.stream)}
      fallback={
        <Section>
          <TextBlock>
            {t(
              "Connection priorities are not supported by {name}. Only SRTLA supports connection priorities.",
              { name: protocolName(props.stream) },
            )}
          </TextBlock>
        </Section>
      }
    >
      <Section>
        <ToggleRow
          label={t("Enabled")}
          on={priorities().enabled}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.srt.connectionPriorities.enabled = on;
            })
          }
        />
      </Section>
      <Section
        footer={
          <Lines
            lines={[
              t(
                "A connection with high priority will be used more than a connection with low priority if the high priority connection is stable. Unstable connections will get lowest priority regardless of configured priority until they are stable again.",
              ),
              "",
              t("Disabled connections will not be used."),
            ]}
          />
        }
      >
        <ReorderableList
          items={priorities().priorities}
          render={(priority) => (
            <PriorityRow
              priority={priority}
              onEnabled={(enabled) =>
                editPriority(priority.id, (draft) => {
                  draft.enabled = enabled;
                })
              }
              onPriority={(value) =>
                editPriority(priority.id, (draft) => {
                  draft.priority = Math.min(10, Math.max(1, Math.trunc(value)));
                })
              }
            />
          )}
          onDelete={(priority) =>
            props.edit((draft) => {
              draft.srt.connectionPriorities.priorities =
                draft.srt.connectionPriorities.priorities.filter(
                  (candidate) => !sameId(candidate.id, priority.id),
                );
            })
          }
          deleteDisabled={(priority) => ["Cellular", "WiFi"].includes(priority.name)}
        />
      </Section>
    </Show>
  );
}
