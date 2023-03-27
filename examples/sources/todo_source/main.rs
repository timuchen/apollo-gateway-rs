use actix_web::{App, HttpServer, web::Data};
use tokio::sync::Mutex;
use api::api_config;
use async_graphql::SimpleObject;
use crate::api::create_schema;

#[derive(SimpleObject, Clone)]
pub struct Todo {
    title: String,
    description: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let (tx, _) = tokio::sync::broadcast::channel::<Todo>(32);
    let store: Vec<Todo> = Vec::new();
    let schema = Data::new(create_schema()
        .data(tx)
        .data(Mutex::new(store))
        .enable_federation()
        .enable_subscription_in_federation()
        .finish());
    HttpServer::new(move || App::new()
        .app_data(schema.clone())
        .configure(api_config)
    )
        .bind(("0.0.0.0", 8085))?
        .run()
        .await
}

mod api {
    use actix_web::{guard, HttpRequest, HttpResponse, web, web::ServiceConfig};
    use actix_web::web::Data;
    use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
    use async_graphql::{Schema, SchemaBuilder};
    use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
    use crate::api::schema::{Mutation, Query, Subscription};

    async fn playground() -> HttpResponse {
        let html = playground_source(GraphQLPlaygroundConfig::new("/"));
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html)
    }

    async fn subscriptions(
        schema: Data<Schema<Query, Mutation, Subscription>>,
        req: HttpRequest,
        payload: web::Payload,
    ) -> actix_web::Result<HttpResponse> {
        let email = req.headers().get("email")
            .and_then(|h| h.to_str().ok())
            .map(|e| e.to_string());
        let schema = schema.get_ref();
        let mut data = async_graphql::context::Data::default();
        data.insert(email);
        GraphQLSubscription::new(schema.clone())
            .with_data(data).start(&req, payload)
    }

    async fn graphql(
        schema: Data<Schema<Query, Mutation, Subscription>>,
        request: HttpRequest,
        req: GraphQLRequest,
    ) -> GraphQLResponse {
        let email = request.headers().get("email")
            .and_then(|h| h.to_str().ok())
            .map(|e| e.to_string());
        let req = req.into_inner().data(email);
        schema.execute(req).await.into()
    }


    pub fn api_config(cfg: &mut ServiceConfig) {
        cfg.service(
            web::resource("/")
                .route(web::post().to(graphql))
                .route(
                    web::get()
                        .guard(guard::Header("upgrade", "websocket"))
                        .to(subscriptions),
                )
                .route(web::get().to(playground)),
        );
    }

    pub fn create_schema() -> SchemaBuilder<Query, Mutation, Subscription> {
        Schema::build(Query {}, Mutation {}, Subscription {})
    }

    mod schema {
        use async_graphql::{Context, Object, InputObject, ID, Guard};
        use tokio::sync::broadcast::Sender;
        use tokio::sync::Mutex;
        use crate::Todo;

        struct AuthGuard;

        #[async_trait::async_trait]
        impl Guard for AuthGuard {
            async fn check(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
                if ctx.data_unchecked::<Option<String>>().is_none() {
                    return Err(async_graphql::Error::new("You are not authorized"));
                }
                Ok(())
            }
        }

        pub struct Query;

        #[Object(extends)]
        impl Query {
            #[graphql(entity)]
            async fn todo_service_entity(&self, _id: ID) -> Todo {
                unimplemented!();
            }
            #[graphql(guard = "AuthGuard")]
            async fn todos<'a>(&self, ctx: &'a Context<'_>) -> Vec<Todo> {
                let store = ctx.data_unchecked::<Mutex<Vec<Todo>>>();
                let store = &*store.lock().await;
                store.clone()
            }
        }

        #[derive(InputObject)]
        pub struct NewTodo {
            title: String,
            description: String,
        }

        pub struct Mutation;

        #[Object]
        impl Mutation {
            #[graphql(guard = "AuthGuard")]
            async fn add_todo(&self, ctx: &Context<'_>, todo: NewTodo) -> &str {
                let mut store = ctx.data_unchecked::<Mutex<Vec<Todo>>>().lock().await;
                let channel = ctx.data_unchecked::<Sender<Todo>>();
                let NewTodo { description, title } = todo;
                let todo = Todo { description, title };
                let _ = channel.send(todo.clone());
                store.push(todo);
                "success"
            }
        }

        pub struct Subscription {}

        use async_graphql::Subscription;
        use futures::Stream;

        #[Subscription]
        impl Subscription {
            #[graphql(guard = "AuthGuard")]
            async fn new_todos<'a>(&self, ctx: &'a Context<'_>) -> impl Stream<Item=Todo> + 'a {
                let channel = ctx.data_unchecked::<Sender<Todo>>();
                let mut channel = channel.subscribe();
                async_stream::stream! {
                    while let Ok(todo) = channel.recv().await {
                        yield todo
                    }
                }
            }
        }
    }
}