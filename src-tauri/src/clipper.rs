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
