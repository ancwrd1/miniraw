use std::{
    env, fs, io,
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time,
};

use log::{error, info, warn};

fn new_filename_from_timestamp() -> io::Result<(fs::File, PathBuf)> {
    let timestamp = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        .as_secs();

    let mut suffix = 0;

    loop {
        let filename = if suffix == 0 {
            format!("{timestamp}.spl")
        } else {
            format!("{timestamp}-{suffix}.spl")
        };

        let filepath = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_owned()))
            .unwrap_or_default()
            .join(filename);

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&filepath)
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

fn handle_request(mut stream: TcpStream, discard_flag: Arc<AtomicBool>) -> io::Result<()> {
    info!("Incoming connection from {}", stream.peer_addr()?);

    if discard_flag.load(Ordering::SeqCst) {
        let bytes = io::copy(&mut stream, &mut io::sink())?;
        info!("Discarded {} bytes", bytes);
    } else if let Ok((mut target, filepath)) = new_filename_from_timestamp() {
        let bytes = io::copy(&mut stream, &mut target)?;
        if bytes > 0 {
            info!(
                "Saved {} bytes into {}",
                bytes,
                filepath.file_name().unwrap().to_string_lossy()
            );
        } else {
            warn!("Ignored empty file");
            let _ = fs::remove_file(filepath);
        }
    }
    Ok(())
}

pub fn start_raw_listener(discard_flag: Arc<AtomicBool>) -> io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), 9100))?;
    info!("Started listener on port 9100");

    while let Ok((stream, _)) = listener.accept() {
        let discard_flag = discard_flag.clone();

        std::thread::spawn(move || {
            let _ = handle_request(stream, discard_flag);
        });
    }
    Ok(())
}
