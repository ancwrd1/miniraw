use std::{env, path::PathBuf, time};

use async_std::{fs, io, net::TcpListener, prelude::*};
use log::{error, info, warn};

async fn new_filename_from_timestamp() -> io::Result<(fs::File, PathBuf)> {
    let timestamp = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut suffix = 0;

    loop {
        let filename = if suffix == 0 {
            format!("{}.spl", timestamp)
        } else {
            format!("{}-{}.spl", timestamp, suffix)
        };

        let filepath = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_owned()))
            .unwrap_or_else(|| PathBuf::new())
            .join(&filename);

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&filepath)
            .await
        {
            Ok(writer) => break Ok((writer, filepath)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(e) => {
                error!("{}", e);
                return Err(e);
            }
        }
    }
}

pub async fn start_raw_listener() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9100").await?;
    info!("Started listener on port 9100");

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        if let Ok(mut stream) = stream {
            info!("Incoming connection from {}", stream.peer_addr()?);

            if let Ok((mut target, filepath)) = new_filename_from_timestamp().await {
                let bytes = io::copy(&mut stream, &mut target).await?;
                if bytes > 0 {
                    info!(
                        "Saved {} bytes into {}",
                        bytes,
                        filepath.file_name().unwrap().to_string_lossy()
                    );
                } else {
                    warn!("Ignored empty file");
                    let _ = fs::remove_file(filepath).await;
                }
            }
        }
    }
    Ok(())
}
