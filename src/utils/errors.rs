use std::fmt;
use std::fmt::Display;
use rocket::http::Status;
use rocket::request::Request;
use rocket::response::{self, Response, Responder};
use serde_json::json;
use std::io::Cursor;

#[derive(Debug)]
pub enum Error {
    Database(String),
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    Gone(String),
    Internal(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Database(msg) => write!(f, "Database error: {}", msg),
            Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            Error::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            Error::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            Error::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            Error::Conflict(msg) => write!(f, "Conflict: {}", msg),
            Error::Gone(msg) => write!(f, "Gone: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl<'r> Responder<'r, 'static> for Error {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        let status = match &self {
            Error::Database(_) => Status::InternalServerError,
            Error::NotFound(_) => Status::NotFound,
            Error::BadRequest(_) => Status::BadRequest,
            Error::Unauthorized(_) => Status::Unauthorized,
            Error::Forbidden(_) => Status::Forbidden,
            Error::Conflict(_) => Status::Conflict,
            Error::Gone(_) => Status::Gone,
            Error::Internal(_) => Status::InternalServerError,
        };

        let code = match &self {
            Error::Database(_) => "500",
            Error::NotFound(_) => "404",
            Error::BadRequest(_) => "400",
            Error::Unauthorized(_) => "401",
            Error::Forbidden(_) => "403",
            Error::Conflict(_) => "409",
            Error::Gone(_) => "410",
            Error::Internal(_) => "500",
        };

        let message = self.to_string();
        let status_text = "failed";

        let body = json!({
            "code": code,
            "message": message,
            "status": status_text,
            "data": null
        });

        Response::build()
            .status(status)
            .header(rocket::http::ContentType::JSON)
            .sized_body(body.to_string().len(), Cursor::new(body.to_string()))
            .ok()
    }
}