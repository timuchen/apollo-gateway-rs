use std::collections::HashMap;

use std::ops::{Deref, DerefMut};

use crate::planner::{RequestData, Response};


use crate::datasource::{Context, RemoteGraphQLDataSource, GraphqlSourceMiddleware};
use crate::Request;


///
/// The key is the service name.
#[derive(Default, Clone)]
pub struct ServiceRouteTable<Source: RemoteGraphQLDataSource + GraphqlSourceMiddleware>(HashMap<String, Source>);

impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> From<HashMap<String, S>> for ServiceRouteTable<S> {
    fn from(map: HashMap<String, S>) -> Self {
        Self (map)
    }
}

impl<Source: RemoteGraphQLDataSource + GraphqlSourceMiddleware> PartialEq for ServiceRouteTable<Source> {
    fn eq(&self, other: &Self) -> bool {
        self.0.keys().all(|key| other.contains_key(key))
    }
}

impl<Source: RemoteGraphQLDataSource + GraphqlSourceMiddleware> Deref for ServiceRouteTable<Source> {
    type Target = HashMap<String, Source>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<S: RemoteGraphQLDataSource + GraphqlSourceMiddleware> DerefMut for ServiceRouteTable<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Source: RemoteGraphQLDataSource + GraphqlSourceMiddleware> ServiceRouteTable<Source> {
    /// Call the GraphQL query of the specified service.
    pub async fn query(
        &self,
        service: impl AsRef<str>,
        request: RequestData,
        ctx: &Context
    ) -> anyhow::Result<Response> {
        let service = service.as_ref();
        let source = self.0.get(service).ok_or_else(|| {
            anyhow::anyhow!("Service '{}' is not defined in the routing table.", service)
        })?;

        let mut headers = HashMap::new();

        source.will_send_request(&mut headers, ctx).await?;

        let mut resp = source.fetch(Request { headers, data: request}).await?;

        source.did_receive_response(&mut resp, ctx).await?;

        Ok(resp)
    }
    pub async fn get_schema(
        &self,
        service: impl AsRef<str>,
        request: RequestData,
    ) -> anyhow::Result<Response> {
        let service = service.as_ref();
        let source = self.0.get(service).ok_or_else(|| {
            anyhow::anyhow!("Service '{}' is not defined in the routing table.", service)
        })?;
        let resp = source.fetch(Request {headers:  HashMap::with_capacity(0) , data: request}).await?;
        Ok(resp)
    }
}
