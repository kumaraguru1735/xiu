use crate::stream::Protocol;
use define::{
    FrameDataReceiver, PacketDataReceiver, PacketDataSender, StatisticData, StatisticDataReceiver,
    StatisticDataSender,
};
use serde_json::{json, Value};
use statistics::{StatisticSubscriber, StatisticsStream};
use tokio::sync::oneshot;
use xflv::define::aac_packet_type;

use crate::define::PacketData;

pub mod define;
pub mod errors;
pub mod notify;
pub mod statistics;
pub mod stream;
pub mod utils;

use {
    crate::notify::Notifier,
    define::{
        BroadcastEvent, BroadcastEventReceiver, BroadcastEventSender, DataReceiver, DataSender,
        FrameData, FrameDataSender, Information, StreamHubEvent, StreamHubEventReceiver,
        StreamHubEventSender, SubscriberInfo, TStreamHandler, TransceiverEvent,
        TransceiverEventReceiver, TransceiverEventSender,
    },
    errors::{StreamHubError, StreamHubErrorValue},
    std::collections::HashMap,
    std::sync::Arc,
    tokio::sync::{broadcast, mpsc, mpsc::UnboundedReceiver, Mutex},
    utils::Uuid,
};

//Receive audio data/video data/meta data/media info from a publisher and send to players/subscribers
//Receive statistic information from a publisher and send to api callers.
pub struct StreamDataTransceiver {
    //used for receiving Audio/Video data from publishers
    data_receiver: DataReceiver,
    //used for receiving event
    event_receiver: TransceiverEventReceiver,
    //used for sending audio/video frame data to players/subscribers
    id_to_frame_sender: Arc<Mutex<HashMap<Uuid, FrameDataSender>>>,
    //used for sending audio/video packet data to players/subscribers
    id_to_packet_sender: Arc<Mutex<HashMap<Uuid, PacketDataSender>>>,
    //publisher and subscribers use this sender to submit statistical data
    statistic_data_sender: StatisticDataSender,
    //used for receiving statistical data from publishers and subscribers
    statistic_data_receiver: StatisticDataReceiver,
    //The publisher and subscribers's statistics data of a stream need to be aggregated and sent to the caller as needed.
    statistic_data: Arc<Mutex<StatisticsStream>>,
    //a hander implement by protocols, such as rtmp, webrtc, http-flv, hls
    stream_handler: Arc<dyn TStreamHandler>,
}

impl StreamDataTransceiver {
    fn new(
        protocol: Protocol,
        name: String,
        data_receiver: DataReceiver,
        event_receiver: UnboundedReceiver<TransceiverEvent>,
        h: Arc<dyn TStreamHandler>,
    ) -> Self {
        let (statistic_data_sender, statistic_data_receiver) = mpsc::unbounded_channel();
        Self {
            data_receiver,
            event_receiver,
            statistic_data_sender,
            statistic_data_receiver,
            id_to_frame_sender: Arc::new(Mutex::new(HashMap::new())),
            id_to_packet_sender: Arc::new(Mutex::new(HashMap::new())),
            stream_handler: h,
            statistic_data: Arc::new(Mutex::new(StatisticsStream::new(protocol, name))),
        }
    }

    async fn receive_frame_data(
        data: Option<FrameData>,
        frame_senders: &Arc<Mutex<HashMap<Uuid, FrameDataSender>>>,
    ) {
        if let Some(val) = data {
            match val {
                FrameData::MetaData {
                    timestamp: _,
                    data: _,
                } => {}
                FrameData::Audio { timestamp, data } => {
                    let data = FrameData::Audio {
                        timestamp,
                        data: data.clone(),
                    };

                    for (_, v) in frame_senders.lock().await.iter() {
                        if let Err(audio_err) = v.send(data.clone()).map_err(|_| StreamHubError {
                            value: StreamHubErrorValue::SendAudioError,
                        }) {
                            log::error!("Transmiter send error: {}", audio_err);
                        }
                    }
                }
                FrameData::Video { timestamp, data } => {
                    let data = FrameData::Video {
                        timestamp,
                        data: data.clone(),
                    };
                    for (_, v) in frame_senders.lock().await.iter() {
                        if let Err(video_err) = v.send(data.clone()).map_err(|_| StreamHubError {
                            value: StreamHubErrorValue::SendVideoError,
                        }) {
                            log::error!("Transmiter send error: {}", video_err);
                        }
                    }
                }
                FrameData::MediaInfo {
                    media_info: info_value,
                } => {
                    let data = FrameData::MediaInfo {
                        media_info: info_value,
                    };
                    for (_, v) in frame_senders.lock().await.iter() {
                        if let Err(media_err) = v.send(data.clone()).map_err(|_| StreamHubError {
                            value: StreamHubErrorValue::SendVideoError,
                        }) {
                            log::error!("Transmiter send error: {}", media_err);
                        }
                    }
                }
            }
        }
    }

    async fn receive_frame_data_loop(
        mut exit: broadcast::Receiver<()>,
        mut receiver: FrameDataReceiver,
        frame_senders: Arc<Mutex<HashMap<Uuid, FrameDataSender>>>,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    data = receiver.recv() => {
                       Self::receive_frame_data(data, &frame_senders).await;
                    }
                    _ = exit.recv()=>{
                        break;
                    }
                }
            }
        });
    }

    async fn receive_packet_data(
        data: Option<PacketData>,
        packet_senders: &Arc<Mutex<HashMap<Uuid, PacketDataSender>>>,
    ) {
        if let Some(val) = data {
            match val {
                PacketData::Audio { timestamp, data } => {
                    let data = PacketData::Audio {
                        timestamp,
                        data: data.clone(),
                    };

                    for (_, v) in packet_senders.lock().await.iter() {
                        if let Err(audio_err) = v.send(data.clone()).map_err(|_| StreamHubError {
                            value: StreamHubErrorValue::SendAudioError,
                        }) {
                            log::error!("Transmiter send error: {}", audio_err);
                        }
                    }
                }
                PacketData::Video { timestamp, data } => {
                    let data = PacketData::Video {
                        timestamp,
                        data: data.clone(),
                    };
                    for (_, v) in packet_senders.lock().await.iter() {
                        if let Err(video_err) = v.send(data.clone()).map_err(|_| StreamHubError {
                            value: StreamHubErrorValue::SendVideoError,
                        }) {
                            log::error!("Transmiter send error: {}", video_err);
                        }
                    }
                }
            }
        }
    }

    async fn receive_packet_data_loop(
        mut exit: broadcast::Receiver<()>,
        mut receiver: PacketDataReceiver,
        packet_senders: Arc<Mutex<HashMap<Uuid, PacketDataSender>>>,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    data = receiver.recv() => {
                       Self::receive_packet_data(data, &packet_senders).await;
                    }
                    _ = exit.recv()=>{
                        break;
                    }
                }
            }
        });
    }

    async fn receive_statistics_data(
        data: Option<StatisticData>,
        statistics_data: &Arc<Mutex<StatisticsStream>>,
    ) {
        if let Some(val) = data {
            match val {
                StatisticData::Audio {
                    uuid,
                    data_size,
                    aac_packet_type,
                    duration: _,
                } => {
                    if let Some(uid) = uuid {
                        let mut statistics_data = statistics_data.lock().await; // Assuming `lock` method for Mutex is defined.

                        // Find and update the subscriber
                        if let Some(sub) = statistics_data.subscribers.iter_mut().find(|sub| sub.id == uid) {
                            sub.send_bytes += data_size;
                        }

                        statistics_data.total_send_bytes += data_size;
                    } else {
                        match aac_packet_type {
                            aac_packet_type::AAC_RAW => {
                                let audio_data = &mut statistics_data.lock().await.publisher.tracks.audio;
                                audio_data.recv_bytes += data_size;
                            }
                            aac_packet_type::AAC_SEQHDR => {}
                            _ => {}
                        }
                        statistics_data.lock().await.total_recv_bytes += data_size;
                    }
                }
                StatisticData::Video {
                    uuid,
                    data_size,
                    frame_count,
                    is_key_frame,
                    duration: _,
                } => {
                    //if it is a subscriber, we need to update the send_bytes
                    if let Some(uid) = uuid {

                        let mut statistics_data = statistics_data.lock().await;

                        // Find and update the subscriber
                        if let Some(sub) = statistics_data.subscribers.iter_mut().find(|sub| sub.id == uid) {
                            sub.send_bytes += data_size;
                            sub.total_send_bytes += data_size;
                        }

                        statistics_data.total_send_bytes += data_size;
                    }
                    //if it is a publisher, we need to update the recv_bytes
                    else {
                        let stat_data = &mut statistics_data.lock().await;
                        stat_data.total_recv_bytes += data_size;
                        stat_data.publisher.tracks.video.recv_bytes += data_size;
                        stat_data.publisher.tracks.video.recv_frame_count += frame_count;
                        stat_data.publisher.recv_bytes += data_size;
                        if let Some(is_key) = is_key_frame {
                            if is_key {
                                stat_data.publisher.tracks.video.gop =
                                    stat_data.publisher.tracks.video.recv_frame_count_for_gop;
                                stat_data.publisher.tracks.video.recv_frame_count_for_gop = 1;
                            } else {
                                stat_data.publisher.tracks.video.recv_frame_count_for_gop += frame_count;
                            }
                        }
                    }
                }
                StatisticData::AudioCodec {
                    sound_format,
                    profile,
                    sample_rate,
                    channels,
                } => {
                    let audio_codec_data = &mut statistics_data.lock().await.publisher.tracks.audio;
                    audio_codec_data.sound_format = sound_format;
                    audio_codec_data.profile = profile;
                    audio_codec_data.sample_rate = sample_rate;
                    audio_codec_data.channels = channels;
                }
                StatisticData::VideoCodec {
                    codec,
                    profile,
                    level,
                    width,
                    height,
                } => {
                    let video_codec_data = &mut statistics_data.lock().await.publisher.tracks.video;
                    video_codec_data.codec = codec;
                    video_codec_data.profile = profile;
                    video_codec_data.level = level;
                    video_codec_data.width = width;
                    video_codec_data.height = height;
                }
                StatisticData::Publisher {
                    id,
                    remote_addr,
                    start_time,
                } => {
                    let publisher = &mut statistics_data.lock().await.publisher;
                    publisher.id = id;
                    publisher.remote_address = remote_addr;

                    publisher.start_time = start_time;
                }
                StatisticData::Subscriber {
                    id,
                    remote_addr,
                    sub_type,
                    start_time,
                } => {
                    let subscriber = &mut statistics_data.lock().await.subscribers;
                    let sub = StatisticSubscriber {
                        id,
                        remote_address: remote_addr,
                        sub_type,
                        start_time,
                        send_bitrate: 0,
                        send_bytes: 0,
                        total_send_bytes: 0,
                    };
                    subscriber.push(sub);
                }
            }
        }
    }

    async fn receive_statistics_data_loop(
        mut exit_receive: broadcast::Receiver<()>,
        exit_caclulate: broadcast::Receiver<()>,
        mut receiver: StatisticDataReceiver,
        statistics_data: Arc<Mutex<StatisticsStream>>,
    ) {
        let mut statistic_calculate =
            statistics::StatisticsCaculate::new(statistics_data.clone(), exit_caclulate);
        tokio::spawn(async move { statistic_calculate.start().await });

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    data = receiver.recv()  =>
                    {
                        Self::receive_statistics_data(data, &statistics_data).await;
                    }
                    _ = exit_receive.recv()=>{
                        break;
                    }
                }
            }
        });
    }

    async fn receive_event_loop(
        stream_handler: Arc<dyn TStreamHandler>,
        exit: broadcast::Sender<()>,
        mut receiver: TransceiverEventReceiver,
        packet_senders: Arc<Mutex<HashMap<Uuid, PacketDataSender>>>,
        frame_senders: Arc<Mutex<HashMap<Uuid, FrameDataSender>>>,
        statistic_sender: StatisticDataSender,
        statistics_data: Arc<Mutex<StatisticsStream>>,
    ) {
        tokio::spawn(async move {
            loop {
                if let Some(val) = receiver.recv().await {
                    match val {
                        TransceiverEvent::Subscribe {
                            sender,
                            info,
                            result_sender,
                        } => {
                            if let Err(err) = stream_handler
                                .send_prior_data(sender.clone(), info.sub_type)
                                .await
                            {
                                log::error!("receive_event_loop send_prior_data err: {}", err);
                                break;
                            }
                            match sender {
                                DataSender::Frame {
                                    sender: frame_sender,
                                } => {
                                    frame_senders.lock().await.insert(info.id, frame_sender);
                                }
                                DataSender::Packet {
                                    sender: packet_sender,
                                } => {
                                    packet_senders.lock().await.insert(info.id, packet_sender);
                                }
                            }

                            if let Err(err) = result_sender.send(statistic_sender.clone()) {
                                log::error!(
                                    "receive_event_loop:send statistic send err :{:?} ",
                                    err
                                )
                            }

                            let mut statistics_data = statistics_data.lock().await;
                            statistics_data.subscriber_count += 1;
                        }
                        TransceiverEvent::UnSubscribe { info } => {
                            // Remove from frame_senders (assuming it's a HashMap or similar)
                            frame_senders.lock().await.remove(&info.id);

                            // Lock and access statistics_data
                            let mut statistics_data = statistics_data.lock().await;

                            // Find the position of the subscriber to remove
                            if let Some(pos) = statistics_data.subscribers.iter().position(|s| s.id == info.id) {
                                // Remove the subscriber at the found position
                                statistics_data.subscribers.remove(pos);

                                // Update subscriber count
                                statistics_data.subscriber_count -= 1;
                            } else {
                                // Handle case where subscriber was not found (optional)
                                log::warn!("Subscriber with id {} not found for removal", info.id);
                            }
                        }
                        TransceiverEvent::UnPublish {} => {
                            if let Err(err) = exit.send(()) {
                                log::error!("TransmitterEvent::UnPublish send error: {}", err);
                            }
                            break;
                        }
                        TransceiverEvent::Api { sender, uuid } => {
                            log::info!("api:  stream identifier: {:?}", uuid);
                            let statistic_data = if let Some(uid) = uuid {
                                statistics_data.lock().await.query_by_uuid(uid)
                            } else {
                                log::info!("api2:  stream identifier: {:?}", statistics_data);
                                statistics_data.lock().await.clone()
                            };

                            if let Err(err) = sender.send(statistic_data) {
                                log::info!("Transmitter send avstatistic data err: {}", err);
                            }
                        }
                        TransceiverEvent::Request { sender } => {
                            stream_handler.send_information(sender).await;
                        }
                    }
                }
            }
        });
    }

    pub async fn run(self) -> Result<(), StreamHubError> {
        let (tx, _) = broadcast::channel::<()>(1);

        if let Some(receiver) = self.data_receiver.frame_receiver {
            Self::receive_frame_data_loop(
                tx.subscribe(),
                receiver,
                self.id_to_frame_sender.clone(),
            )
            .await;
        }

        if let Some(receiver) = self.data_receiver.packet_receiver {
            Self::receive_packet_data_loop(
                tx.subscribe(),
                receiver,
                self.id_to_packet_sender.clone(),
            )
            .await;
        }

        Self::receive_statistics_data_loop(
            tx.subscribe(),
            tx.subscribe(),
            self.statistic_data_receiver,
            self.statistic_data.clone(),
        )
        .await;

        Self::receive_event_loop(
            self.stream_handler,
            tx,
            self.event_receiver,
            self.id_to_packet_sender,
            self.id_to_frame_sender,
            self.statistic_data_sender,
            self.statistic_data.clone(),
        )
        .await;

        Ok(())
    }

    pub fn get_statistics_data_sender(&self) -> StatisticDataSender {
        self.statistic_data_sender.clone()
    }
}

pub struct StreamsHub {
    //stream identifier to transceiver event sender
    streams: HashMap<(Protocol, String), TransceiverEventSender>,
    //construct UnSubscribe and UnPublish event from Subscribe and Publish event to kick off client
    un_pub_sub_events: HashMap<Uuid, StreamHubEvent>,
    //event is consumed in Stream hub, produced from other protocol sessions
    hub_event_receiver: StreamHubEventReceiver,
    //event is produced from other protocol sessions
    hub_event_sender: StreamHubEventSender,
    //
    client_event_sender: BroadcastEventSender,
    //The rtmp static push/pull and the hls transfer is triggered actively,
    //add a control switches separately.
    rtmp_push_enabled: bool,
    rtmp_remuxer_enabled: bool,
    //enable rtmp pull
    rtmp_pull_enabled: bool,
    //enable hls
    hls_enabled: bool,
    //http notifier on sub/pub event
    notifier: Option<Arc<dyn Notifier>>,
}

impl StreamsHub {
    pub fn new(notifier: Option<Arc<dyn Notifier>>) -> Self {
        let (event_producer, event_consumer) = mpsc::unbounded_channel();
        let (client_producer, _) = broadcast::channel(100);

        Self {
            streams: HashMap::new(),
            un_pub_sub_events: HashMap::new(),
            hub_event_receiver: event_consumer,
            hub_event_sender: event_producer,
            client_event_sender: client_producer,
            rtmp_push_enabled: false,
            rtmp_pull_enabled: false,
            rtmp_remuxer_enabled: false,
            hls_enabled: false,
            notifier,
        }
    }
    pub async fn run(&mut self) {
        self.event_loop().await;
    }

    pub fn set_rtmp_push_enabled(&mut self, enabled: bool) {
        self.rtmp_push_enabled = enabled;
    }

    pub fn set_rtmp_pull_enabled(&mut self, enabled: bool) {
        self.rtmp_pull_enabled = enabled;
    }

    pub fn set_rtmp_remuxer_enabled(&mut self, enabled: bool) {
        self.rtmp_remuxer_enabled = enabled;
    }

    pub fn set_hls_enabled(&mut self, enabled: bool) {
        self.hls_enabled = enabled;
    }

    pub fn get_hub_event_sender(&mut self) -> StreamHubEventSender {
        self.hub_event_sender.clone()
    }

    pub fn get_client_event_consumer(&mut self) -> BroadcastEventReceiver {
        self.client_event_sender.subscribe()
    }

    pub async fn event_loop(&mut self) {
        while let Some(event) = self.hub_event_receiver.recv().await {
            let message = event.to_message();
            match event {
                StreamHubEvent::Publish {
                    protocol,
                    name,
                    info,
                    result_sender,
                    stream_handler,
                } => {
                    let (frame_sender, packet_sender, receiver) = match info.pub_data_type {
                        define::PubDataType::Frame => {
                            let (sender_chan, receiver_chan) = mpsc::unbounded_channel();
                            (
                                Some(sender_chan),
                                None,
                                DataReceiver {
                                    frame_receiver: Some(receiver_chan),
                                    packet_receiver: None,
                                },
                            )
                        }
                        define::PubDataType::Packet => {
                            let (sender_chan, receiver_chan) = mpsc::unbounded_channel();
                            (
                                None,
                                Some(sender_chan),
                                DataReceiver {
                                    frame_receiver: None,
                                    packet_receiver: Some(receiver_chan),
                                },
                            )
                        }
                        define::PubDataType::Both => {
                            let (sender_frame_chan, receiver_frame_chan) =
                                mpsc::unbounded_channel();
                            let (sender_packet_chan, receiver_packet_chan) =
                                mpsc::unbounded_channel();

                            (
                                Some(sender_frame_chan),
                                Some(sender_packet_chan),
                                DataReceiver {
                                    frame_receiver: Some(receiver_frame_chan),
                                    packet_receiver: Some(receiver_packet_chan),
                                },
                            )
                        }
                    };

                    let result = match self
                        .publish(protocol.clone(), name.clone(), receiver, stream_handler)
                        .await
                    {
                        Ok(statistic_data_sender) => {
                            if let Some(notifier) = &self.notifier {
                                notifier.on_publish_notify(&message).await;
                            }
                            self.un_pub_sub_events
                                .insert(info.id, StreamHubEvent::UnPublish { protocol, name, info });

                            Ok((frame_sender, packet_sender, Some(statistic_data_sender)))
                        }
                        Err(err) => {
                            log::error!("event_loop Publish err: {}", err);
                            Err(err)
                        }
                    };

                    if result_sender.send(result).is_err() {
                        log::error!("event_loop Subscribe error: The receiver dropped.")
                    }
                }

                StreamHubEvent::UnPublish {
                    protocol,
                    name,
                    info: _,
                } => {
                    if let Err(err) = self.unpublish(protocol.clone(), name.clone()) {
                        log::error!(
                            "event_loop Unpublish err: {} with identifier: {}-{}",
                            err,
                            protocol,
                            name
                        );
                    }

                    if let Some(notifier) = &self.notifier {
                        notifier.on_unpublish_notify(&message).await;
                    }
                }
                StreamHubEvent::Subscribe {
                    protocol,
                    name,
                    info,
                    result_sender,
                } => {
                    let sub_id = info.id;
                    let info_clone = info.clone();

                    //new chan for Frame/Packet sender and receiver
                    let (sender, receiver) = match info.sub_data_type {
                        define::SubDataType::Frame => {
                            let (sender_chan, receiver_chan) = mpsc::unbounded_channel();
                            (
                                DataSender::Frame {
                                    sender: sender_chan,
                                },
                                DataReceiver {
                                    frame_receiver: Some(receiver_chan),
                                    packet_receiver: None,
                                },
                            )
                        }
                        define::SubDataType::Packet => {
                            let (sender_chan, receiver_chan) = mpsc::unbounded_channel();
                            (
                                DataSender::Packet {
                                    sender: sender_chan,
                                },
                                DataReceiver {
                                    frame_receiver: None,
                                    packet_receiver: Some(receiver_chan),
                                },
                            )
                        }
                    };

                    let rv = match self.subscribe(protocol.clone(), name.clone(), info_clone, sender).await {
                        Ok(statistic_data_sender) => {
                            if let Some(notifier) = &self.notifier {
                                notifier.on_play_notify(&message).await;
                            }

                            self.un_pub_sub_events
                                .insert(sub_id, StreamHubEvent::UnSubscribe {protocol, name, info });
                            Ok((receiver, Some(statistic_data_sender)))
                        }
                        Err(err) => {
                            log::error!("event_loop Subscribe error: {}", err);
                            Err(err)
                        }
                    };

                    if result_sender.send(rv).is_err() {
                        log::error!("event_loop Subscribe error: The receiver dropped.")
                    }
                }
                StreamHubEvent::UnSubscribe { protocol, name, info } => {
                    if self.unsubscribe(protocol, name, info).is_ok() {
                        if let Some(notifier) = &self.notifier {
                            notifier.on_stop_notify(&message).await;
                        }
                    }
                }

                StreamHubEvent::ApiStatistic {
                    top_n,
                    protocol,
                    name,
                    uuid,
                    result_sender,
                } => {
                    log::info!("api_statistic1:  stream identifier: {:?}-{:?}", protocol, name);
                    let result = self.api_statistic(top_n, protocol, name, uuid).await.unwrap_or_else(|err| {
                        log::error!("event_loop api error: {}", err);
                        json!(err.to_string())
                    });

                    if let Err(err) = result_sender.send(result) {
                        log::error!("event_loop api error: {}", err);
                    }
                }
                StreamHubEvent::ApiKickClient { id } => {
                    println!("ERR {:?}", self.api_kick_off_client(id));
                    if let Err(err) =self.api_kick_off_client(id) {
                        log::error!("api_kick_off_client api error: {}", err);
                    }
                }
                StreamHubEvent::Request { protocol, name, sender  } => {
                    if let Err(err) = self.request(protocol,name, sender) {
                        log::error!("event_loop request error: {}", err);
                    }
                }
            }
        }
    }

    fn request(
        &mut self,
        protocol: Protocol,
        name: String,
        sender: mpsc::UnboundedSender<Information>,
    ) -> Result<(), StreamHubError> {
        if let Some(producer) = self.streams.get_mut(&(protocol.clone(), name.clone())) {
            let event = TransceiverEvent::Request { sender };
            log::info!("Request:  stream identifier: {}-{}", protocol, name);
            producer.send(event).map_err(|_| StreamHubError {
                value: StreamHubErrorValue::SendError,
            })?;
        }
        Ok(())
    }

    async fn api_statistic(
        &mut self,
        top_n: Option<usize>,
        protocol: Option<Protocol>,
        name: Option<String>,
        uuid: Option<Uuid>,
    ) -> Result<Value, StreamHubError> {
        if self.streams.is_empty() {
            return Ok(json!({}));
        }
        log::info!("api_statistic:  stream identifier: {:?}-{:?}", protocol, name);
        let (stream_sender, mut stream_receiver) = mpsc::unbounded_channel();

        let mut stream_count: usize = 1;

        if let Some(protocol) = protocol {
            if let Some(name) = name {
                if let Some(event_sender) = self.streams.get_mut(&(protocol.clone(), name.clone())) {
                    let event = TransceiverEvent::Api {
                        sender: stream_sender.clone(),
                        uuid,
                    };
                    log::info!("api_statistic:  stream identifier: {}-{}", protocol, name);
                    event_sender.send(event).map_err(|_| StreamHubError {
                        value: StreamHubErrorValue::SendError,
                    })?;
                }
            }
        } else {
            stream_count = self.streams.len();
            for v in self.streams.values() {
                if let Err(err) = v.send(TransceiverEvent::Api {
                    sender: stream_sender.clone(),
                    uuid,
                }) {
                    log::error!("TransmitterEvent  api send data err: {}", err);
                    return Err(StreamHubError {
                        value: StreamHubErrorValue::SendError,
                    });
                }
            }
        }

        let mut data = Vec::new();

        loop {
            log::info!("api_statistic:  stream count: {}", stream_count);
            if let Some(stream_statistics) = stream_receiver.recv().await {
                data.push(stream_statistics);
            }
            if data.len() == stream_count {
                break;
            }
        }

        if let Some(topn) = top_n {
            data.sort_by(|a, b| b.subscriber_count.cmp(&a.subscriber_count));
            let top_streams: Vec<StatisticsStream> = data.into_iter().take(topn).collect();
            return Ok(serde_json::to_value(top_streams)?);
        }

        Ok(serde_json::to_value(data)?)
    }

    fn api_kick_off_client(&mut self, uid: Uuid) -> Result<(), StreamHubError> {
        if let Some(event) = self.un_pub_sub_events.get(&uid) {
            match event {
                StreamHubEvent::UnPublish { protocol, name, info } => {
                    if self
                        .hub_event_sender
                        .send(StreamHubEvent::UnPublish {
                            protocol: protocol.clone(),
                            name: name.clone(),
                            info: info.clone(),
                        })
                        .is_err()
                    {
                        return Err(StreamHubError {
                            value: StreamHubErrorValue::SendError,
                        });
                    }
                }
                StreamHubEvent::UnSubscribe { protocol, name, info } => {
                    if self
                        .hub_event_sender
                        .send(StreamHubEvent::UnSubscribe {
                            protocol: protocol.clone(),
                            name: name.clone(),
                            info: info.clone(),
                        })
                        .is_err()
                    {
                        return Err(StreamHubError {
                            value: StreamHubErrorValue::SendError,
                        });
                    }
                }
                _ => {
                    return Ok(());
                }
            }
        };
        log::warn!("cannot find uid: {}", uid);
        return Err(StreamHubError {
            value: StreamHubErrorValue::NoSession,
        });
    }

    //player subscribe a stream
    pub async fn subscribe(
        &mut self,
        protocol: Protocol,
        name: String,
        sub_info: SubscriberInfo,
        sender: DataSender,
    ) -> Result<StatisticDataSender, StreamHubError> {
        if let Some(event_sender) = self.streams.get_mut(&(protocol.clone(), name.clone())) {
            let (result_sender, result_receiver) = oneshot::channel();
            let event = TransceiverEvent::Subscribe {
                sender,
                info: sub_info,
                result_sender,
            };
            log::info!("subscribe:  stream identifier: {}-{}", protocol, name);
            event_sender.send(event).map_err(|_| StreamHubError {
                value: StreamHubErrorValue::SendError,
            })?;

            return Ok(result_receiver.await?);
        }

        if self.rtmp_pull_enabled {
            log::info!("subscribe: try to pull stream, identifier: {}-{}", protocol, name);

            let client_event = BroadcastEvent::Subscribe {
                protocol,
                name,
            };

            //send subscribe info to pull clients
            self.client_event_sender
                .send(client_event)
                .map_err(|_| StreamHubError {
                    value: StreamHubErrorValue::SendError,
                })?;
        }

        Err(StreamHubError {
            value: StreamHubErrorValue::NoAppOrStreamName,
        })
    }

    pub fn unsubscribe(
        &mut self,
        protocol: Protocol,
        name: String,
        sub_info: SubscriberInfo,
    ) -> Result<(), StreamHubError> {
        match self.streams.get_mut(&(protocol.clone(), name.clone())) {
            Some(producer) => {
                log::info!("unsubscribe....:{}-{}", protocol, name);
                let event = TransceiverEvent::UnSubscribe { info: sub_info };
                producer.send(event).map_err(|_| StreamHubError {
                    value: StreamHubErrorValue::SendError,
                })?;
            }
            None => {
                log::info!("unsubscribe None....:{}-{}", protocol, name);
                return Err(StreamHubError {
                    value: StreamHubErrorValue::NoAppName,
                });
            }
        }

        Ok(())
    }

    //publish a stream
    pub async fn publish(
        &mut self,
        protocol: Protocol,
        name: String,
        receiver: DataReceiver,
        handler: Arc<dyn TStreamHandler>,
    ) -> Result<StatisticDataSender, StreamHubError> {
        if self.streams.get(&(protocol.clone(), name.clone())).is_some() {
            return Err(StreamHubError {
                value: StreamHubErrorValue::Exists,
            });
        }

        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let transceiver =
            StreamDataTransceiver::new(protocol.clone(), name.clone(), receiver, event_receiver, handler);

        let statistic_data_sender = transceiver.get_statistics_data_sender();


        if let Err(err) = transceiver.run().await {
            log::error!(
                "transceiver run error, idetifier: {}-{}, error: {}",
                protocol,
                name,
                err,
            );
        } else {
            log::info!("transceiver run success, idetifier: {}-{}", protocol, name);
        }

        self.streams.insert((protocol.clone(),name.clone()), event_sender);

        if self.rtmp_push_enabled || self.hls_enabled || self.rtmp_remuxer_enabled {
            let client_event = BroadcastEvent::Publish { protocol, name };

            //send publish info to push clients
            self.client_event_sender
                .send(client_event)
                .map_err(|_| StreamHubError {
                    value: StreamHubErrorValue::SendError,
                })?;
        }

        Ok(statistic_data_sender)
    }

    fn unpublish(&mut self, protocol: Protocol, name: String) -> Result<(), StreamHubError> {
        match self.streams.get_mut(&(protocol.clone(), name.clone())) {
            Some(producer) => {
                let event = TransceiverEvent::UnPublish {};
                producer.send(event).map_err(|_| StreamHubError {
                    value: StreamHubErrorValue::SendError,
                })?;
                self.streams.remove(&(protocol.clone(), name.clone()));
                log::info!("unpublish remove stream, stream identifier: {}-{}", protocol, name);
            }
            None => {
                return Err(StreamHubError {
                    value: StreamHubErrorValue::NoAppName,
                });
            }
        }

        Ok(())
    }
}
