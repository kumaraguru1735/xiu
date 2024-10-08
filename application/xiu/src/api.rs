use commonlib::config::save_config;
use commonlib::config::load_config;
use axum::routing::put;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::delete;
use base64::Engine;
use base64::engine::general_purpose;
use tower::ServiceBuilder;
use {
    axum::{
        extract::Query,
        routing::get,
        Json, Router,
    },
    serde::Deserialize,
    serde_json::Value,
    std::sync::Arc,
    streamhub::{
        define::{self, StreamHubEventSender},
        utils::Uuid,
    },
    tokio::{self, sync::oneshot},
};

//use pulse::run_stats;
use commonlib::config::Streams;

#[derive(serde::Serialize)]
struct ApiResponse<T> {
    success: bool,
    message: String,
    data: T,
}

// the input to our `KickOffClient` handler
#[derive(Deserialize)]
struct KickOffClient {
    uuid: String,
}

#[derive(Deserialize, Debug)]
struct QueryWholeStreamsParams {
    // query top N by subscriber's count.
    top: Option<usize>,
}

#[derive(Deserialize)]
struct QueryStream {
    app_name: String,
    stream_name: String,
}

#[derive(Clone)]
struct ApiService {
    channel_event_producer: StreamHubEventSender,
}

impl ApiService {
    async fn root(&self) -> String {
        String::from(
            "Usage of xiu http api:
                ./api/streams(get) query whole streams' information or top streams' information.
                ./api/stream?app_name=demo&stream_name=demo(get) query stream information by identifier and uuid.
                ./api/session/<session_id>(delete) kick off client by publish/subscribe id.\n",
        )
    }

    // async fn pulse(&self) -> Json<ApiResponse<Value>> {
    //     let api_response = ApiResponse {
    //         success: true,
    //         message: String::from("success"),
    //         data: serde_json::from_str(&run_stats()).unwrap(),
    //     };
    //     Json(api_response)
    // }

    async fn query_whole_streams(
        &self,
        params: QueryWholeStreamsParams,
    ) -> Json<ApiResponse<Value>> {
        log::info!("query_whole_streams: {:?}", params);
        let (result_sender, result_receiver) = oneshot::channel();
        let hub_event = define::StreamHubEvent::ApiStatistic {
            top_n: params.top,
            protocol: None,
            name: None,
            uuid: None,
            result_sender,
        };
        if let Err(err) = self.channel_event_producer.send(hub_event) {
            log::error!("send api event error: {}", err);
        }

        match result_receiver.await {
            Ok(dat_val) => {
                let api_response = ApiResponse {
                    success: true,
                    message: String::from("success"),
                    data: dat_val,
                };
                Json(api_response)
            }
            Err(err) => {
                let api_response = ApiResponse {
                    success: false,
                    message: String::from("failed"),
                    data: serde_json::json!(err.to_string()),
                };
                Json(api_response)
            }
        }
    }

    async fn query_stream(&self, stream: QueryStream) -> Json<ApiResponse<Value>> {
        let (result_sender, result_receiver) = oneshot::channel();
        let hub_event = define::StreamHubEvent::ApiStatistic {
            top_n: None,
            protocol: Some(streamhub::stream::Protocol::Rtmp),
            name: Some(format!("{}/{}", stream.app_name, stream.stream_name)),
            uuid: None,
            result_sender,
        };

        if let Err(err) = self.channel_event_producer.send(hub_event) {
            log::error!("send api event error: {}", err);
        }

        match result_receiver.await {
            Ok(dat_val) => {
                let api_response = ApiResponse {
                    success: true,
                    message: String::from("success"),
                    data: dat_val,
                };
                Json(api_response)
            }
            Err(err) => {
                let api_response = ApiResponse {
                    success: false,
                    message: String::from("failed"),
                    data: serde_json::json!(err.to_string()),
                };
                Json(api_response)
            }
        }
    }


    pub async fn add_stream(&self, mut stream: Streams) -> Json<ApiResponse<Value>> {
        let config_path = "config.json";

        // Ensure stream.name follows the correct format
        if !stream.name.contains('/') {
            stream.name = format!("static/{}", stream.name);
        }

        // Load existing config
        let mut config = match load_config(config_path) {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to load config: {}", err);
                return Json(ApiResponse {
                    success: false,
                    message: String::from("failed to load config"),
                    data: serde_json::json!(""),
                });
            }
        };

        // Unwrap or initialize the streams vector
        let streams = config.streams.get_or_insert_with(Vec::new);

        // Check if the stream exists by name and update or add it
        if let Some(existing_stream) = streams.iter_mut().find(|s| s.name == stream.name) {
            *existing_stream = stream.clone();  // Update existing stream
        } else {
            streams.push(stream.clone());  // Add new stream
        }

        // Save the updated config
        match save_config(config_path, &config) {
            Ok(_) => Json(ApiResponse {
                success: true,
                message: String::from("success"),
                data: serde_json::json!(""),
            }),
            Err(err) => {
                log::error!("Failed to save config: {}", err);
                Json(ApiResponse {
                    success: false,
                    message: String::from("failed to save config"),
                    data: serde_json::json!(""),
                })
            }
        }
    }

    pub async fn delete_stream(&self, mut del_stream: Streams) -> Json<ApiResponse<Value>> {
        let config_path = "config.json";

        // Ensure stream.name follows the correct format
        if !del_stream.name.contains('/') {
            del_stream.name = format!("static/{}", del_stream.name);
        }

        // Load existing config
        let mut config = match load_config(config_path) {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to load config: {}", err);
                return Json(ApiResponse {
                    success: false,
                    message: String::from("failed to load config"),
                    data: serde_json::json!(""),
                });
            }
        };

        // Unwrap the streams vector or return an error if not initialized
        let streams = match config.streams.as_mut() {
            Some(streams) => streams,
            None => {
                return Json(ApiResponse {
                    success: false,
                    message: String::from("no streams available to delete"),
                    data: serde_json::json!(""),
                });
            }
        };

        // Find and remove the stream by name
        let original_len = streams.len();
        streams.retain(|s| s.name != del_stream.name);

        if streams.len() == original_len {
            // No stream was deleted
            return Json(ApiResponse {
                success: false,
                message: String::from("stream not found"),
                data: serde_json::json!(""),
            });
        }

        // Save the updated config
        match save_config(config_path, &config) {
            Ok(_) => Json(ApiResponse {
                success: true,
                message: String::from("stream deleted successfully"),
                data: serde_json::json!(""),
            }),
            Err(err) => {
                log::error!("Failed to save config: {}", err);
                Json(ApiResponse {
                    success: false,
                    message: String::from("failed to save config"),
                    data: serde_json::json!(""),
                })
            }
        }
    }

    async fn kick_off_client(&self, id: KickOffClient) -> Json<ApiResponse<Value>> {
        match Uuid::from_str2(&id.uuid) {
            Some(id) => {
                let hub_event = define::StreamHubEvent::ApiKickClient { id };
                println!("kick_off_client: {:?}", id);

                match self.channel_event_producer.send(hub_event) {
                    Ok(_) => Json(ApiResponse {
                        success: true,
                        message: String::from("success"),
                        data: serde_json::json!(""),
                    }),
                    Err(err) => {
                        log::error!("send api kick_off_client event error: {}", err);
                        Json(ApiResponse {
                            success: false,
                            message: String::from("failed to send event"),
                            data: serde_json::json!(""),
                        })
                    }
                }
            },
            None => {
                log::error!("invalid UUID format");
                Json(ApiResponse {
                    success: false,
                    message: String::from("invalid UUID format"),
                    data: serde_json::json!(""),
                })
            }
        }
    }
}



async fn basic_auth(
    req: Request<Body>,
    next: Next,
    username: String,
    password: String,
) -> impl IntoResponse {
    if let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(basic) = auth_str.strip_prefix("Basic ") {
                if let Ok(decoded) = general_purpose::STANDARD.decode(basic) {
                    if let Ok(decoded_str) = String::from_utf8(decoded) {
                        let mut parts = decoded_str.splitn(2, ':');
                        let user = parts.next().unwrap_or("");
                        let pass = parts.next().unwrap_or("");
                        if user == username && pass == password {
                            return next.run(req).await;
                        }
                    }
                }
            }
        }
    }

    (StatusCode::UNAUTHORIZED, Json(ApiResponse {
        success: false,
        message: String::from("UNAUTHORIZED"),
        data: serde_json::json!(""),
    })).into_response()
}

pub async fn run(
    producer: StreamHubEventSender,
    port: usize,
    username: String,
    password: String
) {
    let api = Arc::new(ApiService {
        channel_event_producer: producer,
    });

    let api_root = api.clone();
    let root = move || async move { api_root.root().await };

    let api_query_streams = api.clone();
    let query_streams = move |Query(params): Query<QueryWholeStreamsParams>| async move {
        api_query_streams.query_whole_streams(params).await
    };

    let api_query_stream = api.clone();
    let query_stream = move |Query(params): Query<QueryStream>| async move {
        api_query_stream.query_stream(params).await
    };
    let api_add_stream = api.clone();
    let add_stream = move |Json(stream): Json<Streams>| async move {
        api_add_stream.add_stream(stream).await
    };

    let api_delete_stream = api.clone();
    let delete_stream = move |Json(stream): Json<Streams>| async move {
        api_delete_stream.delete_stream(stream).await
    };

    let api_kick_off = api.clone();
    let kick_off = move |Path(id): Path<String>| async move {
        api_kick_off.kick_off_client(KickOffClient { uuid: id }).await
    };

    // let api_pulse = api.clone();
    // let pulse = move || async move {
    //     api_pulse.pulse().await
    // };

    let app = Router::new()
        .route("/", get(root))
        .route("/api/streams", get(query_streams))
        .route("/api/stream", get(query_stream))
        .route("/api/stream", put(add_stream))
        .route("/api/stream", delete(delete_stream))
        .route("/api/session/:id", delete(kick_off))
        // .route("/api/pulse", get(pulse))
        .layer(ServiceBuilder::new().layer(middleware::from_fn(move |req, next| {
            basic_auth(req, next, username.clone(), password.clone())
        })));

    log::info!("Http api server listening on http://0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
