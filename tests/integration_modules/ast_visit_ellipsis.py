import ast


def visit_ellipsis():
    log = []

    class Visitor(ast.NodeVisitor):
        def visit_Ellipsis(self, node):
            log.append(("Ellipsis", ...))

    mod = ast.parse("e = ...")
    Visitor().visit(mod)
    return log
