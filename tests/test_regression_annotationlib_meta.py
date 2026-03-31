def test_metaclass_annotate_masking(run_integration_module):
    with run_integration_module("annotationlib_meta") as module:
        ann_meta, ann_x, ann_y, meta_annotate, x_annotate, y_annotate = module.run()
        assert ann_meta == {"a": int}
        assert ann_x == {}
        assert ann_y == {"b": float}
        assert meta_annotate == {"a": int}
        assert x_annotate is None
        assert y_annotate == {"b": float}
