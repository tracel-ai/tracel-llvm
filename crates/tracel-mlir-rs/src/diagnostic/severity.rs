#![allow(
    clippy::unnecessary_cast,
    reason = "Normalize bindgen constants to u32; bindgen may emit i32 on some targets."
)]

use crate::Error;
use tracel_mlir_sys::{
    MlirDiagnosticSeverity_MlirDiagnosticError, MlirDiagnosticSeverity_MlirDiagnosticNote,
    MlirDiagnosticSeverity_MlirDiagnosticRemark, MlirDiagnosticSeverity_MlirDiagnosticWarning,
};

/// Diagnostic severity.
#[derive(Clone, Copy, Debug)]
pub enum DiagnosticSeverity {
    Error,
    Note,
    Remark,
    Warning,
}

// Normalize bindgen constants to u32 (bindgen may emit i32 on some targets).
const SEV_ERROR: u32 = MlirDiagnosticSeverity_MlirDiagnosticError as u32;
const SEV_NOTE: u32 = MlirDiagnosticSeverity_MlirDiagnosticNote as u32;
const SEV_REMARK: u32 = MlirDiagnosticSeverity_MlirDiagnosticRemark as u32;
const SEV_WARN: u32 = MlirDiagnosticSeverity_MlirDiagnosticWarning as u32;

#[inline]
fn from_raw_u32(severity: u32) -> Result<DiagnosticSeverity, Error> {
    Ok(match severity {
        SEV_ERROR => DiagnosticSeverity::Error,
        SEV_NOTE => DiagnosticSeverity::Note,
        SEV_REMARK => DiagnosticSeverity::Remark,
        SEV_WARN => DiagnosticSeverity::Warning,
        _ => return Err(Error::UnknownDiagnosticSeverity(severity)),
    })
}

impl TryFrom<u32> for DiagnosticSeverity {
    type Error = Error;
    fn try_from(severity: u32) -> Result<Self, Error> {
        from_raw_u32(severity)
    }
}

impl TryFrom<i32> for DiagnosticSeverity {
    type Error = Error;
    fn try_from(severity: i32) -> Result<Self, Error> {
        if severity < 0 {
            return Err(Error::UnknownDiagnosticSeverity(severity as u32));
        }
        from_raw_u32(severity as u32)
    }
}
