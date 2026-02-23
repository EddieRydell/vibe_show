use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;
use crate::model::{
    BlendMode, ColorGradient, Curve, EffectInstance, EffectKind, EffectParams, EffectTarget,
    ParamKey, ParamValue, Sequence, TimeRange,
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
        blend_mode: BlendMode,
        opacity: f64,
    },
    DeleteEffects {
        sequence_index: usize,
        targets: Vec<(usize, usize)>,
    },
    UpdateEffectParam {
        sequence_index: usize,
        track_index: usize,
        effect_index: usize,
        key: ParamKey,
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
    },
    DeleteTrack {
        sequence_index: usize,
        track_index: usize,
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
    SetScript {
        sequence_index: usize,
        name: String,
        source: String,
    },
    DeleteScript {
        sequence_index: usize,
        name: String,
    },
    RenameScript {
        sequence_index: usize,
        old_name: String,
        new_name: String,
    },
    SetGradient {
        sequence_index: usize,
        name: String,
        gradient: ColorGradient,
    },
    DeleteGradient {
        sequence_index: usize,
        name: String,
    },
    RenameGradient {
        sequence_index: usize,
        old_name: String,
        new_name: String,
    },
    SetCurve {
        sequence_index: usize,
        name: String,
        curve: Curve,
    },
    DeleteCurve {
        sequence_index: usize,
        name: String,
    },
    RenameCurve {
        sequence_index: usize,
        old_name: String,
        new_name: String,
    },
}

impl EditCommand {
    /// Human-readable description for UI tooltips and chat context.
    pub fn description(&self) -> String {
        match self {
            EditCommand::AddEffect { kind, .. } => format!("Add {kind:?} effect"),
            EditCommand::DeleteEffects { targets, .. } => {
                let n = targets.len();
                if n == 1 {
                    "Delete effect".to_string()
                } else {
                    format!("Delete {n} effects")
                }
            }
            EditCommand::UpdateEffectParam { key, .. } => format!("Update {key:?}"),
            EditCommand::UpdateEffectTimeRange { .. } => "Update effect timing".to_string(),
            EditCommand::MoveEffectToTrack { .. } => "Move effect to track".to_string(),
            EditCommand::AddTrack { name, .. } => format!("Add track \"{name}\""),
            EditCommand::DeleteTrack { track_index, .. } => {
                format!("Delete track {track_index}")
            }
            EditCommand::UpdateSequenceSettings { name, .. } => {
                if let Some(n) = name {
                    format!("Rename sequence to \"{n}\"")
                } else {
                    "Update sequence settings".to_string()
                }
            }
            EditCommand::Batch { description, .. } => description.clone(),
            EditCommand::SetScript { name, .. } => format!("Set script \"{name}\""),
            EditCommand::DeleteScript { name, .. } => format!("Delete script \"{name}\""),
            EditCommand::RenameScript { old_name, new_name, .. } => {
                format!("Rename script \"{old_name}\" → \"{new_name}\"")
            }
            EditCommand::SetGradient { name, .. } => format!("Set gradient \"{name}\""),
            EditCommand::DeleteGradient { name, .. } => format!("Delete gradient \"{name}\""),
            EditCommand::RenameGradient { old_name, new_name, .. } => {
                format!("Rename gradient \"{old_name}\" → \"{new_name}\"")
            }
            EditCommand::SetCurve { name, .. } => format!("Set curve \"{name}\""),
            EditCommand::DeleteCurve { name, .. } => format!("Delete curve \"{name}\""),
            EditCommand::RenameCurve { old_name, new_name, .. } => {
                format!("Rename curve \"{old_name}\" → \"{new_name}\"")
            }
        }
    }

    /// Key for coalescing consecutive identical edits into a single undo entry.
    /// When two consecutive commands return the same `Some(key)`, they share one
    /// undo snapshot (the one captured before the first command in the run).
    fn coalesce_key(&self) -> Option<String> {
        match self {
            EditCommand::UpdateEffectParam {
                sequence_index,
                track_index,
                effect_index,
                key,
                ..
            } => Some(format!("param:{sequence_index}:{track_index}:{effect_index}:{key:?}")),
            EditCommand::UpdateEffectTimeRange {
                sequence_index,
                track_index,
                effect_index,
                ..
            } => Some(format!("time:{sequence_index}:{track_index}:{effect_index}")),
            _ => None,
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
            | EditCommand::DeleteTrack { sequence_index, .. }
            | EditCommand::UpdateSequenceSettings { sequence_index, .. }
            | EditCommand::SetScript { sequence_index, .. }
            | EditCommand::DeleteScript { sequence_index, .. }
            | EditCommand::RenameScript { sequence_index, .. }
            | EditCommand::SetGradient { sequence_index, .. }
            | EditCommand::DeleteGradient { sequence_index, .. }
            | EditCommand::RenameGradient { sequence_index, .. }
            | EditCommand::SetCurve { sequence_index, .. }
            | EditCommand::DeleteCurve { sequence_index, .. }
            | EditCommand::RenameCurve { sequence_index, .. } => *sequence_index,
            EditCommand::Batch { commands, .. } => {
                commands.first().map_or(0, EditCommand::sequence_index)
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
    /// When set, consecutive commands with the same coalesce key reuse this
    /// entry's snapshot instead of pushing a new one.
    coalesce_key: Option<String>,
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
    ///
    /// Consecutive commands with the same coalesce key (e.g. repeated param
    /// tweaks on the same effect) share one undo entry so a single Ctrl+Z
    /// reverts the whole run of micro-edits.
    pub fn execute(
        &mut self,
        show: &mut crate::model::Show,
        cmd: &EditCommand,
    ) -> Result<CommandResult, AppError> {
        let seq_idx = cmd.sequence_index();
        let description = cmd.description();
        let new_coalesce_key = cmd.coalesce_key();

        // Check if we can coalesce with the previous undo entry.
        let coalesced = if let Some(ref new_key) = new_coalesce_key {
            self.undo_stack
                .last()
                .and_then(|top| top.coalesce_key.as_ref())
                .is_some_and(|top_key| top_key == new_key)
        } else {
            false
        };

        if !coalesced {
            // Snapshot the sequence before mutation
            let snapshot = show
                .sequences
                .get(seq_idx)
                .ok_or(AppError::InvalidIndex {
                    what: "sequence".into(),
                    index: seq_idx,
                })?
                .clone();

            // Execute the command
            let result = self.apply(show, cmd)?;

            // Push undo entry and clear redo stack
            self.undo_stack.push(UndoEntry {
                description,
                sequence_index: seq_idx,
                snapshot,
                coalesce_key: new_coalesce_key,
            });
            if self.undo_stack.len() > MAX_UNDO_LEVELS {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();

            Ok(result)
        } else {
            // Coalescing: apply the command but keep the existing undo entry's
            // snapshot (which captures the state before the first edit in this run).
            let result = self.apply(show, cmd)?;

            // Update the description to reflect the latest edit.
            if let Some(top) = self.undo_stack.last_mut() {
                top.description = description;
            }

            // Still clear redo — the coalesced edit invalidates any redo history.
            self.redo_stack.clear();

            Ok(result)
        }
    }

    /// Undo the last command. Returns the description of what was undone.
    pub fn undo(&mut self, show: &mut crate::model::Show) -> Result<String, AppError> {
        let entry = self.undo_stack.pop().ok_or(AppError::ValidationError {
            message: "Nothing to undo".into(),
        })?;

        // Snapshot current state for redo
        let current = show
            .sequences
            .get(entry.sequence_index)
            .ok_or(AppError::InvalidIndex {
                what: "sequence".into(),
                index: entry.sequence_index,
            })?
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
            coalesce_key: None,
        });

        Ok(description)
    }

    /// Redo the last undone command. Returns the description of what was redone.
    pub fn redo(&mut self, show: &mut crate::model::Show) -> Result<String, AppError> {
        let entry = self.redo_stack.pop().ok_or(AppError::ValidationError {
            message: "Nothing to redo".into(),
        })?;

        // Snapshot current state for redo
        let current = show
            .sequences
            .get(entry.sequence_index)
            .ok_or(AppError::InvalidIndex {
                what: "sequence".into(),
                index: entry.sequence_index,
            })?
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
            coalesce_key: None,
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
    #[allow(clippy::self_only_used_in_recursion)]
    fn apply(
        &self,
        show: &mut crate::model::Show,
        cmd: &EditCommand,
    ) -> Result<CommandResult, AppError> {
        match cmd {
            EditCommand::AddEffect {
                sequence_index,
                track_index,
                kind,
                start,
                end,
                blend_mode,
                opacity,
            } => {
                let time_range = TimeRange::new(*start, *end).ok_or(AppError::ValidationError {
                    message: format!("Invalid time range: {start}..{end}"),
                })?;
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                let track = sequence
                    .tracks
                    .get_mut(*track_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "track".into(),
                        index: *track_index,
                    })?;
                let effect = EffectInstance {
                    kind: kind.clone(),
                    params: EffectParams::new(),
                    time_range,
                    blend_mode: *blend_mode,
                    opacity: *opacity,
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
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                let mut by_track: std::collections::HashMap<usize, Vec<usize>> =
                    std::collections::HashMap::new();
                for (track_idx, effect_idx) in targets {
                    by_track.entry(*track_idx).or_default().push(*effect_idx);
                }
                for (track_idx, mut effect_indices) in by_track {
                    let track = sequence
                        .tracks
                        .get_mut(track_idx)
                        .ok_or(AppError::InvalidIndex {
                            what: "track".into(),
                            index: track_idx,
                        })?;
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
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                let track = sequence
                    .tracks
                    .get_mut(*track_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "track".into(),
                        index: *track_index,
                    })?;
                let effect = track
                    .effects
                    .get_mut(*effect_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "effect".into(),
                        index: *effect_index,
                    })?;
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
                let time_range = TimeRange::new(*start, *end).ok_or(AppError::ValidationError {
                    message: format!("Invalid time range: {start}..{end}"),
                })?;
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                let track = sequence
                    .tracks
                    .get_mut(*track_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "track".into(),
                        index: *track_index,
                    })?;
                let effect = track
                    .effects
                    .get_mut(*effect_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "effect".into(),
                        index: *effect_index,
                    })?;
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
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                if *from_track >= sequence.tracks.len() {
                    return Err(AppError::InvalidIndex {
                        what: "source track".into(),
                        index: *from_track,
                    });
                }
                if *to_track >= sequence.tracks.len() {
                    return Err(AppError::InvalidIndex {
                        what: "destination track".into(),
                        index: *to_track,
                    });
                }
                let from = sequence.tracks.get(*from_track).ok_or_else(|| {
                    AppError::InvalidIndex {
                        what: "source track".into(),
                        index: *from_track,
                    }
                })?;
                if *effect_index >= from.effects.len() {
                    return Err(AppError::InvalidIndex {
                        what: "effect".into(),
                        index: *effect_index,
                    });
                }
                let effect = sequence.tracks.get_mut(*from_track)
                    .ok_or_else(|| AppError::InvalidIndex { what: "source track".into(), index: *from_track })?
                    .effects.remove(*effect_index);
                let dest_track = sequence.tracks.get_mut(*to_track)
                    .ok_or_else(|| AppError::InvalidIndex { what: "destination track".into(), index: *to_track })?;
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
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                let track = crate::model::Track {
                    name: name.clone(),
                    target: target.clone(),
                    effects: Vec::new(),
                };
                sequence.tracks.push(track);
                Ok(CommandResult::Index(sequence.tracks.len() - 1))
            }

            EditCommand::DeleteTrack {
                sequence_index,
                track_index,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                if *track_index >= sequence.tracks.len() {
                    return Err(AppError::InvalidIndex {
                        what: "track".into(),
                        index: *track_index,
                    });
                }
                sequence.tracks.remove(*track_index);
                Ok(CommandResult::Unit)
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
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                if let Some(n) = name {
                    sequence.name.clone_from(n);
                }
                if let Some(af) = audio_file {
                    sequence.audio_file.clone_from(af);
                }
                if let Some(d) = duration {
                    if *d <= 0.0 {
                        return Err(AppError::ValidationError {
                            message: "Duration must be positive".into(),
                        });
                    }
                    sequence.duration = *d;
                }
                if let Some(fr) = frame_rate {
                    if *fr <= 0.0 {
                        return Err(AppError::ValidationError {
                            message: "Frame rate must be positive".into(),
                        });
                    }
                    sequence.frame_rate = *fr;
                }
                Ok(CommandResult::Unit)
            }

            EditCommand::SetScript {
                sequence_index,
                name,
                source,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                sequence.scripts.insert(name.clone(), source.clone());
                Ok(CommandResult::Unit)
            }

            EditCommand::DeleteScript {
                sequence_index,
                name,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                sequence.scripts.remove(name);
                Ok(CommandResult::Unit)
            }

            EditCommand::RenameScript {
                sequence_index,
                old_name,
                new_name,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                if let Some(source) = sequence.scripts.remove(old_name) {
                    sequence.scripts.insert(new_name.clone(), source);
                    // Update any Script effect kinds that reference the old name
                    for track in &mut sequence.tracks {
                        for effect in &mut track.effects {
                            if effect.kind == EffectKind::Script(old_name.clone()) {
                                effect.kind = EffectKind::Script(new_name.clone());
                            }
                        }
                    }
                }
                Ok(CommandResult::Unit)
            }

            EditCommand::SetGradient {
                sequence_index,
                name,
                gradient,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                sequence.gradient_library.insert(name.clone(), gradient.clone());
                Ok(CommandResult::Unit)
            }

            EditCommand::DeleteGradient {
                sequence_index,
                name,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                sequence.gradient_library.remove(name);
                Ok(CommandResult::Unit)
            }

            EditCommand::RenameGradient {
                sequence_index,
                old_name,
                new_name,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                if let Some(gradient) = sequence.gradient_library.remove(old_name) {
                    sequence.gradient_library.insert(new_name.clone(), gradient);
                    // Update all GradientRef params that referenced the old name
                    for track in &mut sequence.tracks {
                        for effect in &mut track.effects {
                            for val in effect.params.values_mut() {
                                if matches!(val, ParamValue::GradientRef(n) if n == old_name) {
                                    *val = ParamValue::GradientRef(new_name.clone());
                                }
                            }
                        }
                    }
                }
                Ok(CommandResult::Unit)
            }

            EditCommand::SetCurve {
                sequence_index,
                name,
                curve,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                sequence.curve_library.insert(name.clone(), curve.clone());
                Ok(CommandResult::Unit)
            }

            EditCommand::DeleteCurve {
                sequence_index,
                name,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                sequence.curve_library.remove(name);
                Ok(CommandResult::Unit)
            }

            EditCommand::RenameCurve {
                sequence_index,
                old_name,
                new_name,
            } => {
                let sequence = show
                    .sequences
                    .get_mut(*sequence_index)
                    .ok_or(AppError::InvalidIndex {
                        what: "sequence".into(),
                        index: *sequence_index,
                    })?;
                if let Some(curve) = sequence.curve_library.remove(old_name) {
                    sequence.curve_library.insert(new_name.clone(), curve);
                    // Update all CurveRef params that referenced the old name
                    for track in &mut sequence.tracks {
                        for effect in &mut track.effects {
                            for val in effect.params.values_mut() {
                                if matches!(val, ParamValue::CurveRef(n) if n == old_name) {
                                    *val = ParamValue::CurveRef(new_name.clone());
                                }
                            }
                        }
                    }
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
