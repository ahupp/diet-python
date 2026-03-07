class Once:
    def __await__(self):
        yield 'tick'
        return 5

def make_runner(delta):
    outer = delta
    async def run():
        total = 1
        total += outer
        total += await Once()
        return total
    return run()
