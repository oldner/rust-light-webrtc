use std::sync::Arc;
use async_trait::async_trait;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_remote::TrackRemote;

#[async_trait]
pub trait Router: Send + Sync {
    async fn add_producer(&self, track: Arc<TrackRemote>);
    async fn add_consumer(&self, peer_id: String) -> anyhow::Result<Arc<TrackLocalStaticRTP>>;
    async fn remove_consumer(&self, peer_id: String);
}

#[async_trait]
pub trait SignalingService: Send + Sync {
    async fn handle_offer(
        &self,
        peer_id: String,
        offer: RTCSessionDescription,
    ) -> anyhow::Result<RTCSessionDescription>;
}

