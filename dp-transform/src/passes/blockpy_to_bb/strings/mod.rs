use crate::block_py::{
    map_call_args_with, map_keyword_args_with, BlockPyModule, BlockPyModuleMap, CoreBlockPyCall,
    CoreBlockPyCallArg, CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBytesLiteral, IntrinsicCall,
    LocatedCoreBlockPyExpr, LocatedName, NameLocation,
};
use crate::passes::PreparedBbBlockPyPass;
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;

pub fn normalize_bb_module_strings(
    module: &BlockPyModule<PreparedBbBlockPyPass>,
    source: &str,
) -> BlockPyModule<PreparedBbBlockPyPass> {
    module.clone().map_module(&CodegenExprNormalizer { source })
}

struct CodegenExprNormalizer<'a> {
    source: &'a str,
}

impl BlockPyModuleMap<PreparedBbBlockPyPass, PreparedBbBlockPyPass> for CodegenExprNormalizer<'_> {
    fn map_expr(&self, expr: LocatedCoreBlockPyExpr) -> LocatedCoreBlockPyExpr {
        let expr = match expr {
            LocatedCoreBlockPyExpr::Call(call) => LocatedCoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(self.map_expr(*call.func)),
                args: map_call_args_with(call.args, |expr| self.map_expr(expr)),
                keywords: map_keyword_args_with(call.keywords, |expr| self.map_expr(expr)),
            }),
            LocatedCoreBlockPyExpr::Intrinsic(IntrinsicCall {
                intrinsic,
                node_index,
                range,
                args,
            }) => LocatedCoreBlockPyExpr::Intrinsic(IntrinsicCall {
                intrinsic,
                node_index,
                range,
                args: args.into_iter().map(|expr| self.map_expr(expr)).collect(),
            }),
            LocatedCoreBlockPyExpr::Name(_) | LocatedCoreBlockPyExpr::Literal(_) => expr,
        };

        match expr {
            LocatedCoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(node)) => {
                let meta = (node.node_index.clone(), node.range);
                if let Some(src) = source_slice(self.source, node.range) {
                    if has_surrogate_escape(src) {
                        let wrapped = format!("({src})");
                        return decode_literal_source_bytes_call_expr(wrapped.as_bytes(), meta);
                    }
                }
                str_bytes_call_expr_with_meta(node.value.as_bytes(), meta)
            }
            _ => expr,
        }
    }
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

fn source_slice(source: &str, range: TextRange) -> Option<&str> {
    let start = range.start().to_usize();
    let end = range.end().to_usize();
    source.get(start..end)
}

fn has_surrogate_escape(content: &str) -> bool {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        match bytes[i + 1] {
            b'u' => {
                if i + 5 < bytes.len() {
                    if let Some(value) = parse_hex(&bytes[i + 2..i + 6]) {
                        if (0xD800..=0xDFFF).contains(&value) {
                            return true;
                        }
                    }
                    i += 6;
                    continue;
                }
                i += 2;
            }
            b'U' => {
                if i + 9 < bytes.len() {
                    if let Some(value) = parse_hex(&bytes[i + 2..i + 10]) {
                        if (0xD800..=0xDFFF).contains(&value) {
                            return true;
                        }
                    }
                    i += 10;
                    continue;
                }
                i += 2;
            }
            _ => {
                i += 2;
            }
        }
    }
    false
}

fn parse_hex(bytes: &[u8]) -> Option<u32> {
    let mut value: u32 = 0;
    for &b in bytes {
        value <<= 4;
        value |= match b {
            b'0'..=b'9' => (b - b'0') as u32,
            b'a'..=b'f' => (b - b'a' + 10) as u32,
            b'A'..=b'F' => (b - b'A' + 10) as u32,
            _ => return None,
        };
    }
    Some(value)
}

fn load_name(id: &str) -> LocatedName {
    LocatedName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
        location: NameLocation::Global,
    }
}

fn bytes_literal_expr_with_meta(
    bytes: &[u8],
    (node_index, range): (ast::AtomicNodeIndex, TextRange),
) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Literal(CoreBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
        range,
        node_index,
        value: bytes.to_vec(),
    }))
}

fn helper_call_expr_with_meta(
    helper_name: &str,
    args: Vec<LocatedCoreBlockPyExpr>,
    (node_index, range): (ast::AtomicNodeIndex, TextRange),
) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Call(CoreBlockPyCall {
        node_index,
        range,
        func: Box::new(LocatedCoreBlockPyExpr::Name(load_name(helper_name))),
        args: args
            .into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords: Vec::<CoreBlockPyKeywordArg<LocatedCoreBlockPyExpr>>::new(),
    })
}

fn str_bytes_call_expr_with_meta(
    bytes: &[u8],
    meta: (ast::AtomicNodeIndex, TextRange),
) -> LocatedCoreBlockPyExpr {
    helper_call_expr_with_meta(
        "str",
        vec![bytes_literal_expr_with_meta(bytes, meta.clone())],
        meta,
    )
}

fn decode_literal_source_bytes_call_expr(
    bytes: &[u8],
    meta: (ast::AtomicNodeIndex, TextRange),
) -> LocatedCoreBlockPyExpr {
    helper_call_expr_with_meta(
        "__dp_decode_literal_source_bytes",
        vec![bytes_literal_expr_with_meta(bytes, meta.clone())],
        meta,
    )
}

#[cfg(test)]
mod test;
