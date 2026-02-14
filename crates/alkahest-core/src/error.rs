use thiserror::Error;

/// Errors that can occur during Alkahest initialization and runtime.
#[derive(Debug, Error)]
pub enum AlkahestError {
    #[error("WebGPU adapter not found: {0}")]
    AdapterNotFound(String),

    #[error("Failed to request GPU device: {0}")]
    DeviceRequestFailed(String),

    #[error("Surface configuration failed: {0}")]
    SurfaceConfigFailed(String),

    #[error("Surface texture error: {0}")]
    SurfaceTextureError(String),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilationFailed(String),

    #[error("Render pipeline creation failed: {0}")]
    RenderPipelineError(String),
}
