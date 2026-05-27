//! A Koto language module for async HTTP operations

use koto_runtime::{Result, derive::*, prelude::*};
use koto_serde::DeserializableKValue;
use reqwest::{Method, header::HeaderMap};

pub fn make_module() -> KMap {
    let result = KMap::with_type("http");
    let client = Client::new();

    result.add_fn("client", |ctx| match ctx.args() {
        [] => Ok(Client::new().into()),
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("request", {
        let client = client.clone();
        move |ctx| match ctx.args() {
            [KValue::Str(method), KValue::Str(url)] => {
                let method = parse_method(method)?;
                spawn_request(ctx.vm, client.inner.clone(), method, url.to_string(), None)
            }
            [KValue::Str(method), KValue::Str(url), KValue::Map(options)] => {
                let method = parse_method(method)?;
                let options = RequestOptions::from_map(options)?;
                spawn_request(
                    ctx.vm,
                    client.inner.clone(),
                    method,
                    url.to_string(),
                    Some(options),
                )
            }
            unexpected => unexpected_args("|String, String, Map?|", unexpected),
        }
    });

    result.add_fn("get", {
        let client = client.clone();
        move |ctx| request_from_args(ctx.vm, client.inner.clone(), Method::GET, ctx.args())
    });

    result.add_fn("delete", {
        let client = client.clone();
        move |ctx| request_from_args(ctx.vm, client.inner.clone(), Method::DELETE, ctx.args())
    });

    result.add_fn("post", {
        let client = client.clone();
        move |ctx| {
            request_with_optional_body(ctx.vm, client.inner.clone(), Method::POST, ctx.args())
        }
    });

    result.add_fn("put", {
        let client = client.clone();
        move |ctx| request_with_optional_body(ctx.vm, client.inner.clone(), Method::PUT, ctx.args())
    });

    result.add_fn("patch", {
        move |ctx| {
            request_with_optional_body(ctx.vm, client.inner.clone(), Method::PATCH, ctx.args())
        }
    });

    result
}

#[derive(Clone, KotoType, KotoCopy)]
#[koto(runtime = koto_runtime)]
pub struct Client {
    inner: reqwest::Client,
}

impl Client {
    fn new() -> Self {
        let inner = reqwest::Client::builder()
            .user_agent(concat!("koto_http/", env!("CARGO_PKG_VERSION")))
            .no_proxy()
            .build()
            .expect("the default http client configuration should be valid");

        Self { inner }
    }
}

#[koto_impl(runtime = koto_runtime)]
impl Client {
    #[koto_method]
    fn request(ctx: MethodContext<Self>) -> Result<KValue> {
        let client = ctx.instance()?.inner.clone();

        match ctx.args {
            [KValue::Str(method), KValue::Str(url)] => {
                let method = parse_method(method)?;
                spawn_request(ctx.vm, client, method, url.to_string(), None)
            }
            [KValue::Str(method), KValue::Str(url), KValue::Map(options)] => {
                let method = parse_method(method)?;
                let options = RequestOptions::from_map(options)?;
                spawn_request(ctx.vm, client, method, url.to_string(), Some(options))
            }
            unexpected => unexpected_args("|String, String, Map?|", unexpected),
        }
    }

    #[koto_method]
    fn get(ctx: MethodContext<Self>) -> Result<KValue> {
        let client = ctx.instance()?.inner.clone();
        request_from_args(ctx.vm, client, Method::GET, ctx.args)
    }

    #[koto_method]
    fn delete(ctx: MethodContext<Self>) -> Result<KValue> {
        let client = ctx.instance()?.inner.clone();
        request_from_args(ctx.vm, client, Method::DELETE, ctx.args)
    }

    #[koto_method]
    fn post(ctx: MethodContext<Self>) -> Result<KValue> {
        let client = ctx.instance()?.inner.clone();
        request_with_optional_body(ctx.vm, client, Method::POST, ctx.args)
    }

    #[koto_method]
    fn put(ctx: MethodContext<Self>) -> Result<KValue> {
        let client = ctx.instance()?.inner.clone();
        request_with_optional_body(ctx.vm, client, Method::PUT, ctx.args)
    }

    #[koto_method]
    fn patch(ctx: MethodContext<Self>) -> Result<KValue> {
        let client = ctx.instance()?.inner.clone();
        request_with_optional_body(ctx.vm, client, Method::PATCH, ctx.args)
    }
}

impl KotoObject for Client {}

impl From<Client> for KValue {
    fn from(client: Client) -> Self {
        KObject::from(client).into()
    }
}

#[derive(Clone, KotoType, KotoCopy)]
#[koto(runtime = koto_runtime)]
pub struct Response {
    status: u16,
    url: String,
    headers: KMap,
    body: Vec<u8>,
}

impl Response {
    fn header_text(&self, name: &str) -> Option<String> {
        self.headers
            .get(name.to_ascii_lowercase().as_str())
            .and_then(|value| match value {
                KValue::Str(value) => Some(value.to_string()),
                _ => None,
            })
    }
}

#[koto_impl(runtime = koto_runtime)]
impl Response {
    #[koto_method]
    fn status(&self) -> KNumber {
        self.status.into()
    }

    #[koto_method]
    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    #[koto_method]
    fn url(&self) -> &str {
        &self.url
    }

    #[koto_method]
    fn headers(&self) -> KMap {
        self.headers.clone()
    }

    #[koto_method]
    fn header(ctx: MethodContext<Self>) -> Result<KValue> {
        let expected_args = "|String|";

        match ctx.args {
            [KValue::Str(name)] => {
                let response = ctx.instance()?;
                Ok(response
                    .header_text(name.as_str())
                    .map(KValue::from)
                    .unwrap_or(KValue::Null))
            }
            unexpected => unexpected_args(expected_args, unexpected),
        }
    }

    #[koto_method]
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into()
    }

    #[koto_method]
    fn json(&self) -> Result<KValue> {
        match serde_json::from_slice::<DeserializableKValue>(&self.body) {
            Ok(result) => Ok(result.into()),
            Err(error) => runtime_error!("http.Response.json: {error}"),
        }
    }
}

impl KotoObject for Response {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("Response({})", self.status));
        Ok(())
    }
}

impl From<Response> for KValue {
    fn from(response: Response) -> Self {
        KObject::from(response).into()
    }
}

#[derive(Default)]
struct RequestOptions {
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl RequestOptions {
    fn from_map(options: &KMap) -> Result<Self> {
        let mut result = Self::default();

        for (key, value) in options.data().iter() {
            let KValue::Str(key) = key.value() else {
                return runtime_error!("http request option keys should be strings");
            };

            match key.as_str() {
                "headers" => result.headers = parse_headers(value)?,
                "body" => match value {
                    KValue::Str(body) => result.body = Some(body.to_string()),
                    unexpected => return unexpected_type("String", unexpected),
                },
                unexpected => {
                    return runtime_error!("unsupported http request option '{unexpected}'");
                }
            }
        }

        Ok(result)
    }
}

fn parse_method(method: &KString) -> Result<Method> {
    match Method::from_bytes(method.as_str().as_bytes()) {
        Ok(method) => Ok(method),
        Err(error) => runtime_error!("invalid http method '{method}': {error}"),
    }
}

fn request_from_args(
    vm: &KotoVm,
    client: reqwest::Client,
    method: Method,
    args: &[KValue],
) -> Result<KValue> {
    match args {
        [KValue::Str(url)] => spawn_request(vm, client, method, url.to_string(), None),
        [KValue::Str(url), KValue::Map(options)] => {
            let options = RequestOptions::from_map(options)?;
            spawn_request(vm, client, method, url.to_string(), Some(options))
        }
        unexpected => unexpected_args("|String, Map?|", unexpected),
    }
}

fn request_with_optional_body(
    vm: &KotoVm,
    client: reqwest::Client,
    method: Method,
    args: &[KValue],
) -> Result<KValue> {
    match args {
        [KValue::Str(url)] => spawn_request(vm, client, method, url.to_string(), None),
        [KValue::Str(url), KValue::Str(body)] => {
            let options = RequestOptions {
                body: Some(body.to_string()),
                ..Default::default()
            };
            spawn_request(vm, client, method, url.to_string(), Some(options))
        }
        [KValue::Str(url), KValue::Map(options)] => {
            let options = RequestOptions::from_map(options)?;
            spawn_request(vm, client, method, url.to_string(), Some(options))
        }
        [KValue::Str(url), KValue::Str(body), KValue::Map(options)] => {
            let mut options = RequestOptions::from_map(options)?;
            options.body = Some(body.to_string());
            spawn_request(vm, client, method, url.to_string(), Some(options))
        }
        unexpected => unexpected_args("|String, String?, Map?|", unexpected),
    }
}

fn spawn_request(
    vm: &KotoVm,
    client: reqwest::Client,
    method: Method,
    url: String,
    options: Option<RequestOptions>,
) -> Result<KValue> {
    let mut request = client.request(method.clone(), &url);

    if let Some(options) = options {
        for (name, value) in options.headers {
            request = request.header(name, value);
        }

        if let Some(body) = options.body {
            request = request.body(body);
        }
    }

    let method = method.as_str().to_string();
    let task = vm.spawn_future(async move {
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                return runtime_error!("http.{method}: request to '{url}' failed: {error}");
            }
        };

        let status = response.status().as_u16();
        let url = response.url().to_string();
        let headers = headers_to_map(response.headers());
        let body = match response.bytes().await {
            Ok(body) => body.to_vec(),
            Err(error) => {
                return runtime_error!("http.{method}: failed to read response body: {error}");
            }
        };

        Ok(Response {
            status,
            url,
            headers,
            body,
        }
        .into())
    })?;

    Ok(task.into())
}

fn parse_headers(value: &KValue) -> Result<Vec<(String, String)>> {
    let KValue::Map(headers) = value else {
        return unexpected_type("Map", value);
    };

    headers
        .data()
        .iter()
        .map(|(key, value)| {
            let KValue::Str(key) = key.value() else {
                return runtime_error!("http header names should be strings");
            };

            let KValue::Str(value) = value else {
                return unexpected_type("String", value);
            };

            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

fn headers_to_map(headers: &HeaderMap) -> KMap {
    let mut result = ValueMap::with_capacity(headers.len());

    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            result.insert(name.as_str().into(), value.into());
        }
    }

    KMap::from(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{CONTENT_TYPE, HeaderValue};

    #[test]
    fn response_json_and_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = Response {
            status: 200,
            url: "https://example.com".into(),
            headers: headers_to_map(&headers),
            body: br#"{"ok": true, "message": "hello"}"#.to_vec(),
        };

        assert_eq!(response.status(), KNumber::from(200));
        assert!(response.ok());
        assert_eq!(
            response.header_text("content-type").as_deref(),
            Some("application/json")
        );
        assert_eq!(response.text(), r#"{"ok": true, "message": "hello"}"#);

        let json = response.json().unwrap();
        let KValue::Map(json) = json else {
            panic!("expected a map");
        };

        assert!(matches!(json.get("ok"), Some(KValue::Bool(true))));
        assert!(matches!(json.get("message"), Some(KValue::Str(s)) if s.as_str() == "hello"));
    }

    #[test]
    fn request_options_parse_headers_and_body() {
        let headers = KMap::new();
        headers.insert("accept", "application/json");

        let options = KMap::new();
        options.insert("headers", headers);
        options.insert("body", "hello");

        let parsed = RequestOptions::from_map(&options).unwrap();

        assert_eq!(
            parsed.headers,
            vec![("accept".into(), "application/json".into())]
        );
        assert_eq!(parsed.body.as_deref(), Some("hello"));
    }
}
