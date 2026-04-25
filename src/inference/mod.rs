// Copyright 2026 Patrick Creaven
// SPDX-License-Identifier: Apache-2.0

//! Inference abstractions for muir.
//!
//! This module defines the contract every inference adapter must satisfy.
//! Adapters live in submodules (e.g. `birdnet`) and implement [`Detector`];
//! the rest of the pipeline only ever speaks to the trait, never to a
//! concrete adapter directly.

// TODO(PR 9+): remove once inference adapters are wired into the pipeline.
#![allow(dead_code)]

use thiserror::Error;

use crate::types::{Modality, ModelId, SensorId};

/// Errors that a [`Detector`] implementation can produce.
#[derive(Debug, Error)]
pub enum DetectorError {
    // Loading errors
    /// The model file does not exist at the expected path.
    #[error("model not found: {path}")]
    ModelNotFound { path: String },

    /// The model file exists but could not be parsed or loaded.
    #[error("model corrupt: {reason}")]
    ModelCorrupt { reason: String },

    /// The model file is a version this adapter does not support.
    #[error("model version mismatch: expected {expected}, got {actual}")]
    ModelVersionMismatch { expected: String, actual: String },

    // Runtime errors
    /// The detector was asked to process data before it finished loading.
    #[error("detector not ready")]
    NotReady,

    /// The input data was malformed or unusable by this detector.
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },

    /// The model ran but failed during execution.
    #[error("inference failed: {reason}")]
    Inference { reason: String },

    // Output errors
    /// The model produced output that could not be interpreted.
    #[error("invalid output: {reason}")]
    InvalidOutput { reason: String },
}

/// The common interface every inference adapter implements.
///
/// `Detector` expresses identity and readiness. The modality-specific
/// processing method (`detect()`) lives on subtraits — [`AudioDetector`]
/// for acoustic models, `VisionDetector` for camera-trap models — so that
/// input types remain strongly typed per modality.
///
/// [`AudioDetector`]: crate::inference::AudioDetector
pub trait Detector {
    /// The sensor this detector is bound to.
    fn sensor_id(&self) -> &SensorId;

    /// The modality this detector processes.
    fn modality(&self) -> &Modality;

    /// The model this detector wraps.
    fn model_id(&self) -> &ModelId;

    /// Returns `Ok(())` if the detector is initialised and ready to process
    /// data, or a [`DetectorError`] describing why it is not.
    fn ready_for_data(&self) -> Result<(), DetectorError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detector_error_displays_correctly() {
        let err = DetectorError::InvalidInput {
            reason: "missing sensor id".to_string(),
        };
        assert_eq!(format!("{err}"), "invalid input: missing sensor id");
    }

    #[test]
    fn detector_trait_is_implementable() {
        struct MockDetector {
            modality: Modality,
            sensor_id: SensorId,
            model_id: ModelId,
        }

        impl Detector for MockDetector {
            fn sensor_id(&self) -> &SensorId {
                &self.sensor_id
            }

            fn modality(&self) -> &Modality {
                &self.modality
            }

            fn model_id(&self) -> &ModelId {
                &self.model_id
            }

            fn ready_for_data(&self) -> Result<(), DetectorError> {
                Ok(())
            }
        }

        let detector = MockDetector {
            modality: Modality::Audio,
            sensor_id: SensorId("mic-01".to_string()),
            model_id: ModelId("birdnet-v3.0".to_string()),
        };

        assert_eq!(detector.modality(), &Modality::Audio);
    }
}
