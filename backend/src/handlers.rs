use std::{
    convert::Infallible,
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    data::{internal::ServerState, models::*, web::*},
    database::Database,
};
use common::crypto::compare_digest;
use diesel::prelude::*;
use rand::prelude::*;
use tokio::{fs::File, io::AsyncReadExt};
use warp::{
    http::{header, Response, StatusCode},
    Rejection, Reply,
};

pub async fn handle_nonce(state: Arc<Mutex<ServerState>>) -> Response<Vec<u8>> {
    let state = state.lock().unwrap();
    let mut nonce = [0; 32];
    thread_rng().fill_bytes(&mut nonce);

    log::info!("sent nonce: {:x?}", nonce);
    let mut cookie = SessionCookie::new();
    cookie.nonce = nonce;

    Response::builder()
        .header(header::SET_COOKIE, cookie.serialize_as_set_cookie(&state))
        .body(nonce.to_vec())
        .unwrap()
}

pub async fn handle_login(
    session_cookie: String,
    data: common::data::LoginData,
    state: Arc<Mutex<ServerState>>,
    db: Arc<Mutex<Database>>,
) -> Response<warp::hyper::Body> {
    let state = state.lock().unwrap();
    let mut db = db.lock().unwrap();
    log::info!("Login with data: {:?}", data);
    log::debug!("cookie {}", session_cookie);

    if let Some(mut cookie) = SessionCookie::deserialize_with_hmac(&session_cookie, &state) {
        use crate::data::schema::users::dsl::*;
        let all_users = users
            .load::<User>(&mut db.conn)
            .expect("Could not load table \"Users\"");

        let matching_users = all_users
            .into_iter()
            .filter(|user| {
                let expected_passcode_hmac = common::crypto::hmac(&cookie.nonce, &user.passcode);
                compare_digest(&expected_passcode_hmac, &data.passcode_hmac)
            })
            .collect::<Vec<_>>();

        if matching_users.len() == 1 {
            // Todo:
            // 1. forward to correct page depending on user role, ether by returning type
            // itself or by returning a file

            let user = &matching_users[0];
            cookie.user_id = user.id;

            return Response::builder()
                .status(StatusCode::OK)
                .header(header::SET_COOKIE, cookie.serialize_as_set_cookie(&state))
                .body(warp::hyper::body::Body::from(format!(
                    "Hello, {}!",
                    user.name
                )))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(warp::hyper::body::Body::empty())
        .unwrap()
}

pub async fn handle_rejection(_err: Rejection) -> Result<impl Reply, Infallible> {
    Ok(warp::reply::with_status(
        "BAD_REQUEST",
        StatusCode::BAD_REQUEST,
    ))
}

pub async fn handle_ws(ws_connection: warp::ws::WebSocket) {
    println!("WebSocket connection");
    use futures_util::{SinkExt, StreamExt};
    let (mut sink, mut stream) = ws_connection.split();

    tokio::task::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            println!("received: {:?}", msg);
            sink.send(msg).await.unwrap_or_else(|err| {
                eprintln!("WebSocket send error: {}", err);
            })
        }
    });
}

async fn get_file_as_byte_vec(filename: &PathBuf) -> Option<Vec<u8>> {
    let mut f = File::open(&filename).await.ok()?;
    let mut buffer = vec![];
    f.read_to_end(&mut buffer).await.ok()?;

    Some(buffer)
}

async fn retreive_index() -> Vec<u8> {
    let mut filename = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    filename.push("static/index.html");
    log::debug!("Looking for index.html at: {:?}", filename);
    get_file_as_byte_vec(&filename)
        .await
        .expect("Should have an index.html in static")
}
pub async fn handle_index() -> Response<Vec<u8>> {
    let content = retreive_index().await;
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html")
        .body(content)
        .unwrap()
}

pub async fn handle_file(path: String) -> Response<Vec<u8>> {
    let mut filename = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    filename.push("static");
    filename.push(&path);
    log::debug!("Looking for file at: {:?}", filename);

    let maybe_content = get_file_as_byte_vec(&filename).await;

    let ext = path.split(".").last().unwrap_or("bin");

    let mime = match ext {
        "html" | "htm" => "text/html",
        "js" => "text/javascript",
        "wasm" => "application/wasm",
        // Add other mimetypes as needed
        _ => "application/octet-stream",
    };

    if let Some(content) = maybe_content {
        return Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .body(content)
            .unwrap();
    } else {
        let content = retreive_index().await;
        return Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(content)
            .unwrap();
    };
}
