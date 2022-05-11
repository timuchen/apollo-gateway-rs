use actix_web::{HttpServer, App, web::Data, HttpResponse};
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use tracing_actix_web::TracingLogger;
use apollo_gateway_rs::{GatewayServer, actix::{graphql_request, graphql_subscription}};
use crate::common_source::GrpcSource;

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
    let source = GrpcSource::new("grpc-service", "http://[::1]:50051", true).await.unwrap();
    let gateway_server = GatewayServer::builder()
        .with_middleware_source(source)
        .build();
    let gateway_server = Data::new(gateway_server);
    HttpServer::new(move || App::new()
        .app_data(gateway_server.clone())
        .wrap(TracingLogger::default())
        .configure(configure_api)
    )
        .bind("0.0.0.0:3001")?
        .run()
        .await
}

mod common_source {
    use std::collections::HashMap;
    use apollo_gateway_rs::{Context, GraphqlSourceMiddleware, RemoteGraphQLDataSource, Request, Response};

    pub mod graphql {
        use std::collections::HashMap;
        use apollo_gateway_rs::{Request, Response};
        tonic::include_proto!("_");

        impl From<Request> for GraphqlRequest {
            fn from(r: Request) -> GraphqlRequest {
                let r = r.data;
                GraphqlRequest {
                    query: r.query,
                    operation: None,
                    variables: Default::default(),
                }
            }
        }
        impl Into<Response> for GraphqlResponse {
            fn into(self) -> Response {
                Response {
                    data: serde_json::from_str(&self.data).unwrap() ,
                    errors: vec![],
                    headers: HashMap::with_capacity(0),
                    extensions: HashMap::with_capacity(0)
                }
            }
        }
    }

    use graphql::graphql_client::GraphqlClient;

    pub struct GrpcSource {
        pub name: String,
        pub tls: bool,
        pub client: GraphqlClient<tonic::transport::Channel>
    }

    impl GrpcSource {
        pub async fn new(name: &str, addr: &str, tls: bool) -> anyhow::Result<Self> {
            let client = GraphqlClient::connect(addr.to_string()).await?;
            Ok(Self {
                name: name.to_owned(),
                tls,
                client
            })
        }
    }

    impl RemoteGraphQLDataSource for GrpcSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn address(&self) -> &str { "" }
        fn tls(&self) -> bool {
            self.tls
        }
    }
    #[async_trait::async_trait]
    impl GraphqlSourceMiddleware for GrpcSource {
        async fn fetch(&self, request: Request) -> anyhow::Result<Response> {
            let request = tonic::Request::new(request.into());
            let mut client = self.client.clone();
            let response = client.fetch(request).await?.into_inner().into();
            Ok(response)
        }
    }
}