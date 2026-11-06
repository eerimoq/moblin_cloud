import { MenuRow, Section } from "../../Form";
import { pickerItems, secondsItems, type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function ChatEmoteComboForm(props: WidgetFormProps) {
  return (
    <Section>
      <MenuRow
        label={t("Minimum combo")}
        items={pickerItems([2, 3, 4, 5, 6, 7, 8, 9, 10])}
        selected={String(props.widget.chatEmoteCombo.minimumCombo)}
        onPick={(id) =>
          props.edit((draft) => {
            draft.chatEmoteCombo.minimumCombo = Number(id);
          })
        }
      />
      <MenuRow
        label={t("Timeout")}
        items={secondsItems([3, 4, 5, 6, 7, 8, 9, 10])}
        selected={String(props.widget.chatEmoteCombo.resetAfter)}
        onPick={(id) =>
          props.edit((draft) => {
            draft.chatEmoteCombo.resetAfter = Number(id);
          })
        }
      />
    </Section>
  );
}
