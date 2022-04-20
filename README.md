# Apollo-gateway-rs

# Apollo-gateway-rs is [Apollo Federation](https://www.apollographql.com/apollo-federation) implemented in Rust.

## Quick start

Define your remote source and implement RemoteGraphQLDataSource for it

```rust
pub struct CommonSource {
    pub name: String,
    pub addr: String,
    pub tls: bool,
}
#[async_trait::async_trait]
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
```

Build a gateway-server with your sources

```rust
let gateway_server = GatewayServer::builder()
        .with_source(CommonSource::new("countries", "countries.trevorblades.com", true))
        .build();
```

Configure reqwest handlers 

```rust
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use apollo_gateway_rs::{actix::{graphql_request, graphql_subscription}};

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
```
And spawn actix-web server!

```rust
async fn main() -> std::io::Result<()> {
    let gateway_server = GatewayServer::builder()
        .with_source(CommonSource::new("countries", "countries.trevorblades.com", true))
        .build();
    let gateway_server = Data::new(gateway_server);
    HttpServer::new(move || App::new()
        .app_data(gateway_server.clone())
        .configure(configure_api)
    )
        .bind("0.0.0.0:3000")?
        .run()
        .await
}
```

You can see full example in examples/actix/common_usage

## Features

The gateway can modify the details of an incoming request before executing it across your subgraphs. For example, your subgraphs might all use the same authorization token to associate an incoming request with a particular user. The gateway can add that token to each operation it sends to your subgraphs.

```rust
#[async_trait::async_trait]
impl RemoteGraphQLDataSource for AuthSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn address(&self) -> &str {
        &self.addr
    }
    
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        let session = ctx.get_session();
        if let Some(jwt) = response.headers.get("user-id")
            .and_then(|header| header.parse().ok())
            .and_then(|user_id| create_jwt(user_id).ok()) {
            session.insert("auth", jwt);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl RemoteGraphQLDataSource for CommonSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn address(&self) -> &str {
        &self.addr
    }

    async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> anyhow::Result<()> {
        let session = ctx.get_session();
        if let Some(user_id) = session.get("auth")
                .ok()
                .flatten()
                .and_then(|identity| decode_identity(identity).ok())
                .map(|claims| claims.claims.id) {
            request.headers.insert("user-id".to_string(), user_id.to_string());
        }
        Ok(())
    }
}
```

### Subscription support
Apollo-gateway-rs support subscription, use apollo_gateway_rs::actix::graphql_subscription if you want it.

### Backend implementations 
- [x] Actix-web
- [ ] Rocket
- [ ] Warp
- [ ] Axum