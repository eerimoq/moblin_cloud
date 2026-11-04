import { MenuRow, Section, SliderRow, ToggleRow } from "../../Form";
import { EffectsSection } from "./effects";
import { VideoSourceRow, type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function VideoSourceForm(props: WidgetFormProps) {
  const source = () => props.widget.videoSource;
  return (
    <>
      <Section>
        <VideoSourceRow
          editor={props.editor}
          stack={props.stack}
          fields={source()}
          onChange={(change) => props.edit((draft) => change(draft.videoSource))}
        />
      </Section>
      <Section>
        <MenuRow
          label={t("Rotation")}
          items={[0, 90, 180, 270].map((degrees) => ({ id: String(degrees), name: `${degrees}°` }))}
          selected={String(source().rotation)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.videoSource.rotation = Number(id);
            })
          }
        />
        <ToggleRow
          label={t("Mirror")}
          on={source().mirror}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.videoSource.mirror = on;
            })
          }
        />
      </Section>
      <Section header={t("Face tracking")}>
        <ToggleRow
          label={t("Enabled")}
          on={source().trackFaceEnabled}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.videoSource.trackFaceEnabled = on;
            })
          }
        />
        <SliderRow
          label={t("Zoom")}
          value={source().trackFaceZoom}
          minimum={0}
          maximum={1}
          step={0.01}
          onChange={(zoom) =>
            props.edit((draft) => {
              draft.videoSource.trackFaceZoom = zoom;
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}
