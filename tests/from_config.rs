use std::fs::{File, remove_file};
use std::io::Write;
use apollo_gateway_rs::{Context, DefaultSource, GatewayServer, GraphqlSourceMiddleware, RemoteGraphQLDataSource, Request, Response};

fn save_json(path: &str) {
    let json = serde_json::json!({
        "sources": [
            {
                "name": "name",
                "address": "address"
            }
        ]
    }).to_string();
    let mut file = File::create(path).unwrap();
    file.write_all(json.as_bytes()).unwrap();
}

#[test]
fn default_source_from_json() {
    let source = "default_source_from_json.json";
    save_json(source);
    let is_source_loaded = GatewayServer::builder()
        .with_sources_from_json::<DefaultSource>(source);
    remove_file(source).unwrap();
    assert_eq!(true, is_source_loaded.is_ok());
}

#[test]
fn middleware_source_from_json() {
    let source = "middleware_source_from_json.json";
    save_json(source);
    #[derive(serde::Deserialize)]
    struct MiddlewareSource {
        name: String,
        address: String
    }
    impl RemoteGraphQLDataSource for MiddlewareSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn address(&self) -> &str {
            &self.address
        }
    }
    #[async_trait::async_trait]
    impl GraphqlSourceMiddleware for MiddlewareSource {
        async fn will_send_request(&self, _: &mut Request, _: &Context) -> anyhow::Result<()> {
            println!("before request");
            Ok(())
        }
        async fn did_receive_response(&self, _: &mut Response, _: &Context) -> anyhow::Result<()> {
            println!("after request");
            Ok(())
        }
    }
    let is_source_loaded = GatewayServer::builder()
        .with_sources_from_json::<MiddlewareSource>(source);
    remove_file(source).unwrap();
    assert_eq!(true, is_source_loaded.is_ok());
}