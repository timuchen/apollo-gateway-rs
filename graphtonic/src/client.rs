#![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]

use tonic::codegen::*;
use graphql_types::{GraphqlRequest, GraphqlResponse};

#[derive(Debug, Clone)]
pub struct GraphqlClient<T> {
    inner: tonic::client::Grpc<T>,
}

impl GraphqlClient<tonic::transport::Channel> {
    /// Attempt to create a new client by connecting to a given endpoint.
    pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
    {
        let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
        Ok(Self::new(conn))
    }
}

impl<T> GraphqlClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data=Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    pub fn new(inner: T) -> Self {
        let inner = tonic::client::Grpc::new(inner);
        Self { inner }
    }
    pub fn with_interceptor<F>(
        inner: T,
        interceptor: F,
    ) -> GraphqlClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response=http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
    {
        GraphqlClient::new(InterceptedService::new(inner, interceptor))
    }
    /// Compress requests with `gzip`.
    ///
    /// This requires the server to support it otherwise it might respond with an
    /// error.
    #[must_use]
    pub fn send_gzip(mut self) -> Self {
        self.inner = self.inner.send_gzip();
        self
    }
    /// Enable decompressing responses with `gzip`.
    #[must_use]
    pub fn accept_gzip(mut self) -> Self {
        self.inner = self.inner.accept_gzip();
        self
    }
    pub async fn fetch(
        &mut self,
        request: impl tonic::IntoRequest<GraphqlRequest>,
    ) -> Result<tonic::Response<GraphqlResponse>, tonic::Status> {
        self.inner
            .ready()
            .await
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
        let codec = tonic::codec::ProstCodec::default();
        let path = http::uri::PathAndQuery::from_static("/Graphql/fetch");
        self.inner.unary(request.into_request(), path, codec).await
    }
    pub async fn subscribe(
        &mut self,
        request: impl tonic::IntoStreamingRequest<Message=GraphqlRequest>,
    ) -> Result<
        tonic::Response<tonic::codec::Streaming<GraphqlResponse>>,
        tonic::Status,
    > {
        self.inner
            .ready()
            .await
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
        let codec = tonic::codec::ProstCodec::default();
        let path = http::uri::PathAndQuery::from_static("/Graphql/subscribe");
        self.inner.streaming(request.into_streaming_request(), path, codec).await
    }
}
