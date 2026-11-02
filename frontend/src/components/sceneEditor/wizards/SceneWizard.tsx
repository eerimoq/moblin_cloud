import { createSignal } from "solid-js";
import { MenuRow, Section } from "../Form";
import { widgetIcon } from "../icons";
import { CheckList, InlineField, NextRow, Sheet } from "./Sheet";
import type { Editor } from "../store";
import {
  applyVideoSource,
  videoSourceItemId,
  videoSourceMenuItems,
  videoSourceOf,
} from "../videoSources";
import {
  defaultScene,
  defaultSceneWidget,
  makeUniqueName,
  newUuid,
  sameId,
  type SettingsScene,
} from "../../../data/moblinSettings";
import { SceneResolver } from "../../../data/sceneResolver";
import { t } from "../../../i18n";

export function SceneWizard(props: { editor: Editor; onClose: () => void }) {
  const editor = props.editor;
  const [scene, setScene] = createSignal(defaultScene(newUuid()));
  const [name, setName] = createSignal("");
  const [widgetIds, setWidgetIds] = createSignal<string[]>([]);
  const nameError = () =>
    editor.state.scenes.some((candidate) => candidate.name === name())
      ? t("The name '{name}' is already in use.", { name: name() })
      : null;
  const create = () => {
    const created: SettingsScene = { ...scene(), widgets: [] };
    created.name =
      name() === ""
        ? makeUniqueName(
            t("My scene"),
            editor.state.scenes.map((candidate) => candidate.name),
          )
        : name();
    editor.addScene(created);
    for (const widget of editor.state.widgets) {
      if (!widgetIds().some((id) => sameId(id, widget.id))) {
        continue;
      }
      const current = editor.state.scenes.find((candidate) => sameId(candidate.id, created.id));
      const resolver = new SceneResolver(editor.state.scenes, editor.state.widgets);
      if (
        current &&
        resolver.addableWidgets(current).some((candidate) => sameId(candidate.id, widget.id))
      ) {
        editor.editScene(created.id, (draft) => {
          draft.widgets.push(defaultSceneWidget(widget.id, widget.type, widget.scoreboard.type));
        });
      }
    }
    editor.set("selection", null);
    editor.selectScene(created.id);
    props.onClose();
  };
  return (
    <Sheet
      onClose={props.onClose}
      root={(stack) => ({
        title: t("Create scene wizard"),
        render: () => (
          <>
            <InlineField
              header={t("Name")}
              value={name()}
              placeholder={t("My scene")}
              error={nameError()}
              onInput={setName}
            />
            <Section>
              <MenuRow
                label={t("Video source")}
                items={videoSourceMenuItems(editor.state.videoSources, scene())}
                selected={videoSourceItemId(scene())}
                onPick={(id) => {
                  const source = videoSourceOf(editor.state.videoSources, id);
                  if (source) {
                    const next = { ...scene() };
                    applyVideoSource(next, source);
                    setScene(next);
                  }
                }}
              />
            </Section>
            <NextRow
              disabled={nameError() !== null}
              onClick={() =>
                stack.push({
                  title: t("Widgets to add to the scene"),
                  render: () => (
                    <>
                      <CheckList
                        items={editor.state.widgets}
                        label={(widget) => widget.name}
                        icon={(widget) => widgetIcon(widget.type)}
                        checked={(widget) => widgetIds().some((id) => sameId(id, widget.id))}
                        onToggle={(widget) =>
                          setWidgetIds((ids) =>
                            ids.some((id) => sameId(id, widget.id))
                              ? ids.filter((id) => !sameId(id, widget.id))
                              : [...ids, widget.id],
                          )
                        }
                      />
                      <div class="overflow-hidden rounded-[10px] bg-phone-row">
                        <button
                          type="button"
                          class="flex min-h-[44px] w-full items-center justify-center px-4 py-2 text-[17px] text-phone-tint"
                          onClick={create}
                        >
                          {t("Create")}
                        </button>
                      </div>
                    </>
                  ),
                })
              }
            />
          </>
        ),
      })}
    />
  );
}
