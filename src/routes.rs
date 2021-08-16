use axum::body::BoxBody;
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;
use axum::extract::{Extension, Path, Query};

use engine::SearchEngine;
use engine::structures::QueryPayload;

use crate::responders::json_response;

/// Extracts a leased index or returns a json response
/// with a 400 status code.
macro_rules! get_index_or_reject {
    ($engine:expr, $name:expr) => {{
        match $engine.get_index(&$name).await {
            None => return json_response(
                StatusCode::BAD_REQUEST,
                format!("no index exists with name {}", &$name),
            ),
            Some(index) => index,
        }
    }}
}

/// Checks for any errors in the given operation.
///
/// If the error has a source (meaning it has consumed a underlying source)
/// if is marked as a 500 response and logged.
///
/// If the error has no source the request is rejected and marked as a 400,
/// this is also logged as a warning.
macro_rules! check_error {
    ($result:expr, $action:expr) => {{
        match $result {
            Ok(ok) => ok,
            // The error was not custom
            Err(ref e) if e.source().is_some() => {
                error!("failed to {} due to error: {:?}", $action, e);
                return json_response(StatusCode::INTERNAL_SERVER_ERROR, &())  // this will be over-ridden.
            },
            Err(e) => {
                warn!("rejecting {} operation due to bad request: {:?}", $action, &e);
                return json_response(StatusCode::BAD_REQUEST, &e.to_string())  // this will be over-ridden.

            }
        }
    }}
}

pub async fn search_index(
    query: Query<QueryPayload>,
    Path(index_name): Path<String>,
    Extension(engine): Extension<SearchEngine>,
) -> impl IntoResponse {
    let index = get_index_or_reject!(engine, index_name);
    let results = check_error!(index.search(*query).await, "search index");

    json_response(StatusCode::OK, &results)
}

pub async fn create_index(
    Extension(engine): Extension<SearchEngine>,
) -> impl IntoResponse {

}

pub async fn delete_index(
    Path(index_name): Path<String>,
    Extension(engine): Extension<SearchEngine>,
) -> impl IntoResponse {
    let index = get_index_or_reject!(engine, index_name);
}

pub async fn add_document(
    Path(index_name): Path<String>,
    Extension(engine): Extension<SearchEngine>,
) -> impl IntoResponse {
    let index = get_index_or_reject!(engine, index_name);
}

pub async fn get_document(
    Path(index_name): Path<String>,
    Path(document_id): Path<String>,
    Extension(engine): Extension<SearchEngine>,
) -> impl IntoResponse {
    let index = get_index_or_reject!(engine, index_name);
}

pub async fn delete_document(
    Path(index_name): Path<String>,
    Path(document_id): Path<String>,
    Extension(engine): Extension<SearchEngine>,
) -> impl IntoResponse {
    let index = get_index_or_reject!(engine, index_name);
}

pub async fn delete_all_documents(
    Path(index_name): Path<String>,
    Extension(engine): Extension<SearchEngine>,
) -> impl IntoResponse {
    let index = get_index_or_reject!(engine, index_name);
}

/// Modifies select responses.
///
/// If a response has a status code of 404, 405 or 500 a custom
/// response is used.
pub fn map_status(resp: Response<BoxBody>) -> Response<BoxBody> {
    let status = resp.status();
    if status == StatusCode::NOT_FOUND {
        return json_response(StatusCode::NOT_FOUND, "route not found");
    } else if status == StatusCode::METHOD_NOT_ALLOWED {
        return json_response(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
    } else if status == StatusCode::INTERNAL_SERVER_ERROR {
        return json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error while handling request",
        );
    }

    resp
}