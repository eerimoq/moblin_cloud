import { createResource, createSignal, Index, Match, Show, Switch } from "solid-js";
import Icon from "./Icon";
import {
  Choices,
  Empty,
  Group,
  Loading,
  NavRow,
  PanelView,
  PhoneSwitch,
  type Remote,
  rowClass,
} from "./MoblinParts";
import SceneWidgetsPanel from "./sceneEditor/SceneWidgetsPanel";
import StreamsSettings from "./streamSettings/StreamsSettings";
import type { Editor } from "./sceneEditor/store";
import type { Control, Named } from "../data/account";
import { formatBits } from "../data/moblin";
import { t } from "../i18n";

export type Panel =
  | "mic"
  | "bitrate"
  | "priorities"
  | "autoSceneSwitcher"
  | "macros"
  | "sceneWidgets"
  | "settings";

function Mics(props: { remote: Remote; footer?: string }) {
  return (
    <Show when={props.remote.settings} fallback={<Loading />}>
      {(settings) => (
        <Choices
          items={settings().mics}
          selected={props.remote.state.mic}
          icon="mic"
          disabled={props.remote.disabled("setMic")}
          footer={props.footer}
          onPick={(id) => void props.remote.control({ setMic: { id } })}
        />
      )}
    </Show>
  );
}

function Bitrates(props: { remote: Remote }) {
  return (
    <Show when={props.remote.settings} fallback={<Loading />}>
      {(settings) => (
        <Choices
          items={settings().bitratePresets.map((preset) => ({
            id: preset.id,
            name: formatBits(preset.bitrate),
          }))}
          selected={props.remote.state.bitrate}
          disabled={props.remote.disabled("setBitratePreset")}
          onPick={(id) => void props.remote.control({ setBitratePreset: { id } })}
        />
      )}
    </Show>
  );
}

function Priorities(props: { remote: Remote }) {
  return (
    <Show when={props.remote.settings} fallback={<Loading />}>
      {(settings) => (
        <>
          <Group footer={t("Higher priority connections are used first.")}>
            <PhoneSwitch
              label={t("Enabled")}
              on={settings().srt.connectionPrioritiesEnabled}
              disabled={props.remote.disabled("setSrtConnectionPrioritiesEnabled")}
              onToggle={(enabled) =>
                void props.remote.control({ setSrtConnectionPrioritiesEnabled: { enabled } })
              }
            />
          </Group>
          <Show
            when={settings().srt.connectionPriorities.length > 0}
            fallback={<Empty text={t("The stream has no connection priorities.")} />}
          >
            <Group>
              <Index each={settings().srt.connectionPriorities}>
                {(connection) => {
                  const key = () => `setSrtConnectionPriority:${connection().id}`;
                  return (
                    <div class="flex flex-col">
                      <PhoneSwitch
                        label={connection().name}
                        on={connection().enabled}
                        disabled={props.remote.disabled(key())}
                        onToggle={(enabled) =>
                          void props.remote.control({
                            setSrtConnectionPriority: {
                              id: connection().id,
                              priority: connection().priority,
                              enabled,
                            },
                          })
                        }
                      />
                      <div class="flex items-center gap-3 px-4 pb-3">
                        <input
                          type="range"
                          min="1"
                          max="10"
                          step="1"
                          aria-label={t("Priority")}
                          class="flex-1 accent-phone-tint"
                          value={connection().priority}
                          disabled={props.remote.disabled(key()) || !connection().enabled}
                          onChange={async (event) => {
                            const slider = event.currentTarget;
                            await props.remote.control({
                              setSrtConnectionPriority: {
                                id: connection().id,
                                priority: Number(slider.value),
                                enabled: connection().enabled,
                              },
                            });
                            slider.value = String(connection().priority);
                          }}
                        />
                        <span class="w-5 text-right text-[15px]">{connection().priority}</span>
                      </div>
                    </div>
                  );
                }}
              </Index>
            </Group>
          </Show>
        </>
      )}
    </Show>
  );
}

function AutoSceneSwitchers(props: { remote: Remote }) {
  const switchers = () => props.remote.settings?.autoSceneSwitchers ?? [];
  return (
    <Choices
      items={[{ id: "", name: t("Off") }, ...switchers()]}
      selected={props.remote.state.autoSceneSwitcher?.id ?? ""}
      icon="shuffle"
      disabled={props.remote.disabled("setAutoSceneSwitcher")}
      onPick={(id) => void props.remote.control({ setAutoSceneSwitcher: id === "" ? {} : { id } })}
    />
  );
}

function Macros(props: { remote: Remote }) {
  const macros = () => props.remote.state.macros ?? [];
  return (
    <Show when={macros().length > 0} fallback={<Empty text={t("No macros.")} />}>
      <Group>
        <Index each={macros()}>
          {(macro) => (
            <div class={rowClass}>
              <Icon name="list" class="size-[18px] shrink-0 text-phone-tint" />
              <span class="min-w-0 flex-1 truncate">{macro().name}</span>
              <Show when={macro().running}>
                <span class="size-[8px] rounded-full bg-phone-switch" />
              </Show>
              <button
                type="button"
                class="text-phone-tint disabled:opacity-50"
                disabled={props.remote.disabled(
                  `${macro().running ? "stopMacro" : "startMacro"}:${macro().id}`,
                )}
                onClick={() =>
                  void props.remote.control(
                    macro().running
                      ? { stopMacro: { id: macro().id } }
                      : { startMacro: { id: macro().id } },
                  )
                }
              >
                {macro().running ? t("Stop") : t("Run")}
              </button>
            </div>
          )}
        </Index>
      </Group>
    </Show>
  );
}

type CloudWidgetKind = "alerts" | "wheelOfLuck";

const cloudWidgetControl = (kind: CloudWidgetKind, id: string): Control => {
  const body = id === "" ? {} : { id };
  return kind === "alerts"
    ? { setMoblinCloudAlertsWidget: body }
    : { setMoblinCloudWheelOfLuckWidget: body };
};

type CloudWidgets = Record<CloudWidgetKind, Named[]> | undefined;

const cloudWidgetsOf = (widgets: { id: string; name: string; type: string }[]): CloudWidgets => {
  const ofType = (type: string) =>
    widgets.filter((widget) => widget.type === type).map(({ id, name }) => ({ id, name }));
  return { alerts: ofType("Alerts"), wheelOfLuck: ofType("Wheel of luck") };
};

function CloudWidgetChoices(props: {
  remote: Remote;
  kind: CloudWidgetKind;
  widgets: CloudWidgets;
  failed: boolean;
}) {
  const cloud = () => props.remote.settings?.moblinCloud;
  const picked = () =>
    (props.kind === "alerts" ? cloud()?.alertsWidgetId : cloud()?.wheelOfLuckWidgetId) ?? "";
  const key = () =>
    props.kind === "alerts" ? "setMoblinCloudAlertsWidget" : "setMoblinCloudWheelOfLuckWidget";
  return (
    <Show
      when={props.widgets}
      fallback={
        <Show when={props.failed} fallback={<Loading />}>
          <Empty text={t("Couldn't load the widgets.")} />
        </Show>
      }
    >
      {(widgets) => (
        <Choices
          items={[{ id: "", name: t("None") }, ...widgets()[props.kind]]}
          selected={picked()}
          disabled={props.remote.disabled(key())}
          footer={
            props.kind === "alerts"
              ? t(
                  "Alerts widget that shows messages on stream. Messages always show in chat. Create one in the scene editor and add it to the scenes you stream with.",
                )
              : t(
                  "Wheel of luck widget that viewers spin. Viewers can only spin while one is picked. Create one in the scene editor and add it to the scenes you stream with.",
                )
          }
          onPick={(id) => void props.remote.control(cloudWidgetControl(props.kind, id))}
        />
      )}
    </Show>
  );
}

function MoblinCloud(props: {
  remote: Remote;
  widgets: CloudWidgets;
  onOpen: (kind: CloudWidgetKind) => void;
}) {
  const name = (kind: CloudWidgetKind, id: string | undefined) => {
    if (id === undefined) {
      return t("None");
    }
    const widgets = props.widgets;
    if (!widgets) {
      return "…";
    }
    return widgets[kind].find((widget) => widget.id === id)?.name ?? t("Unknown widget");
  };
  return (
    <Show when={props.remote.settings} fallback={<Loading />}>
      {(settings) => (
        <Show
          when={settings().moblinCloud}
          fallback={
            <Empty
              text={t("This Moblin does not share its Moblin Cloud settings. Update Moblin.")}
            />
          }
        >
          {(cloud) => (
            <Group footer={t("The widgets that show what viewers do on your Moblin Cloud page.")}>
              <NavRow
                icon="chat"
                label={t("Message")}
                detail={name("alerts", cloud().alertsWidgetId)}
                onClick={() => props.onOpen("alerts")}
              />
              <NavRow
                icon="wheel"
                label={t("Wheel of luck")}
                detail={name("wheelOfLuck", cloud().wheelOfLuckWidgetId)}
                onClick={() => props.onOpen("wheelOfLuck")}
              />
            </Group>
          )}
        </Show>
      )}
    </Show>
  );
}

type SettingsPage = "root" | "streams" | "macros" | "moblinCloud" | "alerts" | "wheelOfLuck";

function Settings(props: { remote: Remote }) {
  const [page, setPage] = createSignal<SettingsPage>("root");
  const titles = (): Record<SettingsPage, string> => ({
    root: t("Settings"),
    streams: t("Streams"),
    macros: t("Macros"),
    moblinCloud: t("Moblin Cloud"),
    alerts: t("Message"),
    wheelOfLuck: t("Wheel of luck"),
  });
  const parents: Record<SettingsPage, SettingsPage> = {
    root: "root",
    streams: "root",
    macros: "root",
    moblinCloud: "root",
    alerts: "moblinCloud",
    wheelOfLuck: "moblinCloud",
  };
  const parent = () => parents[page()];
  const cloudPages: SettingsPage[] = ["moblinCloud", "alerts", "wheelOfLuck"];
  const [widgets] = createResource(
    () => (cloudPages.includes(page()) ? props.remote.request : undefined),
    async (request) =>
      cloudWidgetsOf(
        (
          await request<{ widgets: { id: string; name: string; type: string }[] }>({
            getWidgets: {},
          })
        ).widgets,
      ),
  );
  const loaded = (): CloudWidgets => (widgets.error ? undefined : widgets.latest);
  return (
    <Show
      when={page() !== "streams"}
      fallback={<StreamsSettings remote={props.remote} onBack={() => setPage("root")} />}
    >
      <PanelView
        title={titles()[page()]}
        back={
          page() === "root"
            ? undefined
            : { label: titles()[parent()], onBack: () => setPage(parent()) }
        }
      >
        <Switch>
          <Match when={page() === "root"}>
            <Group footer={t("Only the settings Moblin shares over remote control are here.")}>
              <NavRow icon="broadcast" label={t("Streams")} onClick={() => setPage("streams")} />
              <NavRow icon="list" label={t("Macros")} onClick={() => setPage("macros")} />
              <NavRow
                icon="cloud"
                label={t("Moblin Cloud")}
                onClick={() => setPage("moblinCloud")}
              />
            </Group>
          </Match>
          <Match when={page() === "macros"}>
            <Macros remote={props.remote} />
          </Match>
          <Match when={page() === "moblinCloud"}>
            <MoblinCloud remote={props.remote} widgets={loaded()} onOpen={setPage} />
          </Match>
          <Match when={page() === "alerts"}>
            <CloudWidgetChoices
              remote={props.remote}
              kind="alerts"
              widgets={loaded()}
              failed={widgets.error !== undefined}
            />
          </Match>
          <Match when={page() === "wheelOfLuck"}>
            <CloudWidgetChoices
              remote={props.remote}
              kind="wheelOfLuck"
              widgets={loaded()}
              failed={widgets.error !== undefined}
            />
          </Match>
        </Switch>
      </PanelView>
    </Show>
  );
}

export default function Panels(props: {
  panel: Panel;
  remote: Remote;
  editor: Editor | null;
  onOpenSceneEditor: () => void;
}) {
  return (
    <Switch>
      <Match when={props.panel === "sceneWidgets"}>
        <Show
          when={props.editor}
          fallback={
            <PanelView title={t("Scene widgets")}>
              <Loading />
            </PanelView>
          }
        >
          {(editor) => (
            <SceneWidgetsPanel
              editor={editor()}
              sceneId={props.remote.state.scene}
              onOpenSceneEditor={props.onOpenSceneEditor}
            />
          )}
        </Show>
      </Match>
      <Match when={props.panel === "mic"}>
        <PanelView title={t("Mic")}>
          <Mics remote={props.remote} footer={t("Highest priority mic at the top of the list.")} />
        </PanelView>
      </Match>
      <Match when={props.panel === "bitrate"}>
        <PanelView title={t("Bitrate")}>
          <Bitrates remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "priorities"}>
        <PanelView title={t("Connection priorities")}>
          <Priorities remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "autoSceneSwitcher"}>
        <PanelView title={t("Auto scene switcher")}>
          <AutoSceneSwitchers remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "macros"}>
        <PanelView title={t("Macros")}>
          <Macros remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "settings"}>
        <Settings remote={props.remote} />
      </Match>
    </Switch>
  );
}
