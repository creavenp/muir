# Muir

> Pre-MVP, under active development.

Muir is a Rust-based edge gateway that ingests heterogeneous sensor streams (audio, camera, PIR) 
and routes them to specialist inference models for conservation field deployments — national parks, 
reserves, and anti-poaching zones.

## Quickstart

_Coming soon._

## Model license

Muir uses the BirdNET acoustic classifier developed by the Cornell Lab of Ornithology.
The BirdNET model weights are licensed under CC BY-NC-SA 4.0 and are **not** bundled
with this software. Run `scripts/fetch_birdnet.sh` to download them. Non-commercial use only.
