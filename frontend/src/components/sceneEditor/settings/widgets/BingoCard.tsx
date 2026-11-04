import Icon from "../../../Icon";
import { ButtonRow, ColorRow, Section } from "../../Form";
import { Lines, type WidgetFormProps } from "./shared";
import { newUuid } from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export function BingoCardForm(props: WidgetFormProps) {
  const bingo = () => props.widget.bingoCard;
  return (
    <>
      <Lines
        header={t("Squares")}
        value={bingo()
          .squares.map((square) => square.text)
          .join("\n")}
        placeholder={t("My text")}
        onChange={(text) =>
          props.edit((draft) => {
            const lines = text.split("\n");
            draft.bingoCard.squares = lines.map((line, index) => {
              const square = draft.bingoCard.squares[index] ?? {
                id: newUuid(),
                text: "",
                checked: false,
              };
              square.text = line.trim();
              return square;
            });
          })
        }
      />
      <Section header={t("Marks")}>
        {bingo().squares.map((square) => (
          <div class="flex min-h-[44px] items-center gap-3 px-4 py-2 text-[17px]">
            <span class="min-w-0 flex-1 truncate">{square.text}</span>
            <button
              type="button"
              class="text-phone-tint"
              onClick={() =>
                props.edit((draft) => {
                  const target = draft.bingoCard.squares.find(
                    (candidate) => candidate.id === square.id,
                  );
                  if (target) {
                    target.checked = !target.checked;
                  }
                })
              }
            >
              <Icon
                name={square.checked ? "square-split-diagonal" : "square"}
                class="size-[26px]"
              />
            </button>
          </div>
        ))}
        <ButtonRow
          label={t("Reset")}
          onClick={() =>
            props.edit((draft) => {
              draft.bingoCard.squares = draft.bingoCard.squares.map((square) => ({
                id: newUuid(),
                text: square.text,
                checked: false,
              }));
            })
          }
        />
      </Section>
      <Section header={t("Colors")}>
        <ColorRow
          label={t("Background")}
          color={bingo().backgroundColor}
          opacity
          onChange={(color) =>
            props.edit((draft) => {
              draft.bingoCard.backgroundColor = color;
            })
          }
        />
        <ColorRow
          label={t("Foreground")}
          color={bingo().foregroundColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.bingoCard.foregroundColor = color;
            })
          }
        />
      </Section>
    </>
  );
}
