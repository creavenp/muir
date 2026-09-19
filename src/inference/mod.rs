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

use crate::types::{Detection, Modality, ModelId, SensorId};

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

/// A [`Detector`] that processes raw audio samples.
///
/// `detect` does not take a sample rate parameter because that is a property
/// of the concrete detector, not of an individual call — see
/// [`sample_rate`](AudioDetector::sample_rate) and
/// [`segment_samples`](AudioDetector::segment_samples).
pub trait AudioDetector: Detector {
    /// Sample rate this detector expects, in Hz (e.g. BirdNET v3.0: 32,000).
    ///
    /// Only meaningful once [`ready_for_data`](Detector::ready_for_data)
    /// returns `Ok` — some detectors determine this from the loaded model
    /// rather than knowing it a priori.
    fn sample_rate(&self) -> u32;

    /// Exact number of samples [`detect`](AudioDetector::detect) expects per
    /// call (e.g. BirdNET v3.0: 160,000 — 5 s at 32 kHz).
    ///
    /// Same readiness precondition as [`sample_rate`](AudioDetector::sample_rate).
    fn segment_samples(&self) -> usize;

    /// Runs inference over `audio` and returns every [`Detection`] found.
    ///
    /// `audio` is raw `f32` PCM samples, already decoded and windowed by the
    /// ingest layer to this detector's [`sample_rate`](AudioDetector::sample_rate)
    /// and [`segment_samples`](AudioDetector::segment_samples).
    fn detect(&self, audio: &[f32]) -> Result<Vec<Detection>, DetectorError>;
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

    #[test]
    fn audio_detector_output_reflects_input_length() {
        use crate::types::DetectionBuilder;
        use chrono::Utc;

        struct MockAudioDetector {
            sensor_id: SensorId,
            model_id: ModelId,
            modality: Modality,
        }

        impl Detector for MockAudioDetector {
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

        impl AudioDetector for MockAudioDetector {
            fn sample_rate(&self) -> u32 {
                32_000
            }

            fn segment_samples(&self) -> usize {
                160_000
            }

            fn detect(&self, audio: &[f32]) -> Result<Vec<Detection>, DetectorError> {
                let detection = DetectionBuilder::new()
                    .source(self.sensor_id.clone())
                    .modality(self.modality)
                    .model(self.model_id.clone())
                    .model_version("mock-1.0".to_string())
                    .timestamp(Utc::now())
                    .ingested_at(Utc::now())
                    .label("mock-detection".to_string())
                    .confidence(0.5)
                    .raw_meta(serde_json::json!({ "sample_count": audio.len() }))
                    .build()
                    .map_err(|reason| DetectorError::Inference {
                        reason: reason.to_string(),
                    })?;
                Ok(vec![detection])
            }
        }

        let detector = MockAudioDetector {
            sensor_id: SensorId("mic-01".to_string()),
            model_id: ModelId("mock-audio-v1".to_string()),
            modality: Modality::Audio,
        };

        let short = vec![0.0_f32; 100];
        let long = vec![0.0_f32; 160_000];

        let short_json = serde_json::to_value(&detector.detect(&short).unwrap()[0]).unwrap();
        let long_json = serde_json::to_value(&detector.detect(&long).unwrap()[0]).unwrap();

        assert_eq!(short_json["raw_meta"]["sample_count"], 100);
        assert_eq!(long_json["raw_meta"]["sample_count"], 160_000);
    }
}
