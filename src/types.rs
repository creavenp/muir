// Copyright 2026 Patrick Creaven
// SPDX-License-Identifier: Apache-2.0

//! Core domain types for muir.
//!
//! This module is the vocabulary every other part of the crate speaks —
//! detections produced by models, events emitted by the fusion layer, and
//! the small value types they are built from.
//!
//! - `Detection` is per-model, per-window: the raw output of a single model run.
//! - `Event` is the fusion-layer aggregate: one or more detections, summarised.
//! - Sensitive data (location) is optional at the type level; redaction is a
//!   sink-layer concern, not a schema concern.

// TODO(PR 3+): remove this once types are imported from other modules.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Schema version stamped on every [`Detection`] and [`Event`].
///
/// Increment on breaking changes: field removal, semantic change to an existing
/// field, or a newly required field. Adding an optional field does not require
/// a bump.
pub const SCHEMA_VERSION: u16 = 1;

// ─── Enums ────────────────────────────────────────────────────────────────

/// The sensor modality that produced a detection.
///
/// `#[non_exhaustive]` allows new variants (e.g. `Thermal`, `Lidar`) to be
/// added in minor versions without breaking downstream `match` expressions —
/// consumers are required to include a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Modality {
    Audio,
    Vision,
    Pir,
}

/// How significant a fused [`Event`] is.
///
/// Variants are ordered `Low < Medium < High` by declaration, which is what
/// the derived [`Ord`] implementation uses. The fusion layer sets severity
/// explicitly based on detection confidence, modality, and configured rules —
/// it is never inferred from a default.
///
/// `#[non_exhaustive]` preserves the ability to add variants (e.g. `Critical`)
/// in minor versions without breaking downstream matches.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Severity {
    Low,
    Medium,
    High,
}

// ─── Value types ──────────────────────────────────────────────────────────

/// Geographic coordinate pair.
///
/// Fields are private; use [`LatLon::new`] to construct a validated instance.
/// `Eq` and `Hash` are intentionally omitted because `f64` contains `NaN`,
/// which violates the total-equality contract those traits require.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LatLon {
    lat: f64,
    lon: f64,
}

impl LatLon {
    /// Constructs a validated [`LatLon`].
    ///
    /// # Errors
    ///
    /// Returns an error if `lat` is outside `[-90.0, 90.0]` or `lon` is
    /// outside `[-180.0, 180.0]`.
    pub fn new(lat: f64, lon: f64) -> Result<Self, String> {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(format!("latitude {lat} is out of range [-90.0, 90.0]"));
        }
        if !(-180.0..=180.0).contains(&lon) {
            return Err(format!("longitude {lon} is out of range [-180.0, 180.0]"));
        }
        Ok(Self { lat, lon })
    }

    /// Returns the latitude component.
    pub fn lat(&self) -> f64 {
        self.lat
    }

    /// Returns the longitude component.
    pub fn lon(&self) -> f64 {
        self.lon
    }
}

/// Opaque identifier for a physical sensor (e.g. `"microphone-01"`).
///
/// Serializes transparently as a plain JSON string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SensorId(pub String);

impl std::fmt::Display for SensorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Opaque identifier for an inference model (e.g. `"birdnet-v3.0"`).
///
/// Serializes transparently as a plain JSON string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(pub String);

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Axis-aligned bounding box for a visual detection.
///
/// Coordinates are normalized to `[0.0, 1.0]` relative to frame dimensions,
/// with a top-left origin. `(x1, y1)` is the top-left corner;
/// `(x2, y2)` is the bottom-right corner.
///
/// `Eq` and `Hash` are omitted because `f32` contains `NaN`.
/// `bbox` is `None` for audio detections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

/// Half-open time range `[start, end)`.
///
/// Fields are private; use [`TimeRange::new`] to construct a validated instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeRange {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl TimeRange {
    /// Constructs a validated [`TimeRange`].
    ///
    /// # Errors
    ///
    /// Returns an error if `start >= end`.
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self, String> {
        if start >= end {
            return Err(format!(
                "start time {start} must be strictly before end time {end}"
            ));
        }
        Ok(Self { start, end })
    }

    /// Returns the start of the range (inclusive).
    pub fn start(&self) -> DateTime<Utc> {
        self.start
    }

    /// Returns the end of the range (exclusive).
    pub fn end(&self) -> DateTime<Utc> {
        self.end
    }
}

// ─── Detection ────────────────────────────────────────────────────────────

/// The raw output of a single model run on a single input window.
///
/// `Detection` is modality-agnostic: the `modality` and `model` fields
/// identify the source. Audio detections have `bbox: None`; vision detections
/// carry a populated [`BBox`].
///
/// `schema_version` is stored per-instance so that a newer gateway can
/// correctly interpret detections produced by an older one (e.g. from a
/// persisted event log).
///
/// Construct via [`DetectionBuilder`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    id: Uuid,
    schema_version: u16,
    source: SensorId,
    sensor_location: Option<LatLon>,
    modality: Modality,
    model: ModelId,
    model_version: String,
    /// When the underlying sensor captured the signal.
    timestamp: DateTime<Utc>,
    /// When muir ingested and processed the signal.
    ingested_at: DateTime<Utc>,
    /// Species or object class (e.g. `"Turdus migratorius_American Robin"`).
    label: String,
    /// Model confidence in `[0.0, 1.0]`.
    confidence: f32,
    bbox: Option<BBox>,
    /// Model-specific extras. Downstream code must not rely on its shape.
    raw_meta: serde_json::Value,
}

/// Builder for [`Detection`].
///
/// `id` and `schema_version` are generated automatically by [`DetectionBuilder::build`]
/// and cannot be set by callers.
#[derive(Debug, Default, Clone)]
pub struct DetectionBuilder {
    source: Option<SensorId>,
    sensor_location: Option<LatLon>,
    modality: Option<Modality>,
    model: Option<ModelId>,
    model_version: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    ingested_at: Option<DateTime<Utc>>,
    label: Option<String>,
    confidence: Option<f32>,
    bbox: Option<BBox>,
    raw_meta: Option<serde_json::Value>,
}

impl DetectionBuilder {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source(mut self, source: SensorId) -> Self {
        self.source = Some(source);
        self
    }

    pub fn sensor_location(mut self, sensor_location: LatLon) -> Self {
        self.sensor_location = Some(sensor_location);
        self
    }

    pub fn modality(mut self, modality: Modality) -> Self {
        self.modality = Some(modality);
        self
    }

    pub fn model(mut self, model: ModelId) -> Self {
        self.model = Some(model);
        self
    }

    pub fn model_version(mut self, model_version: String) -> Self {
        self.model_version = Some(model_version);
        self
    }

    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    pub fn ingested_at(mut self, ingested_at: DateTime<Utc>) -> Self {
        self.ingested_at = Some(ingested_at);
        self
    }

    pub fn label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub fn bbox(mut self, bbox: BBox) -> Self {
        self.bbox = Some(bbox);
        self
    }

    pub fn raw_meta(mut self, raw_meta: serde_json::Value) -> Self {
        self.raw_meta = Some(raw_meta);
        self
    }

    /// Builds the [`Detection`], returning an error if any required field is unset.
    pub fn build(self) -> Result<Detection, &'static str> {
        Ok(Detection {
            id: Uuid::new_v4(),
            schema_version: SCHEMA_VERSION,
            source: self.source.ok_or("source is required")?,
            sensor_location: self.sensor_location,
            modality: self.modality.ok_or("modality is required")?,
            model: self.model.ok_or("model is required")?,
            model_version: self.model_version.ok_or("model_version is required")?,
            timestamp: self.timestamp.ok_or("timestamp is required")?,
            ingested_at: self.ingested_at.ok_or("ingested_at is required")?,
            label: self.label.ok_or("label is required")?,
            confidence: self.confidence.ok_or("confidence is required")?,
            bbox: self.bbox,
            raw_meta: self.raw_meta.ok_or("raw_meta is required")?,
        })
    }
}

// ─── Event ────────────────────────────────────────────────────────────────

/// A fused, summarised observation built from one or more [`Detection`]s.
///
/// Events are produced by the fusion layer. `detections` holds the UUIDs of
/// every contributing detection so the full evidence chain is traceable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    id: Uuid,
    schema_version: u16,
    detections: Vec<Uuid>,
    /// Human-readable summary. Populated by the LLM layer in Phase 4;
    /// set to a structured fallback string in earlier phases.
    summary: String,
    severity: Severity,
    window: TimeRange,
    location: Option<LatLon>,
}

impl Event {
    /// Constructs a new [`Event`]. `id` and `schema_version` are generated automatically.
    pub fn new(
        detections: Vec<Uuid>,
        summary: String,
        severity: Severity,
        window: TimeRange,
        location: Option<LatLon>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            schema_version: SCHEMA_VERSION,
            detections,
            summary,
            severity,
            window,
            location,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn detections(&self) -> &[Uuid] {
        &self.detections
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn window(&self) -> &TimeRange {
        &self.window
    }

    pub fn location(&self) -> Option<LatLon> {
        self.location
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn modality_roundtrips_through_json() {
        let original = Modality::Audio;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"audio\"");
        let back: Modality = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn severity_roundtrips_through_json() {
        let original = Severity::Low;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"low\"");
        let back: Severity = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn detection_roundtrips_through_json() {
        let original = DetectionBuilder::new()
            .source(SensorId("microphone-01".to_string()))
            .sensor_location(LatLon::new(37.8716, -122.2727).unwrap())
            .modality(Modality::Audio)
            .model(ModelId("birdnet-v3.0".to_string()))
            .model_version("3.0".to_string())
            .timestamp(Utc::now())
            .ingested_at(Utc::now())
            .label("Turdus migratorius_American Robin".to_string())
            .confidence(0.92)
            .raw_meta(serde_json::Value::Null)
            .build()
            .unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let back: Detection = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn detection_sensor_location_none_serializes_as_null() {
        let detection = DetectionBuilder::new()
            .source(SensorId("microphone-01".to_string()))
            .modality(Modality::Audio)
            .model(ModelId("birdnet-v3.0".to_string()))
            .model_version("3.0".to_string())
            .timestamp(Utc::now())
            .ingested_at(Utc::now())
            .label("Turdus migratorius_American Robin".to_string())
            .confidence(0.92)
            .raw_meta(serde_json::Value::Null)
            .build()
            .unwrap();
        let json = serde_json::to_string(&detection).unwrap();
        assert!(json.contains("\"sensor_location\":null"));
    }
}
