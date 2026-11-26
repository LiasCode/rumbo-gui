//! The images a person gives with what they say: pasted into the composer,
//! kept in a file of the window's, sent by that file's path with a task or
//! what is told to a run, and shown in the conversation, as the host's
//! history gives their paths back.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Image, ImageFormat, Img, Styled as _, img};
use ui::rems_from_px;

/// An image the window holds, whole, for as long as it shows it, the path
/// of its file, which is how it is given to a run, and its size in pixels,
/// which is how much room it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub image: Arc<Image>,
    pub path: String,
    width: u32,
    height: u32,
}

impl Picture {
    pub fn new(image: Image, path: String) -> Self {
        let size = imagesize::blob_size(&image.bytes).ok();

        Self {
            width: size.map_or(0, |size| size.width as u32),
            height: size.map_or(0, |size| size.height as u32),
            image: Arc::new(image),
            path,
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

/// The formats a model takes, with the media types and the file extensions
/// they are known by.
const FORMATS: [(ImageFormat, &str, &[&str]); 4] = [
    (ImageFormat::Png, "image/png", &["png"]),
    (ImageFormat::Jpeg, "image/jpeg", &["jpg", "jpeg"]),
    (ImageFormat::Webp, "image/webp", &["webp"]),
    (ImageFormat::Gif, "image/gif", &["gif"]),
];

/// The media type of an image a model takes, or none for a format it does
/// not, such as SVG or BMP.
pub fn media_type(image: &Image) -> Option<&'static str> {
    FORMATS
        .iter()
        .find(|(format, _, _)| *format == image.format)
        .map(|(_, media, _)| *media)
}

/// Where pasted images are kept, given `$XDG_CACHE_HOME` and `$HOME`: the
/// protocol gives a file by its path, so what has none is saved first.
/// None when neither is set.
pub fn pasted_dir(cache_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = cache_home
        .filter(|dir| dir.is_absolute())
        .or_else(|| home.map(|home| home.join(".cache")))?;

    Some(base.join("rumbo-gui").join("pasted"))
}

/// Saves a pasted image in `dir`, named after what it holds, so the same
/// image pasted twice is one file, and gives it with that file's path.
pub fn keep(image: Image, dir: &Path) -> Result<Picture, String> {
    let extension = FORMATS
        .iter()
        .find(|(format, _, _)| *format == image.format)
        .map(|(_, _, extensions)| extensions[0])
        .ok_or("the image is not a PNG, JPEG, GIF, or WebP image")?;

    let mut hasher = DefaultHasher::new();
    image.bytes.hash(&mut hasher);

    let path = dir.join(format!("pasted-{:016x}.{extension}", hasher.finish()));

    fs::create_dir_all(dir)
        .and_then(|()| fs::write(&path, &image.bytes))
        .map_err(|error| format!("{} could not be written: {error}", path.display()))?;

    Ok(Picture::new(image, path.to_string_lossy().into_owned()))
}

/// The paths of the images, as a run is given them.
pub fn paths(pictures: &[Picture]) -> Vec<String> {
    pictures
        .iter()
        .map(|picture| picture.path.clone())
        .collect()
}

/// The image at `path`, by its extension, or none when it is no image the
/// window can show, or cannot be read.
pub fn from_path(path: &str) -> Option<Picture> {
    let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    let format = FORMATS
        .iter()
        .find(|(_, _, extensions)| extensions.contains(&extension.as_str()))
        .map(|(format, _, _)| *format)?;
    let bytes = fs::read(path).ok()?;

    Some(Picture::new(
        Image::from_bytes(format, bytes),
        path.to_string(),
    ))
}

/// A file given by its path where the host runs: as it is when absolute,
/// else from `dir`, the session's directory, as the host takes it.
pub fn in_dir(dir: &str, file: &str) -> String {
    if Path::new(file).is_absolute() {
        return file.to_string();
    }

    Path::new(dir).join(file).to_string_lossy().into_owned()
}

/// The files a history gives: the images the window can show, and the
/// paths of the rest, which it names.
pub fn given(files: &[String]) -> (Vec<Picture>, Vec<String>) {
    let mut pictures = Vec::new();
    let mut others = Vec::new();

    for file in files {
        match from_path(file) {
            Some(picture) => pictures.push(picture),
            None => others.push(file.clone()),
        }
    }

    (pictures, others)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pasted_image_is_kept_and_read_back_by_its_path() {
        let dir = std::env::temp_dir().join(format!("rumbo-gui-picture-{}", std::process::id()));
        let image = Image::from_bytes(ImageFormat::Jpeg, b"jpeg bytes".to_vec());

        let kept = keep(image, &dir).expect("kept");

        assert!(kept.path.ends_with(".jpg"));
        assert_eq!(fs::read(&kept.path).expect("written"), b"jpeg bytes");

        let again = keep(
            Image::from_bytes(ImageFormat::Jpeg, b"jpeg bytes".to_vec()),
            &dir,
        )
        .expect("kept");

        assert_eq!(again.path, kept.path, "the same image is one file");

        let back = from_path(&kept.path).expect("an image");

        assert_eq!(back.image.format, ImageFormat::Jpeg);
        assert_eq!(back.image.bytes, b"jpeg bytes");
        assert_eq!(paths(&[back]), vec![kept.path.clone()]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_image_knows_its_size() {
        // The header of a PNG of 720×220: the signature, then IHDR.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(720u32.to_be_bytes());
        png.extend(220u32.to_be_bytes());
        png.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);

        let picture = Picture::new(Image::from_bytes(ImageFormat::Png, png), "a.png".into());

        assert_eq!((picture.width, picture.height), (720, 220));
    }

    #[test]
    fn what_a_model_does_not_take_is_not_kept() {
        let svg = Image::from_bytes(ImageFormat::Svg, b"<svg/>".to_vec());

        assert_eq!(media_type(&svg), None);
        assert!(keep(svg, &std::env::temp_dir()).is_err());
    }

    #[test]
    fn a_history_names_the_files_that_are_no_image() {
        let (pictures, others) = given(&["/nowhere/plan.pdf".into(), "/nowhere/gone.png".into()]);

        assert!(pictures.is_empty());
        assert_eq!(others, vec!["/nowhere/plan.pdf", "/nowhere/gone.png"]);
    }

    #[test]
    fn a_file_is_found_from_the_sessions_directory() {
        assert_eq!(in_dir("/project", "shots/a.png"), "/project/shots/a.png");
        assert_eq!(in_dir("/project", "/elsewhere/b.png"), "/elsewhere/b.png");
    }

    #[test]
    fn pasted_images_go_in_the_cache() {
        assert_eq!(
            pasted_dir(None, Some("/home/me".into())),
            Some(PathBuf::from("/home/me/.cache/rumbo-gui/pasted"))
        );
        assert_eq!(
            pasted_dir(Some("/cache".into()), Some("/home/me".into())),
            Some(PathBuf::from("/cache/rumbo-gui/pasted"))
        );
        assert_eq!(pasted_dir(Some("relative".into()), None), None);
    }
}
