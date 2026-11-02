import { createSignal } from "solid-js";
import { ButtonRow, MenuRow, Section } from "../Form";
import { effectTypeName } from "../settings/widgets/effects";
import { Sheet } from "./Sheet";
import type { Editor } from "../store";
import {
  defaultEffect,
  newUuid,
  videoEffectTypes,
  type VideoEffectType,
} from "../../../data/moblinSettings";
import { t } from "../../../i18n";

export function EffectWizard(props: { editor: Editor; widgetId: string; onClose: () => void }) {
  const [type, setType] = createSignal<VideoEffectType>("shape");
  const create = () => {
    props.editor.editWidget(props.widgetId, (draft) => {
      const effect = defaultEffect(newUuid());
      effect.type = type();
      draft.effects.push(effect);
    });
    props.onClose();
  };
  return (
    <Sheet
      onClose={props.onClose}
      root={() => ({
        title: t("Create effect wizard"),
        render: () => (
          <>
            <Section>
              <MenuRow
                label={t("Type")}
                items={videoEffectTypes.map((candidate) => ({
                  id: candidate,
                  name: effectTypeName(candidate),
                }))}
                selected={type()}
                onPick={(id) => setType(id as VideoEffectType)}
              />
            </Section>
            <Section>
              <ButtonRow label={t("Create")} onClick={create} />
            </Section>
          </>
        ),
      })}
    />
  );
}
