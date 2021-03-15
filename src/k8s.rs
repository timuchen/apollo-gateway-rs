use anyhow::{Context, Result};
use graphgate_handler::{ServiceRoute, ServiceRouteTable};
use k8s_openapi::api::core::v1::Service;
use kube::api::{ListParams, ObjectMeta};
use kube::{Api, Client};

const NAMESPACE_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/namespace";
const LABEL_GRAPHQL_SERVICE: &str = "graphgate.org/service";
const ANNOTATIONS_QUERY_PATH: &str = "graphgate.org/queryPath";
const ANNOTATIONS_SUBSCRIBE_PATH: &str = "graphgate.org/subscribePath";

fn get_label_value<'a>(meta: &'a ObjectMeta, name: &str) -> Option<&'a str> {
    meta.labels
        .iter()
        .flatten()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, value)| value.as_str())
}

fn get_annotation_value<'a>(meta: &'a ObjectMeta, name: &str) -> Option<&'a str> {
    meta.annotations
        .iter()
        .flatten()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, value)| value.as_str())
}

pub async fn find_graphql_services() -> Result<ServiceRouteTable> {
    tracing::trace!("Find GraphQL services.");
    let client = Client::try_default()
        .await
        .context("Failed to create kube client.")?;

    let namespace =
        std::fs::read_to_string(NAMESPACE_PATH).unwrap_or_else(|_| "default".to_string());
    tracing::trace!(namespace = %namespace, "Get current namespace.");

    let mut route_table = ServiceRouteTable::default();
    let services_api: Api<Service> = Api::namespaced(client, &namespace);

    tracing::trace!("List all services.");
    let services = services_api
        .list(&ListParams::default().labels(LABEL_GRAPHQL_SERVICE))
        .await
        .context("Failed to call list services api")?;

    for service in &services {
        if let Some((host, service_name)) = service
            .metadata
            .name
            .as_deref()
            .zip(get_label_value(&service.metadata, LABEL_GRAPHQL_SERVICE))
        {
            for service_port in service
                .spec
                .iter()
                .map(|spec| spec.ports.iter())
                .flatten()
                .flatten()
            {
                let query_path = get_annotation_value(&service.metadata, ANNOTATIONS_QUERY_PATH);
                let subscribe_path =
                    get_annotation_value(&service.metadata, ANNOTATIONS_SUBSCRIBE_PATH);
                route_table.insert(
                    service_name.to_string(),
                    ServiceRoute {
                        addr: format!("{}:{}", host, service_port.port),
                        query_path: query_path.map(ToString::to_string),
                        subscribe_path: subscribe_path.map(ToString::to_string),
                    },
                );
            }
        }
    }

    Ok(route_table)
}
