pub mod errors;

pub mod rtmp_cooker;

use streamhub::{
    define::{BroadcastEvent, BroadcastEventReceiver, StreamHubEventSender},
    stream::StreamIdentifier,
};
use crate::remuxer::errors::RtmpRemuxerError;

//Receive publish event from stream hub and
//remux from other protocols to rtmp
pub struct RtmpRemuxer {
    receiver: BroadcastEventReceiver,
    event_producer: StreamHubEventSender,
}

impl RtmpRemuxer {
    pub fn new(receiver: BroadcastEventReceiver, event_producer: StreamHubEventSender) -> Self {
        Self {
            receiver,
            event_producer,
        }
    }
    pub async fn run(&mut self) -> Result<(), RtmpRemuxerError> {
        log::info!("rtmp remuxer start...");
        Ok(())

        // loop {
        //     let val = self.receiver.recv().await?;
        //     log::info!("{:?}", val);
        //     match val {
        //         BroadcastEvent::Publish { identifier } => match identifier {
        //             StreamIdentifier::Rtsp { stream_path } => {
        //                 let mut session =
        //                     Rtsp2RtmpRemuxerSession::new(stream_path, self.event_producer.clone());
        //                 tokio::spawn(async move {
        //                     if let Err(err) = session.run().await {
        //                         log::error!("rtsp2rtmp session error: {}", err);
        //                     }
        //                 });
        //             }
        //             StreamIdentifier::WebRTC {
        //                 app_name,
        //                 stream_name,
        //             } => {
        //                 let mut session = Whip2RtmpRemuxerSession::new(
        //                     app_name,
        //                     stream_name,
        //                     self.event_producer.clone(),
        //                 );
        //                 tokio::spawn(async move {
        //                     if let Err(err) = session.run().await {
        //                         log::error!("whip2rtmp session error: {}", err);
        //                     }
        //                 });
        //             }
        //
        //             _ => {}
        //         },
        //         _ => {
        //             log::trace!("other infos...");
        //         }
        //     }
        // }
    }
}
