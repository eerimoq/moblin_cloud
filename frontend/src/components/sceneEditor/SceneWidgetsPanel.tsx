import { createMemo, For, Match, Show, Switch } from "solid-js";
import Icon from "../Icon";
import { Empty, Loading, rowClass, Toggle } from "../MoblinParts";
import { ButtonRow, ErrorText, ReadOnly, Section } from "./Form";
import { widgetIcon } from "./icons";
import { SceneWidgetSettings } from "./settings/SceneWidgetSettings";
import { BingoCardQuickControls } from "./settings/widgets/BingoCard";
import { WheelOfLuckQuickControls } from "./settings/widgets/WheelOfLuck";
import { createStack, NavigationView } from "./Stack";
import type { Editor } from "./store";
import { sameId, type SettingsWidget } from "../../data/moblinSettings";
import {
  firstWidgets,
  resolvedEnabled,
  SceneResolver,
  type ResolvedSceneWidget,
} from "../../data/sceneResolver";
import type { Size } from "../../data/widgetOutlines";
import { t } from "../../i18n";

export default function SceneWidgetsPanel(props: {
  editor: Editor;
  sceneId: string | undefined;
  onOpenSceneEditor: () => void;
}) {
  const editor = props.editor;
  const stack = createStack();
  const canvas = (): Size => editor.state.options?.canvasSize ?? { width: 1920, height: 1080 };
  const scene = createMemo(() =>
    editor.state.scenes.find((candidate) => sameId(candidate.id, props.sceneId)),
  );
  const widgets = createMemo(() => {
    const current = scene();
    if (!current) {
      return [];
    }
    const resolver = new SceneResolver(editor.state.scenes, editor.state.widgets);
    return firstWidgets(resolver.resolve(current, true));
  });
  const edit = (id: string) => (change: (widget: SettingsWidget) => void) =>
    editor.editWidget(id, change);
  const open = (resolved: ResolvedSceneWidget) =>
    stack.push({
      title: () =>
        editor.state.widgets.find((candidate) => sameId(candidate.id, resolved.widget.id))?.name ??
        "",
      render: () => (
        <SceneWidgetSettings
          editor={editor}
          stack={stack}
          scene={resolved.scene}
          sceneWidgetId={resolved.sceneWidget.id}
          widgetId={resolved.widget.id}
          canvas={canvas()}
        />
      ),
    });
  const root = {
    title: t("Scene widgets"),
    render: () => (
      <Show
        when={editor.state.loaded}
        fallback={
          <Show when={editor.state.error} fallback={<Loading />}>
            {(error) => <Empty text={error()} />}
          </Show>
        }
      >
        <Show
          when={widgets().length > 0}
          fallback={<Empty text={t("The scene has no widgets.")} />}
        >
          <Section>
            <For each={widgets()}>
              {(resolved) => (
                <>
                  <div class={rowClass}>
                    <button
                      type="button"
                      class="flex min-w-0 flex-1 items-center gap-3 self-stretch text-left"
                      onClick={() => open(resolved)}
                    >
                      <Icon
                        name={widgetIcon(resolved.widget.type)}
                        class="size-[18px] shrink-0 text-phone-tint"
                      />
                      <span class="min-w-0 flex-1 truncate">{resolved.widget.name}</span>
                    </button>
                    <Toggle
                      label={resolved.widget.name}
                      on={resolved.widget.enabled}
                      disabled={editor.readOnly()}
                      onToggle={(on) =>
                        edit(resolved.widget.id)((draft) => {
                          draft.enabled = on;
                        })
                      }
                    />
                    <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
                  </div>
                  <Switch>
                    <Match
                      when={resolved.widget.type === "Wheel of luck" && resolvedEnabled(resolved)}
                    >
                      <WheelOfLuckQuickControls edit={edit(resolved.widget.id)} />
                    </Match>
                    <Match when={resolved.widget.type === "Bingo card"}>
                      <BingoCardQuickControls
                        widget={resolved.widget}
                        edit={edit(resolved.widget.id)}
                      />
                    </Match>
                  </Switch>
                </>
              )}
            </For>
          </Section>
        </Show>
        <ReadOnly.Provider value={() => false}>
          <Section>
            <ButtonRow label={t("Scene editor")} onClick={() => props.onOpenSceneEditor()} />
          </Section>
        </ReadOnly.Provider>
        <ErrorText text={editor.state.error} />
      </Show>
    ),
  };
  return <NavigationView stack={stack} root={root} />;
}
