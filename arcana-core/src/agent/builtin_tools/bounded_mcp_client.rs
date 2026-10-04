//! Bounded raw HTTP backend for RMCP's Streamable HTTP client.
//!
//! RMCP's built-in reqwest adapter bounds individual SSE events, but its JSON
//! response paths call `bytes()`/`text()` without a wire-size limit. This
//! adapter applies a total 1 MiB response limit before JSON or SSE parsing.

use std::{borrow::Cow, collections::HashMap, error::Error, fmt, sync::Arc};

use futures::{stream::BoxStream, StreamExt};
use http::{header, HeaderName, HeaderValue};
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest, ErrorData, ServerJsonRpcMessage},
    transport::streamable_http_client::{
        AuthRequiredError, InsufficientScopeError, SseError, StreamableHttpClient,
        StreamableHttpClientTransport, StreamableHttpClientTransportConfig, StreamableHttpError,
        StreamableHttpPostResponse,
    },
};
use sse_stream::{Sse, SseStream};

pub const MAX_MCP_HTTP_RESPONSE_BYTES: usize = 1024 * 1024;
const ACCEPT_VALUE: &str = "text/event-stream, application/json";
const SESSION_HEADER: &str = "Mcp-Session-Id";
const LAST_EVENT_HEADER: &str = "Last-Event-Id";

#[derive(Clone)]
pub struct BoundedMcpHttpClient {
    client: reqwest::Client,
}

#[derive(Debug)]
pub struct McpHttpError;

impl fmt::Display for McpHttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MCP HTTP request failed")
    }
}

impl Error for McpHttpError {}

fn http_client_error(_: reqwest::Error) -> StreamableHttpError<McpHttpError> {
    StreamableHttpError::Client(McpHttpError)
}

impl BoundedMcpHttpClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: reqwest::Client::builder()
                .pool_max_idle_per_host(0)
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }

    pub fn transport(
        self,
        config: StreamableHttpClientTransportConfig,
    ) -> StreamableHttpClientTransport<Self> {
        StreamableHttpClientTransport::with_client(self, config)
    }
}

impl StreamableHttpClient for BoundedMcpHttpClient {
    type Error = McpHttpError;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.post_message_with_max_sse_event_size(
            uri,
            message,
            session_id,
            auth_header,
            custom_headers,
            MAX_MCP_HTTP_RESPONSE_BYTES,
        )
        .await
    }

    async fn post_message_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let payload = serde_json::to_vec(&message)?;
        let mut request = self
            .client
            .post(uri.as_ref())
            .header(header::ACCEPT, ACCEPT_VALUE);
        if let Some(session) = session_id.as_deref() {
            request = request.header(SESSION_HEADER, session);
        }
        if let Some(token) = auth_header {
            request = request.bearer_auth(token);
        }
        request = apply_custom_headers(request, custom_headers)?;
        request = request
            .header(header::CONTENT_TYPE, "application/json")
            .body(payload);
        let response = request.send().await.map_err(http_client_error)?;
        if let Some(error) = authorization_error(&response)? {
            return Err(error);
        }

        let status = response.status();
        if matches!(
            status,
            reqwest::StatusCode::ACCEPTED | reqwest::StatusCode::NO_CONTENT
        ) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status == reqwest::StatusCode::NOT_FOUND && session_id.is_some() {
            return Err(StreamableHttpError::SessionExpired);
        }

        let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
        let response_session = bounded_header(response.headers(), SESSION_HEADER)?;
        if status.is_success()
            && response.content_length() == Some(0)
            && matches!(
                message,
                ClientJsonRpcMessage::Notification(_)
                    | ClientJsonRpcMessage::Response(_)
                    | ClientJsonRpcMessage::Error(_)
            )
        {
            return Ok(StreamableHttpPostResponse::Accepted);
        }

        if !status.is_success() {
            let body = read_bounded_body(response, MAX_MCP_HTTP_RESPONSE_BYTES).await?;
            let parsed_error = if content_type
                .as_ref()
                .is_some_and(|value| value.as_bytes().starts_with(b"application/json"))
            {
                if let Ok(parsed @ ServerJsonRpcMessage::Error(_)) =
                    serde_json::from_slice::<ServerJsonRpcMessage>(&body)
                {
                    Some(parsed)
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(parsed) = parsed_error.as_ref() {
                return Ok(StreamableHttpPostResponse::Json(
                    parsed.clone(),
                    response_session,
                ));
            }
            // Preserve RMCP's legacy discovery negotiation: some older MCP
            // servers reject `server/discover` with HTTP 4xx rather than a
            // correlated JSON-RPC error. Re-correlate that response so RMCP
            // can retry its supported protocol versions.
            if session_id.is_none() && status.is_client_error() {
                if let ClientJsonRpcMessage::Request(request) = &message {
                    if matches!(request.request, ClientRequest::DiscoverRequest(_)) {
                        let error = match parsed_error {
                            Some(ServerJsonRpcMessage::Error(error)) => error.error,
                            _ => ErrorData::invalid_request(
                                format!("server/discover rejected with HTTP {status}"),
                                None,
                            ),
                        };
                        return Ok(StreamableHttpPostResponse::Json(
                            ServerJsonRpcMessage::error(error, Some(request.id.clone())),
                            None,
                        ));
                    }
                }
            }
            return Err(StreamableHttpError::UnexpectedServerResponse(Cow::Owned(
                format!("MCP endpoint returned HTTP {status}"),
            )));
        }

        match content_type.as_ref().map(HeaderValue::as_bytes) {
            Some(value) if value.starts_with(b"text/event-stream") => {
                let stream = bounded_sse_stream(response, max_sse_event_size);
                Ok(StreamableHttpPostResponse::Sse(stream, response_session))
            }
            Some(value) if value.starts_with(b"application/json") => {
                let body = read_bounded_body(response, MAX_MCP_HTTP_RESPONSE_BYTES).await?;
                match serde_json::from_slice::<ServerJsonRpcMessage>(&body) {
                    Ok(parsed) => Ok(StreamableHttpPostResponse::Json(parsed, response_session)),
                    Err(_)
                        if matches!(
                            message,
                            ClientJsonRpcMessage::Notification(_)
                                | ClientJsonRpcMessage::Response(_)
                                | ClientJsonRpcMessage::Error(_)
                        ) =>
                    {
                        Ok(StreamableHttpPostResponse::Accepted)
                    }
                    Err(error) => Err(StreamableHttpError::UnexpectedServerResponse(Cow::Owned(
                        format!("MCP returned an invalid JSON-RPC response: {error}"),
                    ))),
                }
            }
            _ => Err(StreamableHttpError::UnexpectedContentType(None)),
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        let mut request = self
            .client
            .delete(uri.as_ref())
            .header(SESSION_HEADER, session_id.as_ref());
        if let Some(token) = auth_header {
            request = request.bearer_auth(token);
        }
        request = apply_custom_headers(request, custom_headers)?;
        let response = request.send().await.map_err(http_client_error)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        response.error_for_status().map_err(http_client_error)?;
        Ok(())
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        self.get_stream_with_max_sse_event_size(
            uri,
            session_id,
            last_event_id,
            auth_header,
            custom_headers,
            MAX_MCP_HTTP_RESPONSE_BYTES,
        )
        .await
    }

    async fn get_stream_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max_sse_event_size: usize,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        let mut request = self
            .client
            .get(uri.as_ref())
            .header(header::ACCEPT, ACCEPT_VALUE);
        if let Some(session) = session_id.as_deref() {
            request = request.header(SESSION_HEADER, session);
        }
        if let Some(last_event) = last_event_id {
            request = request.header(LAST_EVENT_HEADER, last_event);
        }
        if let Some(token) = auth_header {
            request = request.bearer_auth(token);
        }
        request = apply_custom_headers(request, custom_headers)?;
        let response = request.send().await.map_err(http_client_error)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        if let Some(error) = authorization_error(&response)? {
            return Err(error);
        }
        let response = response.error_for_status().map_err(http_client_error)?;
        if response
            .headers()
            .get(header::CONTENT_TYPE)
            .map_or(true, |content_type| {
                let value = content_type.as_bytes();
                !value.starts_with(b"text/event-stream") && !value.starts_with(b"application/json")
            })
        {
            return Err(StreamableHttpError::UnexpectedContentType(None));
        }
        ensure_content_length(response.content_length(), MAX_MCP_HTTP_RESPONSE_BYTES)?;
        Ok(bounded_sse_stream(response, max_sse_event_size))
    }
}

fn apply_custom_headers(
    mut request: reqwest::RequestBuilder,
    headers: HashMap<HeaderName, HeaderValue>,
) -> Result<reqwest::RequestBuilder, StreamableHttpError<McpHttpError>> {
    for (name, value) in headers {
        if matches!(name.as_str(), "accept" | "mcp-session-id" | "last-event-id") {
            return Err(StreamableHttpError::ReservedHeaderConflict(
                name.to_string(),
            ));
        }
        request = request.header(name, value);
    }
    Ok(request)
}

fn authorization_error(
    response: &reqwest::Response,
) -> Result<Option<StreamableHttpError<McpHttpError>>, StreamableHttpError<McpHttpError>> {
    let status = response.status();
    if !matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        return Ok(None);
    }
    let Some(challenge) = response.headers().get(header::WWW_AUTHENTICATE) else {
        return Ok(None);
    };
    let challenge = challenge.to_str().map_err(|_| {
        StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(
            "MCP endpoint returned an invalid authentication challenge",
        ))
    })?;
    let challenge = challenge.chars().take(512).collect::<String>();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        Ok(Some(StreamableHttpError::AuthRequired(
            AuthRequiredError::new(challenge),
        )))
    } else {
        Ok(Some(StreamableHttpError::InsufficientScope(
            InsufficientScopeError::new(challenge, None),
        )))
    }
}

fn bounded_header(
    headers: &header::HeaderMap,
    name: &'static str,
) -> Result<Option<String>, StreamableHttpError<McpHttpError>> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| {
        StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(
            "MCP endpoint returned an invalid session header",
        ))
    })?;
    if value.len() > 1024 {
        return Err(StreamableHttpError::UnexpectedServerResponse(
            Cow::Borrowed("MCP session header exceeded Arcana's size limit"),
        ));
    }
    Ok(Some(value.to_owned()))
}

async fn read_bounded_body(
    response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, StreamableHttpError<McpHttpError>> {
    ensure_content_length(response.content_length(), limit)?;
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(http_client_error)? {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(response_limit_error());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn ensure_content_length(
    length: Option<u64>,
    limit: usize,
) -> Result<(), StreamableHttpError<McpHttpError>> {
    if length.is_some_and(|length| length > limit as u64) {
        Err(response_limit_error())
    } else {
        Ok(())
    }
}

fn response_limit_error() -> StreamableHttpError<McpHttpError> {
    StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(
        "MCP response exceeded Arcana's 1 MiB wire limit",
    ))
}

#[derive(Debug)]
struct ResponseLimitExceeded;

impl fmt::Display for ResponseLimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MCP response exceeded Arcana's 1 MiB wire limit")
    }
}

impl Error for ResponseLimitExceeded {}

fn bounded_sse_stream(
    response: reqwest::Response,
    max_event_size: usize,
) -> BoxStream<'static, Result<Sse, SseError>> {
    let limit = MAX_MCP_HTTP_RESPONSE_BYTES.min(max_event_size);
    let source = response
        .bytes_stream()
        .map(|result| result.map_err(|error| SseError::Body(Box::new(error))));
    let limited = futures::stream::unfold(
        (source, 0usize, false),
        move |(mut source, received, failed)| async move {
            if failed {
                return None;
            }
            match source.next().await {
                Some(Ok(chunk)) if received.saturating_add(chunk.len()) <= limit => {
                    Some((Ok(chunk.clone()), (source, received + chunk.len(), false)))
                }
                Some(Ok(_)) => Some((
                    Err(SseError::Body(Box::new(ResponseLimitExceeded))),
                    (source, received, true),
                )),
                Some(Err(error)) => Some((Err(error), (source, received, true))),
                None => None,
            }
        },
    );
    SseStream::from_bytes_stream(limited).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::ClientJsonRpcMessage;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn response_length_guard_accepts_boundary_and_rejects_larger_body() {
        assert!(ensure_content_length(
            Some(MAX_MCP_HTTP_RESPONSE_BYTES as u64),
            MAX_MCP_HTTP_RESPONSE_BYTES
        )
        .is_ok());
        assert!(ensure_content_length(
            Some(MAX_MCP_HTTP_RESPONSE_BYTES as u64 + 1),
            MAX_MCP_HTTP_RESPONSE_BYTES
        )
        .is_err());
    }

    #[tokio::test]
    async fn streamable_http_transport_rejects_oversized_chunked_json_before_parsing() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&buffer[..count]);
            }
            let header_end = request
                .windows(4)
                .position(|part| part == b"\r\n\r\n")
                .unwrap()
                + 4;
            let request_headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = request_headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap();
            while request.len() < header_end + content_length {
                let count = socket.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&buffer[..count]);
            }
            let oversized_payload = "x".repeat(MAX_MCP_HTTP_RESPONSE_BYTES + 1);
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:X}\r\n{}\r\n0\r\n\r\n",
                        oversized_payload.len(), oversized_payload
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });

        let client = BoundedMcpHttpClient::new().unwrap();
        let request: ClientJsonRpcMessage = serde_json::from_value(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ping"
        }))
        .unwrap();
        let error = client
            .post_message(
                Arc::from(format!("http://{address}/mcp")),
                request,
                None,
                None,
                HashMap::new(),
            )
            .await
            .expect_err("oversized raw body must be rejected");
        assert!(error.to_string().contains("1 MiB wire limit"), "{error}");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn streamable_http_transport_bounds_raw_sse_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&buffer[..count]);
            }
            let oversized_event =
                format!("data: {}\n\n", "x".repeat(MAX_MCP_HTTP_RESPONSE_BYTES + 1));
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:X}\r\n{}\r\n0\r\n\r\n",
                        oversized_event.len(), oversized_event
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });

        let client = BoundedMcpHttpClient::new().unwrap();
        let mut events = client
            .get_stream(
                Arc::from(format!("http://{address}/mcp")),
                None,
                None,
                None,
                HashMap::new(),
            )
            .await
            .unwrap();
        let error = events
            .next()
            .await
            .expect("oversized SSE response must fail")
            .expect_err("raw SSE limit must be enforced");
        assert!(error.to_string().contains("1 MiB wire limit"), "{error}");
        server.await.unwrap();
    }
}
