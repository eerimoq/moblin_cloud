import { For, Show } from "solid-js";
import Icon from "../../../Icon";
import {
  ButtonRow,
  ColorRow,
  InlinePicker,
  LinkRow,
  MenuRow,
  ReorderableList,
  Section,
  SliderRow,
  TextEditPage,
  TextRow,
  ToggleRow,
  useReadOnly,
} from "../../Form";
import { formatShortDuration, pickerItems, type WidgetFormProps } from "./shared";
import { NameRow } from "../SceneSettings";
import {
  defaultGolfPlayer,
  makeUniqueName,
  newUuid,
  rgb,
  sameId,
  type ClockDirection,
  type ScoreboardClock,
  type ScoreboardLayout,
  type ScoreboardSport,
  type ScoreboardTeam,
  type SettingsWidget,
} from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export const sports = (): { id: ScoreboardSport; name: string }[] => [
  { id: "generic", name: t("Generic") },
  { id: "padel", name: t("Padel") },
  { id: "golf", name: t("Golf") },
  { id: "golfFullScorecard", name: t("Golf full scorecard") },
  { id: "basketball", name: t("Basketball") },
  { id: "generic2", name: t("Generic 2") },
  { id: "genericSets", name: t("Generic sets") },
  { id: "hockey", name: t("Hockey") },
  { id: "football", name: t("Football") },
  { id: "tennis", name: t("Tennis") },
  { id: "volleyball", name: t("Volleyball") },
];

const layouts = (): { id: string; name: string; layout: ScoreboardLayout }[] => [
  { id: "stacked", name: t("Stacked"), layout: { stacked: {} } },
  { id: "stackedInline", name: t("Stacked inline"), layout: { stackedInline: {} } },
  { id: "sideBySide", name: t("Side by side"), layout: { sideBySide: {} } },
  { id: "stackHistory", name: t("Stack history"), layout: { stackHistory: {} } },
];

const keyOf = (value: object) => Object.keys(value)[0] ?? "";

function ColorsLink(props: WidgetFormProps) {
  const scoreboard = () => props.widget.scoreboard;
  return (
    <LinkRow
      label={t("Colors")}
      onClick={() =>
        props.stack.push({
          title: t("Colors"),
          render: () => (
            <>
              <Section>
                <ColorRow
                  label={t("Text")}
                  color={scoreboard().textColor}
                  onChange={(color) =>
                    props.edit((draft) => {
                      draft.scoreboard.textColor = color;
                    })
                  }
                />
                <ColorRow
                  label={t("Primary background")}
                  color={scoreboard().primaryBackgroundColor}
                  onChange={(color) =>
                    props.edit((draft) => {
                      draft.scoreboard.primaryBackgroundColor = color;
                    })
                  }
                />
                <ColorRow
                  label={t("Secondary background")}
                  color={scoreboard().secondaryBackgroundColor}
                  onChange={(color) =>
                    props.edit((draft) => {
                      draft.scoreboard.secondaryBackgroundColor = color;
                    })
                  }
                />
              </Section>
              <Section>
                <ButtonRow
                  label={t("Reset")}
                  onClick={() =>
                    props.edit((draft) => {
                      draft.scoreboard.textColor = rgb(255, 255, 255);
                      draft.scoreboard.primaryBackgroundColor = rgb(11, 16, 172);
                      draft.scoreboard.secondaryBackgroundColor = rgb(0, 3, 91);
                    })
                  }
                />
              </Section>
            </>
          ),
        })
      }
    />
  );
}

function ClockSection(
  props: WidgetFormProps & {
    clock: ScoreboardClock;
    editClock: (change: (clock: ScoreboardClock) => void) => void;
  },
) {
  return (
    <Section header={t("Clock")}>
      <LinkRow
        label={t("Maximum")}
        value={formatShortDuration(60 * props.clock.maximum)}
        onClick={() =>
          props.stack.push({
            title: t("Maximum"),
            render: () => (
              <TextEditPage
                value={String(props.clock.maximum)}
                validate={(value) => {
                  if (!/^[+-]?\d+$/.test(value.trim())) {
                    return t("Not a number");
                  }
                  const maximum = Number(value.trim());
                  if (maximum <= 0) {
                    return t("Too small");
                  }
                  if (maximum > 180) {
                    return t("Too big");
                  }
                  return null;
                }}
                onSubmit={(value) =>
                  props.editClock((clock) => {
                    clock.maximum = Number(value);
                  })
                }
                onDone={() => props.stack.pop()}
              />
            ),
          })
        }
      />
      <MenuRow
        label={t("Direction")}
        items={[
          { id: "up", name: t("Up") },
          { id: "down", name: t("Down") },
        ]}
        selected={keyOf(props.clock.direction)}
        onPick={(id) =>
          props.editClock((clock) => {
            clock.direction = (id === "up" ? { up: {} } : { down: {} }) as ClockDirection;
          })
        }
      />
    </Section>
  );
}

function TeamRow(
  props: WidgetFormProps & {
    side: string;
    team: ScoreboardTeam;
    editTeam: (change: (team: ScoreboardTeam) => void) => void;
  },
) {
  return (
    <LinkRow
      label={props.side}
      value={props.team.name}
      onClick={() =>
        props.stack.push({
          title: props.side,
          render: () => (
            <Section>
              <TextRow
                stack={props.stack}
                label={t("Name")}
                value={props.team.name}
                onSubmit={(name) =>
                  props.editTeam((team) => {
                    team.name = name;
                  })
                }
              />
              <ColorRow
                label={t("Text")}
                color={props.team.textColor}
                onChange={(color) =>
                  props.editTeam((team) => {
                    team.textColor = color;
                  })
                }
              />
              <ColorRow
                label={t("Background")}
                color={props.team.backgroundColor}
                onChange={(color) =>
                  props.editTeam((team) => {
                    team.backgroundColor = color;
                  })
                }
              />
            </Section>
          ),
        })
      }
    />
  );
}

function PlayersSection(props: WidgetFormProps) {
  const players = () => props.editor.state.scoreboardPlayers;
  const setPlayers = (
    change: (players: { id: string; name: string }[]) => { id: string; name: string }[],
  ) => props.editor.setScoreboardPlayers(change(players().map((player) => ({ ...player }))));
  return (
    <Section header={t("Players")} footer={t("Delete a player with its trash button.")}>
      <ReorderableList
        items={players()}
        render={(player) => (
          <NameRow
            stack={props.stack}
            name={player.name}
            existing={players().map((candidate) => candidate.name)}
            onSubmit={(name) =>
              setPlayers((list) =>
                list.map((candidate) =>
                  candidate.id === player.id ? { ...candidate, name } : candidate,
                ),
              )
            }
          />
        )}
        onMove={(from, to) =>
          setPlayers((list) => {
            const [moved] = list.splice(from, 1);
            list.splice(to, 0, moved!);
            return list;
          })
        }
        onDelete={(player) =>
          setPlayers((list) => list.filter((candidate) => candidate.id !== player.id))
        }
      />
      <ButtonRow
        label={t("Create")}
        onClick={() =>
          setPlayers((list) => [
            ...list,
            {
              id: newUuid(),
              name: makeUniqueName(
                "🇸🇪 Moblin",
                list.map((player) => player.name),
              ),
            },
          ])
        }
      />
    </Section>
  );
}

function PadelSections(props: WidgetFormProps) {
  const padel = () => props.widget.scoreboard.padel;
  const players = () => props.editor.state.scoreboardPlayers;
  const playerRow = (key: "homePlayer1" | "homePlayer2" | "awayPlayer1" | "awayPlayer2") => {
    const player = () => players().find((candidate) => sameId(candidate.id, padel()[key]));
    return (
      <LinkRow
        label={player()?.name ?? "🇸🇪 Moblin"}
        onClick={() =>
          props.stack.push({
            title: t("Name"),
            render: () => (
              <InlinePicker
                items={players()}
                selected={player()?.id ?? padel()[key]}
                onPick={(id) => {
                  props.edit((draft) => {
                    draft.scoreboard.padel[key] = id;
                  });
                  props.stack.pop();
                }}
              />
            ),
          })
        }
      />
    );
  };
  return (
    <>
      <Section header={t("Home")}>
        {playerRow("homePlayer1")}
        <Show when={padel().type === "Double"}>{playerRow("homePlayer2")}</Show>
      </Section>
      <Section header={t("Away")}>
        {playerRow("awayPlayer1")}
        <Show when={padel().type === "Double"}>{playerRow("awayPlayer2")}</Show>
      </Section>
      <PlayersSection {...props} />
    </>
  );
}

function GolfSections(props: WidgetFormProps) {
  const readOnly = useReadOnly();
  const golf = () => props.widget.scoreboard.golf;
  const editGolf = (change: (golf: SettingsWidget["scoreboard"]["golf"]) => void) =>
    props.edit((draft) => change(draft.scoreboard.golf));
  return (
    <>
      <Section header={t("Round")}>
        <TextRow
          stack={props.stack}
          label={t("Title")}
          value={golf().eventName}
          onSubmit={(value) =>
            editGolf((draft) => {
              draft.eventName = value;
            })
          }
        />
        <MenuRow
          label={t("Holes")}
          items={pickerItems([9, 18])}
          selected={String(golf().numberOfHoles)}
          onPick={(id) =>
            editGolf((draft) => {
              draft.numberOfHoles = Number(id);
              draft.currentHole = 0;
            })
          }
        />
        <LinkRow
          label={t("Par")}
          value={String(
            golf()
              .pars.slice(0, golf().numberOfHoles)
              .reduce((sum, par) => sum + par, 0),
          )}
          onClick={() =>
            props.stack.push({
              title: t("Pars"),
              render: () => (
                <Section>
                  <For each={Array.from({ length: golf().numberOfHoles }, (_, index) => index)}>
                    {(index) => (
                      <MenuRow
                        label={t("Hole {number}", { number: String(index + 1) })}
                        items={pickerItems([1, 2, 3, 4, 5, 6, 7, 8, 9])}
                        selected={String(golf().pars[index] ?? 4)}
                        onPick={(id) =>
                          editGolf((draft) => {
                            draft.pars[index] = Number(id);
                          })
                        }
                      />
                    )}
                  </For>
                </Section>
              ),
            })
          }
        />
      </Section>
      <Section header={t("Players")} footer={t("Delete a player with its trash button.")}>
        <For each={golf().players}>
          {(player) => (
            <div class="flex min-h-[44px] items-center">
              <div class="min-w-0 flex-1">
                <TextRow
                  stack={props.stack}
                  label={t("Name")}
                  value={player.name}
                  onSubmit={(name) =>
                    editGolf((draft) => {
                      const target = draft.players.find((candidate) =>
                        sameId(candidate.id, player.id),
                      );
                      if (target) {
                        target.name = name;
                      }
                    })
                  }
                />
              </div>
              <button
                type="button"
                aria-label={t("Remove")}
                class="grid size-[44px] shrink-0 place-items-center text-phone-live disabled:opacity-35"
                disabled={readOnly()}
                onClick={() =>
                  editGolf((draft) => {
                    draft.players = draft.players.filter(
                      (candidate) => !sameId(candidate.id, player.id),
                    );
                  })
                }
              >
                <Icon name="trash" class="size-[18px]" />
              </button>
            </div>
          )}
        </For>
        <Show when={golf().players.length < 4}>
          <ButtonRow
            label={t("Create")}
            onClick={() =>
              editGolf((draft) => {
                draft.players.push(
                  defaultGolfPlayer(newUuid(), `Player ${draft.players.length + 1}`),
                );
              })
            }
          />
        </Show>
      </Section>
    </>
  );
}

function ModularLayoutLink(props: WidgetFormProps) {
  const modular = () => props.widget.scoreboard.modular;
  const editModular = (change: (modular: SettingsWidget["scoreboard"]["modular"]) => void) =>
    props.edit((draft) => change(draft.scoreboard.modular));
  return (
    <LinkRow
      label={t("Layout")}
      onClick={() =>
        props.stack.push({
          title: t("Layout"),
          render: () => (
            <>
              <Section>
                <MenuRow
                  label={t("Type")}
                  items={layouts()}
                  selected={keyOf(modular().layout)}
                  onPick={(id) =>
                    editModular((draft) => {
                      draft.layout = layouts().find((layout) => layout.id === id)?.layout ?? {
                        stacked: {},
                      };
                    })
                  }
                />
              </Section>
              <Section>
                <SliderRow
                  label={t("Width")}
                  value={modular().width}
                  minimum={100}
                  maximum={1000}
                  step={1}
                  format={(value) => String(Math.round(value))}
                  onChange={(value) =>
                    editModular((draft) => {
                      draft.width = value;
                    })
                  }
                />
                <SliderRow
                  label={t("Height")}
                  value={modular().rowHeight}
                  minimum={10}
                  maximum={150}
                  step={1}
                  format={(value) => String(Math.round(value))}
                  onChange={(value) =>
                    editModular((draft) => {
                      draft.rowHeight = value;
                    })
                  }
                />
              </Section>
              <Section>
                <ToggleRow
                  label={t("Title")}
                  on={modular().showTitle}
                  onToggle={(on) =>
                    editModular((draft) => {
                      draft.showTitle = on;
                    })
                  }
                />
                <ToggleRow
                  label={t("More stats")}
                  on={modular().showMoreStats}
                  onToggle={(on) =>
                    editModular((draft) => {
                      draft.showMoreStats = on;
                    })
                  }
                />
                <ToggleRow
                  label={t("Info box")}
                  on={modular().showGlobalStatsBlock}
                  onToggle={(on) =>
                    editModular((draft) => {
                      draft.showGlobalStatsBlock = on;
                    })
                  }
                />
                <ToggleRow
                  label={t("Clock")}
                  on={modular().showClock}
                  disabled={!modular().showGlobalStatsBlock}
                  onToggle={(on) =>
                    editModular((draft) => {
                      draft.showClock = on;
                    })
                  }
                />
                <ToggleRow
                  label={t("Bold")}
                  on={modular().isBold}
                  onToggle={(on) =>
                    editModular((draft) => {
                      draft.isBold = on;
                    })
                  }
                />
              </Section>
            </>
          ),
        })
      }
    />
  );
}

export function ScoreboardForm(props: WidgetFormProps) {
  const scoreboard = () => props.widget.scoreboard;
  const sport = () => scoreboard().type;
  const modular = () => !["generic", "padel", "golf", "golfFullScorecard"].includes(sport());
  const web = () => props.editor.state.options?.webRemote;
  const webUrl = (path: string) => {
    const remote = web();
    const host = remote?.addresses[0];
    if (!remote || !host) {
      return null;
    }
    return `http://${host}${remote.port === 80 ? "" : `:${remote.port}`}${path}`;
  };
  const remoteControlRows = (path: string) => (
    <>
      <span class="px-4 py-2 text-[17px]">
        {t("Use the web based remote control on another device to update the scoreboard.")}
      </span>
      <Show when={web()?.enabled && webUrl(path)}>
        {(url) => (
          <div class="flex items-center gap-2 px-4 py-2 text-[15px]">
            <Icon name="globe" class="size-[18px] shrink-0 text-phone-tint" />
            <span class="min-w-0 flex-1 truncate">{url()}</span>
            <button
              type="button"
              aria-label={t("Copy")}
              class="text-phone-tint"
              onClick={() => void navigator.clipboard?.writeText(url())}
            >
              <Icon name="copy" class="size-[18px]" />
            </button>
          </div>
        )}
      </Show>
      <Show when={!web()?.enabled}>
        <span class="px-4 py-2 text-[17px]">
          {t("⚠️ The web based remote control is not enabled.")}
        </span>
      </Show>
    </>
  );
  return (
    <>
      <Section>
        <MenuRow
          label={t("Sport")}
          items={sports()}
          selected={sport()}
          onPick={(id) =>
            props.edit((draft) => {
              draft.scoreboard.type = id as ScoreboardSport;
            })
          }
        />
        <Show when={sport() === "padel"}>
          <MenuRow
            label={t("Game type")}
            items={[
              { id: "Double", name: t("Doubles") },
              { id: "Single", name: t("Singles") },
            ]}
            selected={scoreboard().padel.type}
            onPick={(id) =>
              props.edit((draft) => {
                draft.scoreboard.padel.type = id as "Double" | "Single";
              })
            }
          />
        </Show>
        <Show when={sport() === "generic"}>
          <TextRow
            stack={props.stack}
            label={t("Title")}
            value={scoreboard().generic.title}
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.scoreboard.generic.title = value;
              })
            }
          />
        </Show>
        <Show when={!modular()}>
          <ColorsLink {...props} />
        </Show>
        <Show when={sport() === "golfFullScorecard"}>
          <ToggleRow
            label={t("Show pars")}
            on={scoreboard().golf.showPars}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.scoreboard.golf.showPars = on;
              })
            }
          />
        </Show>
        <Show when={modular()}>
          <ModularLayoutLink {...props} />
        </Show>
      </Section>
      <Section header={t("Remote control")}>
        <Show when={sport() === "padel" || sport() === "generic"}>
          <span class="px-4 py-2 text-[17px]">
            {t("Use your Apple Watch to update the scoreboard.")}
          </span>
        </Show>
        <Show when={sport() === "golf"}>{remoteControlRows("/golf.html")}</Show>
        <Show when={sport() === "golfFullScorecard"}>
          <span class="px-4 py-2 text-[17px]">
            {t("Use a golf scoreboard widget to control this widget.")}
          </span>
        </Show>
        <Show when={modular()}>{remoteControlRows("/remote.html")}</Show>
      </Section>
      <Show when={sport() === "padel"}>
        <PadelSections {...props} />
      </Show>
      <Show when={sport() === "generic"}>
        <Section header={t("Teams")}>
          <TextRow
            stack={props.stack}
            label={t("Home")}
            value={scoreboard().generic.home}
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.scoreboard.generic.home = value;
              })
            }
          />
          <TextRow
            stack={props.stack}
            label={t("Away")}
            value={scoreboard().generic.away}
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.scoreboard.generic.away = value;
              })
            }
          />
        </Section>
        <ClockSection
          {...props}
          clock={scoreboard().generic.clock}
          editClock={(change) => props.edit((draft) => change(draft.scoreboard.generic.clock))}
        />
      </Show>
      <Show when={sport() === "golf"}>
        <GolfSections {...props} />
      </Show>
      <Show when={modular()}>
        <Section header={t("Teams")}>
          <TeamRow
            {...props}
            side={t("Home")}
            team={scoreboard().modular.home}
            editTeam={(change) => props.edit((draft) => change(draft.scoreboard.modular.home))}
          />
          <TeamRow
            {...props}
            side={t("Away")}
            team={scoreboard().modular.away}
            editTeam={(change) => props.edit((draft) => change(draft.scoreboard.modular.away))}
          />
        </Section>
        <ClockSection
          {...props}
          clock={scoreboard().modular.clock}
          editClock={(change) => props.edit((draft) => change(draft.scoreboard.modular.clock))}
        />
      </Show>
    </>
  );
}
