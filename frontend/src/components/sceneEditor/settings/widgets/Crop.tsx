import { MenuRow, NumberRow, Section } from "../../Form";
import { type WidgetFormProps } from "./shared";
import { sameId } from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export function CropForm(props: WidgetFormProps) {
  const crop = () => props.widget.crop;
  const browsers = () => props.editor.state.widgets.filter((widget) => widget.type === "Browser");
  const exists = () => browsers().some((widget) => sameId(widget.id, crop().sourceWidgetId));
  return (
    <Section>
      <MenuRow
        label={t("Source widget")}
        items={[
          ...(exists() ? [] : [{ id: "", name: "" }]),
          ...browsers().map((widget) => ({ id: widget.id, name: widget.name })),
        ]}
        selected={exists() ? crop().sourceWidgetId : ""}
        onPick={(id) =>
          props.edit((draft) => {
            if (id !== "") {
              draft.crop.sourceWidgetId = id;
            }
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("X")}
        value={crop().x}
        minimum={0}
        integer
        onSubmit={(x) =>
          props.edit((draft) => {
            draft.crop.x = x;
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("Y")}
        value={crop().y}
        minimum={0}
        integer
        onSubmit={(y) =>
          props.edit((draft) => {
            draft.crop.y = y;
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("Width")}
        value={crop().width}
        minimum={1}
        integer
        onSubmit={(width) =>
          props.edit((draft) => {
            draft.crop.width = width;
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("Height")}
        value={crop().height}
        minimum={1}
        integer
        onSubmit={(height) =>
          props.edit((draft) => {
            draft.crop.height = height;
          })
        }
      />
    </Section>
  );
}
