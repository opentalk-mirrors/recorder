// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
};

use anyhow::Result;
use opentalk_orchestrator_client::{
    client::StateProvider, Metrics, RecorderResource, RegisterRecorder, RegisterType,
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
}
