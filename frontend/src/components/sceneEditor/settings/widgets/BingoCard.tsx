import { For, Show } from "solid-js";
import Icon from "../../../Icon";
import { ButtonRow, ColorRow, Section } from "../../Form";
import { Lines, type WidgetFormProps } from "./shared";
import { newUuid, type SettingsWidget } from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

const bingoCardSide = (count: number) => (count <= 4 ? 2 : count <= 9 ? 3 : count <= 16 ? 4 : 5);

export function BingoCardQuickControls(props: {
  widget: SettingsWidget;
  edit: (change: (widget: SettingsWidget) => void) => void;
}) {
  const squares = () => props.widget.bingoCard.squares;
  const side = () => bingoCardSide(squares().length);
  const indexes = (row: number) =>
    Array.from({ length: side() }, (_, column) => row * side() + column);
  return (
    <div class="flex flex-col items-end gap-[5px] px-4 py-2">
      <For each={Array.from({ length: side() }, (_, row) => row)}>
        {(row) => (
          <div class="flex gap-[3px]">
            <For each={indexes(row)}>
              {(index) => (
                <Show
                  when={squares()[index]}
                  fallback={<Icon name="square" class="size-[28px] text-phone-border" />}
                >
                  {(square) => (
                    <button
                      type="button"
                      aria-label={square().text}
                      aria-pressed={square().checked}
                      class="text-phone-tint"
                      onClick={() =>
                        props.edit((draft) => {
                          const target = draft.bingoCard.squares[index];
                          if (target) {
                            target.checked = !target.checked;
                          }
                        })
                      }
                    >
                      <Icon
                        name={square().checked ? "square-split-diagonal" : "square"}
                        class="size-[28px]"
                      />
                    </button>
                  )}
                </Show>
              )}
            </For>
          </div>
        )}
      </For>
    </div>
  );
}

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
