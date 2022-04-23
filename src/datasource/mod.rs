use std::ops::Deref;
use std::sync::Arc;
use actix_web::HttpRequest;
use crate::planner::{Request, Response};

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
        let protocol = self.tls().then(|| "https").unwrap_or("http");
        let path = self.query_path().unwrap_or("");
        format!("{protocol}://{address}/{path}")
    }
    fn url_subscription(&self) -> String {
        let address = self.address();
        let protocol = self.tls().then(|| "wss").unwrap_or("ws");
        let path = self.subscribe_path().unwrap_or("");
        format!("{protocol}://{address}/{path}")
    }
}

#[async_trait::async_trait]
pub trait GraphqlSourceMiddleware: Send + Sync + 'static {
    /// Override willSendRequest to modify your gateway's requests to the subgraph before they're sent.
    #[allow(unused_variables)]
    async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> anyhow::Result<()> {
        Ok(())
    }
    /// Override willSendRequest to modify your gateway's requests to the subgraph before they're sent.
    #[allow(unused_variables)]
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        Ok(())
    }
}

impl RemoteGraphQLDataSource for Arc<dyn GraphqlSource> {
    fn name(&self) -> &str {
        self.deref().name()
    }
    fn address(&self) -> &str {
        self.deref().address()
    }
    fn tls(&self) -> bool {
        self.deref().tls()
    }
    fn query_path(&self) -> Option<&str> {
        self.deref().query_path()
    }
    fn subscribe_path(&self) -> Option<&str> {
        self.deref().subscribe_path()
    }
    fn url_query(&self) -> String {
        self.deref().url_query()
    }
    fn url_subscription(&self) -> String {
        self.deref().url_subscription()
    }
}

#[async_trait::async_trait]
impl GraphqlSourceMiddleware for Arc<dyn GraphqlSource> {
    async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> anyhow::Result<()> {
        self.deref().will_send_request(request, ctx).await
    }
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        self.deref().did_receive_response(response, ctx).await
    }
}

impl GraphqlSource for Arc<dyn GraphqlSource> {}


pub trait GraphqlSource: RemoteGraphQLDataSource + GraphqlSourceMiddleware {}


pub struct SimpleSource<S: RemoteGraphQLDataSource> {
    pub(crate) source: S,
}

impl<S: RemoteGraphQLDataSource> GraphqlSourceMiddleware for SimpleSource<S> {}

impl<S: RemoteGraphQLDataSource> RemoteGraphQLDataSource for SimpleSource<S> {
    fn name(&self) -> &str {
        self.source.name()
    }
    fn address(&self) -> &str {
        self.source.address()
    }
    fn tls(&self) -> bool {
        self.source.tls()
    }
    fn query_path(&self) -> Option<&str> {
        self.source.query_path()
    }
    fn subscribe_path(&self) -> Option<&str> {
        self.source.subscribe_path()
    }
    fn url_query(&self) -> String {
        self.source.url_query()
    }
    fn url_subscription(&self) -> String {
        self.source.url_subscription()
    }
}

impl<S: RemoteGraphQLDataSource> GraphqlSource for SimpleSource<S> {}

pub struct Source<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> {
    pub(crate) source: S,
}

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> RemoteGraphQLDataSource for Source<S> {
    fn name(&self) -> &str {
        self.source.name()
    }
    fn address(&self) -> &str {
        self.source.address()
    }
    fn tls(&self) -> bool {
        self.source.tls()
    }
    fn query_path(&self) -> Option<&str> {
        self.source.query_path()
    }
    fn subscribe_path(&self) -> Option<&str> {
        self.source.subscribe_path()
    }
    fn url_query(&self) -> String {
        self.source.url_query()
    }
    fn url_subscription(&self) -> String {
        self.source.url_subscription()
    }
}

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> GraphqlSource for Source<S> {}

#[async_trait::async_trait]
impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> GraphqlSourceMiddleware for Source<S> {
    async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> anyhow::Result<()> {
        self.source.will_send_request(request, ctx).await
    }
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        self.source.did_receive_response(response, ctx).await
    }
}

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

