import { createMemo, Show } from "solid-js";
import Icon from "../../../Icon";
import { Section } from "../../Form";
import { type WidgetFormProps } from "./shared";
import { sameId } from "../../../../data/moblinSettings";
import { SceneResolver } from "../../../../data/sceneResolver";
import { t } from "../../../../i18n";

export function SceneForm(props: WidgetFormProps) {
  const resolver = createMemo(
    () => new SceneResolver(props.editor.state.scenes, props.editor.state.widgets),
  );
  const scenes = () => resolver().selectableScenes(props.widget);
  return (
    <Section header={t("Scene")}>
      {scenes().map((scene) => (
        <button
          type="button"
          class="flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-left text-[17px]"
          onClick={() =>
            props.edit((draft) => {
              draft.scene.sceneId = scene.id;
            })
          }
        >
          <span class="min-w-0 flex-1 truncate">{scene.name}</span>
          <Show when={sameId(scene.id, props.widget.scene.sceneId)}>
            <Icon name="check" class="size-[18px] text-phone-tint" />
          </Show>
        </button>
      ))}
    </Section>
  );
}
