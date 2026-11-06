import { MenuRow, Section } from "../../Form";
import { EffectsSection } from "./effects";
import { secondsItems, type WidgetFormProps } from "./shared";
import { t } from "../../../../i18n";

export function SnapshotForm(props: WidgetFormProps) {
  return (
    <>
      <Section>
        <MenuRow
          label={t("Showtime")}
          items={secondsItems([3, 5, 10, 15, 30, 60, 120])}
          selected={String(props.widget.snapshot.showtime)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.snapshot.showtime = Number(id);
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}
