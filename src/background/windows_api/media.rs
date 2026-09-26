use base64::Engine;
use serde::{Deserialize, Serialize};
use windows::{
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    },
    Storage::Streams::DataReader,
};

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub source_app: String,
    /// `data:` url of the album art, if any
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaCommand {
    TogglePlayPause,
    Next,
    Previous,
}

pub fn manager() -> Result<SessionManager> {
    Ok(SessionManager::RequestAsync()?.join()?)
}

/// Reads the album art. Expensive, so callers should only do it when the track changes.
fn read_thumbnail(session: &Session) -> Option<String> {
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let reference = props.Thumbnail().ok()?;
    let stream = reference.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 4 * 1024 * 1024 {
        return None;
    }
    let content_type = stream
        .ContentType()
        .map(|c| c.to_string())
        .unwrap_or_default();
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size).ok()?.join().ok()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    let mime = if content_type.is_empty() {
        "image/png".to_string()
    } else {
        content_type
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Returns the current media session. `previous` lets us reuse the thumbnail
/// when the track did not change.
pub fn current(
    manager: &SessionManager,
    previous: Option<&MediaInfo>,
) -> Result<Option<MediaInfo>> {
    let Ok(session) = manager.GetCurrentSession() else {
        return Ok(None);
    };
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let title = props.Title()?.to_string();
    let artist = props.Artist()?.to_string();
    let playing = session
        .GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .map(|s| s == PlaybackStatus::Playing)
        .unwrap_or(false);
    let source_app = session.SourceAppUserModelId()?.to_string();

    let thumbnail = match previous {
        Some(prev) if prev.title == title && prev.artist == artist => prev.thumbnail.clone(),
        _ => read_thumbnail(&session),
    };

    Ok(Some(MediaInfo {
        title,
        artist,
        playing,
        source_app,
        thumbnail,
    }))
}

pub fn send(command: MediaCommand) -> Result<()> {
    let session = manager()?
        .GetCurrentSession()
        .map_err(|_| "no media session")?;
    let op = match command {
        MediaCommand::TogglePlayPause => session.TryTogglePlayPauseAsync()?,
        MediaCommand::Next => session.TrySkipNextAsync()?,
        MediaCommand::Previous => session.TrySkipPreviousAsync()?,
    };
    op.join()?;
    Ok(())
}
