use std::convert::{Infallible, TryInto};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use graphgate_planner::RequestData;
use http::header::HeaderName;
use http::HeaderMap;
use opentelemetry::trace::{FutureExt, TraceContextExt, Tracer};
use opentelemetry::{global, Context};
use warp::http::Response as HttpResponse;
use warp::ws::Ws;
use warp::{Filter, Rejection, Reply};

use crate::constants::*;
use crate::metrics::METRICS;
use crate::{websocket, SharedRouteTable};
use std::time::Instant;
use datasource::RemoteGraphQLDataSource;

// pub fn graphql_websocket<S: RemoteGraphQLDataSource>(
//     config: HandlerConfig<S>,
// ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
//     warp::ws()
//         .and(warp::get())
//         .and(warp::header::exact_ignore_case("upgrade", "websocket"))
//         .and(warp::header::optional::<String>("sec-websocket-protocol"))
//         .and(warp::header::headers_cloned())
//         .and(warp::addr::remote())
//         .map({
//             move |ws: Ws, protocols: Option<String>, header_map, remote_addr: Option<SocketAddr>| {
//                 let config = config.clone();
//                 let protocol = protocols
//                     .and_then(|protocols| {
//                         protocols
//                             .split(',')
//                             .find_map(|p| websocket::Protocols::from_str(p.trim()).ok())
//                     })
//                     .unwrap_or(websocket::Protocols::SubscriptionsTransportWS);
//                 let header_map =
//                     do_forward_headers(&config.forward_headers, &header_map, remote_addr);
//
//                 let reply = ws.on_upgrade(move |websocket| async move {
//                     if let Some((composed_schema, route_table)) =
//                         config.shared_route_table.get().await
//                     {
//                         websocket::server(
//                             composed_schema,
//                             route_table,
//                             websocket,
//                             protocol,
//                             header_map,
//                         )
//                         .await;
//                     }
//                 });
//
//                 warp::reply::with_header(
//                     reply,
//                     "Sec-WebSocket-Protocol",
//                     protocol.sec_websocket_protocol(),
//                 )
//             }
//         })
// }
