use ruff_python_ast::{self as ast, Expr, Stmt};

/// A trait for transforming owned AST nodes.
pub trait OwnedTransform {
    /// Transform an [`Expr`], returning the transformed expression.
    fn visit_expr_owned(&self, expr: Expr) -> Expr {
        walk_expr_owned(self, expr)
    }

    /// Transform a [`Stmt`], returning the transformed statement.
    fn visit_stmt_owned(&self, stmt: Stmt) -> Stmt {
        walk_stmt_owned(self, stmt)
    }

    fn visit_arguments_owned(&self, arguments: ast::Arguments) -> ast::Arguments {
        walk_arguments_owned(self, arguments)
    }

    fn visit_keyword_owned(&self, keyword: ast::Keyword) -> ast::Keyword {
        walk_keyword_owned(self, keyword)
    }

    fn visit_parameters_owned(&self, parameters: ast::Parameters) -> ast::Parameters {
        walk_parameters_owned(self, parameters)
    }

    fn visit_parameter_with_default_owned(
        &self,
        param: ast::ParameterWithDefault,
    ) -> ast::ParameterWithDefault {
        walk_parameter_with_default_owned(self, param)
    }

    fn visit_parameter_owned(&self, parameter: ast::Parameter) -> ast::Parameter {
        walk_parameter_owned(self, parameter)
    }

    fn visit_comprehension_owned(&self, comprehension: ast::Comprehension) -> ast::Comprehension {
        walk_comprehension_owned(self, comprehension)
    }

    fn visit_with_item_owned(&self, with_item: ast::WithItem) -> ast::WithItem {
        walk_with_item_owned(self, with_item)
    }

    fn visit_type_params_owned(&self, type_params: ast::TypeParams) -> ast::TypeParams {
        walk_type_params_owned(self, type_params)
    }

    fn visit_type_param_owned(&self, type_param: ast::TypeParam) -> ast::TypeParam {
        walk_type_param_owned(self, type_param)
    }

    fn visit_match_case_owned(&self, case: ast::MatchCase) -> ast::MatchCase {
        walk_match_case_owned(self, case)
    }

    fn visit_pattern_owned(&self, pattern: ast::Pattern) -> ast::Pattern {
        walk_pattern_owned(self, pattern)
    }

    fn visit_pattern_arguments_owned(&self, args: ast::PatternArguments) -> ast::PatternArguments {
        walk_pattern_arguments_owned(self, args)
    }

    fn visit_pattern_keyword_owned(&self, keyword: ast::PatternKeyword) -> ast::PatternKeyword {
        walk_pattern_keyword_owned(self, keyword)
    }

    fn visit_decorator_owned(&self, decorator: ast::Decorator) -> ast::Decorator {
        walk_decorator_owned(self, decorator)
    }

    fn visit_except_handler_owned(&self, handler: ast::ExceptHandler) -> ast::ExceptHandler {
        walk_except_handler_owned(self, handler)
    }

    fn visit_elif_else_clause_owned(&self, clause: ast::ElifElseClause) -> ast::ElifElseClause {
        walk_elif_else_clause_owned(self, clause)
    }

    fn visit_f_string_owned(&self, f_string: &mut ast::FString) {
        walk_f_string_owned(self, f_string)
    }

    fn visit_t_string_owned(&self, t_string: &mut ast::TString) {
        walk_t_string_owned(self, t_string)
    }

    fn visit_interpolated_string_element_owned(
        &self,
        element: ast::InterpolatedStringElement,
    ) -> ast::InterpolatedStringElement {
        walk_interpolated_string_element_owned(self, element)
    }

    fn visit_interpolated_string_format_spec_owned(
        &self,
        spec: ast::InterpolatedStringFormatSpec,
    ) -> ast::InterpolatedStringFormatSpec {
        walk_interpolated_string_format_spec_owned(self, spec)
    }
}

pub fn walk_expr_owned<T: OwnedTransform + ?Sized>(transformer: &T, expr: Expr) -> Expr {
    match expr {
        Expr::BoolOp(mut node) => {
            node.values = node
                .values
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Expr::BoolOp(node)
        }
        Expr::Named(mut node) => {
            node.target = Box::new(transformer.visit_expr_owned(*node.target));
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            Expr::Named(node)
        }
        Expr::BinOp(mut node) => {
            node.left = Box::new(transformer.visit_expr_owned(*node.left));
            node.right = Box::new(transformer.visit_expr_owned(*node.right));
            Expr::BinOp(node)
        }
        Expr::UnaryOp(mut node) => {
            node.operand = Box::new(transformer.visit_expr_owned(*node.operand));
            Expr::UnaryOp(node)
        }
        Expr::Lambda(mut node) => {
            node.parameters = node
                .parameters
                .map(|parameters| Box::new(transformer.visit_parameters_owned(*parameters)));
            node.body = Box::new(transformer.visit_expr_owned(*node.body));
            Expr::Lambda(node)
        }
        Expr::If(mut node) => {
            node.test = Box::new(transformer.visit_expr_owned(*node.test));
            node.body = Box::new(transformer.visit_expr_owned(*node.body));
            node.orelse = Box::new(transformer.visit_expr_owned(*node.orelse));
            Expr::If(node)
        }
        Expr::Dict(mut node) => {
            node.items = node
                .items
                .into_iter()
                .map(|mut item| {
                    item.key = item.key.map(|key| transformer.visit_expr_owned(key));
                    item.value = transformer.visit_expr_owned(item.value);
                    item
                })
                .collect();
            Expr::Dict(node)
        }
        Expr::Set(mut node) => {
            node.elts = node
                .elts
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Expr::Set(node)
        }
        Expr::ListComp(mut node) => {
            node.generators = node
                .generators
                .into_iter()
                .map(|comp| transformer.visit_comprehension_owned(comp))
                .collect();
            node.elt = Box::new(transformer.visit_expr_owned(*node.elt));
            Expr::ListComp(node)
        }
        Expr::SetComp(mut node) => {
            node.generators = node
                .generators
                .into_iter()
                .map(|comp| transformer.visit_comprehension_owned(comp))
                .collect();
            node.elt = Box::new(transformer.visit_expr_owned(*node.elt));
            Expr::SetComp(node)
        }
        Expr::DictComp(mut node) => {
            node.generators = node
                .generators
                .into_iter()
                .map(|comp| transformer.visit_comprehension_owned(comp))
                .collect();
            node.key = Box::new(transformer.visit_expr_owned(*node.key));
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            Expr::DictComp(node)
        }
        Expr::Generator(mut node) => {
            node.generators = node
                .generators
                .into_iter()
                .map(|comp| transformer.visit_comprehension_owned(comp))
                .collect();
            node.elt = Box::new(transformer.visit_expr_owned(*node.elt));
            Expr::Generator(node)
        }
        Expr::Await(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            Expr::Await(node)
        }
        Expr::Yield(mut node) => {
            node.value = node
                .value
                .map(|value| Box::new(transformer.visit_expr_owned(*value)));
            Expr::Yield(node)
        }
        Expr::YieldFrom(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            Expr::YieldFrom(node)
        }
        Expr::Compare(mut node) => {
            node.left = Box::new(transformer.visit_expr_owned(*node.left));
            node.comparators = node
                .comparators
                .into_vec()
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            Expr::Compare(node)
        }
        Expr::Call(mut node) => {
            node.func = Box::new(transformer.visit_expr_owned(*node.func));
            node.arguments = transformer.visit_arguments_owned(node.arguments);
            Expr::Call(node)
        }
        Expr::FString(mut node) => {
            for part in &mut node.value {
                if let ast::FStringPart::FString(f_string) = part {
                    transformer.visit_f_string_owned(f_string);
                }
            }
            Expr::FString(node)
        }
        Expr::TString(mut node) => {
            for t_string in &mut node.value {
                transformer.visit_t_string_owned(t_string);
            }
            Expr::TString(node)
        }
        Expr::StringLiteral(node) => Expr::StringLiteral(node),
        Expr::BytesLiteral(node) => Expr::BytesLiteral(node),
        Expr::NumberLiteral(node) => Expr::NumberLiteral(node),
        Expr::BooleanLiteral(node) => Expr::BooleanLiteral(node),
        Expr::NoneLiteral(node) => Expr::NoneLiteral(node),
        Expr::EllipsisLiteral(node) => Expr::EllipsisLiteral(node),
        Expr::Attribute(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            Expr::Attribute(node)
        }
        Expr::Subscript(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            node.slice = Box::new(transformer.visit_expr_owned(*node.slice));
            Expr::Subscript(node)
        }
        Expr::Starred(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            Expr::Starred(node)
        }
        Expr::Name(node) => Expr::Name(node),
        Expr::List(mut node) => {
            node.elts = node
                .elts
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Expr::List(node)
        }
        Expr::Tuple(mut node) => {
            node.elts = node
                .elts
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Expr::Tuple(node)
        }
        Expr::Slice(mut node) => {
            node.lower = node
                .lower
                .map(|expr| Box::new(transformer.visit_expr_owned(*expr)));
            node.upper = node
                .upper
                .map(|expr| Box::new(transformer.visit_expr_owned(*expr)));
            node.step = node
                .step
                .map(|expr| Box::new(transformer.visit_expr_owned(*expr)));
            Expr::Slice(node)
        }
        Expr::IpyEscapeCommand(node) => Expr::IpyEscapeCommand(node),
    }
}

pub fn walk_stmt_owned<T: OwnedTransform + ?Sized>(transformer: &T, stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::FunctionDef(mut node) => {
            node.decorator_list = node
                .decorator_list
                .into_iter()
                .map(|decorator| transformer.visit_decorator_owned(decorator))
                .collect();
            node.type_params = node
                .type_params
                .map(|type_params| Box::new(transformer.visit_type_params_owned(*type_params)));
            node.parameters = Box::new(transformer.visit_parameters_owned(*node.parameters));
            node.returns = node
                .returns
                .map(|returns| Box::new(transformer.visit_expr_owned(*returns)));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            Stmt::FunctionDef(node)
        }
        Stmt::ClassDef(mut node) => {
            node.decorator_list = node
                .decorator_list
                .into_iter()
                .map(|decorator| transformer.visit_decorator_owned(decorator))
                .collect();
            node.type_params = node
                .type_params
                .map(|type_params| Box::new(transformer.visit_type_params_owned(*type_params)));
            node.arguments = node
                .arguments
                .map(|arguments| Box::new(transformer.visit_arguments_owned(*arguments)));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            Stmt::ClassDef(node)
        }
        Stmt::Return(mut node) => {
            node.value = node
                .value
                .map(|value| Box::new(transformer.visit_expr_owned(*value)));
            Stmt::Return(node)
        }
        Stmt::Delete(mut node) => {
            node.targets = node
                .targets
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Stmt::Delete(node)
        }
        Stmt::TypeAlias(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            node.type_params = node
                .type_params
                .map(|type_params| Box::new(transformer.visit_type_params_owned(*type_params)));
            node.name = Box::new(transformer.visit_expr_owned(*node.name));
            Stmt::TypeAlias(node)
        }
        Stmt::Assign(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            node.targets = node
                .targets
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Stmt::Assign(node)
        }
        Stmt::AugAssign(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            node.target = Box::new(transformer.visit_expr_owned(*node.target));
            Stmt::AugAssign(node)
        }
        Stmt::AnnAssign(mut node) => {
            node.value = node
                .value
                .map(|value| Box::new(transformer.visit_expr_owned(*value)));
            node.annotation = Box::new(transformer.visit_expr_owned(*node.annotation));
            node.target = Box::new(transformer.visit_expr_owned(*node.target));
            Stmt::AnnAssign(node)
        }
        Stmt::For(mut node) => {
            node.iter = Box::new(transformer.visit_expr_owned(*node.iter));
            node.target = Box::new(transformer.visit_expr_owned(*node.target));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            node.orelse = node
                .orelse
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            Stmt::For(node)
        }
        Stmt::While(mut node) => {
            node.test = Box::new(transformer.visit_expr_owned(*node.test));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            node.orelse = node
                .orelse
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            Stmt::While(node)
        }
        Stmt::If(mut node) => {
            node.test = Box::new(transformer.visit_expr_owned(*node.test));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            node.elif_else_clauses = node
                .elif_else_clauses
                .into_iter()
                .map(|clause| transformer.visit_elif_else_clause_owned(clause))
                .collect();
            Stmt::If(node)
        }
        Stmt::With(mut node) => {
            node.items = node
                .items
                .into_iter()
                .map(|item| transformer.visit_with_item_owned(item))
                .collect();
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            Stmt::With(node)
        }
        Stmt::Match(mut node) => {
            node.subject = Box::new(transformer.visit_expr_owned(*node.subject));
            node.cases = node
                .cases
                .into_iter()
                .map(|case| transformer.visit_match_case_owned(case))
                .collect();
            Stmt::Match(node)
        }
        Stmt::Raise(mut node) => {
            node.exc = node
                .exc
                .map(|exc| Box::new(transformer.visit_expr_owned(*exc)));
            node.cause = node
                .cause
                .map(|cause| Box::new(transformer.visit_expr_owned(*cause)));
            Stmt::Raise(node)
        }
        Stmt::Try(mut node) => {
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            node.handlers = node
                .handlers
                .into_iter()
                .map(|handler| transformer.visit_except_handler_owned(handler))
                .collect();
            node.orelse = node
                .orelse
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            node.finalbody = node
                .finalbody
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            Stmt::Try(node)
        }
        Stmt::Assert(mut node) => {
            node.test = Box::new(transformer.visit_expr_owned(*node.test));
            node.msg = node
                .msg
                .map(|msg| Box::new(transformer.visit_expr_owned(*msg)));
            Stmt::Assert(node)
        }
        Stmt::Import(node) => Stmt::Import(node),
        Stmt::ImportFrom(node) => Stmt::ImportFrom(node),
        Stmt::Global(node) => Stmt::Global(node),
        Stmt::Nonlocal(node) => Stmt::Nonlocal(node),
        Stmt::Expr(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            Stmt::Expr(node)
        }
        Stmt::Pass(node) => Stmt::Pass(node),
        Stmt::Break(node) => Stmt::Break(node),
        Stmt::Continue(node) => Stmt::Continue(node),
        Stmt::IpyEscapeCommand(node) => Stmt::IpyEscapeCommand(node),
    }
}

pub fn walk_arguments_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    arguments: ast::Arguments,
) -> ast::Arguments {
    let ast::Arguments {
        range,
        node_index,
        args,
        keywords,
    } = arguments;
    let args = args
        .into_vec()
        .into_iter()
        .map(|expr| transformer.visit_expr_owned(expr))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let keywords = keywords
        .into_vec()
        .into_iter()
        .map(|kw| transformer.visit_keyword_owned(kw))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    ast::Arguments {
        range,
        node_index,
        args,
        keywords,
    }
}

pub fn walk_keyword_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    keyword: ast::Keyword,
) -> ast::Keyword {
    let ast::Keyword {
        range,
        node_index,
        arg,
        value,
    } = keyword;
    let value = transformer.visit_expr_owned(value);
    ast::Keyword {
        range,
        node_index,
        arg,
        value,
    }
}

pub fn walk_parameters_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    parameters: ast::Parameters,
) -> ast::Parameters {
    let ast::Parameters {
        range,
        node_index,
        posonlyargs,
        args,
        vararg,
        kwonlyargs,
        kwarg,
    } = parameters;
    let posonlyargs = posonlyargs
        .into_iter()
        .map(|p| transformer.visit_parameter_with_default_owned(p))
        .collect();
    let args = args
        .into_iter()
        .map(|p| transformer.visit_parameter_with_default_owned(p))
        .collect();
    let vararg = vararg.map(|p| Box::new(transformer.visit_parameter_owned(*p)));
    let kwonlyargs = kwonlyargs
        .into_iter()
        .map(|p| transformer.visit_parameter_with_default_owned(p))
        .collect();
    let kwarg = kwarg.map(|p| Box::new(transformer.visit_parameter_owned(*p)));
    ast::Parameters {
        range,
        node_index,
        posonlyargs,
        args,
        vararg,
        kwonlyargs,
        kwarg,
    }
}

pub fn walk_parameter_with_default_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    param: ast::ParameterWithDefault,
) -> ast::ParameterWithDefault {
    let ast::ParameterWithDefault {
        range,
        node_index,
        parameter,
        default,
    } = param;
    let parameter = transformer.visit_parameter_owned(parameter);
    let default = default.map(|expr| Box::new(transformer.visit_expr_owned(*expr)));
    ast::ParameterWithDefault {
        range,
        node_index,
        parameter,
        default,
    }
}

pub fn walk_parameter_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    parameter: ast::Parameter,
) -> ast::Parameter {
    let ast::Parameter {
        range,
        node_index,
        name,
        annotation,
    } = parameter;
    let annotation = annotation.map(|expr| Box::new(transformer.visit_expr_owned(*expr)));
    ast::Parameter {
        range,
        node_index,
        name,
        annotation,
    }
}

pub fn walk_comprehension_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    comprehension: ast::Comprehension,
) -> ast::Comprehension {
    let ast::Comprehension {
        range,
        node_index,
        target,
        iter,
        ifs,
        is_async,
    } = comprehension;
    let target = transformer.visit_expr_owned(target);
    let iter = transformer.visit_expr_owned(iter);
    let ifs = ifs
        .into_iter()
        .map(|expr| transformer.visit_expr_owned(expr))
        .collect();
    ast::Comprehension {
        range,
        node_index,
        target,
        iter,
        ifs,
        is_async,
    }
}

pub fn walk_with_item_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    with_item: ast::WithItem,
) -> ast::WithItem {
    let ast::WithItem {
        range,
        node_index,
        context_expr,
        optional_vars,
    } = with_item;
    let context_expr = transformer.visit_expr_owned(context_expr);
    let optional_vars = optional_vars.map(|expr| Box::new(transformer.visit_expr_owned(*expr)));
    ast::WithItem {
        range,
        node_index,
        context_expr,
        optional_vars,
    }
}

pub fn walk_type_params_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    type_params: ast::TypeParams,
) -> ast::TypeParams {
    let ast::TypeParams {
        range,
        node_index,
        type_params,
    } = type_params;
    let type_params = type_params
        .into_iter()
        .map(|tp| transformer.visit_type_param_owned(tp))
        .collect();
    ast::TypeParams {
        range,
        node_index,
        type_params,
    }
}

pub fn walk_type_param_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    type_param: ast::TypeParam,
) -> ast::TypeParam {
    match type_param {
        ast::TypeParam::TypeVar(mut node) => {
            node.bound = node
                .bound
                .map(|bound| Box::new(transformer.visit_expr_owned(*bound)));
            node.default = node
                .default
                .map(|default| Box::new(transformer.visit_expr_owned(*default)));
            ast::TypeParam::TypeVar(node)
        }
        ast::TypeParam::TypeVarTuple(mut node) => {
            node.default = node
                .default
                .map(|default| Box::new(transformer.visit_expr_owned(*default)));
            ast::TypeParam::TypeVarTuple(node)
        }
        ast::TypeParam::ParamSpec(mut node) => {
            node.default = node
                .default
                .map(|default| Box::new(transformer.visit_expr_owned(*default)));
            ast::TypeParam::ParamSpec(node)
        }
    }
}

pub fn walk_match_case_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    case: ast::MatchCase,
) -> ast::MatchCase {
    let ast::MatchCase {
        range,
        node_index,
        pattern,
        guard,
        body,
    } = case;
    let pattern = transformer.visit_pattern_owned(pattern);
    let guard = guard.map(|expr| Box::new(transformer.visit_expr_owned(*expr)));
    let body = body
        .into_iter()
        .map(|stmt| transformer.visit_stmt_owned(stmt))
        .collect();
    ast::MatchCase {
        range,
        node_index,
        pattern,
        guard,
        body,
    }
}

pub fn walk_pattern_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    pattern: ast::Pattern,
) -> ast::Pattern {
    match pattern {
        ast::Pattern::MatchValue(mut node) => {
            node.value = Box::new(transformer.visit_expr_owned(*node.value));
            ast::Pattern::MatchValue(node)
        }
        ast::Pattern::MatchSingleton(node) => ast::Pattern::MatchSingleton(node),
        ast::Pattern::MatchSequence(mut node) => {
            node.patterns = node
                .patterns
                .into_iter()
                .map(|p| transformer.visit_pattern_owned(p))
                .collect();
            ast::Pattern::MatchSequence(node)
        }
        ast::Pattern::MatchMapping(mut node) => {
            node.keys = node
                .keys
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            node.patterns = node
                .patterns
                .into_iter()
                .map(|p| transformer.visit_pattern_owned(p))
                .collect();
            ast::Pattern::MatchMapping(node)
        }
        ast::Pattern::MatchClass(mut node) => {
            node.cls = Box::new(transformer.visit_expr_owned(*node.cls));
            node.arguments = transformer.visit_pattern_arguments_owned(node.arguments);
            ast::Pattern::MatchClass(node)
        }
        ast::Pattern::MatchStar(node) => ast::Pattern::MatchStar(node),
        ast::Pattern::MatchAs(mut node) => {
            node.pattern = node
                .pattern
                .map(|pattern| Box::new(transformer.visit_pattern_owned(*pattern)));
            ast::Pattern::MatchAs(node)
        }
        ast::Pattern::MatchOr(mut node) => {
            node.patterns = node
                .patterns
                .into_iter()
                .map(|p| transformer.visit_pattern_owned(p))
                .collect();
            ast::Pattern::MatchOr(node)
        }
    }
}

pub fn walk_pattern_arguments_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    args: ast::PatternArguments,
) -> ast::PatternArguments {
    let ast::PatternArguments {
        range,
        node_index,
        patterns,
        keywords,
    } = args;
    let patterns = patterns
        .into_iter()
        .map(|p| transformer.visit_pattern_owned(p))
        .collect();
    let keywords = keywords
        .into_iter()
        .map(|k| transformer.visit_pattern_keyword_owned(k))
        .collect();
    ast::PatternArguments {
        range,
        node_index,
        patterns,
        keywords,
    }
}

pub fn walk_pattern_keyword_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    keyword: ast::PatternKeyword,
) -> ast::PatternKeyword {
    let ast::PatternKeyword {
        range,
        node_index,
        attr,
        pattern,
    } = keyword;
    let pattern = transformer.visit_pattern_owned(pattern);
    ast::PatternKeyword {
        range,
        node_index,
        attr,
        pattern,
    }
}

pub fn walk_decorator_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    decorator: ast::Decorator,
) -> ast::Decorator {
    let ast::Decorator {
        range,
        node_index,
        expression,
    } = decorator;
    let expression = transformer.visit_expr_owned(expression);
    ast::Decorator {
        range,
        node_index,
        expression,
    }
}

pub fn walk_except_handler_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    handler: ast::ExceptHandler,
) -> ast::ExceptHandler {
    match handler {
        ast::ExceptHandler::ExceptHandler(mut node) => {
            node.type_ = node
                .type_
                .map(|type_| Box::new(transformer.visit_expr_owned(*type_)));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            ast::ExceptHandler::ExceptHandler(node)
        }
    }
}

pub fn walk_elif_else_clause_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    clause: ast::ElifElseClause,
) -> ast::ElifElseClause {
    let ast::ElifElseClause {
        range,
        node_index,
        test,
        body,
    } = clause;
    let test = test.map(|expr| transformer.visit_expr_owned(expr));
    let body = body
        .into_iter()
        .map(|stmt| transformer.visit_stmt_owned(stmt))
        .collect();
    ast::ElifElseClause {
        range,
        node_index,
        test,
        body,
    }
}

pub fn walk_f_string_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    f_string: &mut ast::FString,
) {
    for element in &mut f_string.elements {
        *element = transformer.visit_interpolated_string_element_owned(element.clone());
    }
}

pub fn walk_t_string_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    t_string: &mut ast::TString,
) {
    for element in &mut t_string.elements {
        *element = transformer.visit_interpolated_string_element_owned(element.clone());
    }
}

pub fn walk_interpolated_string_element_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    element: ast::InterpolatedStringElement,
) -> ast::InterpolatedStringElement {
    match element {
        ast::InterpolatedStringElement::Interpolation(mut node) => {
            let expr = (*node.expression).clone();
            node.expression = Box::new(transformer.visit_expr_owned(expr));
            node.format_spec.as_mut().map(|spec| {
                let spec_value = (**spec).clone();
                **spec = transformer.visit_interpolated_string_format_spec_owned(spec_value);
            });
            ast::InterpolatedStringElement::Interpolation(node)
        }
        ast::InterpolatedStringElement::Literal(node) => {
            ast::InterpolatedStringElement::Literal(node)
        }
    }
}

pub fn walk_interpolated_string_format_spec_owned<T: OwnedTransform + ?Sized>(
    transformer: &T,
    mut spec: ast::InterpolatedStringFormatSpec,
) -> ast::InterpolatedStringFormatSpec {
    for element in &mut spec.elements {
        *element = transformer.visit_interpolated_string_element_owned(element.clone());
    }
    spec
}
