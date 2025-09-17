use std::borrow::Borrow;

use ruff_python_ast::{self as ast, Expr, Stmt};

pub trait OwnNode<T>: Borrow<T> + From<T> + Clone {}

impl<T> OwnNode<T> for T where T: From<T> + Borrow<T> + Clone {}

impl<T> OwnNode<T> for Box<T> where Box<T>: Borrow<T> + From<T> + Clone {}

/// A trait for transforming owned AST nodes.
pub trait OwnedTransform {
    /// Transform an [`Expr`], returning the transformed expression.
    fn visit_expr_owned<N>(&self, expr: N) -> N
    where
        N: OwnNode<Expr>,
    {
        walk_expr_owned(self, expr)
    }

    /// Transform a [`Stmt`], returning the transformed statement.
    fn visit_stmt_owned<N>(&self, stmt: N) -> N
    where
        N: OwnNode<Stmt>,
    {
        walk_stmt_owned(self, stmt)
    }

    fn visit_arguments_owned<N>(&self, arguments: N) -> N
    where
        N: OwnNode<ast::Arguments>,
    {
        walk_arguments_owned(self, arguments)
    }

    fn visit_keyword_owned<N>(&self, keyword: N) -> N
    where
        N: OwnNode<ast::Keyword>,
    {
        walk_keyword_owned(self, keyword)
    }

    fn visit_parameters_owned<N>(&self, parameters: N) -> N
    where
        N: OwnNode<ast::Parameters>,
    {
        walk_parameters_owned(self, parameters)
    }

    fn visit_parameter_with_default_owned<N>(&self, param: N) -> N
    where
        N: OwnNode<ast::ParameterWithDefault>,
    {
        walk_parameter_with_default_owned(self, param)
    }

    fn visit_parameter_owned<N>(&self, parameter: N) -> N
    where
        N: OwnNode<ast::Parameter>,
    {
        walk_parameter_owned(self, parameter)
    }

    fn visit_comprehension_owned<N>(&self, comprehension: N) -> N
    where
        N: OwnNode<ast::Comprehension>,
    {
        walk_comprehension_owned(self, comprehension)
    }

    fn visit_with_item_owned<N>(&self, with_item: N) -> N
    where
        N: OwnNode<ast::WithItem>,
    {
        walk_with_item_owned(self, with_item)
    }

    fn visit_type_params_owned<N>(&self, type_params: N) -> N
    where
        N: OwnNode<ast::TypeParams>,
    {
        walk_type_params_owned(self, type_params)
    }

    fn visit_type_param_owned<N>(&self, type_param: N) -> N
    where
        N: OwnNode<ast::TypeParam>,
    {
        walk_type_param_owned(self, type_param)
    }

    fn visit_match_case_owned<N>(&self, case: N) -> N
    where
        N: OwnNode<ast::MatchCase>,
    {
        walk_match_case_owned(self, case)
    }

    fn visit_pattern_owned<N>(&self, pattern: N) -> N
    where
        N: OwnNode<ast::Pattern>,
    {
        walk_pattern_owned(self, pattern)
    }

    fn visit_pattern_arguments_owned<N>(&self, args: N) -> N
    where
        N: OwnNode<ast::PatternArguments>,
    {
        walk_pattern_arguments_owned(self, args)
    }

    fn visit_pattern_keyword_owned<N>(&self, keyword: N) -> N
    where
        N: OwnNode<ast::PatternKeyword>,
    {
        walk_pattern_keyword_owned(self, keyword)
    }

    fn visit_decorator_owned<N>(&self, decorator: N) -> N
    where
        N: OwnNode<ast::Decorator>,
    {
        walk_decorator_owned(self, decorator)
    }

    fn visit_except_handler_owned<N>(&self, handler: N) -> N
    where
        N: OwnNode<ast::ExceptHandler>,
    {
        walk_except_handler_owned(self, handler)
    }

    fn visit_elif_else_clause_owned<N>(&self, clause: N) -> N
    where
        N: OwnNode<ast::ElifElseClause>,
    {
        walk_elif_else_clause_owned(self, clause)
    }

    fn visit_f_string_owned(&self, f_string: &mut ast::FString) {
        walk_f_string_owned(self, f_string)
    }

    fn visit_t_string_owned(&self, t_string: &mut ast::TString) {
        walk_t_string_owned(self, t_string)
    }

    fn visit_interpolated_string_element_owned<N>(&self, element: N) -> N
    where
        N: OwnNode<ast::InterpolatedStringElement>,
    {
        walk_interpolated_string_element_owned(self, element)
    }

    fn visit_interpolated_string_format_spec_owned<N>(&self, spec: N) -> N
    where
        N: OwnNode<ast::InterpolatedStringFormatSpec>,
    {
        walk_interpolated_string_format_spec_owned(self, spec)
    }
}

pub fn walk_expr_owned<T, N>(transformer: &T, expr: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<Expr>,
{
    let expr = expr.borrow().clone();

    let expr = match expr {
        Expr::BoolOp(mut node) => {
            node.values = node
                .values
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Expr::BoolOp(node)
        }
        Expr::Named(mut node) => {
            node.target = transformer.visit_expr_owned(node.target);
            node.value = transformer.visit_expr_owned(node.value);
            Expr::Named(node)
        }
        Expr::BinOp(mut node) => {
            node.left = transformer.visit_expr_owned(node.left);
            node.right = transformer.visit_expr_owned(node.right);
            Expr::BinOp(node)
        }
        Expr::UnaryOp(mut node) => {
            node.operand = transformer.visit_expr_owned(node.operand);
            Expr::UnaryOp(node)
        }
        Expr::Lambda(mut node) => {
            node.parameters = node
                .parameters
                .map(|parameters| transformer.visit_parameters_owned(parameters));
            node.body = transformer.visit_expr_owned(node.body);
            Expr::Lambda(node)
        }
        Expr::If(mut node) => {
            node.test = transformer.visit_expr_owned(node.test);
            node.body = transformer.visit_expr_owned(node.body);
            node.orelse = transformer.visit_expr_owned(node.orelse);
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
            node.elt = transformer.visit_expr_owned(node.elt);
            Expr::ListComp(node)
        }
        Expr::SetComp(mut node) => {
            node.generators = node
                .generators
                .into_iter()
                .map(|comp| transformer.visit_comprehension_owned(comp))
                .collect();
            node.elt = transformer.visit_expr_owned(node.elt);
            Expr::SetComp(node)
        }
        Expr::DictComp(mut node) => {
            node.generators = node
                .generators
                .into_iter()
                .map(|comp| transformer.visit_comprehension_owned(comp))
                .collect();
            node.key = transformer.visit_expr_owned(node.key);
            node.value = transformer.visit_expr_owned(node.value);
            Expr::DictComp(node)
        }
        Expr::Generator(mut node) => {
            node.generators = node
                .generators
                .into_iter()
                .map(|comp| transformer.visit_comprehension_owned(comp))
                .collect();
            node.elt = transformer.visit_expr_owned(node.elt);
            Expr::Generator(node)
        }
        Expr::Await(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
            Expr::Await(node)
        }
        Expr::Yield(mut node) => {
            node.value = node.value.map(|value| transformer.visit_expr_owned(value));
            Expr::Yield(node)
        }
        Expr::YieldFrom(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
            Expr::YieldFrom(node)
        }
        Expr::Compare(mut node) => {
            node.left = transformer.visit_expr_owned(node.left);
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
            node.func = transformer.visit_expr_owned(node.func);
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
            node.value = transformer.visit_expr_owned(node.value);
            Expr::Attribute(node)
        }
        Expr::Subscript(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
            node.slice = transformer.visit_expr_owned(node.slice);
            Expr::Subscript(node)
        }
        Expr::Starred(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
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
            node.lower = node.lower.map(|expr| transformer.visit_expr_owned(expr));
            node.upper = node.upper.map(|expr| transformer.visit_expr_owned(expr));
            node.step = node.step.map(|expr| transformer.visit_expr_owned(expr));
            Expr::Slice(node)
        }
        Expr::IpyEscapeCommand(node) => Expr::IpyEscapeCommand(node),
    };

    N::from(expr)
}

pub fn walk_stmt_owned<T, N>(transformer: &T, stmt: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<Stmt>,
{
    let stmt = stmt.borrow().clone();

    let stmt = match stmt {
        Stmt::FunctionDef(mut node) => {
            node.decorator_list = node
                .decorator_list
                .into_iter()
                .map(|decorator| transformer.visit_decorator_owned(decorator))
                .collect();
            node.type_params = node
                .type_params
                .map(|type_params| transformer.visit_type_params_owned(type_params));
            node.parameters = transformer.visit_parameters_owned(node.parameters);
            node.returns = node
                .returns
                .map(|returns| transformer.visit_expr_owned(returns));
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
                .map(|type_params| transformer.visit_type_params_owned(type_params));
            node.arguments = node
                .arguments
                .map(|arguments| transformer.visit_arguments_owned(arguments));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            Stmt::ClassDef(node)
        }
        Stmt::Return(mut node) => {
            node.value = node.value.map(|value| transformer.visit_expr_owned(value));
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
            node.value = transformer.visit_expr_owned(node.value);
            node.type_params = node
                .type_params
                .map(|type_params| transformer.visit_type_params_owned(type_params));
            node.name = transformer.visit_expr_owned(node.name);
            Stmt::TypeAlias(node)
        }
        Stmt::Assign(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
            node.targets = node
                .targets
                .into_iter()
                .map(|expr| transformer.visit_expr_owned(expr))
                .collect();
            Stmt::Assign(node)
        }
        Stmt::AugAssign(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
            node.target = transformer.visit_expr_owned(node.target);
            Stmt::AugAssign(node)
        }
        Stmt::AnnAssign(mut node) => {
            node.value = node.value.map(|value| transformer.visit_expr_owned(value));
            node.annotation = transformer.visit_expr_owned(node.annotation);
            node.target = transformer.visit_expr_owned(node.target);
            Stmt::AnnAssign(node)
        }
        Stmt::For(mut node) => {
            node.iter = transformer.visit_expr_owned(node.iter);
            node.target = transformer.visit_expr_owned(node.target);
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
            node.test = transformer.visit_expr_owned(node.test);
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
            node.test = transformer.visit_expr_owned(node.test);
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
            node.subject = transformer.visit_expr_owned(node.subject);
            node.cases = node
                .cases
                .into_iter()
                .map(|case| transformer.visit_match_case_owned(case))
                .collect();
            Stmt::Match(node)
        }
        Stmt::Raise(mut node) => {
            node.exc = node.exc.map(|exc| transformer.visit_expr_owned(exc));
            node.cause = node.cause.map(|cause| transformer.visit_expr_owned(cause));
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
            node.test = transformer.visit_expr_owned(node.test);
            node.msg = node.msg.map(|msg| transformer.visit_expr_owned(msg));
            Stmt::Assert(node)
        }
        Stmt::Import(node) => Stmt::Import(node),
        Stmt::ImportFrom(node) => Stmt::ImportFrom(node),
        Stmt::Global(node) => Stmt::Global(node),
        Stmt::Nonlocal(node) => Stmt::Nonlocal(node),
        Stmt::Expr(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
            Stmt::Expr(node)
        }
        Stmt::Pass(node) => Stmt::Pass(node),
        Stmt::Break(node) => Stmt::Break(node),
        Stmt::Continue(node) => Stmt::Continue(node),
        Stmt::IpyEscapeCommand(node) => Stmt::IpyEscapeCommand(node),
    };

    N::from(stmt)
}

pub fn walk_arguments_owned<T, N>(transformer: &T, arguments: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::Arguments>,
{
    let mut arguments = arguments.borrow().clone();
    let args = arguments
        .args
        .into_vec()
        .into_iter()
        .map(|expr| transformer.visit_expr_owned(expr))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let keywords = arguments
        .keywords
        .into_vec()
        .into_iter()
        .map(|kw| transformer.visit_keyword_owned(kw))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    arguments.args = args;
    arguments.keywords = keywords;
    N::from(arguments)
}

pub fn walk_keyword_owned<T, N>(transformer: &T, keyword: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::Keyword>,
{
    let mut keyword = keyword.borrow().clone();
    keyword.value = transformer.visit_expr_owned(keyword.value);
    N::from(keyword)
}

pub fn walk_parameters_owned<T, N>(transformer: &T, parameters: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::Parameters>,
{
    let mut parameters = parameters.borrow().clone();
    parameters.posonlyargs = parameters
        .posonlyargs
        .into_iter()
        .map(|p| transformer.visit_parameter_with_default_owned(p))
        .collect();
    parameters.args = parameters
        .args
        .into_iter()
        .map(|p| transformer.visit_parameter_with_default_owned(p))
        .collect();
    parameters.vararg = parameters
        .vararg
        .map(|p| transformer.visit_parameter_owned(p));
    parameters.kwonlyargs = parameters
        .kwonlyargs
        .into_iter()
        .map(|p| transformer.visit_parameter_with_default_owned(p))
        .collect();
    parameters.kwarg = parameters
        .kwarg
        .map(|p| transformer.visit_parameter_owned(p));
    N::from(parameters)
}

pub fn walk_parameter_with_default_owned<T, N>(transformer: &T, param: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::ParameterWithDefault>,
{
    let mut param = param.borrow().clone();
    param.parameter = transformer.visit_parameter_owned(param.parameter);
    param.default = param.default.map(|expr| transformer.visit_expr_owned(expr));
    N::from(param)
}

pub fn walk_parameter_owned<T, N>(transformer: &T, parameter: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::Parameter>,
{
    let mut parameter = parameter.borrow().clone();
    parameter.annotation = parameter
        .annotation
        .map(|expr| transformer.visit_expr_owned(expr));
    N::from(parameter)
}

pub fn walk_comprehension_owned<T, N>(transformer: &T, comprehension: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::Comprehension>,
{
    let mut comprehension = comprehension.borrow().clone();
    comprehension.target = transformer.visit_expr_owned(comprehension.target);
    comprehension.iter = transformer.visit_expr_owned(comprehension.iter);
    comprehension.ifs = comprehension
        .ifs
        .into_iter()
        .map(|expr| transformer.visit_expr_owned(expr))
        .collect();
    N::from(comprehension)
}

pub fn walk_with_item_owned<T, N>(transformer: &T, with_item: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::WithItem>,
{
    let mut with_item = with_item.borrow().clone();
    with_item.context_expr = transformer.visit_expr_owned(with_item.context_expr);
    with_item.optional_vars = with_item
        .optional_vars
        .map(|expr| transformer.visit_expr_owned(expr));
    N::from(with_item)
}

pub fn walk_type_params_owned<T, N>(transformer: &T, type_params: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::TypeParams>,
{
    let mut type_params = type_params.borrow().clone();
    type_params.type_params = type_params
        .type_params
        .into_iter()
        .map(|tp| transformer.visit_type_param_owned(tp))
        .collect();
    N::from(type_params)
}

pub fn walk_type_param_owned<T, N>(transformer: &T, type_param: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::TypeParam>,
{
    let type_param = type_param.borrow().clone();
    let type_param = match type_param {
        ast::TypeParam::TypeVar(mut node) => {
            node.bound = node.bound.map(|bound| transformer.visit_expr_owned(bound));
            node.default = node
                .default
                .map(|default| transformer.visit_expr_owned(default));
            ast::TypeParam::TypeVar(node)
        }
        ast::TypeParam::TypeVarTuple(mut node) => {
            node.default = node
                .default
                .map(|default| transformer.visit_expr_owned(default));
            ast::TypeParam::TypeVarTuple(node)
        }
        ast::TypeParam::ParamSpec(mut node) => {
            node.default = node
                .default
                .map(|default| transformer.visit_expr_owned(default));
            ast::TypeParam::ParamSpec(node)
        }
    };

    N::from(type_param)
}

pub fn walk_match_case_owned<T, N>(transformer: &T, case: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::MatchCase>,
{
    let mut case = case.borrow().clone();
    case.pattern = transformer.visit_pattern_owned(case.pattern);
    case.guard = case.guard.map(|expr| transformer.visit_expr_owned(expr));
    case.body = case
        .body
        .into_iter()
        .map(|stmt| transformer.visit_stmt_owned(stmt))
        .collect();
    N::from(case)
}

pub fn walk_pattern_owned<T, N>(transformer: &T, pattern: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::Pattern>,
{
    let pattern = pattern.borrow().clone();
    let pattern = match pattern {
        ast::Pattern::MatchValue(mut node) => {
            node.value = transformer.visit_expr_owned(node.value);
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
            node.cls = transformer.visit_expr_owned(node.cls);
            node.arguments = transformer.visit_pattern_arguments_owned(node.arguments);
            ast::Pattern::MatchClass(node)
        }
        ast::Pattern::MatchStar(node) => ast::Pattern::MatchStar(node),
        ast::Pattern::MatchAs(mut node) => {
            node.pattern = node
                .pattern
                .map(|pattern| transformer.visit_pattern_owned(pattern));
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
    };

    N::from(pattern)
}

pub fn walk_pattern_arguments_owned<T, N>(transformer: &T, args: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::PatternArguments>,
{
    let mut args = args.borrow().clone();
    args.patterns = args
        .patterns
        .into_iter()
        .map(|p| transformer.visit_pattern_owned(p))
        .collect();
    args.keywords = args
        .keywords
        .into_iter()
        .map(|k| transformer.visit_pattern_keyword_owned(k))
        .collect();
    N::from(args)
}

pub fn walk_pattern_keyword_owned<T, N>(transformer: &T, keyword: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::PatternKeyword>,
{
    let mut keyword = keyword.borrow().clone();
    keyword.pattern = transformer.visit_pattern_owned(keyword.pattern);
    N::from(keyword)
}

pub fn walk_decorator_owned<T, N>(transformer: &T, decorator: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::Decorator>,
{
    let mut decorator = decorator.borrow().clone();
    decorator.expression = transformer.visit_expr_owned(decorator.expression);
    N::from(decorator)
}

pub fn walk_except_handler_owned<T, N>(transformer: &T, handler: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::ExceptHandler>,
{
    let handler = handler.borrow().clone();
    let handler = match handler {
        ast::ExceptHandler::ExceptHandler(mut node) => {
            node.type_ = node.type_.map(|type_| transformer.visit_expr_owned(type_));
            node.body = node
                .body
                .into_iter()
                .map(|stmt| transformer.visit_stmt_owned(stmt))
                .collect();
            ast::ExceptHandler::ExceptHandler(node)
        }
    };

    N::from(handler)
}

pub fn walk_elif_else_clause_owned<T, N>(transformer: &T, clause: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::ElifElseClause>,
{
    let mut clause = clause.borrow().clone();
    clause.test = clause.test.map(|expr| transformer.visit_expr_owned(expr));
    clause.body = clause
        .body
        .into_iter()
        .map(|stmt| transformer.visit_stmt_owned(stmt))
        .collect();
    N::from(clause)
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

pub fn walk_interpolated_string_element_owned<T, N>(transformer: &T, element: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::InterpolatedStringElement>,
{
    let element = element.borrow().clone();
    let element = match element {
        ast::InterpolatedStringElement::Interpolation(mut node) => {
            let expression = node.expression;
            node.expression = transformer.visit_expr_owned(expression);
            node.format_spec.as_mut().map(|spec| {
                let spec_value = (**spec).clone();
                **spec = transformer.visit_interpolated_string_format_spec_owned(spec_value);
            });
            ast::InterpolatedStringElement::Interpolation(node)
        }
        ast::InterpolatedStringElement::Literal(node) => {
            ast::InterpolatedStringElement::Literal(node)
        }
    };

    N::from(element)
}

pub fn walk_interpolated_string_format_spec_owned<T, N>(transformer: &T, spec: N) -> N
where
    T: OwnedTransform + ?Sized,
    N: OwnNode<ast::InterpolatedStringFormatSpec>,
{
    let mut spec = spec.borrow().clone();
    for element in &mut spec.elements {
        *element = transformer.visit_interpolated_string_element_owned(element.clone());
    }
    N::from(spec)
}
