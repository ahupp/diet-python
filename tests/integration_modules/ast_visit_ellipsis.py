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

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.visit_ellipsis() == [("Ellipsis", ...)]
