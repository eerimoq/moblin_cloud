import { Section, ToggleRow } from "../sceneEditor/Form";
import { liveDisabled } from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import { t } from "../../i18n";

export function RistSettings(props: StreamFormProps) {
  return (
    <Section>
      <ToggleRow
        label={t("Adaptive bitrate")}
        on={props.stream.rist.adaptiveBitrateEnabled}
        disabled={liveDisabled(props)}
        onToggle={(on) =>
          props.edit((draft) => {
            draft.rist.adaptiveBitrateEnabled = on;
          })
        }
      />
      <ToggleRow
        label={t("Bonding")}
        on={props.stream.rist.bonding}
        disabled={liveDisabled(props)}
        onToggle={(on) =>
          props.edit((draft) => {
            draft.rist.bonding = on;
          })
        }
      />
    </Section>
  );
}
