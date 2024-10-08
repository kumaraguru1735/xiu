use streamhub::stream::Protocol;
use {
    super::errors::ClientError,
    crate::session::client_session::{ClientSession, ClientType},
    streamhub::define::{BroadcastEvent, BroadcastEventReceiver, StreamHubEventSender},
    tokio::net::TcpStream,
};

pub struct PullClient {
    address: String,
    client_event_consumer: BroadcastEventReceiver,
    channel_event_producer: StreamHubEventSender,
}

impl PullClient {
    pub fn new(
        address: String,
        consumer: BroadcastEventReceiver,
        producer: StreamHubEventSender,
    ) -> Self {
        Self {
            address,

            client_event_consumer: consumer,
            channel_event_producer: producer,
        }
    }

    pub async fn run(&mut self) -> Result<(), ClientError> {
        loop {
            let event = self.client_event_consumer.recv().await?;

            if let BroadcastEvent::Subscribe { protocol: Protocol::Rtmp, name } = event {
                let (app_name, stream_name) = {
                    let mut iter = name.split('/');
                    let app_name = iter.next().unwrap_or_default();
                    let stream_name = iter.next().unwrap_or_default();
                    (app_name.to_string(), stream_name.to_string())
                };

                log::info!(
                    "receive pull event, app_name :{}, stream_name: {}",
                    app_name,
                    stream_name
                );
                let stream = TcpStream::connect(self.address.clone()).await?;

                let mut client_session = ClientSession::new(
                    stream,
                    ClientType::Play,
                    self.address.clone(),
                    app_name.clone(),
                    stream_name.clone(),
                    self.channel_event_producer.clone(),
                    0,
                );

                tokio::spawn(async move {
                    if let Err(err) = client_session.run().await {
                        log::error!("client_session as pull client run error: {}", err);
                    }
                });
            }

            // if let BroadcastEvent::Subscribe {
            //     protocol: Protocol::Rtmp,
            //     name: format!("{}/{}", app_name, stream_name),
            // } = event
            // {
            //     log::info!(
            //         "receive pull event, app_name :{}, stream_name: {}",
            //         app_name,
            //         stream_name
            //     );
            //     let stream = TcpStream::connect(self.address.clone()).await?;
            //
            //     let mut client_session = ClientSession::new(
            //         stream,
            //         ClientType::Play,
            //         self.address.clone(),
            //         app_name.clone(),
            //         stream_name.clone(),
            //         self.channel_event_producer.clone(),
            //         0,
            //     );
            //
            //     tokio::spawn(async move {
            //         if let Err(err) = client_session.run().await {
            //             log::error!("client_session as pull client run error: {}", err);
            //         }
            //     });
            // }
        }
    }
}
