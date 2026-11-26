//! The runtime config of the target `client` (`spec/runtime-config.md`): what a deployment chooses.
//!
//! Fill it from any `serde` source, from a flat map of paths (`from_map`), from environment
//! variables whose names you choose (`from_env`) or from a nested JSON value (`from_value`).
//! Every way ends in `check`, which reports every problem at once.

use std::collections::BTreeMap;

use crate::veld::config::{Kind, Setting, Table, Values};
pub use crate::veld::config::{ConfigError, Problem};

/// The settings of one service.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceSettings {
    /// `services.<service>.url`: an absolute `http` or `https` URL.
    pub url: String,
}

/// `services`: one entry for each service the target calls.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientServices {
    pub catalog: ServiceSettings,
}

/// The runtime config of the target `client`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientRuntime {
    pub services: ClientServices,
}

/// The settings, in path order, and the text of every message.
const SETTINGS: &[Setting] = &[
    Setting { path: "services.catalog.url", kind: Kind::Url, required: true, default: None, missing: "services.catalog.url is missing", wrong: "services.catalog.url must be an http or https URL", env_missing: ("services.catalog.url is missing (environment variable ", " is not set)") },
];

const TABLE: Table = Table {
    settings: SETTINGS,
    error_prefix: "invalid runtime config: ",
    not_a_setting: ("", " is not a setting"),
    not_an_object: ("", " must be an object"),
    root_not_an_object: "runtime config must be an object",
};

impl ClientRuntime {
    /// The value of every setting, by path: defaults filled in.
    pub fn values(&self) -> Values {
        let mut values = Values::new();
        values.insert("services.catalog.url".to_string(), self.services.catalog.url.clone());
        values
    }

    fn from_values(values: &Values) -> Self {
        let get = |path: &str| values.get(path).cloned().unwrap_or_default();
        Self {
            services: ClientServices {
                catalog: ServiceSettings { url: get("services.catalog.url") },
            },
        }
    }

    /// Checks the values of this config and returns the checked copy: URLs without a trailing `/`.
    ///
    /// # Errors
    /// [`ConfigError`], with every problem, in path order.
    pub fn check(&self) -> Result<Self, ConfigError> {
        TABLE.check_map(&self.values()).map(|values| Self::from_values(&values))
    }

    /// Builds the config from a nested JSON value (`{ "services": { "iam": { "url": "..." } } }`) and checks it.
    /// Unlike a plain `serde` deserialization, it reports every problem with the text of the spec.
    ///
    /// # Errors
    /// [`ConfigError`], with every problem, in path order.
    pub fn from_value(value: &serde_json::Value) -> Result<Self, ConfigError> {
        TABLE.check_value(value).map(|values| Self::from_values(&values))
    }

    /// Builds the config from a flat map from path (`services.iam.url`) to text, and checks it.
    ///
    /// # Errors
    /// [`ConfigError`], with every problem, in path order.
    pub fn from_map(map: &BTreeMap<String, String>) -> Result<Self, ConfigError> {
        TABLE.check_map(map).map(|values| Self::from_values(&values))
    }

    /// Builds the config from environment variables: `names` pairs a path with the name of the
    /// variable that holds it. A variable that is not set, or is empty, counts as not set.
    ///
    /// # Errors
    /// [`ConfigError`], with every problem, in path order.
    pub fn from_env(names: &[(&str, &str)]) -> Result<Self, ConfigError> {
        Self::from_env_with(names, &|name| std::env::var(name).ok())
    }

    /// Like [`from_env`](Self::from_env), reading the variables through `get`.
    ///
    /// # Errors
    /// [`ConfigError`], with every problem, in path order.
    pub fn from_env_with(names: &[(&str, &str)], get: &dyn Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        TABLE.check_env(names, get).map(|values| Self::from_values(&values))
    }
}

/// A client for each service the target calls.
#[derive(Debug, Clone)]
pub struct ClientClients<T: crate::veld::client::Transport> {
    pub catalog: crate::catalog::catalog_client::CatalogClient<T>,
}

/// Builds every client of the target from the runtime config, each with its URL, and passes
/// `options` to all of them. `transport` makes the transport of one client.
///
/// # Errors
/// [`ConfigError`] when the runtime config does not pass `check`.
pub fn clients_with<T: crate::veld::client::Transport>(runtime: &ClientRuntime, options: crate::veld::client::ClientOptions, mut transport: impl FnMut() -> T) -> Result<ClientClients<T>, ConfigError> {
    let urls = runtime.check()?.values();
    Ok(ClientClients {
        catalog: crate::catalog::catalog_client::CatalogClient::new(transport(), options.clone().base_url(urls.get("services.catalog.url").cloned().unwrap_or_default())).map_err(|e| client_error("services.catalog.url", &e))?,
    })
}

fn client_error(path: &str, error: &crate::veld::Error) -> ConfigError {
    ConfigError::single("invalid runtime config: ", path, format!("{path}: {error}"))
}

/// Like [`clients_with`], on the default transport of the crate.
///
/// # Errors
/// [`ConfigError`] when the runtime config does not pass `check`.
pub fn clients(runtime: &ClientRuntime, options: crate::veld::client::ClientOptions) -> Result<ClientClients<crate::veld::transport_ureq::UreqTransport>, ConfigError> {
    clients_with(runtime, options, crate::veld::transport_ureq::UreqTransport::new)
}
