import Icon from "../../../Icon";
import { Section } from "../../Form";
import { EffectsSection } from "./effects";
import { type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function ImageForm(props: WidgetFormProps) {
  return (
    <>
      <Section footer={t("Pick the image on the phone.")}>
        <div class="flex w-full justify-center px-4 py-3">
          <Icon name="image" class="size-[20px] text-phone-border" />
        </div>
      </Section>
      <EffectsSection {...props} />
    </>
  );
}
