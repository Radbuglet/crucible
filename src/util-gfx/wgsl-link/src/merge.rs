//! Utilities for merging `naga` arenas.

use std::{
    any::{type_name, Any},
    hash,
    num::NonZeroU32,
};

use naga::{Arena, Handle, Span, UniqueArena};

// === Helpers === //

fn handle_from_usize<T>(value: usize) -> Handle<T> {
    let value = u32::try_from(value)
        .ok()
        .filter(|&v| v != u32::MAX)
        .expect("handle too big");
    let value = NonZeroU32::new(value + 1).unwrap();

    unsafe {
        // FIXME: Safety: Nope. This is super unstable.
        std::mem::transmute(value)
    }
}

fn map_naga_range<T>(
    range: naga::Range<T>,
    f: impl FnOnce(Handle<T>, Handle<T>) -> (Handle<T>, Handle<T>),
) -> naga::Range<T> {
    range.first_and_last().map_or(
        naga::Range::new_from_bounds(handle_from_usize(0), handle_from_usize(0)),
        |(l, r)| {
            let (l, r) = f(l, r);
            naga::Range::new_from_bounds(l, r)
        },
    )
}

// === ArenaMerger === //

pub struct ArenaMerger<'a, T> {
    dest_arena: &'a mut Arena<T>,
    src_arena: Option<Arena<T>>,

    // The index of the first handle in destination arena imported from the source arena.
    dest_alloc_start: usize,

    // A map from source handles to destination handles.
    src_to_dest: Vec<Handle<T>>,

    // A map from destination handles—offset by `dest_alloc_start`—to source handles.
    dest_to_src: Vec<Handle<T>>,
}

impl<'a, T> ArenaMerger<'a, T> {
    pub fn new(
        dest_arena: &'a mut Arena<T>,
        src_arena: Arena<T>,
        mut dedup: impl FnMut(Span, &T) -> Option<Handle<T>>,
    ) -> Self {
        let dest_offset = dest_arena.len();
        let mut src_to_dest = Vec::with_capacity(src_arena.len());
        let mut dest_to_src = Vec::new();

        let mut index_alloc = dest_offset;

        for (src_handle, value) in src_arena.iter() {
            let span = src_arena.get_span(src_handle);

            if let Some(map_to) = dedup(span, value) {
                src_to_dest.push(map_to);
            } else {
                src_to_dest.push(handle_from_usize(index_alloc));
                dest_to_src.push(src_handle);
                index_alloc += 1;
            }
        }

        Self {
            dest_arena,
            src_arena: Some(src_arena),
            dest_alloc_start: dest_offset,
            src_to_dest,
            dest_to_src,
        }
    }

    pub fn src_to_dest(&self, src_handle: Handle<T>) -> Handle<T> {
        self.src_to_dest[src_handle.index()]
    }

    pub fn dest_to_src(&self, dest_handle: Handle<T>) -> Option<Handle<T>> {
        dest_handle
            .index()
            .checked_sub(self.dest_alloc_start)
            .map(|idx| self.dest_to_src[idx])
    }

    pub fn lookup_src(&self, src_handle: Handle<T>) -> &T {
        if let Some(src) = &self.src_arena {
            &src[src_handle]
        } else {
            &self.dest_arena[self.src_to_dest(src_handle)]
        }
    }

    pub fn lookup_src_mut(&mut self, src_handle: Handle<T>) -> &mut T {
        let dest_handle = self.src_to_dest(src_handle);

        if let Some(src) = &mut self.src_arena {
            &mut src[src_handle]
        } else {
            &mut self.dest_arena[dest_handle]
        }
    }

    pub fn lookup_dest(&self, dest_handle: Handle<T>) -> &T {
        let src_handle = self.dest_to_src(dest_handle);

        if let (Some(src_handle), Some(src)) = (src_handle, &self.src_arena) {
            &src[src_handle]
        } else {
            &self.dest_arena[dest_handle]
        }
    }

    pub fn lookup_dest_mut(&mut self, dest_handle: Handle<T>) -> &mut T {
        let src_handle = self.dest_to_src(dest_handle);

        if let (Some(src_handle), Some(src)) = (src_handle, &mut self.src_arena) {
            &mut src[src_handle]
        } else {
            &mut self.dest_arena[dest_handle]
        }
    }

    pub fn apply(&mut self, mut adjust: impl FnMut(&Self, Handle<T>, Span, T) -> (Span, T)) {
        let mut inserted_arena = self
            .src_arena
            .take()
            .expect("cannot call `apply` on a given `ArenaMerger` more than once");

        let mut included_handle_iter = self.dest_to_src.iter().copied().peekable();

        for (handle, value, span) in inserted_arena.drain() {
            if included_handle_iter.peek() == Some(&handle) {
                let _ = included_handle_iter.next();
            } else {
                continue;
            }

            let (span, value) = adjust(self, handle, span, value);
            self.dest_arena.append(value, span);
        }
    }
}

impl<T> Folder<Handle<T>> for ArenaMerger<'_, T> {
    fn fold(&self, value: Handle<T>) -> Handle<T> {
        self.src_to_dest(value)
    }
}

// === UniqueArenaMerger === //

pub struct UniqueArenaMerger<T> {
    map: Vec<Handle<T>>,
}

impl<T> UniqueArenaMerger<T> {
    pub fn new<M>(
        dest_arena: &mut UniqueArena<T>,
        src_arena: UniqueArena<T>,
        mut map: impl FnMut(&UniqueArenaMerger<T>, Span, T) -> (MapResult<T>, M),
        mut post_map: impl FnMut(&UniqueArenaMerger<T>, &UniqueArena<T>, M, Handle<T>, Handle<T>),
    ) -> Self
    where
        T: Eq + hash::Hash + Clone,
    {
        // In its current form, Naga never emits recursive types and, indeed, ensures that `UniqueArena`
        // is properly toposorted. Hence, this algorithm is safe.

        let mut mapper = UniqueArenaMerger { map: Vec::new() };

        for (src_handle, value) in src_arena.iter() {
            let span = src_arena.get_span(src_handle);
            let value = value.clone();

            let (map_res, map_meta) = map(&mapper, span, value);

            let dest_handle = match map_res {
                MapResult::Map(span, value) => dest_arena.insert(value, span),
                MapResult::Dedup(handle) => handle,
            };
            mapper.map.push(dest_handle);

            post_map(&mapper, dest_arena, map_meta, src_handle, dest_handle);
        }

        mapper
    }

    pub fn src_to_dest(&self, src_arena: Handle<T>) -> Handle<T> {
        self.map[src_arena.index()]
    }
}

impl<T> Folder<Handle<T>> for UniqueArenaMerger<T> {
    fn fold(&self, value: Handle<T>) -> Handle<T> {
        self.src_to_dest(value)
    }
}

pub enum MapResult<T> {
    Map(Span, T),
    Dedup(Handle<T>),
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
        fn produce<'a, $($name: 'static),*>($($name: &'a (impl ?Sized + $crate::merge::Folder<$name>),)*)
            -> impl 'a $(+ $crate::merge::Folder<$name>)*
        {
            $crate::merge::CompositeFolder(|req| {
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
        let arguments = self
            .arguments
            .into_iter()
            .map(|arg| naga::FunctionArgument {
                name: arg.name,
                ty: f.fold(arg.ty),
                binding: arg.binding,
            })
            .collect();

        let result = self.result.map(|res| naga::FunctionResult {
            ty: f.fold(res.ty),
            binding: res.binding,
        });

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
