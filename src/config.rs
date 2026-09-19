// Copyright 2026 Patrick Creaven
// SPDX-License-Identifier: Apache-2.0

//! Gateway configuration.

// TODO(PR 11): remove once Config is loaded in main.rs.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::types::{LatLon, SensorId};

/// Gateway configuration, loaded from a JSON file.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub sensor_id: SensorId,
    #[serde(default)]
    pub location: Option<LatLon>,
}

impl Config {
    /// Loads and parses a [`Config`] from `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` can't be read or its contents aren't a
    /// valid `Config`.
    pub fn from_file(path: &Path) -> anyhow::Result<Config> {
        let contents = fs::read_to_string(path)?;
        let config = serde_json::from_str(&contents)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_valid_config() {
        let dir = std::env::temp_dir();
        let path = dir.join("muir_test_config.json");
        fs::write(
            &path,
            r#"{"sensor_id":"mic-01","location":{"lat":37.87,"lon":-122.27}}"#,
        )
        .unwrap();

        let config = Config::from_file(&path).unwrap();

        assert_eq!(config.sensor_id, SensorId("mic-01".to_string()));
        assert_eq!(config.location.unwrap().lat(), 37.87);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn loads_config_with_omitted_location() {
        let dir = std::env::temp_dir();
        let path = dir.join("muir_test_config_no_location.json");
        fs::write(&path, r#"{"sensor_id":"mic-01"}"#).unwrap();

        let config = Config::from_file(&path).unwrap();

        assert_eq!(config.location, None);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn rejects_invalid_location() {
        let dir = std::env::temp_dir();
        let path = dir.join("muir_test_config_invalid.json");
        fs::write(
            &path,
            r#"{"sensor_id":"mic-01","location":{"lat":999.0,"lon":0.0}}"#,
        )
        .unwrap();

        let result = Config::from_file(&path);

        assert!(result.is_err());
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_file_errors() {
        let result = Config::from_file(Path::new("/nonexistent/muir.json"));
        assert!(result.is_err());
    }
}
