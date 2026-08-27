// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
};

use anyhow::{Context, Result};
use opentalk_orchestrator_client::{
    client::StateProvider, Metrics, RecorderResource, RegisterRecorder, RegisterType,
    ServiceResource,
};
use opentalk_types_api_internal::recording::RecordingTarget;
use tokio::task::JoinHandle;

use crate::system_info::{CURRENT_LOAD, IS_FEASIBLE};

pub struct OrchestratorStateProvider {
    pub tasks: Arc<Mutex<HashMap<RecordingTarget, JoinHandle<Result<()>>>>>,
}

#[async_trait::async_trait]
impl StateProvider for OrchestratorStateProvider {
    async fn register_type(&mut self) -> RegisterType {
        let tasks = self.tasks.lock().expect("failed to acquire task lock");

        let rooms = tasks
            .iter()
            .map(|(recording, ..)| RecorderResource {
                room_id: recording.room_id,
                breakout_id: recording.breakout_room,
            })
            .collect();

        RegisterType::Recorder(RegisterRecorder { rooms })
    }

    async fn metrics(&mut self) -> Metrics {
        Metrics {
            load: CURRENT_LOAD.load(Ordering::Relaxed),
            accepting_jobs: IS_FEASIBLE.load(Ordering::Relaxed),
        }
    }

    async fn on_resource_collision(&mut self, resources: &[ServiceResource]) -> anyhow::Result<()> {
        let colliding_resources = resources
            .iter()
            .filter_map(|resource| match resource {
                ServiceResource::Recorder(recourder_resource) => Some(*recourder_resource),
                other => {
                    log::error!("Received invalid conflicting resource from orchestrator: {other}");
                    None
                }
            })
            .collect::<Vec<_>>();

        log::warn!("Orchestrator reported resource collision for rooms: {colliding_resources:?}");

        let mut tasks = self.tasks.lock().expect("failed to acquire task lock");

        tasks.retain(|target, handle| {
            let resource = RecorderResource {
                room_id: target.room_id,
                breakout_id: target.breakout_room,
            };

            if colliding_resources.contains(&resource) {
                handle.abort();
                false
            } else {
                true
            }
        });

        Ok(())
    }
}
