use anyhow::Result;

use crate::config::EdgePulseConfig;
use crate::plugins::render::dual_edge_layer::{
    DualEdgeLayerRenderer, DualEdgeRenderRequest, EdgeGeometry, EdgeGradientStyle, EdgeVisibility,
};
use crate::plugins::render::gradient::rgb_from_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgePulseRenderState {
    pub show_left: bool,
    pub show_right: bool,
}

pub struct EdgePulseRenderer {
    inner: DualEdgeLayerRenderer,
}

impl Default for EdgePulseRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgePulseRenderer {
    pub fn new() -> Self {
        Self {
            inner: DualEdgeLayerRenderer::new(),
        }
    }

    pub fn render(
        &mut self,
        state: EdgePulseRenderState,
        config: &EdgePulseConfig,
        target_output: Option<&str>,
        output_height: i32,
    ) -> Result<()> {
        let animation_t = if config.animation_enabled {
            0.001 // > 0.0 triggers animation start in native_surface
        } else {
            -1.0 // disables animation
        };
        self.inner.render(DualEdgeRenderRequest {
            visibility: EdgeVisibility {
                left: state.show_left,
                right: state.show_right,
            },
            geometry: EdgeGeometry {
                width: config.width.max(1) as i32,
                height_ratio: config.height_ratio.clamp(0.0, 1.0),
            },
            left_style: EdgeGradientStyle {
                start: rgb_from_hex(&config.left_gradient_start).unwrap(),
                end: rgb_from_hex(&config.left_gradient_end).unwrap(),
                base_alpha: config.alpha.clamp(0.0, 1.0),
            },
            right_style: EdgeGradientStyle {
                start: rgb_from_hex(&config.right_gradient_start).unwrap(),
                end: rgb_from_hex(&config.right_gradient_end).unwrap(),
                base_alpha: config.alpha.clamp(0.0, 1.0),
            },
            target_output: target_output.map(|s| s.to_string()),
            output_height,
            animation_t,
            animation_style: config.animation_style.clone(),
            animation_amplitude: config.animation_amplitude,
        })
    }

    pub fn shutdown(&mut self) {
        self.inner.shutdown();
    }
}
