/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Diagnostic snapshots for import-cycle analysis, carried with transaction state.

use std::collections::HashMap;
use std::sync::Arc;

use dupe::OptionDupedExt;
use pyrefly_build::handle::Handle;

use crate::error::collector::ModuleDiagnostics;

/// The result of the last import-cycle analysis.
#[derive(Debug, Clone, Default)]
pub struct ImportCycles {
    /// The `import-cycle` diagnostics, stored on the module that reports each cycle.
    errors: Arc<HashMap<Handle, ModuleDiagnostics>>,
}

impl ImportCycles {
    pub fn errors(&self, handle: &Handle) -> Option<ModuleDiagnostics> {
        self.errors.get(handle).duped()
    }
}
