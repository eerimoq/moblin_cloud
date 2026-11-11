import { Section, TextRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import { isValidWebSocketUrl } from "../../data/streamUrls";
import { t } from "../../i18n";

export function OpenStreamingPlatformSettings(props: StreamFormProps) {
  return (
    <Section>
      <TextRow
        stack={props.stack}
        label={t("URL")}
        value={props.stream.openStreamingPlatformUrl}
        placeholder="ws://foo.org:5443/ws"
        validate={isValidWebSocketUrl}
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.openStreamingPlatformUrl = value;
          })
        }
      />
      <TextRow
        stack={props.stack}
        label={t("Channel id")}
        value={props.stream.openStreamingPlatformChannelId}
        placeholder="4e9f02fc-cee9-4d1c-b4b5-99b9496375c8"
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.openStreamingPlatformChannelId = value;
          })
        }
      />
    </Section>
  );
}
