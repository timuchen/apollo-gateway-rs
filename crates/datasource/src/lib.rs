use std::error::Error;
use std::ops::Deref;
use actix_web::HttpRequest;
use graphgate_planner::{Request, Response};

#[async_trait::async_trait]
pub trait RemoteGraphQLDataSource : Sync + Send + 'static + Clone + Default {
    fn name(&self) -> &str;
    fn address(&self) -> &str;
    type Error: Send + Sync + Error;
    async fn will_send_request(&self, request: &mut Request, ctx: &Context) -> Result<(), Self::Error>;
    async fn did_receive_response(&self, response: &mut Response, ctx: &Context) -> Result<(), Self::Error>;
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

