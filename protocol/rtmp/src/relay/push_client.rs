use streamhub::stream::Protocol;
use {
    super::errors::ClientError,
    crate::session::client_session::{ClientSession, ClientType},
    streamhub::{
        define::{StreamHubEventSender, BroadcastEvent, BroadcastEventReceiver},
    },
    tokio::net::TcpStream,
};

pub struct PushClient {
    address: String,
    client_event_consumer: BroadcastEventReceiver,
    channel_event_producer: StreamHubEventSender,
}

impl PushClient {
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
        log::info!("push client run...");

        loop {
            let val = self.client_event_consumer.recv().await?;

            match val {
                BroadcastEvent::Publish { protocol: Protocol::Rtmp, name } => {
                    let (app_name, stream_name) = {
                        let mut iter = name.split('/');
                        let app_name = iter.next().unwrap_or_default();
                        let stream_name = iter.next().unwrap_or_default();
                        (app_name.to_string(), stream_name.to_string())
                    };
                    {
                        log::info!(
                            "publish app_name: {} stream_name: {} address: {}",
                            app_name.clone(),
                            stream_name.clone(),
                            self.address.clone()
                        );
                        let stream = TcpStream::connect(self.address.clone()).await?;

                        let mut client_session = ClientSession::new(
                            stream,
                            ClientType::Publish,
                            self.address.clone(),
                            app_name,
                            stream_name,
                            self.channel_event_producer.clone(),
                            0,
                        );

                        tokio::spawn(async move {
                            if let Err(err) = client_session.run().await {
                                log::error!("client_session as push client run error: {}", err);
                            }
                        });
                    }
                }

                _ => {
                    log::info!("push client receive other events");
                }
            }
        }
    }
}
