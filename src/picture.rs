//! The images a person gives with what they say: pasted into the composer,
//! sent with a task or what is told to a run, and shown in the
//! conversation, as the host's history gives them back.

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use gpui::{Image, ImageFormat, Img, Styled as _, img};
use ui::rems_from_px;

use crate::protocol;

/// An image the window holds, whole, for as long as it shows it, and its
/// size in pixels, which is how much room it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub image: Arc<Image>,
    width: u32,
    height: u32,
}

impl Picture {
    pub fn new(image: Image) -> Self {
        let size = imagesize::blob_size(&image.bytes).ok();

        Self {
            width: size.map_or(0, |size| size.width as u32),
            height: size.map_or(0, |size| size.height as u32),
            image: Arc::new(image),
        }
    }

    /// The image drawn at most `width`×`height` pixels, as the window's
    /// zoom makes them, keeping its proportions, and never larger than it
    /// is.
    pub fn shown(&self, width: f32, height: f32) -> Img {
        let shown = img(self.image.clone()).rounded_md();

        if self.width == 0 || self.height == 0 {
            return shown.max_w(rems_from_px(width)).max_h(rems_from_px(height));
        }

        let (natural_width, natural_height) = (self.width as f32, self.height as f32);
        let ratio = (width / natural_width).min(height / natural_height).min(1.);

        shown
            .w(rems_from_px(natural_width * ratio))
            .h(rems_from_px(natural_height * ratio))
    }
}

/// The formats a model takes, with the media types the protocol names them
/// by.
const FORMATS: [(ImageFormat, &str); 4] = [
    (ImageFormat::Png, "image/png"),
    (ImageFormat::Jpeg, "image/jpeg"),
    (ImageFormat::Webp, "image/webp"),
    (ImageFormat::Gif, "image/gif"),
];

/// The media type of an image a model takes, or none for a format it does
/// not, such as SVG or BMP.
pub fn media_type(image: &Image) -> Option<&'static str> {
    FORMATS
        .iter()
        .find(|(format, _)| *format == image.format)
        .map(|(_, media)| *media)
}

/// The image as the protocol sends it. The host prepares it for the model.
pub fn to_protocol(picture: &Picture) -> protocol::Image {
    protocol::Image {
        media_type: media_type(&picture.image)
            .unwrap_or("image/png")
            .to_string(),
        data: STANDARD.encode(&picture.image.bytes),
    }
}

/// The image the protocol carries, or none when it is not one the window
/// can show.
pub fn from_protocol(image: &protocol::Image) -> Option<Picture> {
    let format = FORMATS
        .iter()
        .find(|(_, media)| *media == image.media_type)
        .map(|(format, _)| *format)?;
    let bytes = STANDARD.decode(&image.data).ok()?;

    Some(Picture::new(Image::from_bytes(format, bytes)))
}

/// The images the protocol carries, those the window can show.
pub fn from_all(images: &[protocol::Image]) -> Vec<Picture> {
    images.iter().filter_map(from_protocol).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_goes_to_the_protocol_and_back() {
        let picture = Picture::new(Image::from_bytes(ImageFormat::Jpeg, b"jpeg bytes".to_vec()));
        let sent = to_protocol(&picture);

        assert_eq!(sent.media_type, "image/jpeg");
        assert_eq!(sent.data, "anBlZyBieXRlcw==");

        let back = from_protocol(&sent).expect("an image");

        assert_eq!(back.image.format, ImageFormat::Jpeg);
        assert_eq!(back.image.bytes, picture.image.bytes);
        assert_eq!(back.image.id(), picture.image.id());
    }

    #[test]
    fn an_image_knows_its_size() {
        // The header of a PNG of 720×220: the signature, then IHDR.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(720u32.to_be_bytes());
        png.extend(220u32.to_be_bytes());
        png.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);

        let picture = Picture::new(Image::from_bytes(ImageFormat::Png, png));

        assert_eq!((picture.width, picture.height), (720, 220));
    }

    #[test]
    fn what_a_model_does_not_take_is_left_out() {
        let svg = Image::from_bytes(ImageFormat::Svg, b"<svg/>".to_vec());

        assert_eq!(media_type(&svg), None);

        let unknown = protocol::Image {
            media_type: "image/bmp".into(),
            data: STANDARD.encode(b"bmp"),
        };
        let broken = protocol::Image {
            media_type: "image/png".into(),
            data: "not base64!".into(),
        };

        assert!(from_all(&[unknown, broken]).is_empty());
    }
}
