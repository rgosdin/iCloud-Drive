use anyhow::{anyhow, Context, Result};
use futures::{stream::FuturesUnordered, Future, FutureExt, StreamExt};
use sqlx::{Pool, Postgres};
use std::{
    collections::HashMap,
    convert::TryFrom,
    fs::Metadata,
    path::{Path, PathBuf},
    pin::Pin,
    time::SystemTime,
};
use tokio::fs::{metadata, read_dir};

use crate::db::{
    cached_files, insert_file, insert_multi_file_album_track, insert_single_file_album, refresh_db,
    remove_file, update_file, update_multi_file_album_track, update_single_file_album,
    FileCacheEntry,
};

pub async fn scan_top_level(
    con: &Pool<Postgres>,
    base_path: PathBuf,
) -> Result<Vec<Result<PathBuf>>> {
    let mut contents = read_dir(&base_path)
        .await
        .with_context(|| format!("Reading top level directory {:?}", &base_path))?;

    let mut known_files = cached_files(con)
        .await
        .context("Reading cached files from DB")?;

    let mut ret = Vec::new();
    loop {
        match contents.next_entry().await {
            Ok(entry) => {
                if let Some(entry) = entry {
                    let path = entry.path();
                    let meta = metadata(&path).await;
                    match meta {
                        Ok(meta) => {
                            if meta.is_dir() {
                                debug!("Entering top level directory {:?}", &path);
                                let path_stripped = path.strip_prefix(&base_path)?;
                                let id =
                                    if let Some(cache_entry) = known_files.remove(path_stripped) {
                                        trace!("Cached entry {:?} for {:?}", &cache_entry, &path);
                                        cache_entry.id
                                    } else {
                                        debug!("New top level directory: {:?}", &path);
                                        insert_file(&con, path_stripped, true, false, None, None)
                                            .await?
                                    };
                                match scan_recursive(con, &base_path, id, &path, &mut known_files)
                                    .await
                                {
                                    Ok(..) => {
                                        ret.push(Ok(path));
                                    }
                                    Err(err) => {
                                        ret.push(Err(anyhow!(err)));
                                    }
                                };
                            } else {
                                debug!("Top level entry is not a directory {:?}", &path);
                            }
                        }
                        Err(err) => {
                            ret.push(Err(anyhow!(err)));
                        }
                    }
                } else {
                    debug!("No more top level entries");
                    break;
                }
            }
            Err(err) => {
                ret.push(Err(anyhow!(err)));
            }
        }
    }

    let mut futs = FuturesUnordered::new();
    for cached in known_files.values() {
        futs.push(remove_file(&con, cached.id));
    }

    while let Some(result) = futs.next().await {
        result.context("Deleting outdated cache entries")?;
    }

    refresh_db(con).await?;

    Ok(ret)
}

fn scan_recursive<'a>(
    con: &'a Pool<Postgres>,
    base_path: &'a Path,
    parent: i32,
    path: &'a Path,
    known_files: &'a mut HashMap<PathBuf, FileCacheEntry>,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    trace!("Scanning {:?}", path);
    async move {
        let mut contents = tokio::fs::read_dir(path)
            .await
            .with_context(|| format!("Error reading library {:?}", path))?;

        while let Some(entry) = contents
            .next_entry()
            .await
            .context("Could not read directory entry")?
        {
            let path = entry.path();
            let meta = metadata(&path)
                .await
                .with_context(|| format!("Could not get metadata of {:?}", &path));
            if let Ok(meta) = meta {
                let path_stripped = path.strip_prefix(base_path)?;
                let cache_entry = known_files.remove(path_stripped);
                trace!("Cached entry {:?} for {:?}", &cache_entry, &path);
                if meta.is_dir() {
                    let id = if let Some(cache_entry) = cache_entry {
                        Ok(cache_entry.id)
                    } else {
                        debug!("New directory: {:?}", &path);
                        insert_file(&con, path_stripped, true, false, None, Some(parent)).await
                    };
                    if let Ok(id) = id {
                        if let Err(err) =
                            scan_recursive(con, base_path, id, &path, known_files).await
                        {
                            info!("Error while recursing into {:?} {}", &path, err);
                        } else {
                            trace!("Done recursing into {:?}", &path);
                        };
                    } else {
                        info!("Error while inserting {:?} {}", &path, id.unwrap_err());
                    }
                } else if let Err(err) =
                    handle_file(con, &path, base_path, parent, meta, cache_entry).await
                {
                    info!("Error while processing {:?} {}", path, err);
                };
            } else {
                info!(
                    "Error while getting metadata for {:?} {}",
                    &path,
                    meta.unwrap_err()
                );
            }
        }

        Ok(())
    }
    .boxed()
}

async fn handle_file(
    con: &Pool<Postgres>,
    path: &Path,
    base_path: &Path,
    parent: i32,
    meta: Metadata,
    cached: Option<FileCacheEntry>,
) -> Result<()> {
    trace!("Deciding what to do with {:?}", &path);

    #[derive(Debug)]
    enum State {
        New,
        Modified(FileCacheEntry),
    }

    let modified = meta
        .modified()
        .ok()
        .and_then(|st| st.duration_since(SystemTime::UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok());
    let state = if let Some(cached) = cached {
        if modified != cached.last_modified {
            State::Modified(cached)
        } else {
            return Ok(());
        }
    } else {
        State::New
    };

    if let Some(ext) = path.extension() {
        let path_stripped = path.strip_prefix(base_path)?;
        if ext == "jpg" || ext == "jpeg" || ext == "png" {
            if let State::Modified(cache) = state {
                debug!("Updating modified Cover {:?}", path);
                update_file(con, cache.id, modified, false, true).await?;
            } else {
                debug!("Inserting new Cover {:?}", path);
                insert_file(con, path_stripped, false, true, None, Some(parent)).await?;
            }
        } else if ext == "m4b" {
            match meta::read_m4b(path).await {
                Ok(album) => {
                    if let State::Modified(cache) = state {
                        update_single_file_album(con, cache.id, modified, album).await?;
                    } else {
                        insert_single_file_album(con, path_stripped, parent, modified, album)
                            .await?;
                    }
                }
                Err(err) => info!("Error reading audio book {:?} {}", path, err),
            }
        } else if ext == "mp3" || ext == "ogg" || ext == "oga" || ext == "wv" || ext == "flac" {
            match meta::read_track(path).await {
                Ok(track) => {
                    if let State::Modified(cache) = state {
                        update_multi_file_album_track(con, cache.id, modified, track).await?;
                    } else {
                        insert_multi_file_album_track(con, path_stripped, parent, modified, track)
                            .await?;
                    }
                }
                Err(err) => info!("Error reading track {:?} {}", path, err),
            }
        }
    } else {
        debug!("Could not get extension of {:?}. Skipping", &path);
    };

    Ok(())
}

mod meta {
    use anyhow::{anyhow, bail, Context, Result};
    use serde_json::{de::from_slice, Value};
    use std::{convert::TryInto, path::Path, process::Stdio};
    use tokio::{io::AsyncReadExt, process::Command};

    use crate::db::{InlineTrack, SingleFileAlbum, Track};

    pub async fn read_m4b(path: &Path) -> Result<SingleFileAlbum> {
        let mut child = Command::new("ffprobe")
            .arg("-print_format")
            .arg("json=c=true")
            .arg("-show_format")
            .arg("-show_chapters")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Can not read file with ffprobe")?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Could not get stdout from ffprobe"))?;
        let mut json = Vec::new();
        stdout.read_to_end(&mut json).await?;
        let mut json = from_slice::<Value>(&json)?;
        let mut tags = if let Some(format_json) = json.get_mut("format") {
            format_json
                .get_mut("tags")
                .map(Value::take)
                .ok_or_else(|| anyhow!("ffprobe json is missing \"tags\""))?
        } else {
            bail!("ffprobe json is missing \"format\"");
        };

        let title = take_title(&mut tags)?;

        if let Some(chapters_json) = json.get_mut("chapters") {
            let chapters_json = chapters_json
                .as_array_mut()
                .ok_or_else(|| anyhow!("chapters is not an array in json"))?;
            let mut tracks = Vec::new();
            for mut chapter in chapters_json {
                let mut tags = chapter
                    .get_mut("tags")
                    .map(Value::take)
                    .ok_or_else(|| anyhow!("ffprobe json is missing \"tags\""))?;
                tracks.push(InlineTrack {
                    start: get_i32(&mut chapter, "start")?,
                    end: get_i32(&mut chapter, "end")?,
                    title: take_title(&mut tags)?,
                });
            }
            Ok(SingleFileAlbum {
                artist: None,
                title,
                tracks,
            })
        } else {
            bail!("ffprobe json is missing \"chapters\"");
        }
    }

    pub async fn read_track(path: &Path) -> Result<Track> {
        let mut child = Command::new("ffprobe")
            .arg("-print_format")
            .arg("json=c=true")
            .arg("-show_format")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Can not read file with ffproble")?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Could not get stdout from ffprobe"))?;
        let mut json = Vec::new();
        stdout.read_to_end(&mut json).await?;
        let mut json = from_slice::<Value>(&json)?;

        let (duration, mut tags) = if let Some(mut format_json) = json.get_mut("format") {
            let duration = take_string(&mut format_json, "duration")?;
            let duration: f64 = duration
                .parse()
                .context("Could not parse duration as f64")?;
            if !duration.is_finite() {
                bail!("Invalid duration from ffprobe {}", duration);
            }
            let duration = duration.ceil() as u64;
            let duration: i32 = duration
                .try_into()
                .context("Convert ffprobe duration to i32")?;
            let tags = format_json
                .get_mut("tags")
                .map(Value::take)
                .ok_or_else(|| anyhow!("ffprobe json is missing \"tags\""))?;
            Ok((duration, tags))
        } else {
            Err(anyhow!("ffprobe json is missing \"format\""))
        }?;

        let album_artist = take_album_artist(&mut tags)?;
        let album_title = take_string(&mut tags, "album")?;
        let title = take_title(&mut tags)?;
        let position = get_track_number(&mut tags)?;
        let disc = get_i16_from_maybe_split_field(&mut tags, "disc")
            .ok()
            .unwrap_or(1);

        Ok(Track {
            album_artist,
            album_title,
            title,
            disc,
            position,
            duration,
        })
    }

    fn take_title(value: &mut Value) -> Result<String> {
        take_string(value, "title")
    }

    fn take_album_artist(value: &mut Value) -> Result<String> {
        if let Ok(v) = take_string(value, "album artist") {
            Ok(v)
        } else if let Ok(v) = take_string(value, "albumartist") {
            Ok(v)
        } else if let Ok(v) = take_string(value, "album_artist") {
            Ok(v)
        } else {
            Err(anyhow!(
                "No \"album artist\", \"albumartist\" or \"album_artist\" tag"
            ))
        }
    }

    fn get_track_number(value: &mut Value) -> Result<i16> {
        if let Ok(v) = get_i16_from_maybe_split_field(value, "track") {
            Ok(v)
        } else if let Ok(v) = get_i16_from_maybe_split_field(value, "tracknumber") {
            Ok(v)
        } else {
            Err(anyhow!("No \"track\" or \"tracknumber\" tag"))
        }
    }

    fn get_i16_from_maybe_split_field(value: &mut Value, key: &str) -> Result<i16> {
        let parts = take_string(value, key)?;
        let parts = parts.split('/').collect::<Vec<&str>>();
        parts
            .first()
            .map(|l| l.parse::<i16>())
            .and_then(Result::ok)
            .with_context(|| format!("Can't convert {} to i16", key))
    }

    fn get_i32(value: &mut Value, key: &str) -> Result<i32> {
        let ret = get_mut_ignore_case(value, key)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("ffprobe did not supply {} in json", key))?;
        let ret: i32 = ret
            .try_into()
            .with_context(|| format!("Convert ffprobe {} to i32", key))?;
        Ok(ret)
    }

    fn take_string(value: &mut Value, key: &str) -> Result<String> {
        let ret = get_mut_ignore_case(value, key)
            .ok_or_else(|| anyhow!("ffprobe did not supply {} in json", key))?;
        if ret.is_string() {
            if let Value::String(s) = ret.take() {
                Ok(s)
            } else {
                unreachable!()
            }
        } else {
            bail!("ffprobe did not supply string in field {}", key);
        }
    }

    fn get_mut_ignore_case<'a>(value: &'a mut Value, key: &'_ str) -> Option<&'a mut Value> {
        if let Value::Object(map) = value {
            for (k, v) in map.iter_mut() {
                if k.to_lowercase() == key.to_lowercase() {
                    return Some(v);
                }
            }
        }
        None
    }
}
