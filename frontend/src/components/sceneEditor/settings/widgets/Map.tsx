import { Section, SliderRow, ToggleRow } from "../../Form";
import { EffectsSection } from "./effects";
import { type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function MapForm(props: WidgetFormProps) {
  const map = () => props.widget.map;
  return (
    <>
      <Section footer={t("The map will rotate based of movement direction if disabled.")}>
        <ToggleRow
          label={t("North up")}
          on={map().northUp}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.map.northUp = on;
            })
          }
        />
      </Section>
      <Section header={t("Size")}>
        <SliderRow
          value={map().scale}
          minimum={500}
          maximum={10000}
          step={100}
          onChange={(scale) =>
            props.edit((draft) => {
              draft.map.scale = scale;
            })
          }
        />
      </Section>
      <Section
        header={t("Delay")}
        footer={t("To show the widget in sync with high latency cameras.")}
      >
        <SliderRow
          value={map().delay}
          minimum={0}
          maximum={10}
          step={0.5}
          format={(value) => value.toFixed(1)}
          onChange={(delay) =>
            props.edit((draft) => {
              draft.map.delay = delay;
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}
