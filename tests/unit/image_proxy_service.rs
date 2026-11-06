use super::ImageProxyService;
use image::ImageFormat;

#[test]
fn streams_when_source_format_is_accepted() {
    assert!(ImageProxyService::accepts_format(
        "image/avif,image/webp,image/*;q=0.8",
        ImageFormat::Png,
    ));
    assert!(ImageProxyService::accepts_format(
        "image/jpeg",
        ImageFormat::Jpeg,
    ));
}

#[test]
fn transcodes_when_source_format_is_not_accepted() {
    assert!(!ImageProxyService::accepts_format(
        "image/avif,image/webp",
        ImageFormat::Png,
    ));
    assert!(!ImageProxyService::accepts_format(
        "image/png;q=0",
        ImageFormat::Png,
    ));
}

#[test]
fn chooses_only_an_accepted_conversion_format() {
    let service = ImageProxyService::new(1024);

    assert_eq!(
        service.get_preferred_format("image/avif,image/webp"),
        ImageFormat::Avif
    );
    assert_eq!(service.get_preferred_format("image/gif"), ImageFormat::Gif);
    assert_eq!(service.get_preferred_format("image/*"), ImageFormat::Jpeg);
}
