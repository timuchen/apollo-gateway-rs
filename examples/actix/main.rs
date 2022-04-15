#[macro_use]
extern crate graphql_gateway;
use std::sync::Arc;
use actix_identity::Identity;
use actix_web::{HttpServer, App, web::Data, HttpRequest, HttpResponse};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use graphql_gateway::{GatewayServer, RemoteGraphQLDataSource, configure};
use crate::auth_source::AuthSource;
use crate::common_source::CommonSource;
use crate::general_source::GeneralSource;

configure!(configurate_api, GeneralSource);

fn init_tracing() {
    tracing_subscriber::registry()
        .with(fmt::layer().compact().with_target(false))
        .with(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new("info"))
                .unwrap(),
        )
        .init();
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let sources = vec![
        GeneralSource::Auth(AuthSource {name: "auth-service".to_string(), addr: "0.0.0.0:8080".to_string() }),
        GeneralSource::Common(CommonSource {name: "user-service".to_string(), addr: "0.0.0.0:8083".to_string() })
    ];
    let gateway_server = GatewayServer::new(sources);
    let gateway_server = Data::new(gateway_server);
    init_tracing();
    HttpServer::new(move || App::new()
        .app_data(gateway_server.clone())
        .wrap(TracingLogger::default())
        .configure(configurate_api)
    )
        .bind("0.0.0.0:40001")?
        .run()
        .await
}

mod auth_source {
    use actix_identity::Identity;
    use actix_web::{HttpRequest, HttpResponse};
    use datasource::{Context, RemoteGraphQLDataSource};
    use graphgate_planner::{Request, Response};

    #[derive(Clone, Default)]
    pub struct AuthSource {
        pub(crate) name: String,
        pub(crate) addr: String
    }

    impl RemoteGraphQLDataSource for AuthSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn address(&self) -> &str {
            &self.name
        }
        fn did_receive_response(&self, response: &Response, ctx: &Context) {
        }
    }
}
mod common_source {
    use std::str::FromStr;
    use actix_identity::Identity;
    use actix_web::{HttpMessage, HttpRequest, HttpResponse};
    use actix_web::http::header::{HeaderName, HeaderValue};
    use datasource::{Context, RemoteGraphQLDataSource};
    use graphgate_planner::{Request, Response};

    #[derive(Clone, Default)]
    pub struct CommonSource {
        pub(crate) name: String,
        pub(crate) addr: String
    }
    impl RemoteGraphQLDataSource for CommonSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn address(&self) -> &str {
            &self.addr
        }
        fn will_send_request(&self, request: &mut Request, ctx: &Context) {
            if let Some(identity) = ctx.extensions().get::<Identity>() {
                if let Some(user_id) = identity.identity() {
                    request.headers.insert("user-id".to_string(), user_id);
                }
            }
        }
    }
}
mod general_source {
    use datasource::{Context, RemoteGraphQLDataSource};
    use graphgate_planner::{Request, Response};
    use crate::{AuthSource, CommonSource};

    #[derive(Clone)]
    pub enum GeneralSource {
        Auth(AuthSource),
        Common(CommonSource),
        None
    }

    impl Default for GeneralSource {
        fn default() -> Self {
            GeneralSource::None
        }
    }

    use GeneralSource::{Common, Auth};

    impl RemoteGraphQLDataSource for GeneralSource {
        fn name(&self) -> &str {
            match &self {
                Common(source) => source.name(),
                Auth(source) => source.name(),
                _ => ""
            }
        }

        fn address(&self) -> &str {
            match self {
                Auth(source) => source.address(),
                Common(source) => source.address(),
                _ => ""
            }
        }

        fn will_send_request(&self, request: &mut Request, ctx: &Context) {
            if let Common(source) = self {
                source.will_send_request(request, ctx);
            }
        }
        fn did_receive_response(&self, response: &Response, ctx: &Context) {
            if let Auth(source) = self {
                source.did_receive_response(response, ctx);
            }
        }
    }
}
