use axum::extract::ConnectInfo;
use futures::channel::mpsc::unbounded;
use {
    axum::{
        body::Body,
        extract::{Request, State},
        handler::Handler,
        http::StatusCode,
        response::Response,
    },
    commonlib::auth::{Auth, SecretCarrier},
    std::net::SocketAddr,
    tokio::{fs::File, net::TcpListener},
    tokio_util::codec::{BytesCodec, FramedRead},
};
use streamhub::define::StreamHubEventSender;
use crate::flv::httpflv::HttpFlv;

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;
static NOTFOUND: &[u8] = b"Not Found";
static UNAUTHORIZED: &[u8] = b"Unauthorized";

async fn handle_connection(
    State((event_producer, auth)): State<(StreamHubEventSender, Option<Auth>)>, // event_producer: ChannelEventProducer
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
) -> Response<Body> {
    let path = req.uri().path();

    let query_string: Option<String> = req.uri().query().map(|s| s.to_string());
    let mut file_path: String = String::from("");

    if path.ends_with(".m3u8") {
        //http://127.0.0.1/app_name/stream_name/index.m3u8
        let m3u8_index = path.find(".m3u8").unwrap();

        if m3u8_index > 0 {
            let (left, _) = path.split_at(m3u8_index);
            let rv: Vec<_> = left.split('/').collect();

            let app_name = String::from(rv[1]);
            let stream_name = String::from(rv[2]);

            if let Some(auth_val) = auth {
                if auth_val
                    .authenticate(
                        &stream_name,
                        &query_string.map(|q| SecretCarrier::Query(q)),
                        true,
                    )
                    .is_err()
                {
                    return Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(UNAUTHORIZED.into())
                        .unwrap();
                }
            }

            file_path = format!("./{app_name}/{stream_name}/index.m3u8");
        }
    } else if path.ends_with(".ts") {
        //http://127.0.0.1/app_name/stream_name/ts_name.m3u8
        let ts_index = path.find(".ts").unwrap();

        if ts_index > 0 {
            let (left, _) = path.split_at(ts_index);

            let rv: Vec<_> = left.split('/').collect();

            let app_name = String::from(rv[1]);
            let stream_name = String::from(rv[2]);
            let ts_name = String::from(rv[3]);

            file_path = format!("./{app_name}/{stream_name}/{ts_name}.ts");
        }
    }else if path.ends_with(".flv") {
        let flv_index = path.find(".flv").unwrap();
        if flv_index > 0 {
            let (left, _) = path.split_at(flv_index);
            let rv: Vec<_> = left.split('/').collect();

            let app_name = String::from(rv[1]);
            let stream_name = String::from(rv[2]);

            if let Some(auth_val) = auth {
                if auth_val
                    .authenticate(
                        &stream_name,
                        &query_string.map(|q| SecretCarrier::Query(q)),
                        true,
                    )
                    .is_err()
                {
                    return Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(UNAUTHORIZED.into())
                        .unwrap();
                }
            }

            let (http_response_data_producer, http_response_data_consumer) = unbounded();

            let mut flv_hanlder = HttpFlv::new(
                app_name,
                stream_name,
                event_producer,
                http_response_data_producer,
                req.uri().to_string(),
                remote_addr,
            );

            tokio::spawn(async move {
                if let Err(err) = flv_hanlder.run().await {
                    log::error!("flv handler run error {}", err);
                }
            });

            let mut resp = Response::new(Body::from_stream(http_response_data_consumer));
            resp.headers_mut()
                .insert("Access-Control-Allow-Origin", "*".parse().unwrap());

            return resp
        }
    }
    simple_file_send(file_path.as_str()).await
}

/// HTTP status code 404
fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(NOTFOUND.into())
        .unwrap()
}

async fn simple_file_send(filename: &str) -> Response<Body> {
    // Serve a file by asynchronously reading it by chunks using tokio-util crate.

    if let Ok(file) = File::open(filename).await {
        let stream = FramedRead::new(file, BytesCodec::new());
        let body = Body::from_stream(stream);
        return Response::new(body);
    }

    not_found()
}

pub async fn run(
    event_producer: StreamHubEventSender,
    port: usize,
    auth: Option<Auth>,
) -> Result<()> {
    let listen_address = format!("0.0.0.0:{port}");
    let sock_addr: SocketAddr = listen_address.parse().unwrap();

    let listener = TcpListener::bind(sock_addr).await?;

    log::info!("Hls server listening on http://{}", sock_addr);

    let handle_connection = handle_connection.with_state((event_producer.clone(), auth));

    axum::serve(
        listener,
        handle_connection.into_make_service_with_connect_info::<SocketAddr>(),
    ).await?;

    Ok(())
}
