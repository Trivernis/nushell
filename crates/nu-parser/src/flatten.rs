use nu_protocol::ast::{
    Block, Expr, Expression, ImportPatternMember, MatchPattern, PathMember, Pattern, Pipeline,
    PipelineElement,
};
use nu_protocol::{engine::StateWorkingSet, Span};
use nu_protocol::{DeclId, VarId};
use std::fmt::{Display, Formatter, Result};

#[derive(Debug, Eq, PartialEq, Ord, Clone, PartialOrd)]
pub enum FlatShape {
    And,
    Binary,
    Block,
    Bool,
    Closure,
    Custom(DeclId),
    DateTime,
    Directory,
    External,
    ExternalArg,
    Filepath,
    Flag,
    Float,
    Garbage,
    GlobPattern,
    Int,
    InternalCall(DeclId),
    Keyword,
    List,
    Literal,
    MatchPattern,
    Nothing,
    Operator,
    Or,
    Pipe,
    Range,
    Record,
    Redirection,
    Signature,
    String,
    StringInterpolation,
    Table,
    Variable(VarId),
    VarDecl(VarId),
}

impl Display for FlatShape {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            FlatShape::And => write!(f, "shape_and"),
            FlatShape::Binary => write!(f, "shape_binary"),
            FlatShape::Block => write!(f, "shape_block"),
            FlatShape::Bool => write!(f, "shape_bool"),
            FlatShape::Closure => write!(f, "shape_closure"),
            FlatShape::Custom(_) => write!(f, "shape_custom"),
            FlatShape::DateTime => write!(f, "shape_datetime"),
            FlatShape::Directory => write!(f, "shape_directory"),
            FlatShape::External => write!(f, "shape_external"),
            FlatShape::ExternalArg => write!(f, "shape_externalarg"),
            FlatShape::Filepath => write!(f, "shape_filepath"),
            FlatShape::Flag => write!(f, "shape_flag"),
            FlatShape::Float => write!(f, "shape_float"),
            FlatShape::Garbage => write!(f, "shape_garbage"),
            FlatShape::GlobPattern => write!(f, "shape_globpattern"),
            FlatShape::Int => write!(f, "shape_int"),
            FlatShape::InternalCall(_) => write!(f, "shape_internalcall"),
            FlatShape::Keyword => write!(f, "shape_keyword"),
            FlatShape::List => write!(f, "shape_list"),
            FlatShape::Literal => write!(f, "shape_literal"),
            FlatShape::MatchPattern => write!(f, "shape_match_pattern"),
            FlatShape::Nothing => write!(f, "shape_nothing"),
            FlatShape::Operator => write!(f, "shape_operator"),
            FlatShape::Or => write!(f, "shape_or"),
            FlatShape::Pipe => write!(f, "shape_pipe"),
            FlatShape::Range => write!(f, "shape_range"),
            FlatShape::Record => write!(f, "shape_record"),
            FlatShape::Redirection => write!(f, "shape_redirection"),
            FlatShape::Signature => write!(f, "shape_signature"),
            FlatShape::String => write!(f, "shape_string"),
            FlatShape::StringInterpolation => write!(f, "shape_string_interpolation"),
            FlatShape::Table => write!(f, "shape_table"),
            FlatShape::Variable(_) => write!(f, "shape_variable"),
            FlatShape::VarDecl(_) => write!(f, "shape_vardecl"),
        }
    }
}

pub fn flatten_block(working_set: &StateWorkingSet, block: &Block) -> Vec<(Span, FlatShape)> {
    let mut output = vec![];

    for pipeline in &block.pipelines {
        output.extend(flatten_pipeline(working_set, pipeline));
    }
    output
}

pub fn flatten_expression(
    working_set: &StateWorkingSet,
    expr: &Expression,
) -> Vec<(Span, FlatShape)> {
    if let Some(custom_completion) = &expr.custom_completion {
        return vec![(expr.span, FlatShape::Custom(*custom_completion))];
    }

    match &expr.expr {
        Expr::BinaryOp(lhs, op, rhs) => {
            let mut output = vec![];
            output.extend(flatten_expression(working_set, lhs));
            output.extend(flatten_expression(working_set, op));
            output.extend(flatten_expression(working_set, rhs));
            output
        }
        Expr::UnaryNot(inner_expr) => {
            let mut output = vec![(
                Span::new(expr.span.start, expr.span.start + 3),
                FlatShape::Operator,
            )];
            output.extend(flatten_expression(working_set, inner_expr));
            output
        }
        Expr::Closure(block_id) => {
            let outer_span = expr.span;

            let mut output = vec![];

            let block = working_set.get_block(*block_id);
            let flattened = flatten_block(working_set, block);

            if let Some(first) = flattened.first() {
                if first.0.start > outer_span.start {
                    output.push((
                        Span::new(outer_span.start, first.0.start),
                        FlatShape::Closure,
                    ));
                }
            }

            let last = if let Some(last) = flattened.last() {
                if last.0.end < outer_span.end {
                    Some((Span::new(last.0.end, outer_span.end), FlatShape::Closure))
                } else {
                    None
                }
            } else {
                None
            };

            output.extend(flattened);
            if let Some(last) = last {
                output.push(last)
            }

            output
        }
        Expr::Block(block_id) | Expr::RowCondition(block_id) | Expr::Subexpression(block_id) => {
            let outer_span = expr.span;

            let mut output = vec![];

            let flattened = flatten_block(working_set, working_set.get_block(*block_id));

            if let Some(first) = flattened.first() {
                if first.0.start > outer_span.start {
                    output.push((Span::new(outer_span.start, first.0.start), FlatShape::Block));
                }
            }

            let last = if let Some(last) = flattened.last() {
                if last.0.end < outer_span.end {
                    Some((Span::new(last.0.end, outer_span.end), FlatShape::Block))
                } else {
                    None
                }
            } else {
                None
            };

            output.extend(flattened);
            if let Some(last) = last {
                output.push(last)
            }

            output
        }
        Expr::Call(call) => {
            let mut output = vec![];

            if call.head.end != 0 {
                // Make sure we don't push synthetic calls
                output.push((call.head, FlatShape::InternalCall(call.decl_id)));
            }

            let mut args = vec![];
            for positional in call.positional_iter() {
                let flattened = flatten_expression(working_set, positional);
                args.extend(flattened);
            }
            for named in call.named_iter() {
                if named.0.span.end != 0 {
                    // Ignore synthetic flags
                    args.push((named.0.span, FlatShape::Flag));
                }
                if let Some(expr) = &named.2 {
                    args.extend(flatten_expression(working_set, expr));
                }
            }
            // sort these since flags and positional args can be intermixed
            args.sort();

            output.extend(args);
            output
        }
        Expr::ExternalCall(head, args, _) => {
            let mut output = vec![];

            match **head {
                Expression {
                    expr: Expr::String(..),
                    span,
                    ..
                } => {
                    output.push((span, FlatShape::External));
                }
                _ => {
                    output.extend(flatten_expression(working_set, head));
                }
            }

            for arg in args {
                //output.push((*arg, FlatShape::ExternalArg));
                match arg {
                    Expression {
                        expr: Expr::String(..),
                        span,
                        ..
                    } => {
                        output.push((*span, FlatShape::ExternalArg));
                    }
                    _ => {
                        output.extend(flatten_expression(working_set, arg));
                    }
                }
            }

            output
        }
        Expr::Garbage => {
            vec![(expr.span, FlatShape::Garbage)]
        }
        Expr::Nothing => {
            vec![(expr.span, FlatShape::Nothing)]
        }
        Expr::DateTime(_) => {
            vec![(expr.span, FlatShape::DateTime)]
        }
        Expr::Binary(_) => {
            vec![(expr.span, FlatShape::Binary)]
        }
        Expr::Int(_) => {
            vec![(expr.span, FlatShape::Int)]
        }
        Expr::Float(_) => {
            vec![(expr.span, FlatShape::Float)]
        }
        Expr::MatchPattern(pattern) => {
            // FIXME: do nicer flattening later
            flatten_pattern(pattern)
        }
        Expr::MatchBlock(matches) => {
            let mut output = vec![];

            for match_ in matches {
                output.extend(flatten_pattern(&match_.0));
                output.extend(flatten_expression(working_set, &match_.1));
            }

            output
        }
        Expr::ValueWithUnit(x, unit) => {
            let mut output = flatten_expression(working_set, x);
            output.push((unit.span, FlatShape::String));

            output
        }
        Expr::CellPath(cell_path) => {
            let mut output = vec![];
            for path_element in &cell_path.members {
                match path_element {
                    PathMember::String { span, .. } => output.push((*span, FlatShape::String)),
                    PathMember::Int { span, .. } => output.push((*span, FlatShape::Int)),
                }
            }
            output
        }
        Expr::FullCellPath(cell_path) => {
            let mut output = vec![];
            output.extend(flatten_expression(working_set, &cell_path.head));
            for path_element in &cell_path.tail {
                match path_element {
                    PathMember::String { span, .. } => output.push((*span, FlatShape::String)),
                    PathMember::Int { span, .. } => output.push((*span, FlatShape::Int)),
                }
            }
            output
        }
        Expr::ImportPattern(import_pattern) => {
            let mut output = vec![(import_pattern.head.span, FlatShape::String)];

            for member in &import_pattern.members {
                match member {
                    ImportPatternMember::Glob { span } => output.push((*span, FlatShape::String)),
                    ImportPatternMember::Name { span, .. } => {
                        output.push((*span, FlatShape::String))
                    }
                    ImportPatternMember::List { names } => {
                        for (_, span) in names {
                            output.push((*span, FlatShape::String));
                        }
                    }
                }
            }

            output
        }
        Expr::Overlay(_) => {
            vec![(expr.span, FlatShape::String)]
        }
        Expr::Range(from, next, to, op) => {
            let mut output = vec![];
            if let Some(f) = from {
                output.extend(flatten_expression(working_set, f));
            }
            if let Some(s) = next {
                output.extend(vec![(op.next_op_span, FlatShape::Operator)]);
                output.extend(flatten_expression(working_set, s));
            }
            output.extend(vec![(op.span, FlatShape::Operator)]);
            if let Some(t) = to {
                output.extend(flatten_expression(working_set, t));
            }
            output
        }
        Expr::Bool(_) => {
            vec![(expr.span, FlatShape::Bool)]
        }
        Expr::Filepath(_) => {
            vec![(expr.span, FlatShape::Filepath)]
        }
        Expr::Directory(_) => {
            vec![(expr.span, FlatShape::Directory)]
        }
        Expr::GlobPattern(_) => {
            vec![(expr.span, FlatShape::GlobPattern)]
        }
        Expr::List(list) => {
            let outer_span = expr.span;
            let mut last_end = outer_span.start;

            let mut output = vec![];
            for l in list {
                let flattened = flatten_expression(working_set, l);

                if let Some(first) = flattened.first() {
                    if first.0.start > last_end {
                        output.push((Span::new(last_end, first.0.start), FlatShape::List));
                    }
                }

                if let Some(last) = flattened.last() {
                    last_end = last.0.end;
                }

                output.extend(flattened);
            }

            if last_end < outer_span.end {
                output.push((Span::new(last_end, outer_span.end), FlatShape::List));
            }
            output
        }
        Expr::StringInterpolation(exprs) => {
            let mut output = vec![];
            for expr in exprs {
                output.extend(flatten_expression(working_set, expr));
            }

            if let Some(first) = output.first() {
                if first.0.start != expr.span.start {
                    // If we aren't a bare word interpolation, also highlight the outer quotes
                    output.insert(
                        0,
                        (
                            Span::new(expr.span.start, expr.span.start + 2),
                            FlatShape::StringInterpolation,
                        ),
                    );
                    output.push((
                        Span::new(expr.span.end - 1, expr.span.end),
                        FlatShape::StringInterpolation,
                    ));
                }
            }
            output
        }
        Expr::Record(list) => {
            let outer_span = expr.span;
            let mut last_end = outer_span.start;

            let mut output = vec![];
            for l in list {
                let flattened_lhs = flatten_expression(working_set, &l.0);
                let flattened_rhs = flatten_expression(working_set, &l.1);

                if let Some(first) = flattened_lhs.first() {
                    if first.0.start > last_end {
                        output.push((Span::new(last_end, first.0.start), FlatShape::Record));
                    }
                }
                if let Some(last) = flattened_lhs.last() {
                    last_end = last.0.end;
                }
                output.extend(flattened_lhs);

                if let Some(first) = flattened_rhs.first() {
                    if first.0.start > last_end {
                        output.push((Span::new(last_end, first.0.start), FlatShape::Record));
                    }
                }
                if let Some(last) = flattened_rhs.last() {
                    last_end = last.0.end;
                }

                output.extend(flattened_rhs);
            }
            if last_end < outer_span.end {
                output.push((Span::new(last_end, outer_span.end), FlatShape::Record));
            }

            output
        }
        Expr::Keyword(_, span, expr) => {
            let mut output = vec![(*span, FlatShape::Keyword)];
            output.extend(flatten_expression(working_set, expr));
            output
        }
        Expr::Operator(_) => {
            vec![(expr.span, FlatShape::Operator)]
        }
        Expr::Signature(_) => {
            vec![(expr.span, FlatShape::Signature)]
        }
        Expr::String(_) => {
            vec![(expr.span, FlatShape::String)]
        }
        Expr::Table(headers, cells) => {
            let outer_span = expr.span;
            let mut last_end = outer_span.start;

            let mut output = vec![];
            for e in headers {
                let flattened = flatten_expression(working_set, e);
                if let Some(first) = flattened.first() {
                    if first.0.start > last_end {
                        output.push((Span::new(last_end, first.0.start), FlatShape::Table));
                    }
                }

                if let Some(last) = flattened.last() {
                    last_end = last.0.end;
                }

                output.extend(flattened);
            }
            for row in cells {
                for expr in row {
                    let flattened = flatten_expression(working_set, expr);
                    if let Some(first) = flattened.first() {
                        if first.0.start > last_end {
                            output.push((Span::new(last_end, first.0.start), FlatShape::Table));
                        }
                    }

                    if let Some(last) = flattened.last() {
                        last_end = last.0.end;
                    }

                    output.extend(flattened);
                }
            }

            if last_end < outer_span.end {
                output.push((Span::new(last_end, outer_span.end), FlatShape::Table));
            }

            output
        }
        Expr::Var(var_id) => {
            vec![(expr.span, FlatShape::Variable(*var_id))]
        }
        Expr::VarDecl(var_id) => {
            vec![(expr.span, FlatShape::VarDecl(*var_id))]
        }

        Expr::Spread(inner_expr) => {
            let mut output = vec![(
                Span::new(expr.span.start, expr.span.start + 3),
                FlatShape::Operator,
            )];
            output.extend(flatten_expression(working_set, inner_expr));
            output
        }
    }
}

pub fn flatten_pipeline_element(
    working_set: &StateWorkingSet,
    pipeline_element: &PipelineElement,
) -> Vec<(Span, FlatShape)> {
    match pipeline_element {
        PipelineElement::Expression(span, expr) => {
            if let Some(span) = span {
                let mut output = vec![(*span, FlatShape::Pipe)];
                output.append(&mut flatten_expression(working_set, expr));
                output
            } else {
                flatten_expression(working_set, expr)
            }
        }
        PipelineElement::Redirection(span, _, expr) => {
            let mut output = vec![(*span, FlatShape::Redirection)];
            output.append(&mut flatten_expression(working_set, expr));
            output
        }
        PipelineElement::SeparateRedirection {
            out: (out_span, out_expr),
            err: (err_span, err_expr),
        } => {
            let mut output = vec![(*out_span, FlatShape::Redirection)];
            output.append(&mut flatten_expression(working_set, out_expr));
            output.push((*err_span, FlatShape::Redirection));
            output.append(&mut flatten_expression(working_set, err_expr));
            output
        }
        PipelineElement::SameTargetRedirection {
            cmd: (cmd_span, cmd_expr),
            redirection: (redirect_span, redirect_expr),
        } => {
            let mut output = if let Some(span) = cmd_span {
                let mut output = vec![(*span, FlatShape::Pipe)];
                output.append(&mut flatten_expression(working_set, cmd_expr));
                output
            } else {
                flatten_expression(working_set, cmd_expr)
            };
            output.push((*redirect_span, FlatShape::Redirection));
            output.append(&mut flatten_expression(working_set, redirect_expr));
            output
        }
        PipelineElement::And(span, expr) => {
            let mut output = vec![(*span, FlatShape::And)];
            output.append(&mut flatten_expression(working_set, expr));
            output
        }
        PipelineElement::Or(span, expr) => {
            let mut output = vec![(*span, FlatShape::Or)];
            output.append(&mut flatten_expression(working_set, expr));
            output
        }
    }
}

pub fn flatten_pipeline(
    working_set: &StateWorkingSet,
    pipeline: &Pipeline,
) -> Vec<(Span, FlatShape)> {
    let mut output = vec![];
    for expr in &pipeline.elements {
        output.extend(flatten_pipeline_element(working_set, expr))
    }
    output
}

pub fn flatten_pattern(match_pattern: &MatchPattern) -> Vec<(Span, FlatShape)> {
    let mut output = vec![];
    match &match_pattern.pattern {
        Pattern::Garbage => {
            output.push((match_pattern.span, FlatShape::Garbage));
        }
        Pattern::IgnoreValue => {
            output.push((match_pattern.span, FlatShape::Nothing));
        }
        Pattern::IgnoreRest => {
            output.push((match_pattern.span, FlatShape::Nothing));
        }
        Pattern::List(items) => {
            if let Some(first) = items.first() {
                if let Some(last) = items.last() {
                    output.push((
                        Span::new(match_pattern.span.start, first.span.start),
                        FlatShape::MatchPattern,
                    ));
                    for item in items {
                        output.extend(flatten_pattern(item));
                    }
                    output.push((
                        Span::new(last.span.end, match_pattern.span.end),
                        FlatShape::MatchPattern,
                    ))
                }
            } else {
                output.push((match_pattern.span, FlatShape::MatchPattern));
            }
        }
        Pattern::Record(items) => {
            if let Some(first) = items.first() {
                if let Some(last) = items.last() {
                    output.push((
                        Span::new(match_pattern.span.start, first.1.span.start),
                        FlatShape::MatchPattern,
                    ));
                    for item in items {
                        output.extend(flatten_pattern(&item.1));
                    }
                    output.push((
                        Span::new(last.1.span.end, match_pattern.span.end),
                        FlatShape::MatchPattern,
                    ))
                }
            } else {
                output.push((match_pattern.span, FlatShape::MatchPattern));
            }
        }
        Pattern::Value(_) => {
            output.push((match_pattern.span, FlatShape::MatchPattern));
        }
        Pattern::Variable(var_id) => {
            output.push((match_pattern.span, FlatShape::VarDecl(*var_id)));
        }
        Pattern::Rest(var_id) => {
            output.push((match_pattern.span, FlatShape::VarDecl(*var_id)));
        }
        Pattern::Or(patterns) => {
            for pattern in patterns {
                output.extend(flatten_pattern(pattern));
            }
        }
    }
    output
}
