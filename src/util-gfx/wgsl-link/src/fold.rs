use std::any::{type_name, Any};

use naga::{Arena, Handle, Span};

use crate::merge::{ArenaMerger, RawNagaHandle};

// === Helpers === //

fn map_naga_range<T>(
    range: naga::Range<T>,
    f: impl FnOnce(Handle<T>, Handle<T>) -> (Handle<T>, Handle<T>),
) -> naga::Range<T> {
    range.first_and_last().map_or(
        naga::Range::new_from_bounds(
            RawNagaHandle::default().as_typed(),
            RawNagaHandle::default().as_typed(),
        ),
        |(l, r)| {
            let (l, r) = f(l, r);
            naga::Range::new_from_bounds(l, r)
        },
    )
}

// === Foldable Core === //

// Trait definitions
pub trait Folder<T> {
    fn fold(&self, value: T) -> T;

    fn fold_opt(&self, value: Option<T>) -> Option<T> {
        value.map(|v| self.fold(v))
    }

    fn fold_collection<C>(&self, collection: C) -> C
    where
        C: IntoIterator<Item = T> + FromIterator<T>,
    {
        collection.into_iter().map(|v| self.fold(v)).collect()
    }
}

impl<F, T> Folder<T> for F
where
    F: Fn(T) -> T,
{
    fn fold(&self, value: T) -> T {
        self(value)
    }
}

pub trait Foldable<F> {
    fn fold(self, f: &F) -> Self;
}

pub trait FolderExt: Sized {
    fn upcast<T>(&self) -> &impl Folder<T>
    where
        Self: Folder<T>,
    {
        self
    }
}

impl<T> FolderExt for T {}

// CompositeFolder
pub struct CompositeFolder<F>(pub F)
where
    F: Fn(&mut FoldReq<'_>);

impl<F, T: 'static> Folder<T> for CompositeFolder<F>
where
    F: Fn(&mut FoldReq<'_>),
{
    fn fold(&self, value: T) -> T {
        let mut value = Some(value);
        let mut req = FoldReq {
            value: &mut value,
            processed: false,
        };

        self.0(&mut req);

        assert!(
            req.processed,
            "no folder specified for `{}`",
            type_name::<T>()
        );
        value.unwrap()
    }
}

pub struct FoldReq<'a> {
    value: &'a mut dyn Any,
    processed: bool,
}

impl FoldReq<'_> {
    pub fn fold<T: 'static>(&mut self, f: &(impl ?Sized + Folder<T>)) -> &mut Self {
        if let Some(target) = self.value.downcast_mut::<Option<T>>() {
            assert!(
                !self.processed,
                "more than one folder specified for `{}`",
                type_name::<T>()
            );
            *target = Some(f.fold(target.take().unwrap()));
            self.processed = true;
        }

        self
    }
}

// Macro
macro_rules! folders {
    ($($name:ident: $folder:expr),+$(,)?) => {{
        #[allow(non_camel_case_types)]
        fn produce<'a, $($name: 'static),*>($($name: &'a (impl ?Sized + $crate::fold::Folder<$name>),)*)
            -> impl 'a $(+ $crate::fold::Folder<$name>)*
        {
            $crate::fold::CompositeFolder(|req| {
                req $(.fold($name))* ;
            })
        }

        produce($($folder),*)
    }};
}

pub(crate) use folders;

// === Foldable Implementations === //

impl<F> Foldable<F> for naga::Type
where
    F: Folder<Handle<naga::Type>> + Folder<Span>,
{
    fn fold(self, f: &F) -> Self {
        use naga::TypeInner::*;

        naga::Type {
            name: self.name,
            inner: match self.inner {
                value @ Scalar(_)
                | value @ Vector { .. }
                | value @ Matrix { .. }
                | value @ Atomic(_)
                | value @ ValuePointer { .. }
                | value @ Image { .. }
                | value @ Sampler { .. }
                | value @ AccelerationStructure
                | value @ RayQuery => value,
                Pointer { base, space } => Pointer {
                    base: f.fold(base),
                    space,
                },
                Array { base, size, stride } => Array {
                    base: f.fold(base),
                    size,
                    stride,
                },
                Struct { members, span } => Struct {
                    members: members
                        .into_iter()
                        .map(|member| naga::StructMember {
                            name: member.name,
                            ty: f.fold(member.ty),
                            binding: member.binding,
                            offset: member.offset,
                        })
                        .collect(),
                    // TODO: How do we map this??
                    span,
                },
                BindingArray { base, size } => BindingArray {
                    base: f.fold(base),
                    size,
                },
            },
        }
    }
}

impl<F> Foldable<F> for naga::Constant
where
    F: Folder<Handle<naga::Type>> + Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        naga::Constant {
            name: self.name,
            ty: f.fold(self.ty),
            init: f.fold(self.init),
        }
    }
}

impl<F> Foldable<F> for naga::Override
where
    F: Folder<Handle<naga::Type>> + Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        naga::Override {
            name: self.name,
            id: self.id,
            ty: f.fold(self.ty),
            init: f.fold_opt(self.init),
        }
    }
}

impl<F> Foldable<F> for naga::GlobalVariable
where
    F: Folder<Handle<naga::Type>> + Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        naga::GlobalVariable {
            name: self.name,
            space: self.space,
            binding: self.binding,
            ty: f.fold(self.ty),
            init: self.init.map(|expr| f.fold(expr)),
        }
    }
}

impl<F> Foldable<F> for naga::Function
where
    F: Folder<Handle<naga::Constant>>
        + Folder<Handle<naga::Override>>
        + Folder<Handle<naga::Type>>
        + Folder<Handle<naga::GlobalVariable>>
        + Folder<Handle<naga::Function>>
        + Folder<Span>,
{
    fn fold(self, f: &F) -> Self {
        // Map arenas
        let mut local_variables_arena = Arena::new();
        let mut local_variables =
            ArenaMerger::new(&mut local_variables_arena, self.local_variables, |_, _| {
                None
            });

        let mut expressions_arena = Arena::new();
        let mut expressions =
            ArenaMerger::new(&mut expressions_arena, self.expressions, |_, _| None);

        expressions.apply(|expressions, _orig_handle, span, expr| {
            (
                f.fold(span),
                expr.fold(&folders!(
                    a: f.upcast::<Handle<naga::Constant>>(),
                    b: f.upcast::<Handle<naga::Override>>(),
                    c: f.upcast::<Handle<naga::Type>>(),
                    d: f.upcast::<Handle<naga::GlobalVariable>>(),
                    e: f.upcast::<Handle<naga::Function>>(),
                    f: expressions,
                    g: &local_variables,
                )),
            )
        });

        local_variables.apply(|_local_variables, _orig_handle, span, var| {
            (
                span,
                naga::LocalVariable {
                    name: var.name,
                    ty: f.fold(var.ty),
                    init: expressions.fold_opt(var.init),
                },
            )
        });

        // Map other collections
        let arguments = self.arguments.into_iter().map(|arg| arg.fold(f)).collect();
        let result = self.result.map(|res| res.fold(f));

        let named_expressions = self
            .named_expressions
            .into_iter()
            .map(|(expr, name)| (expressions.fold(expr), name))
            .collect();

        let body = self.body.fold(&folders!(
            a: f.upcast::<Span>(),
            b: f.upcast::<Handle<naga::Function>>(),
            f: &expressions,
            g: &local_variables,
        ));

        // Map functions
        naga::Function {
            name: self.name,
            arguments,
            result,
            local_variables: local_variables_arena,
            expressions: expressions_arena,
            named_expressions,
            body,
        }
    }
}

impl<F> Foldable<F> for naga::FunctionArgument
where
    F: Folder<Handle<naga::Type>>,
{
    fn fold(self, f: &F) -> Self {
        naga::FunctionArgument {
            name: self.name,
            ty: f.fold(self.ty),
            binding: self.binding,
        }
    }
}

impl<F> Foldable<F> for naga::FunctionResult
where
    F: Folder<Handle<naga::Type>>,
{
    fn fold(self, f: &F) -> Self {
        naga::FunctionResult {
            ty: f.fold(self.ty),
            binding: self.binding,
        }
    }
}

impl<F> Foldable<F> for naga::Expression
where
    F: Folder<Handle<naga::Constant>>
        + Folder<Handle<naga::Override>>
        + Folder<Handle<naga::Type>>
        + Folder<Handle<naga::Expression>>
        + Folder<Handle<naga::GlobalVariable>>
        + Folder<Handle<naga::LocalVariable>>
        + Folder<Handle<naga::Function>>,
{
    fn fold(self, f: &F) -> Self {
        use naga::Expression::*;

        match self {
            v @ Literal(_)
            | v @ FunctionArgument(_)
            | v @ RayQueryProceedResult
            | v @ SubgroupBallotResult => v,
            Constant(val) => Constant(f.fold(val)),
            Override(val) => Override(f.fold(val)),
            ZeroValue(val) => ZeroValue(f.fold(val)),
            Compose { ty, components } => Compose {
                ty: f.fold(ty),
                components: f.fold_collection(components),
            },
            Access { base, index } => Access {
                base: f.fold(base),
                index: f.fold(index),
            },
            AccessIndex { base, index } => AccessIndex {
                base: f.fold(base),
                index,
            },
            Splat { size, value } => Splat {
                size,
                value: f.fold(value),
            },
            Swizzle {
                size,
                vector,
                pattern,
            } => Swizzle {
                size,
                vector: f.fold(vector),
                pattern,
            },
            GlobalVariable(val) => GlobalVariable(f.fold(val)),
            LocalVariable(val) => LocalVariable(f.fold(val)),
            Load { pointer } => Load {
                pointer: f.fold(pointer),
            },
            ImageSample {
                image,
                sampler,
                gather,
                coordinate,
                array_index,
                offset,
                level,
                depth_ref,
            } => ImageSample {
                image: f.fold(image),
                sampler: f.fold(sampler),
                gather,
                coordinate: f.fold(coordinate),
                array_index: f.fold_opt(array_index),
                offset: f.fold_opt(offset),
                level: level.fold(f),
                depth_ref: f.fold_opt(depth_ref),
            },
            ImageLoad {
                image,
                coordinate,
                array_index,
                sample,
                level,
            } => ImageLoad {
                image: f.fold(image),
                coordinate: f.fold(coordinate),
                array_index: f.fold_opt(array_index),
                sample: f.fold_opt(sample),
                level: f.fold_opt(level),
            },
            ImageQuery { image, query } => ImageQuery {
                image: f.fold(image),
                query: query.fold(f),
            },
            Unary { op, expr } => Unary {
                op,
                expr: f.fold(expr),
            },
            Binary { op, left, right } => Binary {
                op,
                left: f.fold(left),
                right: f.fold(right),
            },
            Select {
                condition,
                accept,
                reject,
            } => Select {
                condition: f.fold(condition),
                accept: f.fold(accept),
                reject: f.fold(reject),
            },
            Derivative { axis, ctrl, expr } => Derivative {
                axis,
                ctrl,
                expr: f.fold(expr),
            },
            Relational { fun, argument } => Relational {
                fun,
                argument: f.fold(argument),
            },
            Math {
                fun,
                arg,
                arg1,
                arg2,
                arg3,
            } => Math {
                fun,
                arg: f.fold(arg),
                arg1: f.fold_opt(arg1),
                arg2: f.fold_opt(arg2),
                arg3: f.fold_opt(arg3),
            },
            As {
                expr,
                kind,
                convert,
            } => As {
                expr: f.fold(expr),
                kind,
                convert,
            },
            CallResult(val) => CallResult(f.fold(val)),
            AtomicResult { ty, comparison } => AtomicResult {
                ty: f.fold(ty),
                comparison,
            },
            WorkGroupUniformLoadResult { ty } => WorkGroupUniformLoadResult { ty: f.fold(ty) },
            ArrayLength(val) => ArrayLength(f.fold(val)),

            RayQueryGetIntersection { query, committed } => RayQueryGetIntersection {
                query: f.fold(query),
                committed,
            },
            SubgroupOperationResult { ty } => SubgroupOperationResult { ty: f.fold(ty) },
        }
    }
}

impl<F> Foldable<F> for naga::SampleLevel
where
    F: Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        use naga::SampleLevel::*;

        match self {
            v @ Auto | v @ Zero => v,
            Exact(val) => Exact(f.fold(val)),
            Bias(val) => Bias(f.fold(val)),
            Gradient { x, y } => Gradient {
                x: f.fold(x),
                y: f.fold(y),
            },
        }
    }
}

impl<F> Foldable<F> for naga::ImageQuery
where
    F: Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        use naga::ImageQuery::*;

        match self {
            Size { level } => Size {
                level: f.fold_opt(level),
            },
            v @ NumLevels | v @ NumLayers | v @ NumSamples => v,
        }
    }
}

impl<F> Foldable<F> for naga::Block
where
    F: Folder<Handle<naga::Expression>> + Folder<Handle<naga::Function>> + Folder<Span>,
{
    fn fold(self, f: &F) -> Self {
        let mut body = naga::Block::new();

        for (stmt, span) in self.span_into_iter() {
            body.push(stmt.fold(f), f.fold(span));
        }

        body
    }
}

impl<F> Foldable<F> for naga::Statement
where
    F: Folder<Handle<naga::Expression>> + Folder<Handle<naga::Function>> + Folder<Span>,
{
    fn fold(self, f: &F) -> Self {
        use naga::Statement::*;

        match self {
            v @ Break | v @ Continue | v @ Kill | v @ Barrier(_) => v,
            Emit(range) => Emit(map_naga_range(range, |l, r| (f.fold(l), f.fold(r)))),
            Block(block) => Block(block.fold(f)),
            If {
                condition,
                accept,
                reject,
            } => If {
                condition: f.fold(condition),
                accept: accept.fold(f),
                reject: reject.fold(f),
            },
            Switch { selector, cases } => Switch {
                selector: f.fold(selector),
                cases: cases.into_iter().map(|v| v.fold(f)).collect(),
            },
            Loop {
                body,
                continuing,
                break_if,
            } => Loop {
                body: body.fold(f),
                continuing: continuing.fold(f),
                break_if: f.fold_opt(break_if),
            },
            Return { value } => Return {
                value: f.fold_opt(value),
            },
            Store { pointer, value } => Store {
                pointer: f.fold(pointer),
                value: f.fold(value),
            },
            ImageStore {
                image,
                coordinate,
                array_index,
                value,
            } => ImageStore {
                image: f.fold(image),
                coordinate: f.fold(coordinate),
                array_index: f.fold_opt(array_index),
                value: f.fold(value),
            },
            Atomic {
                pointer,
                fun,
                value,
                result,
            } => Atomic {
                pointer: f.fold(pointer),
                fun: fun.fold(f),
                value: f.fold(value),
                result: f.fold_opt(result),
            },
            WorkGroupUniformLoad { pointer, result } => WorkGroupUniformLoad {
                pointer: f.fold(pointer),
                result: f.fold(result),
            },
            Call {
                function,
                arguments,
                result,
            } => Call {
                function: f.fold(function),
                arguments: f.fold_collection(arguments),
                result: f.fold_opt(result),
            },
            RayQuery { query, fun } => RayQuery {
                query: f.fold(query),
                fun: fun.fold(f),
            },
            SubgroupBallot { result, predicate } => SubgroupBallot {
                result: f.fold(result),
                predicate: f.fold_opt(predicate),
            },
            SubgroupGather {
                mode,
                argument,
                result,
            } => SubgroupGather {
                mode: mode.fold(f),
                argument: f.fold(argument),
                result: f.fold(result),
            },
            SubgroupCollectiveOperation {
                op,
                collective_op,
                argument,
                result,
            } => SubgroupCollectiveOperation {
                op,
                collective_op,
                argument: f.fold(argument),
                result: f.fold(result),
            },
        }
    }
}

impl<F> Foldable<F> for naga::SwitchCase
where
    F: Folder<Handle<naga::Expression>> + Folder<Handle<naga::Function>> + Folder<Span>,
{
    fn fold(self, f: &F) -> Self {
        naga::SwitchCase {
            value: self.value,
            body: self.body.fold(f),
            fall_through: self.fall_through,
        }
    }
}

impl<F> Foldable<F> for naga::AtomicFunction
where
    F: Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        use naga::AtomicFunction::*;

        match self {
            v @ Add
            | v @ Subtract
            | v @ And
            | v @ ExclusiveOr
            | v @ InclusiveOr
            | v @ Min
            | v @ Max => v,
            Exchange { compare } => Exchange {
                compare: f.fold_opt(compare),
            },
        }
    }
}

impl<F> Foldable<F> for naga::RayQueryFunction
where
    F: Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        use naga::RayQueryFunction::*;

        match self {
            Initialize {
                acceleration_structure,
                descriptor,
            } => Initialize {
                acceleration_structure: f.fold(acceleration_structure),
                descriptor: f.fold(descriptor),
            },
            Proceed { result } => Proceed {
                result: f.fold(result),
            },
            Terminate => Terminate,
        }
    }
}

impl<F> Foldable<F> for naga::GatherMode
where
    F: Folder<Handle<naga::Expression>>,
{
    fn fold(self, f: &F) -> Self {
        use naga::GatherMode::*;

        match self {
            BroadcastFirst => BroadcastFirst,
            Broadcast(val) => Broadcast(f.fold(val)),
            Shuffle(val) => Shuffle(f.fold(val)),
            ShuffleDown(val) => ShuffleDown(f.fold(val)),
            ShuffleUp(val) => ShuffleUp(f.fold(val)),
            ShuffleXor(val) => ShuffleXor(f.fold(val)),
        }
    }
}
