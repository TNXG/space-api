use rocket::http::{Accept, Status};
use rocket::local::blocking::Client;
use space_api_rs::config::settings::Config;
use space_api_rs::routes;
use space_api_rs::services::image_proxy_service::ImageProxyService;
use space_api_rs::services::image_service::ImageService;
use std::collections::HashMap;

fn client() -> Client {
    // 路由声明了 Config / ImageProxyService 的 State 哨兵，测试里只需最小可用实例。
    let config: Config = config::Config::builder()
        .add_source(config::File::with_name("config.example.toml"))
        .build()
        .expect("example config should load")
        .try_deserialize()
        .expect("example config should deserialize");
    let proxy = ImageProxyService::new(config.image_proxy.max_image_bytes);

    let rocket = rocket::build()
        .mount("/images", routes::images::routes())
        .manage(config)
        .manage(proxy)
        .manage(ImageService::new());
    Client::tracked(rocket).expect("valid rocket instance")
}

fn draw_json(client: &Client, kind: &str) -> (String, String) {
    let res = client
        .get(format!("/images/{kind}?type=json"))
        .header(Accept::Any)
        .dispatch();
    assert_eq!(
        res.status(),
        Status::Ok,
        "body: {}",
        res.into_string().unwrap_or_default()
    );
    let value: serde_json::Value = res.into_json().expect("json body");
    let image = value["data"]["image"]
        .as_str()
        .expect("image url")
        .to_string();
    let blurhash = value["data"]["blurhash"]
        .as_str()
        .expect("blurhash")
        .to_string();
    (image, blurhash)
}

#[test]
fn json_mode_returns_keyed_wallpaper_with_matching_blurhash() {
    let client = client();
    let mut pairs: HashMap<String, String> = HashMap::new();

    for _ in 0..200 {
        let (image, blurhash) = draw_json(&client, "wallpaper");
        assert!(
            image.starts_with("https://cdn.tnxg.top/images/wallpaper/"),
            "unexpected url: {image}"
        );
        let filename = image.rsplit('/').next().expect("filename").to_string();
        assert!(!blurhash.is_empty(), "blurhash missing for {filename}");
        // 同一个键每次都必须给出同一个 blurhash：键名与值来自同一条索引记录。
        if let Some(previous) = pairs.insert(filename.clone(), blurhash.clone()) {
            assert_eq!(previous, blurhash, "unstable blurhash for {filename}");
        }
    }

    assert!(pairs.len() > 1, "selection never varied across 200 draws");
}

#[test]
fn height_wallpapers_use_the_height_directory() {
    let client = client();

    let (image, blurhash) = draw_json(&client, "wallpaper_height");
    assert!(
        image.starts_with("https://cdn.tnxg.top/images/wallpaper_height/"),
        "unexpected url: {image}"
    );
    assert!(!blurhash.is_empty(), "blurhash missing for {image}");

    let res = client
        .get("/images/wallpaper_height?type=cdn")
        .header(Accept::Any)
        .dispatch();
    assert_eq!(res.status(), Status::Found);
    let location = res.headers().get_one("Location").expect("location header");
    assert!(
        location.starts_with("https://cdn.tnxg.top/images/wallpaper_height/"),
        "unexpected redirect: {location}"
    );
}
