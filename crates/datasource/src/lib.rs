use std::ops::Deref;
use std::sync::Arc;
use actix_web::HttpRequest;
use graphgate_planner::{Request, Response};

#[async_trait::async_trait]
pub trait RemoteGraphQLDataSource : Sync + Send + 'static {
    fn name(&self) -> &str;
    fn address(&self) -> &str;
    fn tls(&self) -> bool {false}
    fn query_path(&self) -> Option<&str> {None}
    fn subscribe_path(&self) -> Option<&str> {None}
    fn url_query(&self) -> String {
        let address= self.address();
        let protocol = self.tls().then(|| "https").unwrap_or("http");
        let path = self.query_path().unwrap_or("");
        format!("{protocol}://{address}/{path}")
    }
    fn url_subscription(&self) -> String {
        let address= self.address();
        let protocol = self.tls().then(|| "wss").unwrap_or("ws");
        let path = self.subscribe_path().unwrap_or("");
        format!("{protocol}://{address}/{path}")
    }
    #[allow(unused_variables)]
    async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> anyhow::Result<()> {
        Ok(())
    }
    #[allow(unused_variables)]
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl RemoteGraphQLDataSource for Arc<dyn RemoteGraphQLDataSource> {
    fn name(&self) -> &str {
        self.deref().name()
    }
    fn address(&self) -> &str {
        self.deref().address()
    }
    fn tls(&self) -> bool {
        self.deref().tls()
    }
    fn query_path(&self) -> Option<&str> {
        self.deref().query_path()
    }
    fn subscribe_path(&self) -> Option<&str> {
        self.deref().subscribe_path()
    }
    async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> anyhow::Result<()> {
        self.deref().will_send_request(request, ctx).await
    }
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> anyhow::Result<()> {
        self.deref().did_receive_response(response, ctx).await
    }
}


pub struct Context {
    request: HttpRequest,
}

impl Context {
    pub fn new(request: HttpRequest) -> Self {
        Self {
            request
        }
    }
}

impl Deref for Context {
    type Target = HttpRequest;
    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

