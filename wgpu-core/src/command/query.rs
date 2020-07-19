/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use hal;
use hal::command::CommandBuffer;

use crate::{
    device::all_buffer_stages,
    hub::{GfxBackend, Global, GlobalIdentityHandlerFactory, Token},
    id::{BufferId, CommandEncoderId, QuerySetId},
    resource::{BufferUse},
};
use wgt::{
    BufferAddress, BufferUsage
};

pub type QueryId = hal::query::Id;

impl<G: GlobalIdentityHandlerFactory> Global<G> {
    pub fn command_encoder_begin_pipeline_statistics_query<B: GfxBackend>(
        &self,
        command_encoder_id: CommandEncoderId,
        query_set: QuerySetId,
        query_index: u32,
    ) {
        let hub = B::hub(self);
        let mut token = Token::root();

        let (mut cmb_guard, mut token) = hub.command_buffers.write(&mut token);
        let cmb = &mut cmb_guard[command_encoder_id];
        let (query_set_guard, _) = hub.query_sets.read(&mut token);
        let query_set = &query_set_guard[query_set];

        let cmb_raw = cmb.raw.last_mut().unwrap();

        let hal_query = hal::query::Query::<B> {
            pool: &query_set.raw,
            id: query_index,
        };

        unsafe {
            cmb_raw.reset_query_pool(&query_set.raw, query_index..(query_index + 1));
            cmb_raw.begin_query(hal_query, hal::query::ControlFlags::empty());
        }
    }

    pub fn command_encoder_end_pipeline_statistics_query<B: GfxBackend>(
        &self,
        command_encoder_id: CommandEncoderId,
        query_set: QuerySetId,
        query_index: u32,
    ) {
        let hub = B::hub(self);
        let mut token = Token::root();

        let (mut cmb_guard, mut token) = hub.command_buffers.write(&mut token);
        let cmb = &mut cmb_guard[command_encoder_id];
        let (query_set_guard, _) = hub.query_sets.read(&mut token);
        let query_set = &query_set_guard[query_set];

        let cmb_raw = cmb.raw.last_mut().unwrap();

        let hal_query = hal::query::Query::<B> {
            pool: &query_set.raw,
            id: query_index,
        };

        unsafe {
            cmb_raw.end_query(hal_query);
        }
    }

    pub fn command_encoder_write_timestamp<B: GfxBackend>(
        &self,
        command_encoder_id: CommandEncoderId,
        query_set: QuerySetId,
        query_index: u32,
        pipeline_stage: hal::pso::PipelineStage,
    ) {
        let hub = B::hub(self);
        let mut token = Token::root();

        let (mut cmb_guard, mut token) = hub.command_buffers.write(&mut token);
        let cmb = &mut cmb_guard[command_encoder_id];
        let (query_set_guard, _) = hub.query_sets.read(&mut token);
        let query_set = &query_set_guard[query_set];

        let cmb_raw = cmb.raw.last_mut().unwrap();

        let hal_query = hal::query::Query::<B> {
            pool: &query_set.raw,
            id: query_index,
        };

        unsafe {
            cmb_raw.write_timestamp(pipeline_stage, hal_query);
        }
    }

    pub fn command_encoder_resolve_query_set<B: GfxBackend>(
        &self,
        command_encoder_id: CommandEncoderId,
        query_set: QuerySetId,
        first_query: QueryId,
        query_count: u32,
        destination: BufferId,
        destination_offset: BufferAddress,
    ) {
        let hub = B::hub(self);
        let mut token = Token::root();

        let (mut cmb_guard, mut token) = hub.command_buffers.write(&mut token);
        let cmb = &mut cmb_guard[command_encoder_id];
        let (query_set_guard, mut token) = hub.query_sets.read(&mut token);
        let query_set = &query_set_guard[query_set];

        let (buffer_guard, _) = hub.buffers.read(&mut token);

        let (dst_buffer, dst_pending) = cmb.trackers.buffers.use_replace(
            &*buffer_guard,
            destination,
            (),
            BufferUse::COPY_DST,
        );
        assert!(
            dst_buffer.usage.contains(BufferUsage::COPY_DST),
            "Destination buffer usage {:?} must contain usage flag COPY_DST",
            dst_buffer.usage
        );
        let dst_barrier = dst_pending.map(|pending| pending.into_hal(dst_buffer));

        // There needs to be logic here to calculate the stride based on the query type.

        let cmb_raw = cmb.raw.last_mut().unwrap();
        unsafe {
            cmb_raw.pipeline_barrier(
                all_buffer_stages()..hal::pso::PipelineStage::TRANSFER,
                hal::memory::Dependencies::empty(),
                dst_barrier,
            );
            cmb_raw.copy_query_pool_results(
                &query_set.raw,
                first_query..(first_query + query_count),
                &dst_buffer.raw,
                destination_offset,
                16,
                hal::query::ResultFlags::WAIT | hal::query::ResultFlags::WITH_AVAILABILITY | hal::query::ResultFlags::BITS_64,
            );
        }
    }
}
