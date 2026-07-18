// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure command coalescing state for the asynchronous WebKit bridge.

#[derive(Clone, Copy)]
pub(super) enum CommandSlot {
    Config,
    Document,
    Frame,
}

#[derive(Default)]
struct CommandBatch {
    config: Option<String>,
    document: Option<String>,
    frame: Option<String>,
}

impl CommandBatch {
    fn enqueue(&mut self, slot: CommandSlot, script: String) {
        match slot {
            CommandSlot::Config => self.config = Some(script),
            CommandSlot::Document => self.document = Some(script),
            CommandSlot::Frame => self.frame = Some(script),
        }
    }

    fn script(&self) -> Option<String> {
        let scripts = [
            self.config.as_deref(),
            self.document.as_deref(),
            self.frame.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        (!scripts.is_empty()).then(|| scripts.join("\n"))
    }

    fn restore_behind(self, newer: &mut Self) {
        if newer.config.is_none() {
            newer.config = self.config;
        }
        if newer.document.is_none() {
            newer.document = self.document;
        }
        if newer.frame.is_none() {
            newer.frame = self.frame;
        }
    }
}

#[derive(Default)]
pub(super) struct BridgeState {
    ready: bool,
    pending: CommandBatch,
    in_flight: Option<CommandBatch>,
}

impl BridgeState {
    pub(super) fn set_ready(&mut self, ready: bool) {
        if !ready {
            self.restore_in_flight();
        }
        self.ready = ready;
    }

    pub(super) fn enqueue(&mut self, slot: CommandSlot, script: String) {
        self.pending.enqueue(slot, script);
    }

    pub(super) fn take_pending(&mut self) -> Option<String> {
        if !self.ready || self.in_flight.is_some() {
            return None;
        }
        let batch = std::mem::take(&mut self.pending);
        let script = batch.script()?;
        self.in_flight = Some(batch);
        Some(script)
    }

    pub(super) fn complete_dispatch(&mut self, succeeded: bool) {
        if succeeded {
            self.in_flight = None;
        } else {
            self.restore_in_flight();
        }
    }

    fn restore_in_flight(&mut self) {
        if let Some(batch) = self.in_flight.take() {
            batch.restore_behind(&mut self.pending);
        }
    }
}

#[cfg(test)]
#[path = "../../../test/web_lyrics_bridge_test.rs"]
mod tests;
