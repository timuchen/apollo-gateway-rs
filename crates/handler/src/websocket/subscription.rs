use std::sync::Arc;
use actix::{Actor, AsyncContext, SpawnHandle, StreamHandler};
use actix::dev::Stream;
use datasource::{Context, RemoteGraphQLDataSource};
use graphgate_schema::ComposedSchema;
use actix_web_actors::ws;
use actix_web_actors::ws::{Message, ProtocolError};
use value::ConstValue;
use graphgate_planner::{Response, ServerError};
use crate::ServiceRouteTable;
use crate::websocket::protocol::{ClientMessage, ConnectionError, ServerMessage};
use crate::websocket::{Protocols, WebSocketController};

pub struct Subscription<S: RemoteGraphQLDataSource> {
    schema: Arc<ComposedSchema>,
    route_table: Arc<ServiceRouteTable<S>>,
    context: Context
}

impl<S: RemoteGraphQLDataSource> Actor for Subscription<S> {
    type Context = ws::WebsocketContext<Subscription<S>>;
}

impl<S: RemoteGraphQLDataSource> StreamHandler<Result<ws::Message, ws::ProtocolError>> for Subscription<S> {
    fn handle(&mut self, item: Result<Message, ProtocolError>, ctx: &mut Self::Context) {
        if let Ok(Message::Binary(bytes)) = item {
            let client_msg = match serde_json::from_slice::<ClientMessage>(&bytes) {
                Ok(client_msg) => client_msg,
                Err(_) => return,
            };
            let mut controller = None;
            match client_msg {
                ClientMessage::ConnectionInit { payload } if controller.is_none() => {
                    controller = Some(WebSocketController::new(self.route_table.clone(), payload));
                    //sink.send(Message::text(serde_json::to_string(&ServerMessage::ConnectionAck).unwrap())).await.ok();
                }
                ClientMessage::ConnectionInit { .. } => {}
                ClientMessage::Start { id, payload } | ClientMessage::Subscribe { id, payload } => {
                    //let controller = controller.get_or_insert_with(|| WebSocketController::new(route_table.clone(), None)).clone();
                    let document = match parser::parse_query(&payload.query) {
                        Ok(document) => document,
                        Err(err) => {
                            let resp = Response {
                                data: ConstValue::Null,
                                errors: vec![ServerError::new(err.to_string())],
                                extensions: Default::default(),
                                headers: Default::default()
                            };
                            let data = ServerMessage::Data { id, payload: resp };
                            //sink.send(Message::text(serde_json::to_string(&data).unwrap())).await.ok();

                            let complete = ServerMessage::Complete { id };
                            //sink.send(Message::text(serde_json::to_string(&complete).unwrap())).await.ok();
                            //continue;
                            return;
                        }
                    };
                    /*
                    let id = Arc::new(id.to_string());
                    let schema = schema.clone();
                    let stream = {
                        let id = id.clone();
                        async_stream::stream! {
                                    let builder = PlanBuilder::new(&schema, document).variables(payload.variables);
                                    let node = match builder.plan() {
                                        Ok(node) => node,
                                        Err(resp) => {
                                            yield resp;
                                            return;
                                        }
                                    };
                                    let executor = Executor::new(&schema);
                                    let mut stream = executor.execute_stream(controller.clone(), &id, &node).await;
                                    while let Some(item) = stream.next().await {
                                        yield item;
                                    }
                                }
                    };
                    streams.insert(id, Box::pin(stream));
                    */
                }
                ClientMessage::Stop { id } => {
                    //let controller = controller.get_or_insert_with(|| WebSocketController::new(route_table.clone(), &header_map, None)).clone();
                    //controller.stop(id).await;
                }
                _ => {}
            }
        }
    }
}