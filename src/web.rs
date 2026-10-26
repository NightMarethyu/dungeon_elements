//! Pictures from the web, like `![Map](https://example.com/map.png)` in a note.
//!
//! They are downloaded in the background the first time a note showing them is read, and
//! kept for as long as the campaign is open.

use std::collections::HashMap;
use std::time::Duration;

use iced::widget::{image, svg};

/// The biggest picture downloaded.
const MAX_SIZE: u64 = 25 * 1024 * 1024;

/// A picture from the web, by its URL.
pub type Pictures = HashMap<String, Picture>;

#[derive(Debug, Clone)]
pub enum Picture {
    Loading,
    Image(image::Handle),
    Svg(svg::Handle),
    /// Why it can't be shown.
    Failed(String),
}

/// Whether `url` is a web address a picture can be downloaded from.
pub fn is_web(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Downloads the picture at `url`.
pub async fn fetch(url: String) -> (String, Picture) {
    let (sender, receiver) = iced::futures::channel::oneshot::channel();
    let address = url.clone();
    // The download blocks, so it gets a thread of its own.
    std::thread::spawn(move || {
        let _ = sender.send(download(&address).and_then(|(kind, bytes)| picture(kind, bytes)));
    });
    let picture = match receiver.await {
        Ok(Ok(picture)) => picture,
        Ok(Err(problem)) => Picture::Failed(problem),
        Err(_) => Picture::Failed("the download stopped".into()),
    };
    (url, picture)
}

/// The file at `url` and its content type.
fn download(url: &str) -> Result<(String, Vec<u8>), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                // The computer's own certificates, so it works behind company proxies too.
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .user_agent(concat!("Dungeon Elements/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();

    let mut response = agent.get(url).call().map_err(|error| match error {
        ureq::Error::StatusCode(code) => format!("the server answered {code}"),
        error => error.to_string(),
    })?;
    let kind = response
        .headers()
        .get("content-type")
        .and_then(|kind| kind.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_SIZE)
        .read_to_vec()
        .map_err(|error| match error {
            ureq::Error::BodyExceedsLimit(_) => "it is bigger than 25 MB".to_string(),
            error => error.to_string(),
        })?;
    Ok((kind, bytes))
}

/// Makes a picture from downloaded `bytes` of content type `kind`.
fn picture(kind: String, bytes: Vec<u8>) -> Result<Picture, String> {
    if kind.starts_with("image/svg") || looks_like_svg(&bytes) {
        return Ok(Picture::Svg(svg::Handle::from_memory(bytes)));
    }
    match ::image::guess_format(&bytes) {
        Ok(
            ::image::ImageFormat::Png
            | ::image::ImageFormat::Jpeg
            | ::image::ImageFormat::Gif
            | ::image::ImageFormat::WebP,
        ) => Ok(Picture::Image(image::Handle::from_bytes(bytes))),
        Ok(format) => Err(format!("{format:?} pictures aren't supported")),
        Err(_) if kind.starts_with("text/html") => Err("it is a web page, not a picture".into()),
        Err(_) => Err("it isn't a picture".into()),
    }
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let start = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    let start = start.trim_start_matches('\u{feff}').trim_start();
    start.starts_with("<svg") || (start.starts_with("<?xml") && start.contains("<svg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_pictures_apart() {
        assert!(is_web("https://example.com/a.png"));
        assert!(is_web("HTTP://example.com/a.png"));
        assert!(!is_web("maps/a.png"));

        let png = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0];
        assert!(matches!(
            picture(String::new(), png.to_vec()),
            Ok(Picture::Image(_))
        ));
        assert!(matches!(
            picture(
                "text/plain".into(),
                b"<?xml version=\"1.0\"?>\n<svg></svg>".to_vec()
            ),
            Ok(Picture::Svg(_))
        ));
        assert!(picture("text/html".into(), b"<html></html>".to_vec()).is_err());
    }
}
