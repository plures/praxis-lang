//! Build script for the px-napi native addon (napi-rs v3).
//!
//! `napi_build::setup()` wires the N-API symbol resolution so the `cdylib`
//! links against the Node runtime that loads it. Without this the addon would
//! fail to resolve `napi_*` symbols at load time.
fn main() {
    napi_build::setup();
}
