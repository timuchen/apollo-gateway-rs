use std::pin::Pin;
use async_graphql::{EmptyMutation, Schema};
use futures::Stream;
use tonic::{transport::Server, Request, Response, Status};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let schema = async_graphql::Schema::build(Query, EmptyMutation, Subscription)
        .enable_federation()
        .enable_subscription_in_federation()
        .finish();
    let greeter = GraphqlService { schema };
    Server::builder()
        .add_service(GraphqlServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}

pub mod graphql {
    use std::collections::HashMap;
    use async_graphql::{Request, Response};
    tonic::include_proto!("_");
    impl Into<Request> for GraphqlRequest {
        fn into(self) -> Request {
            Request {
                query: self.query,
                operation_name: None,
                variables: Default::default(),
                uploads: vec![],
                data: Default::default(),
                extensions: Default::default(),
                disable_introspection: false
            }
        }
    }
    impl From<Response> for GraphqlResponse {
        fn from(r: Response) -> Self {
            Self {
                data: serde_json::to_string(&r.data).unwrap(),
                errors: vec![]
            }
        }
    }
}

use graphql::{GraphqlRequest, GraphqlResponse, graphql_server::{Graphql, GraphqlServer}};
use crate::api::{Query, Subscription};

pub struct GraphqlService {
    schema: Schema<Query, EmptyMutation, Subscription>,
}

#[tonic::async_trait]
impl Graphql for GraphqlService {
    type subscribeStream = Pin<Box<dyn Stream<Item=Result<GraphqlResponse, Status>> + Send>>;
    async fn fetch(
        &self,
        request: Request<GraphqlRequest>,
    ) -> Result<Response<GraphqlResponse>, Status> {
        let req = request.into_inner();
        let response = self.schema.execute(req).await.into();
        Ok(Response::new(response))
    }
    async fn subscribe(
        &self,
        request: Request<GraphqlRequest>,
    ) -> Result<tonic::Response<Self::subscribeStream>, tonic::Status> {
        unimplemented!()
    }
}

mod api {
    use std::time::Duration;
    use async_graphql::{ID, Object, Subscription, SimpleObject};
    use futures::Stream;

    pub struct Query;

    #[derive(SimpleObject)]
    pub struct Entity {id: ID}

    #[Object]
    impl Query {
        #[graphql(entity)]
        async fn todo_service_entity(&self, _id: ID) -> Entity {
            unimplemented!();
        }
        async fn hello_world(&self) -> &str {
            "hello world"
        }
    }

    pub struct Subscription;

    #[Subscription]
    impl Subscription {
        async fn now(&self) -> impl Stream<Item=String> {
            async_stream::stream! {
                loop {
                   tokio::time::sleep(Duration::from_secs(1)).await;
                   yield chrono::Utc::now().time().to_string();
                }
            }
        }
    }
}

