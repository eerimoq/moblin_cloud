import { createMemo, createSignal, Show } from "solid-js";
import Icon from "../../Icon";
import { Section, useReadOnly, ValueEditCompact } from "../Form";
import type { Stack } from "../Stack";
import type { Editor } from "../store";
import { WidgetSettings } from "./WidgetSettings";
import {
  alignmentName,
  hasAlignment,
  hasPosition,
  hasSize,
  isHorizontalCenter,
  isVerticalCenter,
  mirrorPositionHorizontally,
  mirrorPositionVertically,
  sameId,
  type SettingsAlignment,
  type SettingsScene,
  type SettingsSceneWidget,
} from "../../../data/moblinSettings";
import { sceneEditorAlignedLayout } from "../../../data/sceneEditorGeometry";
import {
  sceneEditorWidgetWidth,
  widgetOutlineSize,
  type Layout,
  type Size,
} from "../../../data/widgetOutlines";
import { t } from "../../../i18n";

const [copiedLayout, setCopiedLayout] = createSignal<Layout | null>(null);

const alignments: SettingsAlignment[][] = [
  ["TopLeft", "TopCenter", "TopRight"],
  ["LeftCenter", "Center", "RightCenter"],
  ["BottomLeft", "BottomCenter", "BottomRight"],
];

export function AlignmentPicker(props: {
  alignment: SettingsAlignment;
  onChange: (alignment: SettingsAlignment) => void;
}) {
  const readOnly = useReadOnly();
  return (
    <div class="flex flex-col gap-[5px]">
      {alignments.map((row) => (
        <div class="flex gap-[3px]">
          {row.map((alignment) => (
            <button
              type="button"
              aria-label={alignmentName(alignment)}
              class="text-phone-tint disabled:opacity-50"
              disabled={readOnly()}
              onClick={() => props.onChange(alignment)}
            >
              <Icon
                name={props.alignment === alignment ? "square-fill" : "square"}
                class="size-[26px]"
              />
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}

const layoutButtonClass =
  "min-w-0 flex-1 truncate rounded-[8px] bg-phone-separator px-3 py-1.5 text-[15px] text-phone-tint disabled:opacity-35";

export function CopyLayoutButton(props: { layout: Layout }) {
  return (
    <button
      type="button"
      class={layoutButtonClass}
      onClick={() => {
        const { x, y, size, alignment } = props.layout;
        setCopiedLayout({ x, y, size, alignment });
      }}
    >
      {t("Copy layout")}
    </button>
  );
}

export function PasteLayoutButton(props: { onPaste: (layout: Layout) => void }) {
  const readOnly = useReadOnly();
  return (
    <button
      type="button"
      class={layoutButtonClass}
      disabled={copiedLayout() === null || readOnly()}
      onClick={() => {
        const copied = copiedLayout();
        if (copied) {
          props.onPaste(copied);
        }
      }}
    >
      {t("Paste layout")}
    </button>
  );
}

export function CopyPasteLayoutAndAlignment(props: {
  layout: Layout;
  onPaste: (layout: Layout) => void;
  onAlignment: (alignment: SettingsAlignment) => void;
}) {
  return (
    <div class="flex items-center gap-3 px-4 py-2">
      <div class="flex flex-1 flex-col gap-2">
        <CopyLayoutButton layout={props.layout} />
        <PasteLayoutButton onPaste={props.onPaste} />
      </div>
      <div class="h-[100px] w-px bg-phone-separator" />
      <AlignmentPicker alignment={props.layout.alignment} onChange={props.onAlignment} />
    </div>
  );
}

export function SceneWidgetSettings(props: {
  editor: Editor;
  stack: Stack;
  scene: SettingsScene;
  sceneWidgetId: string;
  widgetId: string;
  canvas: Size;
}) {
  const editor = props.editor;
  const scene = createMemo(() =>
    editor.state.scenes.find((candidate) => sameId(candidate.id, props.scene.id)),
  );
  const sceneWidget = createMemo(() =>
    scene()?.widgets.find((candidate) => candidate.id === props.sceneWidgetId),
  );
  const widget = createMemo(() =>
    editor.state.widgets.find((candidate) => sameId(candidate.id, props.widgetId)),
  );
  const edit = (change: (layout: SettingsSceneWidget) => void) =>
    editor.editScene(props.scene.id, (draft) => {
      const target = draft.widgets.find((candidate) => candidate.id === props.sceneWidgetId);
      if (target) {
        change(target);
      }
    });

  return (
    <Show when={sceneWidget() && widget() && { layout: sceneWidget()!, widget: widget()! }}>
      {(current) => {
        const paste = (layout: Layout) => edit((draft) => Object.assign(draft, layout));
        const width = (size: number) =>
          sceneEditorWidgetWidth(current().widget, current().layout, size, props.canvas);
        return (
          <>
            <Show
              when={
                hasAlignment(current().widget.type) ||
                hasPosition(current().widget.type) ||
                hasSize(current().widget.type)
              }
            >
              <Section
                header={t("Layout")}
                footer={t(
                  "Use copy/paste layout to position a widget in the same place in multiple scenes. Alternatively, use a Scene widget to easily show the same widgets in multiple scenes.",
                )}
              >
                <Show
                  when={hasAlignment(current().widget.type)}
                  fallback={
                    <div class="flex items-center gap-2 px-4 py-2">
                      <CopyLayoutButton layout={current().layout} />
                      <PasteLayoutButton onPaste={paste} />
                    </div>
                  }
                >
                  <CopyPasteLayoutAndAlignment
                    layout={current().layout}
                    onPaste={paste}
                    onAlignment={(alignment) =>
                      edit((draft) =>
                        Object.assign(
                          draft,
                          sceneEditorAlignedLayout(
                            draft,
                            alignment,
                            widgetOutlineSize(current().widget, draft, props.canvas) ?? {
                              width: 0,
                              height: 0,
                            },
                            props.canvas,
                          ),
                        ),
                      )
                    }
                  />
                </Show>
                <Show when={hasPosition(current().widget.type)}>
                  <Show when={!isHorizontalCenter(current().layout.alignment)}>
                    <ValueEditCompact
                      value={current().layout.x}
                      minimum={-100}
                      maximum={100}
                      total={props.canvas.width}
                      mirror={mirrorPositionHorizontally(current().layout.alignment)}
                      decrementIcon="arrow-left-circle"
                      incrementIcon="arrow-right-circle"
                      onChange={(value) =>
                        edit((draft) => {
                          draft.x = value;
                        })
                      }
                    />
                  </Show>
                  <Show when={!isVerticalCenter(current().layout.alignment)}>
                    <ValueEditCompact
                      value={current().layout.y}
                      minimum={-100}
                      maximum={100}
                      total={props.canvas.height}
                      mirror={mirrorPositionVertically(current().layout.alignment)}
                      decrementIcon="arrow-up-circle"
                      incrementIcon="arrow-down-circle"
                      onChange={(value) =>
                        edit((draft) => {
                          draft.y = value;
                        })
                      }
                    />
                  </Show>
                </Show>
                <Show when={hasSize(current().widget.type)}>
                  <ValueEditCompact
                    value={current().layout.size}
                    minimum={1}
                    maximum={100}
                    zero={width(0)}
                    total={width(100)}
                    decrementIcon="resize-down"
                    incrementIcon="resize-up"
                    onChange={(value) =>
                      edit((draft) => {
                        draft.size = value;
                      })
                    }
                  />
                </Show>
              </Section>
            </Show>
            <WidgetSettings editor={editor} stack={props.stack} widgetId={props.widgetId} />
          </>
        );
      }}
    </Show>
  );
}
