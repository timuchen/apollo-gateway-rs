#[forbid(clippy::unwrap_used)]
#[forbid(clippy::panicking_unwrap)]
#[forbid(clippy::unnecessary_unwrap)]
#[forbid(clippy::unwrap_in_result)]
mod datasource;
mod handler;
mod planner;
mod schema;
mod validation;

use std::cell::Cell;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::marker::PhantomData;
use std::sync::Arc;
use serde::Deserialize;
pub use crate::datasource::{RemoteGraphQLDataSource, Context, GraphqlSourceMiddleware, DefaultSource};
use crate::datasource::{Config, GraphqlSource, SimpleSource, Source};
pub use crate::planner::{Response, Request};
use crate::handler::{ServiceRouteTable, SharedRouteTable};

#[derive(Default)]
pub struct GatewayServerBuilder {
    table: HashMap<String, Arc<dyn GraphqlSource>>,
    // Compile time check, because someone can don't use build() and push Data<GatewayServerBuilder> instead of Data<GatewayServer> to state of app
    _marker: PhantomData<Cell<()>>,
}

impl GatewayServerBuilder {
    /// Append sources. Make sure that all sources have unique name
    pub fn with_sources<S: RemoteGraphQLDataSource>(mut self, sources: impl Iterator<Item=S>) -> GatewayServerBuilder {
        let sources = sources
            .map(|source| (source.name().to_string(), Arc::new(SimpleSource { source }) as Arc<dyn GraphqlSource>))
            .collect::<HashMap<String, Arc<dyn GraphqlSource>>>();
        self.table.extend(sources);
        self
    }
    /// Append sources with middleware extension. Make sure that all sources have unique name
    pub fn with_middleware_sources<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware>(mut self, sources: impl Iterator<Item=S>) -> GatewayServerBuilder {
        let sources = sources
            .map(|source| (source.name().to_string(), Arc::new(Source { source }) as Arc<dyn GraphqlSource>))
            .collect::<HashMap<String, Arc<dyn GraphqlSource>>>();
        self.table.extend(sources);
        self
    }
    /// Append source. Make sure that all sources have unique name
    pub fn with_source<S: RemoteGraphQLDataSource>(mut self, source: S) -> GatewayServerBuilder {
        let name = source.name().to_owned();
        let source = Arc::new(SimpleSource { source });
        self.table.insert(name, source);
        self
    }
    /// Append source with middleware extension. Make sure that all sources have unique name
    pub fn with_middleware_source<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware>(mut self, source: S) -> GatewayServerBuilder {
        let name = source.name().to_owned();
        let source: Arc<dyn GraphqlSource> = Arc::new(Source { source });
        self.table.insert(name, source);
        self
    }
    fn from_json<S>(path: &str) -> anyhow::Result<Config<S>> where for<'de> S: Deserialize<'de> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let config = serde_json::from_reader::<_, Config<S>>(reader)?;
        Ok(config)
    }
    /// Append sources from json config for example
    /// ```json
    /// {
    ///     "sources": [
    ///         {
    ///             name: "your-source-name",
    ///             address: "your-source-address",
    ///         }
    ///     ]
    ///
    /// }
    /// ```
    /// Make sure that all sources have unique name
    pub fn with_sources_from_json<S: RemoteGraphQLDataSource>(mut self, path: &str) -> anyhow::Result<GatewayServerBuilder> where for<'de> S: Deserialize<'de> {
        let config = Self::from_json::<S>(path)?;
        let sources = config.simple_sources();
        self.table.extend(sources);
        Ok(self)
    }
    /// Append sources with middleware extension from json config for example
    /// ```json
    /// {
    ///     "sources": [
    ///         {
    ///             name: "your-source-name",
    ///             address: "your-source-address",
    ///         }
    ///     ]
    ///
    /// }
    /// ```
    /// Make sure that all sources have unique name
    pub fn with_middleware_sources_from_json<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware>(mut self, path: &str) -> anyhow::Result<GatewayServerBuilder> where for<'de> S: Deserialize<'de> {
        let config = Self::from_json::<S>(path)?;
        let sources = config.sources();
        self.table.extend(sources);
        Ok(self)
    }

    /// Build a Gateway-Server. After building gateway-server will try to parse a schema from your remote sources.
    pub fn build(self) -> GatewayServer {
        let table = ServiceRouteTable::from(self.table);
        let shared_route_table = SharedRouteTable::default();
        shared_route_table.set_route_table(table);
        GatewayServer {
            table: shared_route_table,
        }
    }
}

/// Gateway-server will parse a schema from your remote sources, fetch request and make subscription. Don't forget to pass it into app_data. See example:
/// ```rust
/// async fn main() -> std::io::Result<()> {
///     use actix_web::{App, HttpServer, web::Data};
///     use apollo_gateway_rs::GatewayServer;
///     let gateway_server = GatewayServer::builder()
///         .with_source(CommonSource::new("countries", "countries.trevorblades.com", true))
///         .build();
///     let gateway_server = Data::new(gateway_server);
///     HttpServer::new(move || App::new()
///         .app_data(gateway_server.clone())
///         .configure(configure_api)
///     )
///         .bind("0.0.0.0:3000")?
///         .run()
///         .await
/// }
/// ```
pub struct GatewayServer {
    table: SharedRouteTable<Arc<dyn GraphqlSource>>,
}

impl GatewayServer {
    /// Create a builder for server
    pub fn builder() -> GatewayServerBuilder {
        GatewayServerBuilder::default()
    }
}

pub mod actix {
    use std::str::FromStr;
    use std::sync::Arc;
    use actix_web::http::header::SEC_WEBSOCKET_PROTOCOL;
    use actix_web::HttpResponse;
    use k8s_openapi::serde_json;
    use opentelemetry::trace::{FutureExt, TraceContextExt, Tracer};
    use crate::{Context, GatewayServer};
    use crate::handler::constants::{KEY_QUERY, KEY_VARIABLES};
    use crate::handler::{Protocols, Subscription};
    use crate::planner::RequestData;

    /// Request handler
    pub async fn graphql_request(
        server: actix_web::web::Data<GatewayServer>,
        request: actix_web::web::Json<RequestData>,
        req: actix_web::HttpRequest,
    ) -> HttpResponse {
        let request = request.into_inner();
        let ctx = Context::new(req);
        let tracer = opentelemetry::global::tracer("graphql");
        let query = opentelemetry::Context::current_with_span(
            tracer
                .span_builder("query")
                .with_attributes(vec![
                    KEY_QUERY.string(request.query.clone()),
                    KEY_VARIABLES.string(serde_json::to_string(&request.variables).unwrap()),
                ])
                .start(&tracer),
        );
        server.table.query(request, ctx).with_context(query).await
    }

    /// Subscription handler
    pub async fn graphql_subscription(
        server: actix_web::web::Data<GatewayServer>,
        req: actix_web::HttpRequest,
        payload: actix_web::web::Payload,
    ) -> HttpResponse {
        let ctx = Arc::new(Context::new(req.clone()));
        let protocols = req.headers().get(SEC_WEBSOCKET_PROTOCOL).and_then(|header| header.to_str().ok());
        let protocol = protocols
            .and_then(|protocols| {
                protocols.split(',').find_map(|p| Protocols::from_str(p.trim()).ok())
            })
            .unwrap_or(Protocols::SubscriptionsTransportWS);
        if let Some((composed_schema, route_table)) = server.table.get().await {
            let protocols = [protocol.sec_websocket_protocol()];
            let subscription = Subscription::new(composed_schema, route_table, ctx, protocol);
            return match actix_web_actors::ws::WsResponseBuilder::new(subscription, &req, payload)
                .protocols(&protocols)
                .start() {
                Ok(r) => r,
                Err(e) => HttpResponse::InternalServerError().body(e.to_string())
            };
        }
        HttpResponse::InternalServerError().finish()
    }
}



