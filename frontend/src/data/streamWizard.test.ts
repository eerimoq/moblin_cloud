import { describe, expect, test } from "vitest";
import {
  createStreamFromWizard,
  defaultWizard,
  extractSrtStreamId,
  type CreateStreamWizard,
} from "./streamWizard";

const wizard = (changes: Partial<CreateStreamWizard>): CreateStreamWizard => ({
  ...defaultWizard(),
  ...changes,
});

const media = (wizard: CreateStreamWizard, highestBitrate?: number) => {
  const stream = createStreamFromWizard(wizard, highestBitrate);
  return {
    url: stream.url,
    codec: stream.codec,
    rateControl: stream.bitrateRateControl,
    audioCodec: stream.audioCodec,
    audioBitrate: stream.audioBitrate,
  };
};

const obsRemoteControl = {
  obsRemoteControlEnabled: true,
  obsRemoteControlUrl: " WS://213.33.45.132:4567 ",
  obsRemoteControlPassword: " po3Gg4pflp3s ",
  obsRemoteControlMainScene: " Main ",
  obsRemoteControlBrbScene: " BRB ",
  obsRemoteControlSourceName: " Moblin ",
};

describe("general", () => {
  test("fields", () => {
    const stream = createStreamFromWizard(
      wizard({
        name: " My Twitch ",
        backgroundStreaming: true,
        goLiveNotificationMoblinWebsite: true,
      }),
    );
    expect(stream.name).toBe("My Twitch");
    expect(stream.enabled).toBe(false);
    expect(stream.backgroundStreaming).toBe(true);
    expect(stream.autoGoLive).toBe(false);
    expect(stream.goLiveNotificationMoblinWebsite).toBe(true);
    expect(stream.chat).toEqual({ bttvEmotes: false, ffzEmotes: false, seventvEmotes: false });
    expect(stream.obsWebSocketEnabled).toBe(false);
  });

  test("new ids", () => {
    expect(createStreamFromWizard(defaultWizard()).id).not.toBe(
      createStreamFromWizard(defaultWizard()).id,
    );
  });
});

describe("platforms", () => {
  test("Twitch direct", () => {
    const created = wizard({
      platform: "twitch",
      networkSetup: "direct",
      twitchChannelName: " MyChannel ",
      twitchChannelId: " 908123903 ",
      directIngest: "rtmp://arn03.contribute.live-video.net/app/",
      directStreamKey: "live_48950233_okF4f455",
      ...obsRemoteControl,
    });
    const stream = createStreamFromWizard(created);
    expect(stream.twitchChannelName).toBe("MyChannel");
    expect(stream.twitchChannelId).toBe("908123903");
    expect(stream.twitchLoggedIn).toBe(false);
    expect(stream.obsWebSocketEnabled).toBe(false);
    expect(media(created)).toEqual({
      url: "rtmp://arn03.contribute.live-video.net/app/live_48950233_okF4f455",
      codec: "H.264/AVC",
      rateControl: "CBR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("Kick OBS", () => {
    const created = wizard({
      platform: "kick",
      networkSetup: "obs",
      kickChannelName: " erimo ",
      obsAddress: "213.33.45.132",
      obsPort: "7654",
      ...obsRemoteControl,
    });
    const stream = createStreamFromWizard(created);
    expect(stream.kickChannelName).toBe("erimo");
    expect(stream.kickLoggedIn).toBe(false);
    expect(stream.obsWebSocketEnabled).toBe(true);
    expect(stream.obsWebSocketUrl).toBe("ws://213.33.45.132:4567");
    expect(stream.obsWebSocketPassword).toBe("po3Gg4pflp3s");
    expect(stream.obsMainScene).toBe("Main");
    expect(stream.obsBrbScene).toBe("BRB");
    expect(stream.obsSourceName).toBe("Moblin");
    expect(stream.streamingDirectlyToObs).toBe(true);
    expect(media(created)).toEqual({
      url: "srt://213.33.45.132:7654",
      codec: "H.265/HEVC",
      rateControl: "ABR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("YouTube BELABOX cloud", () => {
    const created = wizard({
      platform: "youTube",
      networkSetup: "belaboxCloudObs",
      youTubeHandle: " @erimo144 ",
      belaboxUrl: "srtla://uk.srt.belabox.net:5000?streamid=jO4ijfFgrlpv4m2375msdoG3DDr2",
      ...obsRemoteControl,
    });
    const stream = createStreamFromWizard(created);
    expect(stream.youTubeHandle).toBe("@erimo144");
    expect(stream.youTubeWantsToBeLoggedIn).toBe(false);
    expect(stream.obsWebSocketEnabled).toBe(true);
    expect(stream.streamingDirectlyToObs).toBe(false);
    expect(media(created)).toEqual({
      url: "srtla://uk.srt.belabox.net:5000?streamid=jO4ijfFgrlpv4m2375msdoG3DDr2",
      codec: "H.265/HEVC",
      rateControl: "ABR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("SOOP my servers RTMP", () => {
    const created = wizard({
      platform: "soop",
      networkSetup: "myServers",
      customProtocol: "rtmp",
      soopChannelName: " MyChannel ",
      soopStreamId: " 908123903 ",
      customRtmpUrl: "rtmp://192.168.1.2:1935/live/",
      customRtmpStreamKey: " my key ",
    });
    const stream = createStreamFromWizard(created);
    expect(stream.afreecaTvChannelName).toBe("MyChannel");
    expect(stream.afreecaTvStreamId).toBe("908123903");
    expect(media(created)).toEqual({
      url: "rtmp://192.168.1.2:1935/live/my%20key",
      codec: "H.264/AVC",
      rateControl: "CBR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("SOOP needs both fields", () => {
    const stream = createStreamFromWizard(
      wizard({ platform: "soop", soopChannelName: "MyChannel", soopStreamId: "" }),
    );
    expect(stream.afreecaTvChannelName).toBe("");
    expect(stream.afreecaTvStreamId).toBe("");
  });

  test("Twitch my servers SRT", () => {
    const created = wizard({
      platform: "twitch",
      networkSetup: "myServers",
      customProtocol: "srt",
      customSrtUrl: "srt://107.32.12.132:5000?latency=2000&streamid=1234",
      customSrtStreamId: "#!::r=stream/-NDZ1WPA4zjMBTJTyNwU,m=publish",
      ...obsRemoteControl,
    });
    const stream = createStreamFromWizard(created);
    expect(stream.obsWebSocketEnabled).toBe(true);
    expect(stream.streamingDirectlyToObs).toBe(false);
    expect(media(created)).toEqual({
      url: "srt://107.32.12.132:5000?latency=2000&streamid=%23!::r%3Dstream/-NDZ1WPA4zjMBTJTyNwU,m%3Dpublish",
      codec: "H.265/HEVC",
      rateControl: "ABR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("OBS", () => {
    const created = wizard({
      platform: "obs",
      networkSetup: "obs",
      obsAddress: "my.domain.org",
      obsPort: "7654",
    });
    expect(createStreamFromWizard(created).obsWebSocketEnabled).toBe(false);
    expect(media(created)).toEqual({
      url: "srt://my.domain.org:7654",
      codec: "H.265/HEVC",
      rateControl: "ABR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("Mobcam", () => {
    const created = wizard({ platform: "mobcam", autoGoLive: true });
    const stream = createStreamFromWizard(created, 8000000);
    expect(stream.autoGoLive).toBe(true);
    expect(stream.bitrate).toBe(8000000);
    expect(media(created)).toEqual({
      url: "mobcam://localhost:7790",
      codec: "H.265/HEVC",
      rateControl: "CBR",
      audioCodec: "AAC",
      audioBitrate: 192000,
    });
  });

  test("Mobcam without bitrate presets", () => {
    expect(createStreamFromWizard(wizard({ platform: "mobcam" })).bitrate).toBe(5000000);
  });
});

describe("custom", () => {
  test("none", () => {
    const created = wizard({ platform: "custom", customProtocol: "none", ...obsRemoteControl });
    expect(createStreamFromWizard(created).obsWebSocketEnabled).toBe(false);
    expect(media(created)).toEqual({
      url: "srt://my_public_ip:4000",
      codec: "H.264/AVC",
      rateControl: "ABR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("SRT", () => {
    expect(
      media(
        wizard({
          platform: "custom",
          customProtocol: "srt",
          customSrtUrl: " srtla://107.32.12.132:5000?streamid=1234 ",
          customSrtStreamId: " 5678 ",
        }),
      ),
    ).toEqual({
      url: "srtla://107.32.12.132:5000?streamid=5678",
      codec: "H.265/HEVC",
      rateControl: "ABR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("SRT without a query", () => {
    expect(
      createStreamFromWizard(
        wizard({
          platform: "custom",
          customProtocol: "srt",
          customSrtUrl: "srt://107.32.12.132:5000",
          customSrtStreamId: "1234",
        }),
      ).url,
    ).toBe("srt://107.32.12.132:5000");
  });

  test("RTMP", () => {
    expect(
      media(
        wizard({
          platform: "custom",
          customProtocol: "rtmp",
          customRtmpUrl: "RTMP://arn03.contribute.live-video.net/app/",
          customRtmpStreamKey: "live_48950233_okF4f455",
        }),
      ),
    ).toEqual({
      url: "rtmp://arn03.contribute.live-video.net/app/live_48950233_okF4f455",
      codec: "H.264/AVC",
      rateControl: "CBR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("RIST", () => {
    expect(
      media(
        wizard({
          platform: "custom",
          customProtocol: "rist",
          customRistUrl: " rist://120.35.234.2:2030 ",
        }),
      ),
    ).toEqual({
      url: "rist://120.35.234.2:2030",
      codec: "H.265/HEVC",
      rateControl: "ABR",
      audioCodec: "AAC",
      audioBitrate: 128000,
    });
  });

  test("WHIP", () => {
    expect(
      media(
        wizard({
          platform: "custom",
          customProtocol: "whip",
          customWhipUrl: "whip://120.12.32.12:8889/mystream/whip",
        }),
      ),
    ).toEqual({
      url: "whip://120.12.32.12:8889/mystream/whip",
      codec: "H.264/AVC",
      rateControl: "ABR",
      audioCodec: "OPUS",
      audioBitrate: 128000,
    });
  });
});

describe("malformed", () => {
  test("OBS without an address uses the default URL", () => {
    expect(createStreamFromWizard(wizard({ platform: "kick", networkSetup: "obs" })).url).toBe(
      "srt://my_public_ip:4000",
    );
  });

  test("direct without an ingest uses the default URL", () => {
    expect(createStreamFromWizard(wizard({ platform: "kick", networkSetup: "direct" })).url).toBe(
      "srt://my_public_ip:4000",
    );
  });

  test("OBS remote control with a bad URL stays off", () => {
    const stream = createStreamFromWizard(
      wizard({
        platform: "kick",
        networkSetup: "obs",
        obsAddress: "213.33.45.132",
        obsPort: "7654",
        ...obsRemoteControl,
        obsRemoteControlUrl: "http://213.33.45.132:4567",
      }),
    );
    expect(stream.obsWebSocketEnabled).toBe(false);
    expect(stream.obsWebSocketUrl).toBe("");
  });
});

describe("extractSrtStreamId", () => {
  test.each([
    ["srt://107.32.12.132:5000?streamid=1234", "1234"],
    ["srt://107.32.12.132:5000?latency=2000&streamid=a%20b", "a b"],
    ["srt://107.32.12.132:5000?streamid=1&streamid=2", "2"],
    ["srt://107.32.12.132:5000", null],
    ["srt://107.32.12.132:5000?latency=2000", null],
    ["srt://107.32.12.132 :5000?streamid=1234", null],
  ])("%s", (url, streamId) => {
    expect(extractSrtStreamId(url)).toBe(streamId);
  });
});
