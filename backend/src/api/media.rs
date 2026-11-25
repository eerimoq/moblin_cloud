use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::accounts::Platform;
use crate::clips;
use crate::deliveries;
use crate::payments::{Item, MediaPlatform, Order, Payee};
use crate::remote_control::MediaShareState;
use crate::youtube::{self, LookupError};

use super::*;

#[derive(Deserialize)]
pub(super) struct VideoRequest {
    url: String,
    #[serde(default)]
    start: Option<u32>,
    #[serde(default)]
    seconds: Option<u32>,
}

impl VideoRequest {
    pub(super) fn of(url: String) -> Self {
        Self {
            url,
            start: None,
            seconds: None,
        }
    }
}

pub(super) struct Video {
    pub(super) platform: MediaPlatform,
    pub(super) video: String,
    pub(super) start: u32,
    pub(super) seconds: u32,
    pub(super) title: String,
    pub(super) link: Option<String>,
    pub(super) thumbnail: Option<String>,
}

impl Video {
    pub(super) fn item(self, message: Option<String>) -> Item {
        Item::Media {
            video: self.video,
            start: self.start,
            seconds: self.seconds,
            title: self.title,
            message,
            platform: self.platform,
            link: self.link,
            thumbnail: self.thumbnail,
        }
    }

    pub(super) fn delivery(self, user: String, message: String) -> deliveries::Item {
        deliveries::Item::Media {
            user,
            message,
            video: self.video,
            start: self.start,
            seconds: self.seconds,
            title: self.title,
            platform: self.platform,
            link: self.link,
            thumbnail: self.thumbnail,
        }
    }
}

pub(super) fn invalid_video() -> Failure {
    failure(
        "invalid-video",
        StatusCode::BAD_REQUEST,
        "That is not a link to a YouTube video or a Twitch clip.",
    )
}

pub(super) async fn video(
    api: &Api,
    longest: u32,
    request: &VideoRequest,
) -> Result<Video, Failure> {
    if let Some(video) = youtube::video_id(&request.url) {
        return youtube_video(api, longest, request, video).await;
    }
    let slug = clips::twitch_slug(&request.url).ok_or_else(invalid_video)?;
    clip(api, longest, slug).await
}

async fn youtube_video(
    api: &Api,
    longest: u32,
    request: &VideoRequest,
    video: String,
) -> Result<Video, Failure> {
    let seconds = request.seconds.unwrap_or(0);
    if !(1..=longest).contains(&seconds) {
        return Err(failure(
            "invalid-option",
            StatusCode::BAD_REQUEST,
            "The streamer changed how long videos can be. Reload the page.",
        ));
    }
    let start = request
        .start
        .or_else(|| youtube::start(&request.url))
        .unwrap_or(0);
    if start > 24 * 60 * 60 {
        return Err(failure(
            "invalid-video",
            StatusCode::BAD_REQUEST,
            "The start time is too late.",
        ));
    }
    let title = api
        .youtube
        .title(&video)
        .await
        .map_err(|error| match error {
            LookupError::Unavailable => failure(
                "video-unavailable",
                StatusCode::BAD_REQUEST,
                "That video does not exist or can't be played outside YouTube. Pick another one.",
            ),
            LookupError::Unknown => failure(
                "youtube",
                StatusCode::BAD_GATEWAY,
                "YouTube did not answer. Try again.",
            ),
        })?;
    Ok(Video {
        platform: MediaPlatform::YouTube,
        video,
        start,
        seconds,
        title,
        link: None,
        thumbnail: None,
    })
}

async fn clip(api: &Api, longest: u32, slug: String) -> Result<Video, Failure> {
    if !api.auth.has(Platform::Twitch) {
        return Err(failure(
            "clips-unavailable",
            StatusCode::BAD_REQUEST,
            "Twitch clips can't be played here. Pick a YouTube video.",
        ));
    }
    let clip = api
        .auth
        .twitch_clip(&slug)
        .await
        .map_err(|error| {
            warn!("looking up Twitch clip {slug} failed: {error:#}");
            failure(
                "clip-lookup",
                StatusCode::BAD_GATEWAY,
                "Twitch did not answer. Try again.",
            )
        })?
        .ok_or(failure(
            "video-unavailable",
            StatusCode::BAD_REQUEST,
            "That clip does not exist. Pick another one.",
        ))?;
    if clip.seconds > longest {
        return Err(failure(
            "clip-too-long",
            StatusCode::BAD_REQUEST,
            &format!(
                "That clip is {} seconds, and the streamer plays at most {longest}. Pick a shorter one.",
                clip.seconds
            ),
        ));
    }
    Ok(Video {
        platform: MediaPlatform::Twitch,
        video: slug,
        start: 0,
        seconds: clip.seconds.max(1),
        title: clip.title,
        link: Some(clip.link),
        thumbnail: clip
            .thumbnail
            .filter(|thumbnail| clips::is_https(thumbnail)),
    })
}

pub(super) fn media_available(api: &Api, id: &str) -> Result<(), Failure> {
    if api.remote_control.media_device(id).is_none() {
        return Err(if api.remote_control.is_connected(id) {
            failure(
                "no-media",
                StatusCode::CONFLICT,
                "The streamer is not taking videos right now.",
            )
        } else {
            failure(
                "offline",
                StatusCode::NOT_FOUND,
                "The streamer is not taking videos right now. Try again when they are live.",
            )
        });
    }
    Ok(())
}

pub(super) fn held(api: &Api, order: &Order) -> bool {
    matches!(order.item, Item::Media { .. })
        && api.pages.get(&order.streamer).media_rules().moderated
}

#[derive(Deserialize)]
pub(super) struct MediaRequest {
    #[serde(flatten)]
    video: VideoRequest,
    #[serde(default)]
    price: Option<u32>,
    #[serde(flatten)]
    paid: PaidRequest,
}

pub(super) async fn handle_media_order(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    Json(request): Json<MediaRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let order = media_order(&api, page_id, request).await;
    place_order(&api, "media", order).await
}

async fn media_order(api: &Api, id: String, request: MediaRequest) -> Result<Order, Failure> {
    let (account, settings) = page_settings(api, &id);
    let media = settings.media().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer turned videos off. Reload the page.",
    ))?;
    media_available(api, &id)?;
    let payee = request.paid.payee(api, account.as_ref())?;
    if request.price.is_some_and(|price| price != media.price) {
        return Err(failure(
            "invalid-option",
            StatusCode::BAD_REQUEST,
            "The streamer changed the price. Reload the page.",
        ));
    }
    let video = video(api, settings.media_rules().seconds, &request.video).await?;
    request.paid.order(
        id,
        |message| video.item(message),
        payee,
        media.price,
        settings.currency(),
    )
}

#[derive(Serialize)]
pub(super) struct QueuedMedia {
    id: String,
    at: u64,
    user: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    message: String,
    video: String,
    start: u32,
    seconds: u32,
    title: String,
    platform: MediaPlatform,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<u8>,
    #[serde(rename = "kickSubscriber", skip_serializing_if = "std::ops::Not::not")]
    kick_subscriber: bool,
}

#[derive(Serialize)]
pub(super) struct MediaQueueResponse {
    device: Option<String>,
    paused: bool,
    playing: Option<QueuedMedia>,
    queued: Vec<QueuedMedia>,
    held: Vec<QueuedMedia>,
}

fn queued_media(delivery: &deliveries::Delivery, order: Option<&Order>) -> Option<QueuedMedia> {
    let deliveries::Item::Media {
        user,
        message,
        video,
        start,
        seconds,
        title,
        platform,
        link,
        thumbnail,
        ..
    } = &delivery.item
    else {
        return None;
    };
    let paid = order.filter(|order| matches!(order.payee, Payee::PayPal(_) | Payee::Stripe(_)));
    Some(QueuedMedia {
        id: delivery.request_id.clone(),
        at: delivery.requested_at,
        user: user.clone(),
        message: message.clone(),
        video: video.clone(),
        start: *start,
        seconds: *seconds,
        title: title.clone(),
        platform: *platform,
        link: link.clone(),
        thumbnail: thumbnail.clone(),
        amount: paid.map(|order| order.price),
        currency: paid.map(|order| order.currency.clone()),
        tier: order.and_then(|order| match order.payee {
            Payee::Twitch(tier) => Some(tier),
            _ => None,
        }),
        kick_subscriber: order.is_some_and(|order| order.payee == Payee::Kick),
    })
}

fn playing_media(api: &Api, page_id: &str, moblin_id: &str) -> Option<QueuedMedia> {
    let candidates: Vec<String> = [
        Some(moblin_id.to_string()),
        moblin_id.rsplit_once('-').map(|(id, _)| id.to_string()),
    ]
    .into_iter()
    .flatten()
    .collect();
    let deliveries = api.deliveries.get_many(&candidates);
    deliveries
        .get(moblin_id)
        .or_else(|| deliveries.get(moblin_id.rsplit_once('-')?.0))
        .filter(|delivery| delivery.streamer == page_id)
        .and_then(|delivery| queued_media(delivery, None))
}

fn media_queue(api: &Api, page_id: &str) -> MediaQueueResponse {
    let held = api
        .deliveries
        .held(page_id)
        .iter()
        .filter_map(|delivery| {
            let order = api
                .orders
                .get(&delivery.request_id)
                .map(|saved| saved.order);
            queued_media(delivery, order.as_ref())
        })
        .collect();
    let (device, state) = api.remote_control.media(page_id).unzip();
    let playing = match state {
        Some(MediaShareState::Playing { request_id }) => playing_media(api, page_id, &request_id),
        _ => None,
    };
    MediaQueueResponse {
        device,
        paused: api.remote_control.media_paused(page_id),
        playing,
        queued: api
            .deliveries
            .queued_media(page_id)
            .iter()
            .filter_map(|delivery| queued_media(delivery, None))
            .collect(),
        held,
    }
}

pub(super) async fn handle_media_queue(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<MediaQueueResponse>, Failure> {
    assistant(&api, &headers, &page_id)?;
    Ok(Json(media_queue(&api, &page_id)))
}

#[derive(Deserialize)]
pub(super) struct MediaPauseRequest {
    on: bool,
}

pub(super) async fn handle_media_pause(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<MediaPauseRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let user = watcher(&api, &headers, &page_id)?;
    if user.rights.remote_control < Access::Control {
        return Err(failure(
            "no-control",
            StatusCode::FORBIDDEN,
            "The streamer has not let you control Moblin.",
        ));
    }
    api.remote_control.set_media_paused(&page_id, request.on);
    Ok(accepted())
}

fn not_held() -> Failure {
    failure(
        "not-found",
        StatusCode::NOT_FOUND,
        "This video is no longer waiting for approval.",
    )
}

pub(super) async fn handle_media_approve(
    State(api): State<Arc<Api>>,
    Path((page_id, request_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let user = assistant(&api, &headers, &page_id)?;
    let result = match api.deliveries.approve(&page_id, &request_id) {
        Ok(true) => Ok(accepted()),
        Ok(false) => Err(not_held()),
        Err(error) => Err(not_saved(error.into())),
    };
    if result.is_ok() {
        info!(
            "{} approved video request {request_id} of page {page_id}",
            user.signed_in.account.id
        );
        api.remote_control.changes.notify(&page_id, Change::Recent);
        api.remote_control.changes.notify(&page_id, Change::Media);
    }
    counted(&api, "mediaApprovals", result)
}

pub(super) async fn handle_media_reject(
    State(api): State<Arc<Api>>,
    Path((page_id, request_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let user = assistant(&api, &headers, &page_id)?;
    let rejected = tokio::spawn({
        let api = api.clone();
        let page_id = page_id.clone();
        let request_id = request_id.clone();
        async move { reject(&api, &page_id, &request_id).await }
    });
    let result = rejected.await.unwrap_or_else(|_| {
        Err(failure(
            "crashed",
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong. Try again.",
        ))
    });
    if result.is_ok() {
        info!(
            "{} rejected video request {request_id} of page {page_id}",
            user.signed_in.account.id
        );
    }
    counted(&api, "mediaRejections", result.map(|()| accepted()))
}

async fn reject(api: &Api, page_id: &str, request_id: &str) -> Result<(), Failure> {
    if !api
        .deliveries
        .held(page_id)
        .iter()
        .any(|delivery| delivery.request_id == request_id)
    {
        return Err(not_held());
    }
    if let Some(saved) = api.orders.get(request_id) {
        refund_rejected(api, request_id, &saved.order).await?;
    }
    api.deliveries
        .reject(page_id, request_id)
        .map_err(|error| not_saved(error.into()))?;
    api.remote_control.changes.notify(page_id, Change::Recent);
    api.remote_control.changes.notify(page_id, Change::Media);
    Ok(())
}

#[cfg(test)]
#[path = "tests/media.rs"]
mod tests;
