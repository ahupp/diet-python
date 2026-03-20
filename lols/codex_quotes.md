• I’ve confirmed load_name_hook is doing the slowest possible thing today:

That lols/async_with.txt dump is the right model: it already shows yield from lowered as explicit blocks for
  GeneratorExit, throw, close, send, and the per-call exception edges. I’m checking one last detail now: how
  best to extract StopIteration.value in those exception blocks.