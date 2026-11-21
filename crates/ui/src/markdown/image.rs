//! Where an image a text names is: in the text itself, as a `data:` URL;
//! on the web, which is fetched only once the person asks, since an image
//! a model names could carry what it read to whoever serves it; or a file
//! on this machine, by its path, or by one from the folder the text is
//! about.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use gpui::ImageFormat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    /// The image's bytes, in the text.
    Data {
        format: ImageFormat,
        bytes: Vec<u8>,
    },
    Web(String),
    File(PathBuf),
}

/// Where the image at `url` is, the relative paths from `base`. None for
/// what is no image GPUI can draw, or no file without a folder to find it in.
pub(crate) fn source(url: &str, base: Option<&Path>) -> Option<Source> {
    let url = url.trim();

    if let Some(data) = url.strip_prefix("data:") {
        return data_url(data);
    }

    if url.starts_with("https://") || url.starts_with("http://") {
        return Some(Source::Web(url.to_string()));
    }

    if let Some(path) = url.strip_prefix("file://") {
        return Some(Source::File(PathBuf::from(unescape(path))));
    }

    // Another scheme, such as ftp: or mailto:, is no file.
    if url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    }) {
        return None;
    }

    let path = PathBuf::from(unescape(url));

    match path.is_absolute() {
        true => Some(Source::File(path)),
        false => base.map(|base| Source::File(base.join(path))),
    }
}

/// What follows `data:`: a MIME type, `;base64`, a comma, and the bytes.
fn data_url(data: &str) -> Option<Source> {
    let (head, body) = data.split_once(',')?;
    let mut parts = head.split(';');
    let format = ImageFormat::from_mime_type(parts.next()?.trim())?;
    let encoded = parts.any(|part| part.trim() == "base64");

    let bytes = match encoded {
        true => base64::engine::general_purpose::STANDARD
            .decode(body.trim())
            .ok()?,
        false => unescape(body).into_bytes(),
    };

    Some(Source::Data { format, bytes })
}

/// `%20` and the other escapes of a URL, as the bytes they stand for.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| text.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());

        match escaped {
            Some(byte) => {
                out.push(byte);
                index += 3;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// Where a host on the web is, for the person to read before asking it
/// for an image.
pub(crate) fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);

    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_files_by_their_path_or_from_the_folder() {
        let base = Path::new("/work/app");

        assert_eq!(
            source("docs/chart.png", Some(base)),
            Some(Source::File(PathBuf::from("/work/app/docs/chart.png")))
        );
        assert_eq!(
            source("/tmp/a%20b.png", Some(base)),
            Some(Source::File(PathBuf::from("/tmp/a b.png")))
        );
        assert_eq!(
            source("file:///tmp/c.svg", None),
            Some(Source::File(PathBuf::from("/tmp/c.svg")))
        );

        // Without a folder, a relative path finds nothing.
        assert_eq!(source("chart.png", None), None);
    }

    #[test]
    fn keeps_the_web_apart_and_other_schemes_out() {
        assert_eq!(
            source("https://example.com/a.png?size=2", None),
            Some(Source::Web("https://example.com/a.png?size=2".into()))
        );
        assert_eq!(
            source("mailto:someone@example.com", Some(Path::new("/"))),
            None
        );
        assert_eq!(host("https://example.com:8080/a.png?x"), "example.com:8080");
    }

    #[test]
    fn reads_the_bytes_a_data_url_holds() {
        assert_eq!(
            source("data:image/png;base64,iVBORw0=", None),
            Some(Source::Data {
                format: ImageFormat::Png,
                bytes: vec![0x89, 0x50, 0x4e, 0x47, 0x0d],
            })
        );
        assert_eq!(
            source("data:image/svg+xml,%3Csvg%2F%3E", None),
            Some(Source::Data {
                format: ImageFormat::Svg,
                bytes: b"<svg/>".to_vec(),
            })
        );

        // Not an image GPUI draws.
        assert_eq!(source("data:text/plain;base64,aGk=", None), None);
    }
}
