use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};
use webrtc::track::track_remote::TrackRemote;
use crate::domain::Router;

pub struct SFURouter {
    // store the producer's peerconnection so we can send PLI requests to it
    producer_pc: RwLock<Option<Arc<RTCPeerConnection>>>,
    consumers: Arc<RwLock<HashMap<String, Arc<TrackLocalStaticRTP>>>>,
}

impl SFURouter {
    pub fn new() -> Self {
        Self {
            producer_pc: RwLock::new(None),
            consumers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Router for SFURouter {
    async fn add_producer(&self, track: Arc<TrackRemote>) {
        let track_id = track.id();
        let kind = track.kind();
        println!("SFU: New producer track detected: {}", track_id);

        let consumers_clone = Arc::clone(&self.consumers);
        tokio::spawn(async move {
            loop {
                // Read incoming RTP packet from the broadcaster
                match track.read_rtp().await {
                    Ok((package, _)) => {
                        let consumers = consumers_clone.read().await;
                        // forward the packets to each viewer currently connected
                        for (id, consumer) in consumers.iter() {
                            if consumer.kind() == kind {
                                if let Err(err) = consumer.write_rtp(&package).await {
                                    tracing::warn!("SFU: Failed to forward to consumer {}: {:?}", id, err);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        tracing::info!("SFU: producer track {} closed or errored: {}", track_id, err);
                        break;
                    }
                }
            }
        });
    }

    async fn add_consumer(&self, peer_id: String) -> anyhow::Result<Arc<TrackLocalStaticRTP>> {
        let local_track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: "video/vp8".to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "rust-webrtc".to_owned(),
        ));

        let mut consumers = self.consumers.write().await;
        consumers.insert(peer_id.clone(), Arc::clone(&local_track));

        if let Some(pc) = &*self.producer_pc.read().await {
           let pli = PictureLossIndication {
               sender_ssrc: 0,
               media_ssrc: 0, // in a simple SFUs, 0 tells the browser to just send a keyframe on all tracks
           };

            let _ = pc.write_rtcp(&[Box::new(pli)]).await;
            tracing::info!("SFU: Sent PLI to producer for consumer {}", peer_id);
        }

        tracing::info!("SFU: New consumer {} registered", peer_id);
        Ok(local_track)
    }

    async fn remove_consumer(&self, peer_id: String) {
        let mut consumers = self.consumers.write().await;
        if consumers.remove(&peer_id).is_some() {
            tracing::info!("SFU: Consumer track {} removed", peer_id);
        }
    }
}