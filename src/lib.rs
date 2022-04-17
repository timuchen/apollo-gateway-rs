pub use datasource::{RemoteGraphQLDataSource, Context};
pub use graphgate_planner::{RequestData, Request, Response};
use graphgate_handler::{ServiceRouteTable, SharedRouteTable};
use std::collections::HashMap;
use std::iter::FromIterator;


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
            ( $configure_method_name: ident, $t: ident) => {
                async fn graphql_request(
                    server: actix_web::web::Data<GatewayServer<$t>>,
                    request: actix_web::web::Json<graphql_gateway::RequestData>,
                    req: actix_web::HttpRequest,
                ) -> actix_web::HttpResponse {
                    graphql_gateway::actix::graphql_request(server, request, req).await
                }
                pub async fn graphql_subscription(
                    server: actix_web::web::Data<GatewayServer<$t>>,
                    req: actix_web::HttpRequest,
                    payload: actix_web::web::Payload,
                ) -> HttpResponse {
                    graphql_gateway::actix::graphql_subscription(server, req, payload).await
                }
            fn $configure_method_name(config: &mut actix_web::web::ServiceConfig) {
                cfg.service(
                    web::resource("/")
                        .route(web::post().to(graphql_request))
                        .route(
                            web::get()
                                .guard(guard::Header("upgrade", "websocket"))
                                .to(graphql_subscription),
                        )
                        .route(web::get().to(graphql_gateway::actix::playground)),
                );
            };
        }
    }
}

pub mod actix {
    use std::str::FromStr;
    use actix_web::HttpResponse;
    use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
    use k8s_openapi::serde_json;
    use opentelemetry::trace::{FutureExt, TraceContextExt, Tracer};
    use warp::http::header::SEC_WEBSOCKET_PROTOCOL;
    use datasource::{Context, RemoteGraphQLDataSource};
    use graphgate_handler::constants::{KEY_QUERY, KEY_VARIABLES};
    use graphgate_handler::{Protocols, Subscription};
    use graphgate_planner::{RequestData};
    use crate::GatewayServer;

    pub async fn graphql_request<S: RemoteGraphQLDataSource>(
        server: actix_web::web::Data<GatewayServer<S>>,
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

    pub async fn graphql_subscription<S: RemoteGraphQLDataSource>(
        server: actix_web::web::Data<GatewayServer<S>>,
        req: actix_web::HttpRequest,
        payload: actix_web::web::Payload,
    ) -> HttpResponse {
        let ctx = Context::new(req.clone());
        let protocols = req.headers().get(SEC_WEBSOCKET_PROTOCOL).unwrap().to_str().ok();
        let protocol = protocols
            .and_then(|protocols| {
                protocols.split(',').find_map(|p| Protocols::from_str(p.trim()).ok())
            })
            .unwrap_or(Protocols::SubscriptionsTransportWS);
        if let Some((composed_schema, route_table)) = server.table.get().await {
            let subscription = Subscription::new(composed_schema, route_table, ctx, protocol);
            return actix_web_actors::ws::start(subscription, &req, payload).unwrap();
        }
        HttpResponse::InternalServerError().finish()
    }

    pub async fn playground() -> HttpResponse {
        let html = playground_source(GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"));
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html)
    }
}



