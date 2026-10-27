import { createSignal, For, Match, Show, Switch } from "solid-js";
import Icon from "./Icon";
import { ask } from "./Modal";
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
import { formatBits } from "../data/moblin";
import { t } from "../i18n";

export type Panel = "mic" | "bitrate" | "priorities" | "autoSceneSwitcher" | "macros" | "settings";

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
              <For each={settings().srt.connectionPriorities}>
                {(connection) => {
                  const key = `setSrtConnectionPriority:${connection.id}`;
                  return (
                    <div class="flex flex-col">
                      <PhoneSwitch
                        label={connection.name}
                        on={connection.enabled}
                        disabled={props.remote.disabled(key)}
                        onToggle={(enabled) =>
                          void props.remote.control({
                            setSrtConnectionPriority: {
                              id: connection.id,
                              priority: connection.priority,
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
                          value={connection.priority}
                          disabled={props.remote.disabled(key) || !connection.enabled}
                          onChange={async (event) => {
                            const slider = event.currentTarget;
                            await props.remote.control({
                              setSrtConnectionPriority: {
                                id: connection.id,
                                priority: Number(slider.value),
                                enabled: connection.enabled,
                              },
                            });
                            slider.value = String(connection.priority);
                          }}
                        />
                        <span class="w-5 text-right text-[15px]">{connection.priority}</span>
                      </div>
                    </div>
                  );
                }}
              </For>
            </Group>
          </Show>
        </>
      )}
    </Show>
  );
}

const switchStream = async (remote: Remote, id: string) => {
  const name = remote.settings?.streams.find((stream) => stream.id === id)?.name ?? "";
  if (
    !remote.state.streaming ||
    (await ask(t("Switch this device to the stream {name}?", { name }), t("Switch stream")))
  ) {
    await remote.control({ setStream: { id } });
  }
};

function Streams(props: { remote: Remote }) {
  return (
    <Show when={props.remote.settings} fallback={<Loading />}>
      {(settings) => (
        <Choices
          items={settings().streams}
          selected={undefined}
          icon="broadcast"
          disabled={props.remote.disabled("setStream")}
          footer={t("Pick a stream to switch the device to it.")}
          onPick={(id) => void switchStream(props.remote, id)}
        />
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
        <For each={macros()}>
          {(macro) => (
            <div class={rowClass}>
              <Icon name="list" class="size-[18px] shrink-0 text-phone-tint" />
              <span class="min-w-0 flex-1 truncate">{macro.name}</span>
              <Show when={macro.running}>
                <span class="size-[8px] rounded-full bg-phone-switch" />
              </Show>
              <button
                type="button"
                class="text-phone-tint disabled:opacity-50"
                disabled={props.remote.disabled(
                  `${macro.running ? "stopMacro" : "startMacro"}:${macro.id}`,
                )}
                onClick={() =>
                  void props.remote.control(
                    macro.running
                      ? { stopMacro: { id: macro.id } }
                      : { startMacro: { id: macro.id } },
                  )
                }
              >
                {macro.running ? t("Stop") : t("Run")}
              </button>
            </div>
          )}
        </For>
      </Group>
    </Show>
  );
}

type SettingsPage = "root" | "streams" | "macros";

function Settings(props: { remote: Remote; onClose: () => void }) {
  const [page, setPage] = createSignal<SettingsPage>("root");
  const titles = (): Record<SettingsPage, string> => ({
    root: t("Settings"),
    streams: t("Streams"),
    macros: t("Macros"),
  });
  return (
    <PanelView
      title={titles()[page()]}
      back={page() === "root" ? undefined : { label: t("Settings"), onBack: () => setPage("root") }}
      onClose={props.onClose}
    >
      <Switch>
        <Match when={page() === "root"}>
          <Group footer={t("Only the settings Moblin shares over remote control are here.")}>
            <NavRow icon="broadcast" label={t("Streams")} onClick={() => setPage("streams")} />
            <NavRow icon="list" label={t("Macros")} onClick={() => setPage("macros")} />
          </Group>
        </Match>
        <Match when={page() === "streams"}>
          <Streams remote={props.remote} />
        </Match>
        <Match when={page() === "macros"}>
          <Macros remote={props.remote} />
        </Match>
      </Switch>
    </PanelView>
  );
}

export default function Panels(props: { panel: Panel; remote: Remote; onClose: () => void }) {
  return (
    <Switch>
      <Match when={props.panel === "mic"}>
        <PanelView title={t("Mic")} onClose={props.onClose}>
          <Mics remote={props.remote} footer={t("Highest priority mic at the top of the list.")} />
        </PanelView>
      </Match>
      <Match when={props.panel === "bitrate"}>
        <PanelView title={t("Bitrate")} onClose={props.onClose}>
          <Bitrates remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "priorities"}>
        <PanelView title={t("Connection priorities")} onClose={props.onClose}>
          <Priorities remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "autoSceneSwitcher"}>
        <PanelView title={t("Auto scene switcher")} onClose={props.onClose}>
          <AutoSceneSwitchers remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "macros"}>
        <PanelView title={t("Macros")} onClose={props.onClose}>
          <Macros remote={props.remote} />
        </PanelView>
      </Match>
      <Match when={props.panel === "settings"}>
        <Settings remote={props.remote} onClose={props.onClose} />
      </Match>
    </Switch>
  );
}
