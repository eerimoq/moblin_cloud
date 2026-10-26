mod accounts;
mod api;
mod app_attest;
mod auth;
mod challenges;
mod counters;
mod database;
mod deliveries;
mod effects;
mod icons;
mod images;
mod kick;
mod live;
mod live_status;
mod pages;
mod payments;
mod paypal;
mod profiles;
mod remote_control;
mod requests;
mod store;
mod stripe;
mod subscribers;
mod traffic;
mod twitch;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use clap::Parser;
use log::{info, warn};
use reqwest::Client;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};

use crate::accounts::Accounts;
use crate::api::Api;
use crate::app_attest::{AppAttest, Environment};
use crate::auth::{Auth, KickApp, OAuthApp};
use crate::challenges::Challenges;
use crate::counters::Counters;
use crate::database::Database;
use crate::deliveries::Deliveries;
use crate::kick::Kick;
use crate::live::Live;
use crate::live_status::LiveStatus;
use crate::pages::Pages;
use crate::payments::Orders;
use crate::paypal::PayPal;
use crate::profiles::Profiles;
use crate::remote_control::Streamers;
use crate::requests::Requests;
use crate::store::Store;
use crate::stripe::Stripe;
use crate::subscribers::Allowances;
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
    /// Maximum number of listed streamers.
    #[arg(long, env = "MAX_STREAMERS", default_value_t = 6)]
    max_streamers: usize,
    /// Minimum seconds between platform requests.
    #[arg(long, env = "LOOKUP_SPACING", default_value_t = 2)]
    lookup_spacing: u64,
    /// Twitch application client ID.
    #[arg(long, env = "TWITCH_CLIENT_ID", requires = "twitch_client_secret")]
    twitch_client_id: Option<String>,
    /// Twitch application client secret.
    #[arg(
        long,
        env = "TWITCH_CLIENT_SECRET",
        hide_env_values = true,
        requires = "twitch_client_id"
    )]
    twitch_client_secret: Option<String>,
    /// Kick application client ID.
    #[arg(long, env = "KICK_CLIENT_ID", requires = "kick_client_secret")]
    kick_client_id: Option<String>,
    /// Kick application client secret.
    #[arg(
        long,
        env = "KICK_CLIENT_SECRET",
        hide_env_values = true,
        requires = "kick_client_id"
    )]
    kick_client_secret: Option<String>,
    /// Twitch channel whose live status the website shows.
    #[arg(long, env = "TWITCH_LIVE_CHANNEL", default_value = "eerimoq")]
    twitch_live_channel: String,
    /// App ID of the Moblin app, for App Attest.
    #[arg(long, env = "APP_ID", default_value = "L82N7LD4N5.com.eerimoq.Mobs")]
    app_id: String,
    /// App Attest environment.
    #[arg(long, env = "APP_ATTEST_ENVIRONMENT", value_enum, default_value_t = Environment::Production)]
    app_attest_environment: Environment,
    /// SQLite database. In memory without it.
    #[arg(long, env = "DATABASE")]
    database: Option<PathBuf>,
    /// PayPal partner REST app client ID.
    #[arg(
        long,
        env = "PAYPAL_CLIENT_ID",
        requires_all = ["paypal_client_secret", "paypal_partner_id"]
    )]
    paypal_client_id: Option<String>,
    /// PayPal partner REST app client secret.
    #[arg(
        long,
        env = "PAYPAL_CLIENT_SECRET",
        hide_env_values = true,
        requires = "paypal_client_id"
    )]
    paypal_client_secret: Option<String>,
    /// PayPal partner merchant ID.
    #[arg(long, env = "PAYPAL_PARTNER_ID", requires = "paypal_client_id")]
    paypal_partner_id: Option<String>,
    /// PayPal partner BN code.
    #[arg(long, env = "PAYPAL_ATTRIBUTION_ID", requires = "paypal_client_id")]
    paypal_attribution_id: Option<String>,
    /// PayPal webhook ID.
    #[arg(long, env = "PAYPAL_WEBHOOK_ID", requires = "paypal_client_id")]
    paypal_webhook_id: Option<String>,
    /// Use the PayPal sandbox.
    #[arg(long, env = "PAYPAL_SANDBOX")]
    paypal_sandbox: bool,
    /// Base URL of the PayPal API to use instead of PayPal's, for a fake PayPal in tests.
    #[arg(long, value_name = "URL", requires = "paypal_client_id")]
    paypal_url: Option<String>,
    /// Stripe platform secret key. A test mode key takes test payments.
    #[arg(
        long,
        env = "STRIPE_SECRET_KEY",
        hide_env_values = true,
        requires = "stripe_publishable_key"
    )]
    stripe_secret_key: Option<String>,
    /// Stripe platform publishable key.
    #[arg(long, env = "STRIPE_PUBLISHABLE_KEY", requires = "stripe_secret_key")]
    stripe_publishable_key: Option<String>,
    /// Base URL of the Stripe API to use instead of Stripe's, for a fake Stripe in tests.
    #[arg(long, value_name = "URL", requires = "stripe_secret_key")]
    stripe_url: Option<String>,
    /// Session signing secret. Random without it.
    #[arg(long, env = "SESSION_SECRET", hide_env_values = true)]
    session_secret: Option<String>,
    /// URL of this backend.
    #[arg(long, env = "PUBLIC_URL", default_value = "https://api.moblin.app")]
    public_url: String,
    /// URL of the website.
    #[arg(long, env = "FRONTEND_URL", default_value = "https://moblin.app")]
    frontend_url: String,
    /// Accept live posts and Moblin Cloud connections without App Attest, for development.
    #[arg(long)]
    allow_unattested: bool,
    /// Refuse Moblin Cloud connections without a valid App Attest. Without it they are only
    /// counted, for the admin page.
    #[arg(long, env = "REQUIRE_ATTESTED_MOBLIN")]
    require_attested_moblin: bool,
    /// Base URL of the OAuth and Helix endpoints to sign in with instead of Twitch's, for a
    /// fake Twitch in development and tests. No client ID and secret are needed to sign in then.
    #[arg(long, value_name = "URL")]
    twitch_sign_in_url: Option<String>,
    /// Base URL of the OAuth and API endpoints to sign in with instead of Kick's, for a fake
    /// Kick in development and tests. No client ID and secret are needed to sign in then.
    #[arg(long, value_name = "URL")]
    kick_sign_in_url: Option<String>,
    /// Twitch user IDs, or kick:<Kick user ID>, allowed to see the admin page.
    #[arg(long, env = "ADMINS", value_delimiter = ',')]
    admins: Vec<String>,
    /// Twitch user IDs, or kick:<Kick user ID>, of the premium accounts, which get the premium features.
    #[arg(long, env = "PREMIUM_ACCOUNTS", value_delimiter = ',')]
    premium_accounts: Vec<String>,
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
    let twitch_app = match (
        &cli.twitch_client_id,
        &cli.twitch_client_secret,
        &cli.twitch_sign_in_url,
    ) {
        (client_id, client_secret, Some(url)) => {
            warn!("signing in with {url} instead of Twitch");
            Some(OAuthApp::at(
                url,
                url,
                client_id.clone().unwrap_or_default(),
                client_secret.clone().unwrap_or_default(),
            ))
        }
        (Some(client_id), Some(client_secret), None) => {
            Some(OAuthApp::twitch(client_id.clone(), client_secret.clone()))
        }
        _ => {
            warn!("no Twitch client ID and secret, not signing in with Twitch");
            None
        }
    };
    let kick_app = match (
        cli.kick_client_id,
        cli.kick_client_secret,
        &cli.kick_sign_in_url,
    ) {
        (client_id, client_secret, Some(url)) => {
            warn!("signing in with {url} instead of Kick");
            Some(KickApp::at(
                url,
                url,
                client_id.unwrap_or_default(),
                client_secret.unwrap_or_default(),
            ))
        }
        (Some(client_id), Some(client_secret), None) => {
            Some(KickApp::kick(client_id, client_secret))
        }
        _ => {
            warn!("no Kick client ID and secret, not signing in with Kick");
            None
        }
    };
    if cli.session_secret.is_none() {
        warn!("no session secret, signing everyone out on restart");
    }
    let auth = Auth::new(
        http_client(),
        cli.public_url,
        cli.frontend_url,
        twitch_app,
        kick_app,
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
    let remote_control = Streamers::new(Duration::from_secs(60));
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
        warn!("accepting live posts and Moblin Cloud connections from anyone");
        None
    } else {
        info!(
            "accepting live posts from {} in the {:?} App Attest environment",
            cli.app_id, cli.app_attest_environment
        );
        if cli.require_attested_moblin {
            info!("refusing Moblin Cloud connections without App Attest");
        } else {
            info!("counting Moblin Cloud connections without App Attest");
        }
        Some(AppAttest::new(&cli.app_id, cli.app_attest_environment))
    };
    let pages = Pages::new(database.clone());
    let counters = Counters::new(database.clone());
    let deliveries = Deliveries::new(database.clone());
    if cli.admins.is_empty() {
        warn!("no admins, the admin page is unavailable");
    } else {
        info!("{} admins", cli.admins.len());
    }
    let paypal = match (
        cli.paypal_client_id,
        cli.paypal_client_secret,
        cli.paypal_partner_id,
    ) {
        (Some(client_id), Some(client_secret), Some(partner_id)) => {
            let url = match cli.paypal_url {
                Some(url) => {
                    warn!("taking PayPal payments with {url} instead of PayPal");
                    url
                }
                None => {
                    info!(
                        "taking PayPal payments in {} mode",
                        if cli.paypal_sandbox {
                            "sandbox"
                        } else {
                            "live"
                        }
                    );
                    if cli.paypal_sandbox {
                        paypal::SANDBOX_URL
                    } else {
                        paypal::LIVE_URL
                    }
                    .to_string()
                }
            };
            if cli.paypal_webhook_id.is_none() {
                warn!("no PayPal webhook ID, refusing PayPal webhooks");
            }
            Some(PayPal::new(
                http_client(),
                url,
                client_id,
                client_secret,
                partner_id,
                cli.paypal_attribution_id,
                cli.paypal_webhook_id,
            ))
        }
        _ => {
            warn!("no PayPal client ID and secret, PayPal payments are unavailable");
            None
        }
    };
    let stripe = match (cli.stripe_secret_key, cli.stripe_publishable_key) {
        (Some(secret_key), Some(publishable_key)) => {
            let url = match cli.stripe_url {
                Some(url) => {
                    warn!("taking Stripe payments with {url} instead of Stripe");
                    url
                }
                None => stripe::URL.to_string(),
            };
            let stripe = Stripe::new(http_client(), url, secret_key, publishable_key);
            info!(
                "taking Stripe payments in {} mode",
                if stripe.is_test() { "test" } else { "live" }
            );
            Some(stripe)
        }
        _ => {
            warn!("no Stripe keys, Stripe payments are unavailable");
            None
        }
    };
    let api = Arc::new(Api {
        store,
        challenges: Challenges::new(),
        app_attest,
        require_attested_moblin: cli.require_attested_moblin,
        live: twitch.map(|twitch| Live::new(twitch, cli.twitch_live_channel)),
        remote_control,
        pages,
        paypal,
        stripe,
        orders: Orders::new(database.clone()),
        allowances: Allowances::new(database.clone()),
        deliveries,
        auth,
        accounts,
        counters,
        database,
        admins: cli.admins,
        premium_accounts: cli.premium_accounts,
        started: crate::deliveries::now(),
        tests: Mutex::default(),
        requests: Requests::default(),
        stopping: tokio::sync::watch::channel(false).0,
    });
    tokio::spawn({
        let api = api.clone();
        async move { api.deliveries.run(&api.remote_control).await }
    });
    tokio::spawn(api::settle_orders(api.clone()));
    let listener = TcpListener::bind(cli.listen)
        .await
        .with_context(|| format!("failed to listen on {}", cli.listen))?;
    info!("listening on http://{}", cli.listen);
    serve(listener, api::router(api.clone()), async move {
        shutdown_signal().await;
        api.stopping.send_replace(true);
    })
    .await?;
    info!("stopped");
    Ok(())
}

async fn serve(
    listener: TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let mut terminate = match signal(SignalKind::terminate()) {
        Ok(terminate) => terminate,
        Err(error) => {
            warn!("not listening for SIGTERM: {error}");
            std::future::pending().await
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
    info!("shutting down, finishing the requests in flight");
}

#[cfg(test)]
mod tests {
    use axum::routing::get;
    use reqwest::StatusCode;
    use tokio::sync::oneshot;

    use super::*;

    #[tokio::test]
    async fn requests_in_flight_finish_before_stopping() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                "done"
            }),
        );
        let (stop, stopped) = oneshot::channel();
        let server = tokio::spawn(serve(listener, router, async {
            stopped.await.unwrap();
        }));
        let request = tokio::spawn(async move {
            reqwest::Client::new()
                .get(format!("http://{address}/slow"))
                .send()
                .await
                .unwrap()
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        stop.send(()).unwrap();
        let response = request.await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "done");
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            reqwest::Client::new()
                .get(format!("http://{address}/slow"))
                .send()
                .await
                .is_err()
        );
    }
}
