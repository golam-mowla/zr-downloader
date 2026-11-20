use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CLIP_PIECE_SECS: f64 = 300.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipPiece {
    pub index: usize,
    pub start_sec: f64,
    pub end_sec: f64,
    pub filename: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipState {
    pub task_id: String,
    pub total_clip_duration: f64,
    pub target_filename: String,
    pub pieces: Vec<ClipPiece>,
}

pub fn plan_clip_pieces(start_sec: f64, end_sec: f64, piece_len: f64, ext: &str) -> Vec<ClipPiece> {
    let mut pieces = Vec::new();
    if end_sec <= start_sec || piece_len <= 0.0 || !end_sec.is_finite() {
        return pieces;
    }
    let mut cur_start = start_sec;
    let mut idx = 1;
    while cur_start < end_sec {
        let cur_end = (cur_start + piece_len).min(end_sec);
        if cur_end >= end_sec - 0.001 {
            pieces.push(ClipPiece {
                index: idx,
                start_sec: cur_start,
                end_sec,
                filename: format!("piece_{:04}.{}", idx, ext),
                completed: false,
            });
            break;
        } else {
            pieces.push(ClipPiece {
                index: idx,
                start_sec: cur_start,
                end_sec: cur_end,
                filename: format!("piece_{:04}.{}", idx, ext),
                completed: false,
            });
            cur_start = cur_end;
            idx += 1;
        }
    }
    pieces
}

pub fn escape_concat_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized.replace('\'', "'\\''")
}

pub fn generate_concat_list_content(paths: &[String]) -> String {
    let mut out = String::new();
    for p in paths {
        let escaped = escape_concat_path(p);
        out.push_str(&format!("file '{}'\n", escaped));
    }
    out
}

pub fn clip_working_dir(save_folder: &str, task_id: &str) -> PathBuf {
    Path::new(save_folder).join(format!(".zrclip-{}", task_id))
}

pub fn remove_clip_working_dir(save_folder: &str, task_id: &str) {
    let dir = clip_working_dir(save_folder, task_id);
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
}

pub fn load_clip_state(working_dir: &Path) -> Option<ClipState> {
    let state_path = working_dir.join("state.json");
    if !state_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&state_path).ok()?;
    serde_json::from_str::<ClipState>(&content).ok()
}

pub fn save_clip_state(working_dir: &Path, state: &ClipState) -> Result<(), String> {
    let state_json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    let tmp_path = working_dir.join("state.json.tmp");
    let final_path = working_dir.join("state.json");
    fs::write(&tmp_path, state_json).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, &final_path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns true if a format file name or path indicates an audio stream.
pub fn is_audio_format_file(path: &str) -> bool {
    let p = path.to_lowercase();
    p.ends_with(".m4a")
        || p.ends_with(".opus")
        || p.ends_with(".mp3")
        || p.ends_with(".aac")
        || p.ends_with(".ogg")
        || p.ends_with(".weba")
        || p.ends_with(".flac")
        || p.ends_with(".wav")
        || p.contains(".f140.")
        || p.contains(".f251.")
        || p.contains(".f250.")
        || p.contains(".f249.")
        || p.contains(".f139.")
        || p.contains(".f233.")
        || p.contains(".f234.")
}

/// Maps a per-format percent (0..100) onto ONE overall bar (0..99%) when yt-dlp downloads several
/// formats one after another (e.g. YouTube DASH video + audio, HLS video + audio playlists).
/// Clamped strictly below 100% so in-flight downloading tasks never display 100.0% prematurely.
pub fn multi_phase_percent(raw: f64, idx: usize, total: usize, first_is_audio: bool) -> f64 {
    let raw = raw.clamp(0.0, 100.0);
    if total <= 1 {
        return raw.min(99.0);
    }
    let idx = idx.min(total - 1);
    let (lo, hi) = if total == 2 {
        if first_is_audio {
            // Audio downloaded first (~15% of bytes), video downloaded second (~84% of bytes)
            if idx == 0 {
                (0.0, 15.0)
            } else {
                (15.0, 99.0)
            }
        } else {
            // Video downloaded first (~90% of bytes), audio downloaded second (~9% of bytes)
            if idx == 0 {
                (0.0, 90.0)
            } else {
                (90.0, 99.0)
            }
        }
    } else {
        let span = 99.0 / total as f64;
        (span * idx as f64, span * (idx as f64 + 1.0))
    };
    (lo + (hi - lo) * raw / 100.0).clamp(0.0, 99.0)
}

/// Like `multi_phase_percent`, but for the common "video then audio" pair it splits the bar by
/// the REAL size of the first stream instead of a fixed 90/9 guess. The audio size is estimated
/// from the media duration (~128 kbps), so a small low-resolution video no longer shows ~90%
/// when its (relatively large) audio stream has not even started downloading.
/// Falls back to `multi_phase_percent` whenever the size or duration is not known yet.
pub fn multi_phase_percent_sized(
    raw: f64,
    idx: usize,
    total: usize,
    first_is_audio: bool,
    first_stream_bytes: u64,
    duration_secs: Option<f64>,
) -> f64 {
    if total == 2 && !first_is_audio && first_stream_bytes > 0 {
        if let Some(dur) = duration_secs.filter(|d| d.is_finite() && *d > 1.0) {
            let v = first_stream_bytes as f64;
            let a = (dur * 16_000.0).clamp(65_536.0, v.max(65_536.0));
            let span = (99.0 * v / (v + a)).clamp(40.0, 97.0);
            let raw = raw.clamp(0.0, 100.0);
            let (lo, hi) = if idx == 0 { (0.0, span) } else { (span, 99.0) };
            return (lo + (hi - lo) * raw / 100.0).clamp(0.0, 99.0);
        }
    }
    multi_phase_percent(raw, idx, total, first_is_audio)
}
