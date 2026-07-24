// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure command coalescing state for the asynchronous WebKit bridge.

#[derive(Clone, Copy)]
pub(super) enum CommandSlot {
    Config,
    Document,
    Frame {
        seeking: bool,
        document_revision: Option<u64>,
    },
}

struct FrameCommand {
    script: String,
    seeking: bool,
    document_revision: Option<u64>,
}

#[derive(Default)]
struct CommandBatch {
    config: Option<String>,
    document: Option<String>,
    frame: Option<FrameCommand>,
}

impl CommandBatch {
    fn enqueue(&mut self, slot: CommandSlot, script: String) {
        match slot {
            CommandSlot::Config => self.config = Some(script),
            CommandSlot::Document => self.document = Some(script),
            CommandSlot::Frame {
                seeking,
                document_revision,
            } => {
                if self.frame.as_ref().is_some_and(|pending| {
                    pending.seeking && !seeking && pending.document_revision == document_revision
                }) {
                    return;
                }
                self.frame = Some(FrameCommand {
                    script,
                    seeking,
                    document_revision,
                });
            }
        }
    }

    fn script(&self) -> Option<String> {
        let scripts = [
            self.config.as_deref(),
            self.document.as_deref(),
            self.frame.as_ref().map(|frame| frame.script.as_str()),
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
        if let Some(frame) = self.frame
            && (newer.frame.is_none()
                || (frame.seeking
                    && newer.frame.as_ref().is_some_and(|pending| {
                        !pending.seeking && pending.document_revision == frame.document_revision
                    })))
        {
            newer.frame = Some(frame);
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
