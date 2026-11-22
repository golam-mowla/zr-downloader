use std::env;
use std::fs;

#[path = "../src/clipper.rs"]
mod clipper;
use clipper::*;

#[test]
fn piece_planning_exact_multiple() {
    // 0s to 600s with 300s piece = exactly 2 pieces
    let pieces = plan_clip_pieces(0.0, 600.0, CLIP_PIECE_SECS, "mp4");
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0].index, 1);
    assert_eq!(pieces[0].start_sec, 0.0);
    assert_eq!(pieces[0].end_sec, 300.0);
    assert_eq!(pieces[0].filename, "piece_0001.mp4");
    assert!(!pieces[0].completed);

    assert_eq!(pieces[1].index, 2);
    assert_eq!(pieces[1].start_sec, 300.0);
    assert_eq!(pieces[1].end_sec, 600.0);
    assert_eq!(pieces[1].filename, "piece_0002.mp4");
    assert!(!pieces[1].completed);
}

#[test]
fn piece_planning_with_remainder() {
    // 100s to 750s (duration 650s) with 300s piece = 3 pieces: [100..400], [400..700], [700..750]
    let pieces = plan_clip_pieces(100.0, 750.0, CLIP_PIECE_SECS, "mkv");
    assert_eq!(pieces.len(), 3);
    assert_eq!(pieces[0].index, 1);
    assert_eq!(pieces[0].start_sec, 100.0);
    assert_eq!(pieces[0].end_sec, 400.0);
    assert_eq!(pieces[0].filename, "piece_0001.mkv");

    assert_eq!(pieces[1].index, 2);
    assert_eq!(pieces[1].start_sec, 400.0);
    assert_eq!(pieces[1].end_sec, 700.0);
    assert_eq!(pieces[1].filename, "piece_0002.mkv");

    assert_eq!(pieces[2].index, 3);
    assert_eq!(pieces[2].start_sec, 700.0);
    assert_eq!(pieces[2].end_sec, 750.0);
    assert_eq!(pieces[2].filename, "piece_0003.mkv");
}

#[test]
fn piece_planning_short_clip() {
    // 0s to 200s (less than CLIP_PIECE_SECS = 300s) = single piece
    let pieces = plan_clip_pieces(0.0, 200.0, CLIP_PIECE_SECS, "mp4");
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].index, 1);
    assert_eq!(pieces[0].start_sec, 0.0);
    assert_eq!(pieces[0].end_sec, 200.0);
    assert_eq!(pieces[0].filename, "piece_0001.mp4");
}

#[test]
fn piece_planning_infinite_end() {
    // Infinite end returns empty vector (single-pass unchunked behavior)
    let pieces = plan_clip_pieces(60.0, f64::INFINITY, CLIP_PIECE_SECS, "mp4");
    assert_eq!(pieces.len(), 0);
}

#[test]
fn state_json_save_load_round_trip() {
    let tmp = env::temp_dir().join(format!("zr_clip_roundtrip_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp).unwrap();

    let state = ClipState {
        task_id: "test-task-123".to_string(),
        total_clip_duration: 650.0,
        target_filename: "My Final Video [Clip 00-01-40 to 00-12-30].mp4".to_string(),
        pieces: vec![
            ClipPiece {
                index: 1,
                start_sec: 100.0,
                end_sec: 400.0,
                filename: "piece_0001.mp4".to_string(),
                completed: true,
            },
            ClipPiece {
                index: 2,
                start_sec: 400.0,
                end_sec: 700.0,
                filename: "piece_0002.mp4".to_string(),
                completed: false,
            },
        ],
    };

    assert!(save_clip_state(&tmp, &state).is_ok());

    let loaded = load_clip_state(&tmp);
    assert_eq!(loaded, Some(state));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn state_json_corrupt_file_fallback() {
    let tmp = env::temp_dir().join(format!("zr_clip_corrupt_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp).unwrap();

    fs::write(tmp.join("state.json"), "{ invalid json: true, not complete }").unwrap();

    let loaded = load_clip_state(&tmp);
    assert_eq!(loaded, None);

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn concat_list_file_generation_paths_with_spaces_and_apostrophes() {
    let paths = vec![
        "piece 1.mp4".to_string(),
        "C:\\Videos\\Rock 'n' Roll\\piece '2'.mp4".to_string(),
    ];
    let content = generate_concat_list_content(&paths);
    assert_eq!(
        content,
        "file 'piece 1.mp4'\nfile 'C:/Videos/Rock '\\''n'\\'' Roll/piece '\\''2'\\''.mp4'\n"
    );
}

#[test]
fn clip_working_dir_path_and_cleanup() {
    let tmp = env::temp_dir().join(format!("zr_clip_dir_test_{}", uuid::Uuid::new_v4()));
    let folder_str = tmp.to_str().unwrap();
    let dir = clip_working_dir(folder_str, "task-999");
    assert!(dir.to_string_lossy().ends_with(".zrclip-task-999"));

    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("test.txt"), b"test").unwrap();
    assert!(dir.exists());

    remove_clip_working_dir(folder_str, "task-999");
    assert!(!dir.exists());

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn multi_stream_progress_scaling_tests() {
    // 2 formats: video fills 0..90, audio 90..99
    assert_eq!(multi_phase_percent(100.0, 0, 2, false), 90.0);
    assert_eq!(multi_phase_percent(0.0, 1, 2, false), 90.0);
    assert!(multi_phase_percent(50.0, 1, 2, false) > 90.0 && multi_phase_percent(100.0, 1, 2, false) == 99.0);

    // Audio first: audio fills 0..15, video fills 15..99
    assert_eq!(multi_phase_percent(100.0, 0, 2, true), 15.0);
    assert_eq!(multi_phase_percent(0.0, 1, 2, true), 15.0);
    assert_eq!(multi_phase_percent(100.0, 1, 2, true), 99.0);

    // Single format capped at 99.0 during downloading
    assert_eq!(multi_phase_percent(42.0, 0, 1, false), 42.0);
    assert_eq!(multi_phase_percent(100.0, 0, 1, false), 99.0);

    // 3 formats split evenly
    assert!((multi_phase_percent(100.0, 2, 3, false) - 99.0).abs() < 1e-9);

    // Audio format file detection
    assert!(is_audio_format_file("Live.f140.m4a"));
    assert!(is_audio_format_file("track.f251.webm"));
    assert!(is_audio_format_file("song.opus"));
    assert!(is_audio_format_file("audio.mp3"));
    assert!(is_audio_format_file("sound.aac"));
    assert!(!is_audio_format_file("Live.f137.mp4"));
    assert!(!is_audio_format_file("video.webm"));
    assert!(!is_audio_format_file("movie.mkv"));
}

#[test]
fn sized_phase_percent_uses_real_stream_size() {
    // 10 min video, 20 MB video stream: audio ~ 9.6 MB => video span ~ 67%, NOT 90%.
    let end_of_video = multi_phase_percent_sized(100.0, 0, 2, false, 20_000_000, Some(600.0));
    assert!(end_of_video > 60.0 && end_of_video < 75.0, "got {}", end_of_video);
    // audio part continues from there and ends at 99
    let start_audio = multi_phase_percent_sized(0.0, 1, 2, false, 20_000_000, Some(600.0));
    assert!((start_audio - end_of_video).abs() < 1e-9);
    assert_eq!(multi_phase_percent_sized(100.0, 1, 2, false, 20_000_000, Some(600.0)), 99.0);
    // big video: audio is a small share, so the video span is large but below 97
    let big = multi_phase_percent_sized(100.0, 0, 2, false, 2_000_000_000, Some(600.0));
    assert!(big > 95.0 && big <= 97.0, "got {}", big);
    // unknown size / duration falls back to the fixed 90/9 split
    assert_eq!(multi_phase_percent_sized(100.0, 0, 2, false, 0, Some(600.0)), 90.0);
    assert_eq!(multi_phase_percent_sized(100.0, 0, 2, false, 20_000_000, None), 90.0);
}

pub fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

#[path = "../src/settings.rs"]
mod settings;

fn is_ping_request(request_line: &str) -> bool {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    if !method.eq_ignore_ascii_case("get") {
        return false;
    }
    let path_and_query = parts.next().unwrap_or("");
    let path = path_and_query.split('?').next().unwrap_or("");
    path == "/ping"
}

fn parse_open_query(query: &str) -> Option<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let k = kv.next()?.trim();
        let v = kv.next().unwrap_or("").trim();
        if !k.is_empty() {
            let decoded = url_decode_simple(v);
            map.insert(k.to_string(), decoded);
        }
    }
    if !map.contains_key("url") {
        return None;
    }
    Some(map)
}

fn url_decode_simple(input: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(c1), Some(c2)) = (h1, h2) {
                if let Ok(byte) = u8::from_str_radix(&format!("{}{}", c1 as char, c2 as char), 16) {
                    bytes.push(byte);
                    continue;
                }
            }
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

#[test]
fn load_or_default_persists_token_and_keeps_same_token() {
    let temp_dir = std::env::temp_dir().join(format!("zr_test_handoff_settings_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let settings_file = temp_dir.join("settings.json");

    assert!(!settings_file.exists());
    let state1 = settings::SettingsState::load_or_default(Some(settings_file.clone()));
    let token1 = state1.get_blocking().bridge_pair_token;
    assert!(!token1.is_empty(), "Token should be generated on initial missing settings");
    assert!(settings_file.exists(), "settings.json should be persisted immediately to keep token stable");

    let state2 = settings::SettingsState::load_or_default(Some(settings_file.clone()));
    let token2 = state2.get_blocking().bridge_pair_token;
    assert_eq!(token1, token2, "Token must remain identical on subsequent launches");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn ping_exact_match_logic_test() {
    assert!(is_ping_request("GET /ping HTTP/1.1"));
    assert!(is_ping_request("GET /ping?token=secret HTTP/1.1"));
    assert!(is_ping_request("get /ping HTTP/1.0"));
    assert!(!is_ping_request("GET /formats?url=https%3A%2F%2Fx.com%2Fping HTTP/1.1"));
    assert!(!is_ping_request("POST /ping HTTP/1.1"));
    assert!(!is_ping_request("GET /ping_other HTTP/1.1"));
    assert!(!is_ping_request("GET /open?url=http://example.com/ping HTTP/1.1"));
}

#[test]
fn parse_open_query_parses_cookies_and_user_agent_test() {
    let query = "url=https%3A%2F%2Fexample.com%2Ffile.zip&filename=file.zip&cookies=session%3Dabc123%3B%20user%3Dtest&userAgent=Mozilla%2F5.0%20TestBrowser";
    let parsed = parse_open_query(query).expect("Should parse open query");
    assert_eq!(parsed["url"], "https://example.com/file.zip");
    assert_eq!(parsed["filename"], "file.zip");
    assert_eq!(parsed["cookies"], "session=abc123; user=test");
    assert_eq!(parsed["userAgent"], "Mozilla/5.0 TestBrowser");
}
