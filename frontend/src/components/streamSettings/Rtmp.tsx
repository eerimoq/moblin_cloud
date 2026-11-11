import { Section, ToggleRow } from "../sceneEditor/Form";
import { liveDisabled } from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import { t } from "../../i18n";

export function RtmpSettings(props: StreamFormProps) {
  return (
    <Section>
      <ToggleRow
        label={t("Adaptive bitrate")}
        on={props.stream.rtmp.adaptiveBitrateEnabled}
        disabled={liveDisabled(props)}
        onToggle={(on) =>
          props.edit((draft) => {
            draft.rtmp.adaptiveBitrateEnabled = on;
          })
        }
      />
    </Section>
  );
}
