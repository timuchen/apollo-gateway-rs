use std::fmt::Debug;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use actix_web::{HttpRequest, HttpResponse};
use graphgate_planner::{Request, Response};

pub trait RemoteGraphQLDataSource : Sync + Send + 'static + Clone + Default {
    fn name(&self) -> &str;
    fn address(&self) -> &str;
    type Future: Future + Send;
    fn will_send_request(&self, request: &mut Request, ctx: &Context) -> Self::Future;
    fn did_receive_response(&self, response: &Response, ctx: &Context) -> Self::Future;
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

