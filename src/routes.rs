use serde::Deserialize;
use std::sync::Arc;

use axum::body::{box_body, Body, BoxBody};
use axum::extract::{self, Extension, Path, Query};
use axum::extract::rejection::{QueryRejection, JsonRejection, PathParamsRejection};
use axum::http::{Response, StatusCode};

use engine::structures::{IndexDeclaration, QueryPayload};
use engine::tantivy::Document;
use engine::{DocumentPayload, FromValue};
use engine::{LeasedIndex, SearchEngine};

use crate::responders::json_response;

type SharedEngine = Arc<SearchEngine>;

/// Extracts a leased index or returns a json response
/// with a 400 status code.
macro_rules! get_index_or_reject {
    ($engine:expr, $name:expr) => {{
        match $engine.get_index($name).await {
            None => {
                warn!("rejected request due to unknown index {:?}", $name);
                return json_response(
                    StatusCode::BAD_REQUEST,
                    &format!("no index exists with name {:?}", $name),
                );
            }
            Some(index) => index,
        }
    }};
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

/// Checks for any errors in parsing and extracting the path.
///
/// This is used to return a custom error message in a JSON format
/// verses axum's default text/plain response.
macro_rules! check_path {
    ($result:expr) => {{
        match $result {
            Ok(payload) => payload,
            Err(PathParamsRejection::InvalidPathParam(e)) =>
                return json_response(StatusCode::BAD_REQUEST, &format!("invalid path parameter {:?}", e)),
            Err(PathParamsRejection::MissingRouteParams(e)) =>
                return json_response(StatusCode::BAD_REQUEST, &format!("missing required route parameters: {:?}", e)),
            Err(e) =>
                return json_response(StatusCode::BAD_REQUEST, &format!("error with path handling: {:?}", e)),
        }
    }}
}

/// Checks for any errors in parsing and extracting the query.
///
/// This is used to return a custom error message in a JSON format
/// verses axum's default text/plain response.
macro_rules! check_query {
    ($result:expr) => {{
        match $result {
            Ok(payload) => payload,
            Err(QueryRejection::QueryStringMissing(_)) =>
                return json_response(StatusCode::BAD_REQUEST, "missing required query string"),
            Err(QueryRejection::FailedToDeserializeQueryString(e)) =>
                return json_response(StatusCode::BAD_REQUEST, &format!("failed to deserialize query string: {:?}", e)),
            Err(QueryRejection::UriAlreadyExtracted(_)) =>
                return json_response(StatusCode::BAD_REQUEST, "uri already extracted"),
            Err(e) =>
                return json_response(StatusCode::BAD_REQUEST, &format!("error with query string handling: {:?}", e)),
        }
    }}
}

/// Checks for any errors in parsing and extracting the json payload.
///
/// This is used to return a custom error message in a JSON format
/// verses axum's default text/plain response.
macro_rules! check_json {
    ($result:expr) => {{
        match $result {
            Ok(payload) => payload,
            Err(JsonRejection::MissingJsonContentType(_)) =>
                return json_response(StatusCode::BAD_REQUEST, "request missing application/json content type"),
            Err(JsonRejection::InvalidJsonBody(e)) =>
                return json_response(StatusCode::BAD_REQUEST, &format!("invalid JSON body: {:?}", e)),
            Err(JsonRejection::BodyAlreadyExtracted(_)) =>
                return json_response(StatusCode::BAD_REQUEST, "body already extracted"),
            Err(e) =>
                return json_response(StatusCode::BAD_REQUEST, &format!("error with json payload: {:?}", e)),
        }
    }}
}

/// Searches an index with a given query.
pub async fn search_index(
    query: Result<Query<QueryPayload>, QueryRejection>,
    index_name: Result<Path<String>, PathParamsRejection>,
    Extension(engine): Extension<SharedEngine>,
) -> Response<Body> {
    let query = check_query!(query);
    let index_name = Path(check_path!(index_name));

    let index: LeasedIndex = get_index_or_reject!(engine, &index_name);
    let results = check_error!(index.search(query.0).await, "search index");

    json_response(StatusCode::OK, &results)
}

/// The given set of query parameters available to the create
/// index function.
#[derive(Deserialize)]
pub struct CreateIndexQueryParams {
    /// If true this will delete the old index if it existed. (defaults to false)
    override_if_exists: Option<bool>,
}

/// Creates a index / overrides an index with the given payload.
pub async fn create_index(
    query: Result<Query<CreateIndexQueryParams>, QueryRejection>,
    payload: Result<extract::Json<IndexDeclaration>, JsonRejection>,
    Extension(engine): Extension<SharedEngine>,
) -> Response<Body> {
    let query = check_query!(query);
    let payload = check_json!(payload);

    let ignore = query.0;
    check_error!(
        engine
            .add_index(payload.0, ignore.override_if_exists.unwrap_or(false))
            .await,
        "create index"
    );

    json_response(StatusCode::OK, "index created")
}

/// Deletes the given index if it exists.
pub async fn delete_index(
    index_name: Result<Path<String>, PathParamsRejection>,
    Extension(engine): Extension<SharedEngine>,
) -> Response<Body> {
    let index_name = Path(check_path!(index_name));

    check_error!(engine.remove_index(&index_name).await, "delete index");

    json_response(StatusCode::OK, "index deleted")
}

/// The set of query operations that can be given when writing
/// to an index.
#[derive(Deserialize)]
pub struct PendingQueries {
    /// If false this will return immediately without waiting for
    /// all operations to be submitted. (defaults to true).
    ///
    /// It's recommend to wait for the operation to be submitted for
    /// the purposes of backpressure.
    wait: Option<bool>,
}

/// The possible formats for uploading documents.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum DocumentOptions {
    /// A singular document payload.
    Single(DocumentPayload),

    /// An array of documents acting as a bulk insertion.
    Many(Vec<DocumentPayload>),
}

/// Adds one or more documents to the given index.
///
/// This can either return immediately or wait for all operations to be
/// submitted depending on the `wait` query parameter.
pub async fn add_document(
    query: Result<Query<PendingQueries>, QueryRejection>,
    index_name: Result<Path<String>, PathParamsRejection>,
    payload: Result<extract::Json<DocumentOptions>, JsonRejection>,
    Extension(engine): Extension<SharedEngine>,
) -> Response<Body> {
    let index_name = Path(check_path!(index_name));
    let query = check_query!(query);
    let payload = check_json!(payload);

    let index: LeasedIndex = get_index_or_reject!(engine, &index_name);

    let schema = index.schema();
    let wait = query.0.wait.unwrap_or(true);

    match payload.0 {
        DocumentOptions::Single(doc) => {
            let document = check_error!(
                Document::from_value_map(doc, &schema),
                "load document from raw"
            );
            if wait {
                check_error!(index.add_document(document).await, "add document");
            } else {
                tokio::spawn(async move {
                    if let Err(e) = index.add_document(document).await {
                        error!("failed to add document {:?}", e);
                    }
                });
            }
        }
        DocumentOptions::Many(docs) => {
            let documents = check_error!(
                Document::from_many_value_map(docs, &schema),
                "load many documents from raw"
            );
            if wait {
                check_error!(index.add_many_documents(documents).await, "add documents");
            } else {
                tokio::spawn(async move {
                    if let Err(e) = index.add_many_documents(documents).await {
                        error!("failed to add documents {:?}", e);
                    }
                });
            }
        }
    }

    json_response(
        StatusCode::OK,
        if wait {
            "added documents"
        } else {
            "submitted documents"
        },
    )
}

pub async fn get_document(
    index_name: Result<Path<String>, PathParamsRejection>,
    document_id: Result<Path<String>, PathParamsRejection>,
    Extension(engine): Extension<SharedEngine>,
) -> Response<Body> {
    let index_name = Path(check_path!(index_name));
    let _document_id = Path(check_path!(document_id));

    let _index: LeasedIndex = get_index_or_reject!(engine, &index_name);

    json_response(StatusCode::OK, &())
}

pub async fn delete_document(
    index_name: Result<Path<String>, PathParamsRejection>,
    document_id: Result<Path<String>, PathParamsRejection>,
    Extension(engine): Extension<SharedEngine>,
) -> Response<Body> {
    let index_name = Path(check_path!(index_name));
    let _document_id = Path(check_path!(document_id));

    let _index: LeasedIndex = get_index_or_reject!(engine, &index_name);

    json_response(StatusCode::OK, &())
}

pub async fn delete_all_documents(
    index_name: Result<Path<String>, PathParamsRejection>,
    Extension(engine): Extension<SharedEngine>,
) -> Response<Body> {
    let index_name = Path(check_path!(index_name));

    let _index: LeasedIndex = get_index_or_reject!(engine, &index_name);

    json_response(StatusCode::OK, &())
}

/// Converts an arbitary Response<Body> into Response<BoxBody>
fn to_box_body(resp: Response<Body>) -> Response<BoxBody> {
    let (parts, body) = resp.into_parts();
    let body = box_body(body);

    Response::from_parts(parts, body)
}

/// Modifies select responses.
///
/// If a response has a status code of 404, 405 or 500 a custom
/// response is used.
pub fn map_status(resp: Response<BoxBody>) -> Response<BoxBody> {
    let status = resp.status();
    if status == StatusCode::NOT_FOUND {
        return to_box_body(json_response(StatusCode::NOT_FOUND, "route not found"));
    } else if status == StatusCode::METHOD_NOT_ALLOWED {
        return to_box_body(json_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed",
        ));
    } else if status == StatusCode::INTERNAL_SERVER_ERROR {
        return to_box_body(json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error while handling request",
        ));
    }

    resp
}
