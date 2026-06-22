#!/usr/bin/env python3
"""Run in a task dir. Print the FAILURE SIGNATURE: per-assert pass(1)/fail(0) bits of check.py against
the buggy code here. AST-rewrites each `assert` into a non-raising recorder. Diagnostics -> stderr."""
import ast, sys, io, os
sys.path.insert(0, os.getcwd())

def build(src):
    tree = ast.parse(src)
    class T(ast.NodeTransformer):
        def visit_Assert(self, node):
            rec = ast.parse("try:\n __R.append(1 if (None) else 0)\nexcept Exception:\n __R.append(0)").body[0]
            rec.body[0].value.args[0] = ast.IfExp(test=node.test, body=ast.Constant(1), orelse=ast.Constant(0))
            return ast.copy_location(rec, node)
        def visit_Call(self, node):
            self.generic_visit(node)
            if isinstance(node.func, ast.Attribute) and node.func.attr == "exit":
                return ast.Constant(None)
            return node
    tree = T().visit(tree)
    ast.fix_missing_locations(tree)
    return tree

src = open("check.py").read()
tree = build(src)
ns = {"__R": []}
real = sys.stdout
sys.stdout = io.StringIO()
try:
    exec(compile(tree, "<sig>", "exec"), ns)
except BaseException as e:
    sys.stdout = real
    print("ERR:" + repr(e), file=sys.stderr)
finally:
    sys.stdout = real
print("".join(str(b) for b in ns["__R"]))
