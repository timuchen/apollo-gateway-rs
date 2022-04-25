use std::collections::HashMap;
use actix_web::{App, HttpServer, web::Data};
use tokio::sync::Mutex;
use api::api_config;
use async_graphql::SimpleObject;
use crate::api::create_schema;

#[derive(SimpleObject, Clone)]
pub struct AuthorizedUser {
    email: String,
    #[graphql(skip)]
    hash: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let store: HashMap<String, AuthorizedUser> = HashMap::new();
    let schema = Data::new(create_schema()
        .data(Mutex::new(store))
        .enable_federation()
        .finish());
    HttpServer::new(move || App::new()
        .app_data(schema.clone())
        .configure(api_config)
    )
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}

mod api {
    use actix_web::{get, HttpResponse, post, web::ServiceConfig};
    use actix_web::web::Data;
    use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
    use async_graphql::{EmptySubscription, Schema, SchemaBuilder};
    use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
    use crate::api::schema::{Mutation, Query};

    #[get("/")]
    async fn playground() -> HttpResponse {
        let html = playground_source(GraphQLPlaygroundConfig::new("/"));
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html)
    }

    #[post("/")]
    async fn graphql(
        schema: Data<Schema<Query, Mutation, EmptySubscription>>,
        req: GraphQLRequest,
    ) -> GraphQLResponse {
        let req = req.into_inner();
        schema.execute(req).await.into()
    }

    pub fn api_config(config: &mut ServiceConfig) {
        config
            .service(graphql)
            .service(playground);
    }

    pub fn create_schema() -> SchemaBuilder<Query, Mutation, EmptySubscription> {
        Schema::build(Query {}, Mutation {}, EmptySubscription::default())
    }

    mod schema {
        use std::collections::HashMap;
        use argonautica::{Hasher, Verifier};
        use async_graphql::{Context, Object, InputObject, ID};
        use tokio::sync::Mutex;
        use crate::AuthorizedUser;

        pub struct Query;

        async fn hash_password(password: &str) -> anyhow::Result<String> {
            use futures::compat::Future01CompatExt;
            let mut hasher = Hasher::default();
            hasher
                .with_password(password)
                .with_secret_key("secret")
                .hash_non_blocking()
                .compat()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))
        }


        async fn verify_password(hash: &str, password: &str) -> anyhow::Result<bool> {
            use futures::compat::Future01CompatExt;
            let mut verifier = Verifier::default();
            verifier
                .with_hash(hash)
                .with_password(password)
                .with_secret_key("secret")
                .verify_non_blocking()
                .compat()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))
        }


        #[Object(extends)]
        impl Query {
            #[graphql(entity)]
            async fn auth_service_entity(&self, _id: ID) -> AuthorizedUser {
                unimplemented!();
            }
            async fn sign_in(&self, ctx: &Context<'_>, credentials: Credentials) -> anyhow::Result<AuthorizedUser> {
                let store = ctx.data_unchecked::<Mutex<HashMap<String, AuthorizedUser>>>().lock().await;
                let user = store.get(&credentials.email)
                    .ok_or_else(|| anyhow::anyhow!("please provide valid login/password or contact support"))?;
                if !verify_password(&user.hash, &credentials.password).await? {
                    let err = anyhow::anyhow!("please provide valid login/password or contact support");
                    return Err(err);
                }
                ctx.insert_http_header("email", &user.email);
                Ok(user.clone())
            }
        }

        #[derive(InputObject)]
        pub struct Credentials {
            pub email: String,
            pub password: String,
        }

        #[derive(InputObject)]
        pub struct NewUser {
            #[graphql(validator(email))]
            pub email: String,
            #[graphql(validator(min_length = 1))]
            pub password: String,
        }

        pub struct Mutation;

        #[Object]
        impl Mutation {
            async fn sign_up(&self, ctx: &Context<'_>, credentials: NewUser) -> anyhow::Result<AuthorizedUser> {
                let mut store = ctx.data_unchecked::<Mutex<HashMap<String, AuthorizedUser>>>().lock().await;
                if store.get(&credentials.email).is_some() {
                    let err = anyhow::anyhow!("Email Address is Already Registered");
                    return Err(err);
                }
                let hash = hash_password(&credentials.password).await?;
                let user = AuthorizedUser {
                    email: credentials.email.clone(),
                    hash,
                };
                store.insert(credentials.email, user.clone());
                ctx.insert_http_header("email", &user.email);
                Ok(user)
            }
        }
    }
}