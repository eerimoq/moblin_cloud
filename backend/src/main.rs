mod accounts;
mod api;
mod app_attest;
mod auth;
mod challenges;
mod database;
mod deliveries;
mod kick;
mod live;
mod live_status;
mod pages;
mod paypal;
mod profiles;
mod remote_control;
mod store;
mod twitch;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use log::{info, warn};
use reqwest::Client;

use crate::accounts::Accounts;
use crate::api::Api;
use crate::app_attest::{AppAttest, Environment};
use crate::auth::{Auth, OAuthApp};
use crate::challenges::Challenges;
use crate::database::Database;
use crate::deliveries::Deliveries;
use crate::kick::Kick;
use crate::live::Live;
use crate::live_status::LiveStatus;
use crate::pages::Pages;
use crate::paypal::PayPal;
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
    /// The SQLite database the list of streamers, streamer accounts, their
    /// page settings (song requests, tips, the wheel and the look of the
    /// page) and requested songs, tips and wheel spins not yet delivered to
    /// Moblin are saved in.
    /// Without one they are forgotten when the backend restarts.
    #[arg(long, env = "DATABASE")]
    database: Option<PathBuf>,
    /// The client ID of a PayPal REST app of a PayPal partner, used together
    /// with its secret to let streamers connect their PayPal account and ask
    /// viewers to pay for songs, tips and wheel spins. The money goes straight
    /// to the streamer's PayPal account. Without one payments are
    /// unavailable.
    #[arg(
        long,
        env = "PAYPAL_CLIENT_ID",
        requires_all = ["paypal_client_secret", "paypal_partner_id"]
    )]
    paypal_client_id: Option<String>,
    /// The client secret of the PayPal REST app.
    #[arg(
        long,
        env = "PAYPAL_CLIENT_SECRET",
        hide_env_values = true,
        requires = "paypal_client_id"
    )]
    paypal_client_secret: Option<String>,
    /// The PayPal merchant ID of the partner account the REST app belongs to,
    /// for looking up streamers who connected their PayPal account.
    #[arg(long, env = "PAYPAL_PARTNER_ID", requires = "paypal_client_id")]
    paypal_partner_id: Option<String>,
    /// The BN code PayPal gave the partner, sent with every PayPal request.
    #[arg(long, env = "PAYPAL_ATTRIBUTION_ID", requires = "paypal_client_id")]
    paypal_attribution_id: Option<String>,
    /// The ID of the webhook registered for `<public url>/paypal/webhooks` in
    /// the PayPal REST app, subscribed to MERCHANT.ONBOARDING.COMPLETED and
    /// MERCHANT.PARTNER-CONSENT.REVOKED. Without one PayPal webhooks are
    /// refused.
    #[arg(long, env = "PAYPAL_WEBHOOK_ID", requires = "paypal_client_id")]
    paypal_webhook_id: Option<String>,
    /// Use the PayPal sandbox instead of real payments.
    #[arg(long, env = "PAYPAL_SANDBOX")]
    paypal_sandbox: bool,
    /// The secret sessions of signed in streamers are signed with. Without
    /// one a random secret is used and everyone is signed out when the
    /// backend restarts.
    #[arg(long, env = "SESSION_SECRET", hide_env_values = true)]
    session_secret: Option<String>,
    /// The URL this backend is reached at, for the sign in redirect URIs
    /// registered with Twitch.
    #[arg(long, env = "PUBLIC_URL", default_value = "https://api.moblin.app")]
    public_url: String,
    /// The URL of the website, where streamers are sent after signing in.
    #[arg(long, env = "FRONTEND_URL", default_value = "https://moblin.app")]
    frontend_url: String,
    /// Accept live posts from anyone, not only from the Moblin app, for
    /// developing the website.
    #[arg(long)]
    allow_unattested: bool,
    /// Sign in as a made-up Twitch user without asking Twitch, for
    /// developing the website. The login is `streamer`, or the `login`
    /// query parameter of the sign in URL.
    #[arg(long)]
    fake_twitch_sign_in: bool,
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
    let client = http_client();
    let twitch_app = match (&cli.twitch_client_id, &cli.twitch_client_secret) {
        (Some(client_id), Some(client_secret)) => {
            Some(OAuthApp::twitch(client_id.clone(), client_secret.clone()))
        }
        _ => None,
    };
    if cli.session_secret.is_none() {
        warn!("no session secret, signing everyone out on restart");
    }
    if cli.fake_twitch_sign_in {
        warn!("signing anyone in as any Twitch user without asking Twitch");
    }
    let auth = Auth::new(
        http_client(),
        cli.public_url,
        cli.frontend_url,
        twitch_app,
        cli.fake_twitch_sign_in,
        cli.session_secret.as_deref(),
    )?;
    if cli.database.is_none() {
        warn!(
            "no database, forgetting streamers, accounts, page settings and undelivered requests on restart"
        );
    }
    let database = Database::open(cli.database.as_deref())?;
    let store = Arc::new(Store::new(cli.max_streamers, database.clone()));
    let accounts = Accounts::new(database.clone());
    let twitch = match (cli.twitch_client_id, cli.twitch_client_secret) {
        (Some(client_id), Some(client_secret)) => Some(Arc::new(Twitch::new(
            client.clone(),
            client_id,
            client_secret,
        ))),
        _ => {
            warn!(
                "no Twitch client ID and secret, not looking Twitch profiles up, checking who is live nor signing in with Twitch"
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
    let pages = Pages::new(database.clone());
    let deliveries = Deliveries::new(database);
    let paypal = match (
        cli.paypal_client_id,
        cli.paypal_client_secret,
        cli.paypal_partner_id,
    ) {
        (Some(client_id), Some(client_secret), Some(partner_id)) => {
            let url = if cli.paypal_sandbox {
                paypal::SANDBOX_URL
            } else {
                paypal::LIVE_URL
            };
            info!("taking payments through {url}");
            if cli.paypal_webhook_id.is_none() {
                warn!("no PayPal webhook ID, refusing PayPal webhooks");
            }
            Some(PayPal::new(
                http_client(),
                url.to_string(),
                client_id,
                client_secret,
                partner_id,
                cli.paypal_attribution_id,
                cli.paypal_webhook_id,
            ))
        }
        _ => {
            warn!("no PayPal client ID and secret, payments are unavailable");
            None
        }
    };
    let api = Arc::new(Api {
        store,
        challenges: Challenges::new(),
        app_attest,
        live: twitch.map(|twitch| Live::new(twitch, cli.twitch_live_channel)),
        remote_control: Streamers::new(),
        pages,
        paypal,
        deliveries,
        auth,
        accounts,
    });
    tokio::spawn({
        let api = api.clone();
        async move { api.deliveries.run(&api.remote_control).await }
    });
    let listener = tokio::net::TcpListener::bind(cli.listen)
        .await
        .with_context(|| format!("failed to listen on {}", cli.listen))?;
    info!("listening on http://{}", cli.listen);
    axum::serve(listener, api::router(api)).await?;
    Ok(())
}
