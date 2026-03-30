"""Python support package for the SOAC transformed runtime."""

try:
    import _soac_ext as _soac_ext
except Exception as err:
    err.add_note(
        "soac requires the native extension '_soac_ext'; "
        "run 'just build-all' or 'just build-extension <debug|release>'"
    )
    raise

__all__ = ["_soac_ext"]
