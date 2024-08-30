use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use autoken::cap;
use crucible_utils::{
    hash::FxHashMap,
    mem::{defuse, guard},
};
use lang_utils::{
    diagnostic::{
        report_diagnostic, Diagnostic, DiagnosticReporter, DiagnosticReporterCap, DiagnosticWindow,
    },
    span::{NaiveUtf8Segmenter, Span, SpanFile, SpanManager, SpanManagerCap},
    symbol::{Interner, InternerCap},
    tokens::TokenIdent,
};

use crate::{
    driver::parser::parse_directives,
    module::linker::{LinkerImport, LinkerImportError, ModuleHandle, ModuleLinker},
};

// === Language === //

// Core
// TODO: Stop double-validating
pub trait Language: 'static {
    fn emit(&mut self, module: &naga::Module) -> String;

    fn parse(
        &mut self,
        diags: &mut DiagnosticReporter,
        spans: &mut SpanManager,
        file: SpanFile,
        text_and_stubs: &str,
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
        diags: &mut DiagnosticReporter,
        spans: &mut SpanManager,
        file: SpanFile,
        text_and_stubs: &str,
    ) -> Option<naga::Module> {
        let file_len = spans.file_text(file).len();

        let module = match naga::front::wgsl::parse_str(text_and_stubs) {
            Ok(module) => module,
            Err(err) => {
                let mut diag = Diagnostic::new_err(err.message().to_string());

                for (i, (label_span, label_msg)) in err.labels().enumerate() {
                    let Some(label_span) = label_span
                        .to_range()
                        .filter(|label_span| label_span.end > file_len)
                        .map(|label_span| spans.range_to_span(file, label_span))
                    else {
                        // TODO: Handle these
                        continue;
                    };

                    if i == 0 {
                        diag.offending_span = Some(label_span);
                    }

                    diag.windows.push(DiagnosticWindow {
                        span: label_span,
                        label: Some(label_msg.to_string()),
                    });
                }

                // TODO: Include notes

                diags.report(diag);
                return None;
            }
        };

        if let Err(err) = self.validator.validate(&module) {
            let mut diag = Diagnostic::new_err(err.as_inner().to_string());

            for (span, label) in err.spans() {
                let span = span.to_range().unwrap();
                let span = spans.range_to_span(file, span);

                diag.windows.push(DiagnosticWindow {
                    span,
                    label: Some(label.clone()),
                });
            }

            let mut iter = err.source();
            while let Some(curr) = iter {
                diag.subs.push(Diagnostic::new_note(curr.to_string()));
                iter = curr.source();
            }

            diags.report(diag);
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

    pub fn span_mgr(&self) -> &SpanManager {
        &self.services.span_mgr
    }

    pub fn linker(&self) -> &ModuleLinker {
        &self.linker
    }

    pub fn linker_and_language(&mut self) -> (&mut dyn Language, &ModuleLinker) {
        (&mut *self.language, &self.linker)
    }

    pub fn parse(&mut self, path: &Path) -> Result<ModuleHandle, DiagnosticReporter> {
        let mut diag = DiagnosticReporter::new();

        let module = self.ensure_imported(&mut diag, None, path);

        if !diag.has_errors() {
            Ok(module.unwrap())
        } else {
            Err(diag)
        }
    }

    pub fn build(&mut self, modules: impl IntoIterator<Item = ModuleHandle>) -> String {
        self.language.emit(&self.linker.shake_module(modules))
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
                                    "failed to read shader at {:?}: {err}",
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
                let abs_path = path
                    .ancestors()
                    .nth(1)
                    .unwrap_or(path)
                    .join(rel_path.inner.as_str());

                let abs_path = match abs_path.canonicalize() {
                    Ok(abs_path) => abs_path,
                    Err(err) => {
                        report_diagnostic(Diagnostic::opt_span_err(
                            Some(rel_path.span),
                            format!(
                                "failed to read shader at {:?}: {err}",
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
                    imports.iter().map(move |(base, target)| LinkerImport {
                        file: module,
                        orig_name: interner.lookup(base.text),
                        rename_to: target.map(|target| interner.lookup(target.text)),
                        meta: (*base, *target),
                    })
                }),
            |err| match err {
                LinkerImportError::UnknownImport(directive) => {
                    diag.report(Diagnostic::span_err(
                        directive.meta.0.span,
                        format!("shader does not export {:?}", directive.orig_name),
                    ));
                }
                LinkerImportError::DuplicateSources(first, second) => {
                    diag.report(
                        Diagnostic::span_err(
                            second.meta.0.span,
                            format!(
                                "shader export {:?} was imported two different times",
                                first.orig_name
                            ),
                        )
                        .with_window(first.meta.0.span, Some("first import")),
                    );
                }
                LinkerImportError::DuplicateDestinations(first, second) => {
                    diag.report(
                        Diagnostic::span_err(
                            second.meta.1.map_or(second.meta.0.span, |v| v.span),
                            format!(
                                "shader exports were imported with the same name {:?} two different times",
                                second.rename_to.unwrap_or(second.orig_name),
                            ),
                        )
                        .with_window(first.meta.1.map_or(first.meta.0.span, |v| v.span), Some("first import")),
                    );
                },
            },
        );

        let mut output = String::new();
        output.push_str(me.services.span_mgr.file_text(file));
        output.push_str("\n\n// === Stubs === //\n\n");
        output.push_str(&stubs.apply_names_to_stub(me.language.emit(stubs.module())));

        let module = {
            let me = &mut *me;
            let module = me
                .language
                .parse(diag, &mut me.services.span_mgr, file, &output)?;

            me.linker.link(module, &stubs)
        };

        defuse(me);
        *self.files.get_mut(path).unwrap() = ModuleLoadStatus::Loaded(module);
        Some(module)
    }
}
