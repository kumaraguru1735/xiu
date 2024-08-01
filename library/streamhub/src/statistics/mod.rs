use {
    super::stream::StreamIdentifier,
    crate::{define::SubscribeType, utils::Uuid},
    chrono::{DateTime, Local},
    serde::Serialize,
    std::{sync::Arc, time::Duration},
    tokio::{
        sync::{broadcast::Receiver, Mutex},
        time,
    },
    xflv::define::{AacProfile, AvcCodecId, AvcLevel, AvcProfile, SoundFormat},
};

#[derive(Debug, Clone, Serialize, Default)]
pub struct VideoInfo {
    pub codec: AvcCodecId,
    pub profile: AvcProfile,
    pub level: AvcLevel,
    pub width: u32,
    pub height: u32,
    /*used for caculate the bitrate*/
    #[serde(skip_serializing)]
    pub recv_bytes: usize,
    #[serde(rename = "bitrate(kbits/s)")]
    pub bitrate: usize,
    /*used for caculate the frame rate*/
    #[serde(skip_serializing)]
    pub recv_frame_count: usize,
    pub frame_rate: usize,
    /*used for caculate the GOP*/
    #[serde(skip_serializing)]
    pub recv_frame_count_for_gop: usize,
    pub gop: usize,
}
#[derive(Debug, Clone, Serialize, Default)]
pub struct AudioInfo {
    pub sound_format: SoundFormat,
    pub profile: AacProfile,
    pub sample_rate: u32,
    pub channels: u8,
    /*used for caculate the bitrate*/
    #[serde(skip_serializing)]
    pub recv_bytes: usize,
    #[serde(rename = "bitrate(kbits/s)")]
    pub bitrate: usize,
}
#[derive(Debug, Clone, Serialize, Default)]
pub struct StatisticsStream {
    /*publisher infomation */
    pub publisher: StatisticPublisher,
    /*subscriber infomation */
    pub subscribers: Vec<StatisticSubscriber>,
    /*How many clients are subscribing to this stream.*/
    pub subscriber_count: usize,
    /*calculate upstream traffic, now equals audio and video traffic received by this server*/
    pub total_recv_bytes: usize,
    /*calculate downstream traffic, now equals audio and video traffic sent to all subscribers*/
    pub total_send_bytes: usize,
}
#[derive(Debug, Clone, Serialize, Default)]
pub struct StatisticPublisher {
    pub id: Uuid,
    identifier: StreamIdentifier,
    pub start_time: DateTime<Local>,
    pub video: VideoInfo,
    pub audio: AudioInfo,
    pub remote_address: String,
    /*used for caculate the recv_bitrate*/
    #[serde(skip_serializing)]
    pub recv_bytes: usize,
    /*the bitrate at which the server receives streaming data*/
    #[serde(rename = "recv_bitrate(kbits/s)")]
    pub recv_bitrate: usize,
}

impl StatisticPublisher {
    pub fn new(identifier: StreamIdentifier) -> Self {
        Self {
            identifier,
            ..Default::default()
        }
    }
}
#[derive(Debug, Clone, Serialize, Default)]
pub struct StatisticSubscriber {
    pub id: Uuid,
    pub start_time: DateTime<Local>,
    pub remote_address: String,
    pub sub_type: SubscribeType,
    /*used for caculate the send_bitrate*/
    #[serde(skip_serializing)]
    pub send_bytes: usize,
    /*the bitrate at which the server send streaming data to a client*/
    #[serde(rename = "send_bitrate(kbits/s)")]
    pub send_bitrate: usize,
    #[serde(rename = "total_send_bytes(kbits/s)")]
    pub total_send_bytes: usize,
}

impl StatisticsStream {
    pub fn new(identifier: StreamIdentifier) -> Self {
        Self {
            publisher: StatisticPublisher::new(identifier),
            ..Default::default()
        }
    }

    fn get_publisher(&self) -> StatisticsStream {
        let mut statistic_stream = self.clone();
        statistic_stream.subscribers.clear();
        statistic_stream
    }

    fn get_subscriber(&self, uuid: Uuid) -> StatisticsStream {
        // Find the subscriber with the matching UUID
        let found_subscribers: Vec<StatisticSubscriber> = self.subscribers
            .iter()
            .filter(|s| s.id == uuid) // Adjust this according to how your UUID is stored
            .cloned() // Clone the found subscriber(s)
            .collect();

        // Create a new StatisticsStream with the found subscribers
        StatisticsStream {
            publisher: self.publisher.clone(),
            subscribers: found_subscribers.clone(),
            subscriber_count: found_subscribers.len(),
            total_recv_bytes: self.total_recv_bytes,
            total_send_bytes: self.total_send_bytes,
        }
    }

    pub fn query_by_uuid(&self, uuid: Uuid) -> StatisticsStream {
        if uuid == self.publisher.id {
            self.get_publisher()
        } else {
            self.get_subscriber(uuid)
        }
    }
}

pub struct StatisticsCaculate {
    stream: Arc<Mutex<StatisticsStream>>,
    exit: Receiver<()>,
}

impl StatisticsCaculate {
    pub fn new(stream: Arc<Mutex<StatisticsStream>>, exit: Receiver<()>) -> Self {
        Self { stream, exit }
    }

    async fn caculate(&mut self) {
        let mut stream_statistics_clone = self.stream.lock().await;

        stream_statistics_clone.publisher.video.bitrate =
            stream_statistics_clone.publisher.video.recv_bytes * 8 / 5000;
        stream_statistics_clone.publisher.video.recv_bytes = 0;

        stream_statistics_clone.publisher.video.frame_rate =
            stream_statistics_clone.publisher.video.recv_frame_count / 5;
        stream_statistics_clone.publisher.video.recv_frame_count = 0;

        stream_statistics_clone.publisher.audio.bitrate =
            stream_statistics_clone.publisher.audio.recv_bytes * 8 / 5000;
        stream_statistics_clone.publisher.audio.recv_bytes = 0;

        stream_statistics_clone.publisher.recv_bitrate =
            stream_statistics_clone.publisher.recv_bytes * 8 / 5000;
        stream_statistics_clone.publisher.recv_bytes = 0;

        // Corrected loop for mutable references
        for subscriber in stream_statistics_clone.subscribers.iter_mut() {
            subscriber.send_bitrate = subscriber.send_bytes * 8 / 5000;
            subscriber.send_bytes = 0;
        }
    }

    pub async fn start(&mut self) {
        let mut interval = time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.caculate().await;
                },
                _ = self.exit.recv() => {
                    log::info!("avstatistics shutting down");
                    return;
                },
            }
        }
    }
}