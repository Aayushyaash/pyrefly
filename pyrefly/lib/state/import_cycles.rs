/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Syntactic import-graph collection, independent of bindings and type solving.
//!
//! Reading the first-party import closure may load and parse additional modules, so this
//! work is skipped unless a checked module enables `import-cycle`. Source import targets
//! are cached with the load they came from and resolved in the current transaction.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use dupe::Dupe;
use dupe::OptionDupedExt;
use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::module_path::ModulePathDetails;
use pyrefly_util::task_heap::Cancelled;
use pyrefly_util::thread_pool::ThreadPool;
use rayon::prelude::*;
use ruff_python_ast::Expr;
use ruff_python_ast::ModModule;
use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

use crate::config::config::ConfigFile;
use crate::config::error_kind::ErrorKind;
use crate::error::collector::ErrorCollector;
use crate::error::collector::ModuleDiagnostics;
use crate::error::style::ErrorStyle;
use crate::module::module_info::ModuleInfo;
use crate::module::parse::module_parse;
use crate::state::load::Load;
use crate::state::state::Transaction;

/// A module named by an import statement, with the range at which to report importing it.
#[derive(Debug, Clone)]
struct ImportTarget {
    module: ModuleName,
    range: TextRange,
}

/// The import targets of a module's source, kept with the `Load` they were read from.
#[derive(Debug, Clone)]
struct ModuleImports {
    load: Arc<Load>,
    targets: Arc<[ImportTarget]>,
}

/// A module of the import graph, with the modules its imports resolve to.
struct Node {
    imports: ModuleImports,
    edges: Vec<(Handle, TextRange)>,
}

/// The result of the last import-cycle analysis.
#[derive(Debug, Clone, Default)]
pub struct ImportCycles {
    /// The import targets of each module that the analysis visited. A module whose `Load` is
    /// unchanged has unchanged source, so the next analysis reuses its targets.
    imports: Arc<HashMap<Handle, ModuleImports>>,
    /// The `import-cycle` diagnostics, stored on the module that reports each cycle.
    errors: Arc<HashMap<Handle, ModuleDiagnostics>>,
}

impl ImportCycles {
    pub fn errors(&self, handle: &Handle) -> Option<ModuleDiagnostics> {
        self.errors.get(handle).duped()
    }

    /// The analysis for `transaction` after a completed run, reusing the import targets of
    /// modules whose source has not changed since this analysis.
    ///
    /// Only finding the modules that report `import-cycle` scans the whole transaction. When
    /// there are none, as under the default configuration, the analysis does no graph work and
    /// keeps no results.
    pub fn update(&self, transaction: &Transaction, pool: &ThreadPool) -> Result<Self, Cancelled> {
        let reporters: HashSet<Handle> = transaction
            .handles()
            .into_iter()
            .filter(|handle| reports(transaction, handle))
            .collect();
        if reporters.is_empty() {
            return Ok(Self::default());
        }
        let cancellation = transaction.get_cancellation_handle();
        let check_cancelled = || {
            if cancellation.is_cancelled() {
                Err(Cancelled)
            } else {
                Ok(())
            }
        };
        let graph = self.graph(transaction, &reporters, pool, &check_cancelled)?;

        Ok(Self {
            imports: Arc::new(
                graph
                    .into_iter()
                    .map(|(handle, node)| (handle, node.imports))
                    .collect(),
            ),
            errors: Arc::new(HashMap::new()),
        })
    }

    /// The import graph of the source modules reachable from `reporters`. Each breadth-first
    /// layer is read in parallel, since reading a module's imports may parse it.
    fn graph(
        &self,
        transaction: &Transaction,
        reporters: &HashSet<Handle>,
        pool: &ThreadPool,
        check_cancelled: &(impl Fn() -> Result<(), Cancelled> + Sync),
    ) -> Result<HashMap<Handle, Node>, Cancelled> {
        let mut graph = HashMap::new();
        let mut queued: HashSet<Handle> = reporters.clone();
        let mut layer: Vec<Handle> = reporters.iter().cloned().collect();
        while !layer.is_empty() {
            let nodes: Vec<(Handle, Node)> = pool.install(|| {
                layer
                    .into_par_iter()
                    .map(|handle| {
                        check_cancelled()?;
                        let node = self.node(transaction, &handle);
                        Ok((handle, node))
                    })
                    .collect::<Result<_, _>>()
            })?;
            layer = Vec::new();
            for (handle, node) in nodes {
                for (target, _) in &node.edges {
                    if queued.insert(target.dupe()) {
                        layer.push(target.dupe());
                    }
                }
                graph.insert(handle, node);
            }
        }
        Ok(graph)
    }

    fn node(&self, transaction: &Transaction, handle: &Handle) -> Node {
        let load = transaction.get_or_load(handle);
        let imports = match self.imports.get(handle) {
            Some(imports) if Arc::ptr_eq(&imports.load, &load) => imports.clone(),
            _ => {
                let targets = match transaction.get_ast(handle) {
                    Some(ast) => import_targets(&ast, &load.module_info),
                    None => {
                        let (ast, _, _) = module_parse(
                            load.module_info.contents(),
                            handle.sys_info().version(),
                            load.module_info.source_type(),
                            &ErrorCollector::new(load.module_info.dupe(), ErrorStyle::Never),
                            false,
                        );
                        import_targets(&ast, &load.module_info)
                    }
                };
                ModuleImports { load, targets }
            }
        };
        let config = transaction
            .get_config(handle)
            .expect("a loaded module has a config");
        let edges = imports
            .targets
            .iter()
            .filter_map(|target| {
                let imported = transaction
                    .import_handle(handle, target.module, None)
                    .finding()?;
                if !is_first_party_source(&config, handle, &imported) {
                    return None;
                }
                // An open file is checked as an in-memory module, while imports resolve to the
                // file on disk, so an import of an open file reaches the in-memory module that this
                // transaction holds for it, if that module is in the importer's environment.
                let imported = match imported.path().details() {
                    ModulePathDetails::FileSystem(path) if transaction.has_memory(path) => {
                        let open = imported
                            .for_module(imported.module(), ModulePath::memory((**path).clone()))
                            .with_importer_site_package_path(imported.importer_site_package_path());
                        if transaction.get_load(&open).is_some() {
                            open
                        } else {
                            imported
                        }
                    }
                    _ => imported,
                };
                (imported != *handle).then_some((imported, target.range))
            })
            .collect();
        Node { imports, edges }
    }
}

/// Whether `handle`, imported by `importer` whose config is `config`, can be part of an import
/// cycle: first-party source, rather than a stub or a module installed in a site-package path of
/// the Python environment that `importer` is checked in.
fn is_first_party_source(config: &ConfigFile, importer: &Handle, handle: &Handle) -> bool {
    let path = handle.path();
    let environment = importer.importer_site_package_path();
    !path.is_interface()
        && !path.is_bundled()
        && !config.is_third_party_path(
            path.as_path(),
            environment.as_ref().map(|environment| environment.paths()),
        )
}

/// Whether `handle` is a checked first-party source module that reports `import-cycle`.
fn reports(transaction: &Transaction, handle: &Handle) -> bool {
    transaction
        .get_load(handle)
        .is_some_and(|load| load.error_style != ErrorStyle::Never)
        && transaction.get_config(handle).is_some_and(|config| {
            is_first_party_source(&config, handle, handle)
                && config
                    .errors(handle.path())
                    .severity(ErrorKind::ImportCycle)
                    .is_enabled()
        })
}

/// The modules that the import statements of `ast` name, in source order.
fn import_targets(ast: &ModModule, module_info: &ModuleInfo) -> Arc<[ImportTarget]> {
    struct ImportTargets<'a> {
        module_info: &'a ModuleInfo,
        targets: Vec<ImportTarget>,
    }

    impl<'a> Visitor<'a> for ImportTargets<'_> {
        fn visit_stmt(&mut self, stmt: &'a Stmt) {
            match stmt {
                Stmt::Import(x) => {
                    for alias in &x.names {
                        let module = ModuleName::from_name(&alias.name.id);
                        let range = alias.name.range;
                        let name = module.as_str();
                        for (end, _) in name.match_indices('.') {
                            self.targets.push(ImportTarget {
                                module: ModuleName::from_str(&name[..end]),
                                range,
                            });
                        }
                        self.targets.push(ImportTarget { module, range });
                    }
                }
                Stmt::ImportFrom(x) => {
                    let Some(base) = self.module_info.name().new_maybe_relative(
                        self.module_info.path().is_init(),
                        x.level,
                        x.module.as_ref().map(|module| &module.id),
                    ) else {
                        return;
                    };
                    self.targets.push(ImportTarget {
                        module: base,
                        range: x.module.as_ref().map_or(x.range, |module| module.range()),
                    });
                    for alias in &x.names {
                        if &alias.name != "*" {
                            self.targets.push(ImportTarget {
                                module: base.append(&alias.name.id),
                                range: alias.name.range,
                            });
                        }
                    }
                }
                _ => walk_stmt(self, stmt),
            }
        }

        // Import statements never appear inside expressions.
        fn visit_expr(&mut self, _: &'a Expr) {}
    }

    let mut visitor = ImportTargets {
        module_info,
        targets: Vec::new(),
    };
    visitor.visit_body(&ast.body);
    visitor.targets.into()
}
