use std::collections::HashMap;
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};
use futures_util::TryFutureExt;
use crate::planner::{RequestData, Request, Response};
use http::HeaderMap;
use once_cell::sync::Lazy;
use crate::datasource::{Context, RemoteGraphQLDataSource, GraphqlSourceMiddleware};

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(Default::default);

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

        let mut req = Request { headers: HashMap::new()};

        source.will_send_request(&mut req, ctx).await?;

        let url = source.url_query();
        let headers = HeaderMap::try_from(&req.headers)?;

        let raw_resp = HTTP_CLIENT
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .and_then(|res| async move { res.error_for_status() })
            .await?;

        let headers = raw_resp.headers().iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str().to_string(), value.to_string())))
            .collect();

        let mut resp = raw_resp.json::<Response>().await?;

        resp.headers = headers;

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
        let address= source.address();
        let protocol = source.tls().then(|| "https").unwrap_or("http");
        let path = source.query_path().unwrap_or("");
        let url = format!("{protocol}://{address}/{path}") ;
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
