use actix_web::web::ServiceConfig;
pub use datasource::{RemoteGraphQLDataSource, Context};
pub use graphgate_planner::{RequestData, Request, Response};
use graphgate_handler::{ServiceRouteTable, SharedRouteTable};
use std::collections::HashMap;
use std::iter::FromIterator;
use std::ops::Deref;
use std::sync::Arc;
use actix_web::HttpResponse;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};

pub struct GatewayServer<Source: RemoteGraphQLDataSource> {
    table: SharedRouteTable<Source>,
}

impl<Source: RemoteGraphQLDataSource> GatewayServer<Source> {
    pub fn new(sources: Vec<Source>) -> Self {
        let iter = sources.into_iter().map(|s| (s.address().to_string(), s));
        let sources = HashMap::from_iter(iter);
        let table = ServiceRouteTable::from(sources);
        let shared_route_table = SharedRouteTable::default();
        shared_route_table.set_route_table(table);
        Self {
            table: shared_route_table,
        }
    }
}


pub mod macros {
    #[macro_export]
    macro_rules! configure {
        ( $ configure_method_name: ident, $ t: ident) => {
            #[actix_web::post("/")]
            async fn graphql_request(
                config: actix_web::web::Data<GatewayServer<$t>>,
                request: actix_web::web::Json<graphql_gateway::RequestData>,
                req: actix_web::HttpRequest,
            ) -> actix_web::HttpResponse {
                graphql_gateway::actix::graphql_request(config, request, req).await
            }
        fn $configure_method_name(config: &mut actix_web::web::ServiceConfig) {
            config
                .service(graphql_request)
                .service(graphql_gateway::actix::playground);
            }
        };
    }
}

pub mod actix {
    use std::collections::HashMap;
    use actix_web::HttpResponse;
    use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
    use k8s_openapi::serde_json;
    use opentelemetry::trace::{FutureExt, TraceContextExt, Tracer};
    use datasource::{Context, RemoteGraphQLDataSource};
    use graphgate_handler::constants::{KEY_QUERY, KEY_VARIABLES};
    use graphgate_planner::{Request, RequestData};
    use crate::GatewayServer;

    pub async fn graphql_request<S: RemoteGraphQLDataSource>(
        config: actix_web::web::Data<GatewayServer<S>>,
        request: actix_web::web::Json<RequestData>,
        req: actix_web::HttpRequest,
    ) -> HttpResponse {
        let query = request.into_inner();
        let request = Request {
            headers: HashMap::new(),
            data: query,
        };
        let ctx = Context { request: req };
        let tracer = opentelemetry::global::tracer("graphql");
        let query = opentelemetry::Context::current_with_span(
            tracer
                .span_builder("query")
                .with_attributes(vec![
                    KEY_QUERY.string(request.data.query.clone()),
                    KEY_VARIABLES.string(serde_json::to_string(&request.data.variables).unwrap()),
                ])
                .start(&tracer),
        );
        let response = config.table.query(request, ctx).with_context(query).await;
        HttpResponse::Ok().body(response)
    }
    #[actix_web::get("/")]
    pub async fn playground() -> HttpResponse {
        tracing::info!("playground");
        let html = playground_source(GraphQLPlaygroundConfig::new("/"));
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html)
    }
}



