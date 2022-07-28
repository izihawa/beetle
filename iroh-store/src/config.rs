use anyhow::{bail, Result};
use config::{ConfigError, Map, Source, Value};
use iroh_metrics::config::Config as MetricsConfig;
use iroh_rpc_client::Config as RpcClientConfig;
use iroh_rpc_types::{
    store::{StoreClientAddr, StoreServerAddr},
    Addr,
};
use iroh_util::insert_into_config_map;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// CONFIG_FILE_NAME is the name of the optional config file located in the iroh home directory
pub const CONFIG_FILE_NAME: &str = "store.config.toml";
/// ENV_PREFIX should be used along side the config field name to set a config field using
/// environment variables
/// For example, `IROH_STORE_PATH=/path/to/config` would set the value of the `Config.path` field
pub const ENV_PREFIX: &str = "IROH_STORE";

/// The configuration for the store.
#[derive(PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// The location of the content database.
    pub path: PathBuf,
    pub rpc_client: RpcClientConfig,
    pub metrics: MetricsConfig,
}

impl Config {
    pub fn new_with_rpc(path: PathBuf, client_addr: StoreClientAddr) -> Self {
        Self {
            path,
            rpc_client: RpcClientConfig {
                store_addr: Some(client_addr),
                ..Default::default()
            },
            metrics: MetricsConfig::default(),
        }
    }

    #[cfg(feature = "rpc-grpc")]
    pub fn new_grpc(path: PathBuf) -> Self {
        let addr = "grpc://0.0.0.0:4402";
        Self::new_with_rpc(path, addr.parse().unwrap())
    }

    /// Derive server addr for non memory addrs.
    pub fn server_rpc_addr(&self) -> Result<Option<StoreServerAddr>> {
        self.rpc_client
            .store_addr
            .as_ref()
            .map(|addr| match addr {
                #[cfg(feature = "rpc-grpc")]
                Addr::GrpcHttp2(addr) => Ok(Addr::GrpcHttp2(*addr)),
                #[cfg(feature = "rpc-grpc")]
                Addr::GrpcUds(path) => Ok(Addr::GrpcUds(path.clone())),
                #[cfg(feature = "rpc-mem")]
                Addr::Mem(_) => bail!("can not derive rpc_addr for mem addr"),
            })
            .transpose()
    }
}

impl Source for Config {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new(self.clone())
    }
    fn collect(&self) -> Result<Map<String, Value>, ConfigError> {
        let mut map: Map<String, Value> = Map::new();
        let path = self
            .path
            .to_str()
            .ok_or_else(|| ConfigError::Foreign("No `path` set. Path is required.".into()))?;
        insert_into_config_map(&mut map, "path", path);
        insert_into_config_map(&mut map, "rpc_client", self.rpc_client.collect()?);
        insert_into_config_map(&mut map, "metrics", self.metrics.collect()?);

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::Config as ConfigBuilder;

    #[test]
    fn test_collect() {
        let path = PathBuf::new().join("test");
        let default = Config::new_grpc(path);

        let mut expect: Map<String, Value> = Map::new();
        expect.insert(
            "rpc_client".to_string(),
            Value::new(None, default.rpc_client.collect().unwrap()),
        );
        expect.insert(
            "path".to_string(),
            Value::new(None, default.path.to_str().unwrap()),
        );
        expect.insert(
            "metrics".to_string(),
            Value::new(None, default.metrics.collect().unwrap()),
        );

        let got = default.collect().unwrap();
        for key in got.keys() {
            let left = expect.get(key).unwrap();
            let right = got.get(key).unwrap();
            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_build_config_from_struct() {
        let path = PathBuf::new().join("test");
        let expect = Config::new_grpc(path);
        let got: Config = ConfigBuilder::builder()
            .add_source(expect.clone())
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(expect, got);
    }
}
