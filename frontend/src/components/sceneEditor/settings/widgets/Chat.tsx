import { Show } from "solid-js";
import { ColorRow, MenuRow, Section, SliderRow, ToggleRow } from "../../Form";
import { FontRows, pickerItems, type WidgetFormProps } from "./shared";
import { type ChatDisplayStyle } from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export function ChatForm(props: WidgetFormProps) {
  const chat = () => props.widget.chat;
  const showAll = () => props.editor.state.options?.showAllSettings ?? true;
  const styles: { id: ChatDisplayStyle; name: string }[] = [
    { id: "internationalName", name: t("International name") },
    { id: "internationalNameAndUsername", name: t("International name (Username)") },
    { id: "username", name: t("Username") },
  ];
  return (
    <>
      <Section header={t("Font")}>
        <SliderRow
          label={t("Size")}
          value={chat().fontSize}
          minimum={10}
          maximum={50}
          step={1}
          format={(value) => String(Math.round(value))}
          onChange={(fontSize) =>
            props.edit((draft) => {
              draft.chat.fontSize = fontSize;
            })
          }
        />
        <Show when={showAll()}>
          <FontRows
            editor={props.editor}
            stack={props.stack}
            family={chat().fontFamily}
            style={chat().fontStyle}
            onChange={(family, style) =>
              props.edit((draft) => {
                draft.chat.fontFamily = family;
                draft.chat.fontStyle = style;
              })
            }
          />
          <ToggleRow
            label={t("Bold name")}
            on={chat().boldUsername}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.boldUsername = on;
              })
            }
          />
          <ToggleRow
            label={t("Bold message")}
            on={chat().boldMessage}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.boldMessage = on;
              })
            }
          />
        </Show>
      </Section>
      <Section header={t("General")}>
        <MenuRow
          label={t("Messages")}
          items={pickerItems([1, 2, 3, 4, 5])}
          selected={String(chat().maximumNumberOfMessages)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.chat.maximumNumberOfMessages = Number(id);
            })
          }
        />
        <SliderRow
          label={t("Height")}
          value={chat().height}
          minimum={0.1}
          maximum={1}
          step={0.01}
          format={(value) => `${Math.round(100 * value)}%`}
          onChange={(height) =>
            props.edit((draft) => {
              draft.chat.height = height;
            })
          }
        />
        <Show when={showAll()}>
          <MenuRow
            label={t("Display style")}
            items={styles}
            selected={chat().displayStyle}
            onPick={(id) =>
              props.edit((draft) => {
                draft.chat.displayStyle = id as ChatDisplayStyle;
              })
            }
          />
          <ToggleRow
            label={t("Badges")}
            on={chat().badges}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.badges = on;
              })
            }
          />
          <ToggleRow
            label={t("Shared chat icons")}
            on={chat().sharedChatIcons}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.sharedChatIcons = on;
              })
            }
          />
        </Show>
      </Section>
      <Section header={t("Colors")}>
        <Show when={showAll()}>
          <ColorRow
            label={t("Name")}
            color={chat().usernameColor}
            onChange={(color) =>
              props.edit((draft) => {
                draft.chat.usernameColor = color;
              })
            }
          />
          <ColorRow
            label={t("Message")}
            color={chat().messageColor}
            onChange={(color) =>
              props.edit((draft) => {
                draft.chat.messageColor = color;
              })
            }
          />
        </Show>
        <div class="flex items-center">
          <div class="min-w-0 flex-1">
            <ColorRow
              label={t("Background")}
              color={chat().backgroundColor}
              onChange={(color) =>
                props.edit((draft) => {
                  draft.chat.backgroundColor = color;
                })
              }
            />
          </div>
          <div class="pr-1">
            <ToggleRow
              label=""
              on={chat().backgroundColorEnabled}
              onToggle={(on) =>
                props.edit((draft) => {
                  draft.chat.backgroundColorEnabled = on;
                })
              }
            />
          </div>
        </div>
        <div class="flex items-center">
          <div class="min-w-0 flex-1">
            <ColorRow
              label={t("Border")}
              color={chat().shadowColor}
              onChange={(color) =>
                props.edit((draft) => {
                  draft.chat.shadowColor = color;
                })
              }
            />
          </div>
          <div class="pr-1">
            <ToggleRow
              label=""
              on={chat().shadowColorEnabled}
              onToggle={(on) =>
                props.edit((draft) => {
                  draft.chat.shadowColorEnabled = on;
                })
              }
            />
          </div>
        </div>
      </Section>
    </>
  );
}
