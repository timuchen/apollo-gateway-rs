use std::sync::Arc;
use actix_web::HttpResponse;

use anyhow::{Context, Error, Result};
use crate::planner::{PlanBuilder, RequestData, Response, ServerError};
use crate::schema::ComposedSchema;
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry::{global, Context as OpenTelemetryContext};
use parser::Positioned;
use parser::types::{ExecutableDocument, Selection, SelectionSet};
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, Instant};
use crate::datasource::RemoteGraphQLDataSource;
use crate::GraphqlSourceMiddleware;

use super::executor::Executor;
use super::fetcher::HttpFetcher;
use super::service_route::ServiceRouteTable;

enum Command<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> {
    Change(ServiceRouteTable<S>),
}

struct Inner<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> {
    schema: Option<Arc<ComposedSchema>>,
    route_table: Option<Arc<ServiceRouteTable<S>>>,
}

pub struct SharedRouteTable<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> {
    inner: Arc<RwLock<Inner<S>>>,
    tx: mpsc::UnboundedSender<Command<S>>,
}

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> Clone for SharedRouteTable<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            tx: self.tx.clone(),
        }
    }
}

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> Default for SharedRouteTable<S> {
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

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> SharedRouteTable<S> {
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
                    .with_context(|| format!("Failed to fetch SDL from '{}'.", service))?;
                let resp: ResponseQuery =
                    value::from_value(resp.data.unwrap_or_default()).context("Failed to parse response.")?;
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

    pub async fn query(&self, request: RequestData, ctx: crate::datasource::Context, limit: Option<usize>) -> HttpResponse {
        let tracer = global::tracer("graphql");

        let document = match tracer.in_span("parse", |_| parser::parse_query(&request.query)) {
            Ok(document) => document,
            Err(err) => {
                return HttpResponse::BadRequest().body(err.to_string());
            }
        };
        if let Some(limit) = limit {
            match check_recursive_depth(&document, limit) {
                Ok(_) => {},
                Err(e) => {
                    let response = Response {
                        data: None,
                        errors: vec![e],
                        extensions: Default::default(),
                        headers: Default::default(),
                    };
                    let response = match serde_json::to_string(&response) {
                        Ok(r) => r,
                        Err(e) => return HttpResponse::BadRequest().body(e.to_string())
                    };
                    return HttpResponse::Ok().body(response);
                }
            }
        }


        let (composed_schema, route_table) = match self.get().await {
            Some((composed_schema, route_table)) => (composed_schema, route_table),
            _ => {
                let response = Response {
                    data: None,
                    errors: vec![ServerError::new("Not ready.")],
                    extensions: Default::default(),
                    headers: Default::default(),
                };
                let response = match serde_json::to_string(&response) {
                    Ok(r) => r,
                    Err(e) => return HttpResponse::BadRequest().body(e.to_string())
                };
                return HttpResponse::Ok().body(response);
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


fn check_recursive_depth(doc: &ExecutableDocument, max_depth: usize) -> Result<(), ServerError> {
    fn check_selection_set(
        doc: &ExecutableDocument,
        selection_set: &Positioned<SelectionSet>,
        current_depth: usize,
        max_depth: usize,
    ) -> Result<(), ServerError> {
        if current_depth > max_depth {
            return Err(ServerError::new(format!("The recursion depth of the query cannot be greater than `{}`", max_depth)));
        }
        for selection in &selection_set.node.items {
            match &selection.node {
                Selection::Field(field) => {
                    if !field.node.selection_set.node.items.is_empty() {
                        check_selection_set(
                            doc,
                            &field.node.selection_set,
                            current_depth + 1,
                            max_depth,
                        )?;
                    }
                }
                Selection::FragmentSpread(fragment_spread) => {
                    if let Some(fragment) =
                        doc.fragments.get(&fragment_spread.node.fragment_name.node)
                    {
                        check_selection_set(
                            doc,
                            &fragment.node.selection_set,
                            current_depth + 1,
                            max_depth,
                        )?;
                    }
                }
                Selection::InlineFragment(inline_fragment) => {
                    check_selection_set(
                        doc,
                        &inline_fragment.node.selection_set,
                        current_depth + 1,
                        max_depth,
                    )?;
                }
            }
        }
        Ok(())
    }

    if let Some(operation) = doc.operations.iter().next() {
        if let Some(operation) = operation.0 {
            if operation.as_ref() == "IntrospectionQuery" {
                return Ok(())
            }
        }
    }

    for (_, operation) in doc.operations.iter() {
        check_selection_set(doc, &operation.node.selection_set, 0, max_depth)?;
    }
    Ok(())
}
