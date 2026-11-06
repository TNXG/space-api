use rocket::http::{Header, Status};
use rocket::local::blocking::Client;
use space_api_rs::config::settings::Config;
use space_api_rs::routes;
use space_api_rs::services::image_proxy_service::ImageProxyService;
use space_api_rs::services::image_service::ImageService;

fn client() -> Client {
    let config: Config = config::Config::builder()
        .add_source(config::File::with_name("config.example.toml"))
        .build().unwrap().try_deserialize().unwrap();
    let proxy = ImageProxyService::new(config.image_proxy.max_image_bytes);
    Client::tracked(rocket::build().mount("/images", routes::images::routes())
        .manage(config).manage(proxy).manage(ImageService::new())).unwrap()
}

#[test]
fn json_returns_both_domains_and_formats_for_one_index_record() {
    let client = client();
    let index: serde_json::Value = serde_json::from_str(include_str!("../src/data/blurhash.json")).unwrap();
    for (route, group) in [("wallpaper", "weight"), ("wallpaper_height", "height")] {
        for query in ["mode=json", "mode=json&source=cdcn&format=jxl", "type=json", "m=j&type=cdn", "mode=j", "mode=json&m=r"] {
            let res = client.get(format!("/images/{route}?{query}")).dispatch();
            assert_eq!(res.status(), Status::Ok);
            let value: serde_json::Value = res.into_json().unwrap();
            let data = &value["data"];
            let original = data["original"]["cdn"].as_str().unwrap();
            let filename = original.rsplit('/').next().unwrap();
            let stem = filename.rsplit_once('.').unwrap().0;
            assert_eq!(data["filename"], stem);
            assert_eq!(data["blurhash"], index[group][filename]);
            assert_eq!(original, format!("https://cdn.tnxg.top/images/{route}/{filename}"));
            assert_eq!(data["original"]["cdcn"], format!("https://cdcn.tnxg.top/images/{route}/{filename}"));
            assert_eq!(data["jxl"]["cdn"], format!("https://cdn.tnxg.top/images/{route}/{stem}.jxl"));
            assert_eq!(data["jxl"]["cdcn"], format!("https://cdcn.tnxg.top/images/{route}/{stem}.jxl"));
            if query == "type=json" { assert_eq!(data["image"], original); }
        }
    }
}

#[test]
fn redirect_composes_domain_format_and_legacy_type() {
    let client = client();
    for route in ["wallpaper", "wallpaper_height"] {
        for (query, domain, jxl) in [
            ("mode=redirect", "cdn", false),
            ("mode=redirect&source=cdcn", "cdcn", false),
            ("mode=redirect&source=cdn&format=jxl", "cdn", true),
            ("mode=redirect&source=cdcn&format=jxl", "cdcn", true),
            ("type=cdn", "cdn", false),
            ("type=cdn&source=cdcn&format=jxl", "cdcn", true),
            ("mode=redirect&type=json", "cdn", false),
            ("mode=r&s=cn&f=j", "cdcn", true),
            ("m=r&s=c&f=o&type=json", "cdn", false),
            ("mode=redirect&s=cn&f=j", "cdcn", true),
            ("mode=redirect&m=json&source=cdn&s=cn&format=original&f=j", "cdn", false),
        ] {
            let res = client.get(format!("/images/{route}?{query}"))
                .header(Header::new("Accept", "image/jxl")).dispatch();
            assert_eq!(res.status(), Status::Found, "{query}");
            let location = res.headers().get_one("Location").unwrap();
            assert!(location.starts_with(&format!("https://{domain}.tnxg.top/images/{route}/")), "{location}");
            assert_eq!(location.ends_with(".jxl"), jxl, "{location}");
            assert_eq!(res.headers().get_one("Cache-Control"), Some("no-store"));
        }
    }
}

#[test]
fn invalid_parameters_are_not_silently_treated_as_proxy_requests() {
    let client = client();
    for query in ["mode=invalid", "mode=json&source=invalid", "mode=json&format=avif", "type=invalid", "mode=json&s=invalid", "mode=json&f=invalid", "m=invalid&type=json"] {
        let res = client.get(format!("/images/wallpaper?{query}")).dispatch();
        assert_eq!(res.status(), Status::BadRequest, "{query}");
    }
}
