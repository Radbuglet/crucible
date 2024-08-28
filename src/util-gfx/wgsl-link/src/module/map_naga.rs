use crate::module::map::{map_alias, map_collection, Map, MapCombinatorsExt as _};

use super::{
    map::{map_option, MapFn},
    merge::ArenaMerger,
};

// === Helpers === //

fn map_naga_range<T>(
    range: naga::Range<T>,
    f: impl FnOnce(naga::Handle<T>, naga::Handle<T>) -> (naga::Handle<T>, naga::Handle<T>),
) -> naga::Range<T> {
    match range.first_and_last() {
        // Everything here is done with inclusive ranges.
        Some((first, last)) => {
            let (first, last) = f(first, last);
            naga::Range::new_from_bounds(first, last)
        }
        // If we found an empty range, return it unchanged.
        None => range,
    }
}

// === Mappers === //

map_alias! {
    pub trait MapNagaType = naga::Handle<naga::Type>, naga::Span;
}

pub fn map_naga_type<D>(v: naga::Type, f: &impl MapNagaType<D>) -> naga::Type {
    use naga::TypeInner::*;

    naga::Type {
        name: v.name,
        inner: match v.inner {
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
                base: f.map(base),
                space,
            },
            Array { base, size, stride } => Array {
                base: f.map(base),
                size,
                stride,
            },
            Struct { members, span } => Struct {
                members: members
                    .into_iter()
                    .map(|member| naga::StructMember {
                        name: member.name,
                        ty: f.map(member.ty),
                        binding: member.binding,
                        offset: member.offset,
                    })
                    .collect(),
                // TODO: How do we map this??
                span,
            },
            BindingArray { base, size } => BindingArray {
                base: f.map(base),
                size,
            },
        },
    }
}

map_alias! {
    pub trait MapNagaGlobal = naga::Handle<naga::Type>, naga::Handle<naga::Expression>;
}

pub fn map_naga_constant<D>(v: naga::Constant, f: &impl MapNagaGlobal<D>) -> naga::Constant {
    naga::Constant {
        name: v.name,
        ty: f.map(v.ty),
        init: f.map(v.init),
    }
}

pub fn map_naga_override<D>(v: naga::Override, f: &impl MapNagaGlobal<D>) -> naga::Override {
    naga::Override {
        name: v.name,
        id: v.id,
        ty: f.map(v.ty),
        init: map_option(v.init, f),
    }
}

pub fn map_naga_global_variable<D>(
    v: naga::GlobalVariable,
    f: &impl MapNagaGlobal<D>,
) -> naga::GlobalVariable {
    naga::GlobalVariable {
        name: v.name,
        space: v.space,
        binding: v.binding,
        ty: f.map(v.ty),
        init: map_option(v.init, f),
    }
}

map_alias! {
    pub trait MapNagaFunction =
        naga::Handle<naga::Constant>,
        naga::Handle<naga::Override>,
        naga::Handle<naga::Type>,
        naga::Handle<naga::GlobalVariable>,
        naga::Handle<naga::Function>,
        naga::Span;
}

pub fn map_naga_function<D>(v: naga::Function, f: &impl MapNagaFunction<D>) -> naga::Function {
    // Map arenas
    let mut local_variables_arena = naga::Arena::new();
    let mut local_variables =
        super::merge::ArenaMerger::new(&mut local_variables_arena, v.local_variables, |_, _| None);

    let mut expressions_arena = naga::Arena::new();
    let mut expressions = ArenaMerger::new(&mut expressions_arena, v.expressions, |_, _| None);

    expressions.apply(|expressions, _orig_handle, span, expr| {
        (
            f.map(span),
            map_naga_expression(expr, &f.and(expressions).and(&local_variables)),
        )
    });

    local_variables.apply(|_local_variables, _orig_handle, span, var| {
        (
            span,
            naga::LocalVariable {
                name: var.name,
                ty: f.map(var.ty),
                init: map_option(var.init, &expressions),
            },
        )
    });

    // Map other collections
    let arguments = map_collection(v.arguments, &map_naga_function_arg.complete(f));
    let result = map_option(v.result, &map_naga_function_result.complete(f));
    let named_expressions = map_collection(
        v.named_expressions,
        &MapFn(|(expr, name)| (expressions.map(expr), name)),
    );

    let body = map_naga_block(v.body, &f.and(&expressions).and(&local_variables));

    naga::Function {
        name: v.name,
        arguments,
        result,
        local_variables: local_variables_arena,
        expressions: expressions_arena,
        named_expressions,
        body,
    }
}

pub fn map_naga_function_arg<D>(
    v: naga::FunctionArgument,
    f: &impl Map<naga::Handle<naga::Type>, D>,
) -> naga::FunctionArgument {
    naga::FunctionArgument {
        name: v.name,
        ty: f.map(v.ty),
        binding: v.binding,
    }
}

pub fn map_naga_function_result<D>(
    v: naga::FunctionResult,
    f: &impl Map<naga::Handle<naga::Type>, D>,
) -> naga::FunctionResult {
    naga::FunctionResult {
        ty: f.map(v.ty),
        binding: v.binding,
    }
}

map_alias! {
    pub trait MapNagaFunctionInner: [MapNagaFunction] =
        naga::Handle<naga::Expression>,
        naga::Handle<naga::LocalVariable>;
}

pub fn map_naga_expression<D>(
    v: naga::Expression,
    f: &impl MapNagaFunctionInner<D>,
) -> naga::Expression {
    use naga::Expression::*;

    match v {
        v @ Literal(_)
        | v @ FunctionArgument(_)
        | v @ RayQueryProceedResult
        | v @ SubgroupBallotResult => v,
        Constant(val) => Constant(f.map(val)),
        Override(val) => Override(f.map(val)),
        ZeroValue(val) => ZeroValue(f.map(val)),
        Compose { ty, components } => Compose {
            ty: f.map(ty),
            components: map_collection(components, f),
        },
        Access { base, index } => Access {
            base: f.map(base),
            index: f.map(index),
        },
        AccessIndex { base, index } => AccessIndex {
            base: f.map(base),
            index,
        },
        Splat { size, value } => Splat {
            size,
            value: f.map(value),
        },
        Swizzle {
            size,
            vector,
            pattern,
        } => Swizzle {
            size,
            vector: f.map(vector),
            pattern,
        },
        GlobalVariable(val) => GlobalVariable(f.map(val)),
        LocalVariable(val) => LocalVariable(f.map(val)),
        Load { pointer } => Load {
            pointer: f.map(pointer),
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
            image: f.map(image),
            sampler: f.map(sampler),
            gather,
            coordinate: f.map(coordinate),
            array_index: map_option(array_index, f),
            offset: map_option(offset, f),
            level: map_naga_sample_level(level, f),
            depth_ref: map_option(depth_ref, f),
        },
        ImageLoad {
            image,
            coordinate,
            array_index,
            sample,
            level,
        } => ImageLoad {
            image: f.map(image),
            coordinate: f.map(coordinate),
            array_index: map_option(array_index, f),
            sample: map_option(sample, f),
            level: map_option(level, f),
        },
        ImageQuery { image, query } => ImageQuery {
            image: f.map(image),
            query: map_naga_image_query(query, f),
        },
        Unary { op, expr } => Unary {
            op,
            expr: f.map(expr),
        },
        Binary { op, left, right } => Binary {
            op,
            left: f.map(left),
            right: f.map(right),
        },
        Select {
            condition,
            accept,
            reject,
        } => Select {
            condition: f.map(condition),
            accept: f.map(accept),
            reject: f.map(reject),
        },
        Derivative { axis, ctrl, expr } => Derivative {
            axis,
            ctrl,
            expr: f.map(expr),
        },
        Relational { fun, argument } => Relational {
            fun,
            argument: f.map(argument),
        },
        Math {
            fun,
            arg,
            arg1,
            arg2,
            arg3,
        } => Math {
            fun,
            arg: f.map(arg),
            arg1: map_option(arg1, f),
            arg2: map_option(arg2, f),
            arg3: map_option(arg3, f),
        },
        As {
            expr,
            kind,
            convert,
        } => As {
            expr: f.map(expr),
            kind,
            convert,
        },
        CallResult(val) => CallResult(f.map(val)),
        AtomicResult { ty, comparison } => AtomicResult {
            ty: f.map(ty),
            comparison,
        },
        WorkGroupUniformLoadResult { ty } => WorkGroupUniformLoadResult { ty: f.map(ty) },
        ArrayLength(val) => ArrayLength(f.map(val)),

        RayQueryGetIntersection { query, committed } => RayQueryGetIntersection {
            query: f.map(query),
            committed,
        },
        SubgroupOperationResult { ty } => SubgroupOperationResult { ty: f.map(ty) },
    }
}

pub fn map_naga_sample_level<D>(
    v: naga::SampleLevel,
    f: &impl Map<naga::Handle<naga::Expression>, D>,
) -> naga::SampleLevel {
    use naga::SampleLevel::*;

    match v {
        v @ Auto | v @ Zero => v,
        Exact(val) => Exact(f.map(val)),
        Bias(val) => Bias(f.map(val)),
        Gradient { x, y } => Gradient {
            x: f.map(x),
            y: f.map(y),
        },
    }
}

pub fn map_naga_image_query<D>(
    v: naga::ImageQuery,
    f: &impl Map<naga::Handle<naga::Expression>, D>,
) -> naga::ImageQuery {
    use naga::ImageQuery::*;

    match v {
        Size { level } => Size {
            level: map_option(level, f),
        },
        v @ NumLevels | v @ NumLayers | v @ NumSamples => v,
    }
}

map_alias! {
    pub trait MapNagaBlock =
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Function>,
        naga::Span;
}

pub fn map_naga_block<D>(v: naga::Block, f: &impl MapNagaBlock<D>) -> naga::Block {
    let mut body = naga::Block::new();

    for (stmt, span) in v.span_into_iter() {
        body.push(map_naga_statement(stmt, f), f.map(span));
    }

    body
}

pub fn map_naga_statement<D>(v: naga::Statement, f: &impl MapNagaBlock<D>) -> naga::Statement {
    use naga::Statement::*;

    match v {
        v @ Break | v @ Continue | v @ Kill | v @ Barrier(_) => v,
        Emit(range) => Emit(map_naga_range(range, |l, r| (f.map(l), f.map(r)))),
        Block(block) => Block(map_naga_block(block, f)),
        If {
            condition,
            accept,
            reject,
        } => If {
            condition: f.map(condition),
            accept: map_naga_block(accept, f),
            reject: map_naga_block(reject, f),
        },
        Switch { selector, cases } => Switch {
            selector: f.map(selector),
            // This awkward syntax exists to ensure that our mapper doesn't grow unbounded in size.
            cases: map_collection(cases, &MapFn(|v| map_naga_switch_case(v, f))),
        },
        Loop {
            body,
            continuing,
            break_if,
        } => Loop {
            body: map_naga_block(body, f),
            continuing: map_naga_block(continuing, f),
            break_if: map_option(break_if, f),
        },
        Return { value } => Return {
            value: map_option(value, f),
        },
        Store { pointer, value } => Store {
            pointer: f.map(pointer),
            value: f.map(value),
        },
        ImageStore {
            image,
            coordinate,
            array_index,
            value,
        } => ImageStore {
            image: f.map(image),
            coordinate: f.map(coordinate),
            array_index: map_option(array_index, f),
            value: f.map(value),
        },
        Atomic {
            pointer,
            fun,
            value,
            result,
        } => Atomic {
            pointer: f.map(pointer),
            fun: map_naga_atomic_function(fun, f),
            value: f.map(value),
            result: map_option(result, f),
        },
        WorkGroupUniformLoad { pointer, result } => WorkGroupUniformLoad {
            pointer: f.map(pointer),
            result: f.map(result),
        },
        Call {
            function,
            arguments,
            result,
        } => Call {
            function: f.map(function),
            arguments: map_collection(arguments, f),
            result: map_option(result, f),
        },
        RayQuery { query, fun } => RayQuery {
            query: f.map(query),
            fun: map_naga_ray_query_function(fun, f),
        },
        SubgroupBallot { result, predicate } => SubgroupBallot {
            result: f.map(result),
            predicate: map_option(predicate, f),
        },
        SubgroupGather {
            mode,
            argument,
            result,
        } => SubgroupGather {
            mode: map_naga_gather_mode(mode, f),
            argument: f.map(argument),
            result: f.map(result),
        },
        SubgroupCollectiveOperation {
            op,
            collective_op,
            argument,
            result,
        } => SubgroupCollectiveOperation {
            op,
            collective_op,
            argument: f.map(argument),
            result: f.map(result),
        },
    }
}

pub fn map_naga_switch_case<D>(v: naga::SwitchCase, f: &impl MapNagaBlock<D>) -> naga::SwitchCase {
    naga::SwitchCase {
        value: v.value,
        body: map_naga_block(v.body, f),
        fall_through: v.fall_through,
    }
}

pub fn map_naga_atomic_function<D>(
    v: naga::AtomicFunction,
    f: &impl Map<naga::Handle<naga::Expression>, D>,
) -> naga::AtomicFunction {
    use naga::AtomicFunction::*;

    match v {
        v @ Add
        | v @ Subtract
        | v @ And
        | v @ ExclusiveOr
        | v @ InclusiveOr
        | v @ Min
        | v @ Max => v,
        Exchange { compare } => Exchange {
            compare: map_option(compare, f),
        },
    }
}

pub fn map_naga_ray_query_function<D>(
    v: naga::RayQueryFunction,
    f: &impl Map<naga::Handle<naga::Expression>, D>,
) -> naga::RayQueryFunction {
    use naga::RayQueryFunction::*;

    match v {
        Initialize {
            acceleration_structure,
            descriptor,
        } => Initialize {
            acceleration_structure: f.map(acceleration_structure),
            descriptor: f.map(descriptor),
        },
        Proceed { result } => Proceed {
            result: f.map(result),
        },
        Terminate => Terminate,
    }
}

pub fn map_naga_gather_mode<D>(
    v: naga::GatherMode,
    f: &impl Map<naga::Handle<naga::Expression>, D>,
) -> naga::GatherMode {
    use naga::GatherMode::*;

    match v {
        BroadcastFirst => BroadcastFirst,
        Broadcast(val) => Broadcast(f.map(val)),
        Shuffle(val) => Shuffle(f.map(val)),
        ShuffleDown(val) => ShuffleDown(f.map(val)),
        ShuffleUp(val) => ShuffleUp(f.map(val)),
        ShuffleXor(val) => ShuffleXor(f.map(val)),
    }
}
