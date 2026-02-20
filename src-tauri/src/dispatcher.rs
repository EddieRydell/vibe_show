use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{
    BlendMode, EffectInstance, EffectKind, EffectParams, EffectTarget, ParamValue, Sequence,
    TimeRange,
};

/// An undoable editing command. Each variant corresponds to one user action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditCommand {
    AddEffect {
        sequence_index: usize,
        track_index: usize,
        kind: EffectKind,
        start: f64,
        end: f64,
    },
    DeleteEffects {
        sequence_index: usize,
        targets: Vec<(usize, usize)>,
    },
    UpdateEffectParam {
        sequence_index: usize,
        track_index: usize,
        effect_index: usize,
        key: String,
        value: ParamValue,
    },
    UpdateEffectTimeRange {
        sequence_index: usize,
        track_index: usize,
        effect_index: usize,
        start: f64,
        end: f64,
    },
    MoveEffectToTrack {
        sequence_index: usize,
        from_track: usize,
        effect_index: usize,
        to_track: usize,
    },
    AddTrack {
        sequence_index: usize,
        name: String,
        target: EffectTarget,
        blend_mode: BlendMode,
    },
    UpdateSequenceSettings {
        sequence_index: usize,
        name: Option<String>,
        audio_file: Option<Option<String>>,
        duration: Option<f64>,
        frame_rate: Option<f64>,
    },
    Batch {
        description: String,
        commands: Vec<EditCommand>,
    },
}

impl EditCommand {
    /// Human-readable description for UI tooltips and chat context.
    pub fn description(&self) -> String {
        match self {
            EditCommand::AddEffect { kind, .. } => format!("Add {:?} effect", kind),
            EditCommand::DeleteEffects { targets, .. } => {
                let n = targets.len();
                if n == 1 {
                    "Delete effect".to_string()
                } else {
                    format!("Delete {} effects", n)
                }
            }
            EditCommand::UpdateEffectParam { key, .. } => format!("Update {}", key),
            EditCommand::UpdateEffectTimeRange { .. } => "Update effect timing".to_string(),
            EditCommand::MoveEffectToTrack { .. } => "Move effect to track".to_string(),
            EditCommand::AddTrack { name, .. } => format!("Add track \"{}\"", name),
            EditCommand::UpdateSequenceSettings { name, .. } => {
                if let Some(n) = name {
                    format!("Rename sequence to \"{}\"", n)
                } else {
                    "Update sequence settings".to_string()
                }
            }
            EditCommand::Batch { description, .. } => description.clone(),
        }
    }

    /// The sequence index this command operates on.
    fn sequence_index(&self) -> usize {
        match self {
            EditCommand::AddEffect { sequence_index, .. }
            | EditCommand::DeleteEffects { sequence_index, .. }
            | EditCommand::UpdateEffectParam { sequence_index, .. }
            | EditCommand::UpdateEffectTimeRange { sequence_index, .. }
            | EditCommand::MoveEffectToTrack { sequence_index, .. }
            | EditCommand::AddTrack { sequence_index, .. }
            | EditCommand::UpdateSequenceSettings { sequence_index, .. } => *sequence_index,
            EditCommand::Batch { commands, .. } => {
                commands.first().map(|c| c.sequence_index()).unwrap_or(0)
            }
        }
    }
}

/// Result of executing an EditCommand.
#[derive(Debug, Clone, Serialize)]
pub enum CommandResult {
    Index(usize),
    Bool(bool),
    Unit,
}

/// Undo/redo state for the UI.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UndoState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_description: Option<String>,
    pub redo_description: Option<String>,
}

/// An undo entry: the snapshot of the sequence before the command was applied,
/// plus the command description.
struct UndoEntry {
    description: String,
    sequence_index: usize,
    snapshot: Sequence,
}

const MAX_UNDO_LEVELS: usize = 50;

/// Manages command execution with snapshot-based undo/redo.
#[derive(Default)]
pub struct CommandDispatcher {
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
}

impl CommandDispatcher {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Execute an edit command against the show. Returns the command result.
    /// Snapshots the target sequence before mutation for undo.
    pub fn execute(
        &mut self,
        show: &mut crate::model::Show,
        cmd: EditCommand,
    ) -> Result<CommandResult, String> {
        let seq_idx = cmd.sequence_index();
        let description = cmd.description();

        // Snapshot the sequence before mutation
        let snapshot = show
            .sequences
            .get(seq_idx)
            .ok_or("Invalid sequence index")?
            .clone();

        // Execute the command
        let result = self.apply(show, &cmd)?;

        // Push undo entry and clear redo stack
        self.undo_stack.push(UndoEntry {
            description,
            sequence_index: seq_idx,
            snapshot,
        });
        if self.undo_stack.len() > MAX_UNDO_LEVELS {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();

        Ok(result)
    }

    /// Undo the last command. Returns the description of what was undone.
    pub fn undo(&mut self, show: &mut crate::model::Show) -> Result<String, String> {
        let entry = self.undo_stack.pop().ok_or("Nothing to undo")?;

        // Snapshot current state for redo
        let current = show
            .sequences
            .get(entry.sequence_index)
            .ok_or("Invalid sequence index")?
            .clone();

        // Restore the snapshot
        if let Some(seq) = show.sequences.get_mut(entry.sequence_index) {
            *seq = entry.snapshot;
        }

        let description = entry.description.clone();
        self.redo_stack.push(UndoEntry {
            description: entry.description,
            sequence_index: entry.sequence_index,
            snapshot: current,
        });

        Ok(description)
    }

    /// Redo the last undone command. Returns the description of what was redone.
    pub fn redo(&mut self, show: &mut crate::model::Show) -> Result<String, String> {
        let entry = self.redo_stack.pop().ok_or("Nothing to redo")?;

        // Snapshot current state for undo
        let current = show
            .sequences
            .get(entry.sequence_index)
            .ok_or("Invalid sequence index")?
            .clone();

        // Restore the redo snapshot
        if let Some(seq) = show.sequences.get_mut(entry.sequence_index) {
            *seq = entry.snapshot;
        }

        let description = entry.description.clone();
        self.undo_stack.push(UndoEntry {
            description: entry.description,
            sequence_index: entry.sequence_index,
            snapshot: current,
        });

        Ok(description)
    }

    /// Get the current undo/redo state.
    pub fn undo_state(&self) -> UndoState {
        UndoState {
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            undo_description: self.undo_stack.last().map(|e| e.description.clone()),
            redo_description: self.redo_stack.last().map(|e| e.description.clone()),
        }
    }

    /// Clear all undo/redo history (e.g., when switching sequences).
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Apply a command to the show, returning the result. Does not manage undo state.
    fn apply(
        &self,
        show: &mut crate::model::Show,
        cmd: &EditCommand,
    ) -> Result<CommandResult, String> {
        match cmd {
            EditCommand::AddEffect {
                sequence_index,
                track_index,
                kind,
                start,
                end,
            } => {
                let time_range = TimeRange::new(*start, *end).ok_or("Invalid time range")?;
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or("Invalid sequence index")?;
                let track = sequence
                    .tracks
                    .get_mut(*track_index)
                    .ok_or("Invalid track index")?;
                let effect = EffectInstance {
                    kind: kind.clone(),
                    params: EffectParams::new(),
                    time_range,
                };
                // Insert sorted by start time for efficient binary-search evaluation.
                let insert_pos = track.effects.partition_point(|e| {
                    e.time_range.start() < time_range.start()
                });
                track.effects.insert(insert_pos, effect);
                Ok(CommandResult::Index(insert_pos))
            }

            EditCommand::DeleteEffects {
                sequence_index,
                targets,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or("Invalid sequence index")?;
                let mut by_track: std::collections::HashMap<usize, Vec<usize>> =
                    std::collections::HashMap::new();
                for (track_idx, effect_idx) in targets {
                    by_track.entry(*track_idx).or_default().push(*effect_idx);
                }
                for (track_idx, mut effect_indices) in by_track {
                    let track = sequence
                        .tracks
                        .get_mut(track_idx)
                        .ok_or(format!("Invalid track index {track_idx}"))?;
                    effect_indices.sort_unstable();
                    effect_indices.dedup();
                    for &idx in effect_indices.iter().rev() {
                        if idx < track.effects.len() {
                            track.effects.remove(idx);
                        }
                    }
                }
                Ok(CommandResult::Unit)
            }

            EditCommand::UpdateEffectParam {
                sequence_index,
                track_index,
                effect_index,
                key,
                value,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or("Invalid sequence index")?;
                let track = sequence
                    .tracks
                    .get_mut(*track_index)
                    .ok_or("Invalid track index")?;
                let effect = track
                    .effects
                    .get_mut(*effect_index)
                    .ok_or("Invalid effect index")?;
                effect.params.set_mut(key.clone(), value.clone());
                Ok(CommandResult::Bool(true))
            }

            EditCommand::UpdateEffectTimeRange {
                sequence_index,
                track_index,
                effect_index,
                start,
                end,
            } => {
                let time_range = TimeRange::new(*start, *end).ok_or("Invalid time range")?;
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or("Invalid sequence index")?;
                let track = sequence
                    .tracks
                    .get_mut(*track_index)
                    .ok_or("Invalid track index")?;
                let effect = track
                    .effects
                    .get_mut(*effect_index)
                    .ok_or("Invalid effect index")?;
                effect.time_range = time_range;
                // Re-sort to maintain start-time ordering for binary search.
                track.effects.sort_by(|a, b| {
                    a.time_range
                        .start()
                        .partial_cmp(&b.time_range.start())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                Ok(CommandResult::Bool(true))
            }

            EditCommand::MoveEffectToTrack {
                sequence_index,
                from_track,
                effect_index,
                to_track,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or("Invalid sequence index")?;
                if *from_track >= sequence.tracks.len() {
                    return Err(format!("Invalid source track index {from_track}"));
                }
                if *to_track >= sequence.tracks.len() {
                    return Err(format!("Invalid destination track index {to_track}"));
                }
                if *effect_index >= sequence.tracks[*from_track].effects.len() {
                    return Err(format!("Invalid effect index {effect_index}"));
                }
                let effect = sequence.tracks[*from_track].effects.remove(*effect_index);
                let dest_track = &mut sequence.tracks[*to_track];
                let insert_pos = dest_track.effects.partition_point(|e| {
                    e.time_range.start() < effect.time_range.start()
                });
                dest_track.effects.insert(insert_pos, effect);
                Ok(CommandResult::Index(insert_pos))
            }

            EditCommand::AddTrack {
                sequence_index,
                name,
                target,
                blend_mode,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or("Invalid sequence index")?;
                let track = crate::model::Track {
                    name: name.clone(),
                    target: target.clone(),
                    effects: Vec::new(),
                    blend_mode: *blend_mode,
                };
                sequence.tracks.push(track);
                Ok(CommandResult::Index(sequence.tracks.len() - 1))
            }

            EditCommand::UpdateSequenceSettings {
                sequence_index,
                name,
                audio_file,
                duration,
                frame_rate,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or("Invalid sequence index")?;
                if let Some(n) = name {
                    sequence.name = n.clone();
                }
                if let Some(af) = audio_file {
                    sequence.audio_file = af.clone();
                }
                if let Some(d) = duration {
                    if *d <= 0.0 {
                        return Err("Duration must be positive".into());
                    }
                    sequence.duration = *d;
                }
                if let Some(fr) = frame_rate {
                    if *fr <= 0.0 {
                        return Err("Frame rate must be positive".into());
                    }
                    sequence.frame_rate = *fr;
                }
                Ok(CommandResult::Unit)
            }

            EditCommand::Batch { commands, .. } => {
                let mut last_result = CommandResult::Unit;
                for c in commands {
                    last_result = self.apply(show, c)?;
                }
                Ok(last_result)
            }
        }
    }
}
