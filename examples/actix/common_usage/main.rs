use actix_web::{HttpServer, App, web::Data, HttpResponse};
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use tracing_actix_web::TracingLogger;
use apollo_gateway_rs::{GatewayServer, actix::{graphql_request, graphql_subscription}};
use crate::common_source::CommonSource;

pub async fn playground() -> HttpResponse {
    let html = playground_source(GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"));
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

fn configure_api(config: &mut actix_web::web::ServiceConfig) {
    config.service(
        actix_web::web::resource("/")
            .route(actix_web::web::post().to(graphql_request))
            .route(
                actix_web::web::get()
                    .guard(actix_web::guard::Header("upgrade", "websocket"))
                    .to(graphql_subscription),
            )
            .route(actix_web::web::get().to(playground)),
    );
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter("INFO")
        .try_init()
        .expect("failed init tracing");
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_tracing();
    let gateway_server = GatewayServer::builder()
        .with_source(CommonSource::new("countries", "countries.trevorblades.com", true))
        .build();
    let gateway_server = Data::new(gateway_server);
    HttpServer::new(move || App::new()
        .app_data(gateway_server.clone())
        .wrap(TracingLogger::default())
        .configure(configure_api)
    )
        .bind("0.0.0.0:3000")?
        .run()
        .await
}

mod common_source {
    use apollo_gateway_rs::RemoteGraphQLDataSource;

    pub struct CommonSource {
        pub name: String,
        pub addr: String,
        pub tls: bool,
    }

    impl CommonSource {
        pub fn new(name: &str, addr: &str, tls: bool) -> Self {
            Self {
                name: name.to_owned(),
                addr: addr.to_owned(),
                tls
            }
        }
    }

    impl RemoteGraphQLDataSource for CommonSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn address(&self) -> &str {
            &self.addr
        }
        fn tls(&self) -> bool {
            self.tls
        }
    }
}