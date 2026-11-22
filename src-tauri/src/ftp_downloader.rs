use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use suppaftp::AsyncFtpStream;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::downloader::{DownloadTask, UrlMetadata};

pub struct FtpUrlParts {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub path: String,
    pub filename: String,
}

pub fn parse_ftp_url(raw: &str) -> Result<FtpUrlParts, String> {
    let clean = raw.trim();
    if !clean.starts_with("ftp://") {
        return Err("Not an FTP URL".to_string());
    }
    let rest = &clean[6..];
    let (auth_and_host, path_part) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    let (auth, host_port) = match auth_and_host.find('@') {
        Some(idx) => (Some(&auth_and_host[..idx]), &auth_and_host[idx + 1..]),
        None => (None, auth_and_host),
    };

    let (username, password) = if let Some(a) = auth {
        match a.find(':') {
            Some(i) => (a[..i].to_string(), a[i + 1..].to_string()),
            None => (a.to_string(), String::new()),
        }
    } else {
        ("anonymous".to_string(), "anonymous@".to_string())
    };

    let (host, port) = match host_port.find(':') {
        Some(i) => {
            let h = &host_port[..i];
            let p = host_port[i + 1..].parse::<u16>().unwrap_or(21);
            (h.to_string(), p)
        }
        None => (host_port.to_string(), 21),
    };

    let path_str = path_part.to_string();
    let filename = path_str
        .split('/')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("download.bin")
        .to_string();

    Ok(FtpUrlParts {
        host,
        port,
        username,
        password,
        path: path_str,
        filename,
    })
}

pub async fn probe_ftp_url(url: &str) -> Result<UrlMetadata, String> {
    let parts = parse_ftp_url(url)?;
    let addr = format!("{}:{}", parts.host, parts.port);

    let mut stream = AsyncFtpStream::connect(&addr)
        .await
        .map_err(|e| format!("FTP connection failed: {}", e))?;

    stream
        .login(&parts.username, &parts.password)
        .await
        .map_err(|e| format!("FTP login failed: {}", e))?;

    let size = stream.size(&parts.path).await.ok().map(|s| s as u64);
    let _ = stream.quit().await;

    Ok(UrlMetadata {
        url: url.to_string(),
        filename: parts.filename,
        total_size: size,
        supports_ranges: false,
        content_type: None,
        category: "documents".to_string(),
        last_modified: None,
    })
}

pub async fn download_ftp_task(
    app: AppHandle,
    task: DownloadTask,
    cancel_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let parts = parse_ftp_url(&task.url)?;
    let addr = format!("{}:{}", parts.host, parts.port);

    let mut stream = AsyncFtpStream::connect(&addr)
        .await
        .map_err(|e| format!("FTP connection failed: {}", e))?;

    stream
        .login(&parts.username, &parts.password)
        .await
        .map_err(|e| format!("FTP login failed: {}", e))?;

    let total_bytes = task.total_bytes;
    let dest_path = PathBuf::from(&task.save_path);

    if let Some(parent) = dest_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("Failed to create destination file: {}", e))?;

    let mut data_stream = stream
        .retr_as_stream(&parts.path)
        .await
        .map_err(|e| format!("FTP RETR failed: {}", e))?;

    let mut downloaded = 0u64;
    let mut last_emit = Instant::now();
    let mut last_bytes = 0u64;
    let mut speed;

    use futures_util::AsyncReadExt;
    let mut chunk = [0u8; 32768];

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = stream.quit().await;
            return Err("Download paused or cancelled".to_string());
        }

        match data_stream.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                file.write_all(&chunk[..n])
                    .await
                    .map_err(|e| format!("Write failed: {}", e))?;
                downloaded += n as u64;

                let now = Instant::now();
                let elapsed = now.duration_since(last_emit).as_secs_f64();
                if elapsed >= 0.5 {
                    let diff = downloaded.saturating_sub(last_bytes);
                    speed = (diff as f64 / elapsed) as u64;
                    last_emit = now;
                    last_bytes = downloaded;

                    let percent = if total_bytes > 0 {
                        ((downloaded as f64 / total_bytes as f64) * 100.0).clamp(0.0, 99.0)
                    } else {
                        0.0
                    };

                    let mut updated_task = task.clone();
                    updated_task.downloaded_bytes = downloaded;
                    updated_task.percent = percent;
                    updated_task.speed_bps = speed;
                    updated_task.speed_formatted = format!("{:.2} MB/s", speed as f64 / 1_048_576.0);
                    updated_task.status = "downloading".to_string();

                    let _ = app.emit("download-progress", updated_task);
                }
            }
            Err(e) => {
                let _ = stream.quit().await;
                return Err(format!("FTP transfer error: {}", e));
            }
        }
    }

    let _ = file.flush().await;
    let _ = stream.finalize_retr_stream(data_stream).await;
    let _ = stream.quit().await;

    let mut completed_task = task.clone();
    completed_task.downloaded_bytes = downloaded;
    completed_task.total_bytes = downloaded;
    completed_task.percent = 100.0;
    completed_task.speed_bps = 0;
    completed_task.speed_formatted = "0 B/s".to_string();
    completed_task.status = "completed".to_string();

    let _ = app.emit("download-progress", completed_task);

    Ok(())
}
