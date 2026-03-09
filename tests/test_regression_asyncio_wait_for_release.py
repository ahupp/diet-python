from __future__ import annotations

from textwrap import dedent

from tests._integration import transformed_module


def test_wait_for_timeout_releases_payload(tmp_path):
    source = dedent(
        """
        from __future__ import annotations

        import asyncio
        import gc
        import weakref


        class Payload:
            pass


        async def hold_ref(ref_holder):
            obj = Payload()
            ref_holder.append(weakref.ref(obj))
            await asyncio.sleep(10)


        def leak_check():
            ref_holder = []

            async def runner():
                await asyncio.wait_for(hold_ref(ref_holder), 0.01)

            try:
                asyncio.run(runner())
            except asyncio.TimeoutError:
                pass

            gc.collect()
            return ref_holder[0]()
        """
    )

    with transformed_module(tmp_path, "asyncio_wait_for_release_regression", source) as module:
        assert module.leak_check() is None
