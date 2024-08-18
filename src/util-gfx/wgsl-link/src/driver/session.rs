use std::{
    fs,
    path::{Path, PathBuf},
};

use autoken::cap;
use crucible_utils::{
    hash::FxHashMap,
    mem::{defuse, guard},
};
use lang_utils::{
    diagnostic::{report_diagnostic, Diagnostic, DiagnosticReporter, DiagnosticReporterCap},
    span::{NaiveUtf8Segmenter, Span, SpanFile, SpanManager, SpanManagerCap},
    symbol::{Interner, InternerCap},
    tokens::TokenIdent,
};

use crate::{
    driver::parser::parse_directives,
    module::linker::{ModuleHandle, ModuleLinker},
};

// === Language === //

// Core
pub trait Language {
    fn emit(&mut self, module: &naga::Module) -> String;

    fn parse(
        &mut self,
        diag: &mut DiagnosticReporter,
        spans: &mut SpanManager,
        file: SpanFile,
        text: &str,
    ) -> Option<naga::Module>;
}

// Wgsl
pub struct Wgsl {
    validator: naga::valid::Validator,
}

impl Default for Wgsl {
    fn default() -> Self {
        Self {
            validator: naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            ),
        }
    }
}

impl Language for Wgsl {
    fn emit(&mut self, module: &naga::Module) -> String {
        naga::back::wgsl::write_string(
            module,
            &self.validator.validate(module).unwrap(),
            naga::back::wgsl::WriterFlags::all(),
        )
        .unwrap()
    }

    fn parse(
        &mut self,
        diag: &mut DiagnosticReporter,
        spans: &mut SpanManager,
        file: SpanFile,
        text: &str,
    ) -> Option<naga::Module> {
        // TODO: Improve diagnostic spans
        let module = match naga::front::wgsl::parse_str(text) {
            Ok(module) => module,
            Err(err) => {
                diag.report(Diagnostic::span_err(
                    spans.file_span(file),
                    format!("failed to parse WGSL module: {}", err.message()),
                ));
                return None;
            }
        };

        if let Err(err) = self.validator.validate(&module) {
            diag.report(Diagnostic::span_err(
                spans.file_span(file),
                format!(
                    "failed to validate WGSL module: {}",
                    err.emit_to_string(spans.file_name(file)),
                ),
            ));
            return None;
        }

        Some(module)
    }
}

// === Session === //

pub struct Session {
    language: Box<dyn Language>,
    services: SessionServices,
    linker: ModuleLinker,
    files: FxHashMap<PathBuf, ModuleLoadStatus>,
}

#[derive(Debug, Copy, Clone)]
enum ModuleLoadStatus {
    Loaded(ModuleHandle),
    Loading,
    Failed,
}

#[derive(Debug, Default)]
struct SessionServices {
    interner: Interner,
    span_mgr: SpanManager,
}

impl SessionServices {
    fn bind<R>(&mut self, diag: &mut DiagnosticReporter, f: impl FnOnce() -> R) -> R {
        cap! {
            DiagnosticReporterCap: diag,
            InternerCap: &mut self.interner,
            SpanManagerCap: &mut self.span_mgr,
        =>
            f()
        }
    }
}

impl Session {
    pub fn new(language: impl 'static + Language) -> Self {
        Self {
            language: Box::new(language),
            linker: ModuleLinker::new(),
            services: SessionServices::default(),
            files: FxHashMap::default(),
        }
    }

    pub fn link(&mut self, path: &Path) -> String {
        let mut diag = DiagnosticReporter::new();

        self.ensure_imported(&mut diag, None, path);

        // TODO: Emit useful diagnostic messages
        dbg!(diag);

        // TODO: Perform tree-shaking
        self.language.emit(self.linker.full_module())
    }

    fn ensure_imported(
        &mut self,
        diag: &mut DiagnosticReporter,
        origin: Option<Span>,
        path: &Path,
    ) -> Option<ModuleHandle> {
        // (path is assumed to be canonicalized as part of the name resolution procedure)

        // Ensure that the file is not already in the process of being loaded.
        if let Some(module) = self.files.get_mut(path) {
            return match module {
                ModuleLoadStatus::Loaded(module) => Some(*module),
                ModuleLoadStatus::Loading => {
                    *module = ModuleLoadStatus::Failed;
                    diag.report(Diagnostic::opt_span_err(origin, "attempted to import a shader module which recursively depends upon the current module"));
                    None
                }
                ModuleLoadStatus::Failed => None,
            };
        }

        self.files
            .insert(path.to_owned(), ModuleLoadStatus::Loading);

        let mut me = guard(&mut *self, |me| {
            *me.files.get_mut(path).unwrap() = ModuleLoadStatus::Failed;
        });

        // Load the file's source.
        let file = me
            .services
            .span_mgr
            .load(
                &mut NaiveUtf8Segmenter { tab_size: 4 },
                &path.to_string_lossy(),
                |buf| {
                    let file = match fs::read_to_string(path) {
                        Ok(file) => file,
                        Err(err) => {
                            diag.report(Diagnostic::opt_span_err(
                                origin,
                                format!(
                                    "error ocurred while attempting to read shader from {}: {err}",
                                    path.to_string_lossy()
                                ),
                            ));
                            anyhow::bail!("");
                        }
                    };

                    buf.push_str(&file);

                    Ok(())
                },
            )
            .ok()?;

        // Parse its import directives.
        #[derive(Debug)]
        struct ImportDirective {
            span: Span,
            imports: Vec<(TokenIdent, Option<TokenIdent>)>,
            abs_path: PathBuf,
            module: Option<ModuleHandle>,
        }

        let mut directives = Vec::new();

        me.services.bind(diag, || {
            parse_directives(file.span(), |rel_path, imports| {
                let abs_path = path.ancestors().nth(1).unwrap().join(rel_path.inner.as_str());
                let abs_path = match abs_path.canonicalize() {
                    Ok(abs_path) => abs_path,
                    Err(err) => {
                        report_diagnostic(Diagnostic::opt_span_err(
                            origin,
                            format!(
                                "error ocurred while attempting to canonicalize shader path at {}: {err}",
                                abs_path.to_string_lossy(),
                            ),
                        ));
                        return;
                    }
                };

                directives.push(ImportDirective {
                    span: rel_path.span,
                    imports: imports.to_owned(),
                    abs_path,
                    module: None,
                });
            });
        });

        // Ensure that all its source files are imported.
        for directive in &mut directives {
            directive.module = me.ensure_imported(diag, Some(directive.span), &directive.abs_path);
        }

        // Link the modules.
        let interner = &me.services.interner;
        let stubs = me.linker.gen_stubs(
            directives
                .iter()
                // Filter in directives with modules associated
                .filter_map(|directive| {
                    directive
                        .module
                        .map(|module| (module, directive.imports.as_slice()))
                })
                // Flatten their imports
                .flat_map(|(module, imports)| {
                    imports.iter().map(move |(base, target)| {
                        (
                            module,
                            interner.lookup(base.text),
                            target.map(|target| interner.lookup(target.text)),
                        )
                    })
                }),
        );

        let mut output = String::new();
        output.push_str(
            me.services
                .span_mgr
                .span_text(me.services.span_mgr.file_span(file)),
        );
        output.push_str("\n\n// === Stubs === //\n\n");
        output.push_str(&stubs.apply_names_to_stub(me.language.emit(stubs.module())));

        let module = {
            let me = &mut *me;
            me.language
                .parse(diag, &mut me.services.span_mgr, file, &output)?
        };

        let module = me.linker.link(module, &stubs, 0);

        defuse(me);
        *self.files.get_mut(path).unwrap() = ModuleLoadStatus::Loaded(module);
        Some(module)
    }
}
