/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

/*! This is a player library for WebGPU traces.
 *
 * # Notes
 * - we call device_maintain_ids() before creating any refcounted resource,
 *   which is basically everything except for BGL and shader modules,
 *   so that we don't accidentally try to use the same ID.
!*/

use wgc::device::trace;

use std::{ffi::CString, fmt::Debug, fs, marker::PhantomData, path::Path, ptr};

#[macro_export]
macro_rules! gfx_select {
    ($id:expr => $global:ident.$method:ident( $($param:expr),+ )) => {
        match $id.backend() {
            #[cfg(not(any(target_os = "ios", target_os = "macos")))]
            wgt::Backend::Vulkan => $global.$method::<wgc::backend::Vulkan>( $($param),+ ),
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            wgt::Backend::Metal => $global.$method::<wgc::backend::Metal>( $($param),+ ),
            #[cfg(windows)]
            wgt::Backend::Dx12 => $global.$method::<wgc::backend::Dx12>( $($param),+ ),
            #[cfg(windows)]
            wgt::Backend::Dx11 => $global.$method::<wgc::backend::Dx11>( $($param),+ ),
            _ => unreachable!()
        }
    };
}

struct Label(Option<CString>);
impl Label {
    fn new(text: &str) -> Self {
        Self(if text.is_empty() {
            None
        } else {
            Some(CString::new(text).expect("invalid label"))
        })
    }

    fn as_ptr(&self) -> *const std::os::raw::c_char {
        match self.0 {
            Some(ref c_string) => c_string.as_ptr(),
            None => ptr::null(),
        }
    }
}

#[derive(Debug)]
pub struct IdentityPassThrough<I>(PhantomData<I>);

impl<I: Clone + Debug + wgc::id::TypedId> wgc::hub::IdentityHandler<I> for IdentityPassThrough<I> {
    type Input = I;
    fn process(&self, id: I, backend: wgt::Backend) -> I {
        let (index, epoch, _backend) = id.unzip();
        I::zip(index, epoch, backend)
    }
    fn free(&self, _id: I) {}
}

pub struct IdentityPassThroughFactory;

impl<I: Clone + Debug + wgc::id::TypedId> wgc::hub::IdentityHandlerFactory<I>
    for IdentityPassThroughFactory
{
    type Filter = IdentityPassThrough<I>;
    fn spawn(&self, _min_index: u32) -> Self::Filter {
        IdentityPassThrough(PhantomData)
    }
}
impl wgc::hub::GlobalIdentityHandlerFactory for IdentityPassThroughFactory {}

pub trait GlobalPlay {
    fn encode_commands<B: wgc::hub::GfxBackend>(
        &self,
        encoder: wgc::id::CommandEncoderId,
        commands: Vec<trace::Command>,
    ) -> wgc::id::CommandBufferId;
    fn process<B: wgc::hub::GfxBackend>(
        &self,
        device: wgc::id::DeviceId,
        action: trace::Action,
        dir: &Path,
        comb_manager: &mut wgc::hub::IdentityManager,
    );
}

impl GlobalPlay for wgc::hub::Global<IdentityPassThroughFactory> {
    fn encode_commands<B: wgc::hub::GfxBackend>(
        &self,
        encoder: wgc::id::CommandEncoderId,
        commands: Vec<trace::Command>,
    ) -> wgc::id::CommandBufferId {
        for command in commands {
            match command {
                trace::Command::CopyBufferToBuffer {
                    src,
                    src_offset,
                    dst,
                    dst_offset,
                    size,
                } => self
                    .command_encoder_copy_buffer_to_buffer::<B>(
                        encoder, src, src_offset, dst, dst_offset, size,
                    )
                    .unwrap(),
                trace::Command::CopyBufferToTexture { src, dst, size } => self
                    .command_encoder_copy_buffer_to_texture::<B>(encoder, &src, &dst, &size)
                    .unwrap(),
                trace::Command::CopyTextureToBuffer { src, dst, size } => self
                    .command_encoder_copy_texture_to_buffer::<B>(encoder, &src, &dst, &size)
                    .unwrap(),
                trace::Command::CopyTextureToTexture { src, dst, size } => self
                    .command_encoder_copy_texture_to_texture::<B>(encoder, &src, &dst, &size)
                    .unwrap(),
                trace::Command::RunComputePass { base } => {
                    self.command_encoder_run_compute_pass_impl::<B>(encoder, base.as_ref())
                        .unwrap();
                }
                trace::Command::RunRenderPass {
                    base,
                    target_colors,
                    target_depth_stencil,
                } => {
                    self.command_encoder_run_render_pass_impl::<B>(
                        encoder,
                        base.as_ref(),
                        &target_colors,
                        target_depth_stencil.as_ref(),
                    )
                    .unwrap();
                }
            }
        }
        self.command_encoder_finish::<B>(encoder, &wgt::CommandBufferDescriptor { todo: 0 })
            .unwrap()
    }

    fn process<B: wgc::hub::GfxBackend>(
        &self,
        device: wgc::id::DeviceId,
        action: trace::Action,
        dir: &Path,
        comb_manager: &mut wgc::hub::IdentityManager,
    ) {
        use wgc::device::trace::Action as A;
        match action {
            A::Init { .. } => panic!("Unexpected Action::Init: has to be the first action only"),
            A::CreateSwapChain { .. } | A::PresentSwapChain(_) => {
                panic!("Unexpected SwapChain action: winit feature is not enabled")
            }
            A::CreateBuffer { id, desc } => {
                let label = Label::new(&desc.label);
                self.device_maintain_ids::<B>(device);
                self.device_create_buffer::<B>(device, &desc.map_label(|_| label.as_ptr()), id);
            }
            A::DestroyBuffer(id) => {
                self.buffer_destroy::<B>(id);
            }
            A::CreateTexture { id, desc } => {
                let label = Label::new(&desc.label);
                self.device_maintain_ids::<B>(device);
                self.device_create_texture::<B>(device, &desc.map_label(|_| label.as_ptr()), id);
            }
            A::DestroyTexture(id) => {
                self.texture_destroy::<B>(id);
            }
            A::CreateTextureView {
                id,
                parent_id,
                desc,
            } => {
                let label = desc.as_ref().map_or(Label(None), |d| Label::new(&d.label));
                self.device_maintain_ids::<B>(device);
                self.texture_create_view::<B>(
                    parent_id,
                    desc.map(|d| d.map_label(|_| label.as_ptr())).as_ref(),
                    id,
                );
            }
            A::DestroyTextureView(id) => {
                self.texture_view_destroy::<B>(id);
            }
            A::CreateSampler { id, desc } => {
                let label = Label::new(&desc.label);
                self.device_maintain_ids::<B>(device);
                self.device_create_sampler::<B>(device, &desc.map_label(|_| label.as_ptr()), id);
            }
            A::DestroySampler(id) => {
                self.sampler_destroy::<B>(id);
            }
            A::GetSwapChainTexture { id, parent_id } => {
                if let Some(id) = id {
                    self.swap_chain_get_current_texture_view::<B>(parent_id, id)
                        .unwrap()
                        .view_id
                        .unwrap();
                }
            }
            A::CreateBindGroupLayout {
                id,
                ref label,
                ref entries,
            } => {
                self.device_create_bind_group_layout::<B>(
                    device,
                    &wgt::BindGroupLayoutDescriptor {
                        label: Some(label),
                        entries,
                    },
                    id,
                )
                .unwrap();
            }
            A::DestroyBindGroupLayout(id) => {
                self.bind_group_layout_destroy::<B>(id);
            }
            A::CreatePipelineLayout {
                id,
                bind_group_layouts,
                push_constant_ranges,
            } => {
                self.device_maintain_ids::<B>(device);
                self.device_create_pipeline_layout::<B>(
                    device,
                    &wgt::PipelineLayoutDescriptor {
                        bind_group_layouts: &bind_group_layouts,
                        push_constant_ranges: &push_constant_ranges,
                    },
                    id,
                )
                .unwrap();
            }
            A::DestroyPipelineLayout(id) => {
                self.pipeline_layout_destroy::<B>(id);
            }
            A::CreateBindGroup {
                id,
                label,
                layout_id,
                entries,
            } => {
                use wgc::binding_model as bm;
                let entry_vec = entries
                    .iter()
                    .map(|(binding, res)| wgc::binding_model::BindGroupEntry {
                        binding: *binding,
                        resource: match *res {
                            trace::BindingResource::Buffer { id, offset, size } => {
                                bm::BindingResource::Buffer(bm::BufferBinding {
                                    buffer_id: id,
                                    offset,
                                    size,
                                })
                            }
                            trace::BindingResource::Sampler(id) => bm::BindingResource::Sampler(id),
                            trace::BindingResource::TextureView(id) => {
                                bm::BindingResource::TextureView(id)
                            }
                            trace::BindingResource::TextureViewArray(ref binding_array) => {
                                bm::BindingResource::TextureViewArray(binding_array)
                            }
                        },
                    })
                    .collect::<Vec<_>>();
                self.device_maintain_ids::<B>(device);
                self.device_create_bind_group::<B>(
                    device,
                    &wgc::binding_model::BindGroupDescriptor {
                        label: Some(&label),
                        layout: layout_id,
                        entries: &entry_vec,
                    },
                    id,
                )
                .unwrap();
            }
            A::DestroyBindGroup(id) => {
                self.bind_group_destroy::<B>(id);
            }
            A::CreateShaderModule { id, data } => {
                let byte_vec = fs::read(dir.join(data)).unwrap();
                let spv = byte_vec
                    .chunks(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect::<Vec<_>>();
                self.device_create_shader_module::<B>(
                    device,
                    wgc::pipeline::ShaderModuleSource::SpirV(&spv),
                    id,
                );
            }
            A::DestroyShaderModule(id) => {
                self.shader_module_destroy::<B>(id);
            }
            A::CreateComputePipeline { id, desc } => {
                let compute_stage = desc.compute_stage.to_core();
                self.device_maintain_ids::<B>(device);
                self.device_create_compute_pipeline::<B>(
                    device,
                    &wgc::pipeline::ComputePipelineDescriptor {
                        layout: desc.layout,
                        compute_stage,
                    },
                    id,
                )
                .unwrap();
            }
            A::DestroyComputePipeline(id) => {
                self.compute_pipeline_destroy::<B>(id);
            }
            A::CreateRenderPipeline { id, desc } => {
                let vertex_stage = desc.vertex_stage.to_core();
                let fragment_stage = desc.fragment_stage.as_ref().map(|fs| fs.to_core());
                let vertex_buffers = desc
                    .vertex_state
                    .vertex_buffers
                    .iter()
                    .map(|vb| wgt::VertexBufferDescriptor {
                        stride: vb.stride,
                        step_mode: vb.step_mode,
                        attributes: &vb.attributes,
                    })
                    .collect::<Vec<_>>();
                self.device_maintain_ids::<B>(device);
                self.device_create_render_pipeline::<B>(
                    device,
                    &wgc::pipeline::RenderPipelineDescriptor {
                        layout: desc.layout,
                        vertex_stage,
                        fragment_stage,
                        primitive_topology: desc.primitive_topology,
                        rasterization_state: desc.rasterization_state,
                        color_states: &desc.color_states,
                        depth_stencil_state: desc.depth_stencil_state,
                        vertex_state: wgt::VertexStateDescriptor {
                            index_format: desc.vertex_state.index_format,
                            vertex_buffers: &vertex_buffers,
                        },
                        sample_count: desc.sample_count,
                        sample_mask: desc.sample_mask,
                        alpha_to_coverage_enabled: desc.alpha_to_coverage_enabled,
                    },
                    id,
                )
                .unwrap();
            }
            A::DestroyRenderPipeline(id) => {
                self.render_pipeline_destroy::<B>(id);
            }
            A::CreateRenderBundle { id, desc, base } => {
                let label = Label::new(&desc.label);
                let bundle = wgc::command::RenderBundleEncoder::new(
                    &wgt::RenderBundleEncoderDescriptor {
                        label: None,
                        color_formats: &desc.color_formats,
                        depth_stencil_format: desc.depth_stencil_format,
                        sample_count: desc.sample_count,
                    },
                    device,
                    Some(base),
                )
                .unwrap();
                self.render_bundle_encoder_finish::<B>(
                    bundle,
                    &wgt::RenderBundleDescriptor {
                        label: label.as_ptr(),
                    },
                    id,
                )
                .unwrap();
            }
            A::DestroyRenderBundle(id) => {
                self.render_bundle_destroy::<B>(id);
            }
            A::CreateQuerySet {
                id,
                desc,
            } => {
                let type_ = match &desc.type_ {
                    trace::QueryType::Occlusion =>
                        wgt::QueryType::Occlusion,
                    trace::QueryType::PipelineStatistics(pipeline_statistics) =>
                        wgt::QueryType::PipelineStatistics(&pipeline_statistics),
                    trace::QueryType::Timestamp =>
                        wgt::QueryType::Timestamp,
                };

                self.device_create_query_set::<B>(
                    device,
                    &wgt::QuerySetDescriptor {
                        type_,
                        count: desc.count,
                    },
                    id,
                );
            }
            A::DestroyQuerySet(id) => {
                self.query_set_destroy::<B>(id);
            }
            A::WriteBuffer {
                id,
                data,
                range,
                queued,
            } => {
                let bin = std::fs::read(dir.join(data)).unwrap();
                let size = (range.end - range.start) as usize;
                if queued {
                    self.queue_write_buffer::<B>(device, id, range.start, &bin);
                } else {
                    self.device_wait_for_buffer::<B>(device, id).unwrap();
                    self.device_set_buffer_sub_data::<B>(device, id, range.start, &bin[..size]);
                }
            }
            A::WriteTexture {
                to,
                data,
                layout,
                size,
            } => {
                let bin = std::fs::read(dir.join(data)).unwrap();
                self.queue_write_texture::<B>(device, &to, &bin, &layout, &size);
            }
            A::Submit(_index, commands) => {
                let encoder = self.device_create_command_encoder::<B>(
                    device,
                    &wgt::CommandEncoderDescriptor { label: ptr::null() },
                    comb_manager.alloc(device.backend()),
                );
                let comb = self.encode_commands::<B>(encoder, commands);
                self.queue_submit::<B>(device, &[comb]).unwrap();
            }
        }
    }
}
