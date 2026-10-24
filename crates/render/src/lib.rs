use bytemuck::{Pod, Zeroable};
use glyphon::{
    Cache, FontSystem, Metrics, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use std::sync::Arc;
use term_core::{CursorShape, Grid};
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BackgroundVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl BackgroundVertex {
    const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<BackgroundVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ImageVertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

impl ImageVertex {
    const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ImageVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ScreenUniforms {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    SplitHorizontal,
    SplitVertical,
    Find,
    Menu,
}

impl HeaderAction {
    pub const ALL: [HeaderAction; 4] = [
        HeaderAction::SplitHorizontal,
        HeaderAction::SplitVertical,
        HeaderAction::Find,
        HeaderAction::Menu,
    ];

    pub fn rect(self, win_width: f32) -> (f32, f32, f32, f32) {
        let y = 4.0;
        let h = 22.0;
        let w = 28.0;
        let x = match self {
            HeaderAction::SplitHorizontal => win_width - 136.0,
            HeaderAction::SplitVertical => win_width - 104.0,
            HeaderAction::Find => win_width - 72.0,
            HeaderAction::Menu => win_width - 36.0,
        };
        (x, y, w, h)
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            HeaderAction::SplitHorizontal => "Yatay Böl (Ctrl+Shift+D)",
            HeaderAction::SplitVertical => "Dikey Böl (Ctrl+Shift+E)",
            HeaderAction::Find => "Bul (Ctrl+Shift+F)",
            HeaderAction::Menu => "Komut Paleti ve Ayarlar (Ctrl+Shift+P)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopBarHit {
    Tab(usize),
    TabClose(usize),
    NewTab,
    Action(HeaderAction),
}

#[derive(Debug, Clone)]
pub struct CommandPaletteItem {
    pub id: &'static str,
    pub title: &'static str,
    pub shortcut: &'static str,
    pub category: &'static str,
}

#[derive(Debug, Clone)]
pub struct CommandPaletteInfo<'a> {
    pub query: &'a str,
    pub items: &'a [CommandPaletteItem],
    pub selected_index: usize,
}

pub fn get_top_bar_layout(win_width: f32, tab_count: usize) -> (f32, f32, f32, (f32, f32, f32, f32)) {
    let bar_height = 30.0;
    let right_toolbar_w = 144.0;
    let plus_btn_w = 26.0;
    let available_tabs_w = (win_width - right_toolbar_w - plus_btn_w - 12.0).max(60.0);
    let max_w = (available_tabs_w / tab_count.max(1) as f32).min(240.0);
    let tab_w = max_w.max(50.0);
    let tabs_total_w = (tab_count as f32 * tab_w).min(available_tabs_w);
    let plus_x = (tabs_total_w + 4.0).min(win_width - right_toolbar_w - plus_btn_w);
    let plus_rect = (plus_x, 4.0, plus_btn_w, 22.0);
    (tab_w, tabs_total_w, bar_height, plus_rect)
}

pub fn get_top_bar_hit(x: f32, y: f32, win_width: f32, tab_count: usize) -> Option<TopBarHit> {
    let (tab_w, tabs_total_w, bar_height, (px, py, pw, ph)) = get_top_bar_layout(win_width, tab_count);

    if y < 1.0 || y > bar_height - 1.0 {
        return None;
    }

    // 1. Sağdaki araç butonları kontrolü
    for action in HeaderAction::ALL {
        let (bx, by, bw, bh) = action.rect(win_width);
        if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
            return Some(TopBarHit::Action(action));
        }
    }

    // 2. Sekmelerin hemen sağındaki '+' yeni sekme butonu
    if x >= px && x <= px + pw && y >= py && y <= py + ph {
        return Some(TopBarHit::NewTab);
    }

    // 3. Soldaki sekmeler ve '✕' kapatma butonu
    if x >= 0.0 && x < tabs_total_w {
        let tab_idx = (x / tab_w) as usize;
        if tab_idx < tab_count {
            let close_x = tab_idx as f32 * tab_w + tab_w - 24.0;
            let close_y = 6.0;
            let close_size = 18.0;
            if x >= close_x && x <= close_x + close_size && y >= close_y && y <= close_y + close_size {
                return Some(TopBarHit::TabClose(tab_idx));
            } else {
                return Some(TopBarHit::Tab(tab_idx));
            }
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct TabInfo {
    pub title: String,
    pub is_active: bool,
}

#[derive(Clone)]
pub struct PaneDrawInfo<'a> {
    pub id: usize,
    pub grid: &'a Grid,
    pub col_offset: usize,
    pub row_offset: usize,
    pub cols: usize,
    pub rows: usize,
    pub is_active: bool,
    pub ghost_hint: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct PaneCachedState {
    pub pane_id: usize,
    pub version: u64,
    pub cols: usize,
    pub rows: usize,
    pub scroll_offset: usize,
    pub cell_bg_vertices: Vec<BackgroundVertex>,
}

pub struct TerminalRenderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub viewport: Viewport,
    pub text_atlas: TextAtlas,
    pub text_renderer: TextRenderer,
    pub text_buffer: glyphon::Buffer,
    pub search_buffer: glyphon::Buffer,
    pub tab_buffer: glyphon::Buffer,
    pub tooltip_buffer: glyphon::Buffer,
    pub palette_buffer: glyphon::Buffer,
    pub palette_footer_buffer: glyphon::Buffer,
    pub tab_buffers: Vec<glyphon::Buffer>,
    pub pane_buffers: Vec<glyphon::Buffer>,
    pub pane_cached_states: Vec<Option<PaneCachedState>>,
    pub font_family: String,
    pub font_size: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub theme_bg: (u8, u8, u8),
    pub theme_fg: (u8, u8, u8),
    pub theme_cursor_color: [f32; 4],
    pub theme_opacity: f32,
    pub show_header_bar: bool,
    pub hovered_header_action: Option<HeaderAction>,
    pub hovered_top_bar: Option<TopBarHit>,
    pub pressed_top_bar: Option<TopBarHit>,
    pub cursor_smooth_pos: Option<[f32; 2]>,
    pub last_frame_time: std::time::Instant,
    pub is_animating: bool,

    // Arka Plan Dikdörtgen (Quad) Render Hattı
    bg_pipeline: wgpu::RenderPipeline,
    bg_bind_group: wgpu::BindGroup,
    bg_uniform_buffer: wgpu::Buffer,
    bg_vertex_buffer: wgpu::Buffer,
    bg_vertex_capacity: usize,

    // Sixel Görsel Render Hattı
    image_pipeline: wgpu::RenderPipeline,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
    image_vertex_buffer: wgpu::Buffer,
}

const IMAGE_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let ndc_x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / uniforms.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
"#;

const BG_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let ndc_x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / uniforms.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

impl TerminalRenderer {
    pub async fn new(
        window: Arc<Window>,
        custom_font: Option<String>,
        initial_font_size: Option<f32>,
    ) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Uygun GPU adaptörü bulunamadı"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Terminal Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let alpha_mode = if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            surface_caps.alpha_modes[0]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // 1. Glyphon Metin Motoru ve Sabit Genişlikli (Monospace) Yazı Tipi Seçimi
        let mut font_system = FontSystem::new();

        let font_family = {
            let db = font_system.db();
            if let Some(cf) = custom_font {
                if db.faces().any(|f| f.families.iter().any(|(fam, _)| fam == &cf)) {
                    cf
                } else {
                    let preferred = [
                        "MesloLGS Nerd Font Mono",
                        "MesloLGS Nerd Font",
                        "MesloLGM Nerd Font Mono",
                        "MesloLGM Nerd Font",
                        "FantasqueSansM Nerd Font Mono",
                        "Noto Sans Mono",
                        "DejaVu Sans Mono",
                        "Liberation Mono",
                    ];
                    preferred
                        .iter()
                        .find(|&&name| {
                            db.faces()
                                .any(|f| f.families.iter().any(|(fam, _)| fam == name))
                        })
                        .copied()
                        .unwrap_or("monospace")
                        .to_string()
                }
            } else {
                let preferred = [
                    "MesloLGS Nerd Font Mono",
                    "MesloLGS Nerd Font",
                    "MesloLGM Nerd Font Mono",
                    "MesloLGM Nerd Font",
                    "FantasqueSansM Nerd Font Mono",
                    "Noto Sans Mono",
                    "DejaVu Sans Mono",
                    "Liberation Mono",
                ];
                preferred
                    .iter()
                    .find(|&&name| {
                        db.faces()
                            .any(|f| f.families.iter().any(|(fam, _)| fam == name))
                    })
                    .copied()
                    .unwrap_or("monospace")
                    .to_string()
            }
        };

        font_system.db_mut().set_monospace_family(&font_family);

        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let mut viewport = Viewport::new(&device, &cache);
        viewport.update(&queue, Resolution { width, height });

        let mut text_atlas = TextAtlas::new(&device, &queue, &cache, surface_format);
        let text_renderer = TextRenderer::new(
            &mut text_atlas,
            &device,
            wgpu::MultisampleState::default(),
            None,
        );

        let font_size = initial_font_size.unwrap_or(14.0).clamp(8.0, 48.0);
        let mut test_buf =
            glyphon::Buffer::new(&mut font_system, Metrics::new(font_size, font_size * 1.3));
        test_buf.set_text(
            &mut font_system,
            "M",
            glyphon::Attrs::new().family(glyphon::Family::Name(&font_family)),
            glyphon::Shaping::Advanced,
        );
        test_buf.shape_until_scroll(&mut font_system, false);

        let measured_width = test_buf
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.first().map(|g| g.w))
            .unwrap_or(font_size * 0.6);
        let cell_width = measured_width.ceil().max(6.0);
        let cell_height = (font_size * 1.35).ceil().max(12.0);

        let text_buffer =
            glyphon::Buffer::new(&mut font_system, Metrics::new(font_size, cell_height));
        let search_buffer =
            glyphon::Buffer::new(&mut font_system, Metrics::new(font_size, cell_height));
        let tab_buffer =
            glyphon::Buffer::new(&mut font_system, Metrics::new(font_size * 0.9, cell_height));
        let tooltip_buffer = glyphon::Buffer::new(
            &mut font_system,
            Metrics::new((font_size * 0.82).round().max(10.0), cell_height * 0.9),
        );
        let palette_buffer = glyphon::Buffer::new(
            &mut font_system,
            Metrics::new(font_size * 0.9, cell_height),
        );
        let palette_footer_buffer = glyphon::Buffer::new(
            &mut font_system,
            Metrics::new((font_size * 0.8).round().max(9.0), 22.0),
        );

        // 2. Arka Plan Dikdörtgen (Quad) Shader ve Pipeline Kurulumu
        let bg_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terminal Background Shader"),
            source: wgpu::ShaderSource::Wgsl(BG_SHADER.into()),
        });

        let uniforms = ScreenUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        use wgpu::util::DeviceExt;
        let bg_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Background Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bg_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Background Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let bg_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Background Bind Group"),
            layout: &bg_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bg_uniform_buffer.as_entire_binding(),
            }],
        });

        let bg_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Background Pipeline Layout"),
            bind_group_layouts: &[&bg_bind_group_layout],
            push_constant_ranges: &[],
        });

        let bg_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Background Render Pipeline"),
            layout: Some(&bg_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bg_shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[BackgroundVertex::DESC],
            },
            fragment: Some(wgpu::FragmentState {
                module: &bg_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let initial_capacity = 4096;
        let bg_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Background Vertex Buffer"),
            size: (initial_capacity * std::mem::size_of::<BackgroundVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 3. Sixel Görsel Pipeline ve Sampler Kurulumu
        let image_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terminal Image Shader"),
            source: wgpu::ShaderSource::Wgsl(IMAGE_SHADER.into()),
        });

        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Image Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let image_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Image Pipeline Layout"),
                bind_group_layouts: &[&bg_bind_group_layout, &image_bind_group_layout],
                push_constant_ranges: &[],
            });

        let image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Render Pipeline"),
            layout: Some(&image_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &image_shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[ImageVertex::DESC],
            },
            fragment: Some(wgpu::FragmentState {
                module: &image_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Image Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let image_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Image Vertex Buffer"),
            size: (6 * std::mem::size_of::<ImageVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            font_system,
            swash_cache,
            viewport,
            text_atlas,
            text_renderer,
            text_buffer,
            search_buffer,
            tab_buffer,
            tooltip_buffer,
            palette_buffer,
            palette_footer_buffer,
            tab_buffers: Vec::new(),
            pane_buffers: Vec::new(),
            pane_cached_states: Vec::new(),
            font_family,
            font_size,
            cell_width,
            cell_height,
            theme_bg: (0, 0, 0),
            theme_fg: (226, 228, 234),
            theme_cursor_color: [122.0 / 255.0, 162.0 / 255.0, 247.0 / 255.0, 0.9],
            theme_opacity: 1.0,
            show_header_bar: true,
            hovered_header_action: None,
            hovered_top_bar: None,
            pressed_top_bar: None,
            cursor_smooth_pos: None,
            last_frame_time: std::time::Instant::now(),
            is_animating: false,
            bg_pipeline,
            bg_bind_group,
            bg_uniform_buffer,
            bg_vertex_buffer,
            bg_vertex_capacity: initial_capacity,
            image_pipeline,
            image_bind_group_layout,
            image_sampler,
            image_vertex_buffer,
        })
    }

    pub fn set_colors(&mut self, bg: (u8, u8, u8), fg: (u8, u8, u8), cursor: [f32; 4]) {
        self.theme_bg = bg;
        self.theme_fg = fg;
        self.theme_cursor_color = cursor;
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        self.theme_opacity = opacity.clamp(0.05, 1.0);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.viewport
                .update(&self.queue, Resolution { width, height });

            let uniforms = ScreenUniforms {
                screen_size: [width as f32, height as f32],
                _padding: [0.0, 0.0],
            };
            self.queue.write_buffer(
                &self.bg_uniform_buffer,
                0,
                bytemuck::cast_slice(&[uniforms]),
            );
        }
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        let font_size = font_size.clamp(8.0, 48.0);
        self.font_size = font_size;

        let mut test_buf =
            glyphon::Buffer::new(&mut self.font_system, glyphon::Metrics::new(font_size, font_size * 1.3));
        test_buf.set_text(
            &mut self.font_system,
            "M",
            glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)),
            glyphon::Shaping::Advanced,
        );
        test_buf.shape_until_scroll(&mut self.font_system, false);

        let measured_width = test_buf
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.first().map(|g| g.w))
            .unwrap_or(font_size * 0.6);
        self.cell_width = measured_width.ceil().max(6.0);
        self.cell_height = (font_size * 1.35).ceil().max(12.0);

        self.text_buffer =
            glyphon::Buffer::new(&mut self.font_system, glyphon::Metrics::new(font_size, self.cell_height));
        self.search_buffer =
            glyphon::Buffer::new(&mut self.font_system, glyphon::Metrics::new(font_size, self.cell_height));
        self.tab_buffer =
            glyphon::Buffer::new(&mut self.font_system, glyphon::Metrics::new(font_size * 0.9, self.cell_height));
        self.tooltip_buffer =
            glyphon::Buffer::new(&mut self.font_system, glyphon::Metrics::new((font_size * 0.82).round().max(10.0), self.cell_height * 0.9));
        self.palette_buffer =
            glyphon::Buffer::new(&mut self.font_system, glyphon::Metrics::new(font_size * 0.9, self.cell_height));
        self.palette_footer_buffer =
            glyphon::Buffer::new(&mut self.font_system, glyphon::Metrics::new((font_size * 0.8).round().max(9.0), 22.0));
        self.tab_buffers.clear();
        self.pane_buffers.clear();
        self.pane_cached_states.clear();
    }

    pub fn calculate_grid_size(&self, width: u32, height: u32, has_header: bool) -> (usize, usize) {
        let padding_x = 16.0; // 8px sol, 8px sağ
        let padding_top = if has_header { 30.0 + 6.0 } else { 8.0 };
        let padding_y = padding_top + 8.0;
        let usable_width = (width as f32 - padding_x).max(self.cell_width);
        let usable_height = (height as f32 - padding_y).max(self.cell_height);

        let cols = (usable_width / self.cell_width).floor() as usize;
        let rows = (usable_height / self.cell_height).floor() as usize;
        (cols.max(10), rows.max(4))
    }

    pub fn pixel_to_cell(&self, x: f32, y: f32, has_header: bool) -> (usize, usize) {
        let padding_left = 8.0;
        let padding_top = if has_header { 30.0 + 6.0 } else { 8.0 };
        let col = ((x - padding_left).max(0.0) / self.cell_width).floor() as usize;
        let row = ((y - padding_top).max(0.0) / self.cell_height).floor() as usize;
        (col, row)
    }

    pub fn render_panes(
        &mut self,
        panes: &[PaneDrawInfo],
        tabs: Option<&[TabInfo]>,
        palette: Option<&CommandPaletteInfo>,
    ) -> anyhow::Result<()> {
        let output = match self.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Outdated) => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Terminal Render Encoder"),
            });

        // Tema Renkleri
        let default_fg = glyphon::Color::rgb(self.theme_fg.0, self.theme_fg.1, self.theme_fg.2);
        let cursor_color = self.theme_cursor_color;
        let selection_bg_color = [51.0 / 255.0, 70.0 / 255.0, 110.0 / 255.0, 0.85];

        let has_header = self.show_header_bar && tabs.is_some();
        let padding_left = 8.0;
        let padding_top = if has_header { 30.0 + 6.0 } else { 8.0 };

        // Animasyon ve Delta Time hesabı
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32().min(0.1);
        self.last_frame_time = now;
        self.is_animating = false;

        // İmleç yanıp sönme hesabı (500ms döngü)
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base_show_cursor = (now_ms / 500).is_multiple_of(2);

        // 1. Arka Plan Dikdörtgenlerini (Quad) Oluştur
        let mut bg_vertices: Vec<BackgroundVertex> = Vec::new();

        while self.pane_buffers.len() < panes.len() {
            self.pane_buffers.push(glyphon::Buffer::new(
                &mut self.font_system,
                glyphon::Metrics::new(self.font_size, self.cell_height),
            ));
        }
        while self.pane_cached_states.len() < panes.len() {
            self.pane_cached_states.push(None);
        }

        let mut pane_text_areas = Vec::new();
        let mut search_overlay = None;

        for (p_idx, pane) in panes.iter().enumerate() {
            let grid = pane.grid;
            let pane_px = padding_left + pane.col_offset as f32 * self.cell_width;
            let pane_py = padding_top + pane.row_offset as f32 * self.cell_height;
            let pane_pw = pane.cols as f32 * self.cell_width;
            let pane_ph = pane.rows as f32 * self.cell_height;

            if panes.len() > 1 {
                let div_color = [32.0 / 255.0, 35.0 / 255.0, 48.0 / 255.0, 0.85];
                if pane.col_offset > 0 {
                    push_quad(&mut bg_vertices, pane_px - 1.0, pane_py, 1.5, pane_ph, div_color);
                }
                if pane.row_offset > 0 {
                    push_quad(&mut bg_vertices, pane_px, pane_py - 1.0, pane_pw, 1.5, div_color);
                }

                if pane.is_active {
                    let active_border = self.theme_cursor_color;
                    push_quad(&mut bg_vertices, pane_px, pane_py, pane_pw, 1.5, active_border);
                    push_quad(&mut bg_vertices, pane_px, pane_py + pane_ph - 1.5, pane_pw, 1.5, active_border);
                    push_quad(&mut bg_vertices, pane_px, pane_py, 1.5, pane_ph, active_border);
                    push_quad(&mut bg_vertices, pane_px + pane_pw - 1.5, pane_py, 1.5, pane_ph, active_border);
                }
            }

            // Aktif panel için iOS tarzı akıcı yaylanmalı (spring lerp) imleç koordinatı hesabı
            let mut smooth_cursor_quad = None;
            if pane.is_active && grid.scroll_offset == 0 && grid.cursor_visible {
                let target_cx = pane_px + grid.cursor_col as f32 * self.cell_width;
                let target_cy = pane_py + grid.cursor_row as f32 * self.cell_height;

                let (curr_cx, curr_cy) = if let Some(ref mut pos) = self.cursor_smooth_pos {
                    if (target_cx - pos[0]).abs() > self.cell_width * 25.0
                        || (target_cy - pos[1]).abs() > self.cell_height * 15.0
                    {
                        pos[0] = target_cx;
                        pos[1] = target_cy;
                        (target_cx, target_cy)
                    } else {
                        let decay = 35.0; // iOS akıcı yaylanma katsayısı
                        let factor = (1.0 - (-decay * dt).exp()).clamp(0.0, 1.0);
                        pos[0] += (target_cx - pos[0]) * factor;
                        pos[1] += (target_cy - pos[1]) * factor;

                        let dist_sq = (target_cx - pos[0]).powi(2) + (target_cy - pos[1]).powi(2);
                        if dist_sq > 0.08 {
                            self.is_animating = true;
                        } else {
                            pos[0] = target_cx;
                            pos[1] = target_cy;
                        }
                        (pos[0], pos[1])
                    }
                } else {
                    self.cursor_smooth_pos = Some([target_cx, target_cy]);
                    (target_cx, target_cy)
                };

                let effective_show_cursor = base_show_cursor || self.is_animating;
                if effective_show_cursor {
                    smooth_cursor_quad = Some((curr_cx, curr_cy, grid.cursor_shape));
                }
            }

            let is_cache_valid = self.pane_cached_states.get(p_idx).and_then(|c| c.as_ref()).map_or(false, |c| {
                c.pane_id == pane.id
                    && c.version == grid.version
                    && c.cols == pane.cols
                    && c.rows == pane.rows
                    && c.scroll_offset == grid.scroll_offset
            });

            if is_cache_valid {
                if let Some(Some(cached)) = self.pane_cached_states.get(p_idx) {
                    bg_vertices.extend_from_slice(&cached.cell_bg_vertices);
                }
            } else {
                let mut pane_bg_vertices: Vec<BackgroundVertex> = Vec::new();
                let mut spans: Vec<(String, glyphon::Attrs)> = Vec::with_capacity(grid.rows * 4);

                for r in 0..grid.rows {
                    let line_cells = grid.get_display_line(r);
                    let mut current_span_text = String::with_capacity(grid.cols);
                    let mut current_attrs: Option<glyphon::Attrs> = None;

                    let cell_y = pane_py + r as f32 * self.cell_height;
                    let mut active_bg_span: Option<(usize, usize, [f32; 4])> = None;
                    let mut active_ul_span: Option<(usize, usize, [f32; 4])> = None;

                    for (c, cell) in line_cells.iter().enumerate() {
                        let is_selected = grid
                            .selection
                            .as_ref()
                            .is_some_and(|sel| sel.contains(c, r));

                        let is_cursor = pane.is_active
                            && grid.scroll_offset == 0
                            && grid.cursor_visible
                            && r == grid.cursor_row
                            && c == grid.cursor_col;

                        let (mut eff_fg, mut eff_bg) = (cell.fg, cell.bg);
                        if cell.inverse {
                            std::mem::swap(&mut eff_fg, &mut eff_bg);
                        }
                        if cell.dim {
                            eff_fg = (eff_fg.0 / 2, eff_fg.1 / 2, eff_fg.2 / 2);
                        }

                        // Arama eşleşmesi vurgusu (Search match highlight)
                        if let Some(is_active) = grid.get_search_match_status(c, r) {
                            if is_active {
                                eff_bg = (255, 158, 100);
                                eff_fg = (0, 0, 0);
                            } else {
                                eff_bg = (224, 175, 104);
                                eff_fg = (0, 0, 0);
                            }
                        }

                        // 1. Hücre arka plan rengi belirleme ve yatay kümeleme (Batching)
                        let cell_bg = if is_selected {
                            Some(selection_bg_color)
                        } else if eff_bg != self.theme_bg {
                            Some([
                                eff_bg.0 as f32 / 255.0,
                                eff_bg.1 as f32 / 255.0,
                                eff_bg.2 as f32 / 255.0,
                                1.0,
                            ])
                        } else {
                            None
                        };

                        match (active_bg_span, cell_bg) {
                            (Some((start, _, color)), Some(new_color)) if color == new_color => {
                                active_bg_span = Some((start, c + 1, color));
                            }
                            (Some((start, end, color)), new_opt) => {
                                let span_x = pane_px + start as f32 * self.cell_width;
                                let span_w = (end - start) as f32 * self.cell_width;
                                push_quad(&mut pane_bg_vertices, span_x, cell_y, span_w, self.cell_height, color);
                                active_bg_span = new_opt.map(|col| (c, c + 1, col));
                            }
                            (None, Some(new_color)) => {
                                active_bg_span = Some((c, c + 1, new_color));
                            }
                            (None, None) => {}
                        }

                        // Altı çizili metin desteği ve yatay kümeleme (SGR 4)
                        let cell_ul = if cell.underline && !(is_cursor && grid.cursor_shape == CursorShape::Underline) {
                            Some([
                                eff_fg.0 as f32 / 255.0,
                                eff_fg.1 as f32 / 255.0,
                                eff_fg.2 as f32 / 255.0,
                                0.9,
                            ])
                        } else {
                            None
                        };

                        match (active_ul_span, cell_ul) {
                            (Some((start, _, color)), Some(new_color)) if color == new_color => {
                                active_ul_span = Some((start, c + 1, color));
                            }
                            (Some((start, end, color)), new_opt) => {
                                let span_x = pane_px + start as f32 * self.cell_width;
                                let span_w = (end - start) as f32 * self.cell_width;
                                push_quad(&mut pane_bg_vertices, span_x, cell_y + self.cell_height - 1.5, span_w, 1.5, color);
                                active_ul_span = new_opt.map(|col| (c, c + 1, col));
                            }
                            (None, Some(new_color)) => {
                                active_ul_span = Some((c, c + 1, new_color));
                            }
                            (None, None) => {}
                        }

                        // 2. Yazı rengi ve stil belirleme
                        let text_color = if is_selected {
                            glyphon::Color::rgb(255, 255, 255)
                        } else {
                            glyphon::Color::rgb(eff_fg.0, eff_fg.1, eff_fg.2)
                        };

                        let mut attrs = glyphon::Attrs::new()
                            .family(glyphon::Family::Name(&self.font_family))
                            .color(text_color);
                        if cell.bold {
                            attrs = attrs.weight(glyphon::Weight::BOLD);
                        }
                        if cell.italic {
                            attrs = attrs.style(glyphon::Style::Italic);
                        }

                        if cell.c != '\0' {
                            if let Some(active_attrs) = current_attrs {
                                if active_attrs == attrs {
                                    current_span_text.push(cell.c);
                                } else {
                                    if !current_span_text.is_empty() {
                                        spans.push((std::mem::take(&mut current_span_text), active_attrs));
                                    }
                                    current_span_text.push(cell.c);
                                    current_attrs = Some(attrs);
                                }
                            } else {
                                current_span_text.push(cell.c);
                                current_attrs = Some(attrs);
                            }
                        }
                    }

                    // Satır sonu arkaplan ve altçizgi kümeleme tamamlaması
                    if let Some((start, end, color)) = active_bg_span {
                        let span_x = pane_px + start as f32 * self.cell_width;
                        let span_w = (end - start) as f32 * self.cell_width;
                        push_quad(&mut pane_bg_vertices, span_x, cell_y, span_w, self.cell_height, color);
                    }
                    if let Some((start, end, color)) = active_ul_span {
                        let span_x = pane_px + start as f32 * self.cell_width;
                        let span_w = (end - start) as f32 * self.cell_width;
                        push_quad(&mut pane_bg_vertices, span_x, cell_y + self.cell_height - 1.5, span_w, 1.5, color);
                    }

                    if !current_span_text.is_empty() {
                        if let Some(attrs) = current_attrs {
                            spans.push((current_span_text, attrs));
                        }
                    }

                    // Hayalet İpucu (Ghost Hint) - İmlecin bulunduğu aktif komut satırında açıklama basma
                    if r == grid.cursor_row && pane.is_active && grid.scroll_offset == 0 {
                        if let Some(hint_text) = pane.ghost_hint {
                            let hint_attrs = glyphon::Attrs::new()
                                .family(glyphon::Family::Name(&self.font_family))
                                .style(glyphon::Style::Italic)
                                .color(glyphon::Color::rgb(110, 115, 140));
                            spans.push((format!("  ({})", hint_text), hint_attrs));
                        }
                    }

                    spans.push((
                        "\n".to_string(),
                        glyphon::Attrs::new()
                            .family(glyphon::Family::Name(&self.font_family))
                            .color(default_fg),
                    ));
                }

                let span_refs: Vec<(&str, glyphon::Attrs)> = spans
                    .iter()
                    .map(|(text, attrs)| (text.as_str(), *attrs))
                    .collect();
                self.pane_buffers[p_idx].set_rich_text(
                    &mut self.font_system,
                    span_refs,
                    glyphon::Attrs::new()
                        .family(glyphon::Family::Name(&self.font_family))
                        .color(default_fg),
                    glyphon::Shaping::Advanced,
                );
                self.pane_buffers[p_idx]
                    .shape_until_scroll(&mut self.font_system, false);

                bg_vertices.extend_from_slice(&pane_bg_vertices);
                self.pane_cached_states[p_idx] = Some(PaneCachedState {
                    pane_id: pane.id,
                    version: grid.version,
                    cols: pane.cols,
                    rows: pane.rows,
                    scroll_offset: grid.scroll_offset,
                    cell_bg_vertices: pane_bg_vertices,
                });
            }

            // Yumuşak kayan imleci (Smooth Spring Cursor) hücre arka planlarının üzerine çiz
            if let Some((cx, cy, shape)) = smooth_cursor_quad {
                match shape {
                    CursorShape::Block => {
                        push_quad(
                            &mut bg_vertices,
                            cx,
                            cy,
                            self.cell_width,
                            self.cell_height,
                            cursor_color,
                        );
                    }
                    CursorShape::Beam => {
                        push_quad(
                            &mut bg_vertices,
                            cx,
                            cy,
                            2.0,
                            self.cell_height,
                            cursor_color,
                        );
                    }
                    CursorShape::Underline => {
                        push_quad(
                            &mut bg_vertices,
                            cx,
                            cy + self.cell_height - 2.0,
                            self.cell_width,
                            2.0,
                            cursor_color,
                        );
                    }
                }
            }

            pane_text_areas.push((p_idx, pane_px, pane_py, pane_pw, pane_ph));

            // Aktif panel arama çubuğu
            if pane.is_active {
                if let Some(query) = &grid.search_query {
                    let total = grid.search_matches.len();
                    let current_str = if total > 0 {
                        format!("{}/{}", grid.active_match_idx + 1, total)
                    } else {
                        "0/0".to_string()
                    };
                    let bar_text = format!(" Search: {} [{}] ", query, current_str);
                    let bar_width = (bar_text.chars().count() as f32 * self.cell_width) + 24.0;
                    let bar_height = self.cell_height + 8.0;
                    let bar_x = (pane_px + pane_pw - bar_width - 16.0).max(pane_px + 8.0);
                    let bar_y = pane_py + 8.0;

                    push_quad(
                        &mut bg_vertices,
                        bar_x,
                        bar_y,
                        bar_width,
                        bar_height,
                        [18.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 0.98],
                    );
                    let s_border = self.theme_cursor_color;
                    push_quad(&mut bg_vertices, bar_x, bar_y, bar_width, 1.5, s_border);
                    push_quad(&mut bg_vertices, bar_x, bar_y + bar_height - 1.5, bar_width, 1.5, s_border);
                    push_quad(&mut bg_vertices, bar_x, bar_y, 1.5, bar_height, s_border);
                    push_quad(&mut bg_vertices, bar_x + bar_width - 1.5, bar_y, 1.5, bar_height, s_border);

                    self.search_buffer.set_text(
                        &mut self.font_system,
                        &bar_text,
                        glyphon::Attrs::new()
                            .family(glyphon::Family::Name(&self.font_family))
                            .weight(glyphon::Weight::BOLD)
                            .color(glyphon::Color::rgb(226, 228, 234)),
                        glyphon::Shaping::Advanced,
                    );
                    self.search_buffer.shape_until_scroll(&mut self.font_system, false);
                    search_overlay = Some((bar_x, bar_y, bar_width, bar_height));
                }
            }
        }

        // 4. Tek Satır Üst Başlık ve Araç Çubuğu Çizimi (Unified Single-Row Header Bar)
        let mut tooltip_overlay: Option<(f32, f32, f32, f32)> = None;
        if let Some(tab_list) = tabs {
            if has_header {
                let screen_w = self.config.width as f32;
                let (tab_w, tabs_total_w, bar_height, (px, py, pw, ph)) =
                    get_top_bar_layout(screen_w, tab_list.len());

                let is_black_theme = self.theme_bg.0 <= 15 && self.theme_bg.1 <= 15 && self.theme_bg.2 <= 15;

                // Üst çubuk ana zemin rengi (hafif elevated koyu ton)
                let bar_bg = if is_black_theme {
                    [12.0 / 255.0, 13.0 / 255.0, 18.0 / 255.0, 1.0]
                } else {
                    [
                        (self.theme_bg.0 as f32 * 0.75 / 255.0),
                        (self.theme_bg.1 as f32 * 0.75 / 255.0),
                        (self.theme_bg.2 as f32 * 0.75 / 255.0),
                        1.0,
                    ]
                };

                // 1px keskin alt sınır çizgisi
                let bar_border = if is_black_theme {
                    [28.0 / 255.0, 31.0 / 255.0, 44.0 / 255.0, 1.0]
                } else {
                    [
                        (self.theme_bg.0 as f32 + 25.0).min(255.0) / 255.0,
                        (self.theme_bg.1 as f32 + 25.0).min(255.0) / 255.0,
                        (self.theme_bg.2 as f32 + 35.0).min(255.0) / 255.0,
                        1.0,
                    ]
                };

                // Tüm üst çubuk zeminini çiz
                push_quad(&mut bg_vertices, 0.0, 0.0, screen_w, bar_height, bar_bg);

                // 1. Sol Bölge: Sekmeler ve İçlerindeki '✕' Kapatma Butonu
                while self.tab_buffers.len() < tab_list.len() {
                    self.tab_buffers.push(glyphon::Buffer::new(
                        &mut self.font_system,
                        glyphon::Metrics::new((self.font_size * 0.88).round(), self.cell_height),
                    ));
                }

                for (idx, tab) in tab_list.iter().enumerate() {
                    let tx = idx as f32 * tab_w;
                    let is_active = tab.is_active;
                    let is_tab_hovered = self.hovered_top_bar == Some(TopBarHit::Tab(idx));

                    if is_active {
                        // Aktif sekme arka planı doğrudan terminal gövde rengiyle kusursuz birleşir
                        let active_bg = [
                            self.theme_bg.0 as f32 / 255.0,
                            self.theme_bg.1 as f32 / 255.0,
                            self.theme_bg.2 as f32 / 255.0,
                            1.0,
                        ];
                        push_quad(&mut bg_vertices, tx, 0.0, tab_w, bar_height, active_bg);

                        // Üstte 2px tema vurgusu çizgisi (theme_cursor_color)
                        push_quad(
                            &mut bg_vertices,
                            tx,
                            0.0,
                            tab_w,
                            2.0,
                            self.theme_cursor_color,
                        );

                        // Aktif sekmenin yan ince 1px kenarlıkları
                        if idx > 0 {
                            push_quad(&mut bg_vertices, tx, 0.0, 1.0, bar_height, bar_border);
                        }
                        push_quad(&mut bg_vertices, tx + tab_w - 1.0, 0.0, 1.0, bar_height, bar_border);
                        // Not: Aktif sekmenin altında alt çizgi çizilmez, böylece doğrudan terminal gövdesine bağlanır

                        // Aktif sekme önünde parlak tema vurgu noktası (5x5 px)
                        push_quad(&mut bg_vertices, tx + 9.0, 12.5, 5.0, 5.0, self.theme_cursor_color);
                    } else {
                        // Pasif sekme: Hover varsa hafif aydınlat
                        if is_tab_hovered {
                            push_quad(
                                &mut bg_vertices,
                                tx,
                                0.0,
                                tab_w,
                                bar_height - 1.0,
                                [22.0 / 255.0, 24.0 / 255.0, 34.0 / 255.0, 1.0],
                            );
                        }
                        // Pasif sekmenin altındaki 1px sınır çizgisi
                        push_quad(&mut bg_vertices, tx, bar_height - 1.0, tab_w, 1.0, bar_border);

                        // Pasif sekmenin sağındaki 1px dikey ayırıcı çizgi
                        push_quad(
                            &mut bg_vertices,
                            tx + tab_w - 1.0,
                            6.0,
                            1.0,
                            bar_height - 12.0,
                            bar_border,
                        );

                        // Pasif sekme önünde sönük durum noktası (4x4 px)
                        push_quad(
                            &mut bg_vertices,
                            tx + 9.5,
                            13.0,
                            4.0,
                            4.0,
                            [80.0 / 255.0, 85.0 / 255.0, 105.0 / 255.0, 0.7],
                        );
                    }

                    // Sekme içi '✕' kapatma butonu
                    let close_x = tx + tab_w - 24.0;
                    let close_y = 6.0;
                    let close_size = 18.0;
                    let is_close_pressed = self.pressed_top_bar == Some(TopBarHit::TabClose(idx));
                    let is_close_hovered = self.hovered_top_bar == Some(TopBarHit::TabClose(idx));
                    let close_offset_y = if is_close_pressed { 1.0 } else { 0.0 };

                    if is_close_hovered || is_close_pressed {
                        let bg_col = if is_close_pressed {
                            [247.0 / 255.0, 118.0 / 255.0, 142.0 / 255.0, 0.45]
                        } else {
                            [247.0 / 255.0, 118.0 / 255.0, 142.0 / 255.0, 0.28]
                        };
                        push_quad(
                            &mut bg_vertices,
                            close_x,
                            close_y,
                            close_size,
                            close_size,
                            bg_col,
                        );
                        let border_col = [247.0 / 255.0, 118.0 / 255.0, 142.0 / 255.0, 0.6];
                        push_quad(&mut bg_vertices, close_x, close_y, close_size, 1.0, border_col);
                        push_quad(&mut bg_vertices, close_x, close_y + close_size - 1.0, close_size, 1.0, border_col);
                        push_quad(&mut bg_vertices, close_x, close_y, 1.0, close_size, border_col);
                        push_quad(&mut bg_vertices, close_x + close_size - 1.0, close_y, 1.0, close_size, border_col);
                    }

                    let close_fg = if is_close_hovered || is_close_pressed {
                        [255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.0]
                    } else if is_active {
                        [160.0 / 255.0, 166.0 / 255.0, 190.0 / 255.0, 0.75]
                    } else {
                        [100.0 / 255.0, 105.0 / 255.0, 125.0 / 255.0, 0.5]
                    };

                    let cx = close_x + close_size / 2.0;
                    let cy = close_y + close_size / 2.0 + close_offset_y;
                    for d in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                        push_quad(&mut bg_vertices, cx + d, cy + d, 1.2, 1.2, close_fg);
                        push_quad(&mut bg_vertices, cx - d, cy + d, 1.2, 1.2, close_fg);
                    }

                    // Sekme başlık metnini hazırla ve ilgili tab_buffer'a yükle
                    let available_text_px = (tab_w - 48.0).max(20.0);
                    let approx_char_w = (self.cell_width * 0.88).max(6.0);
                    let max_chars = (available_text_px / approx_char_w).floor() as usize;

                    let truncated = if tab.title.chars().count() > max_chars && max_chars >= 4 {
                        let s: String = tab.title.chars().take(max_chars.saturating_sub(2)).collect();
                        format!("{}..", s)
                    } else {
                        tab.title.clone()
                    };

                    let text_fg = if is_active {
                        glyphon::Color::rgb(245, 246, 252) // Parlak beyaz
                    } else if is_tab_hovered {
                        glyphon::Color::rgb(215, 220, 235)
                    } else {
                        glyphon::Color::rgb(130, 135, 155) // Pasif gri
                    };

                    self.tab_buffers[idx].set_text(
                        &mut self.font_system,
                        &truncated,
                        glyphon::Attrs::new()
                            .family(glyphon::Family::Name(&self.font_family))
                            .color(text_fg),
                        glyphon::Shaping::Advanced,
                    );
                    self.tab_buffers[idx].shape_until_scroll(&mut self.font_system, false);
                }

                // Sekmelerin bittiği yerden pencerenin sonuna kadar alt 1px sınır çizgisi
                if tabs_total_w < screen_w {
                    push_quad(
                        &mut bg_vertices,
                        tabs_total_w,
                        bar_height - 1.0,
                        screen_w - tabs_total_w,
                        1.0,
                        bar_border,
                    );
                }

                // 2. Sekmelerin hemen sağındaki '+' Yeni Sekme Butonu
                let is_plus_pressed = self.pressed_top_bar == Some(TopBarHit::NewTab);
                let is_plus_hovered = self.hovered_top_bar == Some(TopBarHit::NewTab);
                let plus_offset_y = if is_plus_pressed { 1.0 } else { 0.0 };

                let (plus_bg, plus_border, plus_fg) = if is_plus_pressed {
                    (
                        Some([16.0 / 255.0, 18.0 / 255.0, 26.0 / 255.0, 1.0]),
                        Some(self.theme_cursor_color),
                        self.theme_cursor_color,
                    )
                } else if is_plus_hovered {
                    (
                        Some([28.0 / 255.0, 31.0 / 255.0, 44.0 / 255.0, 1.0]),
                        Some([60.0 / 255.0, 70.0 / 255.0, 100.0 / 255.0, 0.9]),
                        self.theme_cursor_color,
                    )
                } else {
                    (
                        None,
                        None,
                        [150.0 / 255.0, 155.0 / 255.0, 175.0 / 255.0, 0.85],
                    )
                };

                if let Some(bg) = plus_bg {
                    push_quad(&mut bg_vertices, px, py, pw, ph, bg);
                }
                if let Some(border) = plus_border {
                    push_quad(&mut bg_vertices, px, py, pw, 1.0, border);
                    push_quad(&mut bg_vertices, px, py + ph - 1.0, pw, 1.0, border);
                    push_quad(&mut bg_vertices, px, py, 1.0, ph, border);
                    push_quad(&mut bg_vertices, px + pw - 1.0, py, 1.0, ph, border);
                }

                let pcx = (px + pw / 2.0).round();
                let pcy = (py + ph / 2.0).round() + plus_offset_y;
                push_quad(&mut bg_vertices, pcx - 5.0, pcy - 0.75, 10.0, 1.5, plus_fg);
                push_quad(&mut bg_vertices, pcx - 0.75, pcy - 5.0, 1.5, 10.0, plus_fg);

                // 3. Pencerenin En Sağındaki Araç Butonları (Split H, Split V, Find, Menu)
                for action in HeaderAction::ALL {
                    let (bx, by, bw, bh) = action.rect(screen_w);
                    let is_pressed = self.pressed_top_bar == Some(TopBarHit::Action(action));
                    let is_hovered = self.hovered_top_bar == Some(TopBarHit::Action(action));
                    let offset_y = if is_pressed { 1.0 } else { 0.0 };

                    let (bg_color, border_color, icon_color) = if is_pressed {
                        (
                            Some([16.0 / 255.0, 18.0 / 255.0, 26.0 / 255.0, 1.0]),
                            Some(self.theme_cursor_color),
                            self.theme_cursor_color,
                        )
                    } else if is_hovered {
                        (
                            Some([28.0 / 255.0, 31.0 / 255.0, 44.0 / 255.0, 1.0]),
                            Some([60.0 / 255.0, 70.0 / 255.0, 100.0 / 255.0, 0.9]),
                            self.theme_cursor_color,
                        )
                    } else {
                        (
                            None,
                            None,
                            [150.0 / 255.0, 155.0 / 255.0, 175.0 / 255.0, 0.85],
                        )
                    };

                    if let Some(bg) = bg_color {
                        push_quad(&mut bg_vertices, bx, by, bw, bh, bg);
                    }
                    if let Some(border) = border_color {
                        push_quad(&mut bg_vertices, bx, by, bw, 1.0, border);
                        push_quad(&mut bg_vertices, bx, by + bh - 1.0, bw, 1.0, border);
                        push_quad(&mut bg_vertices, bx, by, 1.0, bh, border);
                        push_quad(&mut bg_vertices, bx + bw - 1.0, by, 1.0, bh, border);
                    }

                    let cx = (bx + bw / 2.0).round();
                    let cy = (by + bh / 2.0).round() + offset_y;

                    match action {
                        HeaderAction::SplitHorizontal => {
                            // Hassas tel kafes bölme ikonu: 12x12 dış kutu + 1px dikey merkez çizgi
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 6.0, 12.0, 1.0, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy + 5.0, 12.0, 1.0, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 6.0, 1.0, 12.0, icon_color);
                            push_quad(&mut bg_vertices, cx + 5.0, cy - 6.0, 1.0, 12.0, icon_color);
                            push_quad(&mut bg_vertices, cx - 0.5, cy - 6.0, 1.0, 12.0, icon_color);
                        }
                        HeaderAction::SplitVertical => {
                            // Hassas tel kafes bölme ikonu: 12x12 dış kutu + 1px yatay merkez çizgi
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 6.0, 12.0, 1.0, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy + 5.0, 12.0, 1.0, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 6.0, 1.0, 12.0, icon_color);
                            push_quad(&mut bg_vertices, cx + 5.0, cy - 6.0, 1.0, 12.0, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 0.5, 12.0, 1.0, icon_color);
                        }
                        HeaderAction::Find => {
                            // Hassas büyüteç ikonu: 8x8 halka + diyagonal sap
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 6.0, 8.0, 1.2, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy + 1.0, 8.0, 1.2, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 6.0, 1.2, 8.0, icon_color);
                            push_quad(&mut bg_vertices, cx + 1.0, cy - 6.0, 1.2, 8.0, icon_color);
                            push_quad(&mut bg_vertices, cx + 2.0, cy + 2.0, 1.8, 1.8, icon_color);
                            push_quad(&mut bg_vertices, cx + 3.8, cy + 3.8, 1.8, 1.8, icon_color);
                            push_quad(&mut bg_vertices, cx + 5.5, cy + 5.5, 1.8, 1.8, icon_color);
                        }
                        HeaderAction::Menu => {
                            // Üç keskin yatay çizgi menü ikonu
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 4.5, 12.0, 1.5, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy - 0.5, 12.0, 1.5, icon_color);
                            push_quad(&mut bg_vertices, cx - 6.0, cy + 3.5, 12.0, 1.5, icon_color);
                        }
                    }
                }

                // 4. Buton Üzerine Gelindiğinde Görsel İpucu Kartı (Floating Visual Tooltip)
                if palette.is_none() {
                    if let Some(hit) = self.hovered_top_bar {
                        let (tip_text, anchor_x, anchor_w) = match hit {
                            TopBarHit::Action(action) => {
                                let (bx, _, bw, _) = action.rect(screen_w);
                                (action.tooltip(), bx, bw)
                            }
                            TopBarHit::NewTab => {
                                ("Yeni Sekme (Ctrl+Shift+T)", px, pw)
                            }
                            TopBarHit::TabClose(idx) => {
                                let tx = idx as f32 * tab_w;
                                let cx = tx + tab_w - 24.0;
                                ("Sekmeyi Kapat (Ctrl+Shift+W)", cx, 18.0)
                            }
                            TopBarHit::Tab(_) => ("", 0.0, 0.0),
                        };

                        if !tip_text.is_empty() {
                            let approx_char_w = (self.cell_width * 0.72).max(6.0);
                            let tip_text_w = tip_text.chars().count() as f32 * approx_char_w;
                            let tip_w = tip_text_w + 20.0;
                            let tip_h = 24.0;
                            let tip_y = bar_height + 4.0;
                            let mut tip_x = (anchor_x + anchor_w / 2.0) - (tip_w / 2.0);
                            tip_x = tip_x.clamp(8.0, (screen_w - tip_w - 8.0).max(8.0));

                            // Tooltip kart gölgesi / arka planı
                            let tip_bg = [15.0 / 255.0, 16.0 / 255.0, 24.0 / 255.0, 0.98];
                            let tip_border = [55.0 / 255.0, 65.0 / 255.0, 95.0 / 255.0, 0.95];
                            push_quad(&mut bg_vertices, tip_x, tip_y, tip_w, tip_h, tip_bg);
                            push_quad(&mut bg_vertices, tip_x, tip_y, tip_w, 1.0, tip_border);
                            push_quad(&mut bg_vertices, tip_x, tip_y + tip_h - 1.0, tip_w, 1.0, tip_border);
                            push_quad(&mut bg_vertices, tip_x, tip_y, 1.0, tip_h, tip_border);
                            push_quad(&mut bg_vertices, tip_x + tip_w - 1.0, tip_y, 1.0, tip_h, tip_border);
                            // Kartın üstüne 2px tema vurgu çizgisi
                            push_quad(&mut bg_vertices, tip_x, tip_y, tip_w, 2.0, self.theme_cursor_color);

                            self.tooltip_buffer.set_text(
                                &mut self.font_system,
                                tip_text,
                                glyphon::Attrs::new()
                                    .family(glyphon::Family::Name(&self.font_family))
                                    .weight(glyphon::Weight::MEDIUM)
                                    .color(glyphon::Color::rgb(240, 242, 252)),
                                glyphon::Shaping::Advanced,
                            );
                            self.tooltip_buffer.shape_until_scroll(&mut self.font_system, false);

                            tooltip_overlay = Some((tip_x, tip_y, tip_w, tip_h));
                        }
                    }
                }
            }
        }

        // 5. Komut Paleti (Command Palette Modal - Ctrl+Shift+P)
        let mut palette_overlay = None;
        if let Some(pal) = palette {
            let screen_w = self.config.width as f32;
            let screen_h = self.config.height as f32;

            // 1. Karartma arkaplanı (Dim Backdrop Veil)
            push_quad(&mut bg_vertices, 0.0, 0.0, screen_w, screen_h, [0.0, 0.0, 0.0, 0.62]);

            let pw = 540.0_f32.min(screen_w - 24.0);
            let ph = 350.0_f32.min(screen_h - 60.0);
            let px = ((screen_w - pw) / 2.0).round();
            let py = 50.0;

            // 2. Gölge / derinlik kenarlığı
            push_quad(&mut bg_vertices, px - 2.0, py - 2.0, pw + 4.0, ph + 4.0, [8.0 / 255.0, 10.0 / 255.0, 16.0 / 255.0, 0.85]);

            // 3. Palet kart gövdesi
            push_quad(&mut bg_vertices, px, py, pw, ph, [16.0 / 255.0, 18.0 / 255.0, 26.0 / 255.0, 0.98]);

            // 4. Kenarlıklar
            let card_border = [50.0 / 255.0, 58.0 / 255.0, 82.0 / 255.0, 0.95];
            push_quad(&mut bg_vertices, px, py, pw, 1.0, card_border);
            push_quad(&mut bg_vertices, px, py + ph - 1.0, pw, 1.0, card_border);
            push_quad(&mut bg_vertices, px, py, 1.0, ph, card_border);
            push_quad(&mut bg_vertices, px + pw - 1.0, py, 1.0, ph, card_border);

            // 5. Üst 2.5px parlak tema vurgu çizgisi
            push_quad(&mut bg_vertices, px, py, pw, 2.5, self.theme_cursor_color);

            // 6. Arama girdi alanı
            let search_h = 38.0;
            push_quad(&mut bg_vertices, px + 1.0, py + 2.5, pw - 2.0, search_h, [12.0 / 255.0, 13.0 / 255.0, 19.0 / 255.0, 1.0]);
            push_quad(&mut bg_vertices, px, py + search_h + 2.5, pw, 1.0, card_border);

            // Arama kutusundaki imleç
            let query_w = (pal.query.chars().count() as f32 + 2.0) * self.cell_width * 0.9;
            let cursor_x = px + 16.0 + query_w;
            let cursor_y = py + 12.0;
            let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
            if (now_ms / 500).is_multiple_of(2) {
                push_quad(&mut bg_vertices, cursor_x, cursor_y, 2.0, 18.0, self.theme_cursor_color);
            }

            // 7. Liste Öğeleri
            let item_h = 32.0;
            let max_visible = 7;
            let items = pal.items;
            let total = items.len();
            let selected = if total > 0 { pal.selected_index.min(total - 1) } else { 0 };
            let start_idx = if total <= max_visible {
                0
            } else {
                selected.saturating_sub(max_visible / 2).min(total - max_visible)
            };
            let end_idx = (start_idx + max_visible).min(total);

            for (visible_i, idx) in (start_idx..end_idx).enumerate() {
                let iy = py + search_h + 8.0 + visible_i as f32 * item_h;
                if idx == selected {
                    push_quad(&mut bg_vertices, px + 6.0, iy, pw - 12.0, item_h - 2.0, [34.0 / 255.0, 42.0 / 255.0, 68.0 / 255.0, 0.9]);
                    push_quad(&mut bg_vertices, px + 6.0, iy, 3.0, item_h - 2.0, self.theme_cursor_color);
                }
            }

            // 8. Alt Bilgi Çubuğu (Footer)
            let footer_y = py + ph - 28.0;
            push_quad(&mut bg_vertices, px, footer_y, pw, 1.0, card_border);
            push_quad(&mut bg_vertices, px + 1.0, footer_y + 1.0, pw - 2.0, 27.0, [12.0 / 255.0, 13.0 / 255.0, 19.0 / 255.0, 1.0]);

            // 9. Metinlerin formatlanması (Spans for palette_buffer)
            let mut pal_spans: Vec<(String, glyphon::Attrs)> = Vec::new();

            // Arama satırı
            pal_spans.push(("> ".to_string(), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)).weight(glyphon::Weight::BOLD).color(glyphon::Color::rgb(122, 162, 247))));
            if pal.query.is_empty() {
                pal_spans.push(("Komut arayın... (örn: sekme, böl, arama)".to_string(), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)).style(glyphon::Style::Italic).color(glyphon::Color::rgb(100, 105, 125))));
            } else {
                pal_spans.push((pal.query.to_string(), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)).weight(glyphon::Weight::BOLD).color(glyphon::Color::rgb(245, 246, 252))));
            }
            pal_spans.push(("\n\n".to_string(), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family))));

            // Liste öğeleri
            let approx_char_w = (self.cell_width * 0.9).max(7.0);
            let available_chars = ((pw - 52.0) / approx_char_w).floor() as usize;

            if total == 0 {
                pal_spans.push(("  Eşleşen komut bulunamadı".to_string(), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)).style(glyphon::Style::Italic).color(glyphon::Color::rgb(130, 135, 155))));
                pal_spans.push(("\n".to_string(), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family))));
            } else {
                for idx in start_idx..end_idx {
                    let item = &items[idx];
                    let is_sel = idx == selected;

                    let title_fg = if is_sel {
                        glyphon::Color::rgb(255, 255, 255)
                    } else {
                        glyphon::Color::rgb(205, 210, 225)
                    };
                    let sc_fg = if is_sel {
                        glyphon::Color::rgb(122, 162, 247)
                    } else {
                        glyphon::Color::rgb(110, 115, 140)
                    };

                    let title_len = item.title.chars().count();
                    let sc_len = item.shortcut.chars().count();
                    let spaces = available_chars.saturating_sub(title_len + sc_len + 4);
                    let space_str = " ".repeat(spaces.max(2));

                    let prefix = if is_sel { "▶ " } else { "  " };
                    pal_spans.push((format!("{}{}", prefix, item.title), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)).weight(if is_sel { glyphon::Weight::BOLD } else { glyphon::Weight::NORMAL }).color(title_fg)));
                    pal_spans.push((space_str, glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family))));
                    pal_spans.push((format!("{}\n", item.shortcut), glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)).weight(glyphon::Weight::MEDIUM).color(sc_fg)));
                }
            }

            let span_refs: Vec<(&str, glyphon::Attrs)> = pal_spans.iter().map(|(t, a)| (t.as_str(), *a)).collect();
            self.palette_buffer.set_rich_text(
                &mut self.font_system,
                span_refs,
                glyphon::Attrs::new().family(glyphon::Family::Name(&self.font_family)).color(glyphon::Color::rgb(200, 205, 220)),
                glyphon::Shaping::Advanced,
            );
            self.palette_buffer.shape_until_scroll(&mut self.font_system, false);

            palette_overlay = Some((px, py, pw, ph, total, selected));
        }

        // Arka plan vertex buffer'ına yaz
        if !bg_vertices.is_empty() {
            if bg_vertices.len() > self.bg_vertex_capacity {
                self.bg_vertex_capacity = bg_vertices.len() * 2;
                self.bg_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Background Vertex Buffer (Expanded)"),
                    size: (self.bg_vertex_capacity * std::mem::size_of::<BackgroundVertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(
                &self.bg_vertex_buffer,
                0,
                bytemuck::cast_slice(&bg_vertices),
            );
        }

        let mut text_areas = Vec::new();
        for (p_idx, px, py, pw, ph) in pane_text_areas {
            text_areas.push(TextArea {
                buffer: &self.pane_buffers[p_idx],
                left: px,
                top: py,
                scale: 1.0,
                bounds: TextBounds {
                    left: px as i32,
                    top: py as i32,
                    right: (px + pw) as i32,
                    bottom: (py + ph) as i32,
                },
                default_color: default_fg,
                custom_glyphs: &[],
            });
        }

        if has_header {
            if let Some(tab_list) = tabs {
                let (tab_w, _, bar_height, _) =
                    get_top_bar_layout(self.config.width as f32, tab_list.len());
                for (idx, tab) in tab_list.iter().enumerate() {
                    let tx = idx as f32 * tab_w;
                    text_areas.push(TextArea {
                        buffer: &self.tab_buffers[idx],
                        left: tx + 20.0,
                        top: 6.0,
                        scale: 1.0,
                        bounds: TextBounds {
                            left: (tx + 18.0) as i32,
                            top: 0,
                            right: (tx + tab_w - 26.0) as i32,
                            bottom: bar_height as i32,
                        },
                        default_color: if tab.is_active {
                            glyphon::Color::rgb(245, 246, 252)
                        } else {
                            glyphon::Color::rgb(130, 135, 155)
                        },
                        custom_glyphs: &[],
                    });
                }
            }
        }

        if let Some((bx, by, bw, bh)) = search_overlay {
            text_areas.push(TextArea {
                buffer: &self.search_buffer,
                left: bx + 8.0,
                top: by + 4.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: bx as i32,
                    top: by as i32,
                    right: (bx + bw) as i32,
                    bottom: (by + bh) as i32,
                },
                default_color: glyphon::Color::rgb(226, 228, 234),
                custom_glyphs: &[],
            });
        }

        if let Some((tx, ty, tw, th)) = tooltip_overlay {
            text_areas.push(TextArea {
                buffer: &self.tooltip_buffer,
                left: tx + 10.0,
                top: ty + 4.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: tx as i32,
                    top: ty as i32,
                    right: (tx + tw) as i32,
                    bottom: (ty + th + 4.0) as i32,
                },
                default_color: glyphon::Color::rgb(240, 242, 252),
                custom_glyphs: &[],
            });
        }

        if let Some((px, py, pw, ph, total, selected)) = palette_overlay {
            // Ana palet listesi
            text_areas.push(TextArea {
                buffer: &self.palette_buffer,
                left: px + 14.0,
                top: py + 10.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: px as i32,
                    top: py as i32,
                    right: (px + pw) as i32,
                    bottom: (py + ph - 28.0) as i32,
                },
                default_color: glyphon::Color::rgb(200, 205, 220),
                custom_glyphs: &[],
            });

            // Alt bilgi metni (palette_footer_buffer kullanılarak)
            let footer_str = format!("  ↑↓ Gezin  •  ↵ Çalıştır  •  Esc Kapat            [ {} / {} ]", if total > 0 { selected + 1 } else { 0 }, total);
            self.palette_footer_buffer.set_text(
                &mut self.font_system,
                &footer_str,
                glyphon::Attrs::new()
                    .family(glyphon::Family::Name(&self.font_family))
                    .color(glyphon::Color::rgb(130, 135, 155)),
                glyphon::Shaping::Advanced,
            );
            self.palette_footer_buffer.shape_until_scroll(&mut self.font_system, false);

            text_areas.push(TextArea {
                buffer: &self.palette_footer_buffer,
                left: px + 8.0,
                top: py + ph - 23.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: px as i32,
                    top: (py + ph - 28.0) as i32,
                    right: (px + pw) as i32,
                    bottom: (py + ph) as i32,
                },
                default_color: glyphon::Color::rgb(130, 135, 155),
                custom_glyphs: &[],
            });
        }

        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.text_atlas,
                &self.viewport,
                text_areas,
                &mut self.swash_cache,
            )
            .map_err(|e| anyhow::anyhow!("Glyphon prepare error: {:?}", e))?;

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Terminal Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.theme_bg.0 as f64 / 255.0,
                            g: self.theme_bg.1 as f64 / 255.0,
                            b: self.theme_bg.2 as f64 / 255.0,
                            a: self.theme_opacity.clamp(0.05, 1.0) as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Adım 1: Hücre ve İmleç Arka Plan Dikdörtgenlerini Çiz
            if !bg_vertices.is_empty() {
                pass.set_pipeline(&self.bg_pipeline);
                pass.set_bind_group(0, &self.bg_bind_group, &[]);
                pass.set_vertex_buffer(0, self.bg_vertex_buffer.slice(..));
                pass.draw(0..bg_vertices.len() as u32, 0..1);
            }

            // Adım 1.5: Sixel Görsellerini Çiz
            for pane in panes {
                let pane_x = padding_left + pane.col_offset as f32 * self.cell_width;
                let pane_y = padding_top + pane.row_offset as f32 * self.cell_height;

                for img in &pane.grid.images {
                    if img.width == 0 || img.height == 0 || img.rgba.is_empty() {
                        continue;
                    }
                    let img_x = pane_x + img.col as f32 * self.cell_width;
                    let img_y = pane_y + img.row as f32 * self.cell_height;
                    let img_w = img.width as f32;
                    let img_h = img.height as f32;

                    let texture_size = wgpu::Extent3d {
                        width: img.width as u32,
                        height: img.height as u32,
                        depth_or_array_layers: 1,
                    };
                    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Sixel Texture"),
                        size: texture_size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &img.rgba,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * img.width as u32),
                            rows_per_image: Some(img.height as u32),
                        },
                        texture_size,
                    );

                    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Sixel Bind Group"),
                        layout: &self.image_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.image_sampler),
                            },
                        ],
                    });

                    let v0 = ImageVertex { position: [img_x, img_y], tex_coords: [0.0, 0.0] };
                    let v1 = ImageVertex { position: [img_x + img_w, img_y], tex_coords: [1.0, 0.0] };
                    let v2 = ImageVertex { position: [img_x, img_y + img_h], tex_coords: [0.0, 1.0] };
                    let v3 = ImageVertex { position: [img_x + img_w, img_y + img_h], tex_coords: [1.0, 1.0] };
                    let quad_vertices = [v0, v2, v1, v1, v2, v3];

                    self.queue.write_buffer(
                        &self.image_vertex_buffer,
                        0,
                        bytemuck::cast_slice(&quad_vertices),
                    );

                    pass.set_pipeline(&self.image_pipeline);
                    pass.set_bind_group(0, &self.bg_bind_group, &[]);
                    pass.set_bind_group(1, &bind_group, &[]);
                    pass.set_vertex_buffer(0, self.image_vertex_buffer.slice(..));
                    pass.draw(0..6, 0..1);
                }
            }

            // Adım 2: Metin Gliflerini Arka Planın Üzerine Çiz
            self.text_renderer
                .render(&self.text_atlas, &self.viewport, &mut pass)?;
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        self.text_atlas.trim();

        Ok(())
    }

    pub fn render(&mut self, grid: &Grid, tabs: Option<&[TabInfo]>) -> anyhow::Result<()> {
        self.render_panes(
            &[PaneDrawInfo {
                id: 0,
                grid,
                col_offset: 0,
                row_offset: 0,
                cols: grid.cols,
                rows: grid.rows,
                is_active: true,
                ghost_hint: None,
            }],
            tabs,
            None,
        )
    }
}

fn push_quad(
    vertices: &mut Vec<BackgroundVertex>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
) {
    let p0 = [x, y];
    let p1 = [x + w, y];
    let p2 = [x, y + h];
    let p3 = [x + w, y + h];

    // İki üçgenden oluşan bir dikdörtgen:
    // Üçgen 1: p0 -> p1 -> p2
    vertices.push(BackgroundVertex { position: p0, color });
    vertices.push(BackgroundVertex { position: p1, color });
    vertices.push(BackgroundVertex { position: p2, color });

    // Üçgen 2: p1 -> p3 -> p2
    vertices.push(BackgroundVertex { position: p1, color });
    vertices.push(BackgroundVertex { position: p3, color });
    vertices.push(BackgroundVertex { position: p2, color });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_bar_layout_and_positions() {
        let win_w = 960.0;
        let (tab_w, tabs_total_w, bar_height, (px, py, pw, ph)) = get_top_bar_layout(win_w, 2);

        assert_eq!(bar_height, 30.0);
        assert_eq!(tab_w, 240.0);
        assert_eq!(tabs_total_w, 480.0);
        assert_eq!(px, 484.0);
        assert_eq!(py, 4.0);
        assert_eq!(pw, 26.0);
        assert_eq!(ph, 22.0);

        // Aksiyon butonlarının sıralaması
        let mut prev_right = 0.0;
        for action in HeaderAction::ALL {
            let (x, y, w, h) = action.rect(win_w);
            assert_eq!(y, 4.0);
            assert_eq!(w, 28.0);
            assert_eq!(h, 22.0);
            assert!(x >= prev_right, "Buttons must be ordered from left to right");
            assert!(x + w <= win_w, "Button must fit within window width");
            prev_right = x + w;
        }
    }

    #[test]
    fn test_top_bar_hit_testing() {
        let win_w = 800.0;

        // 1. Sekmeler ve sekme kapatma (✕) testi
        let hit_tab0 = get_top_bar_hit(50.0, 15.0, win_w, 2);
        assert_eq!(hit_tab0, Some(TopBarHit::Tab(0)));

        let (tab_w, _, _, (px, py, pw, ph)) = get_top_bar_layout(win_w, 2);
        let close0_x = tab_w - 15.0;
        let hit_close0 = get_top_bar_hit(close0_x, 15.0, win_w, 2);
        assert_eq!(hit_close0, Some(TopBarHit::TabClose(0)));

        // 2. '+' Yeni sekme butonu testi
        let hit_plus = get_top_bar_hit(px + pw / 2.0, py + ph / 2.0, win_w, 2);
        assert_eq!(hit_plus, Some(TopBarHit::NewTab));

        // 3. Sağdaki aksiyon butonları testi
        for action in HeaderAction::ALL {
            let (x, y, w, h) = action.rect(win_w);
            let hit = get_top_bar_hit(x + w / 2.0, y + h / 2.0, win_w, 2);
            assert_eq!(hit, Some(TopBarHit::Action(action)));
        }

        // 4. Dışarı tıklama testleri
        assert_eq!(get_top_bar_hit(win_w - 50.0, 35.0, win_w, 2), None); // Terminal gövdesi
        assert_eq!(get_top_bar_hit(win_w - 50.0, 0.0, win_w, 2), None); // Üst kenar sınır
        assert_eq!(get_top_bar_hit(win_w + 10.0, 15.0, win_w, 2), None); // Pencere dışı sağ
    }

    #[test]
    fn test_header_action_tooltips() {
        for action in HeaderAction::ALL {
            let tooltip = action.tooltip();
            assert!(!tooltip.is_empty());
        }
    }

    #[test]
    fn test_pane_cached_state_validation() {
        let cached = PaneCachedState {
            pane_id: 1,
            version: 42,
            cols: 80,
            rows: 24,
            scroll_offset: 0,
            cell_bg_vertices: Vec::new(),
        };

        assert_eq!(cached.pane_id, 1);
        assert_eq!(cached.version, 42);
        assert_eq!(cached.cols, 80);
        assert_eq!(cached.rows, 24);
        assert_eq!(cached.scroll_offset, 0);
    }
}
