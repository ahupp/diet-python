import ast


def visit_ellipsis():
    log = []

    class Visitor(ast.NodeVisitor):
        def visit_Constant(self, node):
            if node.value is Ellipsis:
                log.append(("Ellipsis", ...))

    mod = ast.parse("e = ...")
    Visitor().visit(mod)
    return log
