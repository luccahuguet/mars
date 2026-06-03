use crate::context::webgpu::WgpuContext;
use bytemuck::Zeroable;
use std::borrow::Cow;
use std::path::Path;
use std::time::{Duration, Instant};

pub type GhosttyShaderPath = String;
pub const MAX_GHOSTTY_SHADER_EXTRA_CURSORS: usize = crate::grid::MAX_CURSOR_REVERSE_CELLS;

const TEXTURE_BINDING: u32 = 0;
const SAMPLER_BINDING: u32 = 1;
const UNIFORM_BINDING: u32 = 2;
const SHADER_ANIMATION_WINDOW: Duration = Duration::from_millis(650);
const CURSOR_TRAIL_WARP_MAX_CELLS: f32 = 32.0;
const CURSOR_TRAIL_ONE_ROW_WARP_MAX_VERTICAL_CELLS: f32 = 1.001;

const FULLSCREEN_VERTEX: &str = r#"
@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}
"#;

const SHADERTOY_PREFIX: &str = r#"#version 450

#define YAZELIX_TERMINAL_RIO_TRAIL 1

layout(set = 0, binding = 2, std140) uniform Globals {
    vec3  iResolution;
    float iTime;
    float iTimeDelta;
    float iFrameRate;
    int   iFrame;
    vec4  iChannelTime[4];
    vec3  iChannelResolution[4];
    vec4  iMouse;
    vec4  iDate;
    float iSampleRate;
    vec4  iCurrentCursor;
    vec4  iPreviousCursor;
    vec4  iCurrentCursorColor;
    vec4  iPreviousCursorColor;
    int   iCurrentCursorStyle;
    int   iPreviousCursorStyle;
    int   iCursorVisible;
    float iTimeCursorChange;
    float iTimeFocus;
    int   iFocus;
    vec3  iPalette[256];
    vec3  iBackgroundColor;
    vec3  iForegroundColor;
    vec3  iCursorColor;
    vec3  iCursorText;
    vec3  iSelectionForegroundColor;
    vec3  iSelectionBackgroundColor;
    int   iYazelixExtraCursorCount;
    vec4  iYazelixExtraCursors[256];
    vec4  iYazelixExtraCursorColors[256];
    ivec4 iYazelixExtraCursorStyles[256];
    int   iYazelixRioTrailActive;
    int   iYazelixRioTrailAnimating;
    int   iYazelixRioTrailPad0;
    int   iYazelixRioTrailPad1;
    vec4  iYazelixRioTrailDestinationCursor;
    vec4  iYazelixRioTrailAnimatedCursor;
    vec4  iYazelixRioTrailCorners[4];
};

#define CURSORSTYLE_BLOCK        0
#define CURSORSTYLE_BLOCK_HOLLOW 1
#define CURSORSTYLE_BAR          2
#define CURSORSTYLE_UNDERLINE    3
#define CURSORSTYLE_LOCK         4

layout(set = 0, binding = 0) uniform texture2D iChannel0_texture;
layout(set = 0, binding = 1) uniform sampler iChannel0_sampler;
#define iChannel0 sampler2D(iChannel0_texture, iChannel0_sampler)
#define texture2D texture

layout(location = 0) out vec4 _fragColor;
"#;

const SHADERTOY_SUFFIX: &str = r#"
void main() { mainImage(_fragColor, gl_FragCoord.xy); }
"#;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GhosttyCursorStyle {
    #[default]
    Block,
    BlockHollow,
    Bar,
    Underline,
    Lock,
}

impl GhosttyCursorStyle {
    #[inline]
    fn as_uniform_value(self) -> i32 {
        match self {
            Self::Block => 0,
            Self::BlockHollow => 1,
            Self::Bar => 2,
            Self::Underline => 3,
            Self::Lock => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GhosttyShaderCursor {
    /// Cursor rectangle in drawable pixels: x, y, width, height.
    pub rect: [f32; 4],
    pub color: [f32; 4],
    pub style: GhosttyCursorStyle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GhosttyShaderRioTrail {
    /// Terminal cursor destination rectangle in Ghostty cursor-uniform
    /// coordinates: x, bottom_y, width, height.
    pub destination_cursor: [f32; 4],
    /// Bounding rectangle of Rio's animated trail in Ghostty
    /// cursor-uniform coordinates: x, bottom_y, width, height.
    pub animated_cursor: [f32; 4],
    /// Animated Rio trail corners in drawable-pixel coordinates,
    /// ordered top-left, top-right, bottom-right, bottom-left.
    pub corners: [[f32; 4]; 4],
    pub animating: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GhosttyShaderFrameState {
    pub cursor: Option<GhosttyShaderCursor>,
    pub extra_cursors: Vec<GhosttyShaderCursor>,
    pub rio_trail: Option<GhosttyShaderRioTrail>,
    pub cursor_externally_animated: bool,
    pub cursor_visible: bool,
    pub focus: bool,
    pub palette: [[f32; 4]; 256],
    pub background_color: [f32; 4],
    pub foreground_color: [f32; 4],
    pub cursor_color: [f32; 4],
    pub cursor_text: [f32; 4],
    pub selection_background_color: [f32; 4],
    pub selection_foreground_color: [f32; 4],
}

impl Default for GhosttyShaderFrameState {
    fn default() -> Self {
        Self {
            cursor: None,
            extra_cursors: Vec::new(),
            rio_trail: None,
            cursor_externally_animated: false,
            cursor_visible: false,
            focus: false,
            palette: [[0.0; 4]; 256],
            background_color: [0.0, 0.0, 0.0, 1.0],
            foreground_color: [1.0, 1.0, 1.0, 1.0],
            cursor_color: [1.0, 1.0, 1.0, 1.0],
            cursor_text: [0.0, 0.0, 0.0, 1.0],
            selection_background_color: [0.0, 0.0, 0.0, 1.0],
            selection_foreground_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GhosttyShaderUniforms {
    resolution: [f32; 3],
    time: f32,
    time_delta: f32,
    frame_rate: f32,
    frame: i32,
    _pad_frame: i32,
    channel_time: [[f32; 4]; 4],
    channel_resolution: [[f32; 4]; 4],
    mouse: [f32; 4],
    date: [f32; 4],
    sample_rate: f32,
    _pad_sample_rate: [f32; 3],
    current_cursor: [f32; 4],
    previous_cursor: [f32; 4],
    current_cursor_color: [f32; 4],
    previous_cursor_color: [f32; 4],
    current_cursor_style: i32,
    previous_cursor_style: i32,
    cursor_visible: i32,
    cursor_change_time: f32,
    time_focus: f32,
    focus: i32,
    _pad_focus: [i32; 2],
    palette: [[f32; 4]; 256],
    background_color: [f32; 4],
    foreground_color: [f32; 4],
    cursor_color: [f32; 4],
    cursor_text: [f32; 4],
    selection_foreground_color: [f32; 4],
    selection_background_color: [f32; 4],
    yazelix_extra_cursor_count: i32,
    _pad_yazelix_extra_cursor_count: [i32; 3],
    yazelix_extra_cursors: [[f32; 4]; MAX_GHOSTTY_SHADER_EXTRA_CURSORS],
    yazelix_extra_cursor_colors: [[f32; 4]; MAX_GHOSTTY_SHADER_EXTRA_CURSORS],
    yazelix_extra_cursor_styles: [[i32; 4]; MAX_GHOSTTY_SHADER_EXTRA_CURSORS],
    yazelix_rio_trail_active: i32,
    yazelix_rio_trail_animating: i32,
    _pad_yazelix_rio_trail_flags: [i32; 2],
    yazelix_rio_trail_destination_cursor: [f32; 4],
    yazelix_rio_trail_animated_cursor: [f32; 4],
    yazelix_rio_trail_corners: [[f32; 4]; 4],
}

impl Default for GhosttyShaderUniforms {
    fn default() -> Self {
        let mut uniforms = Self::zeroed();
        uniforms.resolution = [1.0, 1.0, 1.0];
        uniforms.channel_resolution[0] = [1.0, 1.0, 1.0, 0.0];
        uniforms.sample_rate = 44_100.0;
        uniforms
    }
}

#[derive(Default)]
pub struct GhosttyShaderBrush {
    pipelines: Vec<GhosttyShaderPipeline>,
    resources: Option<GhosttyShaderResources>,
    uniforms: GhosttyShaderUniforms,
    frame_state: GhosttyShaderFrameState,
    frame_state_initialized: bool,
    first_frame: Option<Instant>,
    last_frame: Option<Instant>,
    animation_until: Option<Instant>,
    focus: bool,
}

impl GhosttyShaderBrush {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    pub fn update_shaders(&mut self, ctx: &WgpuContext, paths: &[GhosttyShaderPath]) {
        self.pipelines.clear();

        if paths.is_empty() {
            return;
        }

        let resources = self
            .resources
            .get_or_insert_with(|| GhosttyShaderResources::new(ctx));

        for path in paths {
            let shader_source = match std::fs::read_to_string(path) {
                Ok(source) => source,
                Err(err) => {
                    tracing::error!("failed to read Ghostty custom shader {path}: {err}");
                    continue;
                }
            };

            match GhosttyShaderPipeline::new(
                &ctx.device,
                &resources.pipeline_layout,
                ctx.format,
                path,
                &build_shadertoy_glsl(&shader_source),
            ) {
                Ok(pipeline) => self.pipelines.push(pipeline),
                Err(err) => {
                    tracing::error!("failed to load Ghostty custom shader {path}: {err}");
                }
            }
        }
    }

    #[inline]
    pub fn update_frame_state(&mut self, state: GhosttyShaderFrameState) {
        if shader_animation_inputs_changed(
            self.frame_state_initialized,
            &self.frame_state,
            &state,
        ) {
            self.animation_until = Some(Instant::now() + SHADER_ANIMATION_WINDOW);
        }
        self.frame_state = state;
        self.frame_state_initialized = true;
    }

    #[inline]
    pub fn needs_redraw(&self) -> bool {
        self.animation_until
            .is_some_and(|deadline| Instant::now() <= deadline)
    }

    pub fn render(
        &mut self,
        ctx: &WgpuContext,
        encoder: &mut wgpu::CommandEncoder,
        src_texture: &wgpu::Texture,
        dst_texture: &wgpu::Texture,
    ) {
        if self.pipelines.is_empty() {
            return;
        }

        let usage_caps = ctx.surface_caps().usages;
        if !usage_caps.contains(wgpu::TextureUsages::COPY_SRC)
            || !usage_caps.contains(wgpu::TextureUsages::COPY_DST)
        {
            tracing::warn!(
                "selected WGPU surface does not support Ghostty custom shaders"
            );
            return;
        }

        self.update_uniforms(ctx.size.width, ctx.size.height);

        let Some(resources) = &self.resources else {
            return;
        };

        ctx.queue.write_buffer(
            &resources.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniforms),
        );

        let input_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Ghostty Shader Source Texture"),
            size: src_texture.size(),
            mip_level_count: src_texture.mip_level_count(),
            sample_count: src_texture.sample_count(),
            dimension: src_texture.dimension(),
            format: src_texture.format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[src_texture.format()],
        });

        encoder.copy_texture_to_texture(
            src_texture.as_image_copy(),
            input_texture.as_image_copy(),
            input_texture.size(),
        );

        let mut intermediates = Vec::new();
        for _ in 0..self.pipelines.len().saturating_sub(1) {
            intermediates.push(ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Ghostty Shader Intermediate Texture"),
                size: dst_texture.size(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: dst_texture.format(),
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[dst_texture.format()],
            }));
        }

        for (idx, pipeline) in self.pipelines.iter().enumerate() {
            let src = if idx == 0 {
                &input_texture
            } else {
                &intermediates[idx - 1]
            };
            let dst = if idx == self.pipelines.len() - 1 {
                dst_texture
            } else {
                &intermediates[idx]
            };
            pipeline.render(ctx, resources, encoder, src, dst);
        }
    }

    fn update_uniforms(&mut self, width: f32, height: f32) {
        let now = Instant::now();
        let first_frame = *self.first_frame.get_or_insert(now);
        let last_frame = self.last_frame.replace(now).unwrap_or(now);
        let time = now.duration_since(first_frame).as_secs_f32();
        let time_delta = now.duration_since(last_frame).as_secs_f32();

        self.uniforms.time = time;
        self.uniforms.time_delta = time_delta;
        self.uniforms.frame_rate = if time_delta > 0.0 {
            1.0 / time_delta
        } else {
            0.0
        };
        self.uniforms.frame = self.uniforms.frame.wrapping_add(1);
        self.uniforms.resolution = [width, height, 1.0];
        self.uniforms.channel_time[0] = [time, 0.0, 0.0, 0.0];
        self.uniforms.channel_resolution[0] = [width, height, 1.0, 0.0];

        self.uniforms.palette = self.frame_state.palette;
        self.uniforms.background_color = self.frame_state.background_color;
        self.uniforms.foreground_color = self.frame_state.foreground_color;
        self.uniforms.cursor_color = self.frame_state.cursor_color;
        self.uniforms.cursor_text = self.frame_state.cursor_text;
        self.uniforms.selection_background_color =
            self.frame_state.selection_background_color;
        self.uniforms.selection_foreground_color =
            self.frame_state.selection_foreground_color;
        self.uniforms.cursor_visible = i32::from(self.frame_state.cursor_visible);
        self.uniforms.yazelix_extra_cursor_count =
            self.frame_state
                .extra_cursors
                .len()
                .min(MAX_GHOSTTY_SHADER_EXTRA_CURSORS) as i32;
        self.uniforms.yazelix_extra_cursors =
            [[0.0; 4]; MAX_GHOSTTY_SHADER_EXTRA_CURSORS];
        self.uniforms.yazelix_extra_cursor_colors =
            [[0.0; 4]; MAX_GHOSTTY_SHADER_EXTRA_CURSORS];
        self.uniforms.yazelix_extra_cursor_styles =
            [[0; 4]; MAX_GHOSTTY_SHADER_EXTRA_CURSORS];
        for (idx, cursor) in self
            .frame_state
            .extra_cursors
            .iter()
            .take(MAX_GHOSTTY_SHADER_EXTRA_CURSORS)
            .enumerate()
        {
            self.uniforms.yazelix_extra_cursors[idx] = cursor.rect;
            self.uniforms.yazelix_extra_cursor_colors[idx] = cursor.color;
            self.uniforms.yazelix_extra_cursor_styles[idx][0] =
                cursor.style.as_uniform_value();
        }

        if let Some(rio_trail) = self.frame_state.rio_trail {
            self.uniforms.yazelix_rio_trail_active = 1;
            self.uniforms.yazelix_rio_trail_animating = i32::from(rio_trail.animating);
            self.uniforms.yazelix_rio_trail_destination_cursor =
                rio_trail.destination_cursor;
            self.uniforms.yazelix_rio_trail_animated_cursor = rio_trail.animated_cursor;
            self.uniforms.yazelix_rio_trail_corners = rio_trail.corners;
        } else {
            self.uniforms.yazelix_rio_trail_active = 0;
            self.uniforms.yazelix_rio_trail_animating = 0;
            self.uniforms.yazelix_rio_trail_destination_cursor = [0.0; 4];
            self.uniforms.yazelix_rio_trail_animated_cursor = [0.0; 4];
            self.uniforms.yazelix_rio_trail_corners = [[0.0; 4]; 4];
        }

        if let Some(cursor) = self.frame_state.cursor {
            if self.frame_state.cursor_externally_animated {
                let cursor_changed = self.uniforms.current_cursor != cursor.rect
                    || self.uniforms.current_cursor_color != cursor.color;
                self.uniforms.previous_cursor = cursor.rect;
                self.uniforms.previous_cursor_color = cursor.color;
                self.uniforms.current_cursor = cursor.rect;
                self.uniforms.current_cursor_color = cursor.color;
                if cursor_changed {
                    self.uniforms.cursor_change_time = time;
                }
            } else {
                let cursor_changed = self.uniforms.current_cursor != cursor.rect
                    || self.uniforms.current_cursor_color != cursor.color;
                if cursor_changed {
                    if cursor_transition_should_snap(
                        self.uniforms.current_cursor,
                        cursor.rect,
                    ) {
                        self.uniforms.previous_cursor = cursor.rect;
                        self.uniforms.previous_cursor_color = cursor.color;
                    } else {
                        self.uniforms.previous_cursor = self.uniforms.current_cursor;
                        self.uniforms.previous_cursor_color =
                            self.uniforms.current_cursor_color;
                    }
                    self.uniforms.current_cursor = cursor.rect;
                    self.uniforms.current_cursor_color = cursor.color;
                    self.uniforms.cursor_change_time = time;
                }
            }

            let style = cursor.style.as_uniform_value();
            if self.uniforms.current_cursor_style != style {
                self.uniforms.previous_cursor_style = self.uniforms.current_cursor_style;
                self.uniforms.current_cursor_style = style;
            }
        }

        self.uniforms.focus = i32::from(self.frame_state.focus);
        if self.focus != self.frame_state.focus {
            self.focus = self.frame_state.focus;
            if self.focus {
                self.uniforms.time_focus = time;
            }
        }
    }
}

fn cursor_transition_should_snap(previous: [f32; 4], current: [f32; 4]) -> bool {
    if previous[2] <= 0.0 || previous[3] <= 0.0 || current[2] <= 0.0 || current[3] <= 0.0
    {
        return true;
    }

    let cell_width = previous[2].max(current[2]).max(1.0);
    let cell_height = previous[3].max(current[3]).max(1.0);
    let jump_x = (current[0] - previous[0]).abs() / cell_width;
    let jump_y = (current[1] - previous[1]).abs() / cell_height;

    if jump_y <= CURSOR_TRAIL_ONE_ROW_WARP_MAX_VERTICAL_CELLS {
        return false;
    }

    jump_x.hypot(jump_y) > CURSOR_TRAIL_WARP_MAX_CELLS
}

fn shader_animation_inputs_changed(
    initialized: bool,
    previous: &GhosttyShaderFrameState,
    current: &GhosttyShaderFrameState,
) -> bool {
    if !initialized {
        return current.cursor.is_some() || !current.extra_cursors.is_empty();
    }

    cursor_pair_changed(
        previous.cursor,
        current.cursor,
        previous.cursor_externally_animated && current.cursor_externally_animated,
    ) || extra_cursor_pair_changed(&previous.extra_cursors, &current.extra_cursors)
}

fn cursor_pair_changed(
    previous: Option<GhosttyShaderCursor>,
    current: Option<GhosttyShaderCursor>,
    externally_animated: bool,
) -> bool {
    match (previous, current) {
        (Some(previous), Some(current)) if externally_animated => {
            previous.color != current.color || previous.style != current.style
        }
        (Some(previous), Some(current)) => previous != current,
        _ => false,
    }
}

fn extra_cursor_pair_changed(
    previous: &[GhosttyShaderCursor],
    current: &[GhosttyShaderCursor],
) -> bool {
    if previous.is_empty() || current.is_empty() {
        return false;
    }

    previous != current
}

fn build_shadertoy_glsl(source: &str) -> String {
    format!("{SHADERTOY_PREFIX}\n\n{source}\n\n{SHADERTOY_SUFFIX}")
}

fn validate_shadertoy_fragment_glsl(source: &str) -> Result<(), String> {
    let mut frontend = wgpu::naga::front::glsl::Frontend::default();
    let options =
        wgpu::naga::front::glsl::Options::from(wgpu::naga::ShaderStage::Fragment);
    let module = frontend
        .parse(&options, source)
        .map_err(|err| format!("GLSL parse error: {err}"))?;

    let validation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut validator = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        validator.validate(&module)
    }))
    .map_err(|_| "GLSL validation panicked".to_string())?;

    validation.map_err(|err| format!("GLSL validation error: {err}"))?;
    Ok(())
}

struct GhosttyShaderResources {
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
}

impl GhosttyShaderResources {
    fn new(ctx: &WgpuContext) -> Self {
        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Ghostty Shader Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: TEXTURE_BINDING,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float {
                                    filterable: true,
                                },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: SAMPLER_BINDING,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(
                                wgpu::SamplerBindingType::Filtering,
                            ),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: UNIFORM_BINDING,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<GhosttyShaderUniforms>() as u64,
                                ),
                            },
                            count: None,
                        },
                    ],
                });

        let pipeline_layout =
            ctx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Ghostty Shader Pipeline Layout"),
                    bind_group_layouts: &[Some(&bind_group_layout)],
                    immediate_size: 0,
                });

        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Ghostty Shader Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Ghostty Shader Uniform Buffer"),
            size: std::mem::size_of::<GhosttyShaderUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            bind_group_layout,
            pipeline_layout,
            sampler,
            uniform_buffer,
        }
    }
}

struct GhosttyShaderPipeline {
    render_pipeline: wgpu::RenderPipeline,
}

impl GhosttyShaderPipeline {
    fn new(
        device: &wgpu::Device,
        pipeline_layout: &wgpu::PipelineLayout,
        format: wgpu::TextureFormat,
        label: &str,
        source: &str,
    ) -> Result<Self, String> {
        validate_shadertoy_fragment_glsl(source)?;

        let vertex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Ghostty Shader Fullscreen Vertex"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(FULLSCREEN_VERTEX)),
        });
        let fragment = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label_from_path(label)),
            source: wgpu::ShaderSource::Glsl {
                shader: Cow::Borrowed(source),
                stage: wgpu::naga::ShaderStage::Fragment,
                defines: &[],
            },
        });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Ghostty Shader Render Pipeline"),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        Ok(Self { render_pipeline })
    }

    fn render(
        &self,
        ctx: &WgpuContext,
        resources: &GhosttyShaderResources,
        encoder: &mut wgpu::CommandEncoder,
        src_texture: &wgpu::Texture,
        dst_texture: &wgpu::Texture,
    ) {
        let src_view = src_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let dst_view = dst_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ghostty Shader Bind Group"),
            layout: &resources.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: TEXTURE_BINDING,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: SAMPLER_BINDING,
                    resource: wgpu::BindingResource::Sampler(&resources.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: UNIFORM_BINDING,
                    resource: resources.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ghostty Shader Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &dst_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}

fn label_from_path(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Ghostty Shader Fragment")
}

// Test lane: default
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghostty_uniform_layout_matches_std140_offsets() {
        // Defends: Ghostty cursor shader files depend on these std140 offsets.
        assert_eq!(std::mem::size_of::<GhosttyShaderUniforms>(), 16912);
        assert_eq!(bytemuck::offset_of!(GhosttyShaderUniforms, resolution), 0);
        assert_eq!(bytemuck::offset_of!(GhosttyShaderUniforms, time), 12);
        assert_eq!(
            bytemuck::offset_of!(GhosttyShaderUniforms, channel_time),
            32
        );
        assert_eq!(
            bytemuck::offset_of!(GhosttyShaderUniforms, current_cursor),
            208
        );
        assert_eq!(
            bytemuck::offset_of!(GhosttyShaderUniforms, current_cursor_style),
            272
        );
        assert_eq!(bytemuck::offset_of!(GhosttyShaderUniforms, palette), 304);
        assert_eq!(
            bytemuck::offset_of!(GhosttyShaderUniforms, background_color),
            4400
        );
        assert_eq!(
            bytemuck::offset_of!(GhosttyShaderUniforms, yazelix_extra_cursor_count),
            4496
        );
        assert_eq!(
            bytemuck::offset_of!(GhosttyShaderUniforms, yazelix_rio_trail_active),
            16800
        );
        assert_eq!(
            bytemuck::offset_of!(
                GhosttyShaderUniforms,
                yazelix_rio_trail_destination_cursor
            ),
            16816
        );
    }

    #[test]
    fn shadertoy_prefix_exposes_ghostty_cursor_names() {
        // Defends: the runtime accepts Ghostty-style cursor shader source.
        let source = build_shadertoy_glsl(
            "void mainImage(out vec4 c, in vec2 p) { c = vec4(1.0); }",
        );
        for required in [
            "iChannel0",
            "iResolution",
            "iCurrentCursor",
            "iPreviousCursor",
            "iCurrentCursorColor",
            "iCursorVisible",
            "iTimeCursorChange",
            "iPalette",
            "CURSORSTYLE_BLOCK",
            "iYazelixExtraCursorCount",
            "iYazelixExtraCursors",
            "iYazelixRioTrailActive",
            "iYazelixRioTrailDestinationCursor",
            "YAZELIX_TERMINAL_RIO_TRAIL",
            "void main()",
        ] {
            assert!(source.contains(required), "missing {required}");
        }
    }

    #[test]
    fn ghostty_probe_shader_validates_as_wgpu_glsl() {
        // Defends: the checked-in Ghostty cursor probe reaches Naga's GLSL frontend.
        let probe =
            include_str!("../../../../conformance/shaders/ghostty_cursor_probe.glsl");
        let source = build_shadertoy_glsl(probe);

        validate_shadertoy_fragment_glsl(&source)
            .expect("Ghostty cursor probe should validate as WGPU GLSL");
    }

    #[test]
    fn rio_trail_extension_macro_selects_yazelix_branch() {
        // Defends: user shader source can guard Rio-specific reads behind the
        // Yazelix Terminal extension macro.
        let source = build_shadertoy_glsl(
            r#"
void mainImage(out vec4 color, in vec2 fragCoord) {
#ifndef YAZELIX_TERMINAL_RIO_TRAIL
    color = vec4(YAZELIX_TERMINAL_RIO_TRAIL_MISSING);
#else
    vec2 cursor = iYazelixRioTrailDestinationCursor.xy / max(iResolution.xy, vec2(1.0));
    color = vec4(cursor, float(iYazelixRioTrailActive), 1.0);
#endif
}
"#,
        );

        validate_shadertoy_fragment_glsl(&source)
            .expect("Rio trail extension macro should expose Yazelix branch");
    }

    #[test]
    fn rio_trail_extension_uniforms_validate_as_user_shader_reads() {
        // Defends: direct reads from the Rio trail extension are part of the
        // custom-shader ABI, not only strings in the generated prefix.
        let source = build_shadertoy_glsl(
            r#"
void mainImage(out vec4 color, in vec2 fragCoord) {
    vec2 pixel = fragCoord / max(iResolution.xy, vec2(1.0));
    float active = float(iYazelixRioTrailActive);
    float animating = float(iYazelixRioTrailAnimating);
    vec2 destination = iYazelixRioTrailDestinationCursor.xy / max(iResolution.xy, vec2(1.0));
    vec2 animated = iYazelixRioTrailAnimatedCursor.xy / max(iResolution.xy, vec2(1.0));
    vec2 corner = iYazelixRioTrailCorners[0].xy / max(iResolution.xy, vec2(1.0));
    color = vec4(pixel.x + destination.x + animated.x, pixel.y + corner.y, active, animating);
}
"#,
        );

        validate_shadertoy_fragment_glsl(&source)
            .expect("Rio trail extension uniforms should validate as user shader reads");
    }

    #[test]
    fn cursor_frame_state_change_requests_redraw_window() {
        // Defends: event-mode custom shader animation gets a paced redraw window
        // without forcing global game mode.
        let mut brush = GhosttyShaderBrush::default();
        let mut state = GhosttyShaderFrameState::default();
        state.cursor = Some(GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        });

        brush.update_frame_state(state);

        assert!(brush.needs_redraw());
    }

    #[test]
    fn cursor_move_requests_redraw_window_after_initial_frame() {
        let mut brush = GhosttyShaderBrush::default();
        let first = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let second = GhosttyShaderCursor {
            rect: [18.0, 20.0, 8.0, 16.0],
            ..first
        };
        let mut state = GhosttyShaderFrameState {
            cursor: Some(first),
            cursor_visible: true,
            ..GhosttyShaderFrameState::default()
        };
        brush.update_frame_state(state.clone());
        brush.animation_until = None;

        state.cursor = Some(second);
        brush.update_frame_state(state);

        assert!(brush.needs_redraw());
    }

    #[test]
    fn externally_animated_cursor_move_does_not_request_redraw_window() {
        let mut brush = GhosttyShaderBrush::default();
        let first = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let second = GhosttyShaderCursor {
            rect: [18.0, 20.0, 8.0, 16.0],
            ..first
        };
        let mut state = GhosttyShaderFrameState {
            cursor: Some(first),
            cursor_visible: true,
            cursor_externally_animated: true,
            ..GhosttyShaderFrameState::default()
        };
        brush.update_frame_state(state.clone());
        brush.animation_until = None;

        state.cursor = Some(second);
        brush.update_frame_state(state);

        assert!(!brush.needs_redraw());
    }

    #[test]
    fn externally_animated_cursor_uniforms_do_not_add_shader_transition() {
        let mut brush = GhosttyShaderBrush::default();
        let first = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let second = GhosttyShaderCursor {
            rect: [18.0, 20.0, 8.0, 16.0],
            ..first
        };
        let mut state = GhosttyShaderFrameState {
            cursor: Some(first),
            cursor_visible: true,
            cursor_externally_animated: true,
            ..GhosttyShaderFrameState::default()
        };
        brush.update_frame_state(state.clone());
        brush.update_uniforms(800.0, 600.0);

        state.cursor = Some(second);
        brush.update_frame_state(state);
        brush.update_uniforms(800.0, 600.0);

        assert_eq!(brush.uniforms.previous_cursor, second.rect);
        assert_eq!(brush.uniforms.current_cursor, second.rect);
    }

    #[test]
    fn rio_trail_uniforms_are_zero_without_rio_trail_state() {
        let mut brush = GhosttyShaderBrush::default();
        let mut state = GhosttyShaderFrameState::default();
        state.cursor = Some(GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        });

        brush.update_frame_state(state);
        brush.update_uniforms(800.0, 600.0);

        assert_eq!(brush.uniforms.yazelix_rio_trail_active, 0);
        assert_eq!(brush.uniforms.yazelix_rio_trail_animating, 0);
        assert_eq!(
            brush.uniforms.yazelix_rio_trail_destination_cursor,
            [0.0; 4]
        );
        assert_eq!(brush.uniforms.yazelix_rio_trail_animated_cursor, [0.0; 4]);
        assert_eq!(brush.uniforms.yazelix_rio_trail_corners, [[0.0; 4]; 4]);
    }

    #[test]
    fn rio_trail_uniforms_expose_externally_animated_cursor_geometry() {
        let mut brush = GhosttyShaderBrush::default();
        let cursor = GhosttyShaderCursor {
            rect: [18.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let rio_trail = GhosttyShaderRioTrail {
            destination_cursor: cursor.rect,
            animated_cursor: [10.0, 20.0, 16.0, 16.0],
            corners: [
                [10.0, 4.0, 0.0, 0.0],
                [26.0, 4.0, 0.0, 0.0],
                [26.0, 20.0, 0.0, 0.0],
                [10.0, 20.0, 0.0, 0.0],
            ],
            animating: true,
        };
        let state = GhosttyShaderFrameState {
            cursor: Some(cursor),
            cursor_visible: true,
            cursor_externally_animated: true,
            rio_trail: Some(rio_trail),
            ..GhosttyShaderFrameState::default()
        };

        brush.update_frame_state(state);
        brush.update_uniforms(800.0, 600.0);

        assert_eq!(brush.uniforms.current_cursor, cursor.rect);
        assert_eq!(brush.uniforms.previous_cursor, cursor.rect);
        assert_eq!(brush.uniforms.yazelix_rio_trail_active, 1);
        assert_eq!(brush.uniforms.yazelix_rio_trail_animating, 1);
        assert_eq!(
            brush.uniforms.yazelix_rio_trail_destination_cursor,
            rio_trail.destination_cursor
        );
        assert_eq!(
            brush.uniforms.yazelix_rio_trail_animated_cursor,
            rio_trail.animated_cursor
        );
        assert_eq!(brush.uniforms.yazelix_rio_trail_corners, rio_trail.corners);
    }

    #[test]
    fn focus_change_does_not_request_redraw_window() {
        let mut brush = GhosttyShaderBrush::default();
        let cursor = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let mut state = GhosttyShaderFrameState {
            cursor: Some(cursor),
            cursor_visible: true,
            ..GhosttyShaderFrameState::default()
        };
        brush.update_frame_state(state.clone());
        brush.animation_until = None;

        state.focus = true;
        brush.update_frame_state(state);

        assert!(!brush.needs_redraw());
    }

    #[test]
    fn cursor_blink_visibility_does_not_request_redraw_window() {
        let mut brush = GhosttyShaderBrush::default();
        let cursor = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let mut state = GhosttyShaderFrameState {
            cursor: Some(cursor),
            cursor_visible: true,
            ..GhosttyShaderFrameState::default()
        };
        brush.update_frame_state(state.clone());
        brush.animation_until = None;

        state.cursor = None;
        state.cursor_visible = false;
        brush.update_frame_state(state.clone());
        assert!(!brush.needs_redraw());

        state.cursor = Some(cursor);
        state.cursor_visible = true;
        brush.update_frame_state(state);
        assert!(!brush.needs_redraw());
    }

    #[test]
    fn first_cursor_frame_seeds_previous_cursor_to_current() {
        // Defends: the shader trail does not start at window origin.
        let mut brush = GhosttyShaderBrush::default();
        let cursor = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let mut state = GhosttyShaderFrameState::default();
        state.cursor = Some(cursor);

        brush.update_frame_state(state);
        brush.update_uniforms(800.0, 600.0);

        assert_eq!(brush.uniforms.previous_cursor, cursor.rect);
        assert_eq!(brush.uniforms.current_cursor, cursor.rect);
    }

    #[test]
    fn nearby_cursor_move_preserves_shader_trail_transition() {
        let mut brush = GhosttyShaderBrush::default();
        let first = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let second = GhosttyShaderCursor {
            rect: [10.0, 36.0, 8.0, 16.0],
            ..first
        };

        let mut state = GhosttyShaderFrameState::default();
        state.cursor = Some(first);
        brush.update_frame_state(state.clone());
        brush.update_uniforms(800.0, 600.0);

        state.cursor = Some(second);
        brush.update_frame_state(state);
        brush.update_uniforms(800.0, 600.0);

        assert_eq!(brush.uniforms.previous_cursor, first.rect);
        assert_eq!(brush.uniforms.current_cursor, second.rect);
    }

    #[test]
    fn one_row_cursor_column_warp_preserves_shader_trail_transition() {
        let mut brush = GhosttyShaderBrush::default();
        let first = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let second = GhosttyShaderCursor {
            rect: [330.0, 36.0, 8.0, 16.0],
            ..first
        };

        let mut state = GhosttyShaderFrameState::default();
        state.cursor = Some(first);
        brush.update_frame_state(state.clone());
        brush.update_uniforms(800.0, 600.0);

        state.cursor = Some(second);
        brush.update_frame_state(state);
        brush.update_uniforms(800.0, 600.0);

        assert_eq!(brush.uniforms.previous_cursor, first.rect);
        assert_eq!(brush.uniforms.current_cursor, second.rect);
    }

    #[test]
    fn large_cursor_jump_snaps_shader_previous_cursor() {
        let mut brush = GhosttyShaderBrush::default();
        let first = GhosttyShaderCursor {
            rect: [10.0, 20.0, 8.0, 16.0],
            color: [1.0, 0.0, 1.0, 1.0],
            style: GhosttyCursorStyle::Block,
        };
        let second = GhosttyShaderCursor {
            rect: [260.0, 420.0, 8.0, 16.0],
            ..first
        };

        let mut state = GhosttyShaderFrameState::default();
        state.cursor = Some(first);
        brush.update_frame_state(state.clone());
        brush.update_uniforms(800.0, 600.0);

        state.cursor = Some(second);
        brush.update_frame_state(state);
        brush.update_uniforms(800.0, 600.0);

        assert_eq!(brush.uniforms.previous_cursor, second.rect);
        assert_eq!(brush.uniforms.current_cursor, second.rect);
    }

    #[test]
    fn configured_yazelix_ghostty_shader_presets_validate_as_wgpu_glsl() {
        // Defends: generated Yazelix Ghostty shader presets remain compatible with the WGPU shader path.
        let Ok(shader_dir) = std::env::var("YAZELIX_GHOSTTY_SHADER_DIR") else {
            return;
        };

        let shader_dir = std::path::PathBuf::from(shader_dir);
        let mut shader_paths = std::fs::read_dir(&shader_dir)
            .unwrap_or_else(|err| {
                panic!("failed to read shader dir {}: {err}", shader_dir.display())
            })
            .map(|entry| entry.expect("shader dir entry").path())
            .filter(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("glsl")
                    && path.file_name().and_then(|name| name.to_str())
                        != Some("cursor_trail_common.glsl")
            })
            .collect::<Vec<_>>();

        let generated_effects = shader_dir.join("generated_effects");
        if generated_effects.exists() {
            shader_paths.extend(
                std::fs::read_dir(&generated_effects)
                    .unwrap_or_else(|err| {
                        panic!(
                            "failed to read generated effects dir {}: {err}",
                            generated_effects.display()
                        )
                    })
                    .map(|entry| entry.expect("generated effect entry").path())
                    .filter(|path| {
                        path.extension().and_then(|extension| extension.to_str())
                            == Some("glsl")
                    }),
            );
        }

        shader_paths.sort();
        assert!(
            !shader_paths.is_empty(),
            "no generated Yazelix Ghostty shaders found in {}",
            shader_dir.display()
        );

        for path in &shader_paths {
            let source = std::fs::read_to_string(path).unwrap_or_else(|err| {
                panic!("failed to read shader {}: {err}", path.display())
            });
            let source = build_shadertoy_glsl(&source);
            validate_shadertoy_fragment_glsl(&source).unwrap_or_else(|err| {
                panic!("{} failed to validate: {err}", path.display())
            });
        }
    }
}
