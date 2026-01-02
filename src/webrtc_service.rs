use std::sync::Arc;
use async_trait::async_trait;
use webrtc::api::APIBuilder;
use webrtc::api::media_engine::MediaEngine;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use crate::domain::{Router, SignalingService};


// in cpu, as code or text segment
// if I call this without using any arc etc. it is on the STACK.
pub struct WebRTCService {
    // if you use Arc, you say put this into HEAP.
    router: Arc<dyn Router>,
    // it is in STACK
    api: webrtc::api::API,
}

impl WebRTCService {
    // without calling this, in TEXT or CODE segment of CPU.
    pub fn new(router: Arc<dyn Router>) -> Self {
        // it is on the STACK
        let mut m = MediaEngine::default();
        // register the default codecs (H264, VP8, VP9..)
        m.register_default_codecs().expect("Unable to register default codecs");

        // in HEAP
        let api = APIBuilder::new()
            .with_media_engine(m)
            .build();

        Self { router, api }
    }
}

#[async_trait]
impl SignalingService for WebRTCService {
    async fn handle_offer(&self, peer_id: String, offer: RTCSessionDescription) -> anyhow::Result<RTCSessionDescription> {
        // create new webrtc peer connection for this handshake
        let pc = Arc::new(self.api.new_peer_connection(RTCConfiguration::default()).await?);

        let router_clone = Arc::clone(&self.router);

        // broadcaster logic: handle incoming traffics
        pc.on_track(Box::new(move |track, _, _| {
           let router = Arc::clone(&router_clone);

            // we used box::pin because the callback must return a Future that can live
            // as long as the connection. this is basically because of the lib of webrtc that is written by C++.
            // in this part of that code, may use this in the future. so we have to promise that this will live
            // longer enough until it is destroyed (while talking with OS).
            Box::pin(async move {
                router.add_producer(track).await;
            })
        }));

        let consumer_track = self.router.add_consumer(peer_id).await?;
        pc.add_track(Arc::clone(&consumer_track) as Arc<dyn webrtc::track::track_local::TrackLocal + Send + Sync>).await?;

        pc.set_remote_description(offer).await?;
        let answer = pc.create_answer(None).await?;

        let mut gather_complete = pc.gathering_complete_promise().await;
        pc.set_local_description(answer).await?;
        let _ = gather_complete.recv().await;

        let local_desc = pc.local_description().await
            .ok_or_else(|| anyhow::anyhow!("failed to generate local desc"))?;

        Ok(local_desc)
    }
}