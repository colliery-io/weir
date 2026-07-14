//! Runner provisioning ([[WEIR-T-0103]] / [[WEIR-A-0023]]) — the control plane ensures a **per-tenant
//! runner Deployment** on Kubernetes ([[WEIR-I-0018]] pod-per-tenant). The `Actuator` trait +
//! `NoopActuator` are always available; the `KubernetesActuator` + the `kube`/`k8s-openapi` deps are
//! behind the `kubernetes` cargo feature so the default build stays light. Direct Deployments — no CRD.

use async_trait::async_trait;

/// Ensures runner capacity for a tenant. `scale(tenant, 0)` or `remove` = no runner (scale-to-zero).
#[async_trait]
pub trait Actuator: Send + Sync {
    /// Create-or-update the tenant's runner Deployment to `replicas`.
    async fn scale(&self, tenant: &str, replicas: i32) -> Result<(), String>;
    /// Delete the tenant's runner Deployment.
    async fn remove(&self, tenant: &str) -> Result<(), String>;
}

/// Single-node / in-process: the in-process `Fleet` runs the work, so provisioning is a no-op.
pub struct NoopActuator;

#[async_trait]
impl Actuator for NoopActuator {
    async fn scale(&self, _tenant: &str, _replicas: i32) -> Result<(), String> {
        Ok(())
    }
    async fn remove(&self, _tenant: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The runner Deployment name for a tenant (k8s-safe: lowercase alphanumeric + `-`).
pub fn runner_name(tenant: &str) -> String {
    let t: String = tenant
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("weir-runner-{t}")
}

/// Build the per-tenant runner Deployment manifest (JSON) — the single source both the
/// `KubernetesActuator` and the tests use, and what `charts/weir-runner` mirrors ([[WEIR-T-0105]]).
pub fn runner_deployment_json(
    tenant: &str,
    replicas: i32,
    image: &str,
    store_url: &str,
    connectors_dir: &str,
) -> serde_json::Value {
    let name = runner_name(tenant);
    let labels = serde_json::json!({ "app": "weir-runner", "weir/tenant": tenant });
    serde_json::json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": name, "labels": labels },
        "spec": {
            "replicas": replicas,
            "selector": { "matchLabels": labels },
            "template": {
                "metadata": { "labels": labels },
                "spec": {
                    "containers": [{
                        "name": "runner",
                        "image": image,
                        "args": ["runner", "--tenant", tenant],
                        "env": [
                            { "name": "WEIR_DB", "value": store_url },
                            { "name": "WEIR_CONNECTORS_DIR", "value": connectors_dir }
                        ]
                    }]
                }
            }
        }
    })
}

#[cfg(feature = "kubernetes")]
mod k8s {
    use super::*;
    use k8s_openapi::api::apps::v1::Deployment;
    use kube::api::{Api, DeleteParams, Patch, PatchParams};

    /// Provisions per-tenant runner Deployments via the Kubernetes API.
    pub struct KubernetesActuator {
        client: kube::Client,
        namespace: String,
        image: String,
        store_url: String,
        connectors_dir: String,
    }

    impl KubernetesActuator {
        /// Connect using the ambient kubeconfig / in-cluster service account.
        pub async fn new(
            namespace: String,
            image: String,
            store_url: String,
            connectors_dir: String,
        ) -> Result<Self, String> {
            let client = kube::Client::try_default()
                .await
                .map_err(|e| e.to_string())?;
            Ok(Self {
                client,
                namespace,
                image,
                store_url,
                connectors_dir,
            })
        }

        fn deployment(&self, tenant: &str, replicas: i32) -> Result<Deployment, String> {
            serde_json::from_value(runner_deployment_json(
                tenant,
                replicas,
                &self.image,
                &self.store_url,
                &self.connectors_dir,
            ))
            .map_err(|e| e.to_string())
        }
    }

    #[async_trait]
    impl Actuator for KubernetesActuator {
        async fn scale(&self, tenant: &str, replicas: i32) -> Result<(), String> {
            let api: Api<Deployment> = Api::namespaced(self.client.clone(), &self.namespace);
            let dep = self.deployment(tenant, replicas)?;
            // Server-side apply: create-or-update idempotently.
            api.patch(
                &runner_name(tenant),
                &PatchParams::apply("weir").force(),
                &Patch::Apply(dep),
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        async fn remove(&self, tenant: &str) -> Result<(), String> {
            let api: Api<Deployment> = Api::namespaced(self.client.clone(), &self.namespace);
            // Best-effort: a missing Deployment isn't an error.
            let _ = api
                .delete(&runner_name(tenant), &DeleteParams::default())
                .await;
            Ok(())
        }
    }
}

#[cfg(feature = "kubernetes")]
pub use k8s::KubernetesActuator;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_name_is_k8s_safe_and_per_tenant() {
        assert_eq!(runner_name("acme"), "weir-runner-acme");
        assert_eq!(runner_name("Acme_Co"), "weir-runner-acme-co"); // lowercased + sanitized
    }

    #[test]
    fn deployment_manifest_is_per_tenant() {
        let d = runner_deployment_json("acme", 2, "weir:latest", "postgres://s", "/c");
        assert_eq!(d["metadata"]["name"], "weir-runner-acme");
        assert_eq!(d["metadata"]["labels"]["weir/tenant"], "acme");
        assert_eq!(d["spec"]["replicas"], 2);
        assert_eq!(d["spec"]["selector"]["matchLabels"]["weir/tenant"], "acme");
        let c = &d["spec"]["template"]["spec"]["containers"][0];
        assert_eq!(c["image"], "weir:latest");
        assert_eq!(c["args"], serde_json::json!(["runner", "--tenant", "acme"]));
        assert_eq!(c["env"][0]["value"], "postgres://s");
    }

    #[cfg(feature = "kubernetes")]
    #[test]
    fn manifest_deserializes_to_a_k8s_deployment() {
        use k8s_openapi::api::apps::v1::Deployment;
        let d: Deployment = serde_json::from_value(runner_deployment_json(
            "acme",
            3,
            "weir:latest",
            "postgres://s",
            "/c",
        ))
        .expect("valid Deployment");
        assert_eq!(d.metadata.name.as_deref(), Some("weir-runner-acme"));
        assert_eq!(d.spec.unwrap().replicas, Some(3));
    }
}
