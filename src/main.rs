use std::sync::Arc;
use warp::Filter;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use crate::domain::SignalingService;
use crate::sfu::SFURouter;
use crate::webrtc_service::WebRTCService;

mod domain;
mod sfu;
mod webrtc_service;

#[derive(serde::Deserialize)]
struct NegotiationRequest {
    #[serde(rename = "peerId")]
    peer_id: String,
    offer: RTCSessionDescription,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let router = Arc::new(SFURouter::new());

    let service = Arc::new(WebRTCService::new(router));

    let service_filter = Arc::clone(&service);
    let signaling_route = warp::post()
        .and(warp::path("signal"))
        .and(warp::body::json())
        .and(warp::any().map(move || Arc::clone(&service)))
        .and_then(handle_signaling);

    let static_files = warp::fs::dir("public");
    let routes = signaling_route.or(static_files);

    println!("Serving webrtc on 0.0.0.0:8080");

    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
}

async fn handle_signaling(
    req: NegotiationRequest,
    service: Arc<WebRTCService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    tracing::info!("Main: received offer from peer {}", req.peer_id);

    match service.handle_offer(req.peer_id, req.offer).await {
        Ok(answer) => {
            tracing::info!("Main: successfully generated answer");
            Ok(warp::reply::json(&answer))
        },
        Err(e) => {
            tracing::error!("Main: signaling error: {}", e);
            Err(warp::reject::custom(SignalingError))
        }
    }
}

#[derive(Debug)]
struct SignalingError;
impl warp::reject::Reject for SignalingError {}