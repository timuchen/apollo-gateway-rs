use actix_session::SessionMiddleware;
use actix_session::storage::CookieSessionStore;
use actix_web::{HttpServer, App, web::Data, HttpResponse};
use actix_web::cookie::Key;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use tracing_actix_web::TracingLogger;
use apollo_gateway_rs::{GatewayServer, actix::{graphql_request, graphql_subscription}};
use crate::auth_source::AuthSource;
use crate::user_middleware::{UserMiddlewareFactory};
use crate::user_source::UserSource;

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
        .with_middleware_source(UserSource::new("user-service", "user-service:8080"))
        .with_middleware_source(AuthSource::new("auth-service", "auth-service:8085"))
        .build();
    let gateway_server = Data::new(gateway_server);
    let key = Key::generate();
    HttpServer::new(move || App::new()
        .app_data(gateway_server.clone())
        .wrap(TracingLogger::default())
        .wrap(UserMiddlewareFactory::default())
        .wrap(SessionMiddleware::new(CookieSessionStore::default(), key.clone()))
        .configure(configure_api)
    )
        .bind("0.0.0.0:3000")?
        .run()
        .await
}

mod auth_source {
    use actix_session::SessionExt;
    use apollo_gateway_rs::{Context, GraphqlSourceMiddleware, RemoteGraphQLDataSource, Response};
    use crate::jwt::create_jwt;

    pub struct AuthSource {
        pub(crate) name: String,
        pub(crate) addr: String,
    }

    impl AuthSource {
        pub fn new(name: &str, addr: &str) -> Self {
            Self {
                name: name.to_owned(),
                addr: addr.to_owned(),
            }
        }
    }

    impl RemoteGraphQLDataSource for AuthSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn address(&self) -> &str {
            &self.addr
        }
    }

    #[async_trait::async_trait]
    impl GraphqlSourceMiddleware for AuthSource {
        async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
            let session = ctx.get_session();
            if let Some(jwt) = response.headers.get("user-id")
                .and_then(|header| header.parse().ok())
                .and_then(|user_id| create_jwt(user_id).ok()) {
                let _ = session.insert("auth", jwt);
            }
            Ok(())
        }
    }
}

mod user_middleware {
    use actix_service::{Service, Transform};
    use actix_session::SessionExt;
    use actix_web::dev::{ServiceRequest, ServiceResponse};
    use actix_web::{HttpMessage, Error, HttpRequest};
    use futures::future::{Ready, ready};
    use crate::jwt::decode_identity;

    pub struct UserMiddleware<S> {
        service: S,
    }

    #[derive(Clone, Copy)]
    pub struct UserId(pub i32);

    pub trait UserExt {
        fn user_id(&self) -> Option<UserId>;
    }

    impl UserExt for HttpRequest {
        fn user_id(&self) -> Option<UserId> {
            let ext = self.extensions();
            ext.get::<UserId>().cloned()
        }
    }

    impl<S, B> Service<ServiceRequest> for UserMiddleware<S>
        where
            S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error> + 'static
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type Future = <S as Service<ServiceRequest>>::Future;

        actix_service::forward_ready!(service);

        fn call(&self, req: ServiceRequest) -> Self::Future {
            let session = req.get_session();
            if let Some(id) = session.get("auth")
                .ok()
                .flatten()
                .and_then(|identity| decode_identity(identity).ok())
                .map(|claims| claims.claims.id) {
                req.extensions_mut().insert(UserId(id));
            }
            self.service.call(req)
        }
    }


    #[derive(Default)]
    pub struct UserMiddlewareFactory;

    impl<S, B> Transform<S, ServiceRequest> for UserMiddlewareFactory
        where
            S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error> + 'static
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type Transform = UserMiddleware<S>;
        type InitError = ();
        type Future = Ready<Result<Self::Transform, Self::InitError>>;

        fn new_transform(&self, service: S) -> Self::Future {
            ready(Ok(UserMiddleware {
                service,
            }))
        }
    }
}

mod jwt {
    use jsonwebtoken::{Algorithm, encode, EncodingKey, Header, decode, DecodingKey, TokenData, Validation};
    use serde::{Serialize, Deserialize};

    const JWT_SECRET: &[u8; 6] = b"secret";

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Claims {
        pub id: i32,
        exp: usize,
    }

    pub fn decode_identity(identity: String) -> jsonwebtoken::errors::Result<TokenData<Claims>> {
        decode::<Claims>(&identity,
                         &DecodingKey::from_secret(JWT_SECRET),
                         &Validation::new(Algorithm::HS512))
    }

    pub fn create_jwt(id: i32) -> anyhow::Result<String> {
        let expiration = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(60 * 60 * 12))
            .expect("valid timestamp")
            .timestamp();
        let claims = Claims {
            id,
            exp: expiration as usize,
        };
        let header = Header::new(Algorithm::HS512);
        let result = encode(&header, &claims, &EncodingKey::from_secret(JWT_SECRET))?;
        Ok(result)
    }
}

mod user_source {
    use apollo_gateway_rs::{Context, GraphqlSourceMiddleware, RemoteGraphQLDataSource, Request};
    use crate::user_middleware::{UserExt, UserId};
    pub struct UserSource {
        pub(crate) name: String,
        pub(crate) addr: String,
    }

    impl UserSource {
        pub fn new(name: &str, addr: &str) -> Self {
            Self {
                name: name.to_owned(),
                addr: addr.to_owned(),
            }
        }
    }


    impl RemoteGraphQLDataSource for UserSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn address(&self) -> &str { &self.addr }
    }

    #[async_trait::async_trait]
    impl GraphqlSourceMiddleware for UserSource {
        async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> anyhow::Result<()> {
            if let Some(UserId(user_id)) = ctx.user_id() {
                request.headers.insert("user-id".to_string(), user_id.to_string());
            }
            Ok(())
        }
    }
}