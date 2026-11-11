import { Show } from "solid-js";
import { ButtonRow, LinkRow, ReorderableList, Secret, Section } from "../sceneEditor/Form";
import { NameRow } from "../sceneEditor/settings/SceneSettings";
import { liveOrRecordingDisabled, sensitive, TextBlock, ToggleLinkRow } from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import { MultiStreamingUrlSettings } from "./Url";
import { makeUniqueName, sameId } from "../../data/moblinSettings";
import {
  defaultDestination,
  type MultiStreamingDestination,
} from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

type DestinationProps = StreamFormProps & { destinationId: string };

const editDestination = (
  props: DestinationProps,
  change: (destination: MultiStreamingDestination) => void,
) =>
  props.edit((draft) => {
    const destination = draft.multiStreaming.destinations.find((candidate) =>
      sameId(candidate.id, props.destinationId),
    );
    if (destination) {
      change(destination);
    }
  });

function DestinationSettings(props: DestinationProps) {
  const destination = () =>
    props.stream.multiStreaming.destinations.find((candidate) =>
      sameId(candidate.id, props.destinationId),
    );
  return (
    <Show when={destination()}>
      {(current) => (
        <>
          <Section>
            <NameRow
              stack={props.stack}
              name={current().name}
              existing={props.stream.multiStreaming.destinations.map((candidate) => candidate.name)}
              onSubmit={(name) =>
                editDestination(props, (draft) => {
                  draft.name = name;
                })
              }
            />
          </Section>
          <Section>
            <Secret allowed={props.remote.secrets} label={t("URL")}>
              <LinkRow
                label={t("URL")}
                value={sensitive(current().url)}
                onClick={() =>
                  props.stack.push({
                    title: t("URL"),
                    render: () => <MultiStreamingUrlSettings {...props} />,
                  })
                }
              />
            </Secret>
          </Section>
        </>
      )}
    </Show>
  );
}

function MultiStreamingSettings(props: StreamFormProps) {
  const destinations = () => props.stream.multiStreaming.destinations;
  return (
    <>
      <Section>
        <TextBlock>
          <span class="flex flex-col">
            <span>{t("Stream to additional destinations directly from this device.")}</span>
            <br />
            <span>
              {t("⚠️ This will increase network bandwidth usage, system load and device heat.")}
            </span>
          </span>
        </TextBlock>
      </Section>
      <Section>
        <ReorderableList
          items={destinations()}
          render={(destination) => (
            <ToggleLinkRow
              label={destination.name}
              on={destination.enabled}
              disabled={liveOrRecordingDisabled(props)}
              onToggle={(on) =>
                editDestination({ ...props, destinationId: destination.id }, (draft) => {
                  draft.enabled = on;
                })
              }
              onClick={() =>
                props.stack.push({
                  title: t("Destination"),
                  render: () => <DestinationSettings {...props} destinationId={destination.id} />,
                })
              }
            />
          )}
          onDelete={(destination) =>
            props.edit((draft) => {
              draft.multiStreaming.destinations = draft.multiStreaming.destinations.filter(
                (candidate) => !sameId(candidate.id, destination.id),
              );
            })
          }
          deleteDisabled={() => liveOrRecordingDisabled(props)}
        />
        <ButtonRow
          label={t("Create")}
          disabled={liveOrRecordingDisabled(props)}
          onClick={() =>
            props.edit((draft) => {
              draft.multiStreaming.destinations.push(
                defaultDestination(
                  makeUniqueName(
                    "My destination",
                    draft.multiStreaming.destinations.map((destination) => destination.name),
                  ),
                ),
              );
            })
          }
        />
      </Section>
    </>
  );
}

export function MultiStreamingRow(props: StreamFormProps) {
  return (
    <LinkRow
      label={t("Multi streaming")}
      value={String(
        props.stream.multiStreaming.destinations.filter((destination) => destination.enabled)
          .length,
      )}
      onClick={() =>
        props.stack.push({
          title: t("Multi streaming"),
          render: () => <MultiStreamingSettings {...props} />,
        })
      }
    />
  );
}
