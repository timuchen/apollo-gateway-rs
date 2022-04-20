use std::cell::Cell;
pub use datasource::{RemoteGraphQLDataSource, Context};
pub use graphgate_planner::{RequestData, Request, Response};
use graphgate_handler::{ServiceRouteTable, SharedRouteTable};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Default)]
pub struct GatewayServerBuilder {
    table: HashMap<String, Arc<dyn RemoteGraphQLDataSource>>,
    // Compile time check, because someone can don't use build() and push Data<GatewayServerBuilder> instead of Data<GatewayServer> to state of app
    _marker: PhantomData<Cell<()>>
}

impl GatewayServerBuilder {
    pub fn with_source<S: RemoteGraphQLDataSource>(mut self, s: S) -> Self {
        let name = s.name().to_owned();
        let boxed = Arc::new(s);
        self.table.insert(name, boxed);
        self
    }
    pub fn build(self) -> GatewayServer {
        let table = ServiceRouteTable::from(self.table);
        let shared_route_table = SharedRouteTable::default();
        shared_route_table.set_route_table(table);
        GatewayServer {
            table: shared_route_table,
        }
    }
}

pub struct GatewayServer {
    table: SharedRouteTable<Arc<dyn RemoteGraphQLDataSource>>,
}

impl GatewayServer {
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
    use datasource::Context;
    use graphgate_handler::constants::{KEY_QUERY, KEY_VARIABLES};
    use graphgate_handler::{Protocols, Subscription};
    use graphgate_planner::{RequestData};
    use crate::GatewayServer;

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



