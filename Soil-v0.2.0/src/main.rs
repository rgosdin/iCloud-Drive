mod db;
mod multipart;
mod scan;

use std::{path::PathBuf, time::Instant};

#[macro_use]
extern crate log;

use anyhow::{anyhow, Context};
use sqlx::{Pool, Postgres};
use warp::Filter;

use crate::scan::scan_top_level;

#[derive(Debug)]
struct InternalError(String);
impl warp::reject::Reject for InternalError {}

#[derive(Debug)]
struct AnyhowError(anyhow::Error);
impl warp::reject::Reject for AnyhowError {}

async fn startup_db() -> Pool<Postgres> {
    info!("Connecting to database");
    let host = std::env::var("DB_HOST").expect("DB_HOST");
    let user = std::env::var("DB_USER").expect("DB_USER");
    let password = std::env::var("DB_PW").expect("DB_PW");
    let db_name = std::env::var("DB_NAME").expect("DB_NAME");

    let db = format!("postgres://{}:{}@{}/{}", user, password, host, db_name);

    use sqlx::postgres::PgPoolOptions;
    let pool = PgPoolOptions::new()
        .max_connections(15)
        .connect(&db)
        .await
        .expect("Connect to db");
    info!("Connected to database");
    info!("Starting eventual database migrations");
    sqlx::migrate!("db/migrations")
        .run(&pool)
        .await
        .expect("Run migrations");
    info!("Migrations completed");
    pool
}

fn with_db_filter(
    con: Pool<Postgres>,
) -> impl Filter<Extract = (Pool<Postgres>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || con.clone())
}

async fn handle_libraries(con: Pool<Postgres>) -> Result<warp::reply::Json, warp::Rejection> {
    match db::libraries(&con).await {
        Ok(val) => Ok(warp::reply::json(&val)),
        Err(err) => Err(warp::reject::custom(AnyhowError(err))),
    }
}

fn best_content_type<'a>(accept: String) -> Result<Option<String>, warp::Rejection> {
    let mut best: (Option<&str>, f32) = (None, 0.0);
    for val in accept.split_terminator(',') {
        // can't get better than this
        if best.1 == 1.0 {
            break;
        }
        let full_val = val.trim();
        let (val, q) = if let Some(split_pos) = full_val.find(';') {
            let (val, q) = full_val.split_at(split_pos);
            let q = &q[3..];
            (val, q.parse::<f32>())
        } else {
            (full_val, Ok(1.0))
        };

        if let Err(err) = q {
            return Err(warp::reject::custom(AnyhowError(anyhow!(err))));
        } else if let Ok(q) = q {
            if q > best.1 {
                // is val something we support?
                let val = if  val == "application/*" || val == "application/json" {
                    Some("application/json")
                } else if val == "*/*" || val == "audio/*" || val == "audio/ogg" {
                    Some("audio/ogg")
                } else if val == "multipart/*" || val == "multipart/form-data" {
                    Some("multipart/form-data")
                } else {
                    None
                };
                if let Some(val) = val {
                    best = (Some(val), q);
                }
            }
        }
    }

    Ok(best.0.map(|s| s.to_owned()))
}

async fn handle_inline_tracks(
    con: Pool<Postgres>,
    album_id: i32,
) -> Result<warp::reply::Json, warp::Rejection> {
    match db::inline_tracks(&con, album_id).await {
        Ok(val) => Ok(warp::reply::json(&val)),
        Err(err) => Err(warp::reject::custom(AnyhowError(err))),
    }
}

async fn handle_browse_dir(
    con: Pool<Postgres>,
    lib_id: i32,
) -> Result<warp::reply::Json, warp::Rejection> {
    match db::browse_dir(&con, lib_id).await {
        Ok(val) => Ok(warp::reply::json(&val)),
        Err(err) => Err(warp::reject::custom(AnyhowError(err))),
    }
}

async fn handle_track(
    lib: String,
    con: Pool<Postgres>,
    track_id: String,
    accept: String,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    use db::TrackMetaData;
    enum Type {
        Track((String, TrackMetaData)),
        InlineTrack((String, i32, i32, TrackMetaData)),
    }

    let content_type: Option<String> = best_content_type(accept)?;
    if let Some(content_type) = content_type {
        let track = if let Some(pos) = track_id.find('-') {
            let (file, id) = track_id.split_at(pos);
            let file = match file
                .parse()
                .context("Error parsing fileId for inline track")
            {
                Ok(file) => Ok(file),
                Err(err) => Err(warp::reject::custom(AnyhowError(err))),
            }?;
            let id = match id[1..].parse().context("Error parsing id for inline track") {
                Ok(id) => Ok(id),
                Err(err) => Err(warp::reject::custom(AnyhowError(err))),
            }?;
            db::inline_track(&con, file, id)
                .await
                .map(Type::InlineTrack)
        } else {
            let track_id = match track_id.parse::<i32>().context("Failed to parse trackId") {
                Ok(val) => Ok(val),
                Err(err) => Err(warp::reject::custom(AnyhowError(err))),
            }?;
            db::track(&con, track_id).await.map(Type::Track)
        };
        match track {
            Ok(track) => {
                match track {
                    Type::Track((path, meta)) => {
                        if content_type == "application/json" {
                            Ok(Box::new(warp::reply::json(&meta)))
                        } else if content_type == "audio/ogg" {
                            use std::process::Stdio;
                            use tokio::process::Command;
                            use tokio_util::codec::{BytesCodec, FramedRead};
                            //TODO track_meta should contain last_modified for ETag
                            //     and we should check against it
                            let mut result = Command::new("ffmpeg")
                                .arg("-i")
                                .arg(format!("{}/{}", lib, path))
                                .arg("-vn")
                                .arg("-f")
                                .arg("ogg")
                                .arg("-")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                                .expect("spawn ffmpeg");

                            let stdout = result.stdout.take().expect("");
                            Ok(Box::new(
                                hyper::Response::builder()
                                    .header("Content-Type", "audio/ogg")
                                    .body(hyper::Body::wrap_stream(FramedRead::new(
                                        stdout,
                                        BytesCodec::new(),
                                    )))
                                    .unwrap(),
                            ))
                        } else if content_type == "multipart/form-data" {
                            use std::process::Stdio;
                            use tokio::process::Command;
                            use tokio_util::compat::TokioAsyncReadCompatExt;
                            //TODO track_meta should contain last_modified for ETag
                            //     and we should check against it
                            let mut result = Command::new("ffmpeg")
                                .arg("-i")
                                .arg(format!("{}/{}", lib, path))
                                .arg("-vn")
                                .arg("-f")
                                .arg("ogg")
                                .arg("-")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                                .expect("spawn ffmpeg");

                            let json = serde_json::to_vec(&meta)
                                .map_err(|err| warp::reject::custom(AnyhowError(anyhow!(err))))?;
                            let stdout = result.stdout.take().expect("");
                            Ok(Box::new(
                                hyper::Response::builder()
                                    .header("Content-Type", "multipart/form-data;boundary=\"asdf\"")
                                    .body(hyper::Body::wrap_stream(
                                        multipart::MultipartStream::new(
                                            "asdf",
                                            vec![
                                                multipart::Part::new(
                                                    multipart::PartBody::Simple(json.into()),
                                                    "meta",
                                                ),
                                                multipart::Part::new(
                                                    multipart::PartBody::Read(Box::pin(
                                                        stdout.compat(),
                                                    )),
                                                    "track",
                                                ),
                                            ],
                                        ),
                                    ))
                                    .unwrap(),
                            ))
                        } else {
                            unreachable!()
                        }
                    }
                    Type::InlineTrack((path, start, end, meta)) => {
                        if content_type == "application/json" {
                            Ok(Box::new(warp::reply::json(&meta)))
                        } else if content_type == "audio/ogg" {
                            use std::process::Stdio;
                            use tokio::process::Command;
                            use tokio_util::codec::{BytesCodec, FramedRead};
                            //TODO track_meta should contain last_modified for ETag
                            //     and we should check against it

                            let mut start = start.to_string();
                            start.push_str("ms");
                            let mut end = end.to_string();
                            end.push_str("ms");
                            let mut result = Command::new("ffmpeg")
                                .arg("-ss")
                                .arg(start)
                                .arg("-to")
                                .arg(end)
                                .arg("-i")
                                .arg(format!("{}/{}", lib, path))
                                .arg("-vn")
                                .arg("-f")
                                .arg("ogg")
                                .arg("-")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                                .expect("spawn ffmpeg");

                            let stdout = result.stdout.take().expect("");
                            Ok(Box::new(
                                hyper::Response::builder()
                                    .header("Content-Type", "audio/ogg")
                                    .body(hyper::Body::wrap_stream(FramedRead::new(
                                        stdout,
                                        BytesCodec::new(),
                                    )))
                                    .unwrap(),
                            ))
                        } else if content_type == "multipart/form-data" {
                            use std::process::Stdio;
                            use tokio::process::Command;
                            use tokio_util::compat::TokioAsyncReadCompatExt;
                            //TODO track_meta should contain last_modified for ETag
                            //     and we should check against it

                            let mut start = start.to_string();
                            start.push_str("ms");
                            let mut end = end.to_string();
                            end.push_str("ms");
                            let mut result = Command::new("ffmpeg")
                                .arg("-ss")
                                .arg(start)
                                .arg("-to")
                                .arg(end)
                                .arg("-i")
                                .arg(format!("{}/{}", lib, path))
                                .arg("-vn")
                                .arg("-f")
                                .arg("ogg")
                                .arg("-")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                                .expect("spawn ffmpeg");

                            let json = serde_json::to_vec(&meta)
                                .map_err(|err| warp::reject::custom(AnyhowError(anyhow!(err))))?;
                            let stdout = result.stdout.take().expect("");
                            Ok(Box::new(
                                hyper::Response::builder()
                                    .header("Content-Type", "multipart/form-data;boundary=\"asdf\"")
                                    .body(hyper::Body::wrap_stream(
                                        multipart::MultipartStream::new(
                                            "asdf",
                                            vec![
                                                multipart::Part::new(
                                                    multipart::PartBody::Simple(json.into()),
                                                    "meta",
                                                ),
                                                multipart::Part::new(
                                                    multipart::PartBody::Read(Box::pin(
                                                        stdout.compat(),
                                                    )),
                                                    "track",
                                                ),
                                            ],
                                        ),
                                    ))
                                    .unwrap(),
                            ))
                        } else {
                            unreachable!()
                        }
                    }
                }
            }
            Err(err) => Err(warp::reject::custom(AnyhowError(err))),
        }
    } else {
        Ok(Box::new(
            hyper::Response::builder()
                .status(hyper::StatusCode::NOT_ACCEPTABLE)
                .body::<&str>(
                    "Client does not accept: application/json, audio/ogg or multipart/form-data",
                )
                .unwrap(),
        ))
    }
}

fn route(con: Pool<Postgres>, lib: String) -> impl Filter<Extract = impl warp::Reply> + Clone {
    let cors = warp::cors().allow_any_origin();

    let libraries = warp::path("libraries")
        .and(with_db_filter(con.clone()))
        .and_then(handle_libraries);
    let browse_dir = warp::path("directory")
        .and(with_db_filter(con.clone()))
        .and(warp::path::param())
        .and_then(handle_browse_dir);
    let inline_tracks = warp::path("inlineTracks")
        .and(with_db_filter(con.clone()))
        .and(warp::path::param())
        .and_then(handle_inline_tracks);

    let track = warp::path("track")
        .map(move || lib.clone())
        .and(with_db_filter(con))
        .and(warp::path::param())
        .and(warp::header("accept"))
        .and_then(handle_track);

    warp::get()
        .and(browse_dir)
        .or(inline_tracks)
        .or(libraries)
        .or(track)
        .with(cors)
}

fn get_env_var(key: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(key) {
        Ok(val) => Ok(Some(val)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(anyhow::Error::new(e)),
    }
}

fn main() {
    info!("Soil starting up");
    let listen: std::net::SocketAddr = std::env::var("LISTEN")
        .expect("LISTEN")
        .parse()
        .expect("Invalid listen address");

    env_logger::init();

    let library = get_env_var("LIB").expect("LIB").expect("LIB");
    let rt = tokio::runtime::Runtime::new().expect("Tokio runtime");
    let pool = rt.block_on(startup_db());
    let route = route(pool.clone(), library.clone());
    let _ = rt.block_on(async {
        let library = PathBuf::from(library);
        let start = Instant::now();
        let scan_result = scan_top_level(&pool, library)
            .await
            .expect("Scanning library");
        for result in scan_result {
            if let Err(err) = result {
                info!("Error while scanning. {}", err);
            }
        }
        info!("Completed library scan after {:?}", start.elapsed());
        warp::serve(route).run(listen).await;
    });
}
