/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::id;
#[cfg(feature = "trace")]
use std::io::Write as _;
use std::ops::Range;

//TODO: consider a readable Id that doesn't include the backend

type FileName = String;

pub const FILE_NAME: &str = "trace.ron";

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub enum BindingResource {
    Buffer {
        id: id::BufferId,
        offset: wgt::BufferAddress,
        size: Option<wgt::BufferSize>,
    },
    Sampler(id::SamplerId),
    TextureView(id::TextureViewId),
    TextureViewArray(Vec<id::TextureViewId>),
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub struct ProgrammableStageDescriptor {
    pub module: id::ShaderModuleId,
    pub entry_point: String,
}

#[cfg(feature = "trace")]
impl ProgrammableStageDescriptor {
    pub fn new(desc: &crate::pipeline::ProgrammableStageDescriptor) -> Self {
        ProgrammableStageDescriptor {
            module: desc.module,
            entry_point: desc.entry_point.to_string(),
        }
    }
}

#[cfg(feature = "replay")]
impl ProgrammableStageDescriptor {
    pub fn to_core(&self) -> crate::pipeline::ProgrammableStageDescriptor {
        crate::pipeline::ProgrammableStageDescriptor {
            module: self.module,
            entry_point: &self.entry_point,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub struct ComputePipelineDescriptor {
    pub layout: id::PipelineLayoutId,
    pub compute_stage: ProgrammableStageDescriptor,
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub struct VertexBufferDescriptor {
    pub stride: wgt::BufferAddress,
    pub step_mode: wgt::InputStepMode,
    pub attributes: Vec<wgt::VertexAttributeDescriptor>,
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub struct VertexStateDescriptor {
    pub index_format: wgt::IndexFormat,
    pub vertex_buffers: Vec<VertexBufferDescriptor>,
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub struct RenderPipelineDescriptor {
    pub layout: id::PipelineLayoutId,
    pub vertex_stage: ProgrammableStageDescriptor,
    pub fragment_stage: Option<ProgrammableStageDescriptor>,
    pub primitive_topology: wgt::PrimitiveTopology,
    pub rasterization_state: Option<wgt::RasterizationStateDescriptor>,
    pub color_states: Vec<wgt::ColorStateDescriptor>,
    pub depth_stencil_state: Option<wgt::DepthStencilStateDescriptor>,
    pub vertex_state: VertexStateDescriptor,
    pub sample_count: u32,
    pub sample_mask: u32,
    pub alpha_to_coverage_enabled: bool,
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub struct RenderBundleDescriptor {
    pub label: String,
    pub color_formats: Vec<wgt::TextureFormat>,
    pub depth_stencil_format: Option<wgt::TextureFormat>,
    pub sample_count: u32,
}

#[cfg(feature = "trace")]
impl RenderBundleDescriptor {
    pub(crate) fn new(label: super::Label, context: &super::RenderPassContext) -> Self {
        RenderBundleDescriptor {
            label: super::own_label(&label),
            color_formats: context.attachments.colors.to_vec(),
            depth_stencil_format: context.attachments.depth_stencil,
            sample_count: context.sample_count as u32,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub struct QuerySetDescriptor {
    pub type_: QueryType,
    pub count: u32,
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub enum QueryType {
    Occlusion,
    PipelineStatistics(Vec<wgt::PipelineStatisticName>),
    Timestamp,
}

#[cfg(feature = "trace")]
impl QueryType {
    pub(crate) fn new(query_type: &wgt::QueryType) -> Self {
        match query_type {
            wgt::QueryType::Occlusion =>
                Self::Occlusion,
            wgt::QueryType::PipelineStatistics(pipeline_statistic_names) =>
                Self::PipelineStatistics(pipeline_statistic_names.to_vec()),
            wgt::QueryType::Timestamp =>
                Self::Timestamp,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub enum Action {
    Init {
        desc: wgt::DeviceDescriptor,
        backend: wgt::Backend,
    },
    CreateBuffer {
        id: id::BufferId,
        desc: wgt::BufferDescriptor<String>,
    },
    DestroyBuffer(id::BufferId),
    CreateTexture {
        id: id::TextureId,
        desc: wgt::TextureDescriptor<String>,
    },
    DestroyTexture(id::TextureId),
    CreateTextureView {
        id: id::TextureViewId,
        parent_id: id::TextureId,
        desc: Option<wgt::TextureViewDescriptor<String>>,
    },
    DestroyTextureView(id::TextureViewId),
    CreateSampler {
        id: id::SamplerId,
        desc: wgt::SamplerDescriptor<String>,
    },
    DestroySampler(id::SamplerId),
    CreateSwapChain {
        id: id::SwapChainId,
        desc: wgt::SwapChainDescriptor,
    },
    GetSwapChainTexture {
        id: Option<id::TextureViewId>,
        parent_id: id::SwapChainId,
    },
    PresentSwapChain(id::SwapChainId),
    CreateBindGroupLayout {
        id: id::BindGroupLayoutId,
        label: String,
        entries: Vec<wgt::BindGroupLayoutEntry>,
    },
    DestroyBindGroupLayout(id::BindGroupLayoutId),
    CreatePipelineLayout {
        id: id::PipelineLayoutId,
        bind_group_layouts: Vec<id::BindGroupLayoutId>,
        push_constant_ranges: Vec<wgt::PushConstantRange>,
    },
    DestroyPipelineLayout(id::PipelineLayoutId),
    CreateBindGroup {
        id: id::BindGroupId,
        label: String,
        layout_id: id::BindGroupLayoutId,
        entries: std::collections::BTreeMap<u32, BindingResource>,
    },
    DestroyBindGroup(id::BindGroupId),
    CreateShaderModule {
        id: id::ShaderModuleId,
        data: FileName,
    },
    DestroyShaderModule(id::ShaderModuleId),
    CreateComputePipeline {
        id: id::ComputePipelineId,
        desc: ComputePipelineDescriptor,
    },
    DestroyComputePipeline(id::ComputePipelineId),
    CreateRenderPipeline {
        id: id::RenderPipelineId,
        desc: RenderPipelineDescriptor,
    },
    DestroyRenderPipeline(id::RenderPipelineId),
    CreateRenderBundle {
        id: id::RenderBundleId,
        desc: RenderBundleDescriptor,
        base: crate::command::BasePass<crate::command::RenderCommand>,
    },
    DestroyRenderBundle(id::RenderBundleId),
    CreateQuerySet {
        id: id::QuerySetId,
        desc: QuerySetDescriptor,
    },
    DestroyQuerySet(id::QuerySetId),
    WriteBuffer {
        id: id::BufferId,
        data: FileName,
        range: Range<wgt::BufferAddress>,
        queued: bool,
    },
    WriteTexture {
        to: crate::command::TextureCopyView,
        data: FileName,
        layout: wgt::TextureDataLayout,
        size: wgt::Extent3d,
    },
    Submit(crate::SubmissionIndex, Vec<Command>),
}

#[derive(Debug)]
#[cfg_attr(feature = "trace", derive(serde::Serialize))]
#[cfg_attr(feature = "replay", derive(serde::Deserialize))]
pub enum Command {
    CopyBufferToBuffer {
        src: id::BufferId,
        src_offset: wgt::BufferAddress,
        dst: id::BufferId,
        dst_offset: wgt::BufferAddress,
        size: wgt::BufferAddress,
    },
    CopyBufferToTexture {
        src: crate::command::BufferCopyView,
        dst: crate::command::TextureCopyView,
        size: wgt::Extent3d,
    },
    CopyTextureToBuffer {
        src: crate::command::TextureCopyView,
        dst: crate::command::BufferCopyView,
        size: wgt::Extent3d,
    },
    CopyTextureToTexture {
        src: crate::command::TextureCopyView,
        dst: crate::command::TextureCopyView,
        size: wgt::Extent3d,
    },
    RunComputePass {
        base: crate::command::BasePass<crate::command::ComputeCommand>,
    },
    RunRenderPass {
        base: crate::command::BasePass<crate::command::RenderCommand>,
        target_colors: Vec<crate::command::ColorAttachmentDescriptor>,
        target_depth_stencil: Option<crate::command::DepthStencilAttachmentDescriptor>,
    },
}

#[cfg(feature = "trace")]
#[derive(Debug)]
pub struct Trace {
    path: std::path::PathBuf,
    file: std::fs::File,
    config: ron::ser::PrettyConfig,
    binary_id: usize,
}

#[cfg(feature = "trace")]
impl Trace {
    pub fn new(path: &std::path::Path) -> Result<Self, std::io::Error> {
        log::info!("Tracing into '{:?}'", path);
        let mut file = std::fs::File::create(path.join(FILE_NAME))?;
        file.write_all(b"[\n")?;
        Ok(Trace {
            path: path.to_path_buf(),
            file,
            config: ron::ser::PrettyConfig::default(),
            binary_id: 0,
        })
    }

    pub fn make_binary(&mut self, kind: &str, data: &[u8]) -> String {
        self.binary_id += 1;
        let name = format!("data{}.{}", self.binary_id, kind);
        let _ = std::fs::write(self.path.join(&name), data);
        name
    }

    pub(crate) fn add(&mut self, action: Action) {
        match ron::ser::to_string_pretty(&action, self.config.clone()) {
            Ok(string) => {
                let _ = writeln!(self.file, "{},", string);
            }
            Err(e) => {
                log::warn!("RON serialization failure: {:?}", e);
            }
        }
    }
}

#[cfg(feature = "trace")]
impl Drop for Trace {
    fn drop(&mut self) {
        let _ = self.file.write_all(b"]");
    }
}
