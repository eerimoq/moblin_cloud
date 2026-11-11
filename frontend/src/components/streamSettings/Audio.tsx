import { MenuRow, Section } from "../sceneEditor/Form";
import { CommitSlider, liveDisabled, Lines } from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import { formatBits } from "../../data/moblin";
import type { StreamAudioCodec } from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

const bitrateOf = (kbps: number) => Math.ceil(kbps * 1000);

export function AudioSettings(props: StreamFormProps) {
  return (
    <>
      <Section footer={t("WHIP generally only supports Opus.")}>
        <MenuRow
          label={t("Codec")}
          items={[
            { id: "AAC", name: "AAC" },
            { id: "OPUS", name: "Opus" },
          ]}
          selected={props.stream.audioCodec}
          disabled={liveDisabled(props)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.audioCodec = id as StreamAudioCodec;
            })
          }
        />
      </Section>
      <Section
        header={t("Bitrate")}
        footer={
          <Lines
            lines={[
              t("128 Kbps or higher is recommended."),
              "",
              t("The actual bitrate may be lower if the device does not support it."),
            ]}
          />
        }
      >
        <CommitSlider
          value={Math.trunc(props.stream.audioBitrate / 1000)}
          minimum={32}
          maximum={320}
          step={32}
          disabled={liveDisabled(props)}
          format={(value) => formatBits(bitrateOf(value))}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.audioBitrate = bitrateOf(value);
            })
          }
        />
      </Section>
    </>
  );
}
