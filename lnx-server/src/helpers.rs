use hyper::{Body, Request, Response};

use crate::error::Result;

pub type LnxRequest = Request<Body>;
pub type LnxResponse = Result<Response<Body>>;

#[macro_export]
macro_rules! abort {
    ($status:expr, $val:expr) => {{
        Err(crate::error::LnxError::AbortRequest(json_response(
            $status, $val,
        )?))
    }};
}

#[macro_export]
macro_rules! unauthorized {
    ($val:expr) => {{
        Err(crate::error::LnxError::UnAuthorized($val))
    }};
}

#[macro_export]
macro_rules! bad_request {
    ($val:expr) => {{
        Err(crate::error::LnxError::BadRequest($val))
    }};
}

#[macro_export]
macro_rules! json {
    ($body:expr) => {{
        use hyper::body::to_bytes;
        let body = to_bytes($body).await?;
        serde_json::from_slice(&body)?
    }};
}

#[macro_export]
macro_rules! get_or_400 {
    ($val:expr, $err_msg:expr) => {{
        match $val {
            None => return crate::bad_request!($err_msg),
            Some(v) => v,
        }
    }};

    ($val:expr) => {{
        match $val {
            None => return crate::bad_request!("missing required url parameter"),
            Some(v) => v,
        }
    }};
}
