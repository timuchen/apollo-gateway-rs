use std::collections::HashMap;
use std::convert::TryFrom;
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
         request: RequestData,
        ctx: &Context
    ) -> anyhow::Result<Response> {
        let service = service.as_ref();
        let source = self.0.get(service).ok_or_else(|| {
            anyhow::anyhow!("Service '{}' is not defined in the routing table.", service)
        })?;

        let mut request = Request {data: request, headers: HashMap::new()};

        let url = format!("http://{}", source.address()) ;

        source.will_send_request(&mut request, ctx).await?;

        let headers = HeaderMap::try_from(&request.headers)?;

        let raw_resp = HTTP_CLIENT
            .post(&url)
            .headers(headers)
            .json(&request.data)
            .send()
            .and_then(|res| async move { res.error_for_status() })
            .await?;

        let headers = raw_resp.headers().iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str().to_string(), value.to_string())))
            .collect();

        let mut resp = raw_resp.json::<Response>().await?;

        resp.headers = headers;

        source.did_receive_response(&mut resp, &ctx).await?;

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
        let url = format!("http://{}", source.address()) ;
        let raw_resp = HTTP_CLIENT
            .post(&url)
            .json(&request)
            .send()
            .and_then(|res| async move { res.error_for_status() })
            .await;
        let raw_resp = raw_resp?;
        let resp = raw_resp.json::<Response>().await?;
        Ok(resp)
    }
}
