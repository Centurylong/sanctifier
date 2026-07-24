//! Source-optional analysis: inspect a compiled Soroban WASM module directly.
//!
//! Some audit targets are only distributed as a deployed `.wasm` artifact — the
//! Rust source is not available. This module parses the WebAssembly binary by
//! hand (no external crates, to keep the pinned toolchain happy) and runs a set
//! of **basic, bytecode-level** checks that do not need the source.
//!
//! It is deliberately conservative: at the bytecode level we can see the module's
//! shape (imports, exports, memory, Soroban metadata sections, and the value
//! types used in function signatures) but *not* the semantics that the
//! source-based detectors reason about (auth guards, arithmetic overflow,
//! storage-key collisions, …). See [`WasmReport::limitations`] and
//! `docs/wasm-analysis.md` for the full source-vs-WASM comparison.

use crate::finding_codes;
use serde::Serialize;

/// The 4-byte `\0asm` magic that opens every WebAssembly module.
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];
/// The only module-format version Soroban emits (MVP encoding).
const WASM_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

/// Custom-section names the Soroban SDK embeds in a compiled contract.
const SEC_CONTRACT_SPEC: &str = "contractspecv0";
const SEC_CONTRACT_ENV_META: &str = "contractenvmetav0";

/// A single bytecode-level finding.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WasmFinding {
    /// Stable finding code (see [`crate::finding_codes`], `W0xx` family).
    pub code: &'static str,
    /// Coarse severity, mirroring the source-mode severities.
    pub severity: WasmSeverity,
    /// One-line title.
    pub title: String,
    /// Human-readable detail and remediation hint.
    pub detail: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WasmSeverity {
    Info,
    Warning,
    Error,
}

/// Structural facts recovered from the module. Purely descriptive — the checks
/// that turn these into findings live in [`analyze_wasm`].
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct WasmModuleInfo {
    /// Number of function types declared in the type section.
    pub num_types: u32,
    /// Number of *imported* functions (kind = func).
    pub num_func_imports: u32,
    /// Total number of imports of any kind.
    pub num_imports_total: u32,
    /// Distinct import module names (typically just `env` for Soroban).
    pub import_modules: Vec<String>,
    /// Number of functions defined in this module (function section length).
    pub num_functions: u32,
    /// Number of exported functions (kind = func).
    pub num_func_exports: u32,
    /// Names of the exported functions — the callable surface of the contract.
    pub export_names: Vec<String>,
    /// Declared initial memory size, in 64 KiB pages, if a memory is present.
    pub memory_pages_min: Option<u32>,
    /// Declared maximum memory size, in pages, if the module caps it.
    pub memory_pages_max: Option<u32>,
    /// Names of every custom section, in order.
    pub custom_sections: Vec<String>,
    /// Whether the module declares a `start` function.
    pub has_start: bool,
    /// Whether any function signature uses `f32`/`f64` value types.
    pub uses_float: bool,
}

impl WasmModuleInfo {
    fn has_custom(&self, name: &str) -> bool {
        self.custom_sections.iter().any(|s| s == name)
    }
}

/// The result of analysing a WASM module: the recovered facts plus any findings.
#[derive(Debug, Clone, Serialize)]
pub struct WasmReport {
    pub info: WasmModuleInfo,
    pub findings: Vec<WasmFinding>,
}

impl WasmReport {
    /// Whether the module looks like a Soroban contract (embeds the SDK spec
    /// section). Consumers can use this to soften wording for non-Soroban input.
    pub fn is_soroban_contract(&self) -> bool {
        self.info.has_custom(SEC_CONTRACT_SPEC)
    }

    /// A fixed, honest list of what source-optional mode cannot see, so callers
    /// (CLI text/JSON, docs) never oversell bytecode analysis.
    pub fn limitations() -> &'static [&'static str] {
        &[
            "No authentication-gap detection: require_auth() is a host import call, indistinguishable from other env calls at the bytecode level.",
            "No arithmetic-overflow detection: source-level `+`/`-`/`*` are lowered to i128 helper calls or i64 opcodes with no type/context to flag.",
            "No storage-key-collision, event, or upgrade-pattern analysis: these rely on source symbols and macros that do not survive compilation.",
            "No line-accurate locations: findings map to the module, not to a file:line.",
            "Function/argument names are only available for the exported entrypoints (and only when a name section or contract spec is present).",
        ]
    }
}

/// Error returned when the input is not a WASM module we can parse.
#[derive(Debug, Clone, PartialEq)]
pub enum WasmError {
    /// Input is shorter than the 8-byte header.
    TooShort,
    /// Missing the `\0asm` magic number.
    BadMagic,
    /// Unsupported module-format version.
    UnsupportedVersion([u8; 4]),
    /// A section declared a length that runs past the end of the input, or an
    /// integer/vector was truncated. Carries a short human-readable reason.
    Malformed(String),
}

impl std::fmt::Display for WasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WasmError::TooShort => write!(f, "input is too short to be a WASM module"),
            WasmError::BadMagic => write!(f, "not a WASM module (missing \\0asm magic)"),
            WasmError::UnsupportedVersion(v) => {
                write!(f, "unsupported WASM version {v:?} (expected 1)")
            }
            WasmError::Malformed(why) => write!(f, "malformed WASM module: {why}"),
        }
    }
}

impl std::error::Error for WasmError {}

/// A bounds-checked forward cursor over a byte slice. Every read returns `None`
/// on underrun so a truncated/hostile module can never panic the analyzer.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn read_byte(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    /// Read an unsigned LEB128 integer (WASM caps these at 32 bits for the
    /// counts/indices we read here).
    fn read_leb_u32(&mut self) -> Option<u32> {
        let mut result: u64 = 0;
        let mut shift = 0;
        loop {
            let byte = self.read_byte()?;
            result |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 35 {
                return None; // more than 5 bytes → not a valid u32 LEB128
            }
        }
        u32::try_from(result).ok()
    }

    /// Read a length-prefixed UTF-8 name (`vec(byte)` interpreted as text).
    fn read_name(&mut self) -> Option<String> {
        let len = self.read_leb_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Parse `bytes` as a WebAssembly module and run the bytecode-level checks.
///
/// Returns [`WasmError`] only when the header is not WASM at all or a section
/// length overruns the buffer; individual malformed *sub*-sections are skipped
/// defensively so a single bad byte does not sink the whole report.
pub fn analyze_wasm(bytes: &[u8]) -> Result<WasmReport, WasmError> {
    if bytes.len() < 8 {
        return Err(WasmError::TooShort);
    }
    if bytes[0..4] != WASM_MAGIC {
        return Err(WasmError::BadMagic);
    }
    let mut version = [0u8; 4];
    version.copy_from_slice(&bytes[4..8]);
    if version != WASM_VERSION {
        return Err(WasmError::UnsupportedVersion(version));
    }

    let mut info = WasmModuleInfo::default();
    let mut cursor = Cursor::new(bytes);
    cursor.pos = 8; // skip magic + version

    while !cursor.is_empty() {
        let id = cursor
            .read_byte()
            .ok_or_else(|| WasmError::Malformed("truncated section id".into()))?;
        let size = cursor
            .read_leb_u32()
            .ok_or_else(|| WasmError::Malformed("truncated section size".into()))?
            as usize;
        let payload = cursor
            .read_bytes(size)
            .ok_or_else(|| WasmError::Malformed(format!("section {id} overruns module")))?;

        // Each section is parsed against its own bounded sub-cursor. Errors
        // inside a section are swallowed (best-effort) rather than propagated.
        let mut sec = Cursor::new(payload);
        match id {
            0 => parse_custom_section(&mut sec, &mut info),
            1 => parse_type_section(&mut sec, &mut info),
            2 => parse_import_section(&mut sec, &mut info),
            3 => {
                info.num_functions = sec.read_leb_u32().unwrap_or(0);
            }
            5 => parse_memory_section(&mut sec, &mut info),
            7 => parse_export_section(&mut sec, &mut info),
            8 => {
                info.has_start = true;
            }
            _ => {}
        }
    }

    let findings = run_checks(&info);
    Ok(WasmReport { info, findings })
}

fn parse_custom_section(sec: &mut Cursor, info: &mut WasmModuleInfo) {
    if let Some(name) = sec.read_name() {
        info.custom_sections.push(name);
    }
}

/// Type section: `vec(functype)`, each `0x60 vec(valtype) vec(valtype)`. We only
/// need to know whether any signature mentions a float value type.
fn parse_type_section(sec: &mut Cursor, info: &mut WasmModuleInfo) {
    let count = match sec.read_leb_u32() {
        Some(c) => c,
        None => return,
    };
    info.num_types = count;
    for _ in 0..count {
        // functype marker
        if sec.read_byte() != Some(0x60) {
            return; // unexpected encoding; stop scanning defensively
        }
        for _ in 0..2 {
            // params then results, each a vec(valtype)
            let n = match sec.read_leb_u32() {
                Some(n) => n,
                None => return,
            };
            for _ in 0..n {
                match sec.read_byte() {
                    // 0x7D = f32, 0x7C = f64
                    Some(0x7D) | Some(0x7C) => info.uses_float = true,
                    Some(_) => {}
                    None => return,
                }
            }
        }
    }
}

/// Import section: `vec(import)`, each `name name importdesc`.
fn parse_import_section(sec: &mut Cursor, info: &mut WasmModuleInfo) {
    let count = match sec.read_leb_u32() {
        Some(c) => c,
        None => return,
    };
    info.num_imports_total = count;
    for _ in 0..count {
        let module = match sec.read_name() {
            Some(m) => m,
            None => return,
        };
        if sec.read_name().is_none() {
            return; // field name
        }
        if !info.import_modules.contains(&module) {
            info.import_modules.push(module);
        }
        let kind = match sec.read_byte() {
            Some(k) => k,
            None => return,
        };
        match kind {
            0x00 => {
                // func: type index
                info.num_func_imports += 1;
                if sec.read_leb_u32().is_none() {
                    return;
                }
            }
            0x01 => {
                // table: reftype + limits
                if sec.read_byte().is_none() || read_limits(sec).is_none() {
                    return;
                }
            }
            0x02 => {
                // mem: limits
                if read_limits(sec).is_none() {
                    return;
                }
            }
            0x03 => {
                // global: valtype + mut
                if sec.read_byte().is_none() || sec.read_byte().is_none() {
                    return;
                }
            }
            _ => return,
        }
    }
}

/// Memory section: `vec(limits)`. Soroban modules declare exactly one memory;
/// we record the first.
fn parse_memory_section(sec: &mut Cursor, info: &mut WasmModuleInfo) {
    let count = match sec.read_leb_u32() {
        Some(c) => c,
        None => return,
    };
    if count == 0 {
        return;
    }
    if let Some((min, max)) = read_limits(sec) {
        info.memory_pages_min = Some(min);
        info.memory_pages_max = max;
    }
}

/// Read a `limits`: flag byte then min, and max when the flag is set.
fn read_limits(sec: &mut Cursor) -> Option<(u32, Option<u32>)> {
    let flag = sec.read_byte()?;
    let min = sec.read_leb_u32()?;
    let max = if flag & 0x01 != 0 {
        Some(sec.read_leb_u32()?)
    } else {
        None
    };
    Some((min, max))
}

/// Export section: `vec(export)`, each `name exportdesc(kind idx)`.
fn parse_export_section(sec: &mut Cursor, info: &mut WasmModuleInfo) {
    let count = match sec.read_leb_u32() {
        Some(c) => c,
        None => return,
    };
    for _ in 0..count {
        let name = match sec.read_name() {
            Some(n) => n,
            None => return,
        };
        let kind = match sec.read_byte() {
            Some(k) => k,
            None => return,
        };
        if sec.read_leb_u32().is_none() {
            return; // index
        }
        if kind == 0x00 {
            info.num_func_exports += 1;
            info.export_names.push(name);
        }
    }
}

/// Turn recovered facts into findings. Every check is derivable from section
/// structure alone — no opcode/semantic reasoning.
fn run_checks(info: &WasmModuleInfo) -> Vec<WasmFinding> {
    let mut findings = Vec::new();

    if !info.has_custom(SEC_CONTRACT_SPEC) {
        findings.push(WasmFinding {
            code: finding_codes::WASM_NOT_SOROBAN,
            severity: WasmSeverity::Warning,
            title: "No Soroban contract spec embedded".to_string(),
            detail: format!(
                "The module has no `{SEC_CONTRACT_SPEC}` custom section, which the Soroban SDK \
                 always embeds. This may not be a deployable Soroban contract, or it was built \
                 without the SDK. Bytecode checks still ran, but Soroban-specific assumptions do \
                 not apply."
            ),
        });
    }

    if info.num_func_exports == 0 {
        findings.push(WasmFinding {
            code: finding_codes::WASM_NO_EXPORTS,
            severity: WasmSeverity::Warning,
            title: "Module exports no callable functions".to_string(),
            detail: "No functions are exported, so nothing in this module can be invoked as a \
                 contract entrypoint. Verify the build did not strip exports."
                .to_string(),
        });
    }

    if !info.has_custom(SEC_CONTRACT_ENV_META) {
        findings.push(WasmFinding {
            code: finding_codes::WASM_MISSING_ENV_META,
            severity: WasmSeverity::Info,
            title: "Missing Soroban environment metadata".to_string(),
            detail: format!(
                "The module has no `{SEC_CONTRACT_ENV_META}` section, so its target interface \
                 version cannot be read. SDK/protocol compatibility cannot be verified from the \
                 bytecode alone."
            ),
        });
    }

    if info.uses_float {
        findings.push(WasmFinding {
            code: finding_codes::WASM_FLOAT_TYPES,
            severity: WasmSeverity::Error,
            title: "Floating-point types in function signatures".to_string(),
            detail: "One or more function signatures use f32/f64 value types. The Soroban host \
                 environment forbids floating-point, so such a module will be rejected on \
                 deployment or trap at runtime. Use integer/fixed-point arithmetic instead."
                .to_string(),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal helper: build a section (id + LEB size + payload).
    fn section(id: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![id];
        out.extend(leb(payload.len() as u32));
        out.extend_from_slice(payload);
        out
    }

    fn leb(mut v: u32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (v & 0x7F) as u8;
            v >>= 7;
            if v != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if v == 0 {
                break;
            }
        }
        out
    }

    fn header() -> Vec<u8> {
        let mut m = WASM_MAGIC.to_vec();
        m.extend_from_slice(&WASM_VERSION);
        m
    }

    /// A custom section carries a length-prefixed name then its bytes.
    fn custom(name: &str) -> Vec<u8> {
        let mut payload = leb(name.len() as u32);
        payload.extend_from_slice(name.as_bytes());
        section(0, &payload)
    }

    #[test]
    fn rejects_non_wasm() {
        assert_eq!(
            analyze_wasm(b"not wasm!!").unwrap_err(),
            WasmError::BadMagic
        );
        assert_eq!(analyze_wasm(b"\0as").unwrap_err(), WasmError::TooShort);
    }

    #[test]
    fn rejects_bad_version() {
        let mut bytes = WASM_MAGIC.to_vec();
        bytes.extend_from_slice(&[0x02, 0, 0, 0]);
        assert!(matches!(
            analyze_wasm(&bytes).unwrap_err(),
            WasmError::UnsupportedVersion(_)
        ));
    }

    #[test]
    fn empty_module_flags_missing_spec_and_exports() {
        let report = analyze_wasm(&header()).unwrap();
        let codes: Vec<&str> = report.findings.iter().map(|f| f.code).collect();
        assert!(codes.contains(&finding_codes::WASM_NOT_SOROBAN));
        assert!(codes.contains(&finding_codes::WASM_NO_EXPORTS));
        assert!(codes.contains(&finding_codes::WASM_MISSING_ENV_META));
        assert!(!report.is_soroban_contract());
    }

    #[test]
    fn soroban_module_with_exports_is_clean() {
        let mut bytes = header();
        // export section: one function export named "transfer" (kind 0, idx 0)
        let mut exp = leb(1);
        exp.extend(leb("transfer".len() as u32));
        exp.extend_from_slice(b"transfer");
        exp.push(0x00); // func
        exp.extend(leb(0)); // index
        bytes.extend(section(7, &exp));
        bytes.extend(custom(SEC_CONTRACT_SPEC));
        bytes.extend(custom(SEC_CONTRACT_ENV_META));

        let report = analyze_wasm(&bytes).unwrap();
        assert!(report.is_soroban_contract());
        assert_eq!(report.info.num_func_exports, 1);
        assert_eq!(report.info.export_names, vec!["transfer".to_string()]);
        assert!(
            report.findings.is_empty(),
            "expected no findings, got {:?}",
            report.findings
        );
    }

    #[test]
    fn detects_float_signatures() {
        let mut bytes = header();
        // type section: one functype (f32) -> ()
        let mut ty = leb(1);
        ty.push(0x60); // functype
        ty.extend(leb(1)); // 1 param
        ty.push(0x7D); // f32
        ty.extend(leb(0)); // 0 results
        bytes.extend(section(1, &ty));
        bytes.extend(custom(SEC_CONTRACT_SPEC));
        bytes.extend(custom(SEC_CONTRACT_ENV_META));
        // give it an export so W002 doesn't fire
        let mut exp = leb(1);
        exp.extend(leb(1));
        exp.extend_from_slice(b"f");
        exp.push(0x00);
        exp.extend(leb(0));
        bytes.extend(section(7, &exp));

        let report = analyze_wasm(&bytes).unwrap();
        assert!(report.info.uses_float);
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == finding_codes::WASM_FLOAT_TYPES));
    }

    #[test]
    fn parses_imports_and_memory() {
        let mut bytes = header();
        // import section: env.foo as a function (type idx 0)
        let mut imp = leb(1);
        imp.extend(leb(3));
        imp.extend_from_slice(b"env");
        imp.extend(leb(3));
        imp.extend_from_slice(b"foo");
        imp.push(0x00); // func kind
        imp.extend(leb(0)); // type index
        bytes.extend(section(2, &imp));
        // memory section: one memory, min 17 pages, no max
        let mut mem = leb(1);
        mem.push(0x00); // flag: no max
        mem.extend(leb(17));
        bytes.extend(section(5, &mem));

        let report = analyze_wasm(&bytes).unwrap();
        assert_eq!(report.info.num_func_imports, 1);
        assert_eq!(report.info.import_modules, vec!["env".to_string()]);
        assert_eq!(report.info.memory_pages_min, Some(17));
        assert_eq!(report.info.memory_pages_max, None);
    }

    #[test]
    fn truncated_section_is_an_error_not_a_panic() {
        let mut bytes = header();
        bytes.push(7); // export section id
        bytes.extend(leb(50)); // claims 50 bytes...
        bytes.extend_from_slice(&[0x01, 0x02]); // ...but only 2 present
        assert!(matches!(
            analyze_wasm(&bytes).unwrap_err(),
            WasmError::Malformed(_)
        ));
    }

    #[test]
    fn limitations_are_documented() {
        assert!(!WasmReport::limitations().is_empty());
    }
}
