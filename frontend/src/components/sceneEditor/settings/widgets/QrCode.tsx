import { Section, TextRow } from "../../Form";
import { EffectsSection } from "./effects";
import { type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function QrCodeForm(props: WidgetFormProps) {
  return (
    <>
      <Section>
        <TextRow
          stack={props.stack}
          label={t("Message")}
          value={props.widget.qrCode.message}
          onSubmit={(message) =>
            props.edit((draft) => {
              draft.qrCode.message = message;
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}
