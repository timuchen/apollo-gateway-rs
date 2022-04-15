use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;
use actix_web::{HttpRequest, HttpResponse};
use graphgate_planner::{Request, Response};

pub trait RemoteGraphQLDataSource : Sync + Send + 'static + Clone + Default {
    fn name(&self) -> &str;
    fn address(&self) -> &str;
    fn will_send_request(&self, request: &mut Request, ctx: &Context) {}
    fn did_receive_response(&self, response: &Response, ctx: &Context) {}
}


pub struct Context {
    pub request: HttpRequest,
}

impl Deref for Context {
    type Target = HttpRequest;
    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

