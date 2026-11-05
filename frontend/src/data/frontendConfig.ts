export interface FrontendConfig {
  backendUrl: string;
  selfHosted?: boolean;
  landingDetailsEnabled?: boolean;
  tippingEnabled?: boolean;
}

export interface SignInProviders {
  twitch: boolean;
  kick: boolean;
}

export function signInEnabled(providers: SignInProviders | null, platform: "twitch" | "kick") {
  return providers?.[platform] === true;
}
