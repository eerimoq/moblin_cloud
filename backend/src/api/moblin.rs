use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Response;
use log::info;

use super::*;
use crate::remote_control::Socket;

pub(super) async fn handle_remote_control_streamer(
    State(api): State<Arc<Api>>,
    Path(moblin_cloud_key): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let Some(page_id) = api.accounts.page_of_moblin_cloud_key(&moblin_cloud_key) else {
        return Err(StatusCode::NOT_FOUND);
    };
    Ok(upgrade.on_upgrade(move |socket| async move {
        let mut socket = Socket::new(socket);
        let identity = match api.remote_control.identify(&mut socket).await {
            Ok(identity) => identity,
            Err(error) => {
                info!("remote control streamer {page_id} failed to identify: {error:#}");
                return;
            }
        };
        let Some(device) = identity.device else {
            api.remote_control.refuse(socket).await;
            return;
        };
        if let Some(app_attest) = &api.app_attest {
            let attested = match &identity.app_attest {
                Some(attestation) => app_attest
                    .verify(attestation, &identity.challenge)
                    .map_err(|error| error.context("invalid app attest")),
                None => Err(anyhow!("no app attest in identify")),
            };
            if let Err(error) = attested {
                info!(
                    "remote control streamer {page_id} device {device} is not attested: {error:#}"
                );
                if api.require_attested_moblin {
                    api.remote_control.refuse(socket).await;
                    return;
                }
            }
        }
        let Some(connection) = api
            .remote_control
            .connect(&page_id, &device, socket.traffic())
        else {
            info!("remote control streamer {page_id} device {device} rejected, too many devices");
            api.remote_control.refuse(socket).await;
            return;
        };
        api.accounts.seen_device(&page_id, &device);
        api.deliveries.notify();
        api.remote_control
            .serve(connection, socket, |icon| {
                api.pages.set_icon(&page_id, icon.id)
            })
            .await;
        api.accounts.left_device(&page_id, &device);
        api.remote_control.changes.notify(&page_id, Change::Page);
    }))
}

#[cfg(test)]
#[path = "tests/moblin.rs"]
mod tests;
