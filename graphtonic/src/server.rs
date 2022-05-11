#![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
use tonic::codegen::*;
use graphql_types::{GraphqlRequest, GraphqlResponse};

///Generated trait containing gRPC methods that should be implemented for use with GraphqlServer.
#[async_trait]
pub trait Graphql: Send + Sync + 'static {
    async fn fetch(
        &self,
        request: tonic::Request<GraphqlRequest>,
    ) -> Result<tonic::Response<GraphqlResponse>, tonic::Status>;
    ///Server streaming response type for the subscribe method.
    type SubscribeStream: futures_core::Stream<
        Item = Result<GraphqlResponse, tonic::Status>,
    >
    + Send
    + 'static;
    async fn subscribe(
        &self,
        request: tonic::Request<tonic::Streaming<GraphqlRequest>>,
    ) -> Result<tonic::Response<Self::SubscribeStream>, tonic::Status>;
}
#[derive(Debug)]
pub struct GraphqlServer<T: Graphql> {
    inner: Inner<T>,
    accept_compression_encodings: (),
    send_compression_encodings: (),
}
struct Inner<T>(Arc<T>);
impl<T: Graphql> GraphqlServer<T> {
    pub fn new(inner: T) -> Self {
        Self::from_arc(Arc::new(inner))
    }
    pub fn from_arc(inner: Arc<T>) -> Self {
        let inner = Inner(inner);
        Self {
            inner,
            accept_compression_encodings: Default::default(),
            send_compression_encodings: Default::default(),
        }
    }
    pub fn with_interceptor<F>(
        inner: T,
        interceptor: F,
    ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
    {
        InterceptedService::new(Self::new(inner), interceptor)
    }
}
impl<T, B> tonic::codegen::Service<http::Request<B>> for GraphqlServer<T>
    where
        T: Graphql,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
{
    type Response = http::Response<tonic::body::BoxBody>;
    type Error = std::convert::Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn poll_ready(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        let inner = self.inner.clone();
        match req.uri().path() {
            "/Graphql/fetch" => {
                #[allow(non_camel_case_types)]
                struct fetchSvc<T: Graphql>(pub Arc<T>);
                impl<T: Graphql> tonic::server::UnaryService<GraphqlRequest>
                for fetchSvc<T> {
                    type Response = GraphqlResponse;
                    type Future = BoxFuture<
                        tonic::Response<Self::Response>,
                        tonic::Status,
                    >;
                    fn call(
                        &mut self,
                        request: tonic::Request<GraphqlRequest>,
                    ) -> Self::Future {
                        let inner = self.0.clone();
                        let fut = async move { (*inner).fetch(request).await };
                        Box::pin(fut)
                    }
                }
                let accept_compression_encodings = self.accept_compression_encodings;
                let send_compression_encodings = self.send_compression_encodings;
                let inner = self.inner.clone();
                let fut = async move {
                    let inner = inner.0;
                    let method = fetchSvc(inner);
                    let codec = tonic::codec::ProstCodec::default();
                    let mut grpc = tonic::server::Grpc::new(codec)
                        .apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                    let res = grpc.unary(method, req).await;
                    Ok(res)
                };
                Box::pin(fut)
            }
            "/Graphql/subscribe" => {
                #[allow(non_camel_case_types)]
                struct subscribeSvc<T: Graphql>(pub Arc<T>);
                impl<
                    T: Graphql,
                > tonic::server::StreamingService<GraphqlRequest>
                for subscribeSvc<T> {
                    type Response = GraphqlResponse;
                    type ResponseStream = T::SubscribeStream;
                    type Future = BoxFuture<
                        tonic::Response<Self::ResponseStream>,
                        tonic::Status,
                    >;
                    fn call(
                        &mut self,
                        request: tonic::Request<
                            tonic::Streaming<GraphqlRequest>,
                        >,
                    ) -> Self::Future {
                        let inner = self.0.clone();
                        let fut = async move { (*inner).subscribe(request).await };
                        Box::pin(fut)
                    }
                }
                let accept_compression_encodings = self.accept_compression_encodings;
                let send_compression_encodings = self.send_compression_encodings;
                let inner = self.inner.clone();
                let fut = async move {
                    let inner = inner.0;
                    let method = subscribeSvc(inner);
                    let codec = tonic::codec::ProstCodec::default();
                    let mut grpc = tonic::server::Grpc::new(codec)
                        .apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                    let res = grpc.streaming(method, req).await;
                    Ok(res)
                };
                Box::pin(fut)
            }
            _ => {
                Box::pin(async move {
                    Ok(
                        http::Response::builder()
                            .status(200)
                            .header("grpc-status", "12")
                            .header("content-type", "application/grpc")
                            .body(empty_body())
                            .unwrap(),
                    )
                })
            }
        }
    }
}
impl<T: Graphql> Clone for GraphqlServer<T> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self {
            inner,
            accept_compression_encodings: self.accept_compression_encodings,
            send_compression_encodings: self.send_compression_encodings,
        }
    }
}
impl<T: Graphql> Clone for Inner<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for Inner<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
impl<T: Graphql> tonic::transport::NamedService for GraphqlServer<T> {
    const NAME: &'static str = "Graphql";
}