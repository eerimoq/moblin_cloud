import { Section, ToggleRow } from "../../Form";
import { ModelSection, SensitivitySection, VideoSourceRow, type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function PngTuberForm(props: WidgetFormProps) {
  const pngTuber = () => props.widget.pngTuber;
  return (
    <>
      <Section>
        <VideoSourceRow
          editor={props.editor}
          stack={props.stack}
          fields={pngTuber()}
          onChange={(change) => props.edit((draft) => change(draft.pngTuber))}
        />
      </Section>
      <ModelSection
        name={pngTuber().modelName}
        footers={[t("A .save-file from PNGTuberPlus."), t("Pick the model on the phone.")]}
      />
      <SensitivitySection
        sensitivity={pngTuber().sensitivity}
        onChange={(sensitivity) =>
          props.edit((draft) => {
            draft.pngTuber.sensitivity = sensitivity;
          })
        }
      />
      <Section>
        <ToggleRow
          label={t("Mirror")}
          on={pngTuber().mirror}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.pngTuber.mirror = on;
            })
          }
        />
      </Section>
    </>
  );
}
