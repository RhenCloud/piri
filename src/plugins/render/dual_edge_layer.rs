use anyhow::{Context, Result};

use super::gradient::RgbColor;
use super::native_surface::{NativeRenderRequest, NativeSurfaceRenderer};

#[derive(Debug, Clone)]
pub struct EdgeGradientStyle {
    pub start: RgbColor,
    pub end: RgbColor,
    pub base_alpha: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeVisibility {
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeGeometry {
    pub width: i32,
    pub height_ratio: f64,
}

#[derive(Debug, Clone)]
pub struct DualEdgeRenderRequest {
    pub visibility: EdgeVisibility,
    pub geometry: EdgeGeometry,
    pub left_style: EdgeGradientStyle,
    pub right_style: EdgeGradientStyle,
    pub target_output: Option<String>,
    pub output_height: i32,
    // Animation fields
    pub animation_t: f64,
    pub animation_style: String,
    pub animation_amplitude: f64,
}

pub struct DualEdgeLayerRenderer {
    inner: NativeSurfaceRenderer,
    last_request: Option<DualEdgeRenderRequest>,
}

impl Default for DualEdgeLayerRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl DualEdgeLayerRenderer {
    pub fn new() -> Self {
        Self {
            inner: NativeSurfaceRenderer::new(),
            last_request: None,
        }
    }

    pub fn render(&mut self, request: DualEdgeRenderRequest) -> Result<()> {
        let width = request.geometry.width.max(1);
        let height = (request.output_height as f64 * request.geometry.height_ratio.clamp(0.0, 1.0))
            .round() as i32;

        self.last_request = Some(request.clone());

        let native_req = NativeRenderRequest {
            show_left: request.visibility.left,
            show_right: request.visibility.right,
            width: width.max(1),
            height: height.max(1),
            left_start: request.left_style.start,
            left_end: request.left_style.end,
            right_start: request.right_style.start,
            right_end: request.right_style.end,
            base_alpha: request.left_style.base_alpha,
            target_output: request.target_output.clone(),
            animation_t: request.animation_t,
            animation_style: request.animation_style.clone(),
            animation_amplitude: request.animation_amplitude,
        };

        self.inner.render(native_req).context("Failed to send native render command")
    }

    pub fn shutdown(&mut self) {
        if self.last_request.is_some() {
            let native_req = NativeRenderRequest {
                show_left: false,
                show_right: false,
                width: 1,
                height: 1,
                left_start: RgbColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                },
                left_end: RgbColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                },
                right_start: RgbColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                },
                right_end: RgbColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                },
                base_alpha: 0.0,
                target_output: None,
                animation_t: -1.0,
                animation_style: String::new(),
                animation_amplitude: 0.0,
            };
            let _ = self.inner.render(native_req);
        }
        self.inner.shutdown();
        self.last_request = None;
    }
}

impl Drop for DualEdgeLayerRenderer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
