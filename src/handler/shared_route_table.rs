use std::sync::Arc;
use actix_web::HttpResponse;

use anyhow::{Context, Error, Result};
use crate::planner::{PlanBuilder, RequestData, Response, ServerError};
use crate::schema::ComposedSchema;
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry::{global, Context as OpenTelemetryContext};
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, Instant};
use value::ConstValue;
use crate::datasource::RemoteGraphQLDataSource;

use super::executor::Executor;
use super::fetcher::HttpFetcher;
use super::service_route::ServiceRouteTable;

enum Command<S: RemoteGraphQLDataSource> {
    Change(ServiceRouteTable<S>),
}

struct Inner<S: RemoteGraphQLDataSource> {
    schema: Option<Arc<ComposedSchema>>,
    route_table: Option<Arc<ServiceRouteTable<S>>>,
}

pub struct SharedRouteTable<S: RemoteGraphQLDataSource> {
    inner: Arc<RwLock<Inner<S>>>,
    tx: mpsc::UnboundedSender<Command<S>>,
}

impl<S: RemoteGraphQLDataSource> Clone for SharedRouteTable<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            tx: self.tx.clone(),
        }
    }
}

impl<S: RemoteGraphQLDataSource> Default for SharedRouteTable<S> {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let shared_route_table = Self {
            inner: Arc::new(RwLock::new(Inner {
                schema: None,
                route_table: None,
            })),
            tx,
        };
        tokio::spawn({
            let shared_route_table = shared_route_table.clone();
            async move { shared_route_table.update_loop(rx).await }
        });
        shared_route_table
    }
}

impl<S: RemoteGraphQLDataSource> SharedRouteTable<S> {
    async fn update_loop(self, mut rx: mpsc::UnboundedReceiver<Command<S>>) {
        let mut update_interval = tokio::time::interval_at(
            Instant::now() + Duration::from_secs(3),
            Duration::from_secs(30),
        );

        loop {
            tokio::select! {
                _ = update_interval.tick() => {
                    if let Err(err) = self.update().await {
                        tracing::error!(error = %err, "Failed to update schema.");
                    }
                }
                command = rx.recv() => {
                    if let Some(command) = command {
                        match command {
                            Command::Change(route_table) => {
                                let mut inner = self.inner.write().await;
                                inner.route_table = Some(Arc::new(route_table));
                                inner.schema = None;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn update(&self) -> Result<()> {
        const QUERY_SDL: &str = "{ _service { sdl }}";

        #[derive(Deserialize)]
        struct ResponseQuery {
            #[serde(rename = "_service")]
            service: ResponseService,
        }

        #[derive(Deserialize)]
        struct ResponseService {
            sdl: String,
        }

        let route_table = match self.inner.read().await.route_table.clone() {
            Some(route_table) => route_table,
            None => return Ok(()),
        };

        let resp = futures_util::future::try_join_all(route_table.keys().map(|service| {
            let route_table = route_table.clone();
            async move {
                let resp = route_table
                    .get_schema(service, RequestData::new(QUERY_SDL))
                    .await
                    .map_err(|e| {
                        tracing::error!("{e}");
                        e
                    })
                    .with_context(|| format!("Failed to fetch SDL from '{}'.", service))?;
                let resp: ResponseQuery =
                    value::from_value(resp.data).context("Failed to parse response.")?;
                let document = parser::parse_schema(resp.service.sdl)
                    .with_context(|| format!("Invalid SDL from '{}'.", service))?;
                Ok::<_, Error>((service.to_string(), document))
            }
        }))
            .await?;

        let schema = ComposedSchema::combine(resp)?;
        self.inner.write().await.schema = Some(Arc::new(schema));
        Ok(())
    }

    pub fn set_route_table(&self, route_table: ServiceRouteTable<S>) {
        self.tx.send(Command::Change(route_table)).ok();
    }

    pub async fn get(&self) -> Option<(Arc<ComposedSchema>, Arc<ServiceRouteTable<S>>)> {
        let (composed_schema, route_table) = {
            let inner = self.inner.read().await;
            (inner.schema.clone(), inner.route_table.clone())
        };
        composed_schema.zip(route_table)
    }

    pub async fn query(&self, request: RequestData, ctx: crate::datasource::Context) -> HttpResponse {
        let tracer = global::tracer("graphql");

        let document = match tracer.in_span("parse", |_| parser::parse_query(&request.query)) {
            Ok(document) => document,
            Err(err) => {
                return HttpResponse::BadRequest().body(err.to_string());
            }
        };

        let (composed_schema, route_table) = match self.get().await {
            Some((composed_schema, route_table)) => (composed_schema, route_table),
            _ => {
                let response = Response {
                    data: ConstValue::Null,
                    errors: vec![ServerError::new("Not ready.")],
                    extensions: Default::default(),
                    headers: Default::default(),
                };
                let response = match serde_json::to_string(&response) {
                    Ok(r) => r,
                    Err(e) => return HttpResponse::BadRequest().body(e.to_string())
                };
                return HttpResponse::BadRequest().body(response);
            }
        };

        let mut plan_builder =
            PlanBuilder::new(&composed_schema, document).variables(request.variables);

        if let Some(operation) = request.operation {
            plan_builder = plan_builder.operation_name(operation);
        }

        let plan = match tracer.in_span("plan", |_| plan_builder.plan()) {
            Ok(plan) => plan,
            Err(response) => {
                let response = match serde_json::to_string(&response) {
                    Ok(r) => r,
                    Err(e) => return HttpResponse::BadRequest().body(e.to_string())
                };
                return HttpResponse::BadRequest().body(response);
            }
        };

        let executor = Executor::new(&composed_schema);
        let fetcher = HttpFetcher::new(&*route_table, ctx);
        let resp = opentelemetry::trace::FutureExt::with_context(
            executor.execute_query(&fetcher, &plan),
            OpenTelemetryContext::current_with_span(tracer.span_builder("execute").start(&tracer)),
        )
            .await;
        let response = match serde_json::to_string(&resp) {
            Ok(r) => r,
            Err(e) => return HttpResponse::BadRequest().body(e.to_string())
        };
        HttpResponse::Ok().body(response)
    }
}
