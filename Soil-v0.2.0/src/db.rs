use futures::{
    stream::{FuturesUnordered, TryStreamExt},
    try_join, StreamExt,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sqlx::{query, query_scalar, types::chrono::NaiveDateTime, Pool, Postgres};

#[derive(Serialize, Debug)]
pub struct DirectoryListingResponse {
    pub entries: Vec<DirectoryEntryResponse>,
    pub parent: Option<DirectoryEntryResponse>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryResponse {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub entry_type: DirectoryEntryType,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DirectoryEntryType {
    Dir,
    AudioFile,
    InlineAudioFiles,
    InlineAudioFile,
    Img,
}

#[derive(Serialize, Debug)]
pub struct SingleInlineTrackMetaData {
    pub id: String,
    pub title: String,
    pub duration: i32,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrackMetaData {
    duration: i32,
    title: String,
    album_title: String,
    artist_name: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct FullAlbum {
    pub id: i32,
    pub title: String,
    pub artist: Option<String>,
    pub covers: Vec<i32>,
    pub tracks: Vec<AlbumSubDivisionResponse>,
}

#[derive(Serialize, Debug)]
pub struct AlbumSubDivisionResponse {
    tracks: Vec<TrackResponse>,
}

#[derive(Serialize, Debug)]
pub struct TrackResponse {
    pub id: i32,
    pub title: String,
    pub duration: i32,
}

#[derive(Serialize, Debug)]
pub struct DirectoryResponse {
    pub id: i32,
    pub title: String,
    pub covers: Vec<i32>,
}

#[derive(Serialize, Debug)]
pub struct AlbumResponse {
    pub id: i32,
    pub title: String,
    pub artist: Option<String>,
    pub covers: Vec<i32>,
}

#[derive(Debug)]
pub struct FileCacheEntry {
    pub id: i32,
    // seconds since UNIX_TIME
    pub last_modified: Option<i64>,
}

#[derive(Debug)]
pub struct SingleFileAlbum {
    pub artist: Option<String>,
    pub title: String,
    pub tracks: Vec<InlineTrack>,
}

#[derive(Debug)]
pub struct InlineTrack {
    pub start: i32,
    pub end: i32,
    pub title: String,
}

#[derive(Debug)]
pub struct Track {
    pub album_artist: String,
    pub album_title: String,
    pub title: String,
    pub disc: i16,
    pub position: i16,
    pub duration: i32,
}

pub async fn cached_files(con: &Pool<Postgres>) -> Result<HashMap<PathBuf, FileCacheEntry>> {
    Ok(query!(
        r#"
SELECT fp.id AS "id!", fp.path AS "path!", f.last_modified
FROM file_paths fp
INNER JOIN files f ON f.id = fp.id
       "#
    )
    .fetch(con)
    .try_fold(HashMap::new(), |mut acc, r| async move {
        acc.insert(
            PathBuf::from(r.path),
            FileCacheEntry {
                id: r.id,
                last_modified: r.last_modified.as_ref().map(NaiveDateTime::timestamp),
            },
        );
        Ok(acc)
    })
    .await?)
}

pub async fn insert_file(
    con: &Pool<Postgres>,
    path: &Path,
    is_directory: bool,
    is_cover: bool,
    last_modified: Option<i64>,
    parent: Option<i32>,
) -> Result<i32> {
    let path = path
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or_else(|| anyhow!("Path is not valid utf8"))?;
    Ok(query_scalar!(
        "
INSERT INTO files (parent, name, last_modified, is_directory, is_cover)
VALUES            ($1    , $2  , $3           , $4          , $5)
RETURNING id
                  ",
        parent,
        path,
        last_modified.and_then(|lm| NaiveDateTime::from_timestamp_opt(lm, 0)),
        is_directory,
        is_cover
    )
    .fetch_one(con)
    .await?)
}

pub async fn update_file(
    con: &Pool<Postgres>,
    file: i32,
    last_modified: Option<i64>,
    is_directory: bool,
    is_cover: bool,
) -> Result<()> {
    query!(
        "
UPDATE files
SET last_modified = $2, is_directory = $3, is_cover = $4
WHERE id = $1
                  ",
        file,
        last_modified.and_then(|lm| NaiveDateTime::from_timestamp_opt(lm, 0)),
        is_directory,
        is_cover
    )
    .execute(con)
    .await?;

    Ok(())
}

pub async fn refresh_db(con: &Pool<Postgres>) -> Result<()> {
    //TODO also deletes images higher up in hierarchy. fix this
    query!(
        "
WITH RECURSIVE
  audio_leafs(id, parent) AS (
    SELECT id, parent
    FROM files
    WHERE id IN (
        SELECT file
        FROM album_tracks
      UNION
        SELECT file
        FROM albums
    )
  ),
  files_with_albums(id, parent) AS (
      SELECT id, parent
      FROM files
      WHERE
        parent IN (SELECT parent FROM audio_leafs) AND
        NOT is_directory
    UNION
      SELECT files.id, files.parent
      FROM files
      INNER JOIN files_with_albums AS childs ON childs.parent = files.id
  )
DELETE FROM files
WHERE id NOT IN (
  SELECT id FROM files_with_albums
)"
    )
    .execute(con)
    .await?;

    query!(
        "
REFRESH MATERIALIZED VIEW file_paths
    "
    )
    .execute(con)
    .await?;

    Ok(())
}

pub async fn insert_single_file_album(
    con: &Pool<Postgres>,
    path: &Path,
    parent: i32,
    last_modified: Option<i64>,
    album: SingleFileAlbum,
) -> Result<()> {
    let tx = con.begin().await?;
    let file_id = insert_file(con, path, false, false, last_modified, Some(parent)).await?;
    let album_id = upsert_album(con, None, &album.title, Some(file_id)).await?;
    insert_inline_tracks(con, album_id, album.tracks).await?;

    Ok(tx.commit().await?)
}

async fn upsert_album(
    con: &Pool<Postgres>,
    artist: Option<i32>,
    title: &str,
    file: Option<i32>,
) -> Result<i32> {
    let id = if let Some(file) = file {
        query_scalar!(
            "
WITH
input(artist, title, file) AS (VALUES ($1::INTEGER, $2::TEXT, $3::INTEGER)),
ins AS (
    INSERT INTO albums (artist, title, file)
    SELECT * FROM input
    ON CONFLICT (artist, title, file) WHERE file IS NOT NULL DO NOTHING
    RETURNING id
)
SELECT COALESCE (
    (SELECT id FROM ins),
    (SELECT id FROM albums WHERE artist = $1 AND title = $2 AND file = $3)
)",
            artist,
            title,
            file
        )
        .fetch_optional(con)
    } else {
        query_scalar!(
            "
WITH
input(artist, title) AS (VALUES ($1::INTEGER, $2::TEXT)),
ins AS (
    INSERT INTO albums (artist, title)
    SELECT * FROM input
    ON CONFLICT (artist, title) WHERE file IS NULL DO NOTHING
    RETURNING id
)
SELECT COALESCE (
    (SELECT id FROM ins),
    (SELECT id FROM albums WHERE artist = $1 AND title = $2 AND file IS NULL)
)",
            artist,
            title
        )
        .fetch_optional(con)
    }
    .await
    .context("Inserting album")?;

    let id = id.flatten();

    if let Some(id) = id {
        Ok(id)
    } else {
        // Race condition. Someone inserted and commited
        // after we started the query. Since we are READ COMITTED
        // we can't see the id in the same query.
        // We need to execute a seperate query to see the changes.
        Ok(sqlx::query_scalar!(
            "
SELECT id FROM albums WHERE artist = $1 AND title = $2 AND file = $3",
            artist,
            title,
            file
        )
        .fetch_one(con)
        .await
        .context("Selecting album after race condition")?)
    }
}

async fn insert_inline_tracks(
    con: &Pool<Postgres>,
    album: i32,
    tracks: Vec<InlineTrack>,
) -> Result<()> {
    let mut futs = FuturesUnordered::new();
    for track in tracks {
        futs.push(
            query!(
                r#"
INSERT INTO album_inline_tracks (album, start, "end", title)
VALUES                          ($1   , $2   , $3   , $4   )"#,
                album,
                track.start,
                track.end,
                track.title
            )
            .execute(con),
        )
    }

    while let Some(result) = futs.next().await {
        result.context("Inserting album chapters")?;
    }

    Ok(())
}

pub async fn update_single_file_album(
    con: &Pool<Postgres>,
    file: i32,
    last_modified: Option<i64>,
    album: SingleFileAlbum,
) -> Result<()> {
    let tx = con.begin().await?;
    try_join!(
        update_file(con, file, last_modified, false, false),
        delete_album_by_file(con, file)
    )?;

    let album_id = upsert_album(con, None, &album.title, Some(file)).await?;
    insert_inline_tracks(con, album_id, album.tracks).await?;

    Ok(tx.commit().await?)
}

async fn delete_album_by_file(con: &Pool<Postgres>, file: i32) -> Result<()> {
    query!(
        "
DELETE FROM albums
WHERE file = $1",
        file
    )
    .execute(con)
    .await?;

    Ok(())
}

pub async fn insert_multi_file_album_track(
    con: &Pool<Postgres>,
    path: &Path,
    parent: i32,
    last_modified: Option<i64>,
    track: Track,
) -> Result<()> {
    let tx = con.begin().await?;
    let (file_id, artist_id) = try_join!(
        insert_file(con, path, false, false, last_modified, Some(parent)),
        upsert_artist(con, &track.album_artist)
    )?;

    let album_id = upsert_album(con, Some(artist_id), &track.album_title, None).await?;

    upsert_album_tracks(con, file_id, album_id, track).await?;

    Ok(tx.commit().await?)
}

async fn upsert_album_tracks(
    con: &Pool<Postgres>,
    file_id: i32,
    album_id: i32,
    track: Track,
) -> Result<()> {
    query!(
        "
  INSERT INTO album_tracks (file, album, album_disc, title, position, duration)
  VALUES                   ($1  , $2   , $3        , $4   , $5      , $6      )
ON CONFLICT (file) DO
  UPDATE SET
    album = excluded.album,
    album_disc = excluded.album_disc,
    title = excluded.title,
    position = excluded.position,
    duration = excluded.duration
  WHERE album_tracks.file = excluded.file",
        file_id,
        album_id,
        track.disc,
        track.title,
        track.position,
        track.duration
    )
    .execute(con)
    .await?;

    Ok(())
}

pub async fn update_multi_file_album_track(
    con: &Pool<Postgres>,
    file: i32,
    last_modified: Option<i64>,
    track: Track,
) -> Result<()> {
    let tx = con.begin().await?;
    let (_, artist_id) = try_join!(
        update_file(con, file, last_modified, false, false),
        upsert_artist(con, &track.album_artist)
    )?;

    let album_id = upsert_album(con, Some(artist_id), &track.album_title, None).await?;
    upsert_album_tracks(con, file, album_id, track).await?;

    Ok(tx.commit().await?)
}

async fn upsert_artist(con: &Pool<Postgres>, name: &str) -> anyhow::Result<i32> {
    let id = sqlx::query_scalar!(
        "
WITH
input(name) AS (VALUES ($1::TEXT)),
ins AS (
    INSERT INTO artists (name)
    SELECT * FROM input
    ON CONFLICT (name) DO NOTHING
    RETURNING id
)
SELECT COALESCE (
    (SELECT id FROM ins),
    (SELECT id FROM artists WHERE name = $1)
)",
        name
    )
    .fetch_optional(con)
    .await
    .context("inserting artist")?;
    let id = id.flatten();

    if let Some(id) = id {
        Ok(id)
    } else {
        // Race condition. Someone inserted and commited
        // after we started the query. Since we are READ COMITTED
        // we can't see the id in the same query.
        // We need to execute a seperate query to see the changes.
        Ok(sqlx::query_scalar!(
            "
SELECT id FROM artists WHERE name = $1",
            name
        )
        .fetch_one(con)
        .await
        .context("Selecting artist after race condition")?)
    }
}

pub async fn remove_file(con: &Pool<Postgres>, file: i32) -> Result<()> {
    query!(
        "
DELETE FROM files
WHERE id = $1
           ",
        file
    )
    .execute(con)
    .await?;

    Ok(())
}

pub async fn inline_tracks(con: &Pool<Postgres>, album: i32) -> Result<DirectoryListingResponse> {
    query!(
        r#"
SELECT *
FROM (
    SELECT
        CAST (ait.album AS TEXT) || '-' || CAST (ait.start AS TEXT) AS "id!",
        ait.title AS "title!",
        false AS "is_parent!",
        ait.start AS "track_start?"
    FROM
        files f
        INNER JOIN albums ab ON ab.file = $1
        INNER JOIN album_inline_tracks ait ON ait.album = ab.id
    WHERE f.id = $1
UNION ALL
    SELECT
        CAST (f.parent AS TEXT) AS "id!",
        (SELECT name FROM files WHERE id = f.parent) AS "title!",
        true AS "is_parent",
        NULL AS "track_start?"
    FROM
        files f
    WHERE
        f.id = $1
) AS t
ORDER BY t."track_start?"
        "#,
        album
    )
    .fetch(con)
    .try_fold(
        DirectoryListingResponse {
            entries: Vec::new(),
            parent: None,
        },
        |mut acc, r| async move {
            let mut elem = DirectoryEntryResponse {
                id: r.id.to_string(),
                title: r.title,
                entry_type: DirectoryEntryType::InlineAudioFile
            };
            if r.is_parent {
                assert!(
                    acc.parent.is_none(),
                    "DirectoryEntry should only have one parent"
                );
                elem.entry_type = DirectoryEntryType::Dir;
                acc.parent = Some(elem)
            } else {
                acc.entries.push(elem);
            }
            Ok(acc)
        },
    )
    .await
    .context("Fetching inline track contents for browsing")
}

pub async fn inline_track(
    con: &Pool<Postgres>,
    album: i32,
    start: i32,
) -> Result<(String, i32, i32, TrackMetaData)> {
    query!(
        r#"
SELECT
    ((ait.end - ait.start) / 1000) AS "duration!",
    ait.title AS "track_title",
    ait.start AS "track_start",
    ait.end AS "track_end",
    ab.title AS "album_title",
    ar.name AS "artist_name?",
    fp.path AS "path!"
FROM
    album_inline_tracks ait
    INNER JOIN albums ab ON ab.id = $1
    INNER JOIN file_paths fp ON fp.id = ab.file
    LEFT JOIN artists ar ON ar.id = ab.artist
WHERE
    ait.album = $1 AND
    ait.start = $2
        "#,
        album,
        start
    )
    .fetch_one(con)
    .await
    .map(|row| {
        (row.path, row.track_start, row.track_end, TrackMetaData {
            duration: row.duration,
            title: row.track_title,
            album_title: row.album_title,
            artist_name: row.artist_name,
        })
    }).context("Error fetching inline track data")
}

pub async fn track(con: &Pool<Postgres>, id: i32) -> Result<(String, TrackMetaData)> {
    let tst = query!(
        r#"
SELECT
    at.duration,
    at.title,
    ab.title AS "album_title",
    ar.name AS "artist_name?",
    path AS "path!"
FROM
    album_tracks at
    INNER JOIN file_paths fp ON at.file = fp.id
    INNER JOIN albums ab ON at.album = ab.id
    LEFT JOIN artists ar ON ab.artist = ar.id
WHERE at.id = $1
        "#,
        id
    )
    .fetch_one(con)
    .await?;
    let file = tst.path;

    Ok((
        file,
        TrackMetaData {
            duration: tst.duration,
            title: tst.title,
            album_title: tst.album_title,
            artist_name: tst.artist_name,
        },
    ))
}

//TODO return correct IDs or change track endpoint
pub async fn browse_dir(con: &Pool<Postgres>, id: i32) -> Result<DirectoryListingResponse> {
    query!(
        r#"
WITH files_in_dir AS (
    SELECT
        f.id AS "id!",
        name AS "name!",
        is_directory AS "is_directory!",
        is_cover AS "is_cover!",
        CASE
            WHEN EXISTS (
                SELECT
                FROM album_tracks
                WHERE album_tracks.file = f.id
            ) THEN true
            ELSE false
        END AS "is_standalone_track!",
        false AS "is_parent!"
    FROM files f
    WHERE parent = $1
    UNION
    SELECT
        id AS "id!",
        name AS "name!",
        is_directory AS "is_directory!",
        is_cover AS "is_cover!",
        CASE
            WHEN EXISTS (
                SELECT
                FROM album_tracks
                WHERE album_tracks.file = f.id
            ) THEN true
            ELSE false
        END AS "is_standalone_track!",
        true AS "is_parent!"
    FROM files f
    WHERE id = (SELECT parent FROM files WHERE id = $1)
)
SELECT
    files_in_dir."id!",
    files_in_dir."name!",
    files_in_dir."is_directory!",
    files_in_dir."is_cover!",
    files_in_dir."is_standalone_track!",
    files_in_dir."is_parent!",
    album_tracks.id AS "track_id?"
FROM files_in_dir
LEFT JOIN album_tracks
  ON album_tracks.file = "id!"
ORDER BY files_in_dir."name!"
        "#,
        id
    )
    .fetch(con)
    .try_fold(
        DirectoryListingResponse {
            entries: Vec::new(),
            parent: None,
        },
        |mut acc, r| async move {
            let elem = DirectoryEntryResponse {
                id: r.track_id.unwrap_or(r.id).to_string(),
                title: r.name,
                entry_type: if r.is_directory {
                    DirectoryEntryType::Dir
                } else if r.is_cover {
                    DirectoryEntryType::Img
                } else if r.is_standalone_track {
                    DirectoryEntryType::AudioFile
                } else {
                    DirectoryEntryType::InlineAudioFiles
                },
            };
            if r.is_parent {
                assert!(
                    acc.parent.is_none(),
                    "DirectoryEntry should only have one parent"
                );
                acc.parent = Some(elem)
            } else {
                acc.entries.push(elem);
            }
            Ok(acc)
        },
    )
    .await
    .context("Fetching directory contents for browsing")
}

pub async fn libraries(con: &Pool<Postgres>) -> Result<Vec<DirectoryEntryResponse>> {
    query!(
        r#"
SELECT id, name
FROM files
WHERE
  parent IS NULL
  AND is_directory"#
    )
    .fetch(con)
    .map_ok(|r| DirectoryEntryResponse {
        id: r.id.to_string(),
        title: r.name,
        entry_type: DirectoryEntryType::Dir,
    })
    .try_collect()
    .await
    .context("Fetching libs from DB")
}
