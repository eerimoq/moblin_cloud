import { MenuRow, Secret, Section } from "../sceneEditor/Form";
import { EditRow, liveDisabled, Lines } from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import type { WhipHttpTransport } from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

export function WhipSettings(props: StreamFormProps) {
  const bearerToken = () => {
    const authorization = props.stream.whip.headers.find(
      (header) => header.name === "Authorization",
    );
    return authorization ? (/^Bearer (.*)/.exec(authorization.value)?.[1] ?? "") : "";
  };
  const transport = () => Object.keys(props.stream.whip.httpTransport)[0] ?? "standard";
  return (
    <>
      <Section>
        <Secret allowed={props.remote.secrets} label={t("Bearer token")}>
          <EditRow
            stack={props.stack}
            label={t("Bearer token")}
            value={bearerToken()}
            masked
            disabled={liveDisabled(props)}
            onSubmit={(token) =>
              props.edit((draft) => {
                const value = `Bearer ${token}`;
                const authorization = draft.whip.headers.find(
                  (header) => header.name === "Authorization",
                );
                if (authorization) {
                  authorization.value = value;
                } else {
                  draft.whip.headers.push({ name: "Authorization", value });
                }
              })
            }
          />
        </Secret>
      </Section>
      <Section
        footer={
          <Lines
            lines={[
              t("Select {name} to use standard WHIP.", { name: t("Standard") }),
              "",
              t(
                "Select {name} to exchange connection establishment information via the remote control. Configure this device as remote control assistant, and the device you are streaming to as remote control streamer.",
                { name: t("Remote control") },
              ),
            ]}
          />
        }
      >
        <MenuRow
          label={t("HTTP transport")}
          items={[
            { id: "standard", name: t("Standard") },
            { id: "remoteControl", name: t("Remote control") },
          ]}
          selected={transport()}
          disabled={liveDisabled(props)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.whip.httpTransport = { [id]: {} } as WhipHttpTransport;
            })
          }
        />
      </Section>
    </>
  );
}
