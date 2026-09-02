//! End-to-end notification CLI smoke test over a loopback webhook.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn notify_test_webhook_requires_real_delivery() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let server = thread::spawn(move || -> Result<Vec<u8>, std::io::Error> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _peer)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "notification CLI did not connect",
                        ));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        };
        stream.set_nonblocking(false)?;
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1_024];
        let expected = loop {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
            }
            request.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = header_end(&request) {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
                break header_end + 4 + length;
            }
        };
        while request.len() < expected {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")?;
        Ok(request)
    });

    let output = Command::new(env!("CARGO_BIN_EXE_lirvena"))
        .args(["notify", "test", "webhook"])
        .env("LIRVENA_STATE_DIRECTORY", temporary.path().join("state"))
        .env("LIRVENA_NOTIFY_WEBHOOK_URL", format!("http://{address}"))
        .env_remove("LIRVENA_NOTIFY_WEBHOOK_HEADERS_PATH")
        .env_remove("LIRVENA_NOTIFY_WEBHOOK_HMAC_PATH")
        .output()?;
    let request = server
        .join()
        .map_err(|_| std::io::Error::other("loopback webhook panicked"))??;
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "Lirvena notification test delivered to 1 adapter(s)\n"
    );
    assert!(request.starts_with(b"POST / HTTP/1.1\r\n"));
    Ok(())
}

fn header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}
