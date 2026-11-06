#[cfg(test)]
mod tests {
    use super::ImageService;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    #[tokio::test]
    async fn wallpaper_body_is_available_before_upstream_finishes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image.jxl", listener.local_addr().unwrap());
        let (release, wait) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/jxl\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc").unwrap();
            // The last bytes cannot arrive until the consumer has read the first bytes.
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            stream.write_all(b"def").unwrap();
        });
        let mut response = tokio::time::timeout(Duration::from_secs(5), ImageService::new().open_wallpaper(&url))
            .await.unwrap().unwrap();
        assert_eq!(response.content_length(), Some(6));
        let first = tokio::time::timeout(Duration::from_secs(5), response.chunk()).await.unwrap().unwrap().unwrap();
        assert_eq!(&first[..], b"abc");
        release.send(()).unwrap();
        assert_eq!(&response.bytes().await.unwrap()[..], b"def");
        server.join().unwrap();
    }
}
