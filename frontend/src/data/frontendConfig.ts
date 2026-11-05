export interface FrontendConfig {
  backendUrl: string;
  twitchSignInEnabled?: boolean;
  kickSignInEnabled?: boolean;
  landingDetailsEnabled?: boolean;
  tippingEnabled?: boolean;
}

export function signInEnabled(config: FrontendConfig, platform: "twitch" | "kick") {
  return (platform === "twitch" ? config.twitchSignInEnabled : config.kickSignInEnabled) !== false;
}
