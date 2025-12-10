use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{self, Responder, Response};
use std::io::Cursor;

pub struct CustomResponse {
    content_type: ContentType,
    data: Vec<u8>,
    status: Status,
    headers: Vec<(String, String)>,
    cache: bool,
}

impl CustomResponse {
    pub fn new(content_type: ContentType, data: Vec<u8>, status: Status) -> Self {
        Self {
            content_type,
            data,
            status,
            headers: Vec::new(),
            cache: false,
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    pub fn with_headers(mut self, headers: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
        for (k, v) in headers {
            self.headers.push((k.into(), v.into()));
        }
        self
    }

    /// 设置是否命中服务器端缓存
    pub fn with_cache(mut self, cache: bool) -> Self {
        self.cache = cache;
        self
    }
}

impl<'r> Responder<'r, 'static> for CustomResponse {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        let mut builder = Response::build();
        builder.header(self.content_type);
        builder.status(self.status);
        let mut headers = self.headers;
        headers.push(if self.cache {
            ("SERVER-CACHE".into(), "HIT".into())
        } else {
            ("SERVER-CACHE".into(), "MISS".into())
        });

        for (k, v) in headers {
            builder.raw_header(k, v);
        }

        builder.sized_body(self.data.len(), Cursor::new(self.data)).ok()
    }
}
