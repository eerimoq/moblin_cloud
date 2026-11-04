import { Show } from "solid-js";
import { Section, SliderRow, ToggleRow } from "../../Form";
import { ModelSection, SensitivitySection, VideoSourceRow, type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function VTuberForm(props: WidgetFormProps) {
  const vTuber = () => props.widget.vTuber;
  return (
    <>
      <Section>
        <VideoSourceRow
          editor={props.editor}
          stack={props.stack}
          fields={vTuber()}
          onChange={(change) => props.edit((draft) => change(draft.vTuber))}
        />
      </Section>
      <ModelSection
        name={vTuber().modelName}
        footers={[
          t("Most VRM 0.x and 1.0 files and zipped Live2D Cubism models are supported."),
          t("Pick the model on the phone."),
        ]}
      />
      <Show when={vTuber().type === "VRM"}>
        <Section header={t("Camera")}>
          <SliderRow
            label={t("Vertical position")}
            value={vTuber().cameraPositionY}
            minimum={1}
            maximum={2}
            step={0.01}
            onChange={(value) =>
              props.edit((draft) => {
                draft.vTuber.cameraPositionY = value;
              })
            }
          />
          <SliderRow
            label={t("Field of view")}
            value={vTuber().cameraFieldOfView}
            minimum={10}
            maximum={30}
            step={1}
            onChange={(value) =>
              props.edit((draft) => {
                draft.vTuber.cameraFieldOfView = value;
              })
            }
          />
        </Section>
      </Show>
      <Section>
        <ToggleRow
          label={t("Mirror")}
          on={vTuber().mirror}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.vTuber.mirror = on;
            })
          }
        />
      </Section>
      <SensitivitySection
        sensitivity={vTuber().sensitivity}
        onChange={(sensitivity) =>
          props.edit((draft) => {
            draft.vTuber.sensitivity = sensitivity;
          })
        }
      />
      <Show when={vTuber().type === "VRM"}>
        <Section header={t("Angles")}>
          <SliderRow
            label={t("Arms")}
            value={vTuber().armsAngle}
            minimum={20}
            maximum={90}
            step={1}
            onChange={(value) =>
              props.edit((draft) => {
                draft.vTuber.armsAngle = value;
              })
            }
          />
        </Section>
      </Show>
    </>
  );
}
