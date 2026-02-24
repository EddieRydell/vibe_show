#![allow(clippy::needless_pass_by_value, clippy::cast_precision_loss)]

use std::sync::Arc;

use crate::effects::resolve_effect;
use crate::engine::{self, Frame};
use crate::error::AppError;
use crate::commands::{TickResult, EffectThumbnail, ScriptPreviewData};
use crate::registry::params::{
    GetFrameFilteredParams, GetFrameParams, PreviewScriptFrameParams, PreviewScriptParams,
    RenderEffectThumbnailParams, TickParams,
};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::AppState;

pub fn tick(state: &Arc<AppState>, _p: TickParams) -> Result<CommandOutput, AppError> {
    let mut playback = state.playback.lock();
    if !playback.playing {
        return Ok(CommandOutput::new("Not playing.", CommandResult::Tick(None)));
    }

    let now = std::time::Instant::now();
    let real_dt = match playback.last_tick {
        Some(prev) => now.duration_since(prev).as_secs_f64(),
        None => 0.0,
    };
    playback.last_tick = Some(now);

    let show = state.show.lock();
    let duration = show
        .sequences
        .get(playback.sequence_index)
        .map_or(0.0, |s| s.duration);

    playback.current_time += real_dt;

    let effective_end = playback
        .region
        .map_or(duration, |(_, end)| end.min(duration));

    if playback.current_time >= effective_end {
        if playback.looping {
            let loop_start = playback.region.map_or(0.0, |(s, _)| s);
            playback.current_time = loop_start;
            playback.last_tick = Some(now);
        } else {
            playback.current_time = effective_end;
            playback.playing = false;
            playback.last_tick = None;
        }
    }

    let scripts = state.script_cache.lock();
    let libs = state.global_libraries.lock();
    let frame = engine::evaluate(
        &show,
        playback.sequence_index,
        playback.current_time,
        None,
        Some(&scripts),
        &libs.gradients,
        &libs.curves,
    );
    Ok(CommandOutput::new(
        "Tick.",
        CommandResult::Tick(Some(TickResult {
            frame,
            current_time: playback.current_time,
            playing: playback.playing,
        })),
    ))
}

pub fn get_frame(state: &Arc<AppState>, p: GetFrameParams) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let playback = state.playback.lock();
    let scripts = state.script_cache.lock();
    let libs = state.global_libraries.lock();
    let frame: Frame = engine::evaluate(
        &show,
        playback.sequence_index,
        p.time,
        None,
        Some(&scripts),
        &libs.gradients,
        &libs.curves,
    );
    Ok(CommandOutput::new("Frame.", CommandResult::GetFrame(frame)))
}

pub fn get_frame_filtered(
    state: &Arc<AppState>,
    p: GetFrameFilteredParams,
) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let playback = state.playback.lock();
    let scripts = state.script_cache.lock();
    let libs = state.global_libraries.lock();
    let frame: Frame = engine::evaluate(
        &show,
        playback.sequence_index,
        p.time,
        Some(&p.effects),
        Some(&scripts),
        &libs.gradients,
        &libs.curves,
    );
    Ok(CommandOutput::new(
        "Filtered frame.",
        CommandResult::GetFrameFiltered(frame),
    ))
}

pub fn render_effect_thumbnail(
    state: &Arc<AppState>,
    p: RenderEffectThumbnailParams,
) -> Result<CommandOutput, AppError> {
    let show = state.show.lock();
    let sequence = show.sequences.get(p.sequence_index);
    let result = (|| {
        let sequence = sequence?;
        let track = sequence.tracks.get(p.track_index)?;
        let effect_instance = track.effects.get(p.effect_index)?;
        let effect = resolve_effect(&effect_instance.kind)?;
        let time_range = &effect_instance.time_range;

        let mut pixels = Vec::with_capacity(p.pixel_rows * p.time_samples * 4);

        for row in 0..p.pixel_rows {
            for col in 0..p.time_samples {
                let t = if p.time_samples > 1 {
                    col as f64 / (p.time_samples - 1) as f64
                } else {
                    0.5
                };
                let color = effect.evaluate(t, row, p.pixel_rows, &effect_instance.params);
                pixels.push(color.r);
                pixels.push(color.g);
                pixels.push(color.b);
                pixels.push(255);
            }
        }

        Some(EffectThumbnail {
            width: p.time_samples,
            height: p.pixel_rows,
            pixels,
            start_time: time_range.start(),
            end_time: time_range.end(),
        })
    })();

    Ok(CommandOutput::new(
        "Thumbnail.",
        CommandResult::RenderEffectThumbnail(result),
    ))
}

pub fn preview_script(
    state: &Arc<AppState>,
    p: PreviewScriptParams,
) -> Result<CommandOutput, AppError> {
    let cache = state.script_cache.lock();
    let compiled = cache.get(&p.name).ok_or_else(|| AppError::ApiError {
        message: format!("Script '{}' not found in cache", p.name),
    })?;

    let width = p.time_samples;
    let height = p.pixel_count;
    let mut pixels = vec![0u8; width * height * 4];

    for col in 0..p.time_samples {
        let t = if p.time_samples > 1 {
            col as f64 / (p.time_samples - 1) as f64
        } else {
            0.0
        };
        let mut frame = vec![crate::model::color::Color::default(); p.pixel_count];
        crate::effects::script::evaluate_pixels_batch(
            compiled,
            t,
            0.0,
            &mut frame,
            0,
            p.pixel_count,
            &p.params,
            crate::model::timeline::BlendMode::Override,
            1.0,
            None,
            None,
        );
        for (row, color) in frame.iter().enumerate() {
            let idx = (row * width + col) * 4;
            if let Some(chunk) = pixels.get_mut(idx..idx + 4) {
                chunk.copy_from_slice(&[color.r, color.g, color.b, color.a]);
            }
        }
    }

    Ok(CommandOutput::new(
        "Script preview.",
        CommandResult::PreviewScript(ScriptPreviewData {
            width,
            height,
            pixels,
        }),
    ))
}

pub fn preview_script_frame(
    state: &Arc<AppState>,
    p: PreviewScriptFrameParams,
) -> Result<CommandOutput, AppError> {
    let cache = state.script_cache.lock();
    let compiled = cache.get(&p.name).ok_or_else(|| AppError::ApiError {
        message: format!("Script '{}' not found in cache", p.name),
    })?;

    let mut frame = vec![crate::model::color::Color::default(); p.pixel_count];
    crate::effects::script::evaluate_pixels_batch(
        compiled,
        p.t,
        0.0,
        &mut frame,
        0,
        p.pixel_count,
        &p.params,
        crate::model::timeline::BlendMode::Override,
        1.0,
        None,
        None,
    );

    let result: Vec<[u8; 4]> = frame.iter().map(|c| [c.r, c.g, c.b, c.a]).collect();

    Ok(CommandOutput::new(
        "Script frame.",
        CommandResult::PreviewScriptFrame(result),
    ))
}
