import asyncio


async def run():
    async def handler(reader, writer):
        writer.close()
        await writer.wait_closed()

    try:
        server = await asyncio.start_server(handler, "127.0.0.1", 0)
    except Exception as exc:  # pragma: no cover - environment-dependent
        return f"{type(exc).__name__}: {exc}"

    server.close()
    await server.wait_closed()
    return "ok"


RESULT = asyncio.run(run())
