use ruff_python_ast::{Expr, HasNodeIndex, NodeIndex, Stmt};

use crate::body_transform::{walk_expr, walk_stmt, Transformer};


pub(crate) fn ensure_node_indices(body: &mut Vec<Stmt>) {
    let mut scanner = NodeIndexScanner::new();
    scanner.visit_body(body);
    if !scanner.has_missing() {
        return;
    }
    let mut assigner = NodeIndexEnsurer::new(scanner.next_index());
    assigner.visit_body(body);
}


struct NodeIndexScanner {
    max: u32,
    saw_any: bool,
    saw_missing: bool,
}

impl NodeIndexScanner {
    fn new() -> Self {
        Self {
            max: 0,
            saw_any: false,
            saw_missing: false,
        }
    }

    fn observe<T: HasNodeIndex>(&mut self, node: &T) {
        if let Some(value) = node.node_index().load().as_u32() {
            self.saw_any = true;
            self.max = self.max.max(value);
        } else {
            self.saw_missing = true;
        }
    }

    fn has_missing(&self) -> bool {
        self.saw_missing
    }

    fn next_index(&self) -> u32 {
        if self.saw_any {
            self.max + 1
        } else {
            1
        }
    }
}

struct NodeIndexEnsurer {
    next: u32,
}

impl NodeIndexEnsurer {
    fn new(next: u32) -> Self {
        Self { next }
    }

    fn ensure<T: HasNodeIndex>(&mut self, node: &T) {
        if node.node_index().load().as_u32().is_none() {
            let index = NodeIndex::from(self.next);
            self.next += 1;
            node.node_index().set(index);
        }
    }
}

impl Transformer for NodeIndexScanner {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        self.observe(stmt);
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        self.observe(expr);
        walk_expr(self, expr);
    }
}

impl Transformer for NodeIndexEnsurer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        self.ensure(stmt);
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        self.ensure(expr);
        walk_expr(self, expr);
    }
}
