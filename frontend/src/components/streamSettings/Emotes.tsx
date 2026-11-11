import { Section, ToggleRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import { t } from "../../i18n";

export function EmotesSettings(props: StreamFormProps) {
  return (
    <Section>
      <ToggleRow
        label={t("BTTV")}
        on={props.stream.chat.bttvEmotes}
        onToggle={(on) =>
          props.edit((draft) => {
            draft.chat.bttvEmotes = on;
          })
        }
      />
      <ToggleRow
        label={t("FFZ")}
        on={props.stream.chat.ffzEmotes}
        onToggle={(on) =>
          props.edit((draft) => {
            draft.chat.ffzEmotes = on;
          })
        }
      />
      <ToggleRow
        label={t("7TV")}
        on={props.stream.chat.seventvEmotes}
        onToggle={(on) =>
          props.edit((draft) => {
            draft.chat.seventvEmotes = on;
          })
        }
      />
    </Section>
  );
}
