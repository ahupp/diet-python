"""Example demonstrating the diet-python import hook."""

import dis
import shutil
from pathlib import Path

import diet_import_hook

# Ensure the module is recompiled so the hook runs
shutil.rmtree(Path(__file__).with_name("__pycache__"), ignore_errors=True)

diet_import_hook.install()

import example_module

bytecode = list(dis.Bytecode(example_module.add))
opnames = [instr.opname for instr in bytecode]
assert "BINARY_OP" not in opnames, "diet-python import hook did not transform +"
assert any(
    instr.opname == "LOAD_GLOBAL" and instr.argval == "operator" for instr in bytecode
), "diet-python import hook did not insert operator.add call"

print("diet-python import hook transformed example_module.add")
