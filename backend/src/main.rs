mod api;
mod app_attest;
mod challenges;
mod kick;
mod live;
mod live_status;
mod profiles;
mod remote_control;
mod store;
mod twitch;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use log::{info, warn};
use reqwest::Client;

use crate::api::Api;
use crate::app_attest::{AppAttest, Environment};
use crate::challenges::Challenges;
use crate::kick::Kick;
use crate::live::Live;
use crate::live_status::LiveStatus;
use crate::profiles::Profiles;
use crate::remote_control::Streamers;
use crate::store::Store;
use crate::twitch::Twitch;

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Parser)]
#[command(
    name = "moblin-website-backend",
    version,
    about = "Serves the list of streamers who recently went live with Moblin"
)]
struct Cli {
    /// Address to listen on.
    #[arg(long, env = "LISTEN", default_value = "127.0.0.1:8080")]
    listen: SocketAddr,
    /// Never list more than this many streamers, live ones first. Older
    /// streamers who are still live are remembered beyond that, so that
    /// they are listed again when a newer one leaves.
    #[arg(long, env = "MAX_STREAMERS", default_value_t = 6)]
    max_streamers: usize,
    /// Pause at least this many seconds between two requests to the
    /// platforms, when looking profiles and live statuses up.
    #[arg(long, env = "LOOKUP_SPACING", default_value_t = 2)]
    lookup_spacing: u64,
    /// The client ID of a Twitch application, used together with its secret
    /// to look Twitch profiles up. Without one Twitch streamers are listed
    /// without a display name and avatar.
    #[arg(long, env = "TWITCH_CLIENT_ID", requires = "twitch_client_secret")]
    twitch_client_id: Option<String>,
    /// The client secret of the Twitch application.
    #[arg(
        long,
        env = "TWITCH_CLIENT_SECRET",
        hide_env_values = true,
        requires = "twitch_client_id"
    )]
    twitch_client_secret: Option<String>,
    /// The Twitch channel whose live status the website shows, for the
    /// "Erik is live on Twitch" button. Needs the Twitch client ID and
    /// secret.
    #[arg(long, env = "TWITCH_LIVE_CHANNEL", default_value = "eerimoq")]
    twitch_live_channel: String,
    /// The App ID (team ID, a period and the bundle ID) of the Moblin app.
    /// Only live posts signed by it, with App Attest, are accepted.
    #[arg(long, env = "APP_ID", default_value = "L82N7LD4N5.com.eerimoq.Mobs")]
    app_id: String,
    /// The App Attest environment the app runs in. Apps installed by Xcode
    /// use the development environment, distributed apps the production one.
    #[arg(long, env = "APP_ATTEST_ENVIRONMENT", value_enum, default_value_t = Environment::Production)]
    app_attest_environment: Environment,
    /// Accept live posts from anyone, not only from the Moblin app, for
    /// developing the website.
    #[arg(long)]
    allow_unattested: bool,
}

fn http_client() -> Client {
    Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION"),
            " (+https://moblin.app)"
        ))
        .timeout(TIMEOUT)
        .build()
        .expect("a client without a proxy or TLS config always builds")
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let store = Arc::new(Store::new(cli.max_streamers));
    let client = http_client();
    let twitch = match (cli.twitch_client_id, cli.twitch_client_secret) {
        (Some(client_id), Some(client_secret)) => Some(Arc::new(Twitch::new(
            client.clone(),
            client_id,
            client_secret,
        ))),
        _ => {
            warn!(
                "no Twitch client ID and secret, not looking Twitch profiles up nor checking who is live"
            );
            None
        }
    };
    let kick = Arc::new(Kick::new(client.clone()));
    let profiles = Profiles::new(
        client,
        twitch.clone(),
        kick.clone(),
        Duration::from_secs(cli.lookup_spacing),
        store.clone(),
    );
    tokio::spawn(async move { profiles.run().await });
    let live_status = LiveStatus::new(
        twitch.clone(),
        kick,
        Duration::from_secs(cli.lookup_spacing),
        store.clone(),
    );
    tokio::spawn(async move { live_status.run().await });
    let app_attest = if cli.allow_unattested {
        warn!("accepting live posts from anyone");
        None
    } else {
        info!(
            "accepting live posts from {} in the {:?} App Attest environment",
            cli.app_id, cli.app_attest_environment
        );
        Some(AppAttest::new(&cli.app_id, cli.app_attest_environment))
    };
    let api = Arc::new(Api {
        store,
        challenges: Challenges::new(),
        app_attest,
        live: twitch.map(|twitch| Live::new(twitch, cli.twitch_live_channel)),
        remote_control: Streamers::new(),
    });
    let listener = tokio::net::TcpListener::bind(cli.listen)
        .await
        .with_context(|| format!("failed to listen on {}", cli.listen))?;
    info!("listening on http://{}", cli.listen);
    axum::serve(listener, api::router(api)).await?;
    Ok(())
}
