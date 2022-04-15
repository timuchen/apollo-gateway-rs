use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use futures_util::TryFutureExt;
use graphgate_planner::{RequestData, Request, Response};
use http::HeaderMap;
use once_cell::sync::Lazy;
use datasource::{Context, RemoteGraphQLDataSource};

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(Default::default);

///
/// The key is the service name.
#[derive(Default, Clone)]
pub struct ServiceRouteTable<Source: RemoteGraphQLDataSource>(HashMap<String, Source>);

impl<S: RemoteGraphQLDataSource> From<HashMap<String, S>> for ServiceRouteTable<S> {
    fn from(map: HashMap<String, S>) -> Self {
        Self (map)
    }
}

impl<Source: RemoteGraphQLDataSource> PartialEq for ServiceRouteTable<Source> {
    fn eq(&self, other: &Self) -> bool {
        self.0.keys().all(|key| other.contains_key(key))
    }
}

impl<Source: RemoteGraphQLDataSource> Deref for ServiceRouteTable<Source> {
    type Target = HashMap<String, Source>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<S: RemoteGraphQLDataSource> DerefMut for ServiceRouteTable<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Source: RemoteGraphQLDataSource> ServiceRouteTable<Source> {
    /// Call the GraphQL query of the specified service.
    pub async fn query(
        &self,
        service: impl AsRef<str>,
        mut request: Request,
        ctx: &Context
    ) -> anyhow::Result<Response> {
        let service = service.as_ref();
        let source = self.0.get(service).ok_or_else(|| {
            anyhow::anyhow!("Service '{}' is not defined in the routing table.", service)
        })?;

        let url = source.address().to_string();

        source.will_send_request(&mut request, ctx);

        let raw_resp = HTTP_CLIENT
            .post(&url)
            .json(&request)
            .send()
            .and_then(|res| async move { res.error_for_status() })
            .await?;

        let resp = raw_resp.json::<Response>().await?;

        source.did_receive_response(&resp, &ctx);

        Ok(resp)
    }
    pub async fn get_schema(
        &self,
        service: impl AsRef<str>,
        request: RequestData,
    ) -> anyhow::Result<Response> {
        let service = service.as_ref();
        let route = self.0.get(service).ok_or_else(|| {
            anyhow::anyhow!("Service '{}' is not defined in the routing table.", service)
        })?;

        let url = route.address().to_string();

        let raw_resp = HTTP_CLIENT
            .post(&url)
            .json(&request)
            .send()
            .and_then(|res| async move { res.error_for_status() })
            .await;
        tracing::info!("{:?}", raw_resp.as_ref().err());
        let raw_resp = raw_resp?;
        let resp = raw_resp.json::<Response>().await?;

        Ok(resp)
    }
}
