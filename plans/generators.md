
We need to add support for generators.  At the moment, in rewrite_module_with_tracker, the value `core_blockpy_without_await` contains functions that have 'await' reduced to
`yield from _dp_some_helper`.  We need to reduce those to regular functions that implement the generator protocol

It is very important, most important, that the generator transform only consume that value as input (a BlockPyModule<CoreBlockPyPassWithoutAwait>), and returns a BlockPyModule<CoreBlockPyPassWithoutAwaitOrYield>.

If there is insufficient information in the input, stop and describe the issue before proceeding.

The structure of generators will be to have a new, generated, outer function (closure) that holds all the internal state in cells, as well as all locals of the generator.  This function will be "resume", and take two values, "send_value" and "throw_value", where at most one is non-none.  The most important cell is "_dp_pc", the program counter indicating which yield point we're on.

Split the generator blocks at yield points, and map each resume point to a PC.  Then, a yield looks like:

_dp_pc = <next pc>
return value

throw is similar, but throws in a block wrapped by the corresponding exception block.  Be sure to handle all the nuances of generators with e.g throwing an exception, StopIteration etc.



