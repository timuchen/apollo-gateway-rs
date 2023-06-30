#![allow(clippy::obfuscated_if_else)]

use std::collections::HashMap;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use actix::dev::Stream;
use actix_web::HttpRequest;
use futures_util::TryFutureExt;
use http::HeaderMap;
use once_cell::sync::Lazy;
use crate::planner::{Response};

/// Represents a connection between your federated gateway and one of your subgraphs.
pub trait RemoteGraphQLDataSource: Sync + Send + 'static {
    /// If you have a multiple sources they must have a unique name
    fn name(&self) -> &str;
    /// Example countries.trevorblades.com You shouldn`t use http(s)://
    fn address(&self) -> &str;
    fn tls(&self) -> bool { false }
    fn query_path(&self) -> Option<&str> { None }
    fn subscribe_path(&self) -> Option<&str> { None }
    fn url_query(&self) -> String {
        let address = self.address();
        let protocol = self.tls().then_some("https").unwrap_or("http");
        let path = self.query_path().unwrap_or("");
        format!("{protocol}://{address}/{path}")
    }
    fn url_subscription(&self) -> String {
        let address = self.address();
        let protocol = self.tls().then_some("wss").unwrap_or("ws");
        let path = self.subscribe_path().unwrap_or("");
        format!("{protocol}://{address}/{path}")
    }
}

use serde::Deserialize;
use serde_json::Value;
use crate::Request;

#[derive(Deserialize)]
pub struct Config<S> {
    sources: Vec<S>,
}

/// If you want to load your sources from config you can use DefaultSource. If you not provide tls in your config default value would be false
#[derive(Deserialize)]
pub struct DefaultSource {
    name: String,
    address: String,
    #[serde(default = "bool::default")]
    tls: bool,
    query_path: Option<String>,
    subscribe_path: Option<String>,
}

impl RemoteGraphQLDataSource for DefaultSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn address(&self) -> &str {
        &self.address
    }
    fn tls(&self) -> bool {
        self.tls
    }
    fn query_path(&self) -> Option<&str> {
        self.query_path.as_deref()
    }
    fn subscribe_path(&self) -> Option<&str> {
        self.subscribe_path.as_deref()
    }
}

impl<S: RemoteGraphQLDataSource> Config<S> {
    pub fn simple_sources(self) -> HashMap<String, Arc<dyn GraphqlSource>> {
        self.sources.into_iter()
            .map(|source| (source.name().to_string(), Arc::new(SimpleSource { source }) as Arc<dyn GraphqlSource>))
            .collect::<HashMap<String, Arc<dyn GraphqlSource>>>()
    }
}

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> Config<S> {
    pub fn sources(self) -> HashMap<String, Arc<dyn GraphqlSource>> {
        self.sources.into_iter()
            .map(|source| (source.name().to_string(), Arc::new(Source { source }) as Arc<dyn GraphqlSource>))
            .collect::<HashMap<String, Arc<dyn GraphqlSource>>>()
    }
}


static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(Default::default);

type SubscriptionStream = Pin<Box<dyn Stream<Item = anyhow::Result<Response>>>>;
/// Implement GraphqlSourceMiddleware for your source, if you want to modify requests to the subgraph before they're sent and modify response after it.
#[async_trait::async_trait]
pub trait GraphqlSourceMiddleware: Send + Sync + 'static + RemoteGraphQLDataSource {
    /// Override will_send_request to modify your gateway's requests to the subgraph before they're sent.
    #[allow(unused_variables)]
    async fn will_send_request(&self, request: &mut HashMap<String, String>, ctx: &Context) -> anyhow::Result<()> {
        Ok(())
    }
    /// Override did_receive_response to modify your gateway's response after request to the subgraph. It will not modify response of subscription.
    #[allow(unused_variables)]
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        Ok(())
    }

    #[allow(unused_variables)]
    async fn on_connection_init(&self, message: &mut Option<Value>, ctx: &Context) -> anyhow::Result<()> {
        Ok(())
    }

    async fn fetch(&self, request: Request) -> anyhow::Result<Response> {
        let url = self.url_query();
        let headers = HeaderMap::try_from(&request.headers)?;
        let raw_resp = HTTP_CLIENT
            .post(&url)
            .headers(headers)
            .json(&request.data)
            .send()
            .and_then(|res| async move { res.error_for_status() })
            .await?;
        let headers = raw_resp.headers().iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str().to_string(), value.to_string())))
            .collect();
        let mut resp = raw_resp.json::<Response>().await?;
        if !resp.errors.is_empty() {

        }
        resp.headers = headers;
        Ok(resp)
    }
    async fn subscribe(&self, _request: Request) -> SubscriptionStream {
        unimplemented!()
    }
}

impl RemoteGraphQLDataSource for Arc<dyn GraphqlSource> {
    #[inline]
    fn name(&self) -> &str {
        self.deref().name()
    }
    #[inline]
    fn address(&self) -> &str {
        self.deref().address()
    }
    #[inline]
    fn tls(&self) -> bool {
        self.deref().tls()
    }
    #[inline]
    fn query_path(&self) -> Option<&str> {
        self.deref().query_path()
    }
    #[inline]
    fn subscribe_path(&self) -> Option<&str> {
        self.deref().subscribe_path()
    }
    #[inline]
    fn url_query(&self) -> String {
        self.deref().url_query()
    }
    #[inline]
    fn url_subscription(&self) -> String {
        self.deref().url_subscription()
    }
}

#[async_trait::async_trait]
impl GraphqlSourceMiddleware for Arc<dyn GraphqlSource> {
    async fn will_send_request(&self, request: &mut HashMap<String, String>, ctx: &Context) -> anyhow::Result<()> {
        self.deref().will_send_request(request, ctx).await
    }
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        self.deref().did_receive_response(response, ctx).await
    }
    async fn on_connection_init(&self, message: &mut Option<Value>, ctx: &Context) -> anyhow::Result<()> {
        self.deref().on_connection_init(message, ctx).await
    }
    async fn fetch(&self, request: Request) -> anyhow::Result<Response> {
        self.deref().fetch(request).await
    }
    async fn subscribe(&self, request: Request) -> SubscriptionStream {
        self.deref().subscribe(request).await
    }
}

impl GraphqlSource for Arc<dyn GraphqlSource> {}

pub trait GraphqlSource: RemoteGraphQLDataSource + GraphqlSourceMiddleware {}


pub struct SimpleSource<S: RemoteGraphQLDataSource> {
    pub(crate) source: S,
}

impl<S: RemoteGraphQLDataSource> GraphqlSourceMiddleware for SimpleSource<S> {}

impl<S: RemoteGraphQLDataSource> RemoteGraphQLDataSource for SimpleSource<S> {
    #[inline]
    fn name(&self) -> &str {
        self.source.name()
    }
    #[inline]
    fn address(&self) -> &str {
        self.source.address()
    }
    #[inline]
    fn tls(&self) -> bool {
        self.source.tls()
    }
    #[inline]
    fn query_path(&self) -> Option<&str> {
        self.source.query_path()
    }
    #[inline]
    fn subscribe_path(&self) -> Option<&str> {
        self.source.subscribe_path()
    }
    #[inline]
    fn url_query(&self) -> String {
        self.source.url_query()
    }
    #[inline]
    fn url_subscription(&self) -> String {
        self.source.url_subscription()
    }
}

impl<S: RemoteGraphQLDataSource> GraphqlSource for SimpleSource<S> {}

pub struct Source<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> {
    pub(crate) source: S,
}

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> RemoteGraphQLDataSource for Source<S> {
    #[inline]
    fn name(&self) -> &str {
        self.source.name()
    }
    #[inline]
    fn address(&self) -> &str {
        self.source.address()
    }
    #[inline]
    fn tls(&self) -> bool {
        self.source.tls()
    }
    #[inline]
    fn query_path(&self) -> Option<&str> {
        self.source.query_path()
    }
    #[inline]
    fn subscribe_path(&self) -> Option<&str> {
        self.source.subscribe_path()
    }
    #[inline]
    fn url_query(&self) -> String {
        self.source.url_query()
    }
    #[inline]
    fn url_subscription(&self) -> String {
        self.source.url_subscription()
    }
}

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> GraphqlSource for Source<S> {}

#[async_trait::async_trait]
impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> GraphqlSourceMiddleware for Source<S> {
    async fn will_send_request(&self, request: &mut HashMap<String, String>, ctx: &Context) -> anyhow::Result<()> {
        self.source.will_send_request(request, ctx).await
    }
    async fn on_connection_init(&self, message: &mut Option<Value>, ctx: &Context) -> anyhow::Result<()> {
        self.source.on_connection_init(message, ctx).await
    }
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        self.source.did_receive_response(response, ctx).await
    }
    async fn fetch(&self, request: Request) -> anyhow::Result<Response> {
        self.source.fetch(request).await
    }
    async fn subscribe(&self, request: Request) -> SubscriptionStream {
        self.source.subscribe(request).await
    }
}
/// Context give you access to request data like headers, app_data and extensions.
pub struct Context(HttpRequest);

impl Context {
    pub fn new(request: HttpRequest) -> Self {
        Self(request)
    }
}

impl Deref for Context {
    type Target = HttpRequest;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl Send for Context {}

unsafe impl Sync for Context {}

