//! Audio pipeline: synthesise each segment of a [`Project`] individually and
//! concatenate them with silent MP3 frames.
//!
//! ## Design
//! Edge TTS does **not** support `<break>` elements inside SSML — any SSML
//! that contains a break tag returns no audio.  Instead, this module:
//!
//! 1. Calls [`crate::tts::TtsProvider::synthesize`] once per text segment
//!    (each call uses the single-`<prosody>` SSML that edge-tts accepts).
//! 2. Inserts silent MP3 frames between segments to reproduce the timing that
//!    was previously baked into SSML `<break>` elements.
//! 3. Concatenates all raw MP3 byte streams — no decoding, no re-encoding,
//!    no C libraries needed.

use crate::model::{Project, VoiceConfig};
use crate::ssml::strip_chinese_parens;
use crate::tts::TtsProvider;
use futures_util::stream::{self, StreamExt};

// ── Silence frames ────────────────────────────────────────────────────────────

/// Generate approximately `ms` milliseconds of silence as raw MP3 bytes.
///
/// The frame format matches the edge-tts output stream:
/// MPEG-2 Layer III, 24 kHz, 48 kbps, mono.
///
/// Frame header bytes: `FF F3 64 C0`
/// - `FF F3`: sync word + MPEG-2, Layer III, no CRC
/// - `64`: 48 kbps bitrate (index 6), 24 kHz sample-rate (index 1), no padding, private=0
/// - `C0`: channel mode = single-channel (mono), mode ext = 0, copyright = 0,
///         original = 1 (bit 1), emphasis = none
///
/// Each frame is 144 bytes and covers exactly 576 samples = 24 ms at 24 kHz.
/// The 140 bytes after the header are all zero, which decodes as silence.
pub(crate) fn silence_mp3(ms: u32) -> Vec<u8> {
    const FRAME_HEADER: [u8; 4] = [0xFF, 0xF3, 0x64, 0xC0];
    const FRAME_SIZE: usize = 144;
    const FRAME_MS: u32 = 24; // 576 samples / 24000 Hz = 24 ms
    // ceiling division, at least 1。div_ceil 避免 `ms + FRAME_MS - 1` 在极大 gap 值下
    // 溢出 u32（debug 构建会 panic）。
    let n = ms.div_ceil(FRAME_MS).max(1);
    let mut out = Vec::with_capacity(n as usize * FRAME_SIZE);
    for _ in 0..n {
        out.extend_from_slice(&FRAME_HEADER);
        out.extend(std::iter::repeat(0u8).take(FRAME_SIZE - 4));
    }
    out
}

// ── Speaker → voice mapping ───────────────────────────────────────────────────

/// 根据 speaker 选择合成声音。
///
/// - `speaker = None` → `vc.en_voice`(旁白/非对话)
/// - `speaker = Some(s)`:按「首次出现顺序」分配:
///   - 第 0 个出现的说话人 → `vc.teacher_voice`
///   - 第 1 个出现的说话人 → `vc.student_voice`
///   - 第 2 个及之后        → `vc.en_voice`
///
/// `seen` 记录已见过的说话人(有序,唯一);调用者在每个 part 开始时传入空 Vec
/// 以保证角色分配在 part 内一致、跨 part 独立。
fn voice_for_speaker<'a>(
    speaker: &Option<String>,
    seen: &mut Vec<String>,
    vc: &'a VoiceConfig,
) -> &'a str {
    match speaker {
        None => &vc.en_voice,
        Some(s) => {
            if !seen.contains(s) {
                seen.push(s.clone());
            }
            match seen.iter().position(|x| x == s) {
                Some(0) => &vc.teacher_voice,
                Some(1) => &vc.student_voice,
                _ => &vc.en_voice,
            }
        }
    }
}

/// 正文嗓音解析：`item.override_voice` 非空白时优先使用（用户在 UI 逐条指定的声音），
/// 否则回退到按 speaker 分配的 `speaker_voice`。
///
/// 注意：调用方仍须先调用 [`voice_for_speaker`] 以维护 `seen` 的出场顺序（teacher /
/// student 角色分配依赖它），本函数只决定最终采用哪个嗓音。
fn resolve_body_voice<'a>(override_voice: &'a Option<String>, speaker_voice: &'a str) -> &'a str {
    override_voice
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(speaker_voice)
}

// ── Segment specification (Phase A output) ──────────────────────────────────────
//
// The pipeline is split into two phases so that synthesis calls can be issued
// concurrently without ever changing the order in which their bytes are spliced
// into the output:
//
// * Phase A (`build_segments`) is a pure, synchronous walk of the [`Project`]
//   that decides EVERYTHING about ordering, voice selection, silence frames,
//   skips and gaps — exactly as the old sequential loop did — and records the
//   decisions as a flat list of [`Op`]s. Each `Op::Synth` carries an `idx`
//   "synth-ordinal" that increments ONLY on synth ops (silences do not consume
//   an index), giving an unambiguous result slot and error-ordering key.
// * Phase B (`execute_segments`) is the only async part: it runs the synth ops
//   through `buffered(N)` (bounded concurrency), then reassembles strictly by
//   op order. Concatenation order is a pure function of the op vector and is
//   independent of which network future finishes first — so the output is
//   byte-for-byte identical to the sequential implementation, and at N = 1
//   (Edge) `buffered` serializes dispatch too.

/// Which logical segment a [`SynthSpec`] corresponds to — used only to
/// reconstruct the contextual error message verbatim on failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegKind {
    ZhInstruction,
    Label,
    Number,
    Body,
    Repeat,
}

impl SegKind {
    /// Reproduce the exact error templates from the original sequential loop.
    /// `part` / `item` are 1-based.
    fn render(&self, part: usize, item: usize, e: &str) -> String {
        match self {
            SegKind::ZhInstruction => format!("zh TTS part {part} (zh instruction): {e}"),
            SegKind::Label => format!("en TTS part {part} (label): {e}"),
            SegKind::Number => format!("en TTS part {part} item {item} (number): {e}"),
            SegKind::Body => format!("en TTS part {part} item {item} (text): {e}"),
            SegKind::Repeat => format!("en TTS part {part} item {item} (repeat): {e}"),
        }
    }
}

/// A single synthesis call to issue, fully self-contained (owned text/voice so
/// the op vector has no borrows and lifetimes stay trivial).
struct SynthSpec {
    /// Synth-ordinal: increments ONLY on synth ops; indexes the results vec and
    /// is the error-ordering key (smallest idx = earliest in original order).
    idx: usize,
    text: String,
    voice: String,
    rate: i32,
    pitch: i32,
    volume: u32,
    kind: SegKind,
    /// 1-based part / item labels, carried for error-template reconstruction.
    part_label: usize,
    item_label: usize,
}

/// One position in the flat output stream: either a precomputed silence buffer
/// or a synthesis call to be filled in by Phase B.
enum Op {
    Silence(Vec<u8>),
    Synth(SynthSpec),
}

/// Where each output part lives in the flat `ops` vector.
struct PartLayout {
    filename: String,
    range: std::ops::Range<usize>,
}

// ── Phase A: build the ordered op list (pure, synchronous, no await) ────────────

/// Walk the [`Project`] and produce the flat list of [`Op`]s plus per-part
/// layout ranges. This is a mechanical, line-for-line translation of the old
/// sequential loop: every guard, the per-part `seen_speakers` reset, the
/// number-before-skip ordering, the skip-before-`voice_for_speaker` ordering
/// (so a skipped item does NOT advance `seen_speakers`), override resolution,
/// `rate - 5` for numbers, the `repeat >= 2` second reading, every level of
/// gap silence, the disabled-item skip (emits nothing), and filename generation
/// are preserved exactly. The only change is that synthesis results are
/// deferred: instead of awaiting and appending bytes, we push an `Op::Synth`.
fn build_segments(project: &Project) -> (Vec<Op>, Vec<PartLayout>) {
    let vc = &project.voice_config;
    let mut ops: Vec<Op> = Vec::new();
    let mut layouts: Vec<PartLayout> = Vec::new();
    let mut next_idx: usize = 0; // synth-ordinal; increments ONLY on Op::Synth

    for (idx, part) in project.parts.iter().enumerate() {
        let part_label = idx + 1; // 1-based for error messages
        let part_start = ops.len();

        // ── Optional Chinese instruction ──────────────────────────────────
        if part.read_zh_instruction {
            if let Some(ref zh) = part.zh_instruction {
                if zh.chars().any(|c| c.is_alphanumeric()) {
                    ops.push(Op::Synth(SynthSpec {
                        idx: next_idx,
                        text: zh.clone(),
                        voice: vc.zh_voice.clone(),
                        rate: vc.rate,
                        pitch: vc.pitch,
                        volume: vc.volume,
                        kind: SegKind::ZhInstruction,
                        part_label,
                        item_label: 0,
                    }));
                    next_idx += 1;
                    ops.push(Op::Silence(silence_mp3(1000)));
                }
            }
        }

        // ── Part label (English portion) ──────────────────────────────────
        if part.read_label {
            let en_label = strip_chinese_parens(&part.label);
            if en_label.chars().any(|c| c.is_ascii_alphanumeric()) {
                ops.push(Op::Synth(SynthSpec {
                    idx: next_idx,
                    text: en_label,
                    voice: vc.en_voice.clone(),
                    rate: vc.rate,
                    pitch: vc.pitch,
                    volume: vc.volume,
                    kind: SegKind::Label,
                    part_label,
                    item_label: 0,
                }));
                next_idx += 1;
                ops.push(Op::Silence(silence_mp3(2000)));
            }
        }

        // ── Items ─────────────────────────────────────────────────────────
        // Per-part speaker order; reset for each part so A/B assignments are
        // consistent within a part but independent across parts.
        let mut seen_speakers: Vec<String> = Vec::new();

        for (item_idx, item) in part.items.iter().enumerate() {
            if !item.enabled {
                continue;
            }

            let item_label = item_idx + 1; // 1-based for error messages

            // Number label (read at base_rate − 5) — always narrator (en_voice).
            if item.read_number {
                if let Some(n) = item.number {
                    let num_text = format!("Number {n}.");
                    let num_rate = (vc.rate - 5).max(-100);
                    ops.push(Op::Synth(SynthSpec {
                        idx: next_idx,
                        text: num_text,
                        voice: vc.en_voice.clone(),
                        rate: num_rate,
                        pitch: vc.pitch,
                        volume: vc.volume,
                        kind: SegKind::Number,
                        part_label,
                        item_label,
                    }));
                    next_idx += 1;
                    ops.push(Op::Silence(silence_mp3(600)));
                }
            }

            // 正文用英文嗓子(en_voice)合成：若不含任何 ASCII 字母 / 数字
            //（纯中文 / 纯标点，如"词组："这类分类小标题），英文嗓子读不出、
            // edge-tts 会返回空音频，故跳过以免整段合成失败。
            // 注意:此 continue 在 voice_for_speaker 之前,被跳过的 item 不推进
            // seen_speakers(与原顺序实现一致)。
            if !item.text.chars().any(|c| c.is_ascii_alphanumeric()) {
                ops.push(Op::Silence(silence_mp3(item.gap_after_ms)));
                continue;
            }

            // Body text: per-item override_voice 优先，否则按 speaker 分配。
            // voice_for_speaker 仍需调用以维护 seen_speakers 的出场顺序。
            let speaker_voice = voice_for_speaker(&item.speaker, &mut seen_speakers, vc);
            let body_voice = resolve_body_voice(&item.override_voice, speaker_voice).to_owned();

            // Body text (first reading)
            ops.push(Op::Synth(SynthSpec {
                idx: next_idx,
                text: item.text.clone(),
                voice: body_voice.clone(),
                rate: vc.rate,
                pitch: vc.pitch,
                volume: vc.volume,
                kind: SegKind::Body,
                part_label,
                item_label,
            }));
            next_idx += 1;

            // Optional second reading (repeat ≥ 2) — same voice as first reading.
            if item.repeat >= 2 {
                ops.push(Op::Silence(silence_mp3(1500)));
                ops.push(Op::Synth(SynthSpec {
                    idx: next_idx,
                    text: item.text.clone(),
                    voice: body_voice,
                    rate: vc.rate,
                    pitch: vc.pitch,
                    volume: vc.volume,
                    kind: SegKind::Repeat,
                    part_label,
                    item_label,
                }));
                next_idx += 1;
            }

            // Per-item trailing gap
            ops.push(Op::Silence(silence_mp3(item.gap_after_ms)));
        }

        // ── Part trailing gap ─────────────────────────────────────────────
        ops.push(Op::Silence(silence_mp3(part.gap_after_ms)));

        // ── File name ─────────────────────────────────────────────────────
        let filename = format!("{:02}_{}.mp3", idx + 1, sanitize_part_name(part));
        layouts.push(PartLayout {
            filename,
            range: part_start..ops.len(),
        });
    }

    (ops, layouts)
}

// ── Phase B: run synth ops concurrently and reassemble by index (async) ─────────

/// Run every `Op::Synth` through the provider with bounded concurrency
/// (`provider.max_concurrency()`), then reassemble the per-part and full MP3
/// buffers strictly in op order.
///
/// `buffered(n)` yields results in INPUT order (= synth-ordinal ascending) and
/// never spawns — it drives the futures on the current task, so the borrowed
/// `&dyn TtsProvider` and the borrowed `SynthSpec`s need no `Send`/`'static`.
/// At `n == 1` (Edge) dispatch is fully serial, matching the sequential
/// pipeline's timing as well as its bytes.
///
/// On any synthesis error we return the error of the EARLIEST synth-ordinal
/// (the first segment in original order). We collect all results first and scan
/// ascending rather than short-circuiting, so the "earliest in original order"
/// guarantee is explicit and does not depend on combinator internals. (On
/// error, `buffered` drops the remaining in-flight futures — harmless
/// cancellation for Azure REST POSTs; cannot occur for Edge at n = 1.)
/// Issue a single synthesis call for one [`SynthSpec`], returning its
/// synth-ordinal alongside the result (bytes on success, contextualised error
/// message on failure). Written as a named `async fn` with explicit lifetimes
/// to keep `buffered`'s future type inference well-behaved under the
/// `#[tauri::command]` wrapper.
async fn run_one<'a>(
    provider: &'a dyn TtsProvider,
    s: &'a SynthSpec,
) -> (usize, Result<Vec<u8>, String>) {
    let r = provider
        .synthesize(&s.text, &s.voice, s.rate, s.pitch, s.volume)
        .await
        .map_err(|e| s.kind.render(s.part_label, s.item_label, &e));
    (s.idx, r)
}

async fn execute_segments(
    ops: Vec<Op>,
    layouts: Vec<PartLayout>,
    provider: &dyn TtsProvider,
) -> Result<(Vec<u8>, Vec<(String, Vec<u8>)>), String> {
    let n = provider.max_concurrency().max(1);
    let synth_count = ops
        .iter()
        .filter(|o| matches!(o, Op::Synth(_)))
        .count();

    // Build one boxed future per synth op via a named `async fn` (`run_one`).
    // Boxing each future into a `Pin<Box<dyn Future + 'a>>` with an explicit,
    // single lifetime — rather than streaming an iterator of borrowing `async`
    // closures — avoids a higher-ranked-lifetime ("FnOnce not general enough")
    // inference failure when this future is embedded in the `#[tauri::command]`
    // wrapper. The borrowed `&dyn TtsProvider` lives across every `.await`; no
    // future is spawned, so no `Send`/`'static` is required.
    type Job<'a> = std::pin::Pin<
        Box<dyn std::future::Future<Output = (usize, Result<Vec<u8>, String>)> + Send + 'a>,
    >;
    let jobs: Vec<Job> = ops
        .iter()
        .filter_map(|o| if let Op::Synth(s) = o { Some(s) } else { None })
        .map(|s| Box::pin(run_one(provider, s)) as Job)
        .collect();

    // buffered(n): results arrive in input (== idx ascending) order; collect ALL
    // so we can pick the earliest-ordinal error explicitly.
    let collected: Vec<(usize, Result<Vec<u8>, String>)> =
        stream::iter(jobs).buffered(n).collect().await;

    // Scatter into idx-indexed slots.
    let mut results: Vec<Option<Result<Vec<u8>, String>>> =
        (0..synth_count).map(|_| None).collect();
    for (idx, r) in collected {
        results[idx] = Some(r);
    }

    // Error aggregation: ascending scan → first Err = smallest idx = earliest in
    // original order.
    for slot in &results {
        if let Some(Err(msg)) = slot {
            return Err(msg.clone());
        }
    }

    // No error → reassemble strictly by op order, sliced per part.
    let mut parts_out: Vec<(String, Vec<u8>)> = Vec::with_capacity(layouts.len());
    for layout in &layouts {
        let mut part_mp3: Vec<u8> = Vec::new();
        for op in &ops[layout.range.clone()] {
            match op {
                Op::Silence(b) => part_mp3.extend_from_slice(b),
                Op::Synth(s) => part_mp3.extend_from_slice(
                    results[s.idx]
                        .as_ref()
                        .expect("every synth slot filled")
                        .as_ref()
                        .expect("no error reached reassembly"),
                ),
            }
        }
        parts_out.push((layout.filename.clone(), part_mp3));
    }

    // Full MP3 = all parts concatenated in order.
    let total: usize = parts_out.iter().map(|(_, bytes)| bytes.len()).sum();
    let mut full_mp3: Vec<u8> = Vec::with_capacity(total);
    for (_, bytes) in &parts_out {
        full_mp3.extend_from_slice(bytes);
    }

    Ok((full_mp3, parts_out))
}

// ── Main pipeline ─────────────────────────────────────────────────────────────

/// Generate full-project and per-part MP3 audio.
///
/// Returns `(full_mp3, [(filename, part_mp3), …])` where the filenames are
/// `"01_PartLabel.mp3"`, `"02_PartLabel.mp3"`, … (zero-padded two digits).
///
/// Each segment is synthesised independently via
/// [`TtsProvider::synthesize`] (which wraps the text in a single
/// `<prosody>` element — the only SSML form edge-tts reliably accepts).
/// Pauses are produced by inserting [`silence_mp3`] frames.
///
/// If any synthesis call fails the function returns `Err` immediately with a
/// message that identifies the failing part and segment.
pub async fn generate_project_audio(
    project: &Project,
    provider: &dyn TtsProvider,
) -> Result<(Vec<u8>, Vec<(String, Vec<u8>)>), String> {
    // Phase A: decide all ordering / voices / silences sequentially (pure).
    let (ops, layouts) = build_segments(project);
    // Phase B: synthesise concurrently (bounded by provider.max_concurrency())
    // and reassemble strictly by op order.
    execute_segments(ops, layouts, provider).await
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Sanitize a part's label into a safe filename fragment (≤ 40 chars).
///
/// Keeps ASCII alphanumerics and replaces everything else with `_`.
/// Collapses repeated underscores and strips leading/trailing underscores.
fn sanitize_part_name(part: &crate::model::Part) -> String {
    let raw: String = part
        .label
        .chars()
        // 保留 Unicode 字母数字（含中文），其余 → _
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    // Collapse repeated underscores and trim.
    let collapsed = raw
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    // Truncate to 40 chars to keep filenames reasonable.
    collapsed.chars().take(40).collect::<String>()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ExportConfig, Item, Part, Project, SourceType, TaskType, VoiceConfig};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    // ── Mock provider (no network) ────────────────────────────────────────
    //
    // Records every (text, voice) call in invocation order, returns
    // deterministic marker bytes per call so concatenation order is
    // verifiable, tracks the peak number of simultaneously in-flight
    // synthesize() calls, optionally injects an error / sleep keyed by text,
    // and reports a configurable max_concurrency().

    struct MockTtsProvider {
        max_conc: usize,
        /// (text, voice) in the order synthesize() was entered.
        calls: Mutex<Vec<(String, String)>>,
        in_flight: AtomicUsize,
        max_in_flight: AtomicUsize,
        /// text → error message to inject (return Err instead of bytes).
        errors: std::collections::HashMap<String, String>,
        /// text → millis to sleep before returning (to perturb completion order).
        delays: std::collections::HashMap<String, u64>,
    }

    impl MockTtsProvider {
        fn new(max_conc: usize) -> Self {
            Self {
                max_conc,
                calls: Mutex::new(Vec::new()),
                in_flight: AtomicUsize::new(0),
                max_in_flight: AtomicUsize::new(0),
                errors: std::collections::HashMap::new(),
                delays: std::collections::HashMap::new(),
            }
        }

        fn with_error(mut self, text: &str, msg: &str) -> Self {
            self.errors.insert(text.to_string(), msg.to_string());
            self
        }

        fn with_delay(mut self, text: &str, ms: u64) -> Self {
            self.delays.insert(text.to_string(), ms);
            self
        }

        fn call_texts(&self) -> Vec<String> {
            self.calls.lock().unwrap().iter().map(|(t, _)| t.clone()).collect()
        }

        fn peak_in_flight(&self) -> usize {
            self.max_in_flight.load(Ordering::SeqCst)
        }

        /// Deterministic marker bytes for a (text, voice) pair. Distinct inputs
        /// produce distinct, recognisable byte sequences so concatenation order
        /// can be checked exactly.
        fn marker_bytes(text: &str, voice: &str) -> Vec<u8> {
            let tag = format!("<{text}|{voice}>");
            tag.into_bytes()
        }
    }

    #[async_trait::async_trait]
    impl TtsProvider for MockTtsProvider {
        async fn synthesize(
            &self,
            text: &str,
            voice_id: &str,
            _rate: i32,
            _pitch: i32,
            _volume: u32,
        ) -> Result<Vec<u8>, String> {
            self.calls
                .lock()
                .unwrap()
                .push((text.to_string(), voice_id.to_string()));

            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(now, Ordering::SeqCst);

            if let Some(ms) = self.delays.get(text) {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
            }

            self.in_flight.fetch_sub(1, Ordering::SeqCst);

            if let Some(msg) = self.errors.get(text) {
                return Err(msg.clone());
            }
            Ok(MockTtsProvider::marker_bytes(text, voice_id))
        }

        async fn synthesize_ssml(&self, _ssml: &str, _voice_id: &str) -> Result<Vec<u8>, String> {
            unreachable!("audio pipeline never calls synthesize_ssml")
        }

        fn max_concurrency(&self) -> usize {
            self.max_conc
        }
    }

    // ── Project builders for branch coverage ──────────────────────────────

    fn mk_vc() -> VoiceConfig {
        VoiceConfig {
            provider: "mock".into(),
            en_voice: "EN".into(),
            zh_voice: "ZH".into(),
            rate: 0,
            pitch: 0,
            volume: 100,
            teacher_voice: "TEACHER".into(),
            student_voice: "STUDENT".into(),
        }
    }

    fn mk_item(
        id: &str,
        number: Option<u32>,
        text: &str,
        enabled: bool,
        repeat: u8,
        gap: u32,
        read_number: bool,
        override_voice: Option<&str>,
        speaker: Option<&str>,
    ) -> Item {
        Item {
            id: id.into(),
            number,
            text: text.into(),
            enabled,
            repeat,
            gap_after_ms: gap,
            read_number,
            override_voice: override_voice.map(str::to_string),
            speaker: speaker.map(str::to_string),
        }
    }

    /// A Project exercising every Phase A branch: zh instruction on/off, label
    /// with/without ascii, read_number, a pure-Chinese item WITH read_number
    /// (number op + gap, no body), a disabled item, multi-speaker
    /// teacher/student/en fallback ordering, override_voice winning, repeat>=2,
    /// varied gaps, multi-part.
    fn full_project() -> Project {
        let mut p = Project::new("Mock", "x.pdf", SourceType::PdfText);
        p.voice_config = mk_vc();
        p.export_config = ExportConfig::default();

        // Part 1 — zh on, label with ascii, many item shapes.
        p.parts.push(Part {
            id: "p1".into(),
            index: 0,
            label: "Part One. Listen.".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: true,
            zh_instruction: Some("第一大题 listen".into()), // has ascii → spoken
            read_zh_instruction: true,
            items: vec![
                // number + body + repeat, narrator (speaker None → EN)
                mk_item("a", Some(1), "Apple.", true, 2, 3000, true, None, None),
                // disabled → emits NOTHING
                mk_item("b", Some(2), "Banana.", false, 1, 3000, true, None, None),
                // pure-Chinese WITH read_number → number op + 600 + gap, no body,
                // and must NOT advance seen_speakers (skip before voice_for_speaker)
                mk_item("c", Some(3), "词组：", true, 1, 1000, true, None, Some("X")),
                // first real speaker after skip → teacher
                mk_item("d", None, "Dog.", true, 1, 2000, false, None, Some("T1")),
                // second speaker → student
                mk_item("e", None, "Egg.", true, 1, 2000, false, None, Some("S1")),
                // override_voice wins over speaker assignment
                mk_item("f", None, "Fish.", true, 1, 2000, false, Some("OVR"), Some("T1")),
            ],
            gap_after_ms: 5000,
        });

        // Part 2 — zh off (None), label WITHOUT ascii (pure Chinese → skipped),
        // read_number false, fresh seen_speakers.
        p.parts.push(Part {
            id: "p2".into(),
            index: 1,
            label: "第二大题".into(), // no ascii → label NOT spoken, but sanitized into filename
            task_type: TaskType::ListenAndWrite,
            read_label: true,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![
                // speaker reused name "T1" but new part → teacher again
                mk_item("g", None, "Goat.", true, 1, 3000, false, None, Some("T1")),
            ],
            gap_after_ms: 5000,
        });

        p
    }

    /// Independent, hand-rolled SEQUENTIAL reference implementation. Mirrors the
    /// pre-concurrency loop exactly; used as a golden output that the new
    /// pipeline must reproduce byte-for-byte at any concurrency.
    fn sequential_reference(
        project: &Project,
        provider: &MockTtsProvider,
    ) -> (Vec<u8>, Vec<(String, Vec<u8>)>) {
        let vc = &project.voice_config;
        let mut parts_out: Vec<(String, Vec<u8>)> = Vec::new();
        for (idx, part) in project.parts.iter().enumerate() {
            let mut part_mp3: Vec<u8> = Vec::new();
            if part.read_zh_instruction {
                if let Some(ref zh) = part.zh_instruction {
                    if zh.chars().any(|c| c.is_alphanumeric()) {
                        part_mp3.extend(MockTtsProvider::marker_bytes(zh, &vc.zh_voice));
                        part_mp3.extend(silence_mp3(1000));
                    }
                }
            }
            if part.read_label {
                let en_label = strip_chinese_parens(&part.label);
                if en_label.chars().any(|c| c.is_ascii_alphanumeric()) {
                    part_mp3.extend(MockTtsProvider::marker_bytes(&en_label, &vc.en_voice));
                    part_mp3.extend(silence_mp3(2000));
                }
            }
            let mut seen: Vec<String> = Vec::new();
            for item in &part.items {
                if !item.enabled {
                    continue;
                }
                if item.read_number {
                    if let Some(n) = item.number {
                        let num_text = format!("Number {n}.");
                        part_mp3.extend(MockTtsProvider::marker_bytes(&num_text, &vc.en_voice));
                        part_mp3.extend(silence_mp3(600));
                    }
                }
                if !item.text.chars().any(|c| c.is_ascii_alphanumeric()) {
                    part_mp3.extend(silence_mp3(item.gap_after_ms));
                    continue;
                }
                let speaker_voice = voice_for_speaker(&item.speaker, &mut seen, vc);
                let body_voice = resolve_body_voice(&item.override_voice, speaker_voice);
                part_mp3.extend(MockTtsProvider::marker_bytes(&item.text, body_voice));
                if item.repeat >= 2 {
                    part_mp3.extend(silence_mp3(1500));
                    part_mp3.extend(MockTtsProvider::marker_bytes(&item.text, body_voice));
                }
                part_mp3.extend(silence_mp3(item.gap_after_ms));
            }
            part_mp3.extend(silence_mp3(part.gap_after_ms));
            let filename = format!("{:02}_{}.mp3", idx + 1, sanitize_part_name(part));
            parts_out.push((filename, part_mp3));
        }
        let _ = provider; // reference does not actually call the provider
        let total: usize = parts_out.iter().map(|(_, b)| b.len()).sum();
        let mut full = Vec::with_capacity(total);
        for (_, b) in &parts_out {
            full.extend_from_slice(b);
        }
        (full, parts_out)
    }

    // ── Byte-equivalence: N=1 vs N=5 produce identical output ──────────────

    #[tokio::test]
    async fn output_identical_across_concurrency() {
        let project = full_project();

        let p1 = MockTtsProvider::new(1);
        let (full_a, parts_a) = generate_project_audio(&project, &p1).await.unwrap();

        let p5 = MockTtsProvider::new(5);
        let (full_b, parts_b) = generate_project_audio(&project, &p5).await.unwrap();

        assert_eq!(full_a, full_b, "full mp3 must be byte-identical across concurrency");
        assert_eq!(parts_a, parts_b, "per-part output must be byte-identical across concurrency");
    }

    // ── Golden sequential reference: new pipeline reproduces it byte-for-byte ─

    #[tokio::test]
    async fn matches_sequential_reference() {
        let project = full_project();
        let provider = MockTtsProvider::new(1);
        let (full_new, parts_new) = generate_project_audio(&project, &provider).await.unwrap();
        let (full_ref, parts_ref) = sequential_reference(&project, &provider);
        assert_eq!(full_new, full_ref, "full output drifted from sequential reference");
        assert_eq!(parts_new, parts_ref, "per-part output drifted from sequential reference");
    }

    // ── Concatenation order / call order at N=1 (Edge-equivalent) ──────────

    #[tokio::test]
    async fn call_order_matches_sequence_at_n1() {
        let project = full_project();
        let provider = MockTtsProvider::new(1);
        generate_project_audio(&project, &provider).await.unwrap();

        // The synth-call text sequence must follow Phase A op order exactly.
        let expected = vec![
            "第一大题 listen".to_string(), // zh instruction (has ascii)
            "Part One. Listen.".to_string(), // label
            "Number 1.".to_string(),         // item a number
            "Apple.".to_string(),            // item a body
            "Apple.".to_string(),            // item a repeat
            // item b disabled → nothing
            "Number 3.".to_string(),         // item c number (body skipped: pure Chinese)
            "Dog.".to_string(),              // item d body
            "Egg.".to_string(),              // item e body
            "Fish.".to_string(),             // item f body
            // part 2: zh off, label pure-Chinese (skipped), no number
            "Goat.".to_string(),
        ];
        assert_eq!(provider.call_texts(), expected);

        // N=1 → never more than one synthesize in flight (Edge-equivalent serial).
        assert_eq!(provider.peak_in_flight(), 1, "N=1 must be strictly serial");
    }

    // ── Dispatch concurrency actually overlaps at N>1 ─────────────────────

    #[tokio::test]
    async fn concurrency_overlaps_at_n5() {
        let project = full_project();
        // Delay every synth a touch so multiple are genuinely in flight at once.
        let mut provider = MockTtsProvider::new(5);
        for t in ["Number 1.", "Apple.", "Number 3.", "Dog.", "Egg.", "Fish.", "Goat."] {
            provider = provider.with_delay(t, 20);
        }
        generate_project_audio(&project, &provider).await.unwrap();
        let peak = provider.peak_in_flight();
        assert!(peak > 1, "N=5 should overlap synth calls, peak was {peak}");
        assert!(peak <= 5, "must never exceed max_concurrency, peak was {peak}");
    }

    // ── Skipped (pure-Chinese) item does NOT advance seen_speakers ─────────

    #[tokio::test]
    async fn skipped_item_does_not_consume_speaker_slot() {
        let project = full_project();
        let provider = MockTtsProvider::new(1);
        generate_project_audio(&project, &provider).await.unwrap();
        // Item c (speaker "X") is pure-Chinese → skipped → must not become the
        // first speaker. Item d (speaker "T1") is therefore the FIRST real
        // speaker and must get the teacher voice.
        let calls = provider.calls.lock().unwrap();
        let dog = calls.iter().find(|(t, _)| t == "Dog.").expect("Dog. synthesised");
        assert_eq!(dog.1, "TEACHER", "first real speaker after a skip must be teacher");
    }

    // ── Disabled item contributes zero bytes; pure-Chinese item = number+gaps ─

    #[tokio::test]
    async fn disabled_emits_nothing_and_skip_emits_only_number_and_gaps() {
        // Minimal project: one part, a disabled item only.
        let mut disabled = Project::new("d", "x.pdf", SourceType::PdfText);
        disabled.voice_config = mk_vc();
        disabled.parts.push(Part {
            id: "p".into(),
            index: 0,
            label: "X".into(), // ascii → spoken; isolate by measuring against ref
            task_type: TaskType::ListenAndChoose,
            read_label: false,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![mk_item("z", Some(9), "Zoo.", false, 1, 1000, true, None, None)],
            gap_after_ms: 5000,
        });
        let provider = MockTtsProvider::new(1);
        let (_full, parts) = generate_project_audio(&disabled, &provider).await.unwrap();
        // Disabled item emits nothing → part is only the trailing part gap.
        assert_eq!(parts[0].1, silence_mp3(5000));
        assert!(provider.call_texts().is_empty(), "disabled item must not synthesise");

        // Pure-Chinese item WITH read_number: number op bytes + 600 silence + gap,
        // no body.
        let mut skip = Project::new("s", "x.pdf", SourceType::PdfText);
        skip.voice_config = mk_vc();
        skip.parts.push(Part {
            id: "p".into(),
            index: 0,
            label: "X".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: false,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![mk_item("c", Some(3), "词组：", true, 1, 1000, true, None, None)],
            gap_after_ms: 0,
        });
        let provider2 = MockTtsProvider::new(1);
        let (_full2, parts2) = generate_project_audio(&skip, &provider2).await.unwrap();
        let mut expected = MockTtsProvider::marker_bytes("Number 3.", "EN");
        expected.extend(silence_mp3(600));
        expected.extend(silence_mp3(1000)); // item gap
        expected.extend(silence_mp3(0)); // part gap
        assert_eq!(parts2[0].1, expected, "skip item = number + 600 + item gap, no body");
        assert_eq!(provider2.call_texts(), vec!["Number 3.".to_string()]);
    }

    // ── Error determinism: earliest ORDINAL wins, not earliest to complete ──

    #[tokio::test]
    async fn earliest_ordinal_error_wins_under_concurrency() {
        // Two failing segments; the LATER-ordinal one completes first (no delay)
        // while the earlier one sleeps. The earliest-ordinal message must win.
        let project = full_project();
        // "Apple." is an earlier synth-ordinal than "Fish."
        let provider = MockTtsProvider::new(5)
            .with_error("Apple.", "boom-apple")
            .with_delay("Apple.", 40) // earlier ordinal, completes LATER
            .with_error("Fish.", "boom-fish"); // later ordinal, completes first
        let err = generate_project_audio(&project, &provider).await.unwrap_err();
        // Apple. is item a body → "(text)" template, part 1 item 1.
        assert_eq!(err, "en TTS part 1 item 1 (text): boom-apple");
    }

    // ── Error template fidelity for each SegKind ───────────────────────────

    #[tokio::test]
    async fn error_templates_verbatim_per_segkind() {
        // zh instruction
        let p = full_project();
        let prov = MockTtsProvider::new(1).with_error("第一大题 listen", "E");
        assert_eq!(
            generate_project_audio(&p, &prov).await.unwrap_err(),
            "zh TTS part 1 (zh instruction): E"
        );

        // label
        let prov = MockTtsProvider::new(1).with_error("Part One. Listen.", "E");
        assert_eq!(
            generate_project_audio(&p, &prov).await.unwrap_err(),
            "en TTS part 1 (label): E"
        );

        // number (item a, item 1)
        let prov = MockTtsProvider::new(1).with_error("Number 1.", "E");
        assert_eq!(
            generate_project_audio(&p, &prov).await.unwrap_err(),
            "en TTS part 1 item 1 (number): E"
        );

        // body / text (item a, item 1) — but Number 1. is an earlier ordinal,
        // so isolate with a project lacking a number. Reuse item d (Dog., item 4).
        let prov = MockTtsProvider::new(1).with_error("Dog.", "E");
        assert_eq!(
            generate_project_audio(&p, &prov).await.unwrap_err(),
            "en TTS part 1 item 4 (text): E"
        );

        // repeat (item a, item 1) — earlier ordinals (zh, label, number, body)
        // must succeed; only the repeat fails. The repeat reuses text "Apple."
        // same as the body, so injecting on "Apple." would hit the body first.
        // Verify the Repeat template directly instead.
        assert_eq!(
            SegKind::Repeat.render(2, 7, "E"),
            "en TTS part 2 item 7 (repeat): E"
        );

        // Verbatim pass-through of a realistic provider error string.
        let prov = MockTtsProvider::new(1)
            .with_error("Dog.", "timeout（已重试 4 次仍失败）");
        assert_eq!(
            generate_project_audio(&p, &prov).await.unwrap_err(),
            "en TTS part 1 item 4 (text): timeout（已重试 4 次仍失败）"
        );
    }

    // ── Short-circuit: any error → Err, no partial parts returned ──────────

    #[tokio::test]
    async fn error_returns_err_with_no_partial_output() {
        let project = full_project();
        let provider = MockTtsProvider::new(5).with_error("Goat.", "fail");
        let result = generate_project_audio(&project, &provider).await;
        assert!(result.is_err(), "any segment failure must fail the whole export");
    }

    // ── silence_mp3 ───────────────────────────────────────────────────────

    #[test]
    fn silence_mp3_starts_with_sync_bytes() {
        let data = silence_mp3(100);
        assert_eq!(
            &data[..2],
            &[0xFF, 0xF3],
            "must start with MPEG-2 sync bytes FF F3"
        );
    }

    #[test]
    fn silence_mp3_full_header() {
        let data = silence_mp3(100);
        assert_eq!(
            &data[..4],
            &[0xFF, 0xF3, 0x64, 0xC0],
            "frame header must be FF F3 64 C0"
        );
    }

    #[test]
    fn silence_mp3_length_1000ms() {
        // 1000 ms / 24 ms per frame = ceil(41.67) = 42 frames
        // 42 frames × 144 bytes = 6048 bytes
        let data = silence_mp3(1000);
        assert_eq!(data.len(), 42 * 144, "1000 ms should produce 42 frames (6048 bytes)");
    }

    #[test]
    fn silence_mp3_length_24ms_exact() {
        // Exactly one frame duration
        let data = silence_mp3(24);
        assert_eq!(data.len(), 144);
    }

    #[test]
    fn silence_mp3_minimum_one_frame() {
        // Even 0 ms should produce at least 1 frame
        let data = silence_mp3(0);
        assert_eq!(data.len(), 144, "0 ms must still produce 1 frame");
    }

    #[test]
    fn silence_mp3_multiple_of_frame_size() {
        for ms in [100, 500, 999, 1000, 2000, 5000] {
            let data = silence_mp3(ms);
            assert_eq!(
                data.len() % 144,
                0,
                "silence_mp3({ms}) length {} is not a multiple of 144",
                data.len()
            );
        }
    }

    #[test]
    fn silence_mp3_payload_is_zero() {
        let data = silence_mp3(24); // one frame
        // Bytes 4..144 must all be 0x00
        assert!(
            data[4..].iter().all(|&b| b == 0),
            "frame payload (bytes 4..) must be all zeros (silence)"
        );
    }

    // ── sanitize_part_name ────────────────────────────────────────────────

    #[test]
    fn sanitize_part_name_ascii() {
        let part = Part {
            id: "x".into(),
            index: 0,
            label: "Part One. Listen & choose!".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: false,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![],
            gap_after_ms: 5000,
        };
        let name = sanitize_part_name(&part);
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "unexpected chars: {name}"
        );
        assert!(!name.starts_with('_'), "should not start with _: {name}");
        assert!(!name.ends_with('_'), "should not end with _: {name}");
    }

    #[test]
    fn sanitize_part_name_keeps_chinese() {
        let part = Part {
            id: "x".into(),
            index: 0,
            label: "第一大题 听录音".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: false,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![],
            gap_after_ms: 5000,
        };
        let name = sanitize_part_name(&part);
        assert!(name.contains("第一大题"), "中文 label 应保留: {name}");
    }

    #[test]
    fn sanitize_part_name_truncates() {
        let part = Part {
            id: "x".into(),
            index: 0,
            label: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: false,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![],
            gap_after_ms: 5000,
        };
        let name = sanitize_part_name(&part);
        assert!(name.len() <= 40, "should be truncated: {}", name.len());
    }

    // ── voice_for_speaker ─────────────────────────────────────────────────

    fn make_vc() -> crate::model::VoiceConfig {
        crate::model::VoiceConfig {
            provider: "edge".into(),
            en_voice: "en-voice".into(),
            zh_voice: "zh-voice".into(),
            rate: 0,
            pitch: 0,
            volume: 100,
            teacher_voice: "teacher-voice".into(),
            student_voice: "student-voice".into(),
        }
    }

    #[test]
    fn voice_for_speaker_none_returns_en_voice() {
        let vc = make_vc();
        let mut seen: Vec<String> = Vec::new();
        assert_eq!(voice_for_speaker(&None, &mut seen, &vc), "en-voice");
        // seen should remain empty
        assert!(seen.is_empty());
    }

    #[test]
    fn voice_for_speaker_first_speaker_is_teacher() {
        let vc = make_vc();
        let mut seen: Vec<String> = Vec::new();
        let v = voice_for_speaker(&Some("A".into()), &mut seen, &vc);
        assert_eq!(v, "teacher-voice");
    }

    #[test]
    fn voice_for_speaker_second_speaker_is_student() {
        let vc = make_vc();
        let mut seen: Vec<String> = Vec::new();
        voice_for_speaker(&Some("A".into()), &mut seen, &vc);
        let v = voice_for_speaker(&Some("B".into()), &mut seen, &vc);
        assert_eq!(v, "student-voice");
    }

    #[test]
    fn voice_for_speaker_same_speaker_stable() {
        let vc = make_vc();
        let mut seen: Vec<String> = Vec::new();
        // A first → teacher
        voice_for_speaker(&Some("A".into()), &mut seen, &vc);
        // B second → student
        voice_for_speaker(&Some("B".into()), &mut seen, &vc);
        // A again → still teacher
        let v = voice_for_speaker(&Some("A".into()), &mut seen, &vc);
        assert_eq!(v, "teacher-voice");
    }

    #[test]
    fn voice_for_speaker_third_speaker_is_en_voice() {
        let vc = make_vc();
        let mut seen: Vec<String> = Vec::new();
        voice_for_speaker(&Some("A".into()), &mut seen, &vc);
        voice_for_speaker(&Some("B".into()), &mut seen, &vc);
        let v = voice_for_speaker(&Some("C".into()), &mut seen, &vc);
        assert_eq!(v, "en-voice");
    }

    // ── resolve_body_voice ────────────────────────────────────────────────

    #[test]
    fn resolve_body_voice_none_uses_speaker_voice() {
        assert_eq!(resolve_body_voice(&None, "speaker-voice"), "speaker-voice");
    }

    #[test]
    fn resolve_body_voice_override_wins() {
        let ov = Some("en-US-AnaNeural".to_string());
        assert_eq!(resolve_body_voice(&ov, "speaker-voice"), "en-US-AnaNeural");
    }

    #[test]
    fn resolve_body_voice_blank_override_falls_back() {
        // 空白 / 全空格的 override 视为未设置，回退到 speaker_voice
        assert_eq!(resolve_body_voice(&Some("".into()), "speaker-voice"), "speaker-voice");
        assert_eq!(resolve_body_voice(&Some("   ".into()), "speaker-voice"), "speaker-voice");
    }

    // ── E2E (network — run with `cargo test -- --ignored`) ────────────────

    /// End-to-end: build a tiny Project, call generate_project_audio,
    /// assert full mp3 non-empty and starts with ID3 or MPEG sync byte.
    #[tokio::test]
    #[ignore = "requires live Edge TTS network access"]
    async fn e2e_generate_project_audio() {
        use crate::model::{ExportConfig, Project, SourceType, VoiceConfig};
        use crate::tts::edge::EdgeTtsProvider;

        let provider = EdgeTtsProvider::new();

        let mut project = Project::new("TestProject", "test.pdf", SourceType::PdfText);
        project.voice_config = VoiceConfig {
            provider: "edge".into(),
            en_voice: "en-GB-SoniaNeural".into(),
            zh_voice: "zh-CN-XiaoxiaoNeural".into(),
            rate: 0,
            pitch: 0,
            volume: 100,
            teacher_voice: "en-US-GuyNeural".into(),
            student_voice: "en-US-AnaNeural".into(),
        };
        project.export_config = ExportConfig::default();

        // Part 1: 2 items
        project.parts.push(Part {
            id: "part-1".into(),
            index: 0,
            label: "Part One. Listen and choose.".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: true,
            zh_instruction: Some("第一大题,听录音选择。".into()),
            read_zh_instruction: true,
            items: vec![
                Item {
                    id: "i1".into(),
                    number: Some(1),
                    text: "I can take the dishes to the kitchen.".into(),
                    enabled: true,
                    repeat: 2,
                    gap_after_ms: 3000,
                    read_number: true,
                    override_voice: None,
                    speaker: None,
                },
                Item {
                    id: "i2".into(),
                    number: Some(2),
                    text: "The dog is under the table.".into(),
                    enabled: true,
                    repeat: 1,
                    gap_after_ms: 3000,
                    read_number: true,
                    override_voice: None,
                    speaker: None,
                },
            ],
            gap_after_ms: 5000,
        });

        // Part 2: 1 item
        project.parts.push(Part {
            id: "part-2".into(),
            index: 1,
            label: "Part Two. Listen and fill.".into(),
            task_type: TaskType::ListenAndWrite,
            read_label: true,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![Item {
                id: "i3".into(),
                number: Some(1),
                text: "The sun rises in the east.".into(),
                enabled: true,
                repeat: 1,
                gap_after_ms: 3000,
                read_number: false,
                override_voice: None,
                speaker: None,
            }],
            gap_after_ms: 5000,
        });

        let result = generate_project_audio(&project, &provider).await;
        assert!(result.is_ok(), "generate_project_audio failed: {:?}", result);

        let (full, parts) = result.unwrap();
        assert!(!full.is_empty(), "full mp3 must not be empty");
        assert_eq!(parts.len(), 2, "should have 2 part files");
        assert!(!parts[0].1.is_empty(), "part 1 mp3 must not be empty");
        assert!(!parts[1].1.is_empty(), "part 2 mp3 must not be empty");

        // MP3 frames begin with 0xFF 0xFB/0xE0/... or ID3 header "ID3"
        let is_mp3_like = |data: &[u8]| {
            data.starts_with(b"ID3")
                || (data.len() >= 2 && data[0] == 0xFF && data[1] & 0xE0 == 0xE0)
        };
        assert!(
            is_mp3_like(&full),
            "full output doesn't look like MP3: first bytes {:?}",
            &full[..full.len().min(4)]
        );
    }
}
