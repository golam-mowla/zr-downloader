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
