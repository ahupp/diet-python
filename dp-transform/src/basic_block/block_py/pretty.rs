use super::state::collect_parameter_names;
use super::{
    BlockPyBlock, BlockPyCallableDef, BlockPyCfgFragment, BlockPyExpr, BlockPyFunctionKind,
    BlockPyIfTerm, BlockPyLabel, BlockPyModule, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    BlockPyTryJump,
};
use crate::ruff_ast_to_string;
use ruff_python_ast::{self as ast, Expr};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum IfBranchKind {
    Then,
    Else,
}

pub fn blockpy_module_to_string<E>(module: &BlockPyModule<E>) -> String
where
    E: Clone + Into<Expr>,
{
    let mut formatter = BlockPyFormatter::default();
    formatter.write_module(module);
    formatter.finish()
}

#[derive(Default)]
struct BlockPyFormatter {
    out: String,
    indent: usize,
}

impl BlockPyFormatter {
    fn finish(mut self) -> String {
        if self.out.is_empty() {
            self.line("; empty BlockPy module");
        }
        self.out
    }

    fn write_module<E>(&mut self, module: &BlockPyModule<E>)
    where
        E: Clone + Into<Expr>,
    {
        if let Some(module_init) = &module.module_init {
            self.line(format!("module_init: {module_init}"));
        }

        for function in &module.callable_defs {
            if !self.out.is_empty() {
                self.out.push('\n');
            }
            self.write_function(function);
        }
    }

    fn write_function<E>(&mut self, function: &BlockPyCallableDef<E>)
    where
        E: Clone + Into<Expr>,
    {
        let params = format_parameters(&function.params);
        let parameter_names = collect_parameter_names(&function.params);
        let referenced_labels = collect_referenced_labels_from_blocks(&function.blocks);
        let render_layout = BlockRenderLayout::new(function);
        self.line(format!("function {}({params})", function.bind_name));
        self.with_indent(|this| {
            this.line(format!("kind: {}", function_kind_name(function.kind)));
            this.line(format!("bind: {}", function.bind_name));
            this.line(format!("qualname: {}", function.qualname));
            if function.display_name != function.bind_name {
                this.line(format!("display_name: {}", function.display_name));
            }
            if !function.entry_liveins.is_empty() && function.entry_liveins != parameter_names {
                this.line(format!(
                    "entry_liveins: [{}]",
                    function.entry_liveins.join(", ")
                ));
            }
            if !function.local_cell_slots.is_empty() {
                this.line(format!(
                    "local_cell_slots: [{}]",
                    function.local_cell_slots.join(", ")
                ));
            }
            if let Some(layout) = &function.closure_layout {
                if !layout.freevars.is_empty() {
                    this.line(format!(
                        "freevars: [{}]",
                        render_closure_slots(&layout.freevars)
                    ));
                }
                if !layout.cellvars.is_empty() {
                    this.line(format!(
                        "cellvars: [{}]",
                        render_closure_slots(&layout.cellvars)
                    ));
                }
                if !layout.runtime_cells.is_empty() {
                    this.line(format!(
                        "runtime_cells: [{}]",
                        render_closure_slots(&layout.runtime_cells)
                    ));
                }
            }
            if function.blocks.is_empty() {
                this.line("pass");
            } else {
                for root_block in &render_layout.root_blocks {
                    this.write_function_block(
                        function,
                        &render_layout,
                        *root_block,
                        &referenced_labels,
                    );
                }
            }
        });
    }

    fn write_function_block<E>(
        &mut self,
        function: &BlockPyCallableDef<E>,
        render_layout: &BlockRenderLayout,
        block_index: usize,
        referenced_labels: &HashSet<BlockPyLabel>,
    ) where
        E: Clone + Into<Expr>,
    {
        let block = &function.blocks[block_index];
        self.line(format!("block {}:", block.label.as_str()));
        self.with_indent(|this| {
            this.write_block_contents(
                function,
                render_layout,
                Some(block_index),
                block,
                referenced_labels,
            );
            for child_block in &render_layout.child_blocks[block_index] {
                if render_layout.inlined_blocks.contains(child_block) {
                    continue;
                }
                this.write_function_block(function, render_layout, *child_block, referenced_labels);
            }
        });
    }

    fn write_block_contents<E>(
        &mut self,
        function: &BlockPyCallableDef<E>,
        render_layout: &BlockRenderLayout,
        current_block_index: Option<usize>,
        block: &BlockPyBlock<E>,
        referenced_labels: &HashSet<BlockPyLabel>,
    ) where
        E: Clone + Into<Expr>,
    {
        if block.body.is_empty() {
            self.write_term(
                function,
                render_layout,
                current_block_index,
                &block.term,
                referenced_labels,
            );
            return;
        }
        self.write_stmt_list(&block.body, referenced_labels);
        self.write_term(
            function,
            render_layout,
            current_block_index,
            &block.term,
            referenced_labels,
        );
    }

    fn write_stmt_list<E>(
        &mut self,
        stmts: &[BlockPyStmt<E>],
        referenced_labels: &HashSet<BlockPyLabel>,
    ) where
        E: Clone + Into<Expr>,
    {
        for stmt in stmts {
            self.write_stmt(stmt, referenced_labels);
        }
    }

    fn write_stmt_fragment<E>(
        &mut self,
        fragment: &BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>,
        referenced_labels: &HashSet<BlockPyLabel>,
    ) where
        E: Clone + Into<Expr>,
    {
        if fragment.body.is_empty() && fragment.term.is_none() {
            self.line("pass");
            return;
        }
        self.write_stmt_list(&fragment.body, referenced_labels);
        if let Some(term) = &fragment.term {
            self.write_term_inline(term);
        }
    }

    fn write_stmt<E>(&mut self, stmt: &BlockPyStmt<E>, referenced_labels: &HashSet<BlockPyLabel>)
    where
        E: Clone + Into<Expr>,
    {
        match stmt {
            BlockPyStmt::Pass => self.line("pass"),
            BlockPyStmt::Assign(assign) => self.line(format!(
                "{} = {}",
                assign.target.id,
                render_inline_expr(&assign.value)
            )),
            BlockPyStmt::Expr(expr) => self.line(render_inline_expr(expr)),
            BlockPyStmt::Delete(delete) => self.line(format!("del {}", delete.target.id)),
            BlockPyStmt::If(if_stmt) => {
                self.line(format!("if {}:", render_inline_expr(&if_stmt.test)));
                self.with_indent(|this| this.write_stmt_fragment(&if_stmt.body, referenced_labels));
                if !if_stmt.orelse.body.is_empty() || if_stmt.orelse.term.is_some() {
                    self.line("else:");
                    self.with_indent(|this| {
                        this.write_stmt_fragment(&if_stmt.orelse, referenced_labels)
                    });
                }
            }
        }
    }

    fn write_term<E>(
        &mut self,
        function: &BlockPyCallableDef<E>,
        render_layout: &BlockRenderLayout,
        current_block_index: Option<usize>,
        term: &BlockPyTerm<E>,
        referenced_labels: &HashSet<BlockPyLabel>,
    ) where
        E: Clone + Into<Expr>,
    {
        match term {
            BlockPyTerm::Jump(label) => self.line(format!("jump {}", label.as_str())),
            BlockPyTerm::IfTerm(BlockPyIfTerm {
                test,
                then_label,
                else_label,
            }) => {
                self.line(format!("if_term {}:", render_inline_expr(test)));
                self.with_indent(|this| {
                    this.line("then:");
                    this.with_indent(|this| {
                        if let Some(target_index) = current_block_index.and_then(|block_index| {
                            render_layout
                                .inline_if_term_targets
                                .get(&(block_index, IfBranchKind::Then))
                                .copied()
                        }) {
                            this.write_function_block(
                                function,
                                render_layout,
                                target_index,
                                referenced_labels,
                            );
                        } else {
                            this.line(format!("jump {}", then_label.as_str()));
                        }
                    });
                    this.line("else:");
                    this.with_indent(|this| {
                        if let Some(target_index) = current_block_index.and_then(|block_index| {
                            render_layout
                                .inline_if_term_targets
                                .get(&(block_index, IfBranchKind::Else))
                                .copied()
                        }) {
                            this.write_function_block(
                                function,
                                render_layout,
                                target_index,
                                referenced_labels,
                            );
                        } else {
                            this.line(format!("jump {}", else_label.as_str()));
                        }
                    });
                });
            }
            BlockPyTerm::BranchTable(branch) => self.line(format!(
                "branch_table {} -> [{}] default {}",
                render_inline_expr(&branch.index),
                join_labels(&branch.targets),
                branch.default_label.as_str(),
            )),
            BlockPyTerm::Raise(raise_stmt) => self.write_raise(raise_stmt),
            BlockPyTerm::TryJump(try_jump) => self.write_try_jump(try_jump),
            BlockPyTerm::Return(value) => match value {
                Some(value) => self.line(format!("return {}", render_inline_expr(value))),
                None => self.line("return"),
            },
        }
    }

    fn write_raise<E>(&mut self, raise_stmt: &BlockPyRaise<E>)
    where
        E: Clone + Into<Expr>,
    {
        match &raise_stmt.exc {
            Some(exc) => self.line(format!("raise {}", render_inline_expr(exc))),
            None => self.line("raise"),
        }
    }

    fn write_term_inline<E>(&mut self, term: &BlockPyTerm<E>)
    where
        E: Clone + Into<Expr>,
    {
        match term {
            BlockPyTerm::Jump(label) => self.line(format!("jump {}", label.as_str())),
            BlockPyTerm::BranchTable(branch) => self.line(format!(
                "branch_table {} -> [{}] default {}",
                render_inline_expr(&branch.index),
                join_labels(&branch.targets),
                branch.default_label.as_str(),
            )),
            BlockPyTerm::Raise(raise_stmt) => self.write_raise(raise_stmt),
            BlockPyTerm::TryJump(try_jump) => self.write_try_jump(try_jump),
            BlockPyTerm::Return(value) => match value {
                Some(value) => self.line(format!("return {}", render_inline_expr(value))),
                None => self.line("return"),
            },
            BlockPyTerm::IfTerm(_) => {
                panic!("IfTerm is only valid as a top-level block terminator");
            }
        }
    }

    fn write_try_jump(&mut self, try_jump: &BlockPyTryJump) {
        self.line("try_jump:");
        self.with_indent(|this| {
            this.line(format!("body_label: {}", try_jump.body_label.as_str()));
            this.line(format!("except_label: {}", try_jump.except_label.as_str()));
        });
    }

    fn with_indent(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent += 1;
        f(self);
        self.indent -= 1;
    }

    fn line(&mut self, line: impl AsRef<str>) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(line.as_ref());
        self.out.push('\n');
    }
}

fn render_closure_slots(slots: &[crate::basic_block::bb_ir::BbClosureSlot]) -> String {
    slots
        .iter()
        .map(|slot| {
            format!(
                "{}->{}@{}",
                slot.logical_name,
                slot.storage_name,
                closure_init_name(&slot.init),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn closure_init_name(init: &crate::basic_block::bb_ir::BbClosureInit) -> &'static str {
    match init {
        crate::basic_block::bb_ir::BbClosureInit::InheritedCapture => "inherited",
        crate::basic_block::bb_ir::BbClosureInit::Parameter => "param",
        crate::basic_block::bb_ir::BbClosureInit::DeletedSentinel => "deleted",
        crate::basic_block::bb_ir::BbClosureInit::RuntimePcUnstarted => "pc_unstarted",
        crate::basic_block::bb_ir::BbClosureInit::RuntimeNone => "none",
        crate::basic_block::bb_ir::BbClosureInit::Deferred => "deferred",
    }
}

fn function_kind_name(kind: BlockPyFunctionKind) -> &'static str {
    match kind {
        BlockPyFunctionKind::Function => "function",
        BlockPyFunctionKind::Coroutine => "coroutine",
        BlockPyFunctionKind::Generator => "generator",
        BlockPyFunctionKind::AsyncGenerator => "async_generator",
    }
}

fn render_inline_expr<E>(expr: &E) -> String
where
    E: Clone + Into<Expr>,
{
    ruff_ast_to_string(&expr.clone().into())
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_annotation(annotation: Option<&Expr>) -> String {
    annotation
        .map(|expr| {
            format!(
                ": {}",
                ruff_ast_to_string(expr)
                    .lines()
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .unwrap_or_default()
}

fn render_default(default: Option<&Expr>) -> String {
    default
        .map(|expr| {
            format!(
                " = {}",
                ruff_ast_to_string(expr)
                    .lines()
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .unwrap_or_default()
}

fn format_named_parameter(parameter: &ast::Parameter) -> String {
    format!(
        "{}{}",
        parameter.name.id,
        render_annotation(parameter.annotation.as_deref())
    )
}

fn format_parameter_with_default(parameter: &ast::ParameterWithDefault) -> String {
    format!(
        "{}{}",
        format_named_parameter(&parameter.parameter),
        render_default(parameter.default.as_deref())
    )
}

fn format_parameters(parameters: &ast::Parameters) -> String {
    let mut parts = Vec::new();

    for param in &parameters.posonlyargs {
        parts.push(format_parameter_with_default(param));
    }
    if !parameters.posonlyargs.is_empty() {
        parts.push("/".to_string());
    }

    for param in &parameters.args {
        parts.push(format_parameter_with_default(param));
    }

    if let Some(vararg) = &parameters.vararg {
        parts.push(format!("*{}", format_named_parameter(vararg)));
    } else if !parameters.kwonlyargs.is_empty() {
        parts.push("*".to_string());
    }

    for param in &parameters.kwonlyargs {
        parts.push(format_parameter_with_default(param));
    }

    if let Some(kwarg) = &parameters.kwarg {
        parts.push(format!("**{}", format_named_parameter(kwarg)));
    }

    parts.join(", ")
}

fn join_labels(labels: &[BlockPyLabel]) -> String {
    labels
        .iter()
        .map(|label| label.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug)]
struct BlockRenderLayout {
    root_blocks: Vec<usize>,
    child_blocks: Vec<Vec<usize>>,
    inline_if_term_targets: HashMap<(usize, IfBranchKind), usize>,
    inlined_blocks: HashSet<usize>,
}

impl BlockRenderLayout {
    fn new<E>(function: &BlockPyCallableDef<E>) -> Self
    where
        E: Clone + Into<Expr>,
    {
        let block_count = function.blocks.len();
        if block_count == 0 {
            return Self {
                root_blocks: Vec::new(),
                child_blocks: Vec::new(),
                inline_if_term_targets: HashMap::new(),
                inlined_blocks: HashSet::new(),
            };
        }

        let label_to_index = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.label.as_str().to_string(), index))
            .collect::<HashMap<_, _>>();

        let successors = function
            .blocks
            .iter()
            .map(|block| collect_top_level_successors_from_block(block, &label_to_index))
            .collect::<Vec<_>>();
        let predecessors = collect_predecessors(&successors);
        let entry_index = choose_entry_block_index(function, &label_to_index, &predecessors);
        let discovery_order = collect_discovery_order(entry_index, &successors);
        let reachable = discovery_order.iter().copied().collect::<HashSet<_>>();
        let dominators =
            compute_dominators(entry_index, &discovery_order, &predecessors, &reachable);
        let immediate_dominators =
            compute_immediate_dominators(entry_index, &discovery_order, &dominators, &reachable);

        let mut child_blocks = vec![Vec::new(); block_count];
        for (block_index, immediate_dominator) in immediate_dominators.iter().enumerate() {
            if let Some(parent_index) = immediate_dominator {
                child_blocks[*parent_index].push(block_index);
            }
        }
        for children in &mut child_blocks {
            sort_block_indices_by_label(children, function);
        }

        let (inline_if_term_targets, inlined_blocks) = compute_inline_if_term_targets(
            function,
            &label_to_index,
            &predecessors,
            &immediate_dominators,
        );

        let mut root_blocks = vec![entry_index];
        let reachable_roots = discovery_order
            .iter()
            .copied()
            .filter(|index| *index != entry_index && immediate_dominators[*index].is_none())
            .collect::<Vec<_>>();
        root_blocks.extend(reachable_roots);
        root_blocks.extend((0..block_count).filter(|index| !reachable.contains(index)));
        sort_block_indices_by_label(&mut root_blocks[1..], function);

        Self {
            root_blocks,
            child_blocks,
            inline_if_term_targets,
            inlined_blocks,
        }
    }
}

fn sort_block_indices_by_label<E>(indices: &mut [usize], function: &BlockPyCallableDef<E>)
where
    E: Clone + Into<Expr>,
{
    indices.sort_by(|left, right| {
        function.blocks[*left]
            .label
            .as_str()
            .cmp(function.blocks[*right].label.as_str())
    });
}

fn compute_inline_if_term_targets(
    function: &BlockPyCallableDef<impl Clone + Into<Expr>>,
    label_to_index: &HashMap<String, usize>,
    predecessors: &[Vec<usize>],
    immediate_dominators: &[Option<usize>],
) -> (HashMap<(usize, IfBranchKind), usize>, HashSet<usize>) {
    let mut targets = HashMap::new();
    let mut inlined_blocks = HashSet::new();

    for (block_index, block) in function.blocks.iter().enumerate() {
        let BlockPyTerm::IfTerm(BlockPyIfTerm {
            then_label,
            else_label,
            ..
        }) = &block.term
        else {
            continue;
        };

        let then_target = label_to_index.get(then_label.as_str()).copied();
        let else_target = label_to_index.get(else_label.as_str()).copied();

        if let Some(target_index) = then_target {
            if can_inline_if_term_target(
                block_index,
                target_index,
                else_target,
                predecessors,
                immediate_dominators,
            ) {
                targets.insert((block_index, IfBranchKind::Then), target_index);
                inlined_blocks.insert(target_index);
            }
        }

        if let Some(target_index) = else_target {
            if can_inline_if_term_target(
                block_index,
                target_index,
                then_target,
                predecessors,
                immediate_dominators,
            ) {
                targets.insert((block_index, IfBranchKind::Else), target_index);
                inlined_blocks.insert(target_index);
            }
        }
    }

    (targets, inlined_blocks)
}
fn can_inline_if_term_target(
    parent_index: usize,
    target_index: usize,
    sibling_target: Option<usize>,
    predecessors: &[Vec<usize>],
    immediate_dominators: &[Option<usize>],
) -> bool {
    if sibling_target == Some(target_index) {
        return false;
    }
    immediate_dominators[target_index] == Some(parent_index)
        && predecessors[target_index].len() == 1
        && predecessors[target_index][0] == parent_index
}

fn choose_entry_block_index(
    _function: &BlockPyCallableDef<impl Clone + Into<Expr>>,
    _label_to_index: &HashMap<String, usize>,
    _predecessors: &[Vec<usize>],
) -> usize {
    0
}

fn collect_top_level_successors_from_block(
    block: &BlockPyBlock<impl Clone + Into<Expr>>,
    label_to_index: &HashMap<String, usize>,
) -> Vec<usize> {
    let mut successors = Vec::new();
    let mut seen = HashSet::new();
    collect_top_level_successors_from_stmts(
        &block.body,
        label_to_index,
        &mut seen,
        &mut successors,
    );
    collect_top_level_successors_from_term(&block.term, label_to_index, &mut seen, &mut successors);
    successors
}

fn collect_top_level_successors_from_stmts(
    stmts: &[BlockPyStmt<impl Clone + Into<Expr>>],
    label_to_index: &HashMap<String, usize>,
    seen: &mut HashSet<usize>,
    out: &mut Vec<usize>,
) {
    for stmt in stmts {
        match stmt {
            BlockPyStmt::If(if_stmt) => {
                collect_top_level_successors_from_stmts(
                    &if_stmt.body.body,
                    label_to_index,
                    seen,
                    out,
                );
                if let Some(term) = &if_stmt.body.term {
                    collect_top_level_successors_from_term(term, label_to_index, seen, out);
                }
                collect_top_level_successors_from_stmts(
                    &if_stmt.orelse.body,
                    label_to_index,
                    seen,
                    out,
                );
                if let Some(term) = &if_stmt.orelse.term {
                    collect_top_level_successors_from_term(term, label_to_index, seen, out);
                }
            }
            BlockPyStmt::Pass
            | BlockPyStmt::Assign(_)
            | BlockPyStmt::Expr(_)
            | BlockPyStmt::Delete(_) => {}
        }
    }
}

fn collect_top_level_successors_from_term(
    term: &BlockPyTerm<impl Clone + Into<Expr>>,
    label_to_index: &HashMap<String, usize>,
    seen: &mut HashSet<usize>,
    out: &mut Vec<usize>,
) {
    match term {
        BlockPyTerm::Jump(label) => {
            push_top_level_successor(label, label_to_index, seen, out);
        }
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            then_label,
            else_label,
            ..
        }) => {
            push_top_level_successor(then_label, label_to_index, seen, out);
            push_top_level_successor(else_label, label_to_index, seen, out);
        }
        BlockPyTerm::BranchTable(branch) => {
            for label in &branch.targets {
                push_top_level_successor(label, label_to_index, seen, out);
            }
            push_top_level_successor(&branch.default_label, label_to_index, seen, out);
        }
        BlockPyTerm::TryJump(try_jump) => {
            push_top_level_successor(&try_jump.body_label, label_to_index, seen, out);
            push_top_level_successor(&try_jump.except_label, label_to_index, seen, out);
        }
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {}
    }
}

fn push_top_level_successor(
    label: &BlockPyLabel,
    label_to_index: &HashMap<String, usize>,
    seen: &mut HashSet<usize>,
    out: &mut Vec<usize>,
) {
    let Some(successor_index) = label_to_index.get(label.as_str()) else {
        return;
    };
    if seen.insert(*successor_index) {
        out.push(*successor_index);
    }
}

fn collect_predecessors(successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut predecessors = vec![Vec::new(); successors.len()];
    for (source_index, targets) in successors.iter().enumerate() {
        for target_index in targets {
            predecessors[*target_index].push(source_index);
        }
    }
    predecessors
}

fn collect_discovery_order(entry_index: usize, successors: &[Vec<usize>]) -> Vec<usize> {
    fn visit(
        block_index: usize,
        successors: &[Vec<usize>],
        visited: &mut HashSet<usize>,
        order: &mut Vec<usize>,
    ) {
        if !visited.insert(block_index) {
            return;
        }
        order.push(block_index);
        for successor_index in &successors[block_index] {
            visit(*successor_index, successors, visited, order);
        }
    }

    let mut visited = HashSet::new();
    let mut order = Vec::new();
    visit(entry_index, successors, &mut visited, &mut order);
    order
}

fn compute_dominators(
    entry_index: usize,
    discovery_order: &[usize],
    predecessors: &[Vec<usize>],
    reachable: &HashSet<usize>,
) -> Vec<HashSet<usize>> {
    let mut dominators = vec![HashSet::new(); predecessors.len()];
    let all_reachable = reachable.iter().copied().collect::<HashSet<_>>();
    for block_index in discovery_order {
        if *block_index == entry_index {
            dominators[*block_index].insert(*block_index);
        } else {
            dominators[*block_index] = all_reachable.clone();
        }
    }

    loop {
        let mut changed = false;
        for block_index in discovery_order
            .iter()
            .copied()
            .filter(|block_index| *block_index != entry_index)
        {
            let mut reachable_predecessors = predecessors[block_index]
                .iter()
                .copied()
                .filter(|predecessor| reachable.contains(predecessor));
            let Some(first_predecessor) = reachable_predecessors.next() else {
                let mut singleton = HashSet::new();
                singleton.insert(block_index);
                if dominators[block_index] != singleton {
                    dominators[block_index] = singleton;
                    changed = true;
                }
                continue;
            };

            let mut new_dominators = dominators[first_predecessor].clone();
            for predecessor in reachable_predecessors {
                new_dominators = new_dominators
                    .intersection(&dominators[predecessor])
                    .copied()
                    .collect();
            }
            new_dominators.insert(block_index);

            if dominators[block_index] != new_dominators {
                dominators[block_index] = new_dominators;
                changed = true;
            }
        }

        if !changed {
            return dominators;
        }
    }
}

fn compute_immediate_dominators(
    entry_index: usize,
    discovery_order: &[usize],
    dominators: &[HashSet<usize>],
    reachable: &HashSet<usize>,
) -> Vec<Option<usize>> {
    let mut immediate_dominators = vec![None; dominators.len()];
    for block_index in discovery_order
        .iter()
        .copied()
        .filter(|block_index| *block_index != entry_index)
    {
        let strict_dominators = dominators[block_index]
            .iter()
            .copied()
            .filter(|dominator| *dominator != block_index && reachable.contains(dominator))
            .collect::<Vec<_>>();
        let immediate_dominator = strict_dominators.iter().copied().find(|candidate| {
            strict_dominators
                .iter()
                .all(|other| *other == *candidate || dominators[*candidate].contains(other))
        });
        immediate_dominators[block_index] = immediate_dominator;
    }
    immediate_dominators
}

fn collect_referenced_labels_from_blocks(
    blocks: &[BlockPyBlock<impl Clone + Into<Expr>>],
) -> HashSet<BlockPyLabel> {
    let mut referenced = HashSet::new();
    for block in blocks {
        collect_referenced_labels_from_stmts(&block.body, &mut referenced);
        collect_referenced_labels_from_term(&block.term, &mut referenced);
    }
    referenced
}

fn collect_referenced_labels_from_stmts(
    stmts: &[BlockPyStmt<impl Clone + Into<Expr>>],
    referenced: &mut HashSet<BlockPyLabel>,
) {
    for stmt in stmts {
        match stmt {
            BlockPyStmt::If(if_stmt) => {
                collect_referenced_labels_from_stmts(&if_stmt.body.body, referenced);
                if let Some(term) = &if_stmt.body.term {
                    collect_referenced_labels_from_term(term, referenced);
                }
                collect_referenced_labels_from_stmts(&if_stmt.orelse.body, referenced);
                if let Some(term) = &if_stmt.orelse.term {
                    collect_referenced_labels_from_term(term, referenced);
                }
            }
            _ => {}
        }
    }
}

fn collect_referenced_labels_from_term(
    term: &BlockPyTerm<impl Clone + Into<Expr>>,
    referenced: &mut HashSet<BlockPyLabel>,
) {
    match term {
        BlockPyTerm::Jump(label) => {
            referenced.insert(label.clone());
        }
        BlockPyTerm::IfTerm(if_term) => {
            referenced.insert(if_term.then_label.clone());
            referenced.insert(if_term.else_label.clone());
        }
        BlockPyTerm::BranchTable(branch) => {
            referenced.extend(branch.targets.iter().cloned());
            referenced.insert(branch.default_label.clone());
        }
        BlockPyTerm::TryJump(try_jump) => {
            referenced.insert(try_jump.body_label.clone());
            referenced.insert(try_jump.except_label.clone());
        }
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::bb_ir::{BbClosureInit, BbClosureLayout, BbClosureSlot};
    use crate::basic_block::block_py::BlockPyBlockMeta;
    use ruff_python_parser::{parse_expression, parse_module};

    fn wrapped_blockpy(source: &str) -> BlockPyModule {
        crate::transform_str_to_ruff_with_options(source, crate::Options::for_test())
            .unwrap()
            .blockpy_module
            .expect("expected BlockPy module")
    }

    fn parse_blockpy_expr(source: &str) -> BlockPyExpr {
        (*parse_expression(source).unwrap().into_syntax().body).into()
    }

    fn empty_parameters() -> ast::Parameters {
        let body = parse_module(
            r#"
def f():
    pass
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(function_def) = &**body.body.iter().next().unwrap() else {
            unreachable!("expected parsed helper function")
        };
        *function_def.parameters.clone()
    }

    #[test]
    fn renders_blockpy_module_with_module_init_and_nested_blocks() {
        let blockpy = wrapped_blockpy(
            r#"
seed = 1

def classify(a, /, b: int = 1, *args, c=2, **kwargs):
    if a:
        return "yes"
    return "no"
"#,
        );
        let rendered = blockpy_module_to_string(&blockpy);

        assert!(rendered.contains("module_init: _dp_module_init"));
        assert!(rendered.contains(
            "function classify(a, /, b: int = 1, *args, c = 2, **kwargs)\n    kind: function\n    bind: classify\n    qualname: classify"
        ));
        assert!(rendered.contains("function _dp_module_init()"));
        assert!(rendered.contains("block start:"));
        assert!(rendered.contains("if_term a:"));
        assert!(rendered.contains("return \"yes\""));
    }

    #[test]
    fn renders_empty_module_marker() {
        let rendered = blockpy_module_to_string(&BlockPyModule::<BlockPyExpr> {
            module_init: None,
            callable_defs: Vec::new(),
        });
        assert_eq!(rendered, "; empty BlockPy module\n");
    }

    #[test]
    fn transformed_lowering_result_exposes_module_init_blockpy() {
        let lowered = crate::transform_str_to_ruff_with_options(
            r#"
def classify(n):
    if n < 0:
        return "neg"
    return "pos"
"#,
            crate::Options::default(),
        )
        .unwrap();
        let blockpy = lowered.blockpy_module.expect("expected BlockPy module");
        let rendered = blockpy_module_to_string(&blockpy);

        assert_eq!(blockpy.module_init.as_deref(), Some("_dp_module_init"));
        assert!(rendered.contains("function _dp_module_init()"));
    }

    #[test]
    fn renders_generator_kind_without_internal_metadata() {
        let blockpy = wrapped_blockpy(
            r#"
def gen():
    yield 1
"#,
        );
        let rendered = blockpy_module_to_string(&blockpy);

        assert!(rendered.contains("function gen()\n    kind: function"));
        assert!(rendered.contains("function gen_resume()\n    kind: generator"));
        assert!(!rendered.contains("generator_state:"));
    }

    #[test]
    fn renders_public_closure_metadata_in_function_header() {
        let rendered = blockpy_module_to_string(&BlockPyModule::<BlockPyExpr> {
            module_init: None,
            callable_defs: vec![BlockPyCallableDef {
                cfg: crate::basic_block::cfg_ir::CfgCallableDef {
                    function_id: crate::basic_block::bb_ir::FunctionId(0),
                    bind_name: "gen".to_string(),
                    display_name: "gen".to_string(),
                    qualname: "gen".to_string(),
                    kind: BlockPyFunctionKind::Function,
                    params: empty_parameters(),
                    entry_liveins: vec!["_dp_self".to_string(), "_dp_resume_exc".to_string()],
                    blocks: vec![BlockPyBlock {
                        label: "gen_start".into(),
                        body: vec![],
                        term: BlockPyTerm::<BlockPyExpr>::Return(None),
                        meta: BlockPyBlockMeta::default(),
                    }],
                },
                doc: None,
                closure_layout: Some(BbClosureLayout {
                    freevars: vec![BbClosureSlot {
                        logical_name: "factor".to_string(),
                        storage_name: "_dp_cell_factor".to_string(),
                        init: BbClosureInit::InheritedCapture,
                    }],
                    cellvars: vec![BbClosureSlot {
                        logical_name: "total".to_string(),
                        storage_name: "_dp_cell_total".to_string(),
                        init: BbClosureInit::Deferred,
                    }],
                    runtime_cells: vec![BbClosureSlot {
                        logical_name: "_dp_pc".to_string(),
                        storage_name: "_dp_cell__dp_pc".to_string(),
                        init: BbClosureInit::RuntimePcUnstarted,
                    }],
                }),
                local_cell_slots: vec!["_dp_cell__dp_pc".to_string()],
            }],
        });

        assert!(rendered.contains(
            "function gen()\n    kind: function\n    bind: gen\n    qualname: gen\n    entry_liveins: [_dp_self, _dp_resume_exc]\n    local_cell_slots: [_dp_cell__dp_pc]\n    freevars: [factor->_dp_cell_factor@inherited]\n    cellvars: [total->_dp_cell_total@deferred]\n    runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted]"
        ));
        assert!(!rendered.contains("entry:"));
    }

    #[test]
    fn renders_followup_blocks_under_their_owning_entry_block() {
        let function = BlockPyCallableDef {
            cfg: crate::basic_block::cfg_ir::CfgCallableDef {
                function_id: crate::basic_block::bb_ir::FunctionId(0),
                bind_name: "f".to_string(),
                display_name: "f".to_string(),
                qualname: "f".to_string(),
                kind: BlockPyFunctionKind::Function,
                params: empty_parameters(),
                entry_liveins: Vec::new(),
                blocks: vec![
                    BlockPyBlock {
                        label: "start".into(),
                        body: vec![],
                        term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                            test: parse_blockpy_expr("cond"),
                            then_label: "then".into(),
                            else_label: "else".into(),
                        }),
                        meta: BlockPyBlockMeta::default(),
                    },
                    BlockPyBlock {
                        label: "then".into(),
                        body: vec![BlockPyStmt::Expr(parse_blockpy_expr("then_side_effect()"))],
                        term: BlockPyTerm::Jump("after".into()),
                        meta: BlockPyBlockMeta::default(),
                    },
                    BlockPyBlock {
                        label: "else".into(),
                        body: vec![BlockPyStmt::Expr(parse_blockpy_expr("else_side_effect()"))],
                        term: BlockPyTerm::Jump("after".into()),
                        meta: BlockPyBlockMeta::default(),
                    },
                    BlockPyBlock {
                        label: "after".into(),
                        body: vec![BlockPyStmt::Expr(parse_blockpy_expr("finish()"))],
                        term: BlockPyTerm::Return(None),
                        meta: BlockPyBlockMeta::default(),
                    },
                ],
            },
            doc: None,
            closure_layout: None,
            local_cell_slots: Vec::new(),
        };
        let rendered = blockpy_module_to_string(&BlockPyModule {
            module_init: None,
            callable_defs: vec![function],
        });

        assert!(rendered.contains("    block start:\n"));
        assert!(rendered.contains("        block after:\n"));
        assert!(rendered.contains(
            "        if_term cond:\n            then:\n                block then:\n                    then_side_effect()\n                    jump after\n            else:\n                block else:\n                    else_side_effect()\n                    jump after\n        block after:\n            finish()\n            return\n"
        ));
    }

    #[test]
    fn elides_trivial_if_term_jump_wrappers_when_rendering() {
        let blockpy = wrapped_blockpy(
            r#"
def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b
"#,
        );
        let rendered = blockpy_module_to_string(&blockpy);

        assert!(rendered.contains("return a"), "{rendered}");
        assert!(rendered.contains("return b"), "{rendered}");
        assert!(!rendered.contains("block _dp_bb_choose_1_then"));
        assert!(!rendered.contains("block _dp_bb_choose_1_else"));
    }

    #[test]
    fn sorts_rendered_root_and_child_blocks_by_label() {
        let function: BlockPyCallableDef<BlockPyExpr> = BlockPyCallableDef {
            cfg: crate::basic_block::cfg_ir::CfgCallableDef {
                function_id: crate::basic_block::bb_ir::FunctionId(0),
                bind_name: "f".to_string(),
                display_name: "f".to_string(),
                qualname: "f".to_string(),
                kind: BlockPyFunctionKind::Function,
                params: empty_parameters(),
                entry_liveins: Vec::new(),
                blocks: vec![
                    BlockPyBlock {
                        label: "start".into(),
                        body: vec![],
                        term: BlockPyTerm::TryJump(BlockPyTryJump {
                            body_label: "zeta".into(),
                            except_label: "alpha".into(),
                        }),
                        meta: BlockPyBlockMeta::default(),
                    },
                    BlockPyBlock {
                        label: "zeta".into(),
                        body: vec![BlockPyStmt::Pass],
                        term: BlockPyTerm::Return(None),
                        meta: BlockPyBlockMeta::default(),
                    },
                    BlockPyBlock {
                        label: "alpha".into(),
                        body: vec![BlockPyStmt::Pass],
                        term: BlockPyTerm::Return(None),
                        meta: BlockPyBlockMeta::default(),
                    },
                    BlockPyBlock {
                        label: "omega".into(),
                        body: vec![BlockPyStmt::Pass],
                        term: BlockPyTerm::Return(None),
                        meta: BlockPyBlockMeta::default(),
                    },
                    BlockPyBlock {
                        label: "beta".into(),
                        body: vec![BlockPyStmt::Pass],
                        term: BlockPyTerm::Return(None),
                        meta: BlockPyBlockMeta::default(),
                    },
                ],
            },
            doc: None,
            closure_layout: None,
            local_cell_slots: Vec::new(),
        };
        let rendered = blockpy_module_to_string(&BlockPyModule {
            module_init: None,
            callable_defs: vec![function],
        });

        let alpha_pos = rendered.find("block alpha:").expect("alpha block");
        let zeta_pos = rendered.find("block zeta:").expect("zeta block");
        let beta_pos = rendered.find("block beta:").expect("beta block");
        let omega_pos = rendered.find("block omega:").expect("omega block");

        assert!(alpha_pos < zeta_pos, "{rendered}");
        assert!(beta_pos < omega_pos, "{rendered}");
    }
}
