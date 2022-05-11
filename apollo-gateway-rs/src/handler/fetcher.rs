use std::sync::atomic::{AtomicU64, Ordering};
use anyhow::Result;
use crate::planner::{RequestData, Response};
use tokio::sync::mpsc;
use crate::datasource::{Context, RemoteGraphQLDataSource};
use crate::handler::websocket::WebSocketController;
use crate::{GraphqlSourceMiddleware, ServiceRouteTable};

#[async_trait::async_trait]
pub trait Fetcher: Send + Sync {
    async fn query(&self, service: &str, request: RequestData) -> Result<Response>;
}

pub struct HttpFetcher<'a, S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> {
    router_table: &'a ServiceRouteTable<S>,
    pub ctx: Context
}

impl<'a, S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> HttpFetcher<'a, S> {
    pub fn new(router_table: &'a ServiceRouteTable<S>, ctx: Context) -> Self {
        Self {
            router_table,
            ctx
        }
    }
}

#[async_trait::async_trait]
impl<'a, S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> Fetcher for HttpFetcher<'a, S> {
    async fn query(&self, service: &str, request: RequestData) -> Result<Response> {
        self.router_table
            .query(service, request, &self.ctx)
            .await
    }
}

pub struct WebSocketFetcher {
    controller: WebSocketController,
    id: AtomicU64,
}

impl WebSocketFetcher {
    pub fn new(controller: WebSocketController) -> Self {
        Self {
            controller,
            id: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl Fetcher for WebSocketFetcher {
    async fn query(&self, service: &str, request: RequestData) -> Result<Response> {
        let id = self.id.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.controller
            .subscribe(format!("__req{}", id), service, request, tx)
            .await?;
        rx.recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Connection closed."))
    }
}
