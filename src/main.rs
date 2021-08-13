#[macro_use]
extern crate log;

use crate::{
    buffer::{BufferWrapper, Encodable},
    gpu_view::GPUView,
    uniforms::Uniforms,
    util::copy_region,
    view::View,
};
use core::num::NonZeroU32;
use image::{ImageBuffer, Rgba};
use naga::{
    back, front,
    valid::{ValidationFlags, Validator},
};
use std::{
    borrow::Cow,
    convert::TryFrom,
    mem::size_of,
    num::NonZeroU64,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{fs::File, io::AsyncWriteExt, task};
use wgpu::{
    BackendBit, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferAddress,
    BufferBinding, BufferBindingType, BufferDescriptor, BufferUsage, Color, ColorTargetState,
    ColorWrite, CommandEncoderDescriptor, Device, Extent3d, Face, FragmentState, FrontFace,
    ImageCopyBuffer, ImageCopyTexture, ImageDataLayout, Instance, LoadOp, Maintain, MapMode,
    MultisampleState, Operations, Origin3d, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderFlags, ShaderModuleDescriptor, ShaderSource, ShaderStage, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsage, TextureView, VertexState,
};

mod buffer;
mod gpu_view;
mod uniforms;
mod util;
mod view;

const TEMPLATE_SOURCE: &str = include_str!("template.wgsl");

const IMAGE_WIDTH: u32 = 907;
const IMAGE_HEIGHT: u32 = 907;

const TEXTURE_WIDTH: u32 = util::smallest_multiple_containing(
    IMAGE_WIDTH,
    wgpu::COPY_BYTES_PER_ROW_ALIGNMENT / size_of::<u32>() as u32,
);
const TEXTURE_HEIGHT: u32 = IMAGE_HEIGHT;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    info!("Creating View...");
    let view = View::new_centered_uniform(IMAGE_WIDTH as usize, IMAGE_HEIGHT as usize, 3.0);

    info!("Creating Instance...");
    let instance = Instance::new(BackendBit::PRIMARY);
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: Default::default(),
            compatible_surface: None,
        })
        .await
        .unwrap();

    info!("Requesting device...");
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();

    info!("Creating device poll task...");
    let device = Arc::new(device);
    let poll_device = device.clone();
    let status = Arc::new(AtomicBool::new(true));
    let poll_status = status.clone();
    let poll_task = tokio::spawn(async move {
        while poll_status.load(Ordering::Relaxed) {
            poll_device.poll(Maintain::Poll);
            task::yield_now().await;
        }
    });

    info!("Creating framebuffer...");
    let (texture, texture_view) = create_texture(&device, TEXTURE_WIDTH, TEXTURE_HEIGHT);
    let buffer = create_texture_buffer(&device, TEXTURE_WIDTH, TEXTURE_HEIGHT);

    info!("Creating shader module...");
    let shader = load_shaders().await;
    let module = device.create_shader_module(&ShaderModuleDescriptor {
        label: Some("Vertex Shader"),
        source: shader,
        flags: ShaderFlags::VALIDATION | ShaderFlags::EXPERIMENTAL_TRANSLATION,
    });

    info!("Creating uniforms...");
    let uniforms = Uniforms {
        view: GPUView::from_view(view),
    };
    let (uniforms_buffer, uniforms_cb) =
        BufferWrapper::from_data(&device, &[uniforms], BufferUsage::UNIFORM);
    queue.submit([uniforms_cb]);

    let uniform_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Uniforms bind group layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStage::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(NonZeroU64::new(Uniforms::size() as u64).unwrap()),
            },
            count: None,
        }],
    });

    let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Uniforms bind group"),
        layout: &uniform_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: uniforms_buffer.buffer(),
                offset: 0,
                size: None,
            }),
        }],
    });

    info!("Creating render pipeline...");
    let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&uniform_bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: VertexState {
            module: &module,
            entry_point: "vert_main",
            buffers: &[],
        },
        fragment: Some(FragmentState {
            module: &module,
            entry_point: "frag_main",
            targets: &[ColorTargetState {
                format: TextureFormat::Rgba8Unorm,
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrite::ALL,
            }],
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            clamp_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
    });

    info!("Encoding command buffer...");
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Command Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&render_pipeline);
        render_pass.set_bind_group(0, &uniform_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }

    encoder.copy_texture_to_buffer(
        ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
        },
        ImageCopyBuffer {
            buffer: &buffer,
            layout: ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(
                    NonZeroU32::try_from(size_of::<u32>() as u32 * TEXTURE_WIDTH).unwrap(),
                ),
                rows_per_image: Some(NonZeroU32::try_from(TEXTURE_HEIGHT).unwrap()),
            },
        },
        Extent3d {
            width: TEXTURE_WIDTH,
            height: TEXTURE_HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    info!("Submitting command buffer...");
    queue.submit(Some(encoder.finish()));

    {
        info!("Reading framebuffer...");
        let buffer_slice = buffer.slice(..);
        buffer_slice.map_async(MapMode::Read).await.unwrap();

        let data = buffer_slice.get_mapped_range();

        info!("Copying image...");
        let mut image_data =
            vec![0u8; IMAGE_WIDTH as usize * IMAGE_HEIGHT as usize * size_of::<u32>()];
        copy_region(
            data.as_ref(),
            TEXTURE_WIDTH as usize,
            0,
            0,
            &mut image_data,
            IMAGE_WIDTH as usize,
            0,
            0,
            IMAGE_WIDTH as usize,
            IMAGE_HEIGHT as usize,
        );

        info!("Writing image...");
        let image =
            ImageBuffer::<Rgba<u8>, _>::from_raw(IMAGE_WIDTH, IMAGE_HEIGHT, image_data).unwrap();
        image.save("output.png").unwrap();
    }
    buffer.unmap();

    info!("Shutting down...");

    status.store(false, Ordering::Relaxed);
    poll_task.await.unwrap();

    info!("Done.");
}

fn create_texture(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Framebuffer"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsage::COPY_SRC | TextureUsage::RENDER_ATTACHMENT,
    });
    let texture_view = texture.create_view(&Default::default());

    (texture, texture_view)
}

fn create_texture_buffer(device: &Device, width: u32, height: u32) -> Buffer {
    let size = width * height * size_of::<u32>() as u32;
    let texture_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Framebuffer Buffer"),
        size: size as BufferAddress,
        usage: BufferUsage::COPY_DST | BufferUsage::MAP_READ,
        mapped_at_creation: false,
    });

    texture_buffer
}

async fn load_shaders() -> ShaderSource<'static> {
    info!("Loading utility functions...");
    let module = front::wgsl::parse_str(TEMPLATE_SOURCE).unwrap();

    info!("Validating module...");
    let mut validator = Validator::new(ValidationFlags::all(), Default::default());
    let module_info = validator.validate(&module).unwrap();

    info!("Writing module as txt...");
    let mut file = File::create("debug.txt").await.unwrap();
    file.write_all(format!("{:#?}", &module).as_bytes())
        .await
        .unwrap();

    info!("Compiling WGSL...");
    let mut wgsl_str = String::new();
    let mut writer = back::wgsl::Writer::new(&mut wgsl_str);
    writer.write(&module, &module_info).unwrap();
    writer.finish();

    info!("Writing WGSL...");
    let mut wgsl_file = File::create("debug.wgsl").await.unwrap();
    wgsl_file.write_all(wgsl_str.as_bytes()).await.unwrap();

    ShaderSource::Wgsl(Cow::Owned(wgsl_str))
}
