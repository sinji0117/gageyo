//! 이미지 업로드 — 검사, 저장, 내려주기 (bandiuri `files.rs` 를 옮김).
//!
//! - 파일은 `UPLOAD_DIR/{yyyymm}/{무작위 32자}.{확장자}` 에 저장하고 `/api/files/{yyyymm}/{이름}` 으로 엽니다.
//!   화면들은 `/api` 를 API 로 프록시하므로 고객·파트너·운영자 화면 어디서나 같은 주소로 보입니다.
//! - 받는 형식은 JPEG·PNG·GIF·WebP 뿐이고, 형식은 파일 내용(매직 바이트)으로 판단합니다. SVG 는 받지 않습니다.
//! - JPEG·PNG 는 다시 저장합니다: 사진의 위치(GPS) 같은 메타데이터가 지워지고, 휴대폰 사진의 회전이 바로잡히고,
//!   긴 변이 [`MAX_SIDE`] 를 넘으면 줄입니다.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use axum::extract::Multipart;
use image::{imageops::FilterType, ImageFormat, ImageReader, Limits};
use rand::RngCore;
use serde::Serialize;

use crate::{util::new_id, AppError, AppResult, AppState};

pub const MAX_SIDE: u32 = 2400;
const MAX_INPUT_SIDE: u32 = 12_000;
pub const URL_PREFIX: &str = "/api/files/";

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub id: String,
    pub url: String,
    pub mime: String,
    pub bytes: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct Processed {
    pub ext: &'static str,
    pub mime: &'static str,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn bad(msg: &str) -> AppError {
    AppError::bad(msg)
}

/// 이미지인지 확인하고, 필요하면 다시 저장합니다. CPU 를 쓰므로 `spawn_blocking` 안에서 부릅니다.
pub fn process_image(data: &[u8]) -> AppResult<Processed> {
    let unsupported = || bad("JPG·PNG·GIF·WebP 이미지만 올릴 수 있습니다");
    let format = image::guess_format(data).map_err(|_| unsupported())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_INPUT_SIDE);
    limits.max_image_height = Some(MAX_INPUT_SIDE);
    limits.max_alloc = Some(512 * 1024 * 1024);
    let reader = || {
        let mut r = ImageReader::with_format(Cursor::new(data), format);
        r.limits(limits.clone());
        r
    };
    let broken = |_| bad("이미지 파일이 손상되었거나 너무 큽니다");
    match format {
        // 움직이는 GIF·WebP 는 다시 저장하면 움직임이 사라지므로 검사만 하고 그대로 둡니다.
        ImageFormat::Gif | ImageFormat::WebP => {
            let (width, height) = reader().into_dimensions().map_err(broken)?;
            if width == 0 || height == 0 || width > MAX_INPUT_SIDE || height > MAX_INPUT_SIDE {
                return Err(bad("이미지 크기를 확인해 주세요"));
            }
            let (ext, mime) = if format == ImageFormat::Gif { ("gif", "image/gif") } else { ("webp", "image/webp") };
            Ok(Processed { ext, mime, data: data.to_vec(), width, height })
        }
        ImageFormat::Jpeg | ImageFormat::Png => {
            let mut decoder = reader().into_decoder().map_err(broken)?;
            let orientation = image::ImageDecoder::orientation(&mut decoder).ok();
            let mut img = image::DynamicImage::from_decoder(decoder).map_err(broken)?;
            if let Some(o) = orientation {
                img.apply_orientation(o);
            }
            if img.width().max(img.height()) > MAX_SIDE {
                img = img.resize(MAX_SIDE, MAX_SIDE, FilterType::Lanczos3);
            }
            let mut out = Vec::new();
            let (ext, mime) = if format == ImageFormat::Jpeg {
                let rgb = img.to_rgb8();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
                    .encode_image(&rgb)
                    .map_err(|e| AppError::Internal(format!("jpeg encode: {e}")))?;
                ("jpg", "image/jpeg")
            } else {
                img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
                    .map_err(|e| AppError::Internal(format!("png encode: {e}")))?;
                ("png", "image/png")
            };
            Ok(Processed { ext, mime, data: out, width: img.width(), height: img.height() })
        }
        _ => Err(unsupported()),
    }
}

/// 저장된 상대 경로가 우리가 만든 모양인지 (`{yyyymm}/{32자 hex}.{확장자}`)
pub fn is_valid_rel_path(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    let [month, name] = parts.as_slice() else { return false };
    let Some((stem, ext)) = name.split_once('.') else { return false };
    month.len() == 6
        && month.bytes().all(|b| b.is_ascii_digit())
        && stem.len() == 32
        && stem.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && ["jpg", "png", "gif", "webp"].contains(&ext)
}

pub fn disk_path(root: &Path, rel: &str) -> Option<PathBuf> {
    is_valid_rel_path(rel).then(|| root.join(rel))
}

pub fn mime_of(rel: &str) -> &'static str {
    match rel.rsplit('.').next() {
        Some("jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// 우리 서버에 올린 이미지 주소인지 (`/api/files/...`) — 상품·업체 사진 주소 검사에 씀
pub fn is_own_url(url: &str) -> bool {
    url.strip_prefix(URL_PREFIX).is_some_and(is_valid_rel_path)
}

/// 이미지 주소 입력 검사: 우리 서버 이미지 또는 https 주소만
pub fn check_image_url(url: Option<&str>) -> AppResult<Option<String>> {
    let Some(u) = url.map(str::trim).filter(|u| !u.is_empty()) else { return Ok(None) };
    if is_own_url(u) {
        return Ok(Some(u.to_string()));
    }
    if u.len() <= 500 && u.starts_with("https://") && !u.contains(['"', '\'', '<', '>', ' ', '\\']) {
        return Ok(Some(u.to_string()));
    }
    Err(bad("이미지 주소는 직접 올린 이미지나 https:// 주소만 쓸 수 있습니다"))
}

/// 멀티파트에서 `file` 필드 하나를 읽습니다 → (원래 파일 이름, 내용)
pub async fn read_file_field(mut mp: Multipart, max_bytes: usize) -> AppResult<(String, Vec<u8>)> {
    while let Some(mut field) = mp.next_field().await.map_err(|e| bad(&format!("업로드 형식 오류: {e}")))? {
        if field.name() != Some("file") {
            continue;
        }
        let name: String = field.file_name().unwrap_or("").chars().filter(|c| !c.is_control()).take(200).collect();
        let mut buf = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|_| too_big(max_bytes))? {
            if buf.len() + chunk.len() > max_bytes {
                return Err(too_big(max_bytes));
            }
            buf.extend_from_slice(&chunk);
        }
        if buf.is_empty() {
            return Err(bad("빈 파일입니다"));
        }
        return Ok((name, buf));
    }
    Err(bad("file 필드가 없습니다"))
}

fn too_big(max: usize) -> AppError {
    AppError::bad(format!("이미지는 {}MB 까지 올릴 수 있습니다", max / 1024 / 1024))
}

/// 이미지를 검사·저장하고 `files` 에 기록합니다.
pub async fn store_image(
    state: &AppState,
    uploader_user_id: Option<&str>,
    workshop_id: Option<&str>,
    original_name: &str,
    data: Vec<u8>,
) -> AppResult<FileInfo> {
    let p = tokio::task::spawn_blocking(move || process_image(&data))
        .await
        .map_err(|e| AppError::Internal(format!("image task: {e}")))??;
    let mut rnd = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut rnd);
    let month = crate::util::kst_now().format("%Y%m");
    let rel = format!("{month}/{}.{}", hex::encode(rnd), p.ext);
    let full = write_new(state, &rel, &p.data).await?;
    let id = new_id();
    let res = sqlx::query(
        "INSERT INTO files (id, path, mime, bytes, width, height, original_name, uploader_user_id, workshop_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&rel)
    .bind(p.mime)
    .bind(p.data.len() as u32)
    .bind(p.width)
    .bind(p.height)
    .bind(original_name)
    .bind(uploader_user_id)
    .bind(workshop_id)
    .execute(&state.db)
    .await;
    if let Err(e) = res {
        let _ = tokio::fs::remove_file(&full).await;
        return Err(e.into());
    }
    Ok(FileInfo {
        id,
        url: format!("{URL_PREFIX}{rel}"),
        mime: p.mime.into(),
        bytes: p.data.len() as u32,
        width: p.width,
        height: p.height,
    })
}

/// 디스크에 쓰기 (다 쓴 뒤 이름을 바꿔서, 중간에 실패해도 반쯤 쓴 파일이 보이지 않게)
async fn write_new(state: &AppState, rel: &str, data: &[u8]) -> AppResult<PathBuf> {
    let full = state.cfg.upload_dir.join(rel);
    let dir = full.parent().ok_or_else(|| AppError::Internal("upload path".into()))?;
    tokio::fs::create_dir_all(dir).await.map_err(|e| AppError::Internal(format!("mkdir {}: {e}", dir.display())))?;
    let tmp = full.with_extension("part");
    tokio::fs::write(&tmp, data).await.map_err(|e| AppError::Internal(format!("write {}: {e}", tmp.display())))?;
    tokio::fs::rename(&tmp, &full).await.map_err(|e| AppError::Internal(format!("rename: {e}")))?;
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img).write_to(&mut Cursor::new(&mut out), ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn rel_path_shape() {
        assert!(is_valid_rel_path("202610/0123456789abcdef0123456789abcdef.jpg"));
        assert!(!is_valid_rel_path("202610/0123456789ABCDEF0123456789abcdef.jpg"));
        assert!(!is_valid_rel_path("../0123456789abcdef0123456789abcdef.jpg"));
        assert!(!is_valid_rel_path("202610/0123456789abcdef0123456789abcdef.svg"));
        assert!(!is_valid_rel_path("2026/0123456789abcdef0123456789abcdef.png"));
        assert!(!is_valid_rel_path("202610/x/0123456789abcdef0123456789abcdef.png"));
        assert!(is_own_url("/api/files/202610/0123456789abcdef0123456789abcdef.webp"));
        assert!(!is_own_url("/api/files/../../etc/passwd"));
    }

    #[test]
    fn image_url_rules() {
        assert_eq!(check_image_url(None).unwrap(), None);
        assert_eq!(check_image_url(Some(" ")).unwrap(), None);
        assert!(check_image_url(Some("https://images.example.com/a.jpg")).unwrap().is_some());
        assert!(check_image_url(Some("http://x.com/a.jpg")).is_err());
        assert!(check_image_url(Some("javascript:alert(1)")).is_err());
        assert!(check_image_url(Some("https://x.com/a\".jpg")).is_err());
    }

    #[test]
    fn png_is_reencoded_and_resized() {
        let p = process_image(&png(3000, 1500)).unwrap();
        assert_eq!((p.ext, p.width, p.height), ("png", MAX_SIDE, 1200));
        let small = process_image(&png(40, 30)).unwrap();
        assert_eq!((small.width, small.height), (40, 30));
    }

    #[test]
    fn rejects_non_images_and_svg() {
        assert!(process_image(b"hello world, not an image").is_err());
        assert!(process_image(br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#).is_err());
        let mut broken = png(10, 10);
        broken.truncate(40);
        assert!(process_image(&broken).is_err());
    }
}
